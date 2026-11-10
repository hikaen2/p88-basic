//! プログラムの保存: SAVE LOAD FILES KILL

use super::error::{Error, Result};
use super::interp::Flow;
use super::token::{Kw, Token};
use super::value::from_str;
use crate::machine::{FileError, Machine, Platform};
use alloc::string::String;
use alloc::vec::Vec;

impl From<FileError> for Error {
    fn from(e: FileError) -> Self {
        match e {
            FileError::NotFound => Error::FileNotFound,
            FileError::Io => Error::DiskIo,
        }
    }
}

/// ファイル名を大文字にし、拡張子がなければ .BAS を付ける。使えない文字は Bad file name。
fn file_name(s: &str, add_ext: bool) -> Result<String> {
    let s = s.trim().to_uppercase();
    if s.is_empty() || s.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
        return Err(Error::BadFileName);
    }
    if s.contains('.') {
        Ok(s)
    } else if add_ext {
        Ok(s + ".BAS")
    } else {
        Err(Error::BadFileName)
    }
}

impl<P: Platform> Machine<P> {
    pub(crate) fn file_statement(&mut self, kw: Kw) -> Result<Flow> {
        match kw {
            Kw::Save => {
                let name = file_name(&self.string_arg()?, true)?;
                let mut text = String::new();
                for n in self.b.program.keys() {
                    text.push_str(&self.b.list_line(*n).unwrap());
                    text.push('\n');
                }
                self.platform.write_file(&name, text.as_bytes())?;
                Ok(Flow::Next)
            }
            Kw::Load => {
                let name = file_name(&self.string_arg()?, true)?;
                let run = self.accept_op(super::token::Op::Comma);
                if run && !matches!(self.peek(), Some(Token::Ident(r)) if r == "R") {
                    return Err(Error::Syntax);
                }
                if run {
                    self.b.pos += 1;
                }
                let data = self.platform.read_file(&name)?;
                self.load_program(&data)?;
                if run {
                    let first = self.b.program.keys().next().copied();
                    if let Some(n) = first {
                        self.jump(super::interp::Pos { line: Some(n), pos: 0 })?;
                        return Ok(Flow::Next);
                    }
                }
                Ok(Flow::End)
            }
            Kw::Files => {
                let mut files = self.platform.list_files()?;
                files.sort();
                for (n, size) in files {
                    let line = alloc::format!("{n:<14}{size:>8}");
                    self.text.put_str(&line);
                    self.text.newline();
                }
                Ok(Flow::Next)
            }
            Kw::Kill => {
                let name = file_name(&self.string_arg()?, false)?;
                self.platform.delete_file(&name)?;
                Ok(Flow::Next)
            }
            _ => Err(Error::Syntax),
        }
    }

    fn string_arg(&mut self) -> Result<String> {
        Ok(from_str(&self.expr()?.str()?))
    }

    /// UTF-8 テキスト (CRLF / BOM 可) をプログラムとして読み込む
    fn load_program(&mut self, data: &[u8]) -> Result<()> {
        let text = String::from_utf8_lossy(data);
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
        self.b.program.clear();
        self.b.clear_vars();
        let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
        for l in lines {
            let t = l.trim_start();
            if t.is_empty() {
                continue;
            }
            let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
            match t[..digits].parse::<u32>() {
                Ok(n) if n <= super::interp::MAX_LINE as u32 => self.b.store_line(n as u16, &t[digits..]),
                _ => return Err(Error::DirectStatementInFile),
            }
        }
        Ok(())
    }
}
