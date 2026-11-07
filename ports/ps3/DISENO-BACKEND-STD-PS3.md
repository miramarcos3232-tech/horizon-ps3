# Diseño del backend `std` de Rust para PS3

**Estado:** análisis de compatibilidad; no existe aquí un `std` PS3 funcional demostrado. Rust nightly, `rust-src`, el SDK PS3DEV/PSL1GHT y una sonda ABI reproducible ya están instalados y verificados. La copia vendorizada de `libc` compila con el target oficial `powerpc64-sony-ps3`; el bloqueo restante está dentro de la selección PAL de `std`, no en el ABI básico ni en el gameplay.

## Conclusión

Las fuentes upstream muestran APIs reales de PSL1GHT para filesystem, red, tiempo, threads/sincronización y salida de proceso. Sin embargo, sus firmas, estructuras y semánticas no demuestran el contrato POSIX que usa el PAL Unix de Rust. `cfg(unix)` selecciona ese PAL genérico; no certifica que GameOS sea POSIX. El target upstream `powerpc64-sony-ps3` está marcado `std=false`, y `libc 0.2.190` no incluye soporte directo para `powerpc64` con el perfil Newlib descrito en el target experimental. [1] [2] [3]

La ruta segura es diseñar un PAL/sys específico de PS3 y habilitar capacidades solo tras probar su contrato real. No se deben declarar las funciones LV2 como aliases `pthread`/POSIX, copiar los tipos de Linux ni añadir stubs para que compile. La ausencia de una API en las fuentes revisadas no prueba que no exista en otra biblioteca o versión del SDK; significa que todavía no hay evidencia suficiente para depender de ella.

## Hallazgos por subsistema

### Selección de target y `libc`

La receta de ps3toolchain construye GCC para `powerpc64-ps3-elf` con Newlib 1.20.0 parcheada y opciones multithread. Esto confirma cómo se configura esa toolchain, pero no acredita un runtime pthread enlazable ni equivalencia POSIX. El target Rust experimental y el triple del GCC deben validarse como una combinación ABI concreta; no se deben confundir con PowerPC64 Linux. [1] [2]

En `libc 0.2.190`, el módulo newlib no ofrece una rama utilizable para `powerpc64-sony-ps3`: la combinación indicada queda fuera de las arquitecturas/OS admitidos. El soporte que existe para `powerpc64` Linux GNU no es una definición ABI para PSL1GHT. [3]

### Archivos y descriptores

PSL1GHT declara operaciones LV2 de archivos y directorios (`sysLv2Fs*`/`sysFs*`). Son APIs reales, pero las llamadas usan contratos propios: estado y valores de salida por puntero, tamaños/contadores distintos y estructuras específicas. Por ejemplo, `sysFSStat` no es una `struct stat` POSIX intercambiable. [4] [5]

El parche PS3 de Newlib aporta wrappers de aspecto POSIX y callbacks reentrantes; varios fallan con `ENOSYS` si el callback no está instalado. El CRT PSL1GHT conecta algunos callbacks de archivo/TTY, pero no todos los que necesita Rust. Las fuentes revisadas tampoco prueban todo el conjunto requerido por el PAL Unix —entre otros `dup`, `poll`/`fcntl`, `readv`/`writev`, saneamiento de fd estándar y operaciones `*at`—. Hay que revisar símbolos y link map del SDK exacto, no inferir soporte a partir de un prototipo. [6] [7] [8]

### Red y sockets

PSL1GHT aporta syscalls `sysNet*`, APIs `net*` y wrappers BSD para operaciones comunes. Algunos wrappers dependen de inicializar la red y reconocen descriptores etiquetados; esos valores no deben tratarse automáticamente como fd Unix ordinarios. Los headers declaran layouts propios, como `socklen_t = u32`, `sa_family_t = u8` y `sockaddr` con campo de longitud/familia. Se requiere comprobar tamaño, alineación y offsets con la toolchain seleccionada. [9] [10] [11]

