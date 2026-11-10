//! スクリーンエディタ: 画面上の任意の行をカーソルで編集し、RET でその論理行を入力する。
//! 挿入・削除で詰める範囲は現在の物理行 (80 桁) の中だけとする。

use crate::key::Key;
use crate::text::{COLS, TextScreen};
use alloc::string::String;

#[derive(Default)]
pub struct Editor {
    pub insert: bool,
}

impl Editor {
    /// キーを 1 つ処理する。RET なら入力された論理行を返す。
    pub fn key(&mut self, t: &mut TextScreen, k: Key) -> Option<String> {
        if t.x >= COLS && !matches!(k, Key::Char(_)) {
            t.x = COLS - 1;
        }
        match k {
            Key::Char(c) => {
                if self.insert {
                    insert_at(t, c);
                }
                t.put(c);
            }
            Key::Enter => {
                self.insert = false;
                return Some(enter(t));
            }
            Key::Backspace => {
                if t.x > 0 || t.is_cont(t.y) {
                    move_left(t);
                    delete_at(t);
                }
            }
            Key::Delete => delete_at(t),
            Key::Insert => self.insert = !self.insert,
            Key::Home => {
                self.insert = false;
                t.locate(0, 0);
            }
            Key::Clear => {
                self.insert = false;
                t.clear();
            }
            Key::Left => {
                self.insert = false;
                move_left(t);
            }
            Key::Right => {
                self.insert = false;
                move_right(t);
            }
            Key::Up => {
                self.insert = false;
                if t.y > 0 {
                    t.y -= 1;
                    fix_tail(t);
                }
            }
            Key::Down => {
                self.insert = false;
                if t.y + 1 < t.rows() {
                    t.y += 1;
                    fix_tail(t);
                }
            }
            Key::Stop => {}
        }
        None
    }
}

/// 全角の右半分にいたら左半分へ
fn fix_tail(t: &mut TextScreen) {
    if t.x > 0 && t.cell(t.x, t.y).tail {
        t.x -= 1;
    }
}

fn move_left(t: &mut TextScreen) {
    if t.x > 0 {
        t.x -= 1;
    } else if t.y > 0 {
        t.y -= 1;
        t.x = COLS - 1;
    }
    fix_tail(t);
}

fn move_right(t: &mut TextScreen) {
    let step = if t.cell(t.x, t.y).ch != b' ' as u16 && t.x + 1 < COLS && t.cell(t.x + 1, t.y).tail { 2 } else { 1 };
    if t.x + step < COLS {
        t.x += step;
    } else if t.y + 1 < t.rows() {
        t.y += 1;
        t.x = 0;
    }
}

/// カーソル位置の文字を消して、行の残りを左へ詰める
fn delete_at(t: &mut TextScreen) {
    let (x, y) = (t.x, t.y);
    let w = if x + 1 < COLS && t.cell(x + 1, y).tail { 2 } else { 1 };
    rewrite_row(t, x, x + w, x);
}

/// 行 y の [from, COLS) の文字を dst 以降へ書き直す (はみ出した分は消える)
fn rewrite_row(t: &mut TextScreen, clear_from: usize, from: usize, dst: usize) {
    let y = t.y;
    let cells: alloc::vec::Vec<_> = (from.min(COLS)..COLS).map(|i| t.cell(i, y)).collect();
    for i in clear_from..COLS {
        t.set(i, y, b' ' as u16, t.color);
    }
    let mut dst = dst;
    for c in cells.iter().filter(|c| !c.tail) {
        if !t.set(dst, y, c.ch, c.color) {
            break;
        }
        dst += crate::font::width(c.ch);
    }
}

/// カーソル位置に ch が入る幅だけ、行の残りを右へずらす (はみ出した分は消える)
fn insert_at(t: &mut TextScreen, ch: u16) {
    let x = t.x.min(COLS - 1);
    rewrite_row(t, x, x, x + crate::font::width(ch));
}

/// カーソルのある論理行の内容を返し、カーソルを論理行の次の行の先頭へ移す
fn enter(t: &mut TextScreen) -> String {
    enter_from(t, None)
}

