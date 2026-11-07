# Handoff para la siguiente IA — port Horizon a PS3

## Workspace

Usar este directorio como raíz del proyecto. El target oficial es
`powerpc64-sony-ps3`. El SDK instalado está en `/home/ubuntu/ps3dev` y Rust
nightly incluye `rust-src`.

## Estado comprobado

- `cargo +stable fmt --all -- --check`: PASS.
- `cargo +stable test -p horizon-ps3-platform --no-default-features --features host-stub`: **89 tests PASS**.
- `make -C ports/ps3 abi-contract-probe` con `ppu-gcc`: PASS.
- `make -C ports/ps3 libc-official-check`: PASS con `core,alloc` y target oficial.
- La sonda ABI confirma `sysFsStat`, `socket`, `gethostbyname` y `sysThreadCreate` como contratos reales del SDK.

## Bloqueo actual

El cliente original todavía no compila porque Rust `std` no tiene selección PAL
para PS3. La compilación aislada de `std` con `build-std=std,core,alloc` avanza
hasta módulos faltantes de allocator, errores I/O, sincronización, TLS y random.
No añadir aliases POSIX, `pthread` falsos ni stubs de procesos.

## Archivos añadidos o cambiados en esta continuación

- `third_party/libc/src/lib.rs`
- `third_party/libc/src/unix/mod.rs`
- `ports/ps3/libc-check/`
- `ports/ps3/abi_contract_probe.c`
- `ports/ps3/apply-rust-src-libc-overlay.sh`
- `ports/ps3/Makefile`
- `ports/ps3/VALIDACION-SESION-2026-11-06.md`
- `ports/ps3/DISENO-BACKEND-STD-PS3.md`
- `ports/ps3/PLAN-FINAL.md`

## Comandos de validación

```bash
source "$HOME/.cargo/env"
export PS3DEV=/home/ubuntu/ps3dev
export PSL1GHT=/home/ubuntu/ps3dev
export PATH="$PS3DEV/bin:$PS3DEV/ppu/bin:$HOME/.cargo/bin:$PATH"

cargo +stable fmt --all -- --check
cargo +stable test -p horizon-ps3-platform --no-default-features --features host-stub
make -C ports/ps3 abi-contract-probe PPU_CC="$PS3DEV/ppu/bin/ppu-gcc" PSL1GHT="$PSL1GHT" PS3DEV="$PS3DEV"
make -C ports/ps3 libc-official-check
```

El overlay de libc es diagnóstico y debe restaurarse tras una prueba:

```bash
ports/ps3/apply-rust-src-libc-overlay.sh apply
# ejecutar build-std=std,core,alloc
ports/ps3/apply-rust-src-libc-overlay.sh restore
```

El siguiente hito recomendado es crear un overlay PAL std específico para PS3,
comenzando por `sys/alloc`, `sys/io/error` y `sys/sync`, y conservar todos los
patches contra la versión exacta de `rust-src`. No volver todavía a gameplay.
