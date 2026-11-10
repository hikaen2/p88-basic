# GNU Unifont

`unifont_all-18.0.01.hex.gz` は https://ftp.gnu.org/gnu/unifont/unifont-18.0.01/ から取得した GNU Unifont 18.0.01。
ライセンスは GPLv2 以降 (フォント埋め込みの例外付き) と SIL OFL 1.1 のデュアルライセンス。
ビルド時に `crates/p88-core/build.rs` が BMP (U+0000〜U+FFFF) だけを取り出して埋め込む。
