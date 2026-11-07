# Horizon → PS3 migration staging plan

Status: planning/staging only. This document does not claim that the upstream Horizon client runs on PS3.

## Repository state observed during staging

- The destination repository is `miramarcos3232-tech/horizon-ps3`.
- The migration is isolated on branch `ai/ps3-migration-staging`; `main` is not modified by this work.
- The supplied continuation archive `horizon-ps3-continuation-2026-11-07-step46.zip` was inspected separately. It contains the Horizon workspace plus PS3-specific work, but no Git history.
- The archive contains approximately 5,484 files. The destination repository is currently a small documentation repository, so this is a source import, not a small patch.

## Important technical distinction

The current PS3 work is an early native prototype, not a completed port of the original Horizon game client. The PS3 platform crate and generated world-client prototype are not yet integrated with the original `horizon-client` crate graph.

The current reported blocker for the original client is its dependency on Rust `std` and Unix/PAL services (threads, TLS, allocation, I/O, randomness, and related platform behavior). The PS3 target's `core`/`alloc` support does not by itself provide a working PS3 `std` implementation. Do not replace these APIs with fake POSIX stubs.

## Staged migration sequence

1. **Pin provenance.** Record the exact upstream Horizon revision represented by the archive, its license, and any local modifications. The archive has no `.git` directory, so its exact commit cannot be inferred from the archive alone.
2. **Import source as a separate baseline.** Bring in the complete workspace and required build metadata on this branch (or in a dedicated upstream-import branch), preserving upstream paths and licenses. Do not copy only `ports/ps3`: its workspace/build dependencies must remain resolvable.
3. **Keep the PS3 port isolated.** Preserve the PS3 platform crate, PSL1GHT/PPU build scripts, vendored dependencies, and port documentation as a distinct platform layer.
4. **Integrate incrementally.** Map the original client's PAL dependencies, then implement and test real PS3 platform services one subsystem at a time. Track unsupported features explicitly.
5. **Validate in layers.** Run host tests, PS3 target checks, link/package checks, then RPCS3 and physical-console smoke tests. A successful package build is not proof of runtime correctness.
6. **Review before merge.** Submit a draft PR with a file inventory, upstream provenance, license review, exact commands run, and separate lists of passed, failed, and unrun tests.

## Evidence currently available from the supplied archive

- The software rasterizer smoke test was run during archive review and passed: `make -C ports/ps3 software-raster-test`.
- The archive contains 89 Rust test annotations, but that count is not evidence that those tests passed.
- Rust tests were not run in that review environment because `cargo` and `rustc` were unavailable.
- The archive's progress report describes an EBOOT/package build, but also says that execution on a real PS3 was not validated and RPCS3 stopped during `sbrk_init`. Treat those as reported results, not independently reproduced results.
- The PS3 prototype still has reported gaps including real Horizon client integration, textures/atlas, persistent storage/saves, lighting/AO, and hardware validation.

## Import constraints

- Do not overwrite `main` or merge this branch automatically.
- Do not claim that Minecraft/Mojang source or assets are included; this is an independent, non-official project.
- Preserve third-party license and attribution files. Review the vendored `libc` material and all upstream license obligations before publishing a complete import.
- Exclude build outputs and generated binaries unless their provenance and redistribution terms are clear.
- Keep test evidence reproducible and label archive-reported results separately from tests rerun by contributors.

## Current staging limitation

The GitHub connection available for this task supports individual Git objects and text-file operations, but it does not expose a bulk upload operation for the 5,484-file local archive. Therefore this branch currently stages the migration plan only; the source archive has **not** been imported into GitHub. The next required operation is a bulk Git import from a machine or workflow that can access the archive, after provenance and license review.