Rust Unix usa `getaddrinfo`/`freeaddrinfo` y `sockaddr_storage` para resolver y convertir direcciones. En las fuentes PSL1GHT/PS3DK revisadas no se acreditó ese resolver ni `sockaddr_storage`; una declaración de `addrinfo`, `AF_INET6` o `socketpair` no prueba una implementación enlazable o funcional. La vía PS3 revisada muestra `gethostbyname`/`gethostbyaddr`, que no es automáticamente equivalente al contrato de `ToSocketAddrs`. [10] [12] [13]

### Threads, sincronización y TLS

PSL1GHT sí expone creación/join/detach de threads PPU y primitivas LV2 de mutex/condvar. Pero sus tipos y firmas son específicos: la función de entrada y retorno difiere de pthread; la condición PSL1GHT se asocia con un mutex y espera con timeout relativo. No es correcto declarar esas estructuras o símbolos como `pthread_*` sin un adaptador probado. [14] [15] [16]

Rust Unix espera pthread para threads y sincronización, además de claves TLS con destructores. La receta de toolchain que activa opciones multithread no demuestra que la biblioteca PS3 tenga esos símbolos, layouts y semánticas. Las fuentes inspeccionadas no acreditan la API completa pthread/TLS para PSL1GHT; ese punto sigue bloqueado hasta inspección de headers, archives y pruebas. [1] [17]

### Tiempo y esperas

PSL1GHT proporciona `sysGetCurrentTime`, `sysGetTimebaseFrequency`, `sysSleep`, `sysUsleep` y lectura del timebase. Son APIs reales, pero no se probó que sean equivalentes a `clock_gettime(CLOCK_MONOTONIC)`, `nanosleep` o `pthread_cond_timedwait`. Rust separa `SystemTime` de `Instant`, y `thread::sleep` no debe terminar antes de la duración pedida; sus contratos de monotonía, resolución, rangos y errores deben implementarse y probarse por separado. [18] [19]

### Procesos, señales y salida

PSL1GHT tiene una ruta normal de salida mediante `sysProcessExit`; `sysProcessExitSpawn2` reemplaza la ejecución actual y no demuestra el modelo padre-hijo de `Command::spawn`. La tabla de syscalls incluye constantes de wait/kill, pero eso por sí solo no prueba wrappers, firmas, permisos o formato de estado POSIX. El parche Newlib examinado deja `fork`, `kill` y `wait` como `ENOSYS`, mientras `execve`/`getpid` dependen de callbacks. [20] [21] [22]

Rust Unix `Command` espera procesos, pipes, señales y decodificación de estados Unix. Tampoco se verificó que un estado LV2 tenga ese encoding. No debe implementarse `Command` fingiendo que existe; si el PAL PS3 no ofrece la capacidad, debe declararse no soportada de manera explícita. [23]

### Argumentos, entorno e identidad

El CRT PSL1GHT entrega `argc/argv`; el parche Newlib inicializa `environ` vacío y el CRT citado no entrega `envp`. Además, la selección actual de argumentos Unix en Rust no incluye `ps3` en la lista investigada. `sysProcessGetPid` tampoco demuestra un símbolo `getpid()` compatible, y las declaraciones comunes de uid/gid en `libc` no prueban esos servicios para GameOS. [7] [24] [25]

## Secuencia recomendada

