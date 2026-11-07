//! Small allocation-free view/projection helper for the PS3 bring-up renderer.
//!
//! Callers provide an orthonormal camera basis and `focal_y = 1 / tan(fov_y/2)`.
//! The projected depth uses the symmetric clip interval [-1, 1], matching the
//! current pass-through RSX vertex program. This module projects vertices on
//! the CPU; it is not a replacement for the final GPU matrix/uniform path.

const BASIS_TOLERANCE: f32 = 0.01;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraError {
    InvalidConfiguration,
    PointOutsideDepthRange,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    origin: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    forward: [f32; 3],
    focal_y: f32,
    aspect: f32,
    near: f32,
    far: f32,
}

impl Camera {
    /// Construct a perspective camera from a right-handed orthonormal basis.
    /// `forward` points from the eye into the scene; `right × up` should align
    /// with it. `focal_y` is positive and normally equals `1/tan(fov_y/2)`.
    pub fn new(
        origin: [f32; 3],
        right: [f32; 3],
        up: [f32; 3],
        forward: [f32; 3],
        focal_y: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Result<Self, CameraError> {
        if !origin.into_iter().all(f32::is_finite)
            || !right.into_iter().all(f32::is_finite)
            || !up.into_iter().all(f32::is_finite)
            || !forward.into_iter().all(f32::is_finite)
            || !focal_y.is_finite()
            || !aspect.is_finite()
            || !near.is_finite()
            || !far.is_finite()
            || focal_y <= 0.0
            || aspect <= 0.0
            || near <= 0.0
            || far <= near
            || !is_unit(right)
            || !is_unit(up)
            || !is_unit(forward)
            || dot(right, up).abs() > BASIS_TOLERANCE
            || dot(right, forward).abs() > BASIS_TOLERANCE
            || dot(up, forward).abs() > BASIS_TOLERANCE
            || dot(cross(right, up), forward) < 1.0 - BASIS_TOLERANCE
        {
            return Err(CameraError::InvalidConfiguration);
        }
        Ok(Self {
            origin,
            right,
            up,
            forward,
            focal_y,
            aspect,
            near,
            far,
        })
    }

    /// Return the camera-space forward distance of a world-space point.
    pub fn view_depth(self, point: [f32; 3]) -> Result<f32, CameraError> {
        Ok(self.view_coordinates(point)?.2)
    }

    pub(crate) const fn depth_limits(self) -> (f32, f32) {
        (self.near, self.far)
    }

    /// Project a world-space point into RSX clip/NDC coordinates.
    /// Points outside the near/far interval are rejected; X/Y are not clipped
    /// here because the RSX rasterizer clips them against the viewport.
    pub fn project(self, point: [f32; 3]) -> Result<[f32; 3], CameraError> {
        let (camera_x, camera_y, camera_z) = self.view_coordinates(point)?;
        if camera_z < self.near || camera_z > self.far {
            return Err(CameraError::PointOutsideDepthRange);
        }
        self.project_coordinates(camera_x, camera_y, camera_z)
    }

    /// Project after a polygon has been clipped to the camera's depth slab.
    /// Clamp only absorbs floating-point error at an intersection plane.
    pub(crate) fn project_clipped(self, point: [f32; 3]) -> Result<[f32; 3], CameraError> {
        let (camera_x, camera_y, camera_z) = self.view_coordinates(point)?;
        self.project_coordinates(camera_x, camera_y, camera_z.clamp(self.near, self.far))
    }

    fn view_coordinates(self, point: [f32; 3]) -> Result<(f32, f32, f32), CameraError> {
        if !point.into_iter().all(f32::is_finite) {
            return Err(CameraError::InvalidConfiguration);
        }
        let relative = [
            point[0] - self.origin[0],
            point[1] - self.origin[1],
            point[2] - self.origin[2],
        ];
        let camera_x = dot(relative, self.right);
        let camera_y = dot(relative, self.up);
        let camera_z = dot(relative, self.forward);
        if !camera_x.is_finite() || !camera_y.is_finite() || !camera_z.is_finite() {
            return Err(CameraError::InvalidConfiguration);
        }
        Ok((camera_x, camera_y, camera_z))
    }

    fn project_coordinates(
        self,
        camera_x: f32,
        camera_y: f32,
        camera_z: f32,
    ) -> Result<[f32; 3], CameraError> {
        let reciprocal_z = 1.0 / camera_z;
        let depth_range = self.far - self.near;
        let ndc_z = (self.far + self.near) / depth_range
            - (2.0 * self.far * self.near / depth_range) * reciprocal_z;
        let projected = [
            camera_x * self.focal_y * reciprocal_z / self.aspect,
            camera_y * self.focal_y * reciprocal_z,
            ndc_z,
        ];
        if projected.into_iter().all(f32::is_finite) {
            Ok(projected)
        } else {
            Err(CameraError::InvalidConfiguration)
        }
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn is_unit(vector: [f32; 3]) -> bool {
    (dot(vector, vector) - 1.0).abs() <= BASIS_TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera() -> Camera {
        Camera::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            1.0,
            2.0,
            1.0,
            11.0,
        )
        .unwrap()
    }

    #[test]
    fn projects_view_center_and_scales_by_depth_and_aspect() {
        let projected = camera().project([2.0, 0.0, 4.0]).unwrap();
        assert!((projected[0] - 0.25).abs() < 1e-6);
        assert_eq!(projected[1], 0.0);
        assert!((projected[2] - 0.65).abs() < 1e-6);
    }

    #[test]
    fn maps_near_and_far_planes_to_clip_depth() {
        assert_eq!(camera().project([0.0, 0.0, 1.0]), Ok([0.0, 0.0, -1.0]));
        assert_eq!(camera().project([0.0, 0.0, 11.0]), Ok([0.0, 0.0, 1.0]));
    }

    #[test]
    fn rejects_points_outside_depth_range() {
        assert_eq!(
            camera().project([0.0, 0.0, 0.5]),
            Err(CameraError::PointOutsideDepthRange)
        );
        assert_eq!(
            camera().project([0.0, 0.0, 12.0]),
            Err(CameraError::PointOutsideDepthRange)
        );
    }

    #[test]
    fn rejects_invalid_fov_scale_basis_and_clip_planes() {
        let invalid = Camera::new(
            [0.0; 3],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            1.0,
            1.0,
            0.1,
            100.0,
        );
        assert_eq!(invalid, Err(CameraError::InvalidConfiguration));
        assert_eq!(
            Camera::new(
                [0.0; 3],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, -1.0],
                1.0,
                1.0,
                0.1,
                100.0,
            ),
            Err(CameraError::InvalidConfiguration)
        );
        assert_eq!(
            camera().project([f32::NAN, 0.0, 2.0]),
            Err(CameraError::InvalidConfiguration)
        );
    }
}
