# Estrategia eficiente para el port original de Horizon a PS3

## Conclusión

La ruta más eficiente no es intentar compilar todo `horizon-client` inmediatamente ni escribir un cliente alternativo. El target oficial `powerpc64-sony-ps3` soporta actualmente `core` y `alloc`, pero el propio toolchain `rust-ps3` marca `std support` como pendiente [1] [2].

La estrategia recomendada es reutilizar el runtime POSIX/newlib del SDK PS3 y añadir soporte específico de PS3 en dos capas pequeñas y verificables:

1. **ABI de `libc` para PS3.** Añadir tipos, constantes y declaraciones que correspondan a los headers reales de PSL1GHT/PS3DK: filesystem, sockets, tiempo, threads, mutexes y random. No reutilizar el módulo Rust `horizon`, porque pertenece a otro sistema.
2. **Integración mínima de `std`.** Seleccionar la ruta Unix/newlib solo donde la ABI PS3 sea compatible y añadir únicamente los módulos que faltan para PS3, empezando por random, tiempo, threads y filesystem. Cada módulo debe enlazar a las funciones reales de PSL1GHT o PS3DK y tener una prueba de enlace PPU.
3. **Prueba incremental con código real de Horizon.** Antes de intentar `horizon-client`, compilar las dependencias originales en orden y detenerse en la primera dependencia que requiera una API no disponible. No crear un runtime paralelo.
4. **Separación del frontend.** Mantener el gameplay y los tipos originales de Horizon; sustituir únicamente las capas desktop que no existen en PS3: `winit`, `wgpu`, audio, ventanas y entrada. El `--headless` existente sirve como referencia de bucle, pero no elimina las dependencias de `std`, red y filesystem.

## Por qué se descartan las otras rutas

- **Portar todo `std` desde cero:** demasiado grande y duplica la base POSIX que ya aporta newlib/PSL1GHT.
- **Usar el backend `horizon` de Rust/libc:** incorrecto; ese nombre corresponde a otro entorno y no demuestra compatibilidad con CellOS.
- **Cambiar el target a Linux:** puede producir objetos compilables, pero generaría una ABI y llamadas de sistema equivocadas para PS3.
- **Convertir todo el cliente a `no_std`:** obligaría a reescribir filesystem, red, threads, assets y renderer, y perdería la mayor parte del cliente original.

## Riesgos que deben verificarse primero

PS3DK documenta una base newlib 4.4.0, `libsysbase/libgloss`, pthread y sockets [3]. También documenta que su ABI PPU predeterminada es ELF64 con punteros C de 32 bits, mientras que el target Rust PS3 actual es ELFv1 con punteros de 64 bits [1] [3]. Antes de enlazar `std`, hay que fijar explícitamente el ABI usado por Rust y C y verificar tamaños de `time_t`, `off_t`, `pthread_t`, punteros y estructuras de filesystem.

La primera implementación debe ser un pequeño conjunto de declaraciones ABI y una prueba de enlace, no una modificación masiva de Horizon.

## Referencias

[1]: https://doc.rust-lang.org/nightly/rustc/platform-support/powerpc64-sony-ps3.html "Rust target powerpc64-sony-ps3"
[2]: https://github.com/ZephyrCodesStuff/rust-ps3/blob/main/README.md "rust-ps3 README and roadmap"
[3]: https://git.rpcs3.net/rpcs3/PS3DK "RPCS3 PS3DK toolchain and SDK"
