#!/bin/sh
# 実機用 SD カードの中身を pi/sdcard/ に作る。中身を FAT32 の SD カードへコピーすれば起動する。
set -e
DIR=$(cd "$(dirname "$0")" && pwd)
"$DIR/build.sh" hw
OUT="$DIR/sdcard"
mkdir -p "$OUT"
if [ ! -f "$OUT/start.elf" ]; then
    make -C "$DIR/circle/boot" firmware >/dev/null
    cp "$DIR/circle/boot/bootcode.bin" "$DIR/circle/boot/start.elf" \
       "$DIR/circle/boot/fixup.dat" "$DIR/circle/boot/LICENCE.broadcom" "$OUT/"
fi
cp "$DIR/circle/boot/config32.txt" "$OUT/config.txt"
cp "$DIR/shim/kernel.img" "$OUT/kernel.img"
ls -l "$OUT"
