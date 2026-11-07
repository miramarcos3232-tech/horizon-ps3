# Technical audit: Rust/PS3 toolchain and Horizon core extraction

Date: 2026-11-07

## Verified official PS3 Rust target

The official `powerpc64-sony-ps3` target is Tier 3, big-endian PowerPC64 ELFv1 for the Cell PPE under CellOS. It supports `core` and `alloc`; prebuilt target artifacts are not distributed by rustup. Build with nightly, `rust-src`, and Cargo `-Z build-std=core,alloc`.

The target configuration documented by Rust is:

```toml
[target.powerpc64-sony-ps3]
linker = "mold"
rustflags = [
    "-C", "relocation-model=static",
    "-C", "code-model=small",
    "-C", "target-feature=+altivec",
]
```

Build and post-link workflow:

```sh
cargo +nightly build --target powerpc64-sony-ps3 \
  -Z build-std=core,alloc --release
moldier patch path/to/program.ELF
```

`mold` is the tested linker; `lld` currently lacks the required ELFv1 call relocation support. The resulting ELF needs PS3-specific post-link patching. Optional packaging/signing tools are a separate later step; this audit does not claim a bootable HEN package.

Official reference:
https://doc.rust-lang.org/nightly/rustc/platform-support/powerpc64-sony-ps3.html

SDK reference:
https://github.com/ZephyrCodesStuff/rust-ps3

## Horizon source audit

The upstream repository currently has a multi-crate Cargo workspace with distinct core, world, client, input, render, server, launcher, telemetry, shader and tooling crates.

The upstream `crates/horizon-core/Cargo.toml` has a direct dependency on pinned `libm = 0.2.16`. Its `src/lib.rs` currently declares simulation modules including biome, carving, density, geometry, noise, paletted storage, position, prepared files, random, storage, text and ticks. It does not declare `#![no_std]` today.

This makes `horizon-core` a sensible *first audit/extraction candidate*, not yet a proven no_std-compatible crate. Each module must be checked for std imports, filesystem, threading, synchronization, I/O, allocation assumptions, and transitive dependencies before making changes.

Upstream references:
- https://github.com/horizon-rust/horizon
- https://github.com/horizon-rust/horizon/blob/main/Cargo.toml
- https://github.com/horizon-rust/horizon/blob/main/crates/horizon-core/Cargo.toml
- https://github.com/horizon-rust/horizon/blob/main/crates/horizon-core/src/lib.rs

## First engineering milestone

Do not start implementing a full Rust std PAL merely to get a first executable. The target officially supports `core + alloc`. Create a small, independently buildable PS3 smoke-test crate using:
- `#![no_std]` and `#![no_main]` where appropriate for the chosen entry model;
- a real PSL1GHT/CellOS entry and allocator path;
- no fabricated POSIX or std APIs;
- a reproducible `.cargo/config.toml`, pinned toolchain, linker and moldier;
- explicit build logs and emulator/hardware test evidence.

In parallel, audit Horizon core modules and only then port the minimum reusable modules into a separate adapter/workspace crate. Keep renderer and platform runtime behind an interface; do not assume wgpu/Vulkan can run on RSX.

## Validation status

This commit is a source/documentation audit only. No PS3-target compilation was run in this ChatGPT session, no PAL implementation was tested, and no PS3/ RPCS3 execution is claimed. Results in the prior handoff remain reported historical evidence until reproduced.
