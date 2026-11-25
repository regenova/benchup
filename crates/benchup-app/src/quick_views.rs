use super::*;
use benchup_core::{Projection, StandardView};

pub(super) const VIEW_TRANSITION_DURATION: f64 = 0.28;

pub(in crate::app) struct ViewTransition {
    from: Camera,
    to: Camera,
    started: f64,
}

impl ViewTransition {
    pub(super) fn sample(&self, destination: &Camera, now: f64) -> Option<Camera> {
        let progress = ((now - self.started) / VIEW_TRANSITION_DURATION).max(0.);
        if progress >= 1. || &self.to != destination {
            return None;
        }
        if progress == 0. {
            return Some(self.from.clone());
        }
        let t = progress * progress * (3. - 2. * progress);
        let lerp = |from, to| from + (to - from) * t;
        let yaw_delta = self.to.yaw - self.from.yaw;
        let weight = |camera: &Camera| {
            camera.projection_blend.unwrap_or_else(|| {
                f64::from(u8::from(camera.projection == Projection::Perspective))
            })
        };
        let from_weight = weight(&self.from);
        let to_weight = weight(&self.to);
        let perspective = lerp(from_weight, to_weight);
        Some(Camera {
            target: self.from.target.lerp(self.to.target, t),
            distance: lerp(self.from.distance.ln(), self.to.distance.ln()).exp(),
            yaw: self.from.yaw + yaw_delta.sin().atan2(yaw_delta.cos()) * t,
            pitch: lerp(self.from.pitch, self.to.pitch),
            fov: lerp(self.from.fov, self.to.fov),
            projection: if perspective >= 0.5 {
                Projection::Perspective
            } else {
                Projection::Parallel
            },
            projection_blend: (from_weight != to_weight).then_some(perspective),
        })
    }
}

#[derive(Clone, Copy)]
enum QuickView {
    ThreeD,
    Direction(StandardView),
}

impl QuickView {
    fn selected(self, camera: &Camera) -> bool {
        match self {
            Self::ThreeD => camera.projection == Projection::Perspective,
            Self::Direction(view) => {
                camera.projection == Projection::Parallel && camera.standard_view() == Some(view)
            }
        }
    }

    fn apply(self, camera: &mut Camera) {
        match self {
            Self::ThreeD => {
                let default = Camera::default();
                camera.yaw = default.yaw;
                camera.pitch = default.pitch;
                camera.projection = Projection::Perspective;
            }
            Self::Direction(view) => {
                camera.set_standard_view(view);
                camera.projection = Projection::Parallel;
            }
        }
    }
}

fn cube_icon(color: Color32) -> egui::Atom<'static> {
    egui::Atom::paint(Vec2::splat(14.), move |ui, args| {
        let center = args.rect.center();
        let outline = [
            (0., -5.5),
            (5., -2.8),
            (5., 2.8),
            (0., 5.5),
            (-5., 2.8),
            (-5., -2.8),
        ]
        .map(|(x, y)| center + egui::vec2(x, y));
        let stroke = Stroke::new(1., color);
        ui.painter()
            .add(egui::Shape::closed_line(outline.to_vec(), stroke));
        for index in [1, 3, 5] {
            ui.painter().line_segment([outline[index], center], stroke);
        }
    })
}

impl BenchUpApp {
    fn transition_to_view(&mut self, camera: Camera, ctx: &egui::Context) {
        if self.doc.camera == camera {
            return;
        }
        let started = ctx.input(|input| input.time);
        let from = self
            .view_transition
            .as_ref()
            .and_then(|transition| transition.sample(&self.doc.camera, started))
            .unwrap_or_else(|| self.doc.camera.clone());
        self.view_transition = Some(ViewTransition {
            from,
            to: camera.clone(),
            started,
        });
        // Saving and control state use the exact destination from the first frame.
        self.doc.camera = camera;
        self.viewport_pointer = None;
        self.drawing_orbit_pivot = None;
        self.hover = None;
        self.flip_hover = None;
        self.snap = None;
        self.camera_changed(ctx);
        ctx.request_repaint();
    }

    pub(in crate::app) fn interrupt_view_transition(&mut self, ctx: &egui::Context) {
        if let Some(transition) = self.view_transition.take()
            && let Some(mut camera) =
                transition.sample(&self.doc.camera, ctx.input(|input| input.time))
        {
            camera.projection_blend = None;
            self.doc.camera = camera;
            self.camera_changed(ctx);
        }
    }

    pub(super) fn quick_view_bar(&mut self, ui: &egui::Ui, rect: Rect) -> Rect {
        let ctx = ui.ctx();
        let inset = rect.shrink(8.);
        if inset.width() < 30. || inset.height() < 34. {
            return Rect::from_min_size(rect.left_top(), Vec2::ZERO);
        }
        let enabled = ui.is_enabled() && self.tool != Tool::Walk && self.section_drag.is_none();
        let camera = &self.doc.camera;
        let colors = Palette::for_ui(ui);
        let mut requested_view = None;
        let bar = egui::Area::new(egui::Id::unique("viewport-quick-views"))
            .order(egui::Order::Foreground)
            .fixed_pos(inset.min)
            .default_width(inset.width())
            .movable(false)
            .constrain(false)
            .enabled(enabled)
            .show(ctx, |ui| {
                // A canvas gesture owns the pointer until release. Keep the bar visually steady.
                if ui.is_enabled() && self.gesture.is_some() {
                    ui.visuals_mut().disabled_alpha = 1.;
                    ui.disable();
                }
                ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                ui.set_max_width(inset.width());
                egui::Frame::new()
                    .fill(colors.surface)
                    .stroke(Stroke::new(1., colors.border))
                    .corner_radius(9)
                    .inner_margin(3)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(2., 3.);
                        ui.spacing_mut().button_padding = egui::vec2(8., 3.);
                        ui.spacing_mut().interact_size.y = 27.;
                        ui.horizontal_wrapped(|ui| {
                            for (view, label) in [
                                (QuickView::ThreeD, tr!("viewports.three_d")),
                                (
                                    QuickView::Direction(StandardView::Front),
                                    StandardView::Front.label(),
                                ),
                                (
                                    QuickView::Direction(StandardView::Right),
                                    tr!("viewports.side"),
                                ),
                                (
                                    QuickView::Direction(StandardView::Top),
                                    StandardView::Top.label(),
                                ),
                            ] {
                                let selected = view.selected(camera);
                                let text_color = if selected { colors.paper } else { colors.ink };
                                let label = RichText::new(label).color(text_color);
                                let mut button = match view {
                                    QuickView::ThreeD => {
                                        egui::Button::new((cube_icon(text_color), label)).gap(4.)
                                    }
                                    QuickView::Direction(_) => egui::Button::new(label),
                                }
                                .corner_radius(6)
                                .selected(selected);
                                if selected {
                                    button = button.fill(colors.ink).stroke(Stroke::NONE);
                                } else {
                                    button = button.frame_when_inactive(false);
                                }
                                if ui.add(button).clicked() {
                                    requested_view = Some(view);
                                }
                            }
                        });
                    });
            });
        if let Some(view) = requested_view {
            let mut camera = self.doc.camera.clone();
            view.apply(&mut camera);
            self.transition_to_view(camera, ctx);
        }
        bar.response.rect
    }
}
