//! マシン本体: 画面・エディタ・BASIC をまとめ、Platform 経由で外界とやり取りする。

use crate::basic::interp::{Basic, LValue, Pos};
use crate::editor::{Editor, enter_from};
use crate::gfx::Graphics;
use crate::key::{InputFilter, Key, Passthrough};
use crate::screen::{Palette, compose};
use crate::text::TextScreen;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// ファイル操作の失敗
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileError {
    NotFound,
    Io,
}

/// 機種ごとの入出力
pub trait Platform {
    /// 押されたキーを 1 つ返す (なければ None)
    fn poll_key(&mut self) -> Option<Key>;
    /// 起動からのミリ秒
    fn millis(&self) -> u64;
    /// 短いビープ音 (約 0.1 秒)。音が出せなければ何もしない。
    fn beep(&mut self) {}
    /// エラーと Break のメッセージを画面の外 (端末・シリアル) にも残す。既定では何もしない。
    fn log(&mut self, _msg: &str) {}
    /// 保存先のファイルを読む。name は大文字 (例: TEST.BAS)。
    fn read_file(&mut self, _name: &str) -> Result<Vec<u8>, FileError> {
        Err(FileError::Io)
    }
    /// 保存先へ書く (同名は上書き)
    fn write_file(&mut self, _name: &str, _data: &[u8]) -> Result<(), FileError> {
        Err(FileError::Io)
    }
    /// 保存先のファイル名とサイズの一覧
    fn list_files(&mut self) -> Result<Vec<(alloc::string::String, u64)>, FileError> {
        Err(FileError::Io)
    }
    fn delete_file(&mut self, _name: &str) -> Result<(), FileError> {
        Err(FileError::Io)
    }
}

pub const TITLE: &str = "P88 BASIC version 0.1";
/// BASIC が使えるメモリの大きさ (プログラム + 変数)
pub const MEMORY_SIZE: usize = 1 << 20;

/// INPUT / LINE INPUT の入力待ち
pub struct InputState {
    pub targets: Vec<LValue>,
    pub line_input: bool,
    pub prompt: alloc::string::String,
    /// INPUT 文の位置 (Break 後の CONT で再実行する)
    pub stmt: Pos,
    /// 入力の開始位置 (x, y, その時点のスクロール回数)
    pub from: (usize, usize, u32),
}

pub enum Mode {
    /// スクリーンエディタで行を編集中
    Edit,
    /// プログラム (または直接入力した行) を実行中
    Run,
    /// INPUT の入力待ち
    Input(InputState),
}

/// 実行中に押されたキーを INKEY$ のために貯めておく数
const INKEY_BUFFER: usize = 64;

pub struct Machine<P: Platform> {
    pub platform: P,
    pub text: TextScreen,
    pub gfx: Graphics,
    pub palette: Palette,
    pub(crate) b: Basic,
    pub(crate) mode: Mode,
    pub(crate) inkeys: VecDeque<u16>,
    editor: Editor,
    filter: Box<dyn InputFilter>,
    keys: Vec<Key>,
}

impl<P: Platform> Machine<P> {
    pub fn new(platform: P) -> Self {
        let mut m = Machine {
            platform,
            text: TextScreen::new(),
            gfx: Graphics::new(),
            palette: Palette::default(),
            b: Basic::new(),
            mode: Mode::Edit,
            inkeys: VecDeque::new(),
            editor: Editor::default(),
            filter: Box::new(Passthrough),
            keys: Vec::new(),
        };
        m.boot();
        m
    }

    fn boot(&mut self) {
        self.text.clear();
        self.text.put_str(TITLE);
        self.text.newline();
        self.text.put_str(&alloc::format!("{MEMORY_SIZE} Bytes free"));
        self.text.newline();
        self.ok();
    }

    pub(crate) fn ok(&mut self) {
        if self.text.x != 0 {
            self.text.newline();
        }
        self.text.put_str("Ok");
        self.text.newline();
    }

    /// 入力を処理し、実行中なら少し (約 15 ミリ秒) 実行を進める。メインループから繰り返し呼ぶ。
    pub fn step(&mut self) {
        while let Some(k) = self.platform.poll_key() {
            self.filter.filter(k, &mut self.keys);
        }
        let keys: Vec<Key> = self.keys.drain(..).collect();
        for k in keys {
            self.key(k);
        }
        let t0 = self.platform.millis();
        while matches!(self.mode, Mode::Run) {
            self.run_statements(256);
            if self.platform.millis().wrapping_sub(t0) >= 15 {
                break;
            }
        }
    }

