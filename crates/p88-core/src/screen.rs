//! 16 色パレットと、テキスト + グラフィックの合成

use crate::font;
use crate::gfx::{Graphics, HEIGHT, WIDTH};
use crate::text::{COLS, TextScreen};

/// 既定パレット (&HRGB)。8 は灰、9〜15 は 1〜7 の暗い色。
pub const DEFAULT_PALETTE: [u16; 16] = [
    0x000, 0x00F, 0xF00, 0xF0F, 0x0F0, 0x0FF, 0xFF0, 0xFFF, //
    0x777, 0x007, 0x700, 0x707, 0x070, 0x077, 0x770, 0x777,
];

pub struct Palette(pub [u16; 16]);

impl Default for Palette {
    fn default() -> Self {
        Palette(DEFAULT_PALETTE)
    }
}

impl Palette {
    /// 0x00RRGGBB
    pub fn rgb(&self, i: u8) -> u32 {
        let c = self.0[(i & 15) as usize] as u32;
        let (r, g, b) = ((c >> 8) & 15, (c >> 4) & 15, c & 15);
        (r * 0x11) << 16 | (g * 0x11) << 8 | b * 0x11
    }
}

/// 640x400 の 0x00RRGGBB バッファへ合成する。
/// グラフィックの上に文字の前景ドットを重ねる (文字の背景は透ける)。
/// `cursor` が Some のとき、その位置に下線カーソルを描く。
pub fn compose(text: &TextScreen, gfx: &Graphics, pal: &Palette, cursor: Option<(usize, usize)>, out: &mut [u32]) {
    let lut: [u32; 16] = core::array::from_fn(|i| pal.rgb(i as u8));
    for (o, &p) in out.iter_mut().zip(gfx.pix.iter()) {
        *o = lut[(p & 15) as usize];
    }
    let pitch = HEIGHT / text.rows(); // 1 行の高さ (16 または 20)
    let top = (pitch - 16) / 2;
    for ty in 0..text.rows() {
        let y0 = ty * pitch + top;
        for tx in 0..COLS {
            let cell = text.cell(tx, ty);
            if cell.tail || cell.ch == b' ' as u16 {
                continue;
            }
            let g = font::glyph(cell.ch);
            let w = if g.wide { 16 } else { 8 };
            let fg = lut[(cell.color & 15) as usize];
            for gy in 0..16 {
                let row = (y0 + gy) * WIDTH + tx * 8;
                for gx in 0..w.min(WIDTH - tx * 8) {
                    if g.pixel(gx, gy) {
                        out[row + gx] = fg;
                    }
                }
            }
        }
    }
    if let Some((cx, cy)) = cursor {
        let cx = cx.min(COLS - 1);
        let fg = lut[(text.color & 15) as usize];
        for gy in 14..16 {
            let row = (cy * pitch + top + gy) * WIDTH + cx * 8;
            out[row..row + 8].fill(fg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn palette_expands_4bit() {
        let p = Palette::default();
        assert_eq!(p.rgb(2), 0xFF0000);
        assert_eq!(p.rgb(8), 0x777777);
        assert_eq!(p.rgb(9), 0x000077);
    }

    #[test]
    fn text_over_graphics() {
        let mut t = TextScreen::new();
        let mut g = Graphics::new();
        g.pix.fill(4); // 緑
        t.color = 2; // 赤
        t.put('A' as u16);
        let mut out = vec![0u32; WIDTH * HEIGHT];
        compose(&t, &g, &Palette::default(), None, &mut out);
        // 'A' の 6 行目 (0x24): x=2 が立ち、x=0 は背景 (緑が透ける)
        assert_eq!(out[5 * WIDTH + 2], 0xFF0000);
        assert_eq!(out[5 * WIDTH], 0x00FF00);
        assert_eq!(out[399 * WIDTH + 639], 0x00FF00);
    }

    #[test]
    fn twenty_rows_are_spaced() {
        let mut t = TextScreen::new();
        t.set_rows(20);
        t.locate(0, 1);
        t.put('A' as u16);
        let mut out = vec![0u32; WIDTH * HEIGHT];
        compose(&t, &Graphics::new(), &Palette::default(), None, &mut out);
        // 1 行 20 ドット、上に 2 ドットの余白: 'A' の 6 行目は y = 20 + 2 + 5
        assert_eq!(out[27 * WIDTH + 2], 0xFFFFFF);
    }
}
