//! USB キーボードの生の報告 (HID の修飾キーと押下中のキーコード最大 6 個) を Key に変換する。
//! JIS / US 配列とキーリピートに対応する。

use crate::key::Key;
use alloc::vec::Vec;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    Jis,
    Us,
}

const CTRL: u8 = 0x01 | 0x10;
const SHIFT: u8 = 0x02 | 0x20;

/// 押し始めてからリピートが始まるまで (ミリ秒)
const REPEAT_DELAY: u64 = 500;
/// リピートの間隔 (ミリ秒)
const REPEAT_RATE: u64 = 33;

pub struct HidKeyboard {
    pub layout: Layout,
    pressed: [u8; 6],
    mods: u8,
    caps: bool,
    /// リピート中のキーと次に出す時刻
    repeat: Option<(u8, u64)>,
}

impl HidKeyboard {
    pub fn new(layout: Layout) -> Self {
        HidKeyboard { layout, pressed: [0; 6], mods: 0, caps: false, repeat: None }
    }

    /// 報告を 1 つ処理する。新しく押されたキーを out に積む。
    pub fn report(&mut self, mods: u8, keys: [u8; 6], now: u64, out: &mut Vec<Key>) {
        self.mods = mods;
        for &k in keys.iter().filter(|&&k| k > 3) {
            if !self.pressed.contains(&k) {
                if k == 0x39 {
                    self.caps = !self.caps;
                }
                if let Some(key) = self.translate(k) {
                    out.push(key);
                    self.repeat = Some((k, now + REPEAT_DELAY));
                }
            }
        }
        if let Some((k, _)) = self.repeat {
            if !keys.contains(&k) {
                self.repeat = None;
            }
        }
        self.pressed = keys;
    }

    /// 押しっぱなしのキーのリピートを出す。メインループから呼ぶ。
    pub fn tick(&mut self, now: u64, out: &mut Vec<Key>) {
        if let Some((k, at)) = self.repeat {
            if now >= at {
                if let Some(key) = self.translate(k) {
                    // Stop (ESC / Ctrl+C) はリピートしない
                    if key != Key::Stop {
                        out.push(key);
                    }
                }
                self.repeat = Some((k, now + REPEAT_RATE));
            }
        }
    }

    fn translate(&self, k: u8) -> Option<Key> {
        let shift = self.mods & SHIFT != 0;
        let ctrl = self.mods & CTRL != 0;
        let key = match k {
            0x04..=0x1D => {
                let c = b'a' + (k - 0x04);
                if ctrl {
                    return (c == b'c').then_some(Key::Stop);
                }
                let upper = shift != self.caps;
                return Some(Key::Char(if upper { c.to_ascii_uppercase() } else { c } as u16));
            }
            0x28 | 0x58 => Key::Enter,
            0x29 => Key::Stop,
            0x2A => Key::Backspace,
            0x2C => Key::Char(b' ' as u16),
            0x49 => Key::Insert,
            0x4A if shift || ctrl => Key::Clear,
            0x4A => Key::Home,
            0x4B => Key::PageUp,
            0x4C => Key::Delete,
            0x4D => Key::Help,
            0x4E => Key::PageDown,
            0x4F => Key::Right,
            0x50 => Key::Left,
            0x51 => Key::Down,
            0x52 => Key::Up,
            0x54 => Key::Char(b'/' as u16),
            0x55 => Key::Char(b'*' as u16),
            0x56 => Key::Char(b'-' as u16),
            0x57 => Key::Char(b'+' as u16),
            0x59..=0x61 => Key::Char((b'1' + (k - 0x59)) as u16),
            0x62 => Key::Char(b'0' as u16),
            0x63 => Key::Char(b'.' as u16),
            _ => {
                if ctrl {
                    return None;
                }
                let (n, s) = self.symbol(k)?;
                let c = if shift { s } else { n };
                if c == 0 {
                    return None;
                }
                Key::Char(c as u16)
            }
        };
        Some(key)
    }

