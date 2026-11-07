#![deny(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

//! Platform boundary for the Horizon PlayStation 3 port.
//!
//! The game and renderer must depend on this contract rather than on desktop
//! window/event APIs. The host stub makes the contract testable now; the
//! optional `psl1ght-ffi` feature calls the separate C bridge when linked with
//! the selected PPU/RSX SDK.

use core::fmt;

pub mod bounded_alloc;
pub mod camera;
pub mod chunk_mesh;
pub mod chunk_store;
pub mod frame_budget;
pub mod greedy_mesh;
pub mod grounded;
pub mod hotbar;
pub mod interaction;
pub mod mesh_cache;
pub mod player;
pub mod rsx_mesh;
pub mod terrain;
pub mod trig;
pub mod world;

#[cfg(feature = "global-alloc")]
pub mod global_alloc;

#[cfg(feature = "psl1ght-ffi")]
mod client;
#[cfg(feature = "psl1ght-ffi")]
mod psl1ght;
#[cfg(feature = "psl1ght-ffi")]
pub use client::horizon_ps3_rust_world_entry;
#[cfg(feature = "psl1ght-ffi")]
pub use psl1ght::{Psl1ghtApi, horizon_ps3_rust_abi_version, horizon_ps3_rust_smoke_entry};

/// A measured-or-configured budget for one PS3 memory pool.
///
/// `capacity` is deliberately supplied by the selected runtime; the physical
/// size of XDR or GDDR3 is not the amount an application can use. `reserved`
/// is the portion Horizon may claim from that pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryPoolBudget {
    pub capacity: u32,
    pub reserved: u32,
}

impl MemoryPoolBudget {
    pub const fn available(self) -> u32 {
        self.capacity.saturating_sub(self.reserved)
    }
}

/// Conservative first-pass memory budget for the PS3 client, split by pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryBudget {
    /// Cell/XDR allocations: game state, chunk data and decoded resources.
    pub xdr: MemoryPoolBudget,
    /// RSX/GDDR3 allocations: transient geometry and texture uploads.
    pub gddr3: MemoryPoolBudget,
}

impl MemoryBudget {
    /// A bounded starting profile. Values must be measured on real hardware.
    pub const fn initial() -> Self {
        Self {
            xdr: MemoryPoolBudget {
                // Placeholder runtime capacity; must be replaced by telemetry.
                capacity: 192 * 1024 * 1024,
                reserved: 176 * 1024 * 1024,
            },
            gddr3: MemoryPoolBudget {
                // Placeholder runtime capacity; must be replaced by telemetry.
                capacity: 224 * 1024 * 1024,
                reserved: 32 * 1024 * 1024,
            },
        }
    }

    pub const fn total(self) -> u32 {
        self.xdr.reserved.saturating_add(self.gddr3.reserved)
    }

    pub const fn fits(self) -> bool {
        self.xdr.reserved <= self.xdr.capacity && self.gddr3.reserved <= self.gddr3.capacity
    }

    pub const fn accepts_usage(self, usage: MemoryUsage) -> bool {
        usage.xdr_reserved <= self.xdr.capacity && usage.gddr3_reserved <= self.gddr3.capacity
    }
}

/// Runtime-observed bytes reserved by the platform bridge.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MemoryUsage {
    pub xdr_reserved: u32,
    pub gddr3_reserved: u32,
}

impl MemoryUsage {
    pub const fn total(self) -> u32 {
        self.xdr_reserved.saturating_add(self.gddr3_reserved)
    }
}

/// Initial video profile used while bringing up the RSX path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayConfig {
    pub width: u16,
    pub height: u16,
    pub buffers: u8,
    pub vsync: bool,
}

impl DisplayConfig {
    pub const fn initial() -> Self {
        Self {
            width: 1280,
            height: 720,
            buffers: 2,
            vsync: true,
        }
    }

    /// Bytes needed for all colour buffers at RGBA8.
    pub const fn color_bytes(self) -> u32 {
        (self.width as u32)
            .saturating_mul(self.height as u32)
            .saturating_mul(4)
            .saturating_mul(self.buffers as u32)
    }

    /// Bytes needed for the bridge's Z24S8 depth/stencil buffer.
    pub const fn depth_bytes(self) -> u32 {
        (self.width as u32)
            .saturating_mul(self.height as u32)
            .saturating_mul(4)
    }
}

