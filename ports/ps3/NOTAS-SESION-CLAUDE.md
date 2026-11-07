# Notas de la sesión de Claude (después de `Horizon-PS3-updated`)

> **Nota histórica del 5 de noviembre de 2026:** la frase “no compilado” describe
> el estado de esa sesión, no el estado actual. El 6 de noviembre se ejecutaron
> los host tests y la compilación del target Rust PS3. Ver
> [INFORME-AVANCE-2026-11-06.md](INFORME-AVANCE-2026-11-06.md), `README.md` y
> `RUST-ABI.md` para el estado vigente y sus límites.

En la sesión original, Claude no tenía Rust ni PS3DEV en ese entorno. La
recomendación de verificación que sigue se conserva como registro histórico:

    make -C ports/ps3 host-test

Si algo falla, el error será de compilación o de una aserción de prueba; los
módulos son pequeños y autocontenidos, así que se corrigen aislados.

## Módulos nuevos (crate `horizon-ps3-platform`)

- `terrain.rs`: terreno determinista con ruido de valor solo con enteros.
  `fill_chunk` rellena la cuadrícula 18³ (con halo) desde coordenadas de mundo,
  así que el halo de cada chunk coincide con el interior de su vecino.
  Bloques: 0 aire, 1 piedra, 2 tierra, 3 césped, 4 bedrock (ids provisionales).
- `world.rs`: `World<CAPACITY, QUEUE>` conecta terreno → `ChunkStore` →
  `MeshQueue` → `mesh_opaque_greedy`.
  - `stream_around(cx, cz, radio, max_new)`: carga lo más cercano primero,
    expulsa a distancia > radio+1, y limita cuánto genera por llamada.
  - `pump(&mut FrameWorkBudget, scratch, sink)`: malla chunks sucios dentro del
    presupuesto del frame y entrega cada malla al `sink` (aquí irá el upload RSX).
- `chunk_store.rs`: añadidos `fill_with`, `contains`, `dirty_count`, `retain`
  (+1 prueba).

## Segunda tanda: cliente con cámara libre (también sin compilar)

- `trig.rs`: seno/coseno propios (`core` no trae trigonometría; error < 1e-5).
- `player.rs`: `FlyCamera` con el mando. Stick izq. mueve, stick der. mira,
  R2/L2 suben/bajan, R1 corre. Genera la `Camera` del renderer.
- `mesh_cache.rs`: guarda los quads greedy de cada chunk para no re-mallar
  cada frame.
- `client.rs` (solo con `psl1ght-ffi`): `horizon_ps3_rust_world_entry`. Bucle
  por frame: pad → cámara → `stream_around` → `pump` → proyección en CPU →
  un solo upload → draw. Salir: Start+Select. Los buffers grandes (~7 MB) son un
  `static` entregado una vez, no viven en la pila.
- `rsx_mesh.rs`: colores por bloque (`block_color`); el id 1 conserva su color.
- Puente C: `horizon_ps3_begin_scene` (viewport, scissor, clear de color y
  profundidad, depth test) y `horizon_ps3_present_scene` (flip sin la copia
  de 3,6 MB por CPU). Son opcionales: el smoke test original no cambia.
- Makefile: `rust-world-link` y `rust-world-package` (EBOOT aparte, appid
  HZNPS3002). `rust_world_probe.c` es su `main`.

Riesgos conocidos de esta tanda (por orden de probabilidad):
1. Las llamadas RSX de `begin_scene` (`rsxSetViewport`, `rsxClearSurface`,
   `rsxSetDepth*`) las escribí de memoria de PSL1GHT; si no compilan, ajustar
   firmas o constantes.
2. Convención de profundidad: la cámara proyecta z a [-1, 1] y el viewport lo
   mapea a [0, 1] (escala y offset 0,5). Si todo se ve invertido o desaparece,
   revisar eso primero.
3. Sin reloj: `FRAME_DT` asume 60 Hz; a 30 fps irá a mitad de velocidad.
4. La proyección es en CPU y la malla se reconstruye cada frame; sirve para
   verificar, no será la versión final (faltan matrices/uniformes en GPU).

Comprobar primero, en este orden:

    make -C ports/ps3 host-test      # pruebas del crate (nuevas incluidas)
    make -C ports/ps3 ffi-check      # compila client.rs y psl1ght.rs

## Lo que sigue (en orden)

1. Ejecutar `host-test` y `ffi-check` y corregir lo que salga.
2. Probar `rust-world-package` en la consola (o emulador) y mirar el TTY.
3. Mover la proyección a la GPU (matriz como uniforme) y subir mallas por chunk.
4. Colisión con el terreno, romper/colocar bloques (`bump_revision` ya existe).
5. Texturas/atlas (requiere recursos que tengas derecho a usar).
