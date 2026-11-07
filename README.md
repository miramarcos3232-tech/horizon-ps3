# Horizon PS3

Investigación experimental para evaluar la adaptación de componentes de [Horizon](https://github.com/horizon-rust/horizon) a una PS3 Slim con HEN.

> **Estado:** planificación e investigación. Todavía no existe un port funcional, compilación para PS3 ni demo gráfica.

## Objetivo
Determinar si es viable reutilizar partes de Horizon y construir una aplicación homebrew para PS3. Primero se buscarán pruebas pequeñas y reproducibles. No se promete paridad con Minecraft Java o Bedrock moderno.

## Documentación
- [Estado actual](docs/STATUS.md)
- [Hoja de ruta](docs/ROADMAP.md)
- [Arquitectura propuesta](docs/ARCHITECTURE.md)
- [Viabilidad y compatibilidad](docs/FEASIBILITY.md)

## Principios
- No declarar funciones terminadas sin pruebas.
- Registrar versiones, comandos y logs.
- Aislar las modificaciones específicas de PS3.
- Usar código y recursos con licencias compatibles.
- No incluir assets ni código propietario de Minecraft.

## Requisitos
La documentación puede prepararse desde Android. Compilar y probar requerirá un entorno de desarrollo compatible, RPCS3 y/o una PS3 real con HEN.

## Licencia
Pendiente de revisar la licencia de Horizon y sus dependencias antes de publicar código derivado.