/// enter と同じだが、from (論理行の中の位置) があればそこから後ろだけを読む (INPUT 用)
pub fn enter_from(t: &mut TextScreen, from: Option<(usize, usize)>) -> String {
    let mut start = t.y;
    while start > 0 && t.is_cont(start) {
        start -= 1;
    }
    let mut end = t.y;
    while end + 1 < t.rows() && t.is_cont(end + 1) {
        end += 1;
    }
    let (x0, y0) = match from {
        Some((x, y)) if (start..=end).contains(&y) => (x.min(COLS), y),
        _ => (0, start),
    };
    let mut s = String::new();
    for y in y0..=end {
        for x in (if y == y0 { x0 } else { 0 })..COLS {
            let c = t.cell(x, y);
            if !c.tail {
                s.push(char::from_u32(c.ch as u32).unwrap_or('?'));
            }
        }
    }
    t.y = end;
    t.newline();
    String::from(s.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_str(e: &mut Editor, t: &mut TextScreen, s: &str) -> Option<String> {
        let mut r = None;
        for c in s.chars() {
            r = e.key(t, Key::Char(c as u16));
        }
        r
    }

    fn row(t: &TextScreen, y: usize) -> String {
        let s: String =
            (0..COLS).filter(|&x| !t.cell(x, y).tail).map(|x| char::from_u32(t.cell(x, y).ch as u32).unwrap()).collect();
        s.trim_end().into()
    }

    #[test]
    fn typing() {
        let (mut e, mut t) = (Editor::default(), TextScreen::new());
        type_str(&mut e, &mut t, "ABC");
        assert_eq!(row(&t, 0), "ABC");
        assert_eq!(t.x, 3);
    }

    #[test]
    fn backspace_deletes_left() {
        let (mut e, mut t) = (Editor::default(), TextScreen::new());
        type_str(&mut e, &mut t, "ABC");
        e.key(&mut t, Key::Backspace);
        assert_eq!(row(&t, 0), "AB");
        assert_eq!(t.x, 2);
    }

    #[test]
    fn backspace_in_middle_closes_gap() {
        let (mut e, mut t) = (Editor::default(), TextScreen::new());
        type_str(&mut e, &mut t, "ABCD");
        e.key(&mut t, Key::Left);
        e.key(&mut t, Key::Backspace);
        assert_eq!(row(&t, 0), "ABD");
        assert_eq!(t.x, 2);
    }

    #[test]
    fn delete_wide_char() {
        let (mut e, mut t) = (Editor::default(), TextScreen::new());
        t.put_str("aあb");
        t.x = 1;
        e.key(&mut t, Key::Delete);
        assert_eq!(row(&t, 0), "ab");
    }

    #[test]
    fn insert_mode() {
        let (mut e, mut t) = (Editor::default(), TextScreen::new());
        type_str(&mut e, &mut t, "AC");
        e.key(&mut t, Key::Left);
        e.key(&mut t, Key::Insert);
        type_str(&mut e, &mut t, "B");
        assert_eq!(row(&t, 0), "ABC");
        assert_eq!(t.x, 2);
    }

    #[test]
    fn enter_reads_whole_logical_line() {
        let (mut e, mut t) = (Editor::default(), TextScreen::new());
        let long = format!("10 PRINT \"{}\"", "x".repeat(90));
        t.put_str(&long);
        t.newline();
        t.put_str("Ok");
        t.locate(3, 0); // 1 行目の途中
        let got = e.key(&mut t, Key::Enter).unwrap();
        assert_eq!(got, long);
        assert_eq!((t.x, t.y), (0, 2)); // 論理行 (2 物理行) の次
    }

    #[test]
    fn cursor_skips_right_half_of_wide() {
        let (mut e, mut t) = (Editor::default(), TextScreen::new());
        t.put_str("あい");
        t.x = 0;
        e.key(&mut t, Key::Right);
        assert_eq!(t.x, 2);
        e.key(&mut t, Key::Left);
        assert_eq!(t.x, 0);
        t.newline();
        t.x = 1;
        e.key(&mut t, Key::Up);
        assert_eq!(t.x, 0);
    }
}
