#!/bin/bash
# build the three stubs anathema-xboot installs into the kernel:
#   xcp.bin   commpage map stubs (tools/commpage_stub.c): .text.a at 0, .text.b at 0x100
#   xcp2.bin  commpage enter + fixed locator hooks (tools/commpage_enter.c): .anxcave at 0, .anxcave2 at 0x40
#   xcp3.bin  starts tmpfs and the fs compression kexts (tools/loadkexts.c): .anxcave at 0
# the size limits are the ones the loader checks. works with apple clang or linux clang
# (CC=...); needs python3 for tools/xcplink.py.
# usage: tools/mkxcp.sh [OUTDIR]   (default crates/anathema-setup/assets)
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=${1:-$HERE/../crates/anathema-setup/assets}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
build() { # source, output, linker options
  local src=$1 out=$2
  shift 2
  ${CC:-clang} -target x86_64-unknown-linux-gnu -Os -ffreestanding -fpie -fno-stack-protector \
    -mno-red-zone -fno-asynchronous-unwind-tables -fno-unwind-tables -c "$HERE/$src" -o "$TMP/$out.o"
  python3 -I "$HERE/xcplink.py" "$TMP/$out.o" "$OUT/$out" "$@"
}
build commpage_stub.c xcp.bin --at .text.a=0 --at .text.b=0x100 --max 0x400
build commpage_enter.c xcp2.bin --at .anxcave=0 --at .anxcave2=0x40 --max 0x3c0
build loadkexts.c xcp3.bin --at .anxcave=0 --max 0x700
