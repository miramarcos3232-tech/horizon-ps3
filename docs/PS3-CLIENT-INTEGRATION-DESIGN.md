# PS3 client integration design — Step 48

Date: 2026-11-07
Status: architecture proposal based on the supplied Step 46 archive; no client integration is claimed.

## 1. Finding: there are two separate porting problems

The archive contains a native PS3 prototype and the original Horizon desktop client, but they are not yet one application.

The PS3 path is currently rooted in:
- `crates/horizon-ps3-platform` — a `no_std` platform contract and experimental platform logic.
- `ports/ps3/rust-entry` — a `no_std` static-library entry wrapper.
- `ports/ps3/` — C/PSL1GHT bridge, PPU/RSX build and packaging scripts, and the generated-world probe.

The desktop client path is rooted in:
- `crates/horizon-client`
- `crates/horizon-client-render`
- `crates/horizon-client-input`
- `crates/horizon-client-lib`
- `crates/horizon-world` and shared gameplay/data crates.

These graphs are not connected by the current PS3 entry wrapper. The wrapper calls the PS3 platform crate's smoke/world entrypoints, not the original desktop client's application loop.

## 2. Direct dependency blockers confirmed in manifests

| Crate | Relevant dependencies | PS3 consequence |
|---|---|---|
| `horizon-client` | `wgpu`, `winit`, `pollster`, `ureq`, `fs4`, `rustix` on Unix, `horizon-client-lib`, render/input/core crates | Desktop application entry and platform services are not a PS3 entrypoint |
| `horizon-client-render` | `wgpu`, `winit`, `pollster`, `cpal` optional, `ureq`, `rustix` on Unix | Renderer currently uses a desktop GPU abstraction and also owns substantial client UI/resource work |
| `horizon-client-input` | `winit`, `arboard`, `png` | Window events and system clipboard need a PS3-specific input implementation or explicit omission |
| `horizon-ps3-platform` | no external runtime dependencies in its manifest | Good isolated foundation, but its present Platform trait only describes display, pad, and frame presentation |

The client manifest's default features enable audio and controllers. A PS3 build must not inherit desktop defaults accidentally. Disabling default features alone is not enough: `wgpu`, `winit`, clipboard, network, filesystem, timing, and threading dependencies also need explicit target/feature boundaries.

## 3. Proposed architecture boundary

Do not attempt to make the entire existing desktop client `no_std` as the first step. Preserve the desktop build and introduce narrow interfaces at the seams that the PS3 game loop actually needs.

### A. Input
Define a game-facing input snapshot independent of `winit`:
- buttons/actions and edge transitions;
- two analog sticks and triggers;
- repeat/debounce policy;
- focus/pause state.

Desktop input maps `winit` events into this snapshot. PS3 maps the existing PSL1GHT pad state into the same snapshot. Keep clipboard, text IME, mouse and window configuration outside the first PS3 vertical slice.

### B. Rendering
Define a renderer contract in a small shared crate or module:
- create/release bounded GPU resources;
- upload chunk vertex/index data;
- set camera and frame constants;
- draw opaque terrain and simple HUD;
- report device/resource failures.

Keep the current `wgpu` implementation as the desktop backend. Implement a PS3 RSX backend behind the same contract. Do not put RSX FFI in world-generation or gameplay crates.

The existing `horizon-ps3-platform` frame API accepts a full RGBA framebuffer. That is useful for smoke tests, but it is not the final world-rendering interface: CPU-rendering the whole scene into RGBA would waste CPU and memory bandwidth. The RSX mesh upload path should become the PS3 renderer backend, with a simple software framebuffer retained only for tests/fallback diagnostics.

### C. Runtime services
Inventory and abstract only services required by the chosen local single-player slice:
- monotonic time / frame delta;
- bounded allocation and memory telemetry;
- file access for a local world and resource pack;
- deterministic/random source;
- worker scheduling and synchronization, if the slice actually needs them;
- logging and panic/fatal-error handling.

Networking, account authentication, telemetry, friends/relay, mods, cosmetics downloads, narrator, audio, and desktop window management are out of the first offline milestone. They must be explicitly disabled, not replaced with fake implementations.

## 4. Recommended first vertical slice

The first real Horizon integration milestone should be a local offline world viewer, not the complete launcher/client:

1. Load one known local test world or a generated world using Horizon's existing world/chunk data types.
2. Generate or load a small bounded set of chunks.
3. Convert Horizon block/chunk data into the PS3 mesh input format.
4. Upload mesh buffers through the RSX backend.
5. Render camera movement and basic controller input.
6. Add collision and block selection only after the world view is stable.
7. Add block edits and save/load as a separate milestone.

Acceptance criteria:
- the entrypoint links the real Horizon world/data crates, not only the generated PS3 test-world module;
- the same deterministic fixture produces the same chunk/mesh result on host tests;
- memory use is bounded and reported separately for XDR and GDDR3;
- no desktop window, clipboard, audio, online-auth or telemetry dependency is pulled into the PS3 target;
- build, package, emulator execution and physical-console execution are reported as separate results.

## 5. Do not do these shortcuts

- Do not create fake `std::thread`, TLS, filesystem, networking, entropy, or clock stubs just to satisfy the linker.
- Do not force the whole desktop client through a target build before its dependency boundaries exist.
- Do not use full-frame CPU RGBA presentation as the long-term terrain renderer.
- Do not treat a static library, ELF, SELF/EBOOT package, or successful link as proof of runtime playability.
- Do not delete or replace the working desktop backend while adding PS3 support.

## 6. Concrete next implementation task

Before modifying application code, create a dependency ownership map for the minimum offline slice:
- trace the public types and APIs needed from `horizon-world`, `horizon-client-core`, `horizon-client-render`, and `horizon-client-input`;
- identify which modules directly call desktop APIs or OS services;
- identify which modules can compile as ordinary shared Rust code once platform services are injected;
- define a minimal renderer/input/runtime interface and host fake tests.

Then implement one narrow adapter at a time. The current archive lacks an executable Rust toolchain in the review environment, so any code change must be validated in a separate environment with the documented nightly/PS3 SDK toolchain before being called buildable.

## 7. Repository and evidence policy

This design is staged on `ai/ps3-migration-staging`. It does not import the full archive, change `main`, create a PR, or claim that the original client runs on PS3.