    fn key(&mut self, k: Key) {
        match &self.mode {
            Mode::Edit if matches!(k, Key::PageUp | Key::PageDown) => self.page(k == Key::PageDown),
            Mode::Edit if k == Key::Help => self.help(),
            Mode::Edit => {
                if let Some(line) = self.editor.key(&mut self.text, k) {
                    self.enter_line(line);
                }
            }
            Mode::Run => {
                if k == Key::Stop {
                    let at = Pos { line: self.b.line, pos: self.b.pos };
                    self.break_at(at);
                } else if let (Some(c), true) = (k.code(), self.inkeys.len() < INKEY_BUFFER) {
                    self.inkeys.push_back(c);
                }
            }
            Mode::Input(st) => match k {
                Key::Stop => {
                    let at = st.stmt.clone();
                    self.break_at(at);
                }
                Key::Enter => self.finish_input(),
                k => {
                    self.editor.key(&mut self.text, k);
                }
            },
        }
    }

    /// プロンプトを出して入力待ちにする
    pub(crate) fn begin_input(&mut self, mut st: InputState) {
        let prompt = st.prompt.clone();
        self.text.put_str(&prompt);
        st.from = (self.text.x, self.text.y, self.text.scrolls);
        self.editor.insert = false;
        self.mode = Mode::Input(st);
    }

    fn finish_input(&mut self) {
        let Mode::Input(st) = core::mem::replace(&mut self.mode, Mode::Run) else { return };
        let (fx, fy, sc) = st.from;
        let scrolled = self.text.scrolls.wrapping_sub(sc) as usize;
        let from = if scrolled <= fy { Some((fx, fy - scrolled)) } else { None };
        let s = enter_from(&mut self.text, from);
        match self.input_values(&st.targets, st.line_input, &s) {
            Ok(true) => {}
            Ok(false) => {
                self.text.put_str("?Redo from start");
                self.text.newline();
                self.begin_input(st);
            }
            Err(e) => {
                self.print_error(e, st.stmt.line);
                self.finish();
            }
        }
    }

    /// 編集中 (実行も入力待ちもしていない) か
    pub fn is_idle(&self) -> bool {
        matches!(self.mode, Mode::Edit)
    }

    /// 640x400 の 0x00RRGGBB バッファへ描画する。カーソルは 0.5 秒ごとに点滅する。
    pub fn render(&self, out: &mut [u32]) {
        let blink = (self.platform.millis() / 500) % 2 == 0 && !matches!(self.mode, Mode::Run);
        compose(&self.text, &self.gfx, &self.palette, blink.then_some((self.text.x, self.text.y)), out);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use alloc::collections::VecDeque;
    use alloc::string::String;

    /// テスト用の Platform: キー列を流し込める
    #[derive(Default)]
    pub struct Fake {
        pub keys: VecDeque<Key>,
        /// 呼ばれるたびに 1 ミリ秒進む時計
        pub ms: core::cell::Cell<u64>,
        pub beeps: usize,
        pub logs: Vec<String>,
        pub files: alloc::collections::BTreeMap<String, Vec<u8>>,
    }

    impl Platform for Fake {
        fn poll_key(&mut self) -> Option<Key> {
            self.keys.pop_front()
        }
        fn millis(&self) -> u64 {
            self.ms.set(self.ms.get() + 1);
            self.ms.get()
        }
        fn beep(&mut self) {
            self.beeps += 1;
        }
        fn log(&mut self, msg: &str) {
            self.logs.push(String::from(msg));
        }
        fn read_file(&mut self, name: &str) -> Result<Vec<u8>, FileError> {
            self.files.get(name).cloned().ok_or(FileError::NotFound)
        }
        fn write_file(&mut self, name: &str, data: &[u8]) -> Result<(), FileError> {
            self.files.insert(name.into(), data.into());
            Ok(())
        }
        fn list_files(&mut self) -> Result<Vec<(String, u64)>, FileError> {
            Ok(self.files.iter().map(|(n, d)| (n.clone(), d.len() as u64)).collect())
        }
        fn delete_file(&mut self, name: &str) -> Result<(), FileError> {
            self.files.remove(name).map(|_| ()).ok_or(FileError::NotFound)
        }
    }

    pub fn row(t: &TextScreen, y: usize) -> String {
        let s: String = (0..crate::text::COLS)
            .filter(|&x| !t.cell(x, y).tail)
            .map(|x| char::from_u32(t.cell(x, y).ch as u32).unwrap())
            .collect();
        s.trim_end().into()
    }

    #[test]
    fn boot_screen() {
        let m = Machine::new(Fake::default());
        assert_eq!(row(&m.text, 0), TITLE);
        assert_eq!(row(&m.text, 1), "1048576 Bytes free");
        assert_eq!(row(&m.text, 2), "Ok");
        assert_eq!((m.text.x, m.text.y), (0, 3));
    }

    #[test]
    fn non_ascii_keys_are_dropped() {
        let mut m = Machine::new(Fake::default());
        m.platform.keys.extend([Key::Char('あ' as u16), Key::Char('A' as u16)]);
        m.step();
        assert_eq!(row(&m.text, 3), "A");
    }
}
