#![no_std]

extern crate libc;

#[unsafe(no_mangle)]
pub extern "C" fn horizon_ps3_libc_check_marker() -> usize {
    core::mem::size_of::<libc::pthread_t>()
}
