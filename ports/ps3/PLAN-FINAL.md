# Plan final del port PS3

## Estado honesto

El repositorio conserva código y artefactos de un bring-up RSX en C/PSL1GHT compilado en un entorno anterior. En esta continuación se validaron los tests host, el target Rust oficial, la copia vendorizada de `libc` con `core,alloc` y una sonda ABI C contra headers PSL1GHT. Aún no se ejecutó en PS3 o emulador y no es un port jugable terminado de Minecraft/Horizon. El siguiente trabajo debe implementar el PAL `std` específico y convertir el prototipo en un cliente real.

Estimación cualitativa: la capa de arranque PS3 está avanzada; el port completo del juego todavía está en una fase temprana. No es correcto llamarlo "terminado" hasta completar la integración del cliente, probar en una PS3 autorizada y verificar rendimiento/estabilidad.

## Bloqueadores externos

- [ ] Ejecutar el SELF en una PS3 propia o autorizada con HEN y capturar TTY, vídeo, input y salida RSX.
- [x] Seleccionar y compilar el target Rust oficial `powerpc64-sony-ps3` (`core`/`alloc`, nightly + `rust-src`).
- [x] Compilar `libc 0.2.190` vendorizado con el target oficial mediante el crate reproducible `libc-check`.
- [x] Verificar tamaños/offsets ABI y símbolos PSL1GHT con `abi-contract-probe`.
- [x] Construir PS3DEV/PSL1GHT/ps3libraries localmente para ARM64 y verificar PPU GCC, RSX headers/libraries, `make_self` y `sfo`.
- [x] Validar el enlace inicial del ABI Rust/PPU con el C bridge y PS3DEV mediante `rust-ps3-link-probe`; falta validación en hardware.
- [ ] Usar únicamente datos, mundos, texturas y sonidos aportados legalmente por el usuario. El repositorio no debe contener binarios, assets ni código propietario de Minecraft.
- [ ] Si no se dispone de consola, establecer un emulador compatible que ejecute este SELF; el entorno actual no ofrece esa validación.

## Fase 1 — Plataforma PS3 y ABI

- [x] Instalar PS3DEV, PSL1GHT y ps3libraries reproducibles.
- [x] Generar `PARAM.SFO`, `EBOOT.BIN` y carpeta HEN mínima.
- [x] Configurar vídeo, doble buffer RGBA8 y profundidad Z24S8.
- [x] Implementar pad polling, TTY, begin/present y shutdown limitado.
- [x] Añadir telemetría separada XDR/RSX y rechazo por exceso de presupuesto.
- [x] Definir vertex format big-endian XYZ F32 + UV F32 + RGBA8.
- [x] Implementar upload RSX persistente limitado a 4 MiB.
- [x] Implementar binding de atributos y draw indexado con índices 16-bit.
- [x] Compilar y cargar vertex/fragment programs Cg de bring-up.
- [x] Compilar y enlazar el C/RSX bring-up con PS3DEV, usando blobs VPO/FPO precompilados cuando no está disponible un compilador de shaders fuente.
- [x] Generar un paquete smoke `EBOOT.BIN`/`PARAM.SFO` con `make_self` y `sfo`; no ejecutado aún.
- [x] Hacer que el smoke-test dibuje después de copiar el frame CPU y antes de `gcmSetFlip`.
- [ ] Verificar en hardware el modo de vídeo, colores, triángulo, input y salida con START.
- [ ] Medir realmente XDR/GDDR3 y sustituir las capacidades placeholder.
- [x] Allocator acotado host-testable (`bounded_alloc`: arena con marcas + pool de slots con detección de doble free) y `RegionAllocator` opcional; pendiente medirlo en hardware.
- [ ] Implementar teardown RSX real o documentar un único boot por proceso como restricción permanente.

**Criterio de salida:** SELF estable en hardware, sin corrupción visual, sin overflow de pools y con un informe de memoria/frames por segundo.

## Fase 2 — Enlace del cliente Horizon

- [ ] Seleccionar qué crate constituye el entrypoint del cliente en PS3.
- [x] Generar un `no_std` staticlib Rust candidato y enlazarlo con `ppu-gcc` mediante `rust-ppu-link-probe`.
- [x] Ejecutar desde C un entrypoint Rust que arranca `Psl1ghtApi`, valida memoria/input y presenta un frame válido.
- [x] Codificar un triángulo con `rsx_mesh` desde Rust y recorrer upload → binding → draw RSX.
- [x] Generar un chunk 16³ de prueba, meshearlo y enviar sus seis caras visibles como 36 vértices RSX.
- [x] Recorrer una solicitud en cola y renderizar un volumen 4×4×4 de dos materiales (96 quads / 576 vértices).
- [x] Procesar dos chunks adyacentes, rellenar halos compartidos y verificar el culling de sus 16 caras internas por chunk.
- [x] Introducir `ChunkStore` fijo con grids haloed, revisiones y dirty flags conectados a `MeshQueue`.
- [x] Cambiar un bloque, incrementar solo la revisión del chunk afectado y reconstruir/re-subir únicamente su malla.
- [x] Coalescer múltiples cambios pre-worker a una sola revisión y preservar dirty cuando la cola está llena.
- [x] Limitar por frame el número de mallas iniciadas y los bytes acumulados enviados a RSX.
- [x] Crear el adaptador de build que enlace Rust con el bridge C y las librerías PSL1GHT.
- [ ] Sustituir el probe Rust por el entrypoint del cliente Horizon real.
- [x] Decisiones de ABI, panic handler con TTY y allocator global opcional documentados en `RUST-ABI.md` (sin verificar en hardware).
- [x] Compilar `horizon-ps3-platform` con `powerpc64-sony-ps3` y producir `libhorizon_ps3_entry.a` en release desde un wrapper separado.
- [x] Enlazar la staticlib oficial al C bridge con PS3DEV; el probe funciona, pero aún no es el entrypoint del cliente Horizon.
- [x] Aplicar `moldier`, verificar OS/ABI CellLV2 y generar un paquete `EBOOT.BIN`/`PARAM.SFO` del probe Rust.
- [x] Enlazar y empaquetar el entrypoint experimental de mundo Rust con cámara, streaming, meshing, input y draw RSX.
- [x] Añadir edición básica: CROSS rompe, CIRCLE coloca piedra, con raycast acotado, revisiones y halos vecinos.
- [x] Instalar `moldier` y parchear a OS/ABI CellLV2 un ELF Hello World independiente; no es el ejecutable Horizon ni valida su enlace C/RSX.
- [ ] Eliminar la dependencia de un entrypoint C de prueba para el ejecutable del cliente.
- [ ] Conectar `Ps3Platform` al bucle de juego real.
- [ ] Conectar input DualShock 3 al sistema de input de Horizon.
- [ ] Implementar logging y diagnóstico sin filesystem/OS de escritorio.
- [ ] Implementar y probar selección PAL `std` PS3 para allocator, I/O, errores, sincronización, TLS y random.

