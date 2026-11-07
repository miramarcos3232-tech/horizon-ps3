# Informe técnico — integración del cliente Horizon con PSL1GHT/RSX

**Fecha:** 2026-11-07
**Repositorio auditado:** `miramarcos3232-tech/horizon-ps3`
**Rama de trabajo:** `analysis/psl1ght-rsx-integration`
**Upstream auditado:** `horizon-rust/horizon` en `0628aa587022bc584eca0f479d76103755c30db5`
**Alcance:** análisis de arquitectura y propuesta; no se modificó `main` ni se implementó el port.

## 1. Resumen ejecutivo

El camino mínimo viable **no** es compilar el cliente original completo contra PSL1GHT ni intentar sustituir Vulkan/OpenGL por RSX detrás de `wgpu` sin una capa intermedia. El camino mínimo es una integración incremental en cuatro fronteras:

1. **Runtime/ABI PS3:** estabilizar el target `powerpc64-sony-ps3`, el enlazado ELFv1 y el post-link PS3; después implementar el PAL requerido por `std` si se pretende ejecutar el cliente original.
2. **Núcleo portable:** extraer y adaptar primero los módulos de simulación/datos que no dependen de ventana, GPU, audio, red o filesystem host.
3. **Plataforma PS3:** crear interfaces explícitas para reloj, memoria, filesystem, entrada, logging, hilos y presentación; implementar detrás de ellas PSL1GHT/Newlib y las primitivas reales de CellOS.
4. **Renderer separado:** mantener el renderer original `wgpu` intacto en host y crear un backend RSX específico para un subconjunto de operaciones. RSX no debe presentarse como backend `wgpu` hasta demostrar una implementación completa y compatible de la API requerida.

La conclusión de arquitectura es **port por capas/adaptador**, no recompilación directa. El cliente original depende de muchas capacidades que no ofrece una aplicación `no_std + alloc` por sí sola, y el renderer usa una superficie grande de `wgpu` que incluye pipelines, bind groups, buffers, texturas, compute, mapeo/readback, sincronización y WGSL/Naga.

## 2. Base comprobable del repositorio Horizon PS3

El repositorio local contiene documentación y auditorías, pero no contiene el workspace Rust del cliente original ni manifests de `wgpu`/`winit`. Por eso, las referencias al cliente y sus dependencias se verificaron contra el upstream separado indicado arriba.

La documentación existente del repositorio declara explícitamente que el port funcional, la compilación completa de `std`, el EBOOT del cliente original y las pruebas en RPCS3/PS3 no están demostrados. Los resultados históricos del handoff se conservan como evidencia reportada previamente, no como pruebas reproducidas en esta sesión.

No se ejecutaron compilaciones del target PS3, RPCS3 ni hardware real durante esta auditoría. Tampoco se ejecutó una compilación host del workspace upstream como parte de este informe.

## 3. Arquitectura del cliente original

El upstream separa aproximadamente estas responsabilidades:

```text
horizon-client (binario / gameplay / entrypoint)
        |
        +-- horizon-client-lib (biblioteca compartida y herramientas)
        +-- horizon-client-core (NBT, texto, scoreboard, registros)
        +-- horizon-client-input (winit, teclado, mouse, ventana, clipboard)
        +-- horizon-client-render (mundo, meshing, HUD, shaders, renderer)
        +-- horizon-core (simulación y generación)
        +-- world/items/protocol/assets/threads/shaders/telemetry/...
```

`horizon-client/src/main.rs` es deliberadamente pequeño: instala el allocator y llama a `horizon_client::entry::main()`. La inicialización real se encuentra en `src/entry.rs`, que usa `std::env`, filesystem, logging, crash reports, `std::time::Instant`, hilos y salida de proceso antes de iniciar el juego.

El bucle interactivo está en `horizon-client/src/game.rs`. Usa `winit::application::ApplicationHandler`, `EventLoop`, `ActiveEventLoop`, `WindowEvent`, teclado, mouse y `Window`. `game/event_loop.rs` abstrae solo el control del event loop (salir y elegir el próximo wake-up), pero los tipos de eventos continúan siendo tipos de `winit` en el juego.

