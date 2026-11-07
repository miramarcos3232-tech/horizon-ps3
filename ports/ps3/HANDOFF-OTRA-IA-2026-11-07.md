# Handoff: port original de Minecraft Horizon a PlayStation 3

Fecha de corte: 2026-11-07.

## Objetivo

Continuar el port del cliente original de Minecraft Horizon a PS3 real usando
PSL1GHT y el target oficial `powerpc64-sony-ps3`. No crear un juego de prueba,
no reemplazar el gameplay original por una demo y no inventar APIs ni stubs.

## Estado confirmado

El repositorio completo está en la raíz de este paquete. El port existente de
PS3 conserva sus pruebas de host, el ABI bridge PSL1GHT, shaders de bring-up y
el flujo de empaquetado HEN. Esos artefactos de bring-up no son el cliente
original y están marcados como tales en `ports/ps3/Makefile` y la documentación.

Se instaló localmente PS3DEV/PSL1GHT durante la sesión. El SDK compilado se
excluye del ZIP por tamaño; se puede reconstruir con:

```sh
cd ports/ps3
./bootstrap-ps3dev.sh
```

El source parcial de bootstrap queda incluido en `ports/ps3/.ps3dev-source`.

## Cambios reales del port original

1. `ports/ps3/powerpc64-sony-ps3-unix.json` es una especificación experimental
   separada. Mantiene `os = "ps3"`, PowerPC64 big-endian, ELFv1, punteros de
   64 bits, familia Unix y `env = "newlib"`.
2. `third_party/libc` es una copia vendorizada de `libc 0.2.190`, la versión
   fijada por el `Cargo.lock` del workspace. Está conectada mediante
   `[patch.crates-io]` en el `Cargo.toml` raíz.
3. `third_party/libc/src/unix/newlib/mod.rs` contiene únicamente definiciones
   PSL1GHT verificadas contra los headers del SDK: `off_t`, `dev_t`,
   `clock_t`, `wchar_t`, `sockaddr` y `pthread_t` como estructura PSL1GHT de
   16 bytes. No copiar `pthread_t` de Linux.
4. `third_party/libmimalloc-sys/build.rs` tiene el ajuste real necesario para
   que el build script no rechace el target PS3 sin familia estándar declarada.

## Validaciones que pasaron

El crate `libc 0.2.190` vendorizado compila para:

```text
powerpc64-sony-ps3-unix
-Z build-std=core,alloc
```

La ABI básica también se comprobó con `powerpc64-ps3-elf-gcc` y headers
PSL1GHT. Ver `ports/ps3/VALIDACION-SESION-2026-11-06.md`.

## Bloqueo actual verdadero

`horizon-core` original usa `std`. Con `build-std=core,alloc` falla por el
prelude de `std`; no se debe convertir el crate a `no_std` para ocultarlo.

Se probó `build-std=std,core,alloc` conectando temporalmente el libc PS3 a la
copia que usa `rust-src`. En esa prueba `libc` sí compiló con las definiciones
PSL1GHT. El siguiente fallo aparece en el backend Unix genérico de `std`, que
exige macros y símbolos POSIX de procesos/señales:

```text
WIFSIGNALED, WTERMSIG, WCOREDUMP, WIFSTOPPED,
SIGINT, SIGTERM, SIGCHLD, SIGCONT, ...
```

PSL1GHT no proporciona ese contrato con esos nombres. No añadir aliases,
stubs ni reutilizar el backend `horizon` de otra consola: eso no sería un
port original real de PS3.

Las modificaciones temporales a `rust-src` y al caché global fueron
restauradas. Los logs de las pruebas se conservan en `ports/ps3/`.

## Continuación del trabajo — 2026-11-07

Se completó el estudio inicial de la selección del PAL de Rust y la comparación
de siete áreas con fuentes upstream de Rust, Newlib, PSL1GHT y PS3DK. El diseño,
las referencias y la secuencia de verificación están en
`ports/ps3/DISENO-BACKEND-STD-PS3.md`.

Hallazgo central: activar `cfg(unix)` selecciona contratos Unix genéricos, pero
no demuestra que GameOS/PSL1GHT los implemente. Hay primitivas reales de PS3
para filesystem, sockets, threads/sync, tiempo y salida, pero quedan diferencias
de firma, layout, callbacks o semántica. No se deben presentar como aliases
POSIX ni cubrir las brechas con stubs. También se identificaron bloqueos para
resolver DNS, `pthread`/TLS, procesos/señales y varios servicios de `std`.

Este entorno de continuación no trae el SDK PS3 compilado ni herramientas
`rustup`/`rustc`/`cargo`; por eso el avance fue de investigación y diseño, no de
implementación o compilación. El ZIP contiene fuentes de bootstrap en
`ports/ps3/.ps3dev-source` para reconstruir PS3DEV/PSL1GHT.

## Próximo trabajo recomendado

1. Fijar versiones/commits de Rust, `libc`, ps3toolchain, GCC/binutils/Newlib y
   PSL1GHT; conservar target JSON, flags y link map.
2. Reconstruir/instalar la toolchain PS3 y ejecutar pruebas C pequeñas para
   tamaños/alineaciones, símbolos enlazables y comportamiento real. Marcar cada
   API por separado como declarada, enlazada o ejecutada.
3. Fijar una copia reproducible de `rust-src` y diseñar un PAL/sys específico
   para PS3. Empezar por selección de argumentos/runtime y filesystem/stdio;
   continuar con tiempo, threads/sync/TLS y sockets/resolver. Dejar `process`
   y señales para el final.
4. No volver a compilar `horizon-core` original con `build-std=std,core,alloc`
   hasta que la selección de `std`, libc y los contratos ABI mínimos estén
   implementados y probados.
5. Después avanzar por `horizon-world`, `horizon-client-core`, renderer, input
   y entrypoint original. `hidapi`/DualSense es opcional; no sustituirlo por
   una implementación inventada.
6. No declarar el cliente funcional ni hardware-validado hasta disponer de un
   EBOOT del cliente original probado en PS3/RPCS3.

## Comandos de diagnóstico

Desde la raíz:

```sh
export PATH=/usr/local/cargo/bin:$HOME/.cargo/bin:$PATH
cargo +stable tree -p libc --target x86_64-unknown-linux-gnu
cargo +nightly check -Z json-target-spec -Z build-std=core,alloc \
  --manifest-path /tmp/ps3-libc-check/Cargo.toml \
  --target ports/ps3/powerpc64-sony-ps3-unix.json --lib
```

Los resultados históricos y las decisiones están en:

- `ports/ps3/VALIDACION-SESION-2026-11-06.md`
- `ports/ps3/DISENO-BACKEND-STD-PS3.md`
- `ports/ps3/ESTRATEGIA-EFICIENTE-PORT-ORIGINAL.md`
- `ports/ps3/PLAN-FINAL.md`
- `ports/ps3/TOOLCHAIN.md`

## Exclusiones del ZIP

Se excluyen `.git`, `target/`, `ports/ps3/build/`,
`ports/ps3/.ps3dev/` y cachés de compilación. Son reproducibles o demasiado
grandes; el código, documentación, target JSON, libc vendorizado y logs sí se
incluyen.