/// Buttons exposed by the DualShock 3/Sixaxis adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PadState {
    pub cross: bool,
    pub circle: bool,
    pub square: bool,
    pub triangle: bool,
    pub l1: bool,
    pub r1: bool,
    pub l2: u8,
    pub r2: u8,
    pub start: bool,
    pub select: bool,
    pub dpad_x: i8,
    pub dpad_y: i8,
    pub left_x: i8,
    pub left_y: i8,
    pub right_x: i8,
    pub right_y: i8,
}

/// A small frame description passed from gameplay to the platform backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame<'a> {
    /// Packed RGBA8 pixels used by the bring-up path and host stub.
    pub rgba8: &'a [u8],
    pub width: u16,
    pub height: u16,
}

impl Frame<'_> {
    /// Whether the pixel buffer exactly matches the dimensions as RGBA8.
    pub fn is_valid_for(self, display: DisplayConfig) -> bool {
        let expected = usize::from(self.width)
            .saturating_mul(usize::from(self.height))
            .saturating_mul(4);
        self.width != 0
            && self.height != 0
            && self.width == display.width
            && self.height == display.height
            && self.rgba8.len() == expected
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformError {
    NotReady,
    InvalidFrame,
    OutOfMemory,
    Unsupported,
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotReady => "platform is not ready",
            Self::InvalidFrame => "frame dimensions or pixel data are invalid",
            Self::OutOfMemory => "platform memory budget exhausted",
            Self::Unsupported => "operation is not supported by this backend",
        };
        f.write_str(message)
    }
}

/// Operations required by the client loop without assuming a desktop OS.
pub trait Platform {
    fn display(&self) -> DisplayConfig;
    fn poll_pad(&mut self) -> PadState;
    fn begin_frame(&mut self) -> Result<(), PlatformError>;
    fn present(&mut self, frame: Frame<'_>) -> Result<(), PlatformError>;
}

/// Low-level operations supplied by the selected HEN/homebrew SDK.
///
/// Keeping these calls behind a trait means the game-facing code does not
/// depend on PSL1GHT, an SDK fork, or guessed syscall numbers. The concrete
/// implementation can be added once the target toolchain is available.
pub trait Ps3Api {
    fn tty_write(&mut self, bytes: &[u8]) -> Result<(), PlatformError>;
    fn memory_available(&self) -> u32;
    fn memory_usage(&self) -> MemoryUsage;
    fn pad_state(&mut self) -> PadState;
    fn begin_rsx_frame(&mut self, display: DisplayConfig) -> Result<(), PlatformError>;
    fn present_rsx_frame(&mut self, frame: Frame<'_>) -> Result<(), PlatformError>;
}

/// HEN-facing platform adapter. It owns policy and delegates hardware calls
/// to the SDK adapter, which keeps the rest of Horizon independent of HEN.
pub struct Ps3Platform<A> {
    api: A,
    display: DisplayConfig,
    budget: MemoryBudget,
    frame_started: bool,
}

impl<A: Ps3Api> Ps3Platform<A> {
    pub fn new(api: A, display: DisplayConfig, budget: MemoryBudget) -> Self {
        Self {
            api,
            display,
            budget,
            frame_started: false,
        }
    }

    pub fn api(&self) -> &A {
        &self.api
    }

    pub fn api_mut(&mut self) -> &mut A {
        &mut self.api
    }

    pub const fn budget(&self) -> MemoryBudget {
        self.budget
    }

    pub fn memory_usage(&self) -> MemoryUsage {
        self.api.memory_usage()
    }

    pub fn log(&mut self, message: &[u8]) -> Result<(), PlatformError> {
        self.api.tty_write(message)
    }
}

impl<A: Ps3Api> Platform for Ps3Platform<A> {
    fn display(&self) -> DisplayConfig {
        self.display
    }

    fn poll_pad(&mut self) -> PadState {
        self.api.pad_state()
    }

    fn begin_frame(&mut self) -> Result<(), PlatformError> {
        if !self.budget.accepts_usage(self.api.memory_usage()) {
            return Err(PlatformError::OutOfMemory);
        }
        self.api.begin_rsx_frame(self.display)?;
        self.frame_started = true;
        Ok(())
    }

