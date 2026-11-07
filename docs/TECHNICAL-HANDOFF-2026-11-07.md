# Continuación técnica — 7 de noviembre de 2026

Este documento registra el estado descrito en el paquete de continuación aportado al proyecto. Los resultados listados son resultados documentados en ese paquete; no equivalen a una nueva reproducción de las pruebas en esta sesión ni a una validación en una PS3 física.

## Objetivo confirmado

Continuar el port del cliente original de Horizon a PS3 mediante PSL1GHT y el target oficial Rust `powerpc64-sony-ps3`. No sustituir el cliente original por una demo y no simular APIs del sistema con stubs.

## Avances que informa el paquete

- Target oficial `powerpc64-sony-ps3` identificado; `core` y `alloc` se compilan en las pruebas indicadas.
- Copia vendorizada de `libc 0.2.190` conectada al trabajo PS3.
- Sonda C de ABI compilada contra headers PSL1GHT.
- Contratos de ABI documentados para filesystem, sockets, DNS y threads.
- Existe un crate `horizon-ps3-platform` con pruebas host y módulos para mundo, jugador, cámara, chunks, malla y puente PSL1GHT.
- Existe código de bring-up C/RSX, shaders y flujo de empaquetado HEN. Estos artefactos no deben confundirse con el cliente original integrado.

## Validaciones reportadas en el handoff

- `cargo +stable fmt --all -- --check`: PASS.
- `cargo +stable test -p horizon-ps3-platform --no-default-features --features host-stub`: 89 tests PASS.
- `make -C ports/ps3 abi-contract-probe`: PASS con PPU GCC.
- `make -C ports/ps3 libc-official-check`: PASS con `core,alloc` y target oficial.

Las pruebas ABI distinguen declaraciones y contratos enlazables de ejecución real. No demuestran por sí solas que esas funciones se hayan ejecutado correctamente en consola.

## Bloqueo actual

El cliente original depende de `std`, pero la compilación de `std` para PS3 aún no dispone de una selección PAL completa. El build aislado de `std` llega a módulos faltantes de allocator, errores I/O, sincronización, TLS y random. El handoff indica que el overlay diagnóstico de libc debe restaurarse después de cada prueba.

No añadir aliases POSIX falsos, stubs de procesos ni reutilizar el backend de otra consola para ocultar incompatibilidades.

## Próximas acciones

1. Fijar commits y versiones de Rust nightly, `rust-src`, libc, GCC/Newlib, ps3toolchain y PSL1GHT.
2. Reconstruir la toolchain en un entorno de desarrollo y repetir las pruebas documentadas.
3. Implementar el PAL/sys PS3 en cambios pequeños y revisables, comenzando por allocator y errores I/O; después sincronización/TLS y random.
4. Para cada API, registrar por separado: declarada, enlazada y ejecutada.
5. No volver a intentar el cliente original hasta que el build de `std` y los contratos ABI mínimos estén implementados.
6. Después integrar por capas: `horizon-core`, mundo, cliente-core, renderer, input y entrypoint original.
7. No declarar un EBOOT funcional ni compatibilidad real hasta ejecutar el artefacto del cliente original en RPCS3 o PS3 física y conservar logs.

## Límite de validación

El paquete de continuación describe resultados de una sesión anterior. En esta actualización se incorporan sus conclusiones a la documentación; no se ha compilado el proyecto ni se ha probado hardware nuevo.
