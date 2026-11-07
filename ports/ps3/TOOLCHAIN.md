# PS3 toolchain decision and verified state

## Rust target

Use Rust's documented `powerpc64-sony-ps3` target for the PS3 PPU, not `powerpc64-unknown-linux-gnu` as the final target. The official target is Tier 3, big-endian PowerPC64 ELFv1 for CellOS. It supports `core` and `alloc`, but has no prebuilt standard-library artifacts distributed by rustup; compiling requires nightly Rust, `rust-src`, and `-Z build-std=core,alloc`. The documented executable flow uses `mold` and then a PS3 ELF post-link patcher such as `moldier` [1].

The repository keeps `crates/horizon-ps3-platform` as a normal `rlib` so host tests continue to work. `rust-entry/` is a separate `no_std` static-library wrapper that references the existing C ABI entrypoints. This separation is intentional: setting `crate-type = ["rlib", "staticlib"]` on the shared crate caused its host test build to require a panic handler. The wrapper avoids that regression.

Verified commands, run from the repository root:

```sh
make -C ports/ps3 host-test
make -C ports/ps3 rust-ps3-check
make -C ports/ps3 rust-ps3-staticlib
```

The current environment passed 75 Rust unit tests and the C software-raster test. `rust-ps3-check` passed for `horizon-ps3-platform`. The release wrapper static archive was produced, and its members were inspected as ELF64, big-endian PowerPC64 ELFv1 relocatable objects. This verifies Rust code generation and archive creation only. The Horizon artifact members still carry generic SYSV object metadata; no Horizon PS3 executable header, final ELF, SELF/EBOOT, C bridge link, RSX output, or hardware/emulator run has been verified. A separate upstream Rust Hello World ELF was post-linked as described below; it does not validate Horizon.

The current environment has Rust stable 1.99.1, nightly 1.101.0 plus `rust-src`, `mold` 2.30.0, and `moldier` 0.1.0 at `$HOME/.cargo/bin/moldier`. The attached PS3DEV source was built locally for ARM64: PPU GCC 13.2.0, SPU GCC 9.5.0, binutils 2.42, PSL1GHT, PS3 libraries, `make_self`, `sfo`, and `cgcomp` are installed under `/home/ubuntu/work/horizon-ps3/toolchains/ps3dev-install`. RPCS3 and a PS3 are still unavailable. The earlier unrelated `.ELF` and `.BIN` files under `build/` remain historical artifacts.

## C bridge and RSX SDK

The existing C bridge calls PSL1GHT RSX, video, pad, TTY, and memory functions. The attached PS3DEV build now provides the required PPU compiler, headers, libraries, `make_self`, and `sfo`. Its package supplies `cgcomp` but not `rsx-cg-compiler`; `cgcomp` is present but cannot run without its external Cg shared library. The Makefile therefore supports three modes: open `rsx-cg-compiler`, explicit `cgcomp`, or checked-in VPO/FPO blobs. The latter mode was used for the reproducible smoke test; custom-`OUT` symbol renaming passed host checks.

Do not assume matching endianness and ELFv1 alone proves C/Rust interoperability. Rust's official PS3 target has 64-bit pointers. The selected PS3 C SDK's pointer width, structure layout, CRT, import stubs, function descriptors, and TOC handling must be checked before linking. PS3DK documents a default PPU ABI with 32-bit C pointers and an explicit `-mlp64` multilib; it says runtime objects and archives are available for both data models, but that SDK has not been built or tested here [7].

The old `legacy-ffi-check`, `legacy-rust-ppu-lib`, `rust-ppu-link-probe`, and `rust-world-link` path uses `powerpc64-unknown-linux-gnu` and is retained only as a historical cross-link experiment. It is not evidence for the official PS3 target. The official Rust target's final executable must use a compatible linker and PS3-specific post-link processing; `make_self` alone does not prove that a Rust-produced executable is ready to boot [1] [3].

## References

