
## Compatibilidad ABI comprobada

La comprobación con `powerpc64-ps3-elf-gcc` y headers PSL1GHT pasó para los
componentes básicos que necesita una interfaz Rust/C: ELF64 big-endian,
punteros de 64 bits, `size_t` de 64 bits, `sys_ppu_thread_t` de 64 bits y
`uint64_t` de 64 bits. Estos valores coinciden con el target Rust
`powerpc64-sony-ps3`.

La prueba también descubrió una excepción importante: `pthread_t` no es un
entero ni un puntero de 64 bits. PSL1GHT lo define como `pte_handle_t`, una
estructura de 16 bytes con un puntero y un contador de reutilización. La
implementación de `libc` para PS3 debe declarar exactamente esa estructura;
no puede copiar la definición de Linux ni asumir `pthread_t = u64`.

La conclusión es que la base C es compatible, pero la compatibilidad no es
automática. El port de `libc` debe modelar las estructuras PSL1GHT reales y
verificarlas con asserts de tamaño antes de conectar `std`.

## Hito de integración eficiente: libc PS3 vendorizado

Se añadió `third_party/libc` basado en la versión `0.2.190` fijada por el
workspace. El target experimental
`ports/ps3/powerpc64-sony-ps3-unix.json` conserva `os = "ps3"`, añade la
familia Unix y declara `env = "newlib"`, de acuerdo con PSL1GHT.

Las definiciones PS3 añadidas a newlib fueron verificadas contra headers del
SDK: `off_t`, `dev_t`, `clock_t`, `wchar_t`, `sockaddr` y `pthread_t`. En
particular, `pthread_t` usa la estructura PSL1GHT de 16 bytes.

Validación exitosa:

```text
cargo +nightly check -Z json-target-spec -Z build-std=core,alloc \
  --target ports/ps3/powerpc64-sony-ps3-unix.json
  libc 0.2.190 vendorizado: PASS
```

El intento con `horizon-core` usando solo `core,alloc` falla porque el crate
original usa el prelude de `std`; no se modificó el código para ocultar ese
hecho. El intento posterior con `build-std=std,core,alloc` todavía bloquea en
la integración de `libc` que usa el propio árbol de `std` y requiere conectar
el patch vendorizado con ese build-std. Este es el siguiente bloqueo real,
antes de tocar gameplay o inventar un frontend.

## Diagnóstico de `std` real para PS3

Se probó de forma reversible `build-std=std,core,alloc` conectando el
`libc` PS3 a la versión que usa `rust-src`. El crate `libc` pasó dentro del
build de `std`, confirmando que las definiciones PSL1GHT no eran el bloqueo.

El siguiente error aparece en `library/std/src/sys/process/unix/unix.rs`:
`std` selecciona el backend Unix genérico y requiere macros/símbolos POSIX de
señales y procesos (`WIFSIGNALED`, `WTERMSIG`, `SIGINT`, `SIGTERM`, etc.) que
PSL1GHT no proporciona con ese contrato. No se añadieron stubs ni aliases,
porque eso no sería una implementación real del port PS3.

Las modificaciones hechas a `rust-src` y al caché global de Cargo fueron
restauradas. El bloqueo actual requiere implementar un backend `std` específico
para PS3 o demostrar un backend compatible existente; no es un cambio de
Horizon gameplay ni se debe resolver reutilizando el backend `horizon` de otra
consola.

## Continuación reproducible del 2026-11-07

Se instaló Rust nightly `1.101.0-nightly (c980177d8 2026-11-06)` con `rust-src` y `rustfmt`, y se confirmó que el target oficial integrado es `powerpc64-sony-ps3` con `os = ps3`, PowerPC64 big-endian, ELFv1 y `std = false`.

Se añadió `ports/ps3/libc-check`, un crate `no_std` reproducible que fuerza la compilación de la copia vendorizada de `libc 0.2.190` con:

```text
cargo +nightly check -Z build-std=core,alloc \
  --manifest-path ports/ps3/libc-check/Cargo.toml \
  --target powerpc64-sony-ps3
```

La comprobación pasa. Para que el target oficial, que no declara `env = newlib`, seleccione correctamente los bindings Newlib PSL1GHT, `libc` ahora reconoce `target_os = ps3` tanto en la selección del módulo Unix como en los enlaces de `libc`/`libm`. No se añadieron aliases POSIX ni stubs.

También se añadió `abi_contract_probe.c` y el objetivo `make -C ports/ps3 abi-contract-probe`. Compila contra los headers reales PSL1GHT usando `-std=gnu11` —necesario porque `sys/thread.h` contiene `asm` GNU— y verifica por `_Static_assert` tamaños/offsets de punteros, handles PPU, `sockaddr`, `iovec`, `sysFSStat` y `addrinfo`. El objeto conserva referencias no resueltas a `sysFsStat`, `socket`, `gethostbyname` y `sysThreadCreate`, demostrando los contratos declarados/enlazables sin fingir que son wrappers POSIX.

La compilación aislada de `std` con `build-std=std,core,alloc` sigue fallando antes de `horizon-core`: el árbol upstream no selecciona implementación para `ps3` en sincronización, TLS, allocator, I/O y random. Aparecen, entre otros, las implementaciones `no_threads` en un target con threads, imports ausentes de `sys::io`, ausencia de backend de TLS y `fill_bytes`. El log completo es `ports/ps3/std-official-target-check.log`. Este es ahora el siguiente trabajo real: diseñar y probar esos módulos PAL específicos, empezando por allocator, I/O/errores y TLS, antes de volver a compilar el cliente original.

## Overlay de diagnóstico de `std`

Se añadió `apply-rust-src-libc-overlay.sh`, reversible y no permanente, porque
`build-std` resuelve la versión de `libc` desde el registro Cargo activo. El
overlay conecta temporalmente la implementación vendorizada PS3 con esa fuente,
ejecuta el diagnóstico y restaura el registro al terminar.

Con el overlay aplicado, `libc` deja de ser un error de `std`; la compilación
avanza hasta 14 errores concentrados en la selección PAL: allocator, backend de
errores I/O, sincronización, TLS y random. Esto confirma que el contrato `libc`
PS3 ya está conectado y que el siguiente trabajo no debe tocar gameplay. Los
logs son `std-official-overlay-check.log` y
`std-experimental-overlay-check.log`.