El renderizador vive principalmente en `horizon-client-render/src/gfx.rs` y submódulos. El tipo `Renderer` conserva `wgpu::Instance`, `Adapter`, `Device`, `Queue`, `Surface`, `SurfaceConfiguration`, texturas, pipelines y estado de presentación. El código también contiene recuperación ante pérdida de dispositivo, submit en thread separado, pacing, readback, culling y recursos compartidos entre ventanas de splitscreen.

## 4. Dependencias relevantes

### 4.1 `wgpu`

Los manifests de `horizon-client`, `horizon-client-lib` y `horizon-client-render` fijan `wgpu = 30.0.1` con `default-features = false` y features `std`, `parking_lot`, `vulkan`, `metal`, `wgsl` y `naga-ir`. Hay features adicionales por plataforma para `dx12` en Windows y `gles` en plataformas no macOS.

El código de renderer usa directamente, entre otros:

- `Instance`, selección de `Adapter`, `Device` y `Queue`.
- `Surface`, capacidades, configuración y modos de presentación.
- `Buffer`, `Texture`, `TextureView`, `Sampler` y utilidades de upload.
- `RenderPipeline`, `ComputePipeline`, layouts, bind groups y estados de blend/depth.
- Command encoders, render/compute passes, timestamps y submission indices.
- Mapeo/readback de buffers y `COPY_BYTES_PER_ROW_ALIGNMENT`.
- WGSL y módulos Naga, incluyendo traducción/validación de shaders.

Esto excede ampliamente una interfaz de “dibujar triángulos”. En particular, el renderer utiliza compute/culling, múltiples targets, depth, storage buffers, texturas 1D/2D/3D/arrays, formatos específicos, shaders WGSL y operaciones de sincronización. No hay evidencia en el repositorio de que RSX pueda cumplir esta superficie a través de un backend `wgpu` existente.

**Propuesta:** no modificar inicialmente el renderer original para introducir un backend falso. Definir un renderer RSX independiente con un subconjunto explícito y medible, comenzando por clear, triángulo, buffer estático, textura 2D, depth, mesh de chunk simple y presentación. Solo evaluar una capa `wgpu-hal`/`wgpu` después de contar con un backend funcional y una matriz de capacidades real.

### 4.2 `winit`

`winit = 0.30.13` aparece directamente en `horizon-client`, `horizon-client-lib`, `horizon-client-render` y `horizon-client-input`. Se usa para:

- event loop y `ApplicationHandler`;
- creación y configuración de ventanas;
- `WindowEvent`, teclado físico/lógico, mouse e IME;
- cursores, captura del mouse, fullscreen y monitores/video modes;
- tamaños y posiciones físicos/lógicos;
- handle de ventana requerido por la superficie gráfica.

PSL1GHT no proporciona el modelo de ventana de escritorio que espera `winit`. En PS3 la aplicación debe tratar la salida de video y los eventos del controlador como servicios de plataforma. La sustitución mínima es un `PlatformEvent` propio y un loop PS3 que alimente una capa de entrada, dejando `winit` detrás de `cfg` para host.

**Propuesta:** mantener `winit` solo en el adaptador host. Para PS3, crear un módulo de plataforma que traduzca botones/analógicos del controlador y cambios de video a tipos Horizon internos. No intentar construir objetos `winit::Window` sintéticos.

### 4.3 `std` y runtime

El target Rust oficial `powerpc64-sony-ps3` es Tier 3, big-endian PowerPC64 ELFv1 para el PPE de CellOS. La documentación oficial confirma soporte de `core` y `alloc`, pero no distribuye artefactos precompilados de la biblioteca estándar por rustup; requiere nightly, `rust-src` y `-Z build-std=core,alloc`. El enlazador recomendado es `mold`; `lld` no maneja las relocaciones de llamadas ELFv1 indicadas para este target. El ELF requiere post-link PS3 antes de ser ejecutable.

