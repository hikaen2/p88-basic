# Spec Delta

## Purpose

N88-BASIC 風の BASIC 言語として、行番号付きプログラムの入力・編集・実行と、式・型・文字列・関数の振る舞いを定める。

## ADDED Requirements

### Requirement: 行番号付き行の登録と削除
システムは、行番号で始まる入力行をプログラムに行番号順で登録しなければならない (SHALL)。同じ行番号は置き換え、行番号のみの入力はその行を削除しなければならない (SHALL)。行番号は 0〜65529 とする。

#### Scenario: 行の追加と置換
- **WHEN** `20 PRINT 2`、`10 PRINT 1`、`20 PRINT 3` の順に入力して `LIST` する
- **THEN** `10 PRINT 1` と `20 PRINT 3` がこの順で表示される

#### Scenario: 行の削除
- **WHEN** `10 PRINT 1` を登録した後 `10` のみを入力して `LIST` する
- **THEN** 何も表示されない

### Requirement: ダイレクトモード
システムは、行番号で始まらない入力行を直ちに実行し、実行後に `Ok` を表示しなければならない (SHALL)。

#### Scenario: 直接実行
- **WHEN** `PRINT 1+2` を入力する
- **THEN** ` 3` が表示され、続いて `Ok` が表示される

### Requirement: プログラム管理コマンド
システムは `LIST`、`RUN`、`NEW`、`CONT` を提供しなければならない (SHALL)。`LIST` は行範囲指定 (`LIST 10-50`) を受け付け、`RUN` は全変数を初期化して先頭行から実行する。

#### Scenario: NEW でプログラム消去
- **WHEN** プログラムがある状態で `NEW` を実行して `LIST` する
- **THEN** 何も表示されない

#### Scenario: 範囲 LIST
- **WHEN** 行 10, 20, 30 がある状態で `LIST 20-30` を実行する
- **THEN** 行 20 と 30 のみが表示される

### Requirement: キーワードの大文字小文字
システムはキーワードと変数名の大文字小文字を区別せず、`LIST` ではキーワードと変数名を大文字で表示しなければならない (SHALL)。文字列リテラルとコメントの中身は変更しない。

#### Scenario: 小文字入力
- **WHEN** `10 print "abc";a` を入力して `LIST` する
- **THEN** `10 PRINT "abc";A` と表示される

### Requirement: 制御構文
システムは `GOTO`、`GOSUB`/`RETURN`、`IF ... THEN ... ELSE`、`FOR ... TO ... STEP`/`NEXT`、`END`、`STOP`、`REM` (と `'`)、`:` による複文を提供しなければならない (SHALL)。`THEN`/`ELSE` の後の行番号は `GOTO` とみなす。

#### Scenario: FOR ループ
- **WHEN** `10 FOR I=1 TO 3:PRINT I;:NEXT` を `RUN` する
- **THEN** ` 1  2  3 ` が表示される

#### Scenario: IF ELSE
- **WHEN** `10 A=5:IF A>3 THEN PRINT "Y" ELSE PRINT "N"` を `RUN` する
- **THEN** `Y` が表示される

#### Scenario: GOSUB
- **WHEN** `10 GOSUB 100:END` と `100 PRINT "S":RETURN` を `RUN` する
- **THEN** `S` が表示されて終了する

### Requirement: 変数と型
システムは整数型 (`%`、16 ビット符号付き)、単精度型 (`!` または接尾辞なし、IEEE 754 単精度)、文字列型 (`$`) の変数と、`DIM` による多次元配列を提供しなければならない (SHALL)。変数名は先頭 2 文字以上を識別し、未代入の変数は 0 または空文字列とする。`DIM` なしの配列は各次元の添字 0〜10 とする。

#### Scenario: 整数への代入
- **WHEN** `A%=7.6:PRINT A%` を実行する
- **THEN** ` 8` が表示される

