#!/bin/sh
# p88 カーネルをビルドする。 使い方: pi/build.sh [hw|qemu]  (既定: hw)
# モードが前回と違うときだけ Circle を設定し直して全体を再ビルドする。
set -e
MODE=${1:-hw}
DIR=$(cd "$(dirname "$0")" && pwd)
OPTS="-r 1 --kernel-max-size 8"
[ "$MODE" = qemu ] && OPTS="$OPTS --qemu"
cd "$DIR/circle"
if [ "$(cat .p88-mode 2>/dev/null)" != "$MODE" ]; then
    ./configure $OPTS -f >/dev/null
    ./makeall clean >/dev/null
    ./makeall -j"$(nproc)" >/dev/null
    make -C "$DIR/shim" clean >/dev/null
    echo "$MODE" > .p88-mode
fi
make -C "$DIR/shim" -j"$(nproc)"
