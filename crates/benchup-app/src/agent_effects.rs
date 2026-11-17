//! Session-only agent feedback. Geometry is captured once per committed edit;
//! cameras, the robot, scanning and digital assembly animate on the GPU.
use super::*;
use std::collections::BTreeSet;
use wgpu::util::DeviceExt;

const LIFETIME: f64 = 0.95;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Mode {
    Looking,
    Preparing,
}

#[derive(Default)]
pub(crate) struct State {
    starts: Arc<BTreeMap<u64, f64>>,
    ghosts: Vec<(f64, Arc<Ghost>)>,
}

#[derive(Clone, Default)]
pub(crate) struct Frame {
    pub starts: Arc<BTreeMap<u64, f64>>,
    ghosts: Vec<(f64, Arc<Ghost>)>,
    pub now: f64,
    pub activity: Option<(Mode, f64)>,
    pub bounds: Option<(DVec3, DVec3)>,
}

pub(crate) fn timestamp(now: f64) -> f32 {
    now.rem_euclid(4096.) as f32
}

/// A prepared change is installed only after the model commit succeeds.
pub(crate) struct Change {
    ids: BTreeSet<u64>,
    ghost: Option<Arc<Ghost>>,
}
impl Change {
    pub(crate) fn between(
        before: &Model,
        after: &Model,
        display: &benchup_core::DisplaySettings,
    ) -> Self {
        let vertices = changed_ids(
            before.vertices.iter().map(|(&id, v)| (id, v)),
            after.vertices.iter().map(|(&id, v)| (id, v)),
        );
        let mut ids = changed_ids(
            before.faces.iter().map(|(&id, v)| (id, v)),
            after.faces.iter().map(|(&id, v)| (id, v)),
        );
        ids.extend(changed_ids(
            before.edges.iter().map(|(&id, v)| (id, v)),
            after.edges.iter().map(|(&id, v)| (id, v)),
        ));
        ids.extend(changed_ids(
            before.face_colors.iter().map(|(&id, v)| (id, v)),
            after.face_colors.iter().map(|(&id, v)| (id, v)),
        ));
        // A move can keep every face/edge ID while changing just its vertices.
        for model in [before, after] {
            ids.extend(
                model
                    .faces
                    .values()
                    .filter(|face| {
                        face.vertices
                            .iter()
                            .chain(face.holes.iter().flatten())
                            .any(|id| vertices.contains(id))
                    })
                    .map(|face| face.id),
            );
            ids.extend(
                model
                    .edges
                    .values()
                    .filter(|edge| vertices.contains(&edge.a) || vertices.contains(&edge.b))
                    .map(|edge| edge.id),
            );
        }
        // Membership and Outliner metadata can change on an ancestor when a
        // single child is added/erased. They do not change surviving geometry.
        let mut objects: BTreeSet<_> = changed_ids(
            before.objects.iter().map(|(&id, v)| (id, v)),
            after.objects.iter().map(|(&id, v)| (id, v)),
        )
        .into_iter()
        .filter(|id| match (before.objects.get(id), after.objects.get(id)) {
            (Some(a), Some(b)) => {
                a.transform != b.transform
                    || crate::app::surface_appearance::appearance_changed(a, b)
            }
            _ => false,
        })
        .collect();
        let joints = changed_ids(
            before.connections.iter().map(|(&id, v)| (id, v)),
            after.connections.iter().map(|(&id, v)| (id, v)),
        );
        let routes = changed_ids(
            before.routes.iter().map(|(&id, v)| (id, v)),
            after.routes.iter().map(|(&id, v)| (id, v)),
        );
        for model in [before, after] {
            for joint in joints.iter().filter_map(|id| model.connections.get(id)) {
                objects.extend(joint.members);
            }
            objects.extend(routes.iter().filter_map(|id| model.parent(*id)));
            objects.extend(model.objects.values().filter_map(|object| {
                let material = object.stock_material_id?;
                (before.stock_materials.get(&material) != after.stock_materials.get(&material))
                    .then_some(object.id)
            }));
            ids.extend(model.expand_entities(&objects));
        }
        // Tag visibility/hide operations also visibly add or remove geometry.
        let mut visibility: BTreeSet<_> = before
            .hidden
            .symmetric_difference(&after.hidden)
            .copied()
            .collect();
        let tags = changed_ids(
            before.tags.iter().map(|(&id, v)| (id, v)),
            after.tags.iter().map(|(&id, v)| (id, v)),
        );
        visibility.extend(changed_ids(
            before.entity_tags.iter().map(|(&id, v)| (id, v)),
            after.entity_tags.iter().map(|(&id, v)| (id, v)),
        ));
        for model in [before, after] {
            visibility.extend(
                model
                    .entity_tags
                    .iter()
                    .filter(|(_, tag)| tags.contains(*tag))
                    .map(|(&id, _)| id),
            );
            ids.extend(model.expand_entities(&visibility));
        }
        let ghost = Ghost::new(before, &ids, display).map(Arc::new);
        Self { ids, ghost }
    }
}
impl State {
    pub(crate) fn record(&mut self, change: Change, now: f64) {
        self.expire(now);
        // Rapid edits of the same geometry replace its prior departure instead
        // of stacking full overlapping snapshots for the entire animation.
        self.ghosts
            .retain(|(_, ghost)| !ghost.ids.is_subset(&change.ids));
        let starts = Arc::make_mut(&mut self.starts);
        for id in change.ids {
            starts.insert(id, now);
        }
        if let Some(ghost) = change.ghost {
            self.ghosts.push((now, ghost));
        }
    }
    fn expire(&mut self, now: f64) {
        if self.starts.values().any(|at| now - at >= LIFETIME) {
            Arc::make_mut(&mut self.starts).retain(|_, at| now - *at < LIFETIME);
        }
        self.ghosts.retain(|(at, _)| now - at < LIFETIME);
    }
    pub(crate) fn frame(
        &mut self,
        now: f64,
        activity: Option<(Mode, f64)>,
        bounds: Option<(DVec3, DVec3)>,
    ) -> Frame {
        self.expire(now);
        Frame {
            starts: self.starts.clone(),
            ghosts: self.ghosts.clone(),
            now,
            activity,
            bounds,
        }
    }
}
impl Frame {
    pub(crate) fn stamp(&self, id: u64) -> f32 {
        self.starts.get(&id).map_or(-1., |at| timestamp(*at))
    }
    /// Precision meshes call this only while rebuilding after an edit. Retained
    /// geometry instead inherits object timestamps through its state texture.
    pub(crate) fn remap(&mut self, model: &Model) {
        if self.starts.is_empty() {
            return;
        }
        let mut inherited = Vec::new();
        for (&id, &at) in self.starts.iter() {
            if model.objects.contains_key(&id) {
                inherited.extend(
                    model
                        .expand_entities(&BTreeSet::from([id]))
                        .into_iter()
                        .map(|id| (id, at)),
                );
            }
            if let Some(derived) = model.routing_appearance.by_source.get(&id) {
                inherited.extend(derived.iter().map(|id| (*id, at)));
            }
        }
        if !inherited.is_empty() {
            let starts = Arc::make_mut(&mut self.starts);
            for (id, at) in inherited {
                starts
                    .entry(id)
                    .and_modify(|previous| *previous = previous.max(at))
                    .or_insert(at);
            }
        }
    }
    pub(crate) fn parameters(&self, camera: &Camera, rect: Rect) -> [f32; 4] {
        let mut parameters = [timestamp(self.now), -1., 0., 1.];
        if let (Some((Mode::Looking, at)), Some((lo, hi))) = (self.activity, self.bounds) {
            let size = DVec2::new(rect.width() as f64, rect.height() as f64);
            let mut min = 1_f32;
            let mut max = 0_f32;
            for x in [lo.x, hi.x] {
                for y in [lo.y, hi.y] {
                    for z in [lo.z, hi.z] {
                        if let Some(p) = camera.project(DVec3::new(x, y, z), size) {
                            let y = p.y as f32 / rect.height().max(1.);
                            min = min.min(y);
                            max = max.max(y);
                        }
                    }
                }
            }
            min = min.clamp(0., 1.);
            max = max.clamp(0., 1.);
            // A cosine sweep starts at the bottom: up, down, then up.
            let sweep = (1. - ((self.now - at) as f32 * std::f32::consts::PI).cos()) * 0.5;
            parameters[1] = max + (min - max) * sweep;
            parameters[2] = min;
            parameters[3] = max;
        }
        parameters
    }
    pub(crate) fn animated(&self) -> bool {
        self.activity.is_some() || !self.starts.is_empty() || !self.ghosts.is_empty()
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GhostVertex {
    a: [f32; 3],
    b: [f32; 3],
    corner: [f32; 2],
    kind: u32,
}
struct Ghost {
    vertices: Vec<GhostVertex>,
    ids: BTreeSet<u64>,
    origin: DVec3,
    gpu: OnceLock<Vec<(wgpu::Buffer, u32)>>,
}
impl Ghost {
    fn new(
        model: &Model,
        ids: &BTreeSet<u64>,
        display: &benchup_core::DisplaySettings,
    ) -> Option<Self> {
        let (lo, hi) = model.selection_bounds(ids)?;
        let origin = (lo + hi) * 0.5;
        let mut entities = EntityCache::default();
        entities.update(model);
        let mut vertices = Vec::new();
        let mut rendered = BTreeSet::new();
        for id in ids
            .iter()
            .filter(|id| entities.get(**id).displayed(display))
        {
            let start = vertices.len();
            if let Some(face) = model
                .faces
                .get(id)
                .filter(|_| display.face_style != benchup_core::FaceStyle::Wireframe)
            {
                for triangle in model.face_triangles(face) {
                    // Honor active section planes in departing snapshots too.
                    let (polygon, len) = display.section_triangle(triangle);
                    for i in 1..len.saturating_sub(1) {
                        for p in [polygon[0], polygon[i], polygon[i + 1]] {
                            vertices.push(GhostVertex {
                                a: (p - origin).as_vec3().to_array(),
                                b: [0.; 3],
                                corner: [0.; 2],
                                kind: 0,
                            });
                        }
                    }
                }
            }
            if let Some(edge) = model.edges.get(id) {
                let (a, b) = model.points(edge);
                if let Some((a, b)) = display.section_segment(a, b) {
                    for corner in [
                        [0., -1.],
                        [1., -1.],
                        [0., 1.],
                        [0., 1.],
                        [1., -1.],
                        [1., 1.],
                    ] {
                        vertices.push(GhostVertex {
                            a: (a - origin).as_vec3().to_array(),
                            b: (b - origin).as_vec3().to_array(),
                            corner,
                            kind: 1,
                        });
                    }
                }
            }
            if vertices.len() != start {
                rendered.insert(*id);
            }
        }
        (!vertices.is_empty()).then_some(Self {
            vertices,
            ids: rendered,
            origin,
            gpu: OnceLock::new(),
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    matrix: [[f32; 4]; 4],
    viewport: [f32; 4],
    robot: [f32; 4],
    animation: [f32; 4],
    paper: [f32; 4],
}
struct Pipelines {
    ghost: wgpu::RenderPipeline,
    robot: wgpu::RenderPipeline,
}
type UniformBuffers = (wgpu::Buffer, wgpu::BindGroup);
type UniformPool = Arc<Mutex<Vec<UniformBuffers>>>;
#[derive(Default)]
pub(crate) struct Cache {
    robot: UniformPool,
    ghost: UniformPool,
}
struct Draw {
    pool: UniformPool,
    ghost: Option<Arc<Ghost>>,
    uniform: Uniform,
    gpu: OnceLock<(wgpu::Buffer, wgpu::BindGroup)>,
}

pub(crate) fn paint(
    painter: &egui::Painter,
    rect: Rect,
    camera: &Camera,
    frame: &Frame,
    paper: Color32,
    cache: &Cache,
) {
    let size = DVec2::new(rect.width() as f64, rect.height() as f64);
    let base = |origin| Uniform {
        matrix: (camera.matrix(size) * glam::DMat4::from_translation(origin))
            .as_mat4()
            .to_cols_array_2d(),
        viewport: [
            rect.width(),
            rect.height(),
            painter.ctx().pixels_per_point(),
            0.,
        ],
        robot: [0.; 4],
        animation: [0.; 4],
        paper: paper.to_array().map(|v| v as f32 / 255.),
    };
    for (at, ghost) in &frame.ghosts {
        let mut uniform = base(ghost.origin);
        uniform.animation[0] = ((frame.now - at) / LIFETIME).clamp(0., 1.) as f32;
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Draw {
                pool: cache.ghost.clone(),
                ghost: Some(ghost.clone()),
                uniform,
                gpu: OnceLock::new(),
            },
        ));
    }
    if let Some((mode, at)) = frame.activity {
        let mut uniform = base(DVec3::ZERO);
        let mut right = rect.width() * 0.5;
        let mut top = rect.height() * 0.5;
        if let Some((lo, hi)) = frame.bounds {
            right = f32::NEG_INFINITY;
            top = f32::INFINITY;
            for x in [lo.x, hi.x] {
                for y in [lo.y, hi.y] {
                    for z in [lo.z, hi.z] {
                        if let Some(p) = camera.project(DVec3::new(x, y, z), size) {
                            right = right.max(p.x as f32);
                            top = top.min(p.y as f32);
                        }
                    }
                }
            }
        }
        // A bounded billboard sits beside the assembly and remains in the pane.
        let x = (right + 40.).clamp(44., (rect.width() - 44.).max(44.));
        let y = (top + 30.).clamp(44., (rect.height() - 44.).max(44.));
        uniform.robot = [x, y, 38., if mode == Mode::Preparing { 1. } else { 0. }];
        uniform.animation[1] = (frame.now - at) as f32;
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Draw {
                pool: cache.robot.clone(),
                ghost: None,
                uniform,
                gpu: OnceLock::new(),
            },
        ));
    }
    if frame.animated() {
        painter
            .ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub(crate) fn install(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    renderer: &mut egui_wgpu::Renderer,
) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Agent robot and digital transitions"),
        source: wgpu::ShaderSource::Wgsl(
            concat!(
                include_str!("surface_appearance.wgsl"),
                "\n",
                include_str!("agent_effects.wgsl"),
                "\n",
                include_str!("agent_robot.wgsl")
            )
            .into(),
        ),
    });
    let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Uint32];
    let buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GhostVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &attributes,
    })];
    let pipeline = |robot| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(if robot {
                "Agent robot"
            } else {
                "Agent departing geometry"
            }),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(if robot {
                    "robot_vertex"
                } else {
                    "ghost_vertex"
                }),
                buffers: if robot { &[] } else { &buffers },
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(if robot { "robot_color" } else { "ghost_color" }),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    renderer.callback_resources.insert(Pipelines {
        ghost: pipeline(false),
        robot: pipeline(true),
    });
    renderer
        .callback_resources
        .insert(TargetSrgb(format.is_srgb()));
}
impl egui_wgpu::CallbackTrait for Draw {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(pipelines) = resources.get::<Pipelines>() {
            self.gpu.get_or_init(|| {
                let mut uniform = self.uniform;
                // egui's callback target can be either linear or sRGB.
                // The renderer records that bit in the shared viewport pipeline.
                uniform.viewport[3] = if resources.get::<TargetSrgb>().is_some_and(|v| v.0) {
                    1.
                } else {
                    0.
                };
                if let Some((buffer, bind)) = self.pool.lock().ok().and_then(|mut pool| pool.pop())
                {
                    queue.write_buffer(&buffer, 0, bytemuck::bytes_of(&uniform));
                    return (buffer, bind);
                }
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Agent animation parameters"),
                    contents: bytemuck::bytes_of(&uniform),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
                let pipeline = if self.ghost.is_some() {
                    &pipelines.ghost
                } else {
                    &pipelines.robot
                };
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Agent animation"),
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                });
                (buffer, bind)
            });
            if let Some(ghost) = &self.ghost {
                ghost.gpu.get_or_init(|| {
                    // Large assemblies must also fit WebGL's buffer limit.
                    let limit = device.limits().max_buffer_size.min(16 * 1024 * 1024);
                    let count =
                        (limit as usize / std::mem::size_of::<GhostVertex>() / 3 * 3).max(3);
                    ghost
                        .vertices
                        .chunks(count)
                        .map(|vertices| {
                            let buffer =
                                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("Agent retained departing geometry"),
                                    contents: bytemuck::cast_slice(vertices),
                                    usage: wgpu::BufferUsages::VERTEX,
                                });
                            (buffer, vertices.len() as u32)
                        })
                        .collect()
                });
            }
        }
        vec![]
    }
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let (Some(pipelines), Some((_, bind))) = (resources.get::<Pipelines>(), self.gpu.get()) {
            pass.set_pipeline(if self.ghost.is_some() {
                &pipelines.ghost
            } else {
                &pipelines.robot
            });
            pass.set_bind_group(0, bind, &[]);
            if let Some(ghost) = &self.ghost {
                if let Some(buffers) = ghost.gpu.get() {
                    for (buffer, count) in buffers {
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.draw(0..*count, 0..1);
                    }
                }
            } else {
                pass.draw(0..6, 0..1);
            }
        }
    }
}
impl Drop for Draw {
    fn drop(&mut self) {
        if let Some(buffers) = self.gpu.take()
            && let Ok(mut pool) = self.pool.lock()
            && pool.len() < 32
        {
            pool.push(buffers);
        }
    }
}
struct TargetSrgb(bool);