    /// 数字と記号のキー: (通常, Shift)。0 は入力なし。
    fn symbol(&self, k: u8) -> Option<(u8, u8)> {
        let us = self.layout == Layout::Us;
        Some(match k {
            0x1E..=0x26 => {
                let i = (k - 0x1E) as usize;
                let shifted = if us { b"!@#$%^&*(" } else { b"!\"#$%&'()" };
                (b'1' + i as u8, shifted[i])
            }
            0x27 => (b'0', if us { b')' } else { 0 }),
            0x2D => (b'-', if us { b'_' } else { b'=' }),
            0x2E => if us { (b'=', b'+') } else { (b'^', b'~') },
            0x2F => if us { (b'[', b'{') } else { (b'@', b'`') },
            0x30 => if us { (b']', b'}') } else { (b'[', b'{') },
            0x31 | 0x32 => if us { (b'\\', b'|') } else { (b']', b'}') },
            0x33 => if us { (b';', b':') } else { (b';', b'+') },
            0x34 => if us { (b'\'', b'"') } else { (b':', b'*') },
            0x35 => if us { (b'`', b'~') } else { return None }, // JIS: 半角/全角
            0x36 => (b',', b'<'),
            0x37 => (b'.', b'>'),
            0x38 => (b'/', b'?'),
            0x87 => (b'\\', b'_'), // JIS: ろ
            0x89 => (b'\\', b'|'), // JIS: ¥
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(kb: &mut HidKeyboard, mods: u8, k: u8) -> Vec<Key> {
        let mut out = Vec::new();
        kb.report(mods, [k, 0, 0, 0, 0, 0], 0, &mut out);
        kb.report(mods, [0; 6], 0, &mut out);
        out
    }

    fn ch(c: char) -> Vec<Key> {
        alloc::vec![Key::Char(c as u16)]
    }

    #[test]
    fn letters_and_shift() {
        let mut kb = HidKeyboard::new(Layout::Jis);
        assert_eq!(press(&mut kb, 0, 0x04), ch('a'));
        assert_eq!(press(&mut kb, 0x02, 0x04), ch('A'));
        press(&mut kb, 0, 0x39); // CapsLock
        assert_eq!(press(&mut kb, 0, 0x05), ch('B'));
    }

    #[test]
    fn jis_symbols() {
        let mut kb = HidKeyboard::new(Layout::Jis);
        assert_eq!(press(&mut kb, 0x02, 0x1F), ch('"')); // Shift+2
        assert_eq!(press(&mut kb, 0x02, 0x34), ch('*')); // Shift+:
        assert_eq!(press(&mut kb, 0, 0x2F), ch('@'));
        assert_eq!(press(&mut kb, 0x02, 0x2D), ch('='));
        assert_eq!(press(&mut kb, 0x02, 0x89), ch('|'));
        assert_eq!(press(&mut kb, 0x02, 0x27), alloc::vec![]); // Shift+0 は何もしない
    }

    #[test]
    fn us_symbols() {
        let mut kb = HidKeyboard::new(Layout::Us);
        assert_eq!(press(&mut kb, 0x02, 0x1F), ch('@'));
        assert_eq!(press(&mut kb, 0x02, 0x34), ch('"'));
        assert_eq!(press(&mut kb, 0x20, 0x2E), ch('+'));
    }

    #[test]
    fn special_keys() {
        let mut kb = HidKeyboard::new(Layout::Jis);
        assert_eq!(press(&mut kb, 0, 0x29), alloc::vec![Key::Stop]);
        assert_eq!(press(&mut kb, 0x01, 0x06), alloc::vec![Key::Stop]); // Ctrl+C
        assert_eq!(press(&mut kb, 0x02, 0x4A), alloc::vec![Key::Clear]); // Shift+HOME
        assert_eq!(press(&mut kb, 0, 0x4A), alloc::vec![Key::Home]);
        assert_eq!(press(&mut kb, 0, 0x52), alloc::vec![Key::Up]);
        assert_eq!(press(&mut kb, 0, 0x2A), alloc::vec![Key::Backspace]);
    }

    #[test]
    fn held_key_does_not_repeat_until_delay() {
        let mut kb = HidKeyboard::new(Layout::Jis);
        let mut out = Vec::new();
        kb.report(0, [0x04, 0, 0, 0, 0, 0], 0, &mut out);
        kb.report(0, [0x04, 0, 0, 0, 0, 0], 100, &mut out); // 同じ報告が続いても 1 回
        kb.tick(400, &mut out);
        assert_eq!(out.len(), 1);
        kb.tick(500, &mut out);
        kb.tick(533, &mut out);
        assert_eq!(out.len(), 3);
        kb.report(0, [0; 6], 540, &mut out);
        kb.tick(600, &mut out);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn second_key_while_holding_first() {
        let mut kb = HidKeyboard::new(Layout::Jis);
        let mut out = Vec::new();
        kb.report(0, [0x04, 0, 0, 0, 0, 0], 0, &mut out);
        kb.report(0, [0x04, 0x05, 0, 0, 0, 0], 10, &mut out);
        assert_eq!(out, [Key::Char('a' as u16), Key::Char('b' as u16)]);
    }
}