El cliente original tiene una dependencia fuerte de `std`: en la auditoría del checkout upstream se observaron referencias de `std` en aproximadamente 713 archivos bajo `crates/horizon-client`, 233 bajo `horizon-client-render`, 56 bajo `horizon-core` y 446 bajo `horizon-world` (conteo de archivos que contienen `std::`, `use std` o `extern crate std`; no es una medida de funciones ni un resultado de compilación).

Capacidades relevantes usadas por el cliente y sus dependencias:

- allocator global y asignación dinámica;
- filesystem, paths, env, argumentos y archivos temporales;
- `Instant`, `Duration`, threads, channels, `Arc`, `Mutex`, atomics y TLS;
- sockets, DNS, HTTPS/TLS y polling de red;
- salida de proceso, señales/limitación de files y crash reports;
- audio, clipboard, gamepads y bibliotecas nativas host.

Por tanto, `core + alloc` solo permite un smoke test o una porción cuidadosamente aislada; no es suficiente para enlazar el cliente original sin un PAL/sys de `std` real o sin rediseñar una parte sustancial de sus dependencias.

## 5. Camino mínimo recomendado

### Fase A — reproducibilidad de toolchain

Fijar commit/versiones de Rust nightly, `rust-src`, `mold`, `moldier`, ps3toolchain, GCC/Newlib y PSL1GHT. Añadir archivos de build reproducible para un binario PS3 mínimo, sin mezclarlo con el cliente.

Criterios de salida:

- binario `no_std + alloc` enlazado para `powerpc64-sony-ps3`;
- ELF post-patcheado según CellOS;
- logs de comando, hashes y artefactos;
- ejecución documentada en RPCS3 antes de afirmar ejecución en hardware.

### Fase B — plataforma PS3 mínima

Crear un crate o módulo `platform/ps3` que exponga contratos propios para:

- `clock` y frame pacing;
- allocator/heap;
- logging y diagnóstico;
- filesystem de recursos/saves;
- entrada de controlador;
- memoria de video y buffers RSX;
- presentación y eventos de salida;
- sincronización/hilos únicamente donde exista una primitiva real.

La implementación debe registrar cada API como **declarada**, **enlazada** y **ejecutada**. No usar aliases POSIX por nombre parecido ni stubs que retornen éxito ficticio.

### Fase C — extracción del núcleo

El primer candidato es `horizon-core`, pero no debe asumirse `no_std`: su `Cargo.toml` depende de `libm = 0.2.16` y su `lib.rs` declara módulos de biome, carving, density, geometry, noise, paletted storage, position, random, storage, text y ticks; contiene usos de `std` según la auditoría. Auditar módulo por módulo y trasladar solo los que pasen:

- ausencia de filesystem/threading/I/O;
- dependencias compatibles con `core`/`alloc`;
- endianess y layout verificados;
- pruebas deterministas de paridad.

Después, evaluar `horizon-client-core` para tipos de datos, NBT/texto y tablas. No incorporar aún `horizon-client` completo ni `horizon-client-render` al target PS3.

### Fase D — bring-up RSX independiente

Implementar y probar, en orden:

1. entrada PSL1GHT/CellOS y salida de log;
2. allocator de heap y memoria de video;
3. clear de framebuffer y presentación;
4. triángulo con vertex buffer;
5. textura 2D y sampler;
6. depth buffer y pipeline simple;
7. mesh de chunk pequeño generado por CPU;
8. upload de recursos y doble/triple buffering;
9. medición de memoria, tiempo de frame y stalls.

Solo después definir qué parte de `horizon-client-render` puede reutilizarse: preferentemente meshing, cámara, formatos de vértices y lógica de selección, no sus objetos `wgpu`.

### Fase E — integración del cliente adaptado

El entrypoint PS3 debe ser nuevo y separado del `main` original. El orden mínimo sugerido es:

```text
PS3 entry
  -> Platform PS3
  -> world/core adaptado
  -> input PS3
  -> renderer RSX mínimo
  -> frame loop
```

La integración del gameplay completo, networking, assets complejos, audio, shaders de packs, mods, telemetry y launcher debe quedar fuera del primer milestone.

