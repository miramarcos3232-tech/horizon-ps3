# Horizon PS3 port

This is an early native port of **Horizon**, an unofficial Rust reimplementation of Minecraft: Java Edition 26.3. It is not an official Minecraft PS3 release, and it does not contain Mojang/Microsoft code, game assets, or prepared game data. The PS3 work is a platform experiment, not a playable game.

## Current state

The repository has a C/PSL1GHT bring-up path for controller input and RSX presentation, plus host-testable Rust code for fixed-capacity chunks, greedy meshing, camera projection, bounded upload buffers, and a small generated test world. The prebuilt `.ELF`/`EBOOT.BIN` files under `build/` came from an earlier environment; they are not evidence that this checkout was rebuilt or that the binaries ran on a console or emulator.

This update adds a dedicated `no_std` Rust entry wrapper at `rust-entry/`. The shared `horizon-ps3-platform` crate remains an `rlib`, while the wrapper produces a separate static library and anchors the existing smoke/world entrypoints. This avoids changing the crate type used by the host test harness.

The official Rust target `powerpc64-sony-ps3` now compiles the platform crate and produces a release static archive. Its members were inspected as ELF64, big-endian PowerPC64 ELFv1 objects. The archive is **not** a final PS3 executable: its members still show the generic SYSV object ABI, and the PS3-specific final link, post-link patch, C/PSL1GHT bridge link, and runtime have not been validated.

On the current build environment, Rust stable 1.99.1, nightly 1.101.0 with `rust-src`, `mold` 2.30.0, and `moldier` 0.1.0 are installed. The attached PS3DEV source was also built locally for ARM64, providing PPU GCC 13.2.0, PSL1GHT, RSX libraries, `make_self`, and `sfo`. RPCS3 and a physical PS3 are not available here.

A separate upstream Rust Hello World smoke test produced a patched ELF with CellLV2 OS/ABI `0x66`. It is **not Horizon**, and it was not run in RPCS3 or on hardware. The test required temporary-only linker/config workarounds; see [TOOLCHAIN.md](TOOLCHAIN.md). The project's C/RSX link is still blocked on an accessible compatible SDK.

With the locally built PS3DEV SDK, the project's C/RSX bring-up now compiles and links to `bringup.elf`, then packages successfully into `EBOOT.BIN` and `PARAM.SFO`. This is a native controller/video/RSX smoke package, not the Horizon client, and it has not been run on RPCS3 or hardware. Shader source compilation remains pending because the SDK's `cgcomp` requires an external Cg runtime; the smoke package uses the checked-in VPO/FPO blobs.

The experimental Rust world client now also links and packages through `rust-ps3-world-package`. It contains the generated terrain stream, greedy meshing, camera, controller polling, RSX upload/draw path, and an exit condition on START+SELECT. The resulting `Horizon World (unverified)` package is a technical bring-up and has not been executed on RPCS3 or hardware.

The world loop now has the first gameplay interaction: aim at a resident voxel and press **CROSS** to break it or **CIRCLE** to place the selected block in the adjacent empty cell. D-pad left/right selects between stone, dirt, and grass. Actions are edge-triggered, use a bounded 32-block ray, update chunk revisions, and refresh loaded neighbour halos. This remains unverified on hardware.

The client now appends a lightweight three-slot HUD hotbar to the same RSX upload. The selected slot is highlighted, so D-pad selection is visible in the experimental client. It reuses the existing flat-colour bring-up shader and does not add a texture or allocation.

Each slot now starts with 16 items. Breaking a stone, dirt, or grass block returns one item to its matching slot; placing consumes one item and refuses placement at zero. Bedrock remains non-placeable and is not collected.

The HUD now renders up to eight compact quantity pips per slot. One pip represents two items, so the count changes are visible without adding a font or texture system.

`START` toggles a pause state that freezes movement, physics, streaming, hotbar selection, and block editing while leaving the current scene rendered. `START+SELECT` remains the client exit chord.

While paused, the HUD draws a centered two-bar pause icon using the same flat-colour vertex path. Resuming clears those icon vertices before the next upload.

The HUD also draws a centered four-arm crosshair for voxel targeting. It uses clip-space coloured quads and does not change the RSX shader or memory model.

The crosshair is green when the raycast finds a resident solid block within reach and white otherwise. Paused frames keep the neutral white crosshair.

Bedrock is now shown in red: it is a valid raycast hit but intentionally cannot be broken. This distinguishes an unbreakable target from empty space without adding UI text.

The edit path now rejects attempts to break `BEDROCK`, preserving the bottom layer as an unbreakable boundary. Bedrock is also absent from the placeable hotbar materials.

Placement now checks the player's body bounds before writing the target voxel. A block cannot be placed when it would overlap the player's horizontal radius or eye-height volume, preventing self-trapping.

If the body falls below world height, the client respawns it at the initial spawn point `(8, 50, 8)` and clears vertical velocity, preventing an unrecoverable void fall.

## Build and host checks

From the repository root:

```sh
make -C ports/ps3 host-test
make -C ports/ps3 rust-ps3-check
make -C ports/ps3 rust-ps3-staticlib
```

`host-test` currently passes 85 Rust unit tests and the C software-raster test. `rust-ps3-check` builds the platform crate for the official PS3 target using nightly and `-Z build-std=core,alloc`. `rust-ps3-staticlib` builds `rust-entry/` in release mode. `rust-ps3-world-package` additionally compiles and packages the experimental client through the PPU C/RSX bridge. Hardware execution remains unverified.

The target is documented by the Rust project as Tier 3. It requires nightly Rust, `rust-src`, `build-std`, and a suitable final-link/post-link flow; Tier 3 does not mean the Rust project guarantees the target will always build. See [TOOLCHAIN.md](TOOLCHAIN.md) and [RUST-ABI.md](RUST-ABI.md).

## Remaining work

The next technical gate is to select and install a compatible open PS3 C/RSX SDK, verify its PPU C data model against Rust's 64-bit PS3 target, and link the static archive with the bridge using one documented linker contract. The final ELF then needs the PS3-specific post-link processing and a run on RPCS3 or a PS3 the tester owns or is authorised to use.

Only after that gate should this port connect the real Horizon client to the backend. Still open are the RSX world renderer, texture/light pipeline, Horizon world/resource integration, controls, a playable local loop, save/load, measured memory and frame-rate budgets, and stability testing. See [PLAN-FINAL.md](PLAN-FINAL.md).

## Source and safety boundaries

Use only game data and assets the user is legally entitled to use. Keep game binaries, assets, account tokens, and saves out of source archives. Test only on a PS3 the tester owns or is authorised to use; do not bypass signatures, DRM, or game licensing.
