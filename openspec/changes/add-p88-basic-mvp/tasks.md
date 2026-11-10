# Tasks

## 1. ツールチェーンと骨組み

- [x] 1.1 `arm-none-eabi-gcc` を導入し、`arm-none-eabi-gcc --version` が通ることを確認する
- [x] 1.2 Cargo workspace (`p88-core`, `p88-host`, `p88-pi`) と日付固定の `rust-toolchain.toml` (nightly、`rust-src`) を作り、ホストで `cargo build` が通ることを確認する
- [x] 1.3 Circle を submodule として追加し、`RASPPI=1` でビルドして Circle 付属のサンプルが `kernel.img` になることを確認する
- [x] 1.4 `qemu-system-arm` を導入し、1.3 の Circle サンプルを `-M raspi0` (`-kernel`、SD イメージ、`-device usb-kbd`) で起動して画面表示と USB キーボード入力が通るかを確認する。通らない部分は README に記し、その部分の確認は実機のみとする
- [x] 1.5 README にライセンス (GPLv3)、必要ハード、ビルド手順、QEMU での起動手順を書き、手順どおりに 1.2〜1.4 が再現できることを確認する

## 2. 実機スパイク: Rust から Circle 経由で HDMI を塗る

- [x] 2.1 `pi/shim` に最小の CKernel を書き、FB を初期化して `rust_main()` を呼ぶ。`p88_present` と `p88_millis` を C ABI で公開する
- [x] 2.2 `p88-pi` を `armv6-none-eabihf` + build-std の staticlib にし、GlobalAlloc (`p88_malloc`/`p88_free`) と panic handler を実装する
- [ ] 2.3 Rust から 640x400 バッファに 16 色の縦帯を描いて present し、QEMU (1.4 で動いた場合) と実機の HDMI に縦帯が表示されることを確認する (リンク・float ABI の検証)

## 3. フォントとテキスト画面 (core)

- [x] 3.1 `build.rs` で Unifont `.hex` からオフセット表とグリフ列を生成し、`'A'` が幅 8、`'あ'` が幅 16 と返るテストを通す
- [x] 3.2 テキスト VRAM (80x25、色、`wide_tail`)、文字出力、行末折り返し、全角の行末送り、スクロールを実装し、spec の text-screen のシナリオ (全角 2 セル、スクロール) をユニットテストで確認する
- [x] 3.3 グラフィック VRAM (640x400) と、テキスト前景を重ねる 640x400 合成を実装し、合成結果の特定ピクセルの色をテストで確認する
- [x] 3.4 `p88-host` に minifb ウィンドウを作り、合成バッファを表示して、`漢字` と 16 色の文字がウィンドウに出ることを目視で確認する

## 4. スクリーンエディタ

- [x] 4.1 `KeyEvent` → `InputFilter` (素通し) → `EditorInput` の経路を実装し、ホストの PC キーを `KeyEvent` に変換する
- [x] 4.2 カーソル表示 (点滅)、文字入力、矢印、BS、DEL、INS、HOME、画面消去を実装し、screen-editor の BS・挿入モードのシナリオをテストで確認する
- [x] 4.3 RET で論理行 (折り返しを含む) を取り出す処理と、全角の右セルにカーソルを置かない移動を実装し、該当シナリオをテストで確認する
- [x] 4.4 起動画面 (タイトル、`Bytes free`、`Ok`) を実装し、ホスト版を起動してそれが表示されることを確認する

## 5. BASIC インタプリタ

- [x] 5.1 トークナイザとデトークナイザ、行の登録・置換・削除、`LIST` (範囲指定)、`NEW` を実装し、basic-language の行編集・大文字化のシナリオをテストで確認する
- [x] 5.2 値 (`Int`/`Sng`/`Str`)、変数 (2 文字識別、型接尾辞)、`DIM` 配列、式評価 (優先順位、比較、論理、`&H`) を実装し、優先順位・型・配列のシナリオをテストで確認する
- [x] 5.3 `PRINT` (`;` `,` 数値の書式)、ダイレクトモードと `Ok`、エラー報告 (` in 行番号`) を実装し、該当シナリオをテストで確認する
- [x] 5.4 `GOTO`、`GOSUB`/`RETURN`、`IF THEN ELSE`、`FOR`/`NEXT`、`END`、`STOP`、`REM`/`'`、`RUN`、`CONT` を実装し、制御構文のシナリオをテストで確認する
- [x] 5.5 文字列関数 (コードポイント単位) と数値関数を実装し、`LEN("あいう")`、`CHR$`/`ASC`、`INT(-2.5)` のシナリオをテストで確認する
- [x] 5.6 `INPUT`、`LINE INPUT`、`INKEY$`、`READ`/`DATA`/`RESTORE` を実装し、該当シナリオをテストで確認する
- [x] 5.7 メインループ (`Machine::step`) と ESC/Ctrl+C による `Break in` を実装し、ホスト版で `10 GOTO 10` を RUN して ESC で止まり、`CONT` で再開できることを確認する

## 6. 画面制御とグラフィック命令

- [x] 6.1 `CLS [n]`、`LOCATE`、`POS`、`CSRLIN`、`COLOR`、`WIDTH` を実装し、text-screen のシナリオをテストで確認する
- [x] 6.2 16 色パレット (既定値)、`COLOR=(p,c)`、`COLOR=NEW` を実装し、graphics のパレットのシナリオ (描画済みの色が変わる) をテストで確認する
- [x] 6.3 `PSET`/`PRESET`、`LINE` (`B`/`BF`、始点省略、クリッピング)、`POINT` を実装し、graphics のシナリオをテストで確認する
- [x] 6.4 `CIRCLE` と `PAINT` (境界色、スタックの溢れない塗り) を実装し、円の内側だけが塗られるシナリオをテストで確認する
- [x] 6.5 `BEEP` を Platform 経由で実装 (ホストは何もしないか端末ベル) し、エラーにならないことをテストで確認する

## 7. プログラムの保存

- [x] 7.1 Platform のファイル操作と `SAVE`、`LOAD [,R]`、`FILES`、`KILL`、ファイルエラーを実装し、ホストの `./disk/` を使った program-storage のシナリオ (UTF-8・CRLF・BOM を含む) をテストで確認する
- [x] 7.2 `samples/` にデモプログラム (日本語表示、円と塗りつぶし、INKEY$ で動くキャラクタ) を置き、ホスト版で `LOAD` して `RUN` し、期待どおり動くことを確認する

## 8. 実機統合

- [ ] 8.1 シムに USB キーボード (`p88_poll_key`) を追加し、実機で打鍵した文字と矢印・BS・ESC がエディタに反映されることを確認する
- [ ] 8.2 シムに SD + FatFs のファイル操作 (保存先 `/P88/`) を追加し、実機で `SAVE` したファイルを PC で開けること、PC で書いた `.BAS` を実機で `LOAD` できることを確認する
- [ ] 8.3 シムに HDMI 音声による `p88_beep` を追加し、実機で `BEEP` が鳴ることを確認する
- [ ] 8.4 SD カードの作り方 (ファームウェア、`config.txt`、`kernel.img`) を README に書き、その手順で作った SD で起動から `samples/` のデモ実行まで通ることを確認する
