//! Free-fly camera driven by the DualShock 3, for exploring the bring-up world.
//!
//! Left stick moves (up = forward, sideways = strafe), right stick looks,
//! R2/L2 rise and sink, R1 sprints. Yaw 0 looks along +Z; positive yaw turns
//! toward +X. All state is plain data, so the whole module is host-testable.

use core::f32::consts::{PI, TAU};

use crate::PadState;
use crate::camera::{Camera, CameraError};
use crate::trig::{cos, sin};

const DEADZONE: i16 = 16;
const MOVE_SPEED: f32 = 10.0;
const SPRINT_FACTOR: f32 = 3.0;
const TURN_SPEED: f32 = 2.0;
const MAX_PITCH: f32 = 1.45;
const MIN_HEIGHT: f32 = 2.0;
const MAX_HEIGHT: f32 = 120.0;
const FOCAL_Y: f32 = 1.428_148; // 1 / tan(35 degrees): 70 degree vertical FOV
const NEAR: f32 = 0.1;
const FAR: f32 = 128.0;

/// Stick value in `-128..=127` to `-1.0..=1.0` with a dead zone around centre.
fn axis(raw: i8) -> f32 {
    let value = i16::from(raw);
    let magnitude = value.abs();
    if magnitude <= DEADZONE {
        return 0.0;
    }
    let scaled = f32::from(magnitude - DEADZONE) / f32::from(127 - DEADZONE);
    let scaled = scaled.min(1.0);
    if value < 0 { -scaled } else { scaled }
}

fn floor_chunk(coordinate: f32) -> i32 {
    let q = coordinate / 16.0;
    let t = q as i32;
    if (t as f32) > q { t - 1 } else { t }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyCamera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
}

