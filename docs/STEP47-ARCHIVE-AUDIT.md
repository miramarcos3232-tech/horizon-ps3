# Step 47 — archive audit and next engineering gate

Date: 2026-11-07

## Scope and integrity

Reviewed the user-supplied `horizon-ps3-continuation-2026-11-07-step46.zip` in an isolated extraction directory. The archive passed `unzip -t` with no CRC errors.

- 6,458 ZIP entries (5,484 files and 974 directory entries).
- Includes the upstream Horizon Rust workspace, `ports/ps3/`, `crates/horizon-ps3-platform/`, `ports/ps3/rust-entry/`, vendored dependencies, and build/documentation material.
- The archive does not contain Git history. Its exact upstream commit and local-diff provenance remain unverified.
- This audit does not import the archive into this repository and does not change `main`.

## What exists

The PS3 work is a real but experimental platform prototype:

- A PSL1GHT C bridge for controller/video/RSX bring-up.
- A separate `horizon-ps3-platform` Rust crate with fixed-capacity chunk storage, greedy meshing, camera/projection, player/interaction, bounded memory helpers, and RSX mesh upload support.
- A separate `horizon-ps3-entry` static-library wrapper.
- An experimental generated-world probe/package path, distinct from the original Horizon game client.
- Build targets for host tests, target checks, static library creation, bridge/link probes, package generation, and a C software-raster test.

The PS3 README explicitly describes this as an early native port/experiment, not a playable game. Existing `.ELF` and `EBOOT.BIN` artifacts are historical build outputs and are not proof that this extracted checkout was rebuilt or that the binaries execute.

## Main integration blocker

The original `horizon-client` crate is not currently wired into the PS3 entrypoint. Its manifest pulls in a desktop-oriented dependency graph including `wgpu`, `winit`, `ureq`, `rustix`, `fs4`, and other client/resource systems. The PS3 prototype is a separate crate graph.

The correct next step is not to stub desktop/Unix APIs indiscriminately. First produce a dependency/PAL inventory for the smallest local single-player client slice, then define explicit PS3 platform traits and implement only the required services with tests. Networking, mods, telemetry, narrator/audio, online authentication, and desktop windowing should remain disabled until the local client path is viable.

The documented Rust PS3 target supports `core`/ `alloc` build paths, but this does not supply a complete PS3 `std` implementation. Threading, TLS, allocator, filesystem/I/O, entropy/randomness, time, and panic behavior need a real supported implementation or an explicitly scoped replacement. Fake POSIX stubs would create unsafe false confidence.

## Validation performed during this audit

Command run in this review environment:

```sh
make -C ports/ps3 software-raster-test
```

Result: PASS — `software triangle fill and reciprocal-depth occlusion: PASS`.

Not run in this review:

- Rust host tests or PS3 target builds: `cargo`/`rustc` are unavailable in this environment.
- PPU/PSL1GHT link and packaging targets.
- RPCS3 or physical PS3 execution.

The archive contains Rust test annotations, but an annotation count is not a test result. The progress report's prior build/package claims remain archive-reported and have not been independently reproduced here.

## Next engineering gate

1. Establish exact upstream provenance (commit/tag, license, and local patch inventory).
2. Import the full source baseline through a bulk Git workflow on a dedicated import/staging branch; do not use the GitHub web PR description as a ZIP upload mechanism.
3. Generate a reproducible dependency/PAL report for `horizon-client` and its transitive client crates.
4. Select a minimal offline single-player vertical slice and design its PS3 platform interface.
5. Build and test the slice incrementally before attempting the full desktop client dependency graph.

## Safety and merge policy

- Keep `main` unchanged.
- Do not create or merge a PR claiming the full source import until the source is actually present and reviewed.
- Preserve upstream and third-party licenses; do not commit proprietary Minecraft assets, binaries, or account data.
- Distinguish source compilation, ELF/SELF packaging, emulator execution, and real-console validation as separate milestones.