#### Scenario: 配列
- **WHEN** `DIM B(20):B(15)=3:PRINT B(15)` を実行する
- **THEN** ` 3` が表示される

### Requirement: 式と演算子
システムは算術演算子 (`+ - * / \ ^ MOD`)、比較演算子 (`= <> < > <= >=`)、論理演算子 (`NOT AND OR XOR`)、文字列連結 (`+`) を、N88-BASIC と同じ優先順位で評価しなければならない (SHALL)。比較の真は -1、偽は 0 とする。`&H` で始まる 16 進定数を受け付ける。

#### Scenario: 優先順位
- **WHEN** `PRINT 2+3*4^2` を実行する
- **THEN** ` 50` が表示される

#### Scenario: 比較結果
- **WHEN** `PRINT (1<2)` を実行する
- **THEN** `-1` が表示される

### Requirement: コードポイント単位の文字列
システムは文字列を Unicode BMP のコードポイント列として扱い、`LEN`、`LEFT$`、`RIGHT$`、`MID$`、`INSTR`、`CHR$`、`ASC`、`STR$`、`VAL`、`STRING$`、`SPACE$`、`HEX$` をコードポイント単位で提供しなければならない (SHALL)。

#### Scenario: 全角文字の長さ
- **WHEN** `PRINT LEN("あいう")` を実行する
- **THEN** ` 3` が表示される

#### Scenario: CHR$ と ASC
- **WHEN** `PRINT CHR$(&H3042);ASC("あ")` を実行する
- **THEN** `あ 12354` が表示される

### Requirement: 数値関数
システムは `ABS`、`INT`、`FIX`、`SGN`、`SQR`、`SIN`、`COS`、`TAN`、`ATN`、`EXP`、`LOG`、`RND` を提供しなければならない (SHALL)。

#### Scenario: INT は切り捨て
- **WHEN** `PRINT INT(-2.5)` を実行する
- **THEN** `-3` が表示される

### Requirement: PRINT と INPUT
システムは `PRINT` (`;` で連結、`,` で 14 桁ごとのタブ位置、末尾 `;` で改行抑止、数値は正なら先頭空白と末尾空白) と、`INPUT` (プロンプト文字列付き、`?` 表示)、`LINE INPUT` を提供しなければならない (SHALL)。

#### Scenario: INPUT
- **WHEN** `10 INPUT "N";N:PRINT N*2` を `RUN` し、`21` と RET を入力する
- **THEN** `N? ` が表示され、入力後に ` 42` が表示される

### Requirement: READ と DATA
システムは `READ`、`DATA`、`RESTORE` を提供しなければならない (SHALL)。

#### Scenario: DATA の読み込み
- **WHEN** `10 READ A,B$:PRINT A;B$` と `20 DATA 5,XY` を `RUN` する
- **THEN** ` 5 XY` が表示される

### Requirement: エラー報告
システムはエラー発生時に実行を止め、N88-BASIC 風の英語メッセージ (例: `Syntax error`、`Undefined line number`、`Division by zero`、`Type mismatch`、`Out of memory`) を、プログラム実行中なら ` in <行番号>` を付けて表示しなければならない (SHALL)。

#### Scenario: 実行中の構文エラー
- **WHEN** `10 PRINT (` を `RUN` する
- **THEN** `Syntax error in 10` が表示され、`Ok` に戻る

#### Scenario: ゼロ除算
- **WHEN** `PRINT 1/0` を実行する
- **THEN** `Division by zero` が表示される

### Requirement: BEEP
システムは `BEEP` で短いビープ音 (約 0.1 秒) を鳴らさなければならない (SHALL)。音声出力のない環境では何もせずエラーにもしない。実機では HDMI の音声として出力する。

#### Scenario: ビープ
- **WHEN** HDMI 音声が出るディスプレイで `BEEP` を実行する
- **THEN** 短い音が鳴り、`Ok` が表示される
