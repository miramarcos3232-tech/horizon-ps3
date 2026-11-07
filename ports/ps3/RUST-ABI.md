# Rust ↔ PPU ABI: current decisions and open checks

## Selected Rust target

The platform crate now cross-compiles for `powerpc64-sony-ps3`, Rust's Tier 3 target for the PS3 PPU. The target is big-endian PowerPC64 ELFv1 for CellOS and reports a 64-bit pointer width. It supports `core` and `alloc`; use nightly Rust, `rust-src`, and `-Z build-std=core,alloc`. The documented final executable path uses `mold`, followed by PS3-specific ELF processing such as `moldier` [1].

The shared crate remains `rlib`-only. `ports/ps3/rust-entry/` is a separate `no_std` static-library wrapper that explicitly references both existing entrypoints. This keeps the host test harness's panic runtime separate from the PS3 archive.

## Reproducible commands

From the repository root:

```sh
make -C ports/ps3 host-test
make -C ports/ps3 rust-ps3-check
make -C ports/ps3 rust-ps3-staticlib
```

The underlying PS3 target build is equivalent to:

```sh
RUSTFLAGS='-C panic=abort -C relocation-model=static -C code-model=small -C target-feature=+altivec' \
CARGO_TARGET_POWERPC64_SONY_PS3_LINKER=mold \
cargo +nightly build \
  --manifest-path ports/ps3/rust-entry/Cargo.toml \
  -Z build-std=core,alloc \
  --target powerpc64-sony-ps3 --release --lib
```

The current run passed 75 Rust host tests and the C software-raster test. The platform crate's `psl1ght-ffi` feature compiled for the PS3 target. The release static archive was produced; archive members were inspected as ELF64, big-endian PowerPC64 ELFv1 relocatable objects. This does not prove the final executable ABI. The archive member's generic SYSV object metadata is not the PS3 executable OS/ABI header; that is a later final-link/post-link concern.

## Open ABI and runtime checks

1. Select a C/RSX SDK and record the PPU ABI it actually uses. Check pointer width, `size_t`, structure layout, argument passing, ELFv1 function descriptors, TOC relocations, and the SDK CRT/import conventions. Do not treat endianness or 64-bit ELF alone as proof of compatibility. PS3DK's README, for example, documents a default PPU ABI with ELF64 and 32-bit C pointers [4], unlike this Rust target's 64-bit pointers.
2. Link the wrapper against the C bridge using a documented linker/runtime combination. The old `powerpc64-unknown-linux-gnu` + `ppu-gcc` probe is a historical cross-link experiment, not validation of `powerpc64-sony-ps3`.
3. Run the final patched ELF on RPCS3 or an authorised PS3 and capture TTY, video mode, input, RSX output, and exit behavior. No current evidence shows the new Rust target binary running on either.
4. Only after that, connect the actual Horizon world/resource systems and replace the bring-up renderer.

Rust Tier 3 means that the Rust project makes no build or test guarantee for the target [2]. Keep the build isolated and recheck it when updating nightly.

## References

- [1] [Rust target: `powerpc64-sony-ps3`](https://doc.rust-lang.org/nightly/rustc/platform-support/powerpc64-sony-ps3.html)
- [2] [Rust target-tier policy](https://doc.rust-lang.org/nightly/rustc/target-tier-policy.html)
- [3] [Open-source Rust PS3 SDK and `moldier`](https://github.com/ZephyrCodesStuff/rust-ps3)
- [4] [PS3DK PPU ABI documentation](https://git.rpcs3.net/rpcs3/PS3DK)
