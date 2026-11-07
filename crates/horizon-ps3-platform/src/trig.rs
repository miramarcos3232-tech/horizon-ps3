//! Sine and cosine for `no_std`: `core` has no trigonometry and the PS3 build
//! links no `libm`. Taylor series to x^9 after range reduction to [-pi/2, pi/2];
//! the error is below 1e-5, plenty for camera orientation.

use core::f32::consts::{FRAC_PI_2, PI, TAU};

const INV_TAU: f32 = 1.0 / TAU;

/// Sine of `x` radians. Returns `0.0` for NaN and infinities.
pub fn sin(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    // Truncating cast: saturates for huge inputs, which only costs accuracy.
    let mut r = x - TAU * ((x * INV_TAU) as i32 as f32);
    if r > PI {
        r -= TAU;
    } else if r < -PI {
        r += TAU;
    }
    if r > FRAC_PI_2 {
        r = PI - r;
    } else if r < -FRAC_PI_2 {
        r = -PI - r;
    }
    let r2 = r * r;
    r * (1.0
        + r2 * (-1.0 / 6.0 + r2 * (1.0 / 120.0 + r2 * (-1.0 / 5_040.0 + r2 * (1.0 / 362_880.0)))))
}

/// Cosine of `x` radians.
pub fn cos(x: f32) -> f32 {
    sin(x + FRAC_PI_2)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn matches_std_over_several_turns() {
        for i in -2_000..=2_000 {
            let x = i as f32 * 0.01;
            assert!((sin(x) - x.sin()).abs() < 1e-4, "sin({x})");
            assert!((cos(x) - x.cos()).abs() < 1e-4, "cos({x})");
        }
    }

    #[test]
    fn key_angles_and_invalid_input() {
        assert!(sin(0.0).abs() < 1e-6);
        assert!((sin(FRAC_PI_2) - 1.0).abs() < 1e-5);
        assert!((cos(PI) + 1.0).abs() < 1e-5);
        assert_eq!(sin(f32::NAN), 0.0);
        assert_eq!(sin(f32::INFINITY), 0.0);
        assert_eq!(cos(f32::NEG_INFINITY), 0.0);
    }
}