1. Fijar commits/versiones de `rustc`/`rust-src`, `libc`, GCC/binutils/linker, Newlib parcheada y PSL1GHT. La selección custom de target de Rust es inestable; guardar target JSON y flags.
2. Reinstalar o reconstruir PS3DEV/PSL1GHT en el entorno de trabajo. Compilar un programa C de inspección que registre tamaños, alineaciones y offsets de tipos de filesystem, red, tiempo, threads y proceso; inspeccionar archives con `nm`/`readelf` y conservar link map.
3. Hacer pruebas C pequeñas de cada API candidata y distinguir claramente **declarada**, **enlazada** y **ejecutada**. Probar primero entrada/salida, argumentos y filesystem; luego tiempo; después threads/sync/TLS; luego sockets y resolver; dejar procesos/señales para el final.
4. Fijar un snapshot de `rust-src` y crear el esqueleto PAL/sys PS3. Para cada capacidad Rust, registrar el símbolo PSL1GHT correspondiente, ABI, prueba y limitación. Usar `Unsupported` cuando aplique; no inventar aliases.
5. Compilar primero el runtime mínimo de `std`; avanzar por filesystem/stdio, tiempo, threads/sync/TLS y red solo después de que cada prueba se enlace y ejecute. Recién entonces volver a compilar `horizon-core` original y avanzar por las dependencias de gameplay.

Este avance es investigación/diseño solamente. No generó un EBOOT del cliente original ni valida ejecución en RPCS3 o PS3 física.

## Referencias

[1]: https://github.com/ps3dev/ps3toolchain/blob/master/scripts/002-gcc-newlib-PPU.sh "Receta GCC/Newlib para PPU de PS3"
[2]: https://github.com/rust-lang/rust/blob/main/compiler/rustc_target/src/spec/targets/powerpc64_sony_ps3.rs "Target Rust powerpc64 Sony PS3"
[3]: https://github.com/rust-lang/libc/blob/0.2.190/src/unix/newlib/mod.rs "Bindings Newlib de libc 0.2.190"
[4]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/sys/file.h "API de archivos PSL1GHT"
[5]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/lv2/sysfs.h "Tipos y API sysFs de PSL1GHT"
[6]: https://github.com/ps3dev/ps3toolchain/blob/master/patches/newlib-1.20.0-PS3.patch "Parche PS3 de Newlib y libsysbase"
[7]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/crt/crt1.c "Inicialización CRT y callbacks PSL1GHT"
[8]: https://github.com/rust-lang/rust/blob/main/library/std/src/sys/fs/unix.rs "Backend Unix filesystem de Rust std"
[9]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/sys/socket.h "Syscalls de red LV2 de PSL1GHT"
[10]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/net/socket.h "Tipos y declaraciones de sockets PSL1GHT"
[11]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/sprx/libnet/socket.c "Wrappers de sockets de PSL1GHT"
[12]: https://github.com/rust-lang/rust/blob/main/library/std/src/sys/net/connection/socket/mod.rs "Backend común de sockets de Rust std"
[13]: https://git.rpcs3.net/rpcs3/PS3DK/src/branch/main/sdk/include/netdb.h "Header netdb de PS3DK"
[14]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/sys/thread.h "Threads PPU de PSL1GHT"
[15]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/sys/mutex.h "Mutexes PSL1GHT"
[16]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/sys/cond.h "Condvars PSL1GHT"
[17]: https://github.com/rust-lang/rust/blob/main/library/std/src/sys/pal/unix/sync/condvar.rs "Condvars del PAL Unix de Rust"
[18]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/sys/systime.h "Reloj y esperas de PSL1GHT"
[19]: https://doc.rust-lang.org/src/std/time.rs.html "Contratos de tiempo de Rust std"
[20]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/lv2/process.h "Interfaz de proceso LV2 PSL1GHT"
[21]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/lv2/syscalls.h "Constantes de syscalls LV2"
[22]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/librt/exit.c "Salida normal PSL1GHT"
[23]: https://github.com/rust-lang/rust/blob/main/library/std/src/sys/process/unix/unix.rs "Backend Unix de procesos de Rust std"
[24]: https://github.com/ps3dev/PSL1GHT/blob/master/ppu/include/sys/process.h "Funciones de proceso PSL1GHT"
[25]: https://github.com/rust-lang/rust/blob/main/library/std/src/sys/args/unix.rs "Selección de argumentos Unix de Rust std"
