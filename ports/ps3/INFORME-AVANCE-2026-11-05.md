# Informe de avance — port de Horizon a PS3

**Fecha:** 5 de noviembre de 2026  
**Alcance:** port de Horizon, proyecto de código limpio; no es un port del ejecutable propietario de Minecraft ni incluye sus archivos.

## Resultado de esta sesión

- Se añadió `make -C ports/ps3 host-test`, que ejecuta en una sola orden las pruebas host del crate `horizon-ps3-platform` y el test C de rasterizado.
- Se implementó `greedy_mesh::mesh_opaque_greedy` sin asignación dinámica y se conectó el entrypoint FFI de prueba al encoder de rectángulos para su ruta upload/bind/draw.
- Se añadió `Camera` con proyección perspective a NDC y clipping CPU de triángulos contra near/far, conectado al probe antes del upload con buffers limitados por el presupuesto de 40.000 bytes; faltan matrices/uniformes GPU y clipping X/Y específico si RSX no lo cubre.
- Rust estable 1.99.1 y el target `powerpc64-unknown-linux-gnu` quedaron instalados en este sandbox.
- Pasaron **49/49 pruebas unitarias Rust** del crate PS3 y la prueba C `software triangle fill and reciprocal-depth occlusion`.
- Pasó `cargo check` con `psl1ght-ffi` para `powerpc64-unknown-linux-gnu` y se generó una biblioteca estática Rust `no_std` candidata de 11 MB.
- En el caso sintético de chunk completamente sólido, el greedy mesher fusiona 1.536 caras unitarias en seis rectángulos (reducción geométrica de 99,61% para ese caso, no una medición de FPS).
- Una prueba diferencial adicional compara área visible por dirección/material contra el mesher unitario y confirma que el greedy no pierde ni añade cobertura en una forma mixta con huecos.

Estas validaciones cubren lógica host y compilación cruzada candidata. No prueban compatibilidad ABI con PSL1GHT, enlace PPU, presentación RSX ni ejecución en consola.

## Estado comprobado del proyecto

El ZIP trae código de plataforma, prototipos de malla/chunks y SELF de prueba. El SELF es un *smoke test* de plataforma, no un juego jugable ni una escena de Minecraft. En este entorno no están disponibles `ppu-gcc`/PSL1GHT ni una PS3 o emulador para verificar el binario. Además, el cliente de escritorio depende de `winit`, `wgpu`, filesystem y otras capacidades que no se pueden llevar directamente a PS3 sin una capa de cliente/renderizado específica.

Siguen pendientes los bloqueos decisivos: enlazar el entrypoint real de Horizon al ABI PPU, renderer voxel RSX con cámara/texturas/profundidad, conexión con los datos de mundo y recursos autorizados, controles y ciclo jugable, guardado, mediciones y pruebas de estabilidad en hardware o emulador.

## Porcentaje pendiente

El plan `PLAN-FINAL.md` contiene **65 hitos** en sus seis fases: 26 marcados completos y 39 abiertos. Por conteo simple, queda **60% del checklist**.

Ese conteo pesa igual cada casilla y no representa horas de ingeniería: las fases 4, 5 y 6 —renderer, cliente jugable y entrega/estabilidad— siguen sin cerrar; la fase 4 ya tiene proyección y clipping near/far CPU en el probe, pero no un renderer completo. Para el objetivo práctico de tener un **port jugable de Horizon en PS3**, estimo que queda aproximadamente **85% del trabajo**, con alta incertidumbre; el avance real demostrado hasta ahora es principalmente la base de plataforma/prototipo.

## Siguiente incremento necesario

1. Conseguir una toolchain PSL1GHT/PS3DEV verificable en un host compatible y probar el enlace Rust↔PPU, sin considerar `powerpc64-unknown-linux-gnu` como ABI PS3 ya validado.
2. Arrancar el smoke SELF en hardware propio/autorizado o emulador compatible; registrar vídeo, TTY, mando, RSX y memoria.
3. Integrar un renderer voxel original con cámara, depth y texturas de prueba; después conectar almacenamiento de chunks y el loop del cliente.
4. Incorporar solo recursos que el usuario tenga derecho a usar y medir FPS, uso de memoria y estabilidad.
