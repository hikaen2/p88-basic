# program-storage Specification

## Purpose

BASIC プログラムを記憶媒体 (実機では microSD カード) へ UTF-8 テキストとして保存・読み込み・一覧表示する振る舞いを定める。

## Requirements

### Requirement: SAVE
システムは `SAVE "名前"` で現在のプログラムを、`LIST` と同じ表記の UTF-8 テキスト (改行 LF) として、保存先ディレクトリの `名前.BAS` に書き込まなければならない (SHALL)。名前に拡張子が含まれる場合はそれを使い、同名ファイルは上書きする。

#### Scenario: 保存
- **WHEN** `10 PRINT "あ"` を入力して `SAVE "TEST"` を実行する
- **THEN** 保存先に `TEST.BAS` ができ、その内容は UTF-8 の `10 PRINT "あ"` と改行である

### Requirement: LOAD
システムは `LOAD "名前"` で現在のプログラムと変数を消去し、ファイルの各行を行番号付き行として読み込まなければならない (SHALL)。`LOAD "名前",R` は読み込み後に実行する。拡張子の省略時は `.BAS` を補う。

#### Scenario: PC で書いたファイルを読み込む
- **WHEN** PC で `10 PRINT "こんにちは"` と書いた `HELLO.BAS` (UTF-8、CRLF) を保存先に置き、`LOAD "HELLO"` の後に `RUN` する
- **THEN** `こんにちは` が表示される

#### Scenario: 行番号のない行
- **WHEN** 行番号のない行を含むファイルを `LOAD` する
- **THEN** その行は読み飛ばされず `Direct statement in file` が表示され、読み込みは中止される

### Requirement: FILES
システムは `FILES` で保存先のファイル名とサイズを一覧表示しなければならない (SHALL)。

#### Scenario: 一覧
- **WHEN** `SAVE "A"` と `SAVE "B"` の後に `FILES` を実行する
- **THEN** `A.BAS` と `B.BAS` が表示される

### Requirement: KILL
システムは `KILL "ファイル名"` で指定したファイルを削除しなければならない (SHALL)。拡張子は省略できない。

#### Scenario: 削除
- **WHEN** `KILL "A.BAS"` を実行して `FILES` する
- **THEN** `A.BAS` は表示されない

### Requirement: ファイルエラー
システムは存在しないファイルの `LOAD`/`KILL` で `File not found` を、記憶媒体が使えない場合は `Disk I/O error` を表示しなければならない (SHALL)。

#### Scenario: 存在しないファイル
- **WHEN** `LOAD "NOFILE"` を実行する
- **THEN** `File not found` が表示され、現在のプログラムは変化しない
