# Arquitectura propuesta

Este documento es una hipótesis de diseño, no una implementación existente. Se ajustará después de auditar el código upstream.

## Separación de responsabilidades

```text
          Aplicación adaptada
                  |
      +-----------+-----------+
      |           |           |
    Núcleo     Plataforma   Recursos
   mundo/juego     PS3      propios
      |           |
   Render API   Entrada / reloj /
      |         archivos / memoria
   Backend RSX
      |
   GPU de PS3
```

## Núcleo
Investigar qué partes de Horizon implementan simulación, entidades, chunks, generación de geometría y serialización. No asumir que sus límites actuales son portables sin leer el código.

## Capa de plataforma
Encapsular inicialización, cierre, reloj, mando, filesystem, memoria, logs, hilos y sincronización. Implementar solo las funciones que el runtime homebrew realmente ofrezca.

## Renderizado
La PS3 utiliza RSX. No asumir soporte de Vulkan, OpenGL moderno ni APIs de escritorio. Orden de pruebas:
1. Inicializar dispositivo y presentar frame.
2. Limpiar pantalla.
3. Dibujar triángulo.
4. Crear buffers de geometría.
5. Cargar textura original de prueba.
6. Probar cámara y geometría de bloques.

## Memoria y rendimiento
Medir antes de optimizar. Limitar chunks y distancia de visión, evitar copias innecesarias, liberar recursos gráficos explícitamente y registrar picos de memoria y tiempos por frame.

## Estructura objetivo
- `docs/`: decisiones, pruebas y guías.
- `platform/ps3/`: integración de plataforma.
- `renderer/rsx/`: backend gráfico si se confirma viable.
- `tests/ps3/`: pruebas pequeñas.
- `tools/`: scripts reproducibles de build y empaquetado.

No crear módulos vacíos ni mover código upstream hasta conocer su licencia y estructura.
