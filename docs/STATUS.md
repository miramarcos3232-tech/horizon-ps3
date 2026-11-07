# Estado del proyecto

**Estado:** port experimental en investigación e integración de plataforma.  
**Cliente original funcional:** no demostrado.  
**Compilación completa de std para PS3:** bloqueada.  
**EBOOT del cliente original probado:** no.  
**PS3 física / RPCS3 validada en esta etapa:** no.

## Avances documentados en el paquete de continuación

- Target Rust oficial `powerpc64-sony-ps3` usado para comprobaciones de `core` y `alloc`.
- `libc 0.2.190` vendorizado y comprobado de forma aislada con `core,alloc`.
- Sonda ABI C contra headers PSL1GHT.
- Crate `horizon-ps3-platform` con pruebas host; el handoff informa 89 tests PASS.
- Bring-up C/RSX, shaders y flujo de empaquetado HEN existentes como pruebas de plataforma, no como cliente original.

Estos resultados provienen del paquete de continuación aportado. Deben reproducirse en un entorno de desarrollo antes de considerarlos verificados en esta sesión.

## Bloqueo actual

El cliente Horizon original usa `std`. La selección PAL para PS3 aún carece de implementaciones necesarias de allocator, errores I/O, sincronización, TLS y random. El siguiente hito es completar ese PAL con contratos reales de PSL1GHT/Newlib, sin aliases POSIX ficticios ni stubs que oculten fallos.

## Próximo hito

Seguir [el plan de PAL std para PS3](STD-PAL-PLAN.md), fijar versiones de toolchain y reproducir las pruebas. Luego retomar la integración de `horizon-core` y los crates del cliente.

## Documentación
- [Hoja de ruta](ROADMAP.md)
- [Arquitectura propuesta](ARCHITECTURE.md)
- [Viabilidad](FEASIBILITY.md)
- [Continuación técnica 2026-11-07](TECHNICAL-HANDOFF-2026-11-07.md)
- [Plan de PAL std](STD-PAL-PLAN.md)
