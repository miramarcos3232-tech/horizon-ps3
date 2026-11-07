#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
WORK_DIR="${PS3DEV_SOURCE_DIR:-$ROOT_DIR/.ps3dev-source}"
INSTALL_DIR="${PS3DEV:-$ROOT_DIR/.ps3dev}"

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"

if [[ "$INSTALL_DIR" != /* ]]; then
    echo "error: PS3DEV must be an absolute path" >&2
    exit 1
fi

if [[ ! -d "$WORK_DIR/.git" ]]; then
    git clone --depth 1 https://github.com/ps3dev/ps3dev.git "$WORK_DIR"
else
    git -C "$WORK_DIR" fetch --depth 1 origin main
    git -C "$WORK_DIR" reset --hard FETCH_HEAD
fi

cat <<EOF
PS3DEV source is ready at:
  $WORK_DIR

Detected host: $HOST_OS $HOST_ARCH

The current ps3toolchain prebuilt releases publish Linux X64 and macOS
X64/ARM64 archives, but no Linux ARM64 archive. On Linux ARM64, use an
x86_64 build machine or an x86_64 emulator/container for the prebuilt
toolchain, or set LOCAL_TOOLCHAIN_BUILD=1 and build the compiler locally.

The complete build is intentionally not started automatically because it
compiles a PPU/SPU GCC toolchain and can take a long time. To build it:

  export PS3DEV=$INSTALL_DIR
  export PSL1GHT=\$PS3DEV
  cd $WORK_DIR
  ./prepare.sh
  ./build-all.sh

Afterward verify:

  export PATH=\$PS3DEV/bin:\$PS3DEV/ppu/bin:\$PS3DEV/spu/bin:\$PATH
  ppu-gcc --version
EOF
