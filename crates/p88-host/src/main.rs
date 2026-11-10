//! PC 上で p88 を動かす (開発用)。プログラムの保存先は ./disk/ (--disk DIR で変更)。
//!   cargo run -p p88-host                          ウィンドウで起動
//!   cargo run -p p88-host -- --disk samples        samples/ のプログラムを LOAD できる
//!   cargo run -p p88-host -- --exec 'LINE' ... --screenshot F
//!       行を順に実行し (最大 5 秒)、画面を F (PPM) に書いて終了する
use minifb::{InputCallback, Key as K, KeyRepeat, Scale, Window, WindowOptions};
use p88_core::gfx::{HEIGHT, WIDTH};
use p88_core::key::Key;
use p88_core::machine::{FileError, Machine, Platform};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

struct Host {
    keys: VecDeque<Key>,
    start: Instant,
    dir: PathBuf,
}

impl Host {
    /// 大文字小文字を区別せずにファイルを探す
    fn find(&self, name: &str) -> Option<PathBuf> {
        std::fs::read_dir(&self.dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name)))
    }
}

fn io(e: std::io::Error) -> FileError {
    if e.kind() == std::io::ErrorKind::NotFound { FileError::NotFound } else { FileError::Io }
}

impl Platform for Host {
    fn poll_key(&mut self) -> Option<Key> {
        self.keys.pop_front()
    }
    fn millis(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
    fn beep(&mut self) {
        print!("\x07");
        let _ = std::io::stdout().flush();
    }
    fn read_file(&mut self, name: &str) -> Result<Vec<u8>, FileError> {
        let p = self.find(name).ok_or(FileError::NotFound)?;
        std::fs::read(p).map_err(io)
    }
    fn write_file(&mut self, name: &str, data: &[u8]) -> Result<(), FileError> {
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let p = self.find(name).unwrap_or_else(|| self.dir.join(name));
        std::fs::write(p, data).map_err(io)
    }
    fn list_files(&mut self) -> Result<Vec<(String, u64)>, FileError> {
        let Ok(rd) = std::fs::read_dir(&self.dir) else { return Ok(Vec::new()) };
        Ok(rd
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| (e.file_name().to_string_lossy().to_uppercase(), e.metadata().map_or(0, |m| m.len())))
            .collect())
    }
    fn delete_file(&mut self, name: &str) -> Result<(), FileError> {
        let p = self.find(name).ok_or(FileError::NotFound)?;
        std::fs::remove_file(p).map_err(io)
    }
}

/// minifb の文字入力を受け取る
struct Chars(Rc<RefCell<Vec<u32>>>);

impl InputCallback for Chars {
    fn add_char(&mut self, c: u32) {
        self.0.borrow_mut().push(c);
    }
}

/// 特殊キーを Key に変換する
fn special(k: K, shift: bool, ctrl: bool) -> Option<Key> {
    Some(match k {
        K::Enter | K::NumPadEnter => Key::Enter,
        K::Backspace => Key::Backspace,
        K::Delete => Key::Delete,
        K::Insert => Key::Insert,
        K::Home if shift => Key::Clear,
        K::Home => Key::Home,
        K::Left => Key::Left,
        K::Right => Key::Right,
        K::Up => Key::Up,
        K::Down => Key::Down,
        K::Escape => Key::Stop,
        K::C if ctrl => Key::Stop,
        _ => return None,
    })
}

fn write_ppm(path: &Path, buf: &[u32]) -> std::io::Result<()> {
    let mut out = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    for &p in buf {
        out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    std::fs::write(path, out)
}

/// 行を画面に書いて RET し、止まるまで (最大 5 秒) 実行する
fn exec(m: &mut Machine<Host>, line: &str) {
    m.text.put_str(line);
    m.platform.keys.push_back(Key::Enter);
    let t0 = Instant::now();
    m.step();
    while !m.is_idle() && t0.elapsed().as_secs() < 5 {
        m.step();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.iter().position(|a| a == "--disk").map_or("disk", |i| &args[i + 1]);
    let host = Host { keys: VecDeque::new(), start: Instant::now(), dir: PathBuf::from(dir) };
    let mut m = Machine::new(host);
    let mut buf = vec![0u32; WIDTH * HEIGHT];

    let mut shot = None;
    let mut i = 1;
    while i + 1 < args.len() {
        match args[i].as_str() {
            "--exec" => exec(&mut m, &args[i + 1]),
            "--screenshot" => shot = Some(PathBuf::from(&args[i + 1])),
            "--disk" => {}
            a => panic!("unknown option {a}"),
        }
        i += 2;
    }
    if let Some(path) = shot {
        m.render(&mut buf);
        write_ppm(&path, &buf).expect("screenshot");
        return;
    }

    let opts = WindowOptions { scale: Scale::X2, ..WindowOptions::default() };
    let mut win = Window::new("p88", WIDTH, HEIGHT, opts).expect("window");
    win.set_target_fps(60);
    let chars = Rc::new(RefCell::new(Vec::new()));
    win.set_input_callback(Box::new(Chars(chars.clone())));
    while win.is_open() {
        let shift = win.is_key_down(K::LeftShift) || win.is_key_down(K::RightShift);
        let ctrl = win.is_key_down(K::LeftCtrl) || win.is_key_down(K::RightCtrl);
        for k in win.get_keys_pressed(KeyRepeat::Yes) {
            if let Some(k) = special(k, shift, ctrl) {
                m.platform.keys.push_back(k);
            }
        }
        for c in chars.borrow_mut().drain(..) {
            if !ctrl && c >= 0x20 && c != 0x7F && c <= 0xFFFF {
                m.platform.keys.push_back(Key::Char(c as u16));
            }
        }
        m.step();
        m.render(&mut buf);
        win.update_with_buffer(&buf, WIDTH, HEIGHT).expect("update");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(dir: &Path) -> Machine<Host> {
        let mut m = Machine::new(Host { keys: VecDeque::new(), start: Instant::now(), dir: dir.into() });
        m.text.clear();
        m
    }

    #[test]
    fn disk_roundtrip() {
        let dir = std::env::temp_dir().join(format!("p88-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut m = host(&dir);
        exec(&mut m, "10 PRINT \"あ\"");
        exec(&mut m, "SAVE \"T\"");
        assert_eq!(std::fs::read_to_string(dir.join("T.BAS")).unwrap(), "10 PRINT \"あ\"\n");
        std::fs::write(dir.join("hello.bas"), "\u{FEFF}10 PRINT \"こんにちは\"\r\n").unwrap();
        exec(&mut m, "LOAD \"HELLO\",R");
        exec(&mut m, "FILES");
        exec(&mut m, "KILL \"T.BAS\"");
        let mut screen = String::new();
        for y in 0..25 {
            for x in 0..80 {
                let c = m.text.cell(x, y);
                if !c.tail {
                    screen.push(char::from_u32(c.ch as u32).unwrap());
                }
            }
            screen.push('\n');
        }
        assert!(screen.contains("こんにちは"));
        assert!(screen.contains("HELLO.BAS"));
        assert!(!dir.join("T.BAS").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
