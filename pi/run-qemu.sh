#!/bin/sh
# QEMU で P88 を起動する。Circle は ./configure --qemu でビルドしておくこと。
# 使い方: pi/run-qemu.sh [追加の qemu オプション...]
set -e
DIR=$(cd "$(dirname "$0")" && pwd)
SD="$DIR/sd.img"
[ -f "$SD" ] || { truncate -s 64M "$SD" && mkfs.vfat -F 32 "$SD" >/dev/null; }
exec qemu-system-arm -M raspi0 -bios "$DIR/shim/kernel.img" \
    -drive file="$SD",if=sd,format=raw \
    -usb -device usb-kbd -serial stdio "$@"
