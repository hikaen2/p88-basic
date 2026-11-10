//! GNU Unifont (.hex.gz) から BMP のグリフ表を作る。
//! - font_index.bin: 65536 x u32 (LE)。グリフ列内のバイト位置。0xFFFF_FFFF はグリフなし。
//!   最上位ビットが 1 なら全角 (16x16 = 32 バイト)、0 なら半角 (8x16 = 16 バイト)。
//! - font_data.bin : グリフのビットマップ列。1 行 1 バイト (半角) / 2 バイト (全角)。
use std::io::{BufRead, BufReader};
use std::{env, fs::File, path::PathBuf};

const HEX: &str = "../../assets/unifont/unifont_all-18.0.01.hex.gz";
const NONE: u32 = 0xFFFF_FFFF;
const WIDE: u32 = 0x8000_0000;

fn main() {
    println!("cargo::rerun-if-changed={HEX}");
    let gz = flate2::read::GzDecoder::new(File::open(HEX).expect("unifont hex"));
    let mut index = vec![NONE; 0x10000];
    let mut data = Vec::new();
    for line in BufReader::new(gz).lines() {
        let line = line.unwrap();
        let Some((cp, bits)) = line.split_once(':') else { continue };
        if cp.len() != 4 {
            continue; // BMP 外
        }
        let cp = usize::from_str_radix(cp, 16).unwrap();
        let wide = match bits.len() {
            32 => 0,
            64 => WIDE,
            _ => continue,
        };
        index[cp] = data.len() as u32 | wide;
        for i in (0..bits.len()).step_by(2) {
            data.push(u8::from_str_radix(&bits[i..i + 2], 16).unwrap());
        }
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let idx: Vec<u8> = index.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(out.join("font_index.bin"), idx).unwrap();
    std::fs::write(out.join("font_data.bin"), data).unwrap();
}
