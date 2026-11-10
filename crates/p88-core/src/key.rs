//! キー入力。Platform がキーボードを Key に変換し、InputFilter を通してエディタへ渡す。

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    /// 印字可能文字
    Char(u16),
    Enter,
    Backspace,
    Delete,
    Insert,
    /// カーソルを左上へ
    Home,
    /// 画面消去 (Shift+HOME)
    Clear,
    Left,
    Right,
    Up,
    Down,
    /// 実行の中断 (ESC / Ctrl+C)
    Stop,
}

impl Key {
    /// INKEY$ が返す文字コード (N88-BASIC と同じ制御コード)
    pub fn code(self) -> u16 {
        match self {
            Key::Char(c) => c,
            Key::Enter => 0x0D,
            Key::Backspace => 0x08,
            Key::Delete => 0x7F,
            Key::Insert => 0x12,
            Key::Home => 0x0B,
            Key::Clear => 0x0C,
            Key::Right => 0x1C,
            Key::Left => 0x1D,
            Key::Up => 0x1E,
            Key::Down => 0x1F,
            Key::Stop => 0x03,
        }
    }
}

/// キー入力の変換層。将来の IME (ローマ字かな、かな漢字変換) はこの実装として差し込む。
pub trait InputFilter {
    /// 1 キーを受け取り、エディタへ渡すキーを out に積む
    fn filter(&mut self, key: Key, out: &mut alloc::vec::Vec<Key>);
}

/// MVP: 素通し。キーボードからの文字は ASCII 印字可能文字のみ受け付ける。
pub struct Passthrough;

impl InputFilter for Passthrough {
    fn filter(&mut self, key: Key, out: &mut alloc::vec::Vec<Key>) {
        match key {
            Key::Char(c) if !(0x20..0x7F).contains(&c) => {}
            k => out.push(k),
        }
    }
}
