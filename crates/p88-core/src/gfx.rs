//! グラフィック VRAM: 640x400、1 ドット 1 バイト (パレット番号 0〜15)

use alloc::vec;
use alloc::vec::Vec;

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 400;

pub struct Graphics {
    pub pix: Vec<u8>,
    /// 最終参照点 (最後に描いた点)
    pub last: (i32, i32),
}

/// 点 (中心からの相対、y は下向き) の角度が弧 s〜e (反時計回り) に入るか
fn in_arc(x: i32, y: i32, s: f32, e: f32) -> bool {
    let mut a = libm::atan2f(-y as f32, x as f32);
    if a < 0.0 {
        a += 2.0 * core::f32::consts::PI;
    }
    if s <= e { s <= a && a <= e } else { a >= s || a <= e }
}

impl Default for Graphics {
    fn default() -> Self {
        Self::new()
    }
}

impl Graphics {
    pub fn new() -> Self {
        Graphics { pix: vec![0; WIDTH * HEIGHT], last: (0, 0) }
    }

    pub fn clear(&mut self) {
        self.pix.fill(0);
    }

    /// 画面外は無視する
    pub fn pset(&mut self, x: i32, y: i32, c: u8) {
        if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
            self.pix[y as usize * WIDTH + x as usize] = c;
        }
    }

    /// 画面外は -1
    pub fn point(&self, x: i32, y: i32) -> i32 {
        if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
            self.pix[y as usize * WIDTH + x as usize] as i32
        } else {
            -1
        }
    }

    /// 線分 (Bresenham)。画面外の部分は描かない。
    pub fn line(&mut self, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: u8) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.pset(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// 塗りつぶした矩形 (両端を含む)
    pub fn fill_box(&mut self, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: u8) {
        let (xa, xb) = (x0.min(x1).max(0), x0.max(x1).min(WIDTH as i32 - 1));
        let (ya, yb) = (y0.min(y1).max(0), y0.max(y1).min(HEIGHT as i32 - 1));
        if xa > xb || ya > yb {
            return;
        }
        for y in ya..=yb {
            let row = y as usize * WIDTH;
            self.pix[row + xa as usize..=row + xb as usize].fill(c);
        }
    }

    /// 枠だけの矩形
    pub fn draw_box(&mut self, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: u8) {
        self.line((x0, y0), (x1, y0), c);
        self.line((x1, y0), (x1, y1), c);
        self.line((x1, y1), (x0, y1), c);
        self.line((x0, y1), (x0, y0), c);
    }

    /// 円
    pub fn circle(&mut self, center: (i32, i32), r: i32, c: u8) {
        self.ellipse(center, r, r, None, c);
    }

    /// 横半径 rx、縦半径 ry の楕円 (中点アルゴリズム)。
    /// arc が Some((開始, 終了)) なら、その角度 (ラジアン、右が 0 で反時計回り、0〜2π) の弧だけ描く。
    pub fn ellipse(&mut self, (cx, cy): (i32, i32), rx: i32, ry: i32, arc: Option<(f32, f32)>, c: u8) {
        let plot = |g: &mut Self, x: i64, y: i64| {
            let (x, y) = (x as i32, y as i32);
            for (px, py) in [(x, y), (-x, y), (x, -y), (-x, -y)] {
                if arc.is_none_or(|(s, e)| in_arc(px, py, s, e)) {
                    g.pset(cx + px, cy + py, c);
                }
            }
        };
        if ry == 0 {
            for x in 0..=rx as i64 {
                plot(self, x, 0);
            }
            return;
        }
        // 4 倍して整数で計算する
        let (a2, b2) = ((rx as i64).pow(2), (ry as i64).pow(2));
        let (mut x, mut y) = (0i64, ry as i64);
        let mut d = 4 * b2 - 4 * a2 * y + a2;
        while b2 * x < a2 * y {
            plot(self, x, y);
            if d < 0 {
                d += 4 * b2 * (2 * x + 3);
            } else {
                d += 4 * b2 * (2 * x + 3) + 4 * a2 * (2 - 2 * y);
                y -= 1;
            }
            x += 1;
        }
        let mut d = b2 * (2 * x + 1).pow(2) + 4 * a2 * (y - 1).pow(2) - 4 * a2 * b2;
        while y >= 0 {
            plot(self, x, y);
            if d > 0 {
                d += 4 * a2 * (3 - 2 * y);
            } else {
                d += 4 * b2 * (2 * x + 2) + 4 * a2 * (3 - 2 * y);
                x += 1;
            }
            y -= 1;
        }
    }

    /// 境界色 border で囲まれた領域を c で塗る (走査線方式、再帰なし)
    pub fn paint(&mut self, (x, y): (i32, i32), c: u8, border: u8) {
        let stop = |g: &Graphics, x: i32, y: i32| {
            let p = g.point(x, y);
            p < 0 || p as u8 == border || p as u8 == c
        };
        if stop(self, x, y) {
            return;
        }
        let mut stack = vec![(x, y)];
        while let Some((x, y)) = stack.pop() {
            if stop(self, x, y) {
                continue;
            }
            let mut l = x;
            while !stop(self, l - 1, y) {
                l -= 1;
            }
            let mut r = x;
            while !stop(self, r + 1, y) {
                r += 1;
            }
            for xx in l..=r {
                self.pset(xx, y, c);
            }
            for ny in [y - 1, y + 1] {
                let mut xx = l;
                while xx <= r {
                    if !stop(self, xx, ny) {
                        stack.push((xx, ny));
                        while xx <= r && !stop(self, xx, ny) {
                            xx += 1;
                        }
                    }
                    xx += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_box_inclusive() {
        let mut g = Graphics::new();
        g.fill_box((10, 10), (20, 20), 6);
        assert_eq!(g.point(10, 10), 6);
        assert_eq!(g.point(20, 20), 6);
        assert_eq!(g.point(21, 20), 0);
    }

    #[test]
    fn line_clips() {
        let mut g = Graphics::new();
        g.line((-100, -100), (700, 300), 7);
        assert_eq!(g.point(100, 0), 7);
        assert_eq!(g.point(600, 250), 7);
        assert_eq!(g.point(400, 400), -1);
    }

    #[test]
    fn paint_inside_circle_only() {
        let mut g = Graphics::new();
        g.circle((320, 200), 50, 2);
        g.paint((320, 200), 2, 2);
        assert_eq!(g.point(320, 200), 2);
        assert_eq!(g.point(320 + 49, 200), 2);
        assert_eq!(g.point(320 + 52, 200), 0);
        assert_eq!(g.point(0, 0), 0);
    }

    #[test]
    fn paint_whole_screen_does_not_overflow() {
        let mut g = Graphics::new();
        g.paint((0, 0), 5, 5);
        assert!(g.pix.iter().all(|&p| p == 5));
    }
}
