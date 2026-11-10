//! テキスト VRAM: 80 桁 x 25 (または 20) 行。全角文字は 2 セルを占める。

use crate::font;
use alloc::vec;
use alloc::vec::Vec;

pub const COLS: usize = 80;
pub const MAX_ROWS: usize = 25;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cell {
    pub ch: u16,
    pub color: u8,
    /// 全角文字の右半分 (左のセルに文字がある)
    pub tail: bool,
}

const BLANK: Cell = Cell { ch: b' ' as u16, color: 7, tail: false };

pub struct TextScreen {
    cells: Vec<Cell>,
    /// 行 i が行 i-1 からの折り返しの続きか
    cont: [bool; MAX_ROWS],
    rows: usize,
    /// カーソル桁。COLS は「行末まで書いた直後」(次の文字で折り返す)
    pub x: usize,
    pub y: usize,
    pub color: u8,
    /// これまでにスクロールした回数 (INPUT の開始位置の追跡用)
    pub scrolls: u32,
}

impl Default for TextScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl TextScreen {
    pub fn new() -> Self {
        TextScreen { cells: vec![BLANK; COLS * MAX_ROWS], cont: [false; MAX_ROWS], rows: MAX_ROWS, x: 0, y: 0, color: 7, scrolls: 0 }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cell(&self, x: usize, y: usize) -> Cell {
        self.cells[y * COLS + x]
    }

    /// 行 y が前の行からの折り返しの続きか
    pub fn is_cont(&self, y: usize) -> bool {
        self.cont[y]
    }

    /// 画面を消去してカーソルを左上へ
    pub fn clear(&mut self) {
        self.cells.fill(BLANK);
        self.cont = [false; MAX_ROWS];
        self.x = 0;
        self.y = 0;
    }

    /// 行数を変更する (WIDTH)。画面は消去される。
    pub fn set_rows(&mut self, rows: usize) {
        self.rows = rows;
        self.clear();
    }

    /// カーソルを移す (LOCATE)。cont は変えない。
    pub fn locate(&mut self, x: usize, y: usize) {
        self.x = x.min(COLS - 1);
        self.y = y.min(self.rows - 1);
    }

    /// (x,y) を空白にする。全角文字の片側なら相方も空白にする。
    fn clear_at(&mut self, x: usize, y: usize) {
        let i = y * COLS + x;
        let c = self.cells[i];
        if c.tail && x > 0 {
            self.cells[i - 1] = Cell { color: self.cells[i - 1].color, ..BLANK };
        } else if !c.tail && x + 1 < COLS && self.cells[i + 1].tail {
            self.cells[i + 1] = Cell { color: c.color, ..BLANK };
        }
        self.cells[i] = Cell { color: c.color, ..BLANK };
    }

    /// (x,y) に文字を書く。カーソルは動かさない。全角が行末に収まらないときは何もせず false。
    pub fn set(&mut self, x: usize, y: usize, ch: u16, color: u8) -> bool {
        let w = font::width(ch);
        if x + w > COLS {
            return false;
        }
        for i in 0..w {
            self.clear_at(x + i, y);
        }
        self.cells[y * COLS + x] = Cell { ch, color, tail: false };
        if w == 2 {
            self.cells[y * COLS + x + 1] = Cell { ch, color, tail: true };
        }
        true
    }

    /// 次の行へ。最下行ならスクロールする。cont は呼び出し側で設定する。
    fn next_row(&mut self) {
        if self.y + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.y += 1;
        }
    }

    /// 画面全体を 1 行上へスクロールする
    pub fn scroll_up(&mut self) {
        let rows = self.rows;
        self.cells.copy_within(COLS..rows * COLS, 0);
        self.cells[(rows - 1) * COLS..rows * COLS].fill(BLANK);
        self.cont.copy_within(1..rows, 0);
        self.cont[rows - 1] = false;
        self.scrolls = self.scrolls.wrapping_add(1);
    }

    /// 改行 (カーソルを次の行の先頭へ)
    pub fn newline(&mut self) {
        self.x = 0;
        self.next_row();
        self.cont[self.y] = false;
    }

    /// 折り返し (次の行を前の行の続きとする)
    fn wrap(&mut self) {
        self.x = 0;
        self.next_row();
        self.cont[self.y] = true;
    }

    /// カーソル位置に 1 文字出力して進める。CR / LF は改行。その他の制御文字は無視する。
    pub fn put(&mut self, ch: u16) {
        match ch {
            0x0D | 0x0A => return self.newline(),
            c if c < 0x20 => return,
            _ => {}
        }
        if self.x >= COLS {
            self.wrap();
        }
        let w = font::width(ch);
        if self.x + w > COLS {
            // 全角が行末に収まらない: 残りを空白にして次の行へ
            self.clear_at(self.x, self.y);
            self.wrap();
        }
        let color = self.color;
        self.set(self.x, self.y, ch, color);
        self.x += w;
    }

    pub fn put_str(&mut self, s: &str) {
        for c in s.chars() {
            self.put(c as u32 as u16);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_text(t: &TextScreen, y: usize) -> String {
        (0..COLS)
            .filter(|&x| !t.cell(x, y).tail)
            .map(|x| char::from_u32(t.cell(x, y).ch as u32).unwrap())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn eighty_columns_do_not_wrap_early() {
        let mut t = TextScreen::new();
        t.put_str(&"*".repeat(80));
        t.newline();
        assert_eq!(row_text(&t, 0), "*".repeat(80));
        assert_eq!((t.x, t.y), (0, 1));
        assert!(!t.is_cont(1));
    }

    #[test]
    fn wide_takes_two_cells() {
        let mut t = TextScreen::new();
        t.put_str("Aあ");
        assert_eq!(t.x, 3);
        assert!(t.cell(2, 0).tail);
        assert_eq!(row_text(&t, 0), "Aあ");
    }

    #[test]
    fn wide_at_last_column_moves_to_next_row() {
        let mut t = TextScreen::new();
        t.put_str(&"a".repeat(79));
        t.put_str("あ");
        assert_eq!(t.cell(79, 0).ch, b' ' as u16);
        assert_eq!(row_text(&t, 1), "あ");
        assert!(t.is_cont(1));
        assert_eq!((t.x, t.y), (2, 1));
    }

    #[test]
    fn overwriting_half_of_wide_clears_partner() {
        let mut t = TextScreen::new();
        t.put_str("あ");
        t.set(1, 0, b'x' as u16, 7);
        assert_eq!(row_text(&t, 0), " x");
        assert!(!t.cell(1, 0).tail);
    }

    #[test]
    fn scrolls_at_bottom() {
        let mut t = TextScreen::new();
        for i in 1..=30 {
            t.put_str(&format!("{i}"));
            t.newline();
        }
        assert_eq!(row_text(&t, 23), "30");
        assert_eq!(row_text(&t, 0), "7");
        assert_eq!(t.y, 24);
    }

    #[test]
    fn wrap_marks_continuation_and_scroll_keeps_it() {
        let mut t = TextScreen::new();
        t.locate(0, 24);
        t.put_str(&"b".repeat(85));
        assert_eq!(t.y, 24);
        assert!(t.is_cont(24));
        assert!(!t.is_cont(23));
        assert_eq!(row_text(&t, 24), "bbbbb");
    }

    #[test]
    fn twenty_rows() {
        let mut t = TextScreen::new();
        t.set_rows(20);
        for _ in 0..30 {
            t.newline();
        }
        assert_eq!(t.y, 19);
    }
}