**Criterio de salida:** el ejecutable Rust real arranca, crea la plataforma PS3, lee input y mantiene un bucle de frames en hardware.

## Fase 3 — Mundo, chunks y meshing

- [x] Mesher host-testable de chunk 16³ con halo de un voxel.
- [x] FIFO fija de solicitudes de meshing sin asignación dinámica.
- [ ] Conectar almacenamiento de bloques de Horizon al mesher.
- [ ] Definir materiales opacos/translúcidos y reglas de caras visibles.
- [x] Añadir greedy meshing para caras opacas coplanares del mismo material; host-testable y conectado al probe Rust/RSX (pendiente validación PPU/RSX en hardware).
- [ ] Añadir payload de luz, AO/iluminación, normales, UV y tintes.
- [ ] Añadir worker scheduling acotado para Cell/SPU o un fallback PPU medible.
- [ ] Añadir streaming de chunks alrededor del jugador con límites XDR/GDDR3.
- [ ] Reutilizar buffers RSX por chunk y retirar mallas fuera de la ventana visible.

**Criterio de salida:** un mundo pequeño de prueba carga varios chunks, genera mallas correctas y no supera memoria bajo movimiento.

## Fase 4 — Renderer RSX real

- [ ] Sustituir el shader de bring-up por vertex/fragment programs de mundo.
- [ ] Completar matriz cámara/proyección y constantes por frame en RSX (el probe ya proyecta y recorta near/far en CPU; faltan uniformes GPU y validación en hardware).
- [ ] Añadir atlas de texturas con datos de prueba originales.
- [ ] Implementar upload de texturas, estados de sampler y gestión de lifetime.
- [ ] Implementar depth, culling, alpha-test/transparencia y orden de capas.
- [ ] Implementar batches por material/chunk para reducir cambios de estado.
- [ ] Añadir frustum/occlusion culling apropiado para el presupuesto de RSX.
- [ ] Medir draw calls, triángulos, uploads y tiempo de CPU/RSX.

**Criterio de salida:** renderizar un mundo voxel de prueba con cámara libre, texturas, profundidad y transparencias básicas.

## Fase 5 — Cliente jugable

- [ ] Cargar recursos desde una fuente legal proporcionada por el usuario.
- [ ] Implementar menú mínimo, pausa y pantalla de error.
- [ ] Movimiento, cámara, salto, colisión y selección de bloque.
- [ ] Romper/colocar bloques y actualizar chunks vecinos.
- [ ] Inventario/toolbar mínimo.
- [ ] Guardado/carga de mundo en el formato permitido por el proyecto.
- [ ] Sonido opcional y configurable; no bloquear el port por audio.
- [ ] Networking solo después del modo local estable.

**Criterio de salida:** iniciar un mundo local, moverse, mirar, romper/colocar bloques, guardar y volver a cargar sin crash.

## Fase 6 — Rendimiento, compatibilidad y entrega

- [ ] Presupuesto de memoria medido por perfil 576p/720p.
- [ ] Objetivo de frame time definido y medido en hardware.
- [ ] Pruebas largas de 30–60 minutos con streaming y cambios de chunks.
- [ ] Pruebas de pérdida de input, falta de memoria y datos corruptos.
- [ ] Empaquetado HEN reproducible con versión, título e ID configurables.
- [ ] Manual de instalación que no incluya firmware, DRM ni datos propietarios.
- [ ] Informe de limitaciones y lista de funciones no implementadas.

**Criterio de salida:** build reproducible, instalación documentada, pruebas de estabilidad completadas y estado funcional declarado sin exageraciones.

## Orden de ejecución recomendado

1. Fijar el ABI Rust/PPU y enlazar la staticlib oficial con el C bridge mediante un SDK seleccionado.
2. Aplicar el post-link PS3 y validar el smoke ELF en RPCS3 o hardware autorizado.
3. Allocator/telemetría medida.
4. Integración chunk storage → mesher → upload RSX.
5. Renderer voxel con recursos de prueba originales.
6. Movimiento y edición de mundo local.
7. Guardado, optimización y pruebas largas.
8. Recursos legales del usuario y empaquetado final.

## Definición de "terminado"

El port solo se puede marcar como terminado cuando se cumplan simultáneamente los criterios de salida de las seis fases, exista una ejecución comprobada en PS3 o emulador compatible y el paquete no dependa de recursos propietarios incluidos en este repositorio.
