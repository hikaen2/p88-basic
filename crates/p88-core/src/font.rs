//! 内蔵 GNU Unifont (BMP)。グリフは 8x16 (半角) か 16x16 (全角)。

static INDEX: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/font_index.bin"));
static DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/font_data.bin"));

/// グリフがない文字の代わりに表示する文字 (□)
pub const REPLACEMENT: u16 = 0x25A1;

pub struct Glyph {
    /// 16 行分。半角は 1 行 1 バイト、全角は 1 行 2 バイト (左が上位)
    pub bits: &'static [u8],
    pub wide: bool,
}

impl Glyph {
    /// 行 y・列 x (0..幅) のドットが立っているか
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        let (byte, bit) = if self.wide { (self.bits[y * 2 + x / 8], x % 8) } else { (self.bits[y], x) };
        byte & (0x80 >> bit) != 0
    }
}

fn lookup(ch: u16) -> Option<Glyph> {
    let i = ch as usize * 4;
    let v = u32::from_le_bytes([INDEX[i], INDEX[i + 1], INDEX[i + 2], INDEX[i + 3]]);
    if v == u32::MAX {
        return None;
    }
    let wide = v & 0x8000_0000 != 0;
    let off = (v & 0x7FFF_FFFF) as usize;
    let len = if wide { 32 } else { 16 };
    Some(Glyph { bits: &DATA[off..off + len], wide })
}

/// 表示に使うグリフ。サロゲートやグリフのない文字は □ になる。
pub fn glyph(ch: u16) -> Glyph {
    let ch = if (0xD800..=0xDFFF).contains(&ch) { REPLACEMENT } else { ch };
    lookup(ch).or_else(|| lookup(REPLACEMENT)).unwrap()
}

/// 文字が占めるセル数 (1 か 2)。□ で代替する文字は 1 セルとする。
pub fn width(ch: u16) -> usize {
    match lookup(ch) {
        Some(g) if g.wide && !(0xD800..=0xDFFF).contains(&ch) => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths() {
        assert_eq!(width('A' as u16), 1);
        assert_eq!(width('あ' as u16), 2);
        assert_eq!(width('漢' as u16), 2);
    }

    #[test]
    fn glyph_a() {
        let g = glyph('A' as u16);
        assert!(!g.wide);
        assert_eq!(g.bits[5], 0x24); // 0041:0000000018242442...
        assert!(g.pixel(2, 5) && g.pixel(5, 5) && !g.pixel(0, 5));
    }

    #[test]
    fn missing_and_surrogate_use_replacement() {
        assert_eq!(glyph(0xD800).bits, glyph(REPLACEMENT).bits);
        assert_eq!(width(0xD800), 1);
    }
}
