#!/usr/bin/env bash
set -Eeuo pipefail

# This intentionally touches only the active Cargo registry libc source tree. The
# backup is required so the overlay can be removed after diagnostics.
ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
TARGET_LIBC="$(find "$CARGO_HOME/registry/src" -maxdepth 2 -type d -name 'libc-0.2.191' -print -quit)"
SOURCE_LIBC="$ROOT_DIR/third_party/libc/src"
BACKUP_DIR="${RUST_LIBC_BACKUP:-$ROOT_DIR/ports/ps3/.rust-src-libc-backup}"

if [[ ! -d "$SOURCE_LIBC" ]]; then
  echo "error: vendored libc source not found: $SOURCE_LIBC" >&2
  exit 1
fi
if [[ ! -d "$TARGET_LIBC" ]]; then
  echo "error: Cargo registry libc 0.2.191 not found: $TARGET_LIBC" >&2
  exit 1
fi

case "${1:-apply}" in
  apply)
    if [[ -e "$BACKUP_DIR" ]]; then
      echo "error: backup already exists; run restore first: $BACKUP_DIR" >&2
      exit 1
    fi
    mkdir -p "$(dirname "$BACKUP_DIR")"
    cp -a "$TARGET_LIBC" "$BACKUP_DIR"
    rm -rf "$TARGET_LIBC"
    cp -a "$SOURCE_LIBC" "$TARGET_LIBC"
    echo "Applied libc PS3 overlay to $TARGET_LIBC"
    ;;
  restore)
    if [[ ! -e "$BACKUP_DIR" ]]; then
      echo "error: no overlay backup found: $BACKUP_DIR" >&2
      exit 1
    fi
    rm -rf "$TARGET_LIBC"
    mv "$BACKUP_DIR" "$TARGET_LIBC"
    echo "Restored original rust-src libc at $TARGET_LIBC"
    ;;
  *)
    echo "usage: $0 [apply|restore]" >&2
    exit 2
    ;;
esac
