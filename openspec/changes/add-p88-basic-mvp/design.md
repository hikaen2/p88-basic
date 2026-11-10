# Design

## Context

リポジトリは空 (README のみ)。対象は Raspberry Pi Zero W (BCM2835、ARM1176JZF-S、ARMv6 + VFPv2、512MB)。USB キーボードを動かすにはベアメタルの USB ホストスタックが必要で、自作はそれだけで大仕事になる。開発者は C++ を書きたくない。手元には rustc 1.98 (stable) があり、ターゲット一覧に `armv6-none-eabihf` (Tier 3) がある。`arm-none-eabi-gcc` は未導入。動機と範囲は proposal.md、振る舞いは specs/ を参照。

## Goals / Non-Goals

**Goals:**
- 開発の大半を PC 上 (`cargo test` とホスト版ウィンドウ) で完結させ、実機は最終確認に使う
- C++ はハードウェアの初期化と C ABI の公開だけをする薄いシム 1 ファイルに閉じ込める
- 将来の IME を、エディタに手を入れずに差し込める入力経路にする

**Non-Goals:**
- USB・SD・HDMI ドライバの自作
- PC-8801 の実機やバイナリとの互換 (中間コード形式、ROM、MBF 浮動小数点、Shift_JIS)
- 実行速度の最適化 (1GHz なので素朴なインタプリタで十分)

## Decisions

### 1. ハードウェア層は Circle に任せる
Circle が起動、HDMI フレームバッファ、USB HID キーボード、SD + FatFs、HDMI 音声を提供する。
- 代替: 自作 (USB が重すぎる)、USPi + 自作 (FB・SD は結局自作)。
- 結果として Circle の GPLv3 がプロジェクト全体に及ぶ (了承済み)。

### 2. 言語は Rust、構成は 3 crate + C++ シム
```
p88-basic/
  crates/p88-core/   no_std + alloc。BASIC、エディタ、VRAM、フォント、合成
  crates/p88-host/   std。minifb でウィンドウ、PC キー、ローカル FS
  crates/p88-pi/     staticlib。Platform を C ABI 経由で実装、GlobalAlloc、panic
  pi/shim/           Circle の CKernel (C++) + Makefile。Rust の .a をリンク
  assets/unifont/    unifont .hex (ビルド時に変換)
```
- D (betterC) も検討したが、druntime なしでは利点が薄く情報も少ない。
- core は `Platform` trait だけで外界と話す:
  ```
  trait Platform {
      fn poll_key(&mut self) -> Option<KeyEvent>;   // keycode + modifiers
      fn present(&mut self, fb: &[u32]);            // 640x400 RGB
      fn read_file / write_file / list_files / delete_file
      fn beep(&mut self);
      fn millis(&self) -> u64;
  }
  ```
- シムの C ABI は小さく保つ: `p88_present(ptr)`, `p88_poll_key(*mut ev) -> bool`, `p88_file_*`, `p88_beep()`, `p88_millis()`, `p88_malloc/free`。シムは初期化後に `rust_main()` を呼び、二度と戻らない。

### 3. Pi 用ビルド
- `rust-toolchain.toml` で nightly の日付を固定し、`-Zbuild-std=core,alloc`、`--target armv6-none-eabihf`、`-C target-cpu=arm1176jzf-s` で staticlib を作る。
- Circle は `RASPPI=1`、hard-float で同じ CPU 設定にビルドし、Makefile で `libp88_pi.a` をリンクする。
- 代替: 自前ターゲット JSON (公式ターゲットがあるので不要)。

### 4. 実行モデル: 協調的なメインループ
割り込み駆動の並行性は持たない。core の `Machine::step()` が「キーを 1 つ処理、または BASIC を N 文実行」を行い、一定間隔 (約 60Hz) で合成して `present` する。ESC / Ctrl+C の検出は文の実行の合間に `poll_key` を覗いて行う。
- 代替: タイマー割り込みで画面更新 (ロックが必要になり複雑)。

### 5. BASIC の内部表現
- 入力行はトークン化 (キーワードを 1 バイトコード、他はそのまま) して `BTreeMap<u16, Vec<u8>>` に保持。`LIST` はデトークン化して大文字表記を再生成する。
- 実行は再帰下降の式評価 + 文ごとのディスパッチ。`GOTO` 先は毎回の探索 (BTreeMap) で十分。
- 値: `enum Value { Int(i16), Sng(f32), Str(Rc<[u16]>) }`。文字列は UTF-16 単位 = BMP コードポイント単位で、`LEN`/`MID$` が O(1)。
- 代替: 文字列を UTF-8 (`String`) にする案は、`MID$` などでの位置計算が煩雑になるので不採用。