- [1] [Rust target: `powerpc64-sony-ps3`](https://doc.rust-lang.org/nightly/rustc/platform-support/powerpc64-sony-ps3.html)
- [2] [Rust target-tier policy](https://doc.rust-lang.org/nightly/rustc/target-tier-policy.html)
- [3] [Open-source Rust PS3 SDK and `moldier`](https://github.com/ZephyrCodesStuff/rust-ps3)
- [4] [PS3DK open-source toolchain and PPU ABI notes](https://git.rpcs3.net/rpcs3/PS3DK)
- [5] [PSL1GHT open-source PS3 homebrew SDK](https://github.com/ps3dev/PSL1GHT)

## PS3DK candidate reviewed on 2026-11-06

The current PS3DK README describes a modern open-source PPU/SPU toolchain, a PSL1GHT v3 compatibility front-end, and an in-tree `rsx-cg-compiler`, which could replace this project's proprietary `cgcomp` assumption [6]. Its PPU default is ELF64 with 32-bit C pointers; `-mlp64` selects the LP64 multilib. The SDK says it builds runtime objects, native archives, and NID/FNID stub archives for both data models [7]. This offers a plausible match for Rust's 64-bit pointer target, but the local bridge still needs a real compile/link probe.

The documented native Linux build uses `source ./scripts/env.sh`, `./scripts/bootstrap.sh`, then `./scripts/build-ppu-toolchain.sh --only binutils` as a short host-compatibility check. The full PPU toolchain, PSL1GHT and SDK require additional source builds; the README cautions that GCC/newlib bootstrap is CPU-heavy [6]. The current environment is Ubuntu 24.04 AArch64; the README names Linux as the primary host but does not explicitly guarantee AArch64. An older upstream ps3toolchain issue records successful Apple M1 builds after patches, which is supporting context but not proof for this PS3DK version [9].

PS3DK's root MIT license applies only to its own integration code; imported toolchain sources retain their own licenses [8]. The direct `git.rpcs3.net` page and archive are blocked by Cloudflare (`403`, “Please contact the site owner for access”). The GitHub mirror named in its README returned `404` for both `main` and `master`; the attached PS3DEV source was used instead. No PS3DK source was successfully downloaded.

- [6] [PS3DK README and build instructions](https://git.rpcs3.net/rpcs3/PS3DK/src/branch/main/README.md)
- [7] [PS3DK CellOS PPU ABI specification](https://git.rpcs3.net/rpcs3/PS3DK/src/branch/main/docs/abi/cellos-lv2-abi-spec.md)
- [8] [PS3DK root license](https://git.rpcs3.net/rpcs3/PS3DK/raw/branch/main/LICENSE)
- [9] [ps3toolchain ARM64 build issue](https://github.com/ps3dev/ps3toolchain/issues/114)

## Rust post-link smoke test on 2026-11-06

The MIT/Apache-2.0 Rust PS3 SDK source was downloaded from its public GitHub archive; `moldier` 0.1.0 was compiled for the host and installed at `$HOME/.cargo/bin/moldier` [10] [11]. In an isolated copy, its `hello-world` example compiled for `powerpc64-sony-ps3` and `moldier patch` produced an ELF64 big-endian PowerPC64 ELFv1 executable with OS/ABI `0x66` (`ELFOSABI_CELLLV2`). This validates the generic Rust→mold→moldier path, not the Horizon project, its C bridge, or actual PS3 execution.

Two temporary-only adjustments were needed for this smoke test: the archive includes `ps3/sprx.example.toml` while the build script expects `ps3/sprx.toml`, so the example file was copied under that name in the isolated test tree; and the test copy's four `--undefined-glob` linker flags were removed because installed `mold` 2.30.0 rejects that option. No upstream or Horizon source was changed for those workarounds. `make_fself` is absent, so no `EBOOT.BIN` was produced. The patched ELF was not run on RPCS3 or hardware.

- [10] [rust-ps3 README and build flow](https://github.com/ZephyrCodesStuff/rust-ps3/blob/main/README.md)
- [11] [rust-ps3 license](https://github.com/ZephyrCodesStuff/rust-ps3/blob/main/LICENSE)