## 6. Archivos afectados

### En el repositorio Horizon PS3

Este cambio de análisis agrega únicamente este informe:

- `docs/PSL1GHT-RSX-INTEGRATION-REPORT.md`

No se modificó `main`, no se agregó un crate incompleto y no se alteraron los documentos previos.

Para la implementación futura, los cambios probables serían:

- `.cargo/config.toml` y `rust-toolchain.toml` para toolchain/target;
- `platform/ps3/` o `crates/horizon-ps3-platform/` para contratos y adaptación PSL1GHT;
- `renderer/rsx/` para el backend RSX, shaders y gestión de memoria;
- `tests/ps3/` para smoke tests y pruebas de integración;
- `tools/` y `ports/ps3/` para ABI, link, post-link y empaquetado;
- documentación de versiones, logs y matriz de capacidades.

No se recomienda modificar el `main` del cliente original. El entrypoint PS3 debe ser un binario separado que consuma las capas adaptadas.

### En el upstream Horizon, solo como referencia de port

Los puntos de acoplamiento principales auditados son:

- `crates/horizon-client/Cargo.toml`, `src/main.rs`, `src/entry.rs`, `src/game.rs` y `src/game/event_loop.rs`;
- `crates/horizon-client-lib/Cargo.toml` y `src/client_lib.rs`;
- `crates/horizon-client-core/Cargo.toml` y `src/lib.rs`;
- `crates/horizon-client-input/Cargo.toml`, `src/lib.rs` y `src/window.rs`;
- `crates/horizon-client-render/Cargo.toml`, `src/lib.rs`, `src/graphics.rs`, `src/gfx.rs` y submódulos `src/gfx/`;
- `crates/horizon-core/Cargo.toml` y `src/lib.rs`.

Estos archivos no fueron copiados ni modificados en este repositorio.

## 7. Pruebas necesarias

### Toolchain y ABI

- `rustc -Vv`, `cargo -V`, `rustup show` y hash de `rust-src`/nightly;
- build reproducible del smoke test con `-Z build-std=core,alloc`;
- inspección de ELF antes/después de `moldier patch`;
- ABI probe C/Rust contra headers PSL1GHT para tipos, calling convention, tamaños y endianess;
- enlace de allocator, logging, reloj y entrada sin stubs falsos.

### Host

- tests de `horizon-core` y del adapter portable en host;
- tests de serialización/endianess y paridad determinista;
- validación del renderer RSX-facing con un backend de captura o fake que no pretenda ser GPU;
- `cargo fmt --check`, `cargo clippy` y tests del workspace afectado, documentando el comando exacto y su salida.

### RPCS3 y hardware

- arranque del smoke test post-patcheado en RPCS3;
- clear, triángulo, textura, depth y mesh de chunk en RPCS3;
- logs de entrada, frame time, memoria PPU/RSX y errores de driver;
- repetición en PS3 real con HEN solo después de la validación del emulador;
- evidencia de artefactos y logs por commit.

Un `cargo check` host, una declaración de ABI o un build de `core/alloc` no demuestra que `std`, el cliente original, RSX ni un EBOOT sean funcionales.

## 8. Riesgos y decisiones no recomendadas

- **No** activar `wgpu` con features de Vulkan/Metal/GLES esperando que PSL1GHT provea esos backends.
- **No** crear un `winit::Window` sintético para ocultar la ausencia de window system.
- **No** declarar compatibilidad de WGSL/Naga con RSX sin decidir si habrá traducción offline a shaders RSX y probar sus límites.
- **No** portar todo `std` antes del smoke test `no_std + alloc`; eso aumenta el alcance sin validar entrypoint, memoria ni presentación.
- **No** llamar “cliente original integrado” a una demo C/RSX o a un crate de plataforma aislado.
- **No** declarar EBOOT funcional ni compatibilidad PS3 sin ejecución y logs conservados.

## 9. Fuentes