    fn present(&mut self, frame: Frame<'_>) -> Result<(), PlatformError> {
        if !self.frame_started {
            return Err(PlatformError::NotReady);
        }
        if !frame.is_valid_for(self.display) {
            return Err(PlatformError::InvalidFrame);
        }
        let result = self.api.present_rsx_frame(frame);
        if result.is_ok() {
            self.frame_started = false;
        }
        result
    }
}

/// Deterministic backend for unit tests and early renderer bring-up.
#[derive(Debug)]
pub struct HostStub {
    display: DisplayConfig,
    pad: PadState,
    presented_frames: u64,
}

impl HostStub {
    pub const fn new(display: DisplayConfig) -> Self {
        Self {
            display,
            pad: PadState {
                cross: false,
                circle: false,
                square: false,
                triangle: false,
                l1: false,
                r1: false,
                l2: 0,
                r2: 0,
                start: false,
                select: false,
                dpad_x: 0,
                dpad_y: 0,
                left_x: 0,
                left_y: 0,
                right_x: 0,
                right_y: 0,
            },
            presented_frames: 0,
        }
    }

    pub const fn presented_frames(&self) -> u64 {
        self.presented_frames
    }

    pub fn set_pad(&mut self, pad: PadState) {
        self.pad = pad;
    }
}

impl Default for HostStub {
    fn default() -> Self {
        Self::new(DisplayConfig::initial())
    }
}

impl Platform for HostStub {
    fn display(&self) -> DisplayConfig {
        self.display
    }

    fn poll_pad(&mut self) -> PadState {
        self.pad
    }

