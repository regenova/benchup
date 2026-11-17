//! GPU bounds and reconstruction parameters for per-fragment overflow tint.
use super::*;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Parameters {
    pub inverse: [[f32; 4]; 4],
    pub depth: [f32; 4],
    pub low: [f32; 4],
    pub high: [f32; 4],
}

impl Parameters {
    pub(super) fn new(bounds: Option<(DVec3, DVec3)>, projection: (glam::DMat4, [f32; 4])) -> Self {
        let Some((low, high)) = bounds else {
            return bytemuck::Zeroable::zeroed();
        };
        // Reconstruct relative to the volume center; keep f64 precision until
        // uploading the small uniform, without adding per-vertex attributes.
        let center = (low + high) * 0.5;
        Self {
            inverse: (glam::DMat4::from_translation(-center) * projection.0.inverse())
                .as_mat4()
                .to_cols_array_2d(),
            depth: projection.1,
            low: (low - center).extend(1.).as_vec4().to_array(),
            high: (high - center).extend(0.).as_vec4().to_array(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projected_overflow_reconstructs_crossing_faces_in_both_projections() {
        let bounds = (
            DVec3::new(-0.128, -0.128, 0.),
            DVec3::new(0.128, 0.128, 0.256),
        );
        let center = (bounds.0 + bounds.1) * 0.5;
        let size = DVec2::new(800., 600.);
        for projection in [
            benchup_core::Projection::Perspective,
            benchup_core::Projection::Parallel,
        ] {
            let mut camera = Camera {
                projection,
                ..Default::default()
            };
            camera.fit(Some(bounds), size);
            let (near, far) = resident::clip_planes(&camera, bounds);
            let matrix = camera.matrix_with_clip_planes(size, near, far);
            for remap in [false, true] {
                let depth = if remap { [0.1, 0.8, 0.1, 1.] } else { [0.; 4] };
                let uniform = Parameters::new(Some(bounds), (matrix, depth));
                let inverse = glam::Mat4::from_cols_array_2d(&uniform.inverse).as_dmat4();
                for x in [-0.2, -0.128, -0.05, 0., 0.05, 0.128, 0.2] {
                    let point = DVec3::new(x, 0., 0.128);
                    let mut screen = matrix.project_point3(point).as_vec3();
                    if remap {
                        screen.z = depth[2] + 0.8 * (screen.z - depth[0]) / depth[1];
                        screen.z = depth[0] + (screen.z - depth[2]) * depth[1] / 0.8;
                    }
                    let world = inverse.project_point3(screen.as_dvec3()) + center;
                    assert!(
                        world.distance(point) < 2e-5,
                        "{projection:?}, remap={remap}, point={point:?}"
                    );
                }
            }
        }
        assert_eq!(
            Parameters::new(None, (glam::DMat4::IDENTITY, [0.; 4])).low,
            [0.; 4]
        );
    }
}
