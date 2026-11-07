# Informe de avance — port clean-room de Horizon a PS3

**Fecha:** 6 de noviembre de 2026  
**Alcance:** reimplementación no oficial de Minecraft Java en Rust; no se portó el ejecutable propietario de Minecraft ni se incluyeron archivos o recursos propietarios.

## Estado actual

El cliente experimental PS3 compila y se empaqueta como homebrew para PS3 con HEN. Incluye mundo procedural por chunks, greedy meshing, streaming acotado, cámara, física terrestre, gravedad, salto, colisiones, selección de bloques por raycast, romper/colocar bloques, hotbar de tres materiales, HUD RSX de color plano, pausa y panel de inventario básico.

El artefacto más reciente se generó en:

- `build/rust-ps3-link/rust-world/USRDIR/EBOOT.BIN`
- `build/rust-ps3-link/rust-world/PARAM.SFO`

El ZIP entregable está en `/home/ubuntu/work/horizon-ps3/horizon-world-ps3-inventory.zip`.

## Toolchain y enlace

Se utiliza Rust nightly con `rust-src`, target `powerpc64-sony-ps3`, `core`/`alloc` construidos con `-Z build-std`, PPU GCC/PSL1GHT de PS3DEV, shaders RSX VPO/FPO precompilados y `moldier` para aplicar el formato CellLV2. El ELF final es PowerPC64 big-endian ELFv1, con OS/ABI CellLV2 `0x66`, seis program headers y entrypoint `0x10000000`.

El wrapper `ports/ps3/rust-entry/` mantiene una staticlib independiente y el bridge C/PSL1GHT enlaza el entrypoint `horizon_ps3_rust_world_entry` con RSX, input y librerías GameOS. `make_self` genera `EBOOT.BIN` y `sfo` genera `PARAM.SFO`.

## Funcionalidad del cliente

- Generación determinista de terreno y almacenamiento de bloques residente.
- Streaming nearest-first y presupuestos por frame para generación, meshing y upload.
- Greedy meshing con halos de chunks y caché de mallas.
- Proyección/recorte CPU y upload de vértices al RSX con layout explícito big-endian.
- Movimiento terrestre con gravedad, salto en `SQUARE`, colisión y respawn si se cae fuera del mundo.
- `CROSS` rompe el primer bloque sólido; `CIRCLE` coloca el material activo en el voxel vacío anterior.
- Hotbar de piedra, tierra y césped; selección por D-pad; contador inicial de 16 unidades; romper devuelve materiales y colocar consume unidades.
- HUD con mira contextual, ocho pips compactos por slot, icono de pausa y panel central de inventario.
- `START` pausa/reanuda y bloquea movimiento, física, streaming y edición. `TRIANGLE` abre/cierra inventario y también bloquea esas acciones. `START+SELECT` sale del cliente.

## Validación comprobada en esta iteración

- `cargo fmt --all`: correcto.
- Suite host de `horizon-ps3-platform`: **89/89 pruebas Rust pasaron**.
- Prueba C `software-raster-test`: **PASS**.
- `rust-ps3-staticlib`: correcto.
- Enlace PPU GCC + bridge PSL1GHT + RSX: correcto.
- `moldier patch`: correcto; validó OS/ABI `0x66`, entrypoint y seis segmentos.
- `make_self`: generó `EBOOT.BIN` de 332288 bytes.
- `sfo`: generó `PARAM.SFO` con app ID `HZNPS3002`.
- ZIP: `/home/ubuntu/work/horizon-ps3/horizon-world-ps3-inventory.zip`; SHA-256 `d522b9adbe5b5facd699012addcea4dde7f54f2904afd0643f7e5f5232570541`.

## Limitaciones conocidas

RPCS3 ARM64 del sandbox carga el ELF y sus imports, pero se detiene en `sbrk_init` antes de llegar al bucle del juego. Un programa C mínimo construido con el mismo PS3DEV/ppu-gcc falla en el mismo punto; por tanto, el bloqueo se atribuye al runtime RPCS3/PS3DEV del sandbox y no constituye una prueba de fallo del cliente. No hay todavía validación en una PS3 física.

El renderer actual usa colores planos y shaders de bring-up; aún no carga un atlas de texturas real. También quedan pendientes integrar el almacenamiento persistente completo de Horizon, guardado/carga compatible, iluminación AO/lightmap, menú de inventario con datos completos y mediciones de rendimiento en hardware.

## Próximos pasos

1. Integrar texturas autorizadas de Horizon mediante atlas RSX.
2. Conectar el almacenamiento real de bloques de Horizon con el mesher PS3.
3. Ampliar el inventario y añadir pantalla de selección/menú.
4. Implementar guardado y carga clean-room.
5. Añadir iluminación AO/lightmap y validar el presupuesto de memoria en hardware.
