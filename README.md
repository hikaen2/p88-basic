# p88-basic

Raspberry Pi Zero W でベアメタル起動する、N88-BASIC 風の BASIC マシン。
HDMI 画面と USB キーボードで動く単体の 8 ビット PC 風マシンを目指す。

## ライセンス

GPLv3 (`LICENSE`)。ハードウェア層に [Circle](https://github.com/rsta2/circle) (GPLv3) を使うため。

## 必要なもの

- Raspberry Pi Zero W、HDMI ディスプレイ、USB キーボード (OTG 変換)、microSD
- ビルド環境 (Ubuntu で確認):
  - Rust (rustup)。nightly は `rust-toolchain.toml` で固定してあり、初回の `cargo` 実行時に入る
  - `arm-none-eabi-gcc`、QEMU (`qemu-system-arm`)、`mkfs.vfat`

```
sudo apt install -y gcc-arm-none-eabi qemu-system-arm dosfstools
git submodule update --init
```

## ビルド

### ホスト版 (PC 上で開発・テスト)

```
cargo build
cargo test
```

### Circle

Circle は `pi/circle` に submodule として置いてある (タグ `Step51.1.1`)。
ビルド設定は実機用と QEMU 用で異なる。

```
cd pi/circle
./configure -r 1 --kernel-max-size 8 -f           # 実機 (Pi Zero W) 用
./configure -r 1 --kernel-max-size 8 --qemu -f    # QEMU 用
./makeall clean && ./makeall -j8                  # 設定を変えたら clean から
```

動作確認には Circle のサンプルを使える:

```
cd sample/08-usbkeyboard && make -j8              # kernel.img ができる
```

### p88 カーネル

```
pi/build.sh qemu      # QEMU 用 (pi/shim/kernel.img)
pi/build.sh hw        # 実機用
pi/make-sd.sh         # 実機用をビルドし、SD カードの中身を pi/sdcard/ に作る
```

`pi/build.sh` はモードが変わったときだけ Circle を設定し直して再ビルドする。

## 実機で起動

FAT32 でフォーマットした microSD に `pi/sdcard/` の中身をすべてコピーし、Pi Zero W に挿して電源を入れる。
ログは UART (GPIO14 = TX、115200bps) に出る。

## QEMU で起動

`pi/run-qemu.sh` が `pi/shim/kernel.img` を起動する (SD イメージ `pi/sd.img` は初回に自動作成)。
手で起動する場合は次のとおり。

QEMU 用の設定でビルドした `kernel.img` を、32 ビットでは `-kernel` ではなく `-bios` で渡す。

```
truncate -s 64M sd.img && mkfs.vfat -F 32 sd.img  # SD カードイメージ (初回のみ)
qemu-system-arm -M raspi0 -bios kernel.img \
    -drive file=sd.img,if=sd,format=raw \
    -usb -device usb-kbd -serial stdio
```

QEMU での確認状況 (Circle Step51.1.1、QEMU 10.2.1):

| 機能 | 状態 |
|---|---|
| HDMI 画面 (640x480) | 動く |
| USB キーボード (`-device usb-kbd`) | 動く |
| SD カード | イメージを付けて起動できる (読み書きは未確認) |
| HDMI 音声 | 未確認。最終確認は実機で行う |

## 既知の制約

- Circle のキーマップは DE / ES / FR / IT / UK / US のみで、JIS 配列はない。
