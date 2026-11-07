# Plan de implementación del PAL std para PS3

## Propósito

Desbloquear la compilación del cliente original de Horizon implementando las piezas de plataforma que Rust `std` necesita para PS3. Este plan sigue el diagnóstico del paquete de continuación del 2026-11-07.

## Regla de seguridad técnica

No declarar una función POSIX disponible solo porque exista una función PSL1GHT con nombre parecido. Verificar firma, ABI, ownership, callbacks, errores y semántica. No crear stubs que devuelvan éxito ficticio.

## Preparación reproducible

Antes de editar Rust:
- Fijar el commit de Rust nightly y la versión exacta de `rust-src`.
- Fijar versión/hash de PSL1GHT, ps3toolchain, GCC, binutils y Newlib.
- Conservar target JSON, flags, linker command y link map.
- Mantener el overlay de diagnóstico reversible y restaurarlo incluso si el build falla.
- Guardar cada log con fecha, comando y código de salida.

## Orden de trabajo

### PAL-0 — Selección del target
- Revisar cómo Rust selecciona `library/std/src/sys/pal`.
- Definir explícitamente qué cfgs activa PS3.
- Evitar que `cfg(unix)` implique capacidades POSIX no implementadas.
- Resultado: un mapa de módulos y capacidades que compila hasta el siguiente bloqueo real.

### PAL-1 — Allocator
- Inspeccionar las expectativas de `std` para asignación y liberación.
- Mapearlas a una primitiva real disponible en el runtime PS3.
- Definir alineación, tamaños, errores, concurrencia y política de agotamiento.
- Probar asignación, realloc, liberación, alineación y agotamiento.
- No conectar el allocator global del juego hasta completar las pruebas.

### PAL-2 — Errores I/O y filesystem/stdio
- Definir el tipo de error y conversiones desde errores reales de PSL1GHT/Newlib.
- Verificar apertura, lectura, escritura, seek, metadata y cierre.
- Probar rutas y dispositivos disponibles en homebrew.
- Distinguir API declarada, símbolo enlazado y operación ejecutada.

### PAL-3 — Tiempo y sincronización
- Mapear reloj monotónico y esperas a primitivas verificadas.
- Implementar mutex/condvar solo después de verificar sus contratos.
- Probar concurrencia, timeout, wakeups y shutdown.
- No asumir que `pthread_t` es un entero: el handoff documenta que PSL1GHT usa una estructura de 16 bytes.

### PAL-4 — TLS y random
- Identificar las necesidades exactas de TLS del runtime.
- Verificar si el runtime y toolchain ofrecen almacenamiento local por hilo.
- Investigar una fuente real de aleatoriedad y documentar su calidad/limitaciones.
- No devolver bytes constantes ni usar stubs.

### PAL-5 — Red y capacidades no soportadas
- Investigar sockets y resolución DNS como áreas separadas.
- Dejar procesos y señales para el final; no emular semánticas que PS3 no ofrece.
- Cuando una capacidad no exista, usar una representación explícita de no soportado si la arquitectura de Rust lo permite.

## Matriz de verificación

Para cada función requerida, registrar:

| Campo | Descripción |
|---|---|
| API Rust | Contrato requerido por `std` |
| API PS3 | Función PSL1GHT/Newlib candidata |
| ABI | Tipos, tamaños, alineación y convención |
| Declarada | Header/prototipo disponible |
| Enlazada | Símbolo resuelto en el link |
| Ejecutada | Prueba real en emulador/hardware |
| Diferencias | Semántica, errores y limitaciones |

## Criterio de salida

El PAL no se considera listo por superar un `cargo check` aislado. Debe compilar `std`, enlazar una aplicación mínima, ejecutar pruebas de allocator/I/O/sync en un entorno PS3 y documentar capacidades ausentes. Recién entonces se retoma la integración del cliente Horizon original.
