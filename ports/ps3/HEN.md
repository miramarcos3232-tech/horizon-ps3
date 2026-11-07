# HEN bring-up workflow

HEN is a possible homebrew test environment, but it does not supply a Rust compiler, a PS3 C SDK, an RSX renderer, or a compatible final-link process. This document describes the project's test boundary; it is not an installation or console-modification guide.

## Safety boundaries

- Use only a PS3 that you own or are authorised to test.
- Do not bypass PS3 security, signatures, DRM, or game licensing.
- Keep Minecraft binaries, assets, account tokens, and saves out of this repository and its build archives.
- Use only game data and assets that the user is legally entitled to use.

## Current state

`horizon-ps3-platform` is a `no_std` Rust platform crate. The `psl1ght-ffi` feature calls a separate C bridge for boot, video, memory telemetry, TTY, controller input, and RSX presentation. The C bring-up program and previously generated `.ELF`/`EBOOT.BIN` files are historical smoke-test artifacts, not a playable Horizon client and not evidence of a current successful console boot.

A separate crate at `rust-entry/` now builds a release Rust static library for `powerpc64-sony-ps3` using nightly Rust and `-Z build-std=core,alloc`. The platform crate compiles for this target, and the release archive's members are big-endian PowerPC64 ELFv1 objects. The static archive is not a final PS3 ELF or `EBOOT.BIN`; it has not been linked to the C bridge or run on RPCS3 or hardware.

The current build environment has Rust stable 1.99.1, nightly 1.101.0 with `rust-src`, `mold` 2.30.0, and `moldier` 0.1.0. PS3DEV was built locally for ARM64 and supplies PPU GCC 13.2.0, PSL1GHT, RSX libraries, `make_self`, and `sfo`. RPCS3 and a PS3 are still unavailable. A separate upstream Rust Hello World was patched to a CellLV2 ELF as a toolchain smoke test, but it is not Horizon and was not executed. Historical references to `/home/ubuntu/ps3dev`, PPU GCC 7.2.0, and a rebuilt smoke package refer to an earlier environment and must not be used as current setup instructions.

## Reproducible checks

From the repository root:

```sh
make -C ports/ps3 host-test
make -C ports/ps3 rust-ps3-check
make -C ports/ps3 rust-ps3-staticlib
```

The host check currently passes 75 Rust unit tests and the software triangle/depth test. The Rust PS3 check and static-library build verify code generation and archive creation only. A final ELF still needs a chosen PPU C/RSX SDK, a verified C/Rust ABI, PS3-specific post-link processing, and a run on RPCS3 or an authorised PS3.

The Makefile shader recipe now targets PS3DK's open `rsx-cg-compiler` and uses a stable relative symbol prefix when `OUT` changes. Its dry-run and a host `objcopy` symbol check passed; compiling the actual shaders still requires the unavailable C SDK. See [TOOLCHAIN.md](TOOLCHAIN.md) for evidence and limitations.

The attached PS3DEV SDK has since been built locally. `make -C ports/ps3 smoke-package SHADER_BLOB_MODE=prebuilt` now compiles the C bridge and RSX objects, links `bringup.elf`, and creates a smoke `USRDIR/EBOOT.BIN` plus `PARAM.SFO`. This package is not the Rust Horizon client and has not been executed.

The reproducible target `rust-ps3-link-package` now builds the official Rust static library, links it to the PS3DEV bridge, applies `moldier`, verifies the CellLV2 OS/ABI (`0x66`), and creates a separate `rust-abi/USRDIR/EBOOT.BIN` plus `PARAM.SFO`. This is an ABI probe, not the Horizon game; it has not been executed in RPCS3 or on hardware.

The target `rust-ps3-world-package` links the experimental generated-world entrypoint instead of the ABI-only probe and creates `rust-world/USRDIR/EBOOT.BIN` with title `Horizon World (unverified)`. The world loop includes camera movement, pad polling, bounded chunk streaming/meshing and RSX drawing. It remains unverified until an authorised RPCS3 or PS3 run.

The current world package also supports CROSS to break the first solid voxel under the camera ray and CIRCLE to place stone in the adjacent empty voxel. The edit path is bounded and updates dirty revisions/loaded neighbour halos; it has passed host tests and the PS3 cross-build but has not been executed on target hardware.