    fn begin_frame(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn present(&mut self, frame: Frame<'_>) -> Result<(), PlatformError> {
        if !frame.is_valid_for(self.display) {
            return Err(PlatformError::InvalidFrame);
        }
        self.presented_frames += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakePs3 {
        begins: u32,
        presents: u32,
        usage: MemoryUsage,
    }

    impl Ps3Api for FakePs3 {
        fn tty_write(&mut self, _bytes: &[u8]) -> Result<(), PlatformError> {
            Ok(())
        }

        fn memory_available(&self) -> u32 {
            MemoryBudget::initial().total()
        }

        fn memory_usage(&self) -> MemoryUsage {
            self.usage
        }

        fn pad_state(&mut self) -> PadState {
            PadState::default()
        }

        fn begin_rsx_frame(&mut self, _display: DisplayConfig) -> Result<(), PlatformError> {
            self.begins += 1;
            Ok(())
        }

        fn present_rsx_frame(&mut self, _frame: Frame<'_>) -> Result<(), PlatformError> {
            self.presents += 1;
            Ok(())
        }
    }

    #[test]
    fn initial_budget_is_bounded() {
        let budget = MemoryBudget::initial();
        assert_eq!(budget.total(), 208 * 1024 * 1024);
        assert!(budget.fits());
        assert_eq!(budget.xdr.available(), 16 * 1024 * 1024);
        assert!(budget.accepts_usage(MemoryUsage {
            xdr_reserved: 1 * 1024 * 1024,
            gddr3_reserved: 11 * 1024 * 1024,
        }));
        assert!(!budget.accepts_usage(MemoryUsage {
            xdr_reserved: 193 * 1024 * 1024,
            gddr3_reserved: 0,
        }));
        assert_eq!(
            MemoryUsage {
                xdr_reserved: u32::MAX,
                gddr3_reserved: 1,
            }
            .total(),
            u32::MAX
        );
    }

    #[test]
    fn initial_display_has_two_rgba_buffers_and_depth() {
        let display = DisplayConfig::initial();
        assert_eq!(display.color_bytes(), 1280 * 720 * 4 * 2);
        assert_eq!(display.depth_bytes(), 1280 * 720 * 4);
    }

    #[test]
    fn ps3_adapter_rejects_frame_when_runtime_pool_is_over_capacity() {
        let config = DisplayConfig {
            width: 2,
            height: 1,
            buffers: 2,
            vsync: true,
        };
        let mut platform = Ps3Platform::new(
            FakePs3 {
                begins: 0,
                presents: 0,
                usage: MemoryUsage {
                    xdr_reserved: 193 * 1024 * 1024,
                    gddr3_reserved: 0,
                },
            },
            config,
            MemoryBudget::initial(),
        );
        assert_eq!(platform.begin_frame(), Err(PlatformError::OutOfMemory));
        assert_eq!(platform.api().begins, 0);
    }

    #[test]
    fn memory_and_surface_sizes_saturate_instead_of_wrapping() {
        let budget = MemoryBudget {
            xdr: MemoryPoolBudget {
                capacity: u32::MAX,
                reserved: u32::MAX,
            },
            gddr3: MemoryPoolBudget {
                capacity: u32::MAX,
                reserved: 1,
            },
        };
        assert_eq!(budget.total(), u32::MAX);

        let display = DisplayConfig {
            width: u16::MAX,
            height: u16::MAX,
            buffers: u8::MAX,
            vsync: true,
        };
        assert_eq!(display.color_bytes(), u32::MAX);
        assert_eq!(display.depth_bytes(), u32::MAX);
    }

    #[test]
    fn host_stub_accepts_a_full_frame() {
        let config = DisplayConfig {
            width: 2,
            height: 1,
            buffers: 2,
            vsync: true,
        };
        let mut platform = HostStub::new(config);
        let pixels = [0_u8; 8];
        platform.begin_frame().unwrap();
        platform
            .present(Frame {
                rgba8: &pixels,
                width: 2,
                height: 1,
            })
            .unwrap();
        assert_eq!(platform.presented_frames(), 1);
    }

    #[test]
    fn host_stub_rejects_a_partial_frame() {
        let mut platform = HostStub::new(DisplayConfig {
            width: 2,
            height: 1,
            buffers: 2,
            vsync: true,
        });
        assert_eq!(
            platform.present(Frame {
                rgba8: &[0; 4],
                width: 2,
                height: 1
            }),
            Err(PlatformError::InvalidFrame)
        );
    }

    #[test]
    fn ps3_adapter_requires_begin_before_present() {
        let config = DisplayConfig {
            width: 2,
            height: 1,
            buffers: 2,
            vsync: true,
        };
        let mut platform = Ps3Platform::new(
            FakePs3 {
                begins: 0,
                presents: 0,
                usage: MemoryUsage::default(),
            },
            config,
            MemoryBudget::initial(),
        );
        let pixels = [0_u8; 8];
        assert_eq!(
            platform.present(Frame {
                rgba8: &pixels,
                width: 2,
                height: 1
            }),
            Err(PlatformError::NotReady)
        );
        platform.begin_frame().unwrap();
        platform
            .present(Frame {
                rgba8: &pixels,
                width: 2,
                height: 1,
            })
            .unwrap();
        assert_eq!(platform.api().begins, 1);
        assert_eq!(platform.api().presents, 1);
    }

    #[test]
    fn ps3_adapter_rejects_invalid_frame_before_calling_backend() {
        let config = DisplayConfig {
            width: 2,
            height: 1,
            buffers: 2,
            vsync: true,
        };
        let mut platform = Ps3Platform::new(
            FakePs3 {
                begins: 0,
                presents: 0,
                usage: MemoryUsage::default(),
            },
            config,
            MemoryBudget::initial(),
        );
        platform.begin_frame().unwrap();
        assert_eq!(
            platform.present(Frame {
                rgba8: &[0; 4],
                width: 2,
                height: 1
            }),
            Err(PlatformError::InvalidFrame)
        );
        assert_eq!(platform.api().presents, 0);
        // A rejected frame leaves the begun frame available for a corrected retry.
        platform
            .present(Frame {
                rgba8: &[0; 8],
                width: 2,
                height: 1,
            })
            .unwrap();
        assert_eq!(platform.api().presents, 1);
    }

    #[test]
    fn frame_validation_rejects_zero_dimensions_and_wrong_resolution() {
        let display = DisplayConfig {
            width: 2,
            height: 1,
            buffers: 2,
            vsync: true,
        };
        assert!(
            !Frame {
                rgba8: &[],
                width: 0,
                height: 1
            }
            .is_valid_for(display)
        );
        assert!(
            !Frame {
                rgba8: &[0; 8],
                width: 1,
                height: 2
            }
            .is_valid_for(display)
        );
    }
}
