# Mapa de ownership — primera rebanada offline PS3

**Rama:** `ai/manus-ps3-port`
**Upstream de referencia:** `horizon-rust/horizon` en `0628aa587022bc584eca0f479d76103755c30db5`
**Estado:** mapa de implementación; no reclama integración PS3.

## Objetivo de la primera rebanada

Construir un visor offline de un mundo local o fixture determinista: cargar/crear una cantidad limitada de chunks, generar mesh de terreno en CPU, mover una cámara con DualShock 3 y presentar geometría básica por RSX. Quedan fuera launcher, autenticación, red, mods, telemetry, audio, clipboard, IME, shader packs, splitscreen y postprocesado avanzado.

La rebanada debe tener un entrypoint PS3 separado y no modificar `main` del cliente original.

## Ownership por capa

| Capa | Fuente upstream candidata | Responsabilidad portable | Dependencia que debe aislarse |
|---|---|---|---|
| Coordenadas y matemáticas | `crates/horizon-core/src/position.rs`, `crates/horizon-client-render/src/camera.rs` | posiciones, matrices, FOV, cámara | `glam`/`libm`, endianess y alloc |
| Datos de mundo | `crates/horizon-world/src/chunk.rs`, `src/block_render.rs`, `src/heightmap.rs` | estados de bloques, secciones y consultas | filesystem/JSON/registry loading |
| Generación | `crates/horizon-core/src/noise.rs`, `src/density.rs`, `crates/horizon-world/src/worldgen/` | fixture o generación acotada | gran huella de `std`, datos y caches |
| Colisiones | `crates/horizon-world/src/block_collision.rs`, `src/block_collision_scan.rs` | consultas de AABB y selección | carga de tablas de colisión |
| Meshing | `crates/horizon-client-render/src/mesher/`, `src/chunk/` | quads visibles, vertices e índices | tipos de recursos/texturas y estado GPU |
| Cámara/render view | `crates/horizon-client-render/src/camera.rs`, `src/gfx/view.rs` | view/projection y culling básico | `wgpu` solo en `gfx/view.rs` si aparece indirectamente |
| Input | `crates/horizon-client-input/src/input.rs`, `src/interaction_input.rs`, `src/keymapping.rs` | acciones, edge transitions, movimiento | `winit::KeyCode`, mouse e IME |
| Loop | `crates/horizon-client/src/game/event_loop.rs` como referencia | tick/render pacing y shutdown | `winit::EventLoop`, threads y `std::time` |
| Renderer host | `crates/horizon-client-render/src/gfx.rs` | contrato de referencia | `wgpu::Device/Queue/Surface` no deben cruzar al backend PS3 |
| Renderer PS3 | nuevo `renderer/rsx/` al importar baseline | buffers, uploads, draw, present | FFI PSL1GHT, memoria XDR/GDDR3, fences |
| Runtime PS3 | nuevo `platform/ps3/` o crate existente del baseline | clock, alloc, files, logging, pad, sync | contratos reales PSL1GHT/Newlib |

## Tipos que deben cruzar la frontera renderer

El contrato común no debe exponer `wgpu` ni handles RSX. La primera versión debería usar tipos de datos propios equivalentes a:

```text
FrameTarget { width, height, color_format }
CameraUniform { view_proj: [[f32; 4]; 4], origin: [f32; 3] }
TerrainVertex { position, uv/layer, light, color }
ChunkMesh { chunk_pos, vertices, indices, bounds }
InputSnapshot { actions, left_stick, right_stick, triggers, pressed, released }
```

El backend host puede traducirlos a buffers/pipelines `wgpu`; el backend RSX puede traducirlos a command buffers y memoria de video. Ningún crate de mundo debe almacenar `wgpu::Buffer`, `wgpu::Texture` o tipos de PSL1GHT.

## Qué reutilizar y qué no

### Reutilizar después de una auditoría de compilación

- Matemáticas puras y pruebas de `camera.rs`.
- Coordenadas de `horizon-core`.
- Algoritmos de visibilidad y packing de vertices de `mesher/packed.rs` si sus tipos se desacoplan.
- Ordenamiento y selección de caras de `mesher/sides.rs`.
- Reglas de input que puedan recibir acciones abstractas en lugar de `winit::KeyCode`.
- Consultas de colisión que operen sobre traits de mundo y no sobre carga de archivos.

### Mantener fuera de la primera rebanada

- `horizon-client/src/entry.rs` y `src/game.rs` completos: inicialización desktop, filesystem, `winit`, threads y servicios.
- `horizon-client-render/src/gfx.rs` completo: está construido sobre `wgpu` y contiene device loss, surfaces, submit threads, readback, timing y postprocesado.
- `horizon-client-input/src/window.rs`: fullscreen, monitores, video modes, iconos y objetos `winit`.
- `gfx/pack_*`, `native_post`, `visbuffer`, sombras, HDR, shader packs y compute culling.
- Networking, `ureq`, `mio`, autenticación, friends, telemetry, narrator, audio y mods.

## Primer contrato de plataforma

El baseline real deberá aportar o recibir una interfaz equivalente a:

```text
Platform::now_ns() -> u64
Platform::sleep_until(deadline_ns) -> Result<(), PlatformError>
Platform::log(level, bytes)
Platform::read_resource(path, dst) -> Result<usize, PlatformError>
Platform::write_save(path, bytes) -> Result<(), PlatformError>
Platform::poll_input() -> InputSnapshot
Platform::framebuffer_size() -> [u32; 2]
Platform::present(frame) -> Result<(), PlatformError>
```

Los nombres son diseño, no código existente. Antes de implementarlos hay que mapear cada operación a una API real y registrar `declarada`, `enlazada` y `ejecutada`.

## Orden de implementación cuando el baseline esté disponible

1. Importar el baseline completo en una rama de importación, verificando licencia, hashes y ausencia de binarios no trazables.
2. Ejecutar el smoke test existente del baseline y conservar el log exacto.
3. Añadir un crate de contrato común para `InputSnapshot`, `ChunkMesh`, `CameraUniform` y errores de plataforma.
4. Implementar un adapter host determinista para probar el contrato sin RSX.
5. Traducir un fixture pequeño de bloques a `ChunkMesh` y probar vértices/índices en host.
6. Conectar `InputSnapshot` a una fuente de teclado host y a una fuente DualShock 3 separadas.
7. Implementar en RSX únicamente clear, triángulo, buffer de vértices, textura 2D, depth y presentación.
8. Conectar un único `ChunkMesh` al backend RSX.
9. Añadir cámara y movimiento acotado.
10. Añadir carga/guardado local solo cuando filesystem y límites de memoria estén probados.

## Pruebas de aceptación

- El mismo fixture produce el mismo hash de `ChunkMesh` en host y PS3.
- El contrato común compila sin `wgpu`, `winit` ni FFI PSL1GHT.
- El renderer RSX no necesita una ventana `winit`.
- La prueba de input verifica pressed/released, deadzone, inversión de ejes y pérdida de foco/reset.
- El renderer verifica bounds, stride, endianess, alineación y liberación de buffers.
- El frame loop registra tiempo, memoria XDR/GDDR3, uploads, stalls y errores de presentación.
- Build host, build PS3, link/post-link, RPCS3 y hardware se reportan como resultados independientes.

## Bloqueo actual

El código necesario para ejecutar este mapa no está en los refs Git disponibles. El adjunto menciona un ZIP externo con el baseline PS3, pero no está presente en `/home/ubuntu/upload` ni en otro path local descubierto. Por eso este archivo documenta ownership y no crea implementaciones ficticias.