impl FlyCamera {
    pub const fn new(position: [f32; 3], yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
        }
    }

    pub const fn position(&self) -> [f32; 3] {
        self.position
    }

    pub const fn yaw(&self) -> f32 {
        self.yaw
    }

    pub const fn pitch(&self) -> f32 {
        self.pitch
    }

    pub const fn set_position(&mut self, position: [f32; 3]) {
        self.position = position;
    }

    /// Horizontal velocity for the grounded controller, in world X/Z units
    /// per second, using the same stick convention as free-fly movement.
    pub fn horizontal_velocity(&self, pad: PadState) -> [f32; 2] {
        let (sy, cy) = (sin(self.yaw), cos(self.yaw));
        let forward = -axis(pad.left_y);
        let strafe = axis(pad.left_x);
        let sprint = if pad.r1 { SPRINT_FACTOR } else { 1.0 };
        let speed = MOVE_SPEED * sprint;
        [
            (sy * forward + cy * strafe) * speed,
            (cy * forward - sy * strafe) * speed,
        ]
    }

    /// Unit vector pointing from the camera into the world.
    pub fn look_direction(&self) -> [f32; 3] {
        let (sy, cy) = (sin(self.yaw), cos(self.yaw));
        let (sp, cp) = (sin(self.pitch), cos(self.pitch));
        [sy * cp, sp, cy * cp]
    }

    /// Chunk column (`x`, `z`) the camera is currently in.
    pub fn chunk_coords(&self) -> (i32, i32) {
        (floor_chunk(self.position[0]), floor_chunk(self.position[2]))
    }

    /// Advances the camera by `dt` seconds of input. Invalid `dt` is ignored and
    /// large values are capped so a slow frame cannot teleport the player.
    pub fn update(&mut self, pad: PadState, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.1);

        self.yaw += axis(pad.right_x) * TURN_SPEED * dt;
        if self.yaw > PI {
            self.yaw -= TAU;
        } else if self.yaw < -PI {
            self.yaw += TAU;
        }
        // Stick up is negative, and looking up means a larger pitch.
        self.pitch =
            (self.pitch - axis(pad.right_y) * TURN_SPEED * dt).clamp(-MAX_PITCH, MAX_PITCH);

        let (sy, cy) = (sin(self.yaw), cos(self.yaw));
        let forward = -axis(pad.left_y);
        let strafe = axis(pad.left_x);
        let vertical = (f32::from(pad.r2) - f32::from(pad.l2)) / 255.0;
        let sprint = if pad.r1 { SPRINT_FACTOR } else { 1.0 };
        let step = MOVE_SPEED * dt * sprint;

        self.position[0] += (sy * forward + cy * strafe) * step;
        self.position[2] += (cy * forward - sy * strafe) * step;
        self.position[1] = (self.position[1] + vertical * step).clamp(MIN_HEIGHT, MAX_HEIGHT);
    }

    /// Builds the renderer camera for the current pose.
    pub fn camera(&self, aspect: f32) -> Result<Camera, CameraError> {
        let (sy, cy) = (sin(self.yaw), cos(self.yaw));
        let (sp, cp) = (sin(self.pitch), cos(self.pitch));
        let forward = [sy * cp, sp, cy * cp];
        let right = [cy, 0.0, -sy];
        // right x up = forward, as `Camera::new` requires.
        let up = cross(forward, right);
        Camera::new(
            self.position,
            right,
            up,
            forward,
            FOCAL_Y,
            aspect,
            NEAR,
            FAR,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn pad() -> PadState {
        PadState::default()
    }

    #[test]
    fn dead_zone_and_range() {
        assert_eq!(axis(0), 0.0);
        assert_eq!(axis(16), 0.0);
        assert_eq!(axis(-16), 0.0);
        assert!(axis(17) > 0.0 && axis(17) < 0.05);
        assert_eq!(axis(127), 1.0);
        assert_eq!(axis(-128), -1.0);
    }

    #[test]
    fn idle_pad_does_not_move() {
        let mut cam = FlyCamera::new([5.0, 40.0, 5.0], 0.7, 0.2);
        let before = cam;
        cam.update(pad(), DT);
        assert_eq!(cam, before);
    }

    #[test]
    fn stick_up_moves_along_facing_direction() {
        let mut cam = FlyCamera::new([0.0, 40.0, 0.0], 0.0, 0.0);
        cam.update(
            PadState {
                left_y: -127,
                ..pad()
            },
            DT,
        );
        assert!(cam.position()[2] > 0.1 && cam.position()[0].abs() < 1e-3);

        let mut east = FlyCamera::new([0.0, 40.0, 0.0], core::f32::consts::FRAC_PI_2, 0.0);
        east.update(
            PadState {
                left_y: -127,
                ..pad()
            },
            DT,
        );
        assert!(east.position()[0] > 0.1 && east.position()[2].abs() < 1e-3);
    }

    #[test]
    fn strafe_and_vertical_and_clamps() {
        let mut cam = FlyCamera::new([0.0, 2.05, 0.0], 0.0, 0.0);
        cam.update(
            PadState {
                left_x: 127,
                l2: 255,
                ..pad()
            },
            DT,
        );
        assert!(cam.position()[0] > 0.1);
        assert_eq!(cam.position()[1], MIN_HEIGHT);

        let mut high = FlyCamera::new([0.0, MAX_HEIGHT, 0.0], 0.0, 0.0);
        high.update(PadState { r2: 255, ..pad() }, DT);
        assert_eq!(high.position()[1], MAX_HEIGHT);
    }

    #[test]
    fn look_limits_and_yaw_wraps() {
        let mut cam = FlyCamera::new([0.0, 40.0, 0.0], 0.0, 0.0);
        for _ in 0..600 {
            cam.update(
                PadState {
                    right_y: -127,
                    right_x: 127,
                    ..pad()
                },
                DT,
            );
        }
        assert!((cam.pitch() - MAX_PITCH).abs() < 1e-4);
        assert!(cam.yaw() >= -PI && cam.yaw() <= PI);
    }

    #[test]
    fn bad_dt_is_ignored_and_big_dt_is_capped() {
        let mut cam = FlyCamera::new([0.0, 40.0, 0.0], 0.0, 0.0);
        let moving = PadState {
            left_y: -127,
            ..pad()
        };
        cam.update(moving, f32::NAN);
        cam.update(moving, -1.0);
        assert_eq!(cam.position(), [0.0, 40.0, 0.0]);
        cam.update(moving, 100.0);
        assert!(cam.position()[2] <= MOVE_SPEED * 0.1 + 1e-3);
    }

    #[test]
    fn renderer_camera_is_valid_for_any_pose() {
        for yaw in [-3.1, -1.0, 0.0, 0.5, 2.0, 3.1] {
            for pitch in [-1.45, -0.5, 0.0, 0.9, 1.45] {
                let cam = FlyCamera::new([8.0, 50.0, 8.0], yaw, pitch);
                assert!(cam.camera(16.0 / 9.0).is_ok(), "yaw {yaw} pitch {pitch}");
            }
        }
    }

    #[test]
    fn chunk_coords_floor_for_negative_positions() {
        let at = |x, z| FlyCamera::new([x, 40.0, z], 0.0, 0.0).chunk_coords();
        assert_eq!(at(0.0, 0.0), (0, 0));
        assert_eq!(at(15.9, 16.0), (0, 1));
        assert_eq!(at(-0.1, -16.0), (-1, -1));
        assert_eq!(at(-16.1, -32.5), (-2, -3));
    }
}