### 6. 画面: 2 つの VRAM と合成
- テキスト VRAM: `[[Cell; 80]; 25]`、`Cell { ch: u16, color: u8, wide_tail: bool }`。全角は左セルに文字、右セルに `wide_tail`。
- グラフィック VRAM: 640x400 の 1 バイト/ドット (パレット番号 0〜15)。テキスト画面と同じ寸法なので、ドットは正方形になり座標変換が不要になる。PC-8801 の 640x200 モードと `SCREEN` による切替は持たない。
- パレット: `[u16; 16]` (`0x0RGB`)。VRAM はパレット番号だけを持ち、合成時に RGB へ変換する。そのため `COLOR=` は再描画なしで画面全体に効く。PC-9801 の 16 色モード (4096 色中 16 色) にならうが、色指定の表記は直感的な `&HRGB` を採る (PC-98 の BASIC との数値互換は保証しない)。
- 合成: グラフィックをそのまま写し、その上にテキストの前景ドットを上書きし、パレットで RGB に変換して 640x400 の `u32` バッファにする。Pi では 640x400 のフレームバッファを確保し、ディスプレイへの拡大は GPU に任せる。画素の並び順 (RGB / BGR) は起動時にファームウェアへ問い合わせて合わせる。ホストではウィンドウ側の拡大に任せる。

### 7. フォント: Unifont を変換して埋め込む
`build.rs` で `unifont-*.hex` を読み、(a) 65536 エントリのオフセット表 (u32、なしは 0xFFFFFFFF) と (b) グリフビットマップ列 (8x16 は 16B、16x16 は 32B) を生成して `include_bytes!` する。幅はグリフのバイト長で決まる。合計約 2MB。
- 代替: 圧縮 (不要)、SD から読み込み (起動時の依存が増える)。

### 8. 入力経路と IME の差し込み口
```
KeyEvent(keycode, mods) --> InputFilter --> EditorInput { Char(u16) | Key(Edit) }
```
MVP の `InputFilter` は ASCII と編集キーへ写すだけの素通し。将来の IME はこの trait の別実装として入れる。
Circle のキーマップには JIS 配列がなく、Shift+HOME も区別できないため、Pi ではキーボードの生の報告 (修飾キー + キーコード 6 個) をシムのリングバッファ経由で受け取り、`p88-core` の `hid.rs` で JIS / US 配列の文字とキーリピートに変換する。配列は JIS が既定で、`cmdline.txt` の `keymap=US` で US にする。

### 9. ファイル
保存先は Pi では SD の `/P88/`、ホストではカレントの `./disk/`。形式は LIST と同じ UTF-8 テキスト。`LOAD` は CRLF と BOM を許容する。

## Risks / Trade-offs

- [Rust の staticlib と Circle のリンク不整合 (float ABI、`__aeabi_*`、`memcpy` の重複など)] → タスクの最初に「Rust から FB を塗る」スパイクで検証する。重複シンボルは Rust 側を `compiler_builtins` の mem 機能、または Circle 側に寄せて解決する。
- [Tier 3 ターゲットで nightly が壊れる] → `rust-toolchain.toml` で日付を固定する。
- [Unifont で kernel.img が約 2MB 増えて起動が遅くなる] → 実測して問題なら後で圧縮する (MVP では対策しない)。
- [USB キーボードの相性] → Circle の対応範囲に従う。動作確認したキーボードを README に記す。
- [Circle が QEMU `-M raspi0` で一部 (特に USB) 動かない] → QEMU は Pi 向けビルドの起動・リンク確認の補助に留め、最終確認は実機で行う。
- [ホスト版と実機で挙動がずれる] → 差異は Platform の実装に閉じ込め、core のテストはホストで回す。

## Migration Plan

新規プロジェクトのため移行なし。配布物は SD カードの FAT32 ブート領域に置く Pi のファームウェア (`bootcode.bin`, `start.elf`, `fixup.dat`)、`config.txt`、`kernel.img`。

## Open Questions

- `Shift+HOME` 以外の `CLR` キー割り当て (USB キーボードに `CLR` がないため)。実装中に決めても specs に影響しない。
- HDMI の出力解像度 (1280x800 で 2 倍か、ディスプレイ任せか)。表示倍率だけの問題なので後で決められる。
