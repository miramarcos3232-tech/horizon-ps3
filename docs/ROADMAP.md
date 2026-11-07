# Hoja de ruta — Horizon PS3

## Objetivo
Investigar si es viable adaptar componentes reutilizables de Horizon a una PS3 Slim con HEN. La primera meta es una prueba mínima que compile, arranque y dibuje una escena; no se promete paridad con Minecraft Java o Bedrock moderno.

## Fase 0 — Viabilidad
- [ ] Auditar el código upstream, licencia, dependencias y versión exacta.
- [ ] Identificar renderer, APIs gráficas, shaders y requisitos nativos.
- [ ] Verificar target Rust, linker, SDK y empaquetado homebrew.
- [ ] Documentar bloqueos y decidir si el enfoque es viable.

## Fase 1 — Toolchain
- [ ] Preparar entorno reproducible.
- [ ] Compilar un programa mínimo para PS3.
- [ ] Arrancarlo en RPCS3.
- [ ] Validarlo en PS3 real con HEN y guardar logs.

## Fase 2 — Gráficos
- [ ] Investigar acceso a RSX en el entorno homebrew elegido.
- [ ] Presentar un frame y limpiar pantalla.
- [ ] Dibujar triángulo, geometría con buffers y textura.
- [ ] Medir memoria, sincronización y tiempo por frame.

## Fase 3 — Adaptación
- [ ] Separar lógica de plataforma y renderizado.
- [ ] Reemplazar dependencias incompatibles.
- [ ] Crear una capa de plataforma PS3.
- [ ] Integrar un módulo pequeño de Horizon y probarlo.

## Fase 4 — Prototipo
- [ ] Cargar un mundo de prueba pequeño.
- [ ] Mostrar un conjunto limitado de bloques/chunks.
- [ ] Añadir cámara y controles.
- [ ] Evaluar guardado, audio y optimización.

## Criterios de éxito
Cada hito debe tener instrucciones reproducibles, versiones, logs y pruebas. No declarar compatibilidad sin ejecución comprobada. Usar únicamente código y recursos con licencias compatibles; no incluir assets propietarios de Minecraft.

## Riesgos
Cell/PowerPC, RSX, memoria limitada, dependencias nativas, toolchain Rust incompleto y APIs gráficas no disponibles pueden impedir o limitar el port.
