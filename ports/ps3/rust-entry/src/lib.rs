#![no_std]

/// C-visible link anchor for the minimal platform smoke entrypoint.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ps3_rust_smoke_link_anchor() -> i32 {
    horizon_ps3_platform::horizon_ps3_rust_smoke_entry()
}

/// C-visible link anchor for the experimental generated-world client.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ps3_rust_world_link_anchor() -> i32 {
    horizon_ps3_platform::horizon_ps3_rust_world_entry()
}