- Upstream Horizon auditado: `https://github.com/horizon-rust/horizon` — commit `0628aa587022bc584eca0f479d76103755c30db5`.
- Target oficial Rust PS3: `https://doc.rust-lang.org/nightly/rustc/platform-support/powerpc64-sony-ps3.html`.
- SDK PSL1GHT: `https://github.com/ps3dev/PSL1GHT`.
- Repositorio analizado: `https://github.com/miramarcos3232-tech/horizon-ps3`.

La documentación oficial del target Rust confirma Tier 3, big-endian PowerPC64 ELFv1, soporte de `core`/`alloc`, necesidad de nightly + `rust-src` + `build-std`, uso de `mold` y post-link PS3. El README oficial de PSL1GHT lo describe como SDK homebrew y remite al entorno `ps3dev`/`ps3toolchain`; sus samples gráficos y librerías son evidencia de una ruta RSX de bajo nivel, no de un backend `wgpu` o un window system `winit`.

## 10. Discrepancia con el estado descrito en el adjunto

El adjunto afirma que ya existen `ports/ps3/`, `crates/horizon-ps3-platform/` y `ports/ps3/rust-entry/`. Se comprobó el checkout local contra `origin/main`, `origin/ai/coordination-protocol` y `origin/ai/ps3-migration-staging`: esos paths no están presentes en ninguno de esos refs. La rama de staging sí contiene documentos que describen un archivo ZIP externo de continuación y dejan constancia de que ese archivo no fue importado a GitHub.

Por ello, en esta sesión no es posible modificar de forma verificable el prototipo mencionado ni integrarlo con el cliente original: el código fuente requerido no forma parte del repositorio accesible y el ZIP no está disponible como archivo local. Crear ahora crates equivalentes, stubs o funciones vacías incumpliría las reglas del adjunto y produciría una integración ficticia.

El siguiente gate de implementación es importar el baseline completo en una rama de importación separada, conservar licencias y provenance, y recién después ejecutar el mapa de dependencias de la rebanada offline. Hasta superar ese gate, el único cambio seguro en `ai/manus-ps3-port` es documentación auditable; `main` permanece intacto.

## 11. Importación del baseline de continuación

El archivo `horizon-ps3-continuation-2026-11-07-step46.zip` fue recibido y validado antes de la importación. Resultado de integridad: ZIP sin errores, 6.458 entradas, 5.484 archivos, SHA-256 `91adbe06d5766738566ff2da549fb02b0c72303068ee5d9fbee9bd4842f11355`. Contiene `ports/ps3/`, `ports/ps3/rust-entry/`, `crates/horizon-ps3-platform/`, `third_party/libc/`, el workspace Horizon y documentación de provenance. No contenía `.git`, `target/` ni salidas binarias; la copia importada fue comprobada nuevamente sin `.elf`, `.self`, objetos ni librerías generadas.

La importación se hizo en `ai/manus-ps3-port` conservando los documentos locales de análisis. `main` no fue modificada.

### Validación ejecutada en esta sesión

La validación se ejecutó con `PATH=/usr/local/cargo/bin:/home/ubuntu/.cargo/bin:$PATH`. Resultados reproducidos en esta sesión:

- `cargo +stable fmt --all -- --check`: **PASS**.
- `cargo +stable test -p horizon-ps3-platform --no-default-features --features host-stub`: **89 passed, 0 failed**; doctests: 0 tests, PASS.
- `RUSTUP_TOOLCHAIN=stable CARGO=/usr/local/cargo/bin/cargo make -C ports/ps3 host-test`: **PASS**; incluye los mismos 89 tests y `software triangle fill and reciprocal-depth occlusion: PASS`.

El primer intento de `make host-test` falló únicamente porque la receta invocó `cargo` sin `+stable` y rustup no tenía toolchain por defecto. No se modificó el Makefile para ocultar esa condición; la ejecución válida usó variables de entorno explícitas.

No se ejecutaron `abi-contract-probe`, `libc-official-check` ni builds PS3 porque el SDK no está instalado en este entorno (`/home/ubuntu/ps3dev` no existe). Los resultados históricos consignados en los documentos del ZIP siguen siendo históricos y no fueron tratados como reproducidos.
