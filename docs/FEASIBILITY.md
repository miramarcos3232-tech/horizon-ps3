# Viabilidad y compatibilidad

## Pregunta central
¿Se puede compilar y ejecutar una parte útil de Horizon en PS3 Slim con HEN, y cuánto trabajo requieren las dependencias de plataforma y renderizado? La respuesta aún no está demostrada.

## Condicionantes
- CPU Cell Broadband Engine con núcleo principal PowerPC y unidades SPU.
- GPU RSX.
- Memoria y rendimiento muy limitados frente a equipos modernos.
- El entorno HEN ejecuta homebrew compatible, pero no elimina restricciones del hardware ni sustituye un SDK.
- No asumir que binarios o bibliotecas de PC funcionan directamente.

## Auditoría upstream
Referencia: https://github.com/horizon-rust/horizon

Antes de portar:
1. Registrar commit/tag exacto y licencia.
2. Leer versión de Rust y edition.
3. Enumerar crates y dependencias nativas.
4. Localizar punto de entrada y módulos principales.
5. Identificar renderer, API gráfica y shaders.
6. Revisar threads, atomics, SIMD, sockets y filesystem.
7. Clasificar módulos portables y no portables.

## Matriz inicial

| Área | Riesgo | Verificación |
|---|---|---|
| Rust/toolchain | Alto | target, LLVM, linker, runtime |
| Dependencias nativas | Alto | APIs C/C++ y sistema |
| Gráficos | Muy alto | ruta RSX homebrew |
| Memoria | Muy alto | huella y asignaciones |
| CPU/hilos | Alto | concurrencia y rendimiento |
| Entrada/archivos | Medio | APIs disponibles |
| Audio/red | Alto | APIs y latencia |

## Experimentos
- P0: programa Rust mínimo y registro de target, versiones y comandos.
- P1: homebrew mínimo en RPCS3 y luego PS3 real.
- P2: pantalla sólida y triángulo con RSX.
- P3: compilar una biblioteca pequeña del upstream sin renderer.

## Referencias
- Horizon: https://github.com/horizon-rust/horizon
- Rust platform support: https://doc.rust-lang.org/beta/rustc/platform-support.html
- RPCS3: https://rpcs3.net/
- PSL1GHT: https://github.com/ps3dev/PSL1GHT

Son puntos de partida, no pruebas de que exista un toolchain Rust completo para PS3.

## Registro de experimentos
Para cada prueba registrar fecha, hardware/emulador y versión, HEN/firmware, herramientas, commit, comando, resultado, logs y siguiente paso.