// Precision renderer packets share retained geometry across animation and
// volume-bound changes. Each pane owns its uniform and projection.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ProjectedUniform {
    parameters: [f32; 4],
    size: [f32; 4],
    volume: reference_volume::Parameters,
}
pub(super) struct Projected {
    uniform: ProjectedUniform,
    gpu: OnceLock<(wgpu::Buffer, wgpu::BindGroup)>,
}
struct ProjectedPipelines {
    faces: wgpu::RenderPipeline,
    materials: wgpu::RenderPipeline,
    shadows: wgpu::RenderPipeline,
    material_shadows: wgpu::RenderPipeline,
    depth: wgpu::RenderPipeline,
    edges: wgpu::RenderPipeline,
    empty: wgpu::TextureView,
    sampler: wgpu::Sampler,
    srgb: bool,
}
pub(super) fn projected_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Agent precision shading parameters"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<ProjectedUniform>() as u64,
                    ),
                },
                count: None,
            },
        ],
    })
}
pub(super) fn install_projected(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    renderer: &mut egui_wgpu::Renderer,
) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Agent precision scene shading"),
        source: wgpu::ShaderSource::Wgsl(
            concat!(
                include_str!("surface_appearance.wgsl"),
                "\n",
                include_str!("agent_effects.wgsl"),
                "\n",
                include_str!("edges.wgsl")
            )
            .into(),
        ),
    });
    let pipeline = |pass| create_pipeline_with_effects(device, &shader, format, pass, 1, true);
    let empty = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Unused agent shadow mask"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default());
    renderer.callback_resources.insert(ProjectedPipelines {
        faces: pipeline(PipelinePass::Faces),
        materials: pipeline(PipelinePass::MaterialFaces),
        shadows: pipeline(PipelinePass::ShadowFaces),
        material_shadows: pipeline(PipelinePass::MaterialShadowFaces),
        depth: pipeline(PipelinePass::EdgeDepth),
        edges: pipeline(PipelinePass::Edges),
        empty,
        sampler: device.create_sampler(&Default::default()),
        srgb: format.is_srgb(),
    });
}
impl Projected {
    pub(super) fn new(frame: &Frame, camera: &Camera, rect: Rect,
        volume: Option<(DVec3, DVec3)>, projection: (glam::DMat4, [f32; 4])) -> Option<Self> {
        (frame.animated() || volume.is_some()).then(|| Self {
            uniform: ProjectedUniform {
                parameters: frame.parameters(camera, rect),
                size: [
                    rect.width(),
                    rect.height(),
                    if frame.starts.is_empty() { 0. } else { 1. },
                    0.,
                ],
                volume: reference_volume::Parameters::new(volume, projection),
            },
            gpu: OnceLock::new(),
        })
    }
    pub(super) fn prepare(
        &self,
        scene: &SceneCallback,
        device: &wgpu::Device,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(pipelines) = resources.get::<ProjectedPipelines>() {
            self.gpu.get_or_init(|| {
                let mut uniform = self.uniform;
                uniform.size[3] = if pipelines.srgb { 1. } else { 0. };
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Precision scene effects"),
                    contents: bytemuck::bytes_of(&uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let view = scene
                    .antialiased
                    .get()
                    .map_or(&pipelines.empty, |mask| &mask.view);
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Agent precision scene animation"),
                    layout: &pipelines.faces.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&pipelines.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: buffer.as_entire_binding(),
                        },
                    ],
                });
                (buffer, bind)
            });
        }
    }
    pub(super) fn paint(
        &self,
        scene: &SceneCallback,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) -> bool {
        let (Some(pipelines), Some((_, bind))) =
            (resources.get::<ProjectedPipelines>(), self.gpu.get())
        else {
            return false;
        };
        let materials = !scene.material_vertices.is_empty();
        let faces = match (scene.antialiased.get().is_some(), materials) {
            (true, true) => &pipelines.material_shadows,
            (true, false) => &pipelines.shadows,
            (false, true) => &pipelines.materials,
            (false, false) => &pipelines.faces,
        };
        pass.set_bind_group(0, bind, &[]);
        scene.draw(pass, [faces, &pipelines.depth, &pipelines.edges]);
        true
    }
}

#[cfg(test)]
#[path = "agent_effects_tests.rs"]
mod tests;
