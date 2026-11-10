# Proposal

## Why

Raspberry Pi Zero W を、電源を入れると N88-BASIC 風の BASIC が起動する単体の 8 ビット PC 風マシンにして遊びたい。リポジトリは空なので、まず「起動して、打ち込んで、絵を描いて、保存できる」最小のマシン (MVP) を作る。

## What Changes

- Pi Zero W 上でベアメタル起動し、HDMI 画面に `Ok` プロンプトを出す BASIC マシンを新規作成する
- USB キーボードによる N88-BASIC 風スクリーンエディタ (画面上の行にカーソルを合わせて RET で入力)
- テキスト画面 80x25 (8x16 セル) とグラフィック画面 640x400 16 色の重ね合わせ表示 (4096 色中 16 色のパレット)
- GNU Unifont を内蔵し Unicode BMP の文字を表示 (全角は 2 セル)
- N88-BASIC 風の命令サブセット: 行番号編集、制御構文、文字列/数値関数、`CLS` `LOCATE` `COLOR` `COLOR=` `WIDTH` `INKEY$`、`PSET` `LINE` `CIRCLE` `PAINT`、`BEEP`
- SD カードへの `SAVE` / `LOAD` / `FILES` (UTF-8 テキスト形式)
- 開発用に PC 上で動くホスト版 (ウィンドウ表示) も用意する

対象外 (将来): 日本語入力 (ローマ字かな、かな漢字変換 IME)、倍精度 `#`、`PLAY`/`SOUND`、`GET@`/`PUT@`、`VIEW`/`WINDOW`、ファイル I/O (`OPEN` 等)、`ON ERROR`、BMP 外の文字・結合文字・RTL。

## Capabilities

### New Capabilities

- `basic-language`: BASIC の言語仕様。プログラム行の編集・実行、文、式、型 (`%` `!` `$`)、コードポイント単位の文字列、関数、エラー報告
- `screen-editor`: 起動時のプロンプトとキーボードによる全画面エディタの振る舞い、`STOP` による中断
- `text-screen`: 80x25 テキスト画面、Unifont による BMP 文字表示、全角 2 セル、`CLS` `LOCATE` `COLOR` `WIDTH` の画面効果
- `graphics`: 640x400 16 色のグラフィック画面とテキスト画面の重ね合わせ、16 色パレット (`COLOR=`)、`PSET` `LINE` `CIRCLE` `PAINT`
- `program-storage`: `SAVE` / `LOAD` / `FILES` によるプログラムの保存と読み込み

### Modified Capabilities

(なし)

## Impact

- 新規コード一式 (Rust の BASIC コア、Pi 用ターゲット、ホスト版、Circle と接続する薄い C++ シム)
- 外部依存: Circle (GPLv3、これによりプロジェクトは GPLv3)、GNU Unifont (GPLv2+ 埋め込み例外 / OFL 1.1)、Rust nightly (`armv6-none-eabihf` + build-std)、`arm-none-eabi-gcc`、ホスト版のウィンドウ用 crate
- 必要ハード: Raspberry Pi Zero W、HDMI ディスプレイ、USB キーボード (OTG 変換)、microSD
