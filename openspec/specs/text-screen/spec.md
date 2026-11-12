# text-screen Specification

## Purpose

80x25 のテキスト画面に、内蔵した GNU Unifont で Unicode BMP の文字を表示し、`CLS`・`LOCATE`・`COLOR`・`WIDTH` で制御する振る舞いを定める。

## Requirements

### Requirement: テキスト画面の寸法
システムは 640x400 ドットの画面に、1 セル 8x16 ドットで横 80 桁 × 縦 25 行のテキスト画面を表示しなければならない (SHALL)。HDMI 出力ではこの画面を縦横同じ整数倍に拡大し、余白は黒とする。

#### Scenario: 80 桁表示
- **WHEN** `PRINT STRING$(80,"*")` を実行する
- **THEN** `*` が 1 行に 80 個並び、折り返さない

### Requirement: Unicode BMP の文字表示
システムは U+0000〜U+FFFF のうち GNU Unifont にグリフがある文字をそのグリフで表示しなければならない (SHALL)。グリフのない文字とサロゲート領域は代替グリフ (□ 相当) で 1 セルに表示する。

#### Scenario: 漢字の表示
- **WHEN** `PRINT CHR$(&H6F22);CHR$(&H5B57)` を実行する
- **THEN** `漢字` が Unifont のグリフで表示される

### Requirement: 全角文字は 2 セル
システムは Unifont で幅 16 ドットのグリフを持つ文字を 2 セル、幅 8 ドットの文字を 1 セルで表示しなければならない (SHALL)。全角文字が行末の 1 セルに収まらない場合は次の行の先頭に表示する。

#### Scenario: 全角と半角の混在
- **WHEN** `PRINT "Aあ";:PRINT POS(0)` を実行する
- **THEN** `Aあ` が 3 セルを占め、続けて ` 3` が表示される

### Requirement: CLS
システムは `CLS` でテキスト画面を消去してカーソルを左上へ移さなければならない (SHALL)。`CLS 1` はテキストのみ、`CLS 2` はグラフィックのみ、`CLS 3` は両方を消去する。引数なしは `CLS 1` と同じとする。

#### Scenario: テキスト消去
- **WHEN** 文字が表示された状態で `CLS` を実行する
- **THEN** テキスト画面が空になり、`Ok` が 1 行目に表示される

### Requirement: LOCATE と POS/CSRLIN
システムは `LOCATE x,y` でカーソルを x 桁 y 行 (0 起点) へ移し、`POS(0)` と `CSRLIN` で現在の桁と行を返さなければならない (SHALL)。範囲外の値は `Illegal function call` とする。

#### Scenario: 指定位置に表示
- **WHEN** `CLS:LOCATE 10,5:PRINT "X"` を実行する
- **THEN** `X` が左から 11 桁目、上から 6 行目に表示される

### Requirement: 文字色
システムは `COLOR n` (n=0〜15) で以後に表示する文字の色をパレット番号 n に設定しなければならない (SHALL)。色はグラフィックと共通の 16 色パレットで決まる (graphics の「16 色パレット」を参照)。起動時は 7 (白) とし、範囲外は `Illegal function call` とする。

#### Scenario: 赤い文字
- **WHEN** `COLOR 2:PRINT "R":COLOR 7` を実行する
- **THEN** `R` が赤で表示され、以後の文字は白に戻る

### Requirement: WIDTH
システムは `WIDTH 80,25` と `WIDTH 80,20` を受け付けてテキスト画面の行数を切り替えなければならない (SHALL)。それ以外の値は `Illegal function call` とする。

#### Scenario: 20 行表示
- **WHEN** `WIDTH 80,20` を実行する
- **THEN** テキスト画面が 20 行になり、スクロールは 20 行目で起こる

### Requirement: 画面スクロール
システムはカーソルが最下行を越えて進むとき、テキスト画面全体を 1 行上へスクロールしなければならない (SHALL)。グラフィック画面はスクロールしない。

#### Scenario: 26 行目の出力
- **WHEN** `FOR I=1 TO 30:PRINT I:NEXT` を実行する
- **THEN** 最下行付近に ` 30` と `Ok` が表示され、` 1` は画面外へ消えている
