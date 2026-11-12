//! インタプリタ本体: プログラムの保持、文の実行、式の評価。
//! 実行状態は Machine に持たせ、Machine::step から少しずつ実行する。

use super::error::{Error, Result};
use super::token::{Kw, Op, Token, detokenize, tokenize};
use super::value::{Value, format_num, int_or_sng, parse_num, round_i16, to_str};
use crate::machine::{Machine, Mode, Platform};
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub const MAX_LINE: u16 = 65529;

/// 実行位置 (line が None なら直接入力された行)
#[derive(Clone, Debug, PartialEq)]
pub struct Pos {
    pub line: Option<u16>,
    pub pos: usize,
}

struct ForFrame {
    var: String,
    limit: f32,
    step: f32,
    body: Pos,
}

struct Array {
    dims: Vec<usize>,
    data: Vec<Value>,
}

/// 代入先
#[derive(Clone, Debug)]
pub enum LValue {
    Var(String),
    Elem(String, Vec<usize>),
}

/// 文の実行結果
pub enum Flow {
    Next,
    /// 実行終了 (Ok を出して編集へ)
    End,
    /// STOP による中断
    Stop,
    /// 入力待ち (INPUT)
    Suspend,
}

#[derive(Default)]
pub struct Basic {
    pub program: BTreeMap<u16, Rc<[Token]>>,
    vars: BTreeMap<String, Value>,
    arrays: BTreeMap<String, Array>,
    gosub: Vec<Pos>,
    fors: Vec<ForFrame>,
    data: Option<Vec<(u16, String)>>,
    data_ptr: usize,
    pub cont: Option<Pos>,
    pub line: Option<u16>,
    /// EDIT . の対象 (最後に入力した行、またはエラー・中断の行)
    pub current: Option<u16>,
    /// EDIT で画面の一番上にある行 (None なら EDIT モードでない)
    pub edit_top: Option<Slot>,
    toks: Rc<[Token]>,
    pub pos: usize,
    /// 実行中の文の先頭 (中断・エラー時の位置)
    pub stmt_start: usize,
    /// 文の実行中に別の位置へ飛んだ
    jumped: bool,
    direct: Rc<[Token]>,
    rnd: u32,
    last_rnd: f32,
}

impl Basic {
    pub fn new() -> Self {
        Basic { rnd: 0x2545_F491, ..Default::default() }
    }

    /// 変数と制御スタックを消す (RUN / NEW / LOAD)
    pub fn clear_vars(&mut self) {
        self.vars.clear();
        self.arrays.clear();
        self.gosub.clear();
        self.fors.clear();
        self.data = None;
        self.data_ptr = 0;
        self.cont = None;
    }

    /// 行番号付きの行を登録する。本文が空なら削除。
    /// 行番号の直後の空白 1 つは区切りとして捨て、残りはインデントとして保持する。
    pub fn store_line(&mut self, n: u16, body: &str) {
        let body = body.strip_prefix(' ').unwrap_or(body);
        if body.trim().is_empty() {
            self.program.remove(&n);
        } else {
            self.program.insert(n, tokenize(body).into());
            self.current = Some(n);
        }
        self.cont = None;
        self.data = None;
    }

    /// LIST 用の 1 行
    pub fn list_line(&self, n: u16) -> Option<String> {
        self.program.get(&n).map(|t| alloc::format!("{n} {}", detokenize(t)))
    }
}

/// EDIT 中の画面の行: プログラムの行、または先頭より上・末尾より下の空行 (何行目か)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    Above(u32),
    Line(u16),
    Below(u32),
}

/// LIST した 1 行が画面で占める行数 (全角が行末に入らなければ次の行へ送る)
fn line_rows(s: &str) -> usize {
    let (mut rows, mut x) = (1, 0);
    for c in s.encode_utf16() {
        let w = crate::font::width(c);
        if x + w > crate::text::COLS {
            rows += 1;
            x = 0;
        }
        x += w;
    }
    rows
}

/// 変数名を型付きのキーにする (接尾辞なしと # は単精度 !)
fn var_key(name: &str) -> String {
    let mut k = String::from(name);
    match k.chars().last() {
        Some('%' | '!' | '$') => {}
        Some('#') => {
            k.pop();
            k.push('!');
        }
        _ => k.push('!'),
    }
    k
}

fn default_value(key: &str) -> Value {
    match key.chars().last() {
        Some('%') => Value::Int(0),
        Some('$') => Value::Str(Rc::from([])),
        _ => Value::Sng(0.0),
    }
}

/// 代入時の型変換
fn coerce(key: &str, v: Value) -> Result<Value> {
    match (key.chars().last(), v) {
        (Some('$'), v @ Value::Str(_)) => Ok(v),
        (Some('$'), _) | (_, Value::Str(_)) => Err(Error::TypeMismatch),
        (Some('%'), v) => Ok(Value::Int(v.int()?)),
        (_, v) => Ok(Value::Sng(v.num()?)),
    }
}

fn check_f(f: f32) -> Result<Value> {
    if f.is_finite() { Ok(Value::Sng(f)) } else { Err(Error::Overflow) }
}

impl<P: Platform> Machine<P> {
    // ---- 行の入力と実行の開始 ----

    /// エディタで RET された行を処理する
    pub(crate) fn enter_line(&mut self, line: String) {
        let t = line.trim_start();
        if t.is_empty() {
            return;
        }
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 {
            match t[..digits].parse::<u32>() {
                Ok(n) if n <= MAX_LINE as u32 => self.b.store_line(n as u16, &t[digits..]),
                _ => {
                    self.print_error(Error::Syntax, None);
                    self.ok();
                }
            }
            return;
        }
        self.b.edit_top = None;
        self.b.direct = tokenize(t).into();
        self.b.line = None;
        self.b.toks = self.b.direct.clone();
        self.b.pos = 0;
        self.mode = Mode::Run;
    }

    pub(crate) fn print_error(&mut self, e: Error, line: Option<u16>) {
        if self.text.x != 0 {
            self.text.newline();
        }
        if line.is_some() {
            self.b.current = line;
        }
        let msg = match line {
            Some(n) => alloc::format!("{} in {n}", e.message()),
            None => String::from(e.message()),
        };
        self.text.put_str(&msg);
        self.text.newline();
        self.platform.log(&msg);
    }

    /// 実行を止めて編集に戻る
    pub(crate) fn finish(&mut self) {
        self.mode = Mode::Edit;
        self.ok();
    }

    /// Break (ESC / Ctrl+C / STOP)
    pub(crate) fn break_at(&mut self, at: Pos) {
        let msg = match at.line {
            Some(n) => alloc::format!("Break in {n}"),
            None => String::from("Break"),
        };
        if at.line.is_some() {
            self.b.current = at.line;
            self.b.cont = Some(at);
        }
        if self.text.x != 0 {
            self.text.newline();
        }
        self.text.put_str(&msg);
        self.text.newline();
        self.platform.log(&msg);
        self.finish();
    }

    /// 文を最大 n 個実行する
    pub(crate) fn run_statements(&mut self, n: usize) {
        for _ in 0..n {
            if !matches!(self.mode, Mode::Run) {
                return;
            }
            match self.exec_one() {
                Ok(Flow::Next) => {}
                Ok(Flow::Suspend) => return,
                Ok(Flow::End) => {
                    self.finish();
                    return;
                }
                Ok(Flow::Stop) => {
                    let at = Pos { line: self.b.line, pos: self.b.pos };
                    self.break_at(at);
                    return;
                }
                Err(e) => {
                    let line = self.b.line;
                    self.print_error(e, line);
                    self.b.cont = None;
                    self.finish();
                    return;
                }
            }
        }
    }

    /// 現在位置の文を 1 つ実行する
    fn exec_one(&mut self) -> Result<Flow> {
        self.skip_ws();
        if self.b.pos >= self.b.toks.len() {
            return self.next_line();
        }
        if self.accept_op(Op::Colon) {
            return Ok(Flow::Next);
        }
        self.b.stmt_start = self.b.pos;
        self.b.jumped = false;
        let flow = self.statement()?;
        if let (Flow::Next, false) = (&flow, self.b.jumped) {
            self.skip_ws();
            match self.b.toks.get(self.b.pos) {
                None | Some(Token::Op(Op::Colon)) => {}
                Some(Token::Kw(Kw::Else)) | Some(Token::Rem(..)) => self.b.pos = self.b.toks.len(),
                _ => return Err(Error::Syntax),
            }
        }
        Ok(flow)
    }

    /// 行末に来たら次の行へ
    fn next_line(&mut self) -> Result<Flow> {
        let Some(cur) = self.b.line else { return Ok(Flow::End) };
        let next = self.b.program.range(cur + 1..).next().map(|(n, t)| (*n, t.clone()));
        match next {
            Some((n, t)) => {
                self.b.line = Some(n);
                self.b.toks = t;
                self.b.pos = 0;
                Ok(Flow::Next)
            }
            None => Ok(Flow::End),
        }
    }

    pub(crate) fn jump(&mut self, p: Pos) -> Result<()> {
        match p.line {
            Some(n) => {
                let t = self.b.program.get(&n).ok_or(Error::UndefinedLine)?.clone();
                self.b.line = Some(n);
                self.b.toks = t;
            }
            None => {
                self.b.line = None;
                self.b.toks = self.b.direct.clone();
            }
        }
        self.b.pos = p.pos;
        self.b.jumped = true;
        Ok(())
    }

    fn goto_line(&mut self, n: u16) -> Result<()> {
        self.jump(Pos { line: Some(n), pos: 0 })
    }

    // ---- トークンの読み取り ----

    fn skip_ws(&mut self) {
        while let Some(Token::Ws(_)) = self.b.toks.get(self.b.pos) {
            self.b.pos += 1;
        }
    }

    pub(crate) fn peek(&mut self) -> Option<Token> {
        self.skip_ws();
        self.b.toks.get(self.b.pos).cloned()
    }

    fn is_op(&mut self, op: Op) -> bool {
        self.skip_ws();
        matches!(self.b.toks.get(self.b.pos), Some(Token::Op(o)) if *o == op)
    }

    pub(crate) fn accept_op(&mut self, op: Op) -> bool {
        let r = self.is_op(op);
        if r {
            self.b.pos += 1;
        }
        r
    }

    pub(crate) fn expect_op(&mut self, op: Op) -> Result<()> {
        if self.accept_op(op) { Ok(()) } else { Err(Error::Syntax) }
    }

    fn is_kw(&mut self, kw: Kw) -> bool {
        self.skip_ws();
        matches!(self.b.toks.get(self.b.pos), Some(Token::Kw(k)) if *k == kw)
    }

    pub(crate) fn accept_kw(&mut self, kw: Kw) -> bool {
        let r = self.is_kw(kw);
        if r {
            self.b.pos += 1;
        }
        r
    }

    /// 文の終わり (行末、:、ELSE)
    pub(crate) fn at_end(&mut self) -> bool {
        self.skip_ws();
        matches!(
            self.b.toks.get(self.b.pos),
            None | Some(Token::Op(Op::Colon)) | Some(Token::Kw(Kw::Else)) | Some(Token::Rem(..))
        )
    }

    fn line_number(&mut self) -> Result<u16> {
        match self.peek() {
            Some(Token::Num(v, _)) => {
                self.b.pos += 1;
                let f = v.num()?;
                if f < 0.0 || f > MAX_LINE as f32 || f != libm::floorf(f) {
                    return Err(Error::Syntax);
                }
                Ok(f as u16)
            }
            _ => Err(Error::Syntax),
        }
    }

    // ---- 文 ----

    fn statement(&mut self) -> Result<Flow> {
        let Some(tok) = self.peek() else { return Ok(Flow::Next) };
        let kw = match tok {
            Token::Kw(k) => k,
            Token::Ident(_) => return self.st_let(),
            Token::Rem(..) => {
                self.b.pos = self.b.toks.len();
                return Ok(Flow::Next);
            }
            _ => return Err(Error::Syntax),
        };
        self.b.pos += 1;
        match kw {
            Kw::Print => self.st_print(),
            Kw::Input => self.st_input(false),
            Kw::Line if self.accept_kw(Kw::Input) => self.st_input(true),
            Kw::Line => self.screen_statement(Kw::Line),
            Kw::Let => self.st_let(),
            Kw::If => self.st_if(),
            Kw::Goto => {
                let n = self.line_number()?;
                self.goto_line(n)?;
                Ok(Flow::Next)
            }
            Kw::Gosub => {
                let n = self.line_number()?;
                if !self.at_end() {
                    return Err(Error::Syntax);
                }
                self.b.gosub.push(Pos { line: self.b.line, pos: self.b.pos });
                self.goto_line(n)?;
                Ok(Flow::Next)
            }
            Kw::Return => {
                let p = self.b.gosub.pop().ok_or(Error::ReturnWithoutGosub)?;
                self.jump(p)?;
                Ok(Flow::Next)
            }
            Kw::For => self.st_for(),
            Kw::Next => self.st_next(),
            Kw::End => {
                self.b.cont = None;
                Ok(Flow::End)
            }
            Kw::Stop => Ok(Flow::Stop),
            Kw::Rem => {
                self.b.pos = self.b.toks.len();
                Ok(Flow::Next)
            }
            Kw::Data => {
                if let Some(Token::Data(_)) = self.b.toks.get(self.b.pos) {
                    self.b.pos += 1;
                }
                Ok(Flow::Next)
            }
            Kw::Dim => self.st_dim(),
            Kw::Read => self.st_read(),
            Kw::Restore => {
                let n = if self.at_end() { 0 } else { self.line_number()? };
                self.build_data();
                let data = self.b.data.as_ref().unwrap();
                self.b.data_ptr = data.iter().position(|(l, _)| *l >= n).unwrap_or(data.len());
                Ok(Flow::Next)
            }
            Kw::List => self.st_list(),
            Kw::Edit => self.st_edit(),
            Kw::Run => {
                let start = if self.at_end() { None } else { Some(self.line_number()?) };
                self.b.clear_vars();
                let first = match start {
                    Some(n) => Some(n),
                    None => self.b.program.keys().next().copied(),
                };
                match first {
                    Some(n) => {
                        self.goto_line(n)?;
                        Ok(Flow::Next)
                    }
                    None => Ok(Flow::End),
                }
            }
            Kw::New => {
                self.b.program.clear();
                self.b.clear_vars();
                Ok(Flow::End)
            }
            Kw::Cont => {
                let p = self.b.cont.take().ok_or(Error::CantContinue)?;
                self.jump(p)?;
                Ok(Flow::Next)
            }
            Kw::Else => {
                self.b.pos = self.b.toks.len();
                Ok(Flow::Next)
            }
            _ => self.screen_statement(kw),
        }
    }

    /// 文字列をテキスト画面へ出力する
    pub(crate) fn print_str(&mut self, s: &[u16]) {
        for &c in s {
            self.text.put(c);
        }
    }

    fn st_print(&mut self) -> Result<Flow> {
        let mut newline = true;
        while !self.at_end() {
            if self.accept_op(Op::Semi) {
                newline = false;
                continue;
            }
            if self.accept_op(Op::Comma) {
                let next = (self.text.x / 14 + 1) * 14;
                if next >= crate::text::COLS {
                    self.text.newline();
                } else {
                    while self.text.x < next {
                        self.text.put(b' ' as u16);
                    }
                }
                newline = false;
                continue;
            }
            let v = self.expr()?;
            match &v {
                Value::Str(s) => self.print_str(s),
                v => {
                    let s = format_num(v);
                    self.text.put_str(&s);
                    self.text.put(b' ' as u16);
                }
            }
            newline = true;
        }
        if newline {
            self.text.newline();
        }
        Ok(Flow::Next)
    }

    fn st_input(&mut self, line_input: bool) -> Result<Flow> {
        let start = self.b.stmt_start;
        let mut prompt = String::new();
        let mut question = true;
        if let Some(Token::Str(s, _)) = self.peek() {
            self.b.pos += 1;
            prompt = s;
            if self.accept_op(Op::Comma) {
                question = false;
            } else {
                self.expect_op(Op::Semi)?;
            }
        }
        if line_input {
            question = false;
        }
        if question {
            prompt.push_str("? ");
        }
        let mut targets = vec![self.lvalue()?];
        while !line_input && self.accept_op(Op::Comma) {
            targets.push(self.lvalue()?);
        }
        if line_input && !targets.iter().all(|t| lv_key(t).ends_with('$')) {
            return Err(Error::TypeMismatch);
        }
        if !self.at_end() {
            return Err(Error::Syntax);
        }
        self.begin_input(crate::machine::InputState {
            targets,
            line_input,
            prompt,
            stmt: Pos { line: self.b.line, pos: start },
            from: (0, 0, 0),
        });
        Ok(Flow::Suspend)
    }

    fn st_let(&mut self) -> Result<Flow> {
        let lv = self.lvalue()?;
        self.expect_op(Op::Eq)?;
        let v = self.expr()?;
        self.assign(&lv, v)?;
        Ok(Flow::Next)
    }

    fn st_if(&mut self) -> Result<Flow> {
        let cond = self.expr()?.num()? != 0.0;
        let then = self.accept_kw(Kw::Then);
        if !then && !self.is_kw(Kw::Goto) {
            return Err(Error::Syntax);
        }
        if cond {
            return self.branch_target();
        }
        // ELSE を探す (入れ子の IF の ELSE は飛ばす)
        let mut depth = 0;
        let mut i = self.b.pos;
        while i < self.b.toks.len() {
            match self.b.toks[i] {
                Token::Kw(Kw::If) => depth += 1,
                Token::Kw(Kw::Else) if depth == 0 => {
                    self.b.pos = i + 1;
                    return self.branch_target();
                }
                Token::Kw(Kw::Else) => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        self.b.pos = self.b.toks.len();
        Ok(Flow::Next)
    }

    /// THEN / ELSE の後: 行番号なら GOTO、そうでなければその文を続けて実行する
    fn branch_target(&mut self) -> Result<Flow> {
        self.accept_kw(Kw::Goto);
        if let Some(Token::Num(..)) = self.peek() {
            let n = self.line_number()?;
            self.goto_line(n)?;
            return Ok(Flow::Next);
        }
        // 次の文として実行させる: 区切りの : があるかのように扱う
        self.b.stmt_start = self.b.pos;
        if self.at_end() {
            return Ok(Flow::Next);
        }
        self.statement()
    }

    fn st_for(&mut self) -> Result<Flow> {
        let Some(Token::Ident(name)) = self.peek() else { return Err(Error::Syntax) };
        self.b.pos += 1;
        let key = var_key(&name);
        if key.ends_with('$') {
            return Err(Error::TypeMismatch);
        }
        self.expect_op(Op::Eq)?;
        let start = self.expr()?;
        if !self.accept_kw(Kw::To) {
            return Err(Error::Syntax);
        }
        let limit = self.expr()?.num()?;
        let step = if self.accept_kw(Kw::Step) { self.expr()?.num()? } else { 1.0 };
        if !self.at_end() {
            return Err(Error::Syntax);
        }
        self.assign(&LValue::Var(key.clone()), start)?;
        self.b.fors.retain(|f| f.var != key);
        let v = self.get_var(&key).num()?;
        if (step >= 0.0 && v > limit) || (step < 0.0 && v < limit) {
            return self.skip_for();
        }
        self.b.fors.push(ForFrame { var: key, limit, step, body: Pos { line: self.b.line, pos: self.b.pos } });
        Ok(Flow::Next)
    }

    /// 1 回も実行しない FOR: 対応する NEXT の後ろへ飛ぶ
    fn skip_for(&mut self) -> Result<Flow> {
        let mut depth = 1;
        loop {
            while self.b.pos < self.b.toks.len() {
                let t = self.b.toks[self.b.pos].clone();
                self.b.pos += 1;
                match t {
                    Token::Kw(Kw::For) => depth += 1,
                    Token::Kw(Kw::Next) => {
                        // NEXT I,J は 2 つ分
                        depth -= 1;
                        while depth > 0 {
                            let save = self.b.pos;
                            if self.peek().is_some_and(|t| matches!(t, Token::Ident(_))) {
                                self.b.pos += 1;
                                if self.accept_op(Op::Comma) {
                                    depth -= 1;
                                    continue;
                                }
                            } else {
                                self.b.pos = save;
                            }
                            break;
                        }
                        if depth == 0 {
                            while !self.at_end() {
                                self.b.pos += 1;
                            }
                            return Ok(Flow::Next);
                        }
                    }
                    _ => {}
                }
            }
            match self.b.line.and_then(|l| self.b.program.range(l + 1..).next().map(|(n, _)| *n)) {
                Some(n) => self.goto_line(n)?,
                None => return Err(Error::NextWithoutFor),
            }
        }
    }

    fn st_next(&mut self) -> Result<Flow> {
        loop {
            let name = match self.peek() {
                Some(Token::Ident(n)) if !self.at_end() => {
                    self.b.pos += 1;
                    Some(var_key(&n))
                }
                _ => None,
            };
            let idx = match &name {
                Some(k) => self.b.fors.iter().rposition(|f| &f.var == k),
                None => self.b.fors.len().checked_sub(1),
            }
            .ok_or(Error::NextWithoutFor)?;
            self.b.fors.truncate(idx + 1);
            let f = &self.b.fors[idx];
            let (key, limit, step, body) = (f.var.clone(), f.limit, f.step, f.body.clone());
            let v = self.get_var(&key).num()? + step;
            self.assign(&LValue::Var(key), check_f(v)?)?;
            if (step >= 0.0 && v <= limit) || (step < 0.0 && v >= limit) {
                self.jump(body)?;
                return Ok(Flow::Next);
            }
            self.b.fors.pop();
            if !self.accept_op(Op::Comma) {
                return Ok(Flow::Next);
            }
        }
    }

    fn st_dim(&mut self) -> Result<Flow> {
        loop {
            let Some(Token::Ident(name)) = self.peek() else { return Err(Error::Syntax) };
            self.b.pos += 1;
            let key = var_key(&name);
            self.expect_op(Op::LParen)?;
            let mut dims = Vec::new();
            loop {
                let n = self.expr()?.int()?;
                if n < 0 {
                    return Err(Error::IllegalFunctionCall);
                }
                dims.push(n as usize + 1);
                if !self.accept_op(Op::Comma) {
                    break;
                }
            }
            self.expect_op(Op::RParen)?;
            if self.b.arrays.contains_key(&key) {
                return Err(Error::DuplicateDefinition);
            }
            self.make_array(key, dims)?;
            if !self.accept_op(Op::Comma) {
                return Ok(Flow::Next);
            }
        }
    }

    fn make_array(&mut self, key: String, dims: Vec<usize>) -> Result<()> {
        let n = dims.iter().try_fold(1usize, |a, &d| a.checked_mul(d)).ok_or(Error::OutOfMemory)?;
        if n > crate::machine::MEMORY_SIZE / 4 {
            return Err(Error::OutOfMemory);
        }
        let data = vec![default_value(&key); n];
        self.b.arrays.insert(key, Array { dims, data });
        Ok(())
    }

    fn build_data(&mut self) {
        if self.b.data.is_some() {
            return;
        }
        let mut items = Vec::new();
        for (n, toks) in &self.b.program {
            for t in toks.iter() {
                if let Token::Data(d) = t {
                    for item in split_fields(d) {
                        items.push((*n, item));
                    }
                }
            }
        }
        self.b.data = Some(items);
        self.b.data_ptr = 0;
    }

    fn st_read(&mut self) -> Result<Flow> {
        self.build_data();
        loop {
            let lv = self.lvalue()?;
            let data = self.b.data.as_ref().unwrap();
            let (_, item) = data.get(self.b.data_ptr).ok_or(Error::OutOfData)?.clone();
            self.b.data_ptr += 1;
            let v = field_value(&lv_key(&lv), &item).ok_or(Error::Syntax)?;
            self.assign(&lv, v)?;
            if !self.accept_op(Op::Comma) {
                return Ok(Flow::Next);
            }
        }
    }

    fn st_list(&mut self) -> Result<Flow> {
        let (mut from, mut to) = (0u16, MAX_LINE);
        if !self.at_end() {
            if !self.is_op(Op::Minus) {
                from = self.line_number()?;
                to = from;
            }
            if self.accept_op(Op::Minus) {
                to = if self.at_end() { MAX_LINE } else { self.line_number()? };
            }
        }
        let lines: Vec<u16> = self.b.program.range(from..=to).map(|(n, _)| *n).collect();
        for n in lines {
            let s = self.b.list_line(n).unwrap();
            self.text.put_str(&s);
            self.text.newline();
        }
        Ok(Flow::Next)
    }

    /// EDIT 行番号 / EDIT . : その行から 1 画面分を表示して編集に戻る。
    /// 以後 PageUp / PageDown で 1 行ずつスクロールできる (直接入力の文を実行すると解除)。
    fn st_edit(&mut self) -> Result<Flow> {
        let n = match self.peek() {
            Some(Token::Other('.')) => {
                self.b.pos += 1;
                self.b.current.ok_or(Error::UndefinedLine)?
            }
            _ => self.line_number()?,
        };
        if !self.at_end() {
            return Err(Error::Syntax);
        }
        if !self.b.program.contains_key(&n) {
            return Err(Error::UndefinedLine);
        }
        self.b.current = Some(n);
        self.text.locate(0, 0);
        self.show_page(Slot::Line(n));
        self.b.cont = None;
        self.mode = Mode::Edit;
        Ok(Flow::Suspend)
    }

    /// 行 top から画面に収まるだけ表示する (カーソルは動かさない)
    fn show_page(&mut self, top: Slot) {
        let (x, y) = (self.text.x, self.text.y);
        self.text.clear();
        let (mut used, mut slot) = (0, top);
        loop {
            let s = match slot {
                Slot::Line(n) => self.b.list_line(n).unwrap(),
                _ => String::new(),
            };
            used += line_rows(&s);
            if used > self.text.rows() && slot != top {
                break;
            }
            if slot != top {
                self.text.newline();
            }
            self.text.put_str(&s);
            slot = self.next_slot(slot);
        }
        self.text.locate(x, y);
        self.b.edit_top = Some(top);
    }

    /// 次 (下) の行
    fn next_slot(&self, s: Slot) -> Slot {
        let first = || self.b.program.keys().next().map_or(Slot::Below(1), |&n| Slot::Line(n));
        match s {
            Slot::Above(1) => first(),
            Slot::Above(k) => Slot::Above(k - 1),
            Slot::Line(n) => self.b.program.range(n + 1..).next().map_or(Slot::Below(1), |(&n, _)| Slot::Line(n)),
            Slot::Below(k) => Slot::Below(k + 1),
        }
    }

    /// 前 (上) の行
    fn prev_slot(&self, s: Slot) -> Slot {
        let last = || self.b.program.keys().next_back().map_or(Slot::Above(1), |&n| Slot::Line(n));
        match s {
            Slot::Above(k) => Slot::Above(k + 1),
            Slot::Line(n) => self.b.program.range(..n).next_back().map_or(Slot::Above(1), |(&n, _)| Slot::Line(n)),
            Slot::Below(1) => last(),
            Slot::Below(k) => Slot::Below(k - 1),
        }
    }

    /// HELP キー: EDIT . の行 (なければ先頭行) から EDIT モードに入る
    pub(crate) fn help(&mut self) {
        let from = self.b.current.unwrap_or(0);
        let n = self.b.program.range(from..).next().or(self.b.program.iter().next_back()).map(|(n, _)| *n);
        if let Some(n) = n {
            self.b.current = Some(n);
            self.text.locate(0, 0);
            self.show_page(Slot::Line(n));
        }
    }

    /// EDIT 中の PageDown / PageUp: 1 行ずつ、端を越えても空行でスクロールする。
    pub(crate) fn page(&mut self, down: bool) {
        let Some(top) = self.b.edit_top else { return };
        let t = if down { self.next_slot(top) } else { self.prev_slot(top) };
        self.show_page(t);
        // PageUp は 1 行目、PageDown は最終行へカーソルを移す (桁はそのまま)
        let y = if down { self.text.rows() - 1 } else { 0 };
        self.text.locate(self.text.x, y);
    }

    // ---- 変数 ----

    fn get_var(&self, key: &str) -> Value {
        self.b.vars.get(key).cloned().unwrap_or_else(|| default_value(key))
    }

    /// 代入先を読む: 変数名か配列要素
    pub(crate) fn lvalue(&mut self) -> Result<LValue> {
        let Some(Token::Ident(name)) = self.peek() else { return Err(Error::Syntax) };
        self.b.pos += 1;
        let key = var_key(&name);
        if self.accept_op(Op::LParen) {
            let idx = self.indices()?;
            Ok(LValue::Elem(key, idx))
        } else {
            Ok(LValue::Var(key))
        }
    }

    /// ( の後の添字の並びと ) を読む
    fn indices(&mut self) -> Result<Vec<usize>> {
        let mut idx = Vec::new();
        loop {
            let i = self.expr()?.int()?;
            if i < 0 {
                return Err(Error::SubscriptOutOfRange);
            }
            idx.push(i as usize);
            if !self.accept_op(Op::Comma) {
                break;
            }
        }
        self.expect_op(Op::RParen)?;
        Ok(idx)
    }

    fn elem_index(&mut self, key: &str, idx: &[usize]) -> Result<usize> {
        if !self.b.arrays.contains_key(key) {
            self.make_array(String::from(key), vec![11; idx.len()])?;
        }
        let a = &self.b.arrays[key];
        if a.dims.len() != idx.len() {
            return Err(Error::SubscriptOutOfRange);
        }
        let mut off = 0;
        for (i, (&x, &d)) in idx.iter().zip(&a.dims).enumerate() {
            if x >= d {
                return Err(Error::SubscriptOutOfRange);
            }
            off = if i == 0 { x } else { off * d + x };
        }
        Ok(off)
    }

    pub(crate) fn assign(&mut self, lv: &LValue, v: Value) -> Result<()> {
        match lv {
            LValue::Var(k) => {
                let v = coerce(k, v)?;
                self.b.vars.insert(k.clone(), v);
            }
            LValue::Elem(k, idx) => {
                let v = coerce(k, v)?;
                let i = self.elem_index(k, idx)?;
                self.b.arrays.get_mut(k).unwrap().data[i] = v;
            }
        }
        Ok(())
    }

    // ---- 式 ----

    pub(crate) fn expr(&mut self) -> Result<Value> {
        let mut a = self.or_expr()?;
        while self.accept_kw(Kw::Xor) {
            let b = self.or_expr()?;
            a = Value::Int(a.int()? ^ b.int()?);
        }
        Ok(a)
    }

    fn or_expr(&mut self) -> Result<Value> {
        let mut a = self.and_expr()?;
        while self.accept_kw(Kw::Or) {
            let b = self.and_expr()?;
            a = Value::Int(a.int()? | b.int()?);
        }
        Ok(a)
    }

    fn and_expr(&mut self) -> Result<Value> {
        let mut a = self.not_expr()?;
        while self.accept_kw(Kw::And) {
            let b = self.not_expr()?;
            a = Value::Int(a.int()? & b.int()?);
        }
        Ok(a)
    }

    fn not_expr(&mut self) -> Result<Value> {
        if self.accept_kw(Kw::Not) {
            let a = self.not_expr()?;
            return Ok(Value::Int(!a.int()?));
        }
        self.rel_expr()
    }

    fn rel_expr(&mut self) -> Result<Value> {
        let mut a = self.add_expr()?;
        loop {
            let op = match self.peek() {
                Some(Token::Op(o @ (Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge))) => o,
                _ => return Ok(a),
            };
            self.b.pos += 1;
            let b = self.add_expr()?;
            let ord = match (&a, &b) {
                (Value::Str(x), Value::Str(y)) => x.cmp(y),
                (Value::Str(_), _) | (_, Value::Str(_)) => return Err(Error::TypeMismatch),
                _ => a.num()?.partial_cmp(&b.num()?).unwrap_or(core::cmp::Ordering::Equal),
            };
            use core::cmp::Ordering::*;
            let r = match op {
                Op::Eq => ord == Equal,
                Op::Ne => ord != Equal,
                Op::Lt => ord == Less,
                Op::Gt => ord == Greater,
                Op::Le => ord != Greater,
                _ => ord != Less,
            };
            a = Value::Int(if r { -1 } else { 0 });
        }
    }

    fn add_expr(&mut self) -> Result<Value> {
        let mut a = self.mod_expr()?;
        loop {
            let plus = if self.accept_op(Op::Plus) {
                true
            } else if self.accept_op(Op::Minus) {
                false
            } else {
                return Ok(a);
            };
            let b = self.mod_expr()?;
            a = match (a, b, plus) {
                (Value::Str(x), Value::Str(y), true) => Value::Str(x.iter().chain(y.iter()).copied().collect()),
                (Value::Str(_), _, _) | (_, Value::Str(_), _) => return Err(Error::TypeMismatch),
                (Value::Int(x), Value::Int(y), p) => int_or_sng(if p { x as i32 + y as i32 } else { x as i32 - y as i32 }),
                (x, y, p) => check_f(if p { x.num()? + y.num()? } else { x.num()? - y.num()? })?,
            };
        }
    }

    fn mod_expr(&mut self) -> Result<Value> {
        let mut a = self.idiv_expr()?;
        while self.accept_kw(Kw::Mod) {
            let b = self.idiv_expr()?.int()?;
            if b == 0 {
                return Err(Error::DivisionByZero);
            }
            a = int_or_sng(a.int()? as i32 % b as i32);
        }
        Ok(a)
    }

    fn idiv_expr(&mut self) -> Result<Value> {
        let mut a = self.mul_expr()?;
        while self.accept_op(Op::IntDiv) {
            let b = self.mul_expr()?.int()?;
            if b == 0 {
                return Err(Error::DivisionByZero);
            }
            a = int_or_sng(a.int()? as i32 / b as i32);
        }
        Ok(a)
    }

    fn mul_expr(&mut self) -> Result<Value> {
        let mut a = self.unary()?;
        loop {
            let mul = if self.accept_op(Op::Mul) {
                true
            } else if self.accept_op(Op::Div) {
                false
            } else {
                return Ok(a);
            };
            let b = self.unary()?;
            a = match (a, b, mul) {
                (Value::Int(x), Value::Int(y), true) => int_or_sng(x as i32 * y as i32),
                (x, y, true) => check_f(x.num()? * y.num()?)?,
                (x, y, false) => {
                    let d = y.num()?;
                    if d == 0.0 {
                        return Err(Error::DivisionByZero);
                    }
                    check_f(x.num()? / d)?
                }
            };
        }
    }

    fn unary(&mut self) -> Result<Value> {
        if self.accept_op(Op::Minus) {
            let a = self.unary()?;
            return negate(a);
        }
        if self.accept_op(Op::Plus) {
            return self.unary();
        }
        self.pow_expr()
    }

    /// ^ は右結合 (2^3^2 = 2^9)。指数には符号を付けられる。
    fn pow_expr(&mut self) -> Result<Value> {
        let a = self.primary()?;
        if !self.accept_op(Op::Pow) {
            return Ok(a);
        }
        let b = self.unary()?;
        let (x, y) = (a.num()?, b.num()?);
        if x == 0.0 && y < 0.0 {
            return Err(Error::DivisionByZero);
        }
        check_f(libm::powf(x, y))
    }

    fn primary(&mut self) -> Result<Value> {
        let tok = self.peek().ok_or(Error::Syntax)?;
        self.b.pos += 1;
        match tok {
            Token::Num(v, _) => Ok(v),
            Token::Str(s, _) => Ok(Value::Str(to_str(&s))),
            Token::Op(Op::LParen) => {
                let v = self.expr()?;
                self.expect_op(Op::RParen)?;
                Ok(v)
            }
            Token::Ident(name) => {
                let key = var_key(&name);
                if self.accept_op(Op::LParen) {
                    let idx = self.indices()?;
                    let i = self.elem_index(&key, &idx)?;
                    Ok(self.b.arrays[&key].data[i].clone())
                } else {
                    Ok(self.get_var(&key))
                }
            }
            Token::Kw(kw) => self.function(kw),
            _ => Err(Error::Syntax),
        }
    }

    /// ( 引数, ... ) を読む
    pub(crate) fn args(&mut self) -> Result<Vec<Value>> {
        self.expect_op(Op::LParen)?;
        let mut v = vec![self.expr()?];
        while self.accept_op(Op::Comma) {
            v.push(self.expr()?);
        }
        self.expect_op(Op::RParen)?;
        Ok(v)
    }

    fn function(&mut self, kw: Kw) -> Result<Value> {
        // 引数なしの関数
        match kw {
            Kw::Inkey => {
                return Ok(Value::Str(match self.inkeys.pop_front() {
                    Some(c) => Rc::from([c]),
                    None => Rc::from([]),
                }));
            }
            Kw::Csrlin => return Ok(Value::Int(self.text.y as i16)),
            Kw::Rnd if !self.is_op(Op::LParen) => return Ok(Value::Sng(self.rnd(1.0))),
            _ => {}
        }
        let a = self.args()?;
        let n = a.len();
        let arity = |lo: usize, hi: usize| if (lo..=hi).contains(&n) { Ok(()) } else { Err(Error::Syntax) };
        let num1 = |f: fn(f32) -> f32| -> Result<Value> {
            arity(1, 1)?;
            check_f(f(a[0].num()?))
        };
        match kw {
            Kw::Abs => match &a[..] {
                [Value::Int(i)] => Ok(int_or_sng((*i as i32).abs())),
                _ => num1(libm::fabsf),
            },
            Kw::Int => match &a[..] {
                [Value::Int(i)] => Ok(Value::Int(*i)),
                _ => num1(libm::floorf),
            },
            Kw::Fix => match &a[..] {
                [Value::Int(i)] => Ok(Value::Int(*i)),
                _ => num1(libm::truncf),
            },
            Kw::Sgn => {
                arity(1, 1)?;
                let x = a[0].num()?;
                Ok(Value::Int(if x > 0.0 {
                    1
                } else if x < 0.0 {
                    -1
                } else {
                    0
                }))
            }
            Kw::Sqr => {
                arity(1, 1)?;
                let x = a[0].num()?;
                if x < 0.0 {
                    return Err(Error::IllegalFunctionCall);
                }
                check_f(libm::sqrtf(x))
            }
            Kw::Sin => num1(libm::sinf),
            Kw::Cos => num1(libm::cosf),
            Kw::Tan => num1(libm::tanf),
            Kw::Atn => num1(libm::atanf),
            Kw::Exp => num1(libm::expf),
            Kw::Log => {
                arity(1, 1)?;
                let x = a[0].num()?;
                if x <= 0.0 {
                    return Err(Error::IllegalFunctionCall);
                }
                check_f(libm::logf(x))
            }
            Kw::Rnd => {
                arity(1, 1)?;
                let x = a[0].num()?;
                Ok(Value::Sng(self.rnd(x)))
            }
            Kw::Len => {
                arity(1, 1)?;
                Ok(Value::Int(a[0].str()?.len() as i16))
            }
            Kw::Left | Kw::Right => {
                arity(2, 2)?;
                let s = a[0].str()?;
                let k = (count(&a[1])?).min(s.len());
                Ok(Value::Str(if kw == Kw::Left { s[..k].into() } else { s[s.len() - k..].into() }))
            }
            Kw::Mid => {
                arity(2, 3)?;
                let s = a[0].str()?;
                let start = count(&a[1])?;
                if start == 0 {
                    return Err(Error::IllegalFunctionCall);
                }
                let len = if n == 3 { count(&a[2])? } else { usize::MAX };
                let b = (start - 1).min(s.len());
                let e = b.saturating_add(len).min(s.len());
                Ok(Value::Str(s[b..e].into()))
            }
            Kw::Instr => {
                arity(2, 3)?;
                let (start, h, nd) = if n == 3 { (count(&a[0])?, a[1].str()?, a[2].str()?) } else { (1, a[0].str()?, a[1].str()?) };
                if start == 0 {
                    return Err(Error::IllegalFunctionCall);
                }
                let r = if start > h.len() {
                    0
                } else if nd.is_empty() {
                    start
                } else {
                    h[start - 1..].windows(nd.len()).position(|w| *w == *nd).map_or(0, |p| p + start)
                };
                Ok(Value::Int(r as i16))
            }
            Kw::Chr => {
                arity(1, 1)?;
                let c = a[0].num()?;
                if !(0.0..=65535.0).contains(&c) {
                    return Err(Error::IllegalFunctionCall);
                }
                Ok(Value::Str(Rc::from([libm::floorf(c + 0.5) as u16])))
            }
            Kw::Asc => {
                arity(1, 1)?;
                let s = a[0].str()?;
                let c = *s.first().ok_or(Error::IllegalFunctionCall)?;
                Ok(int_or_sng(c as i32))
            }
            Kw::Str => {
                arity(1, 1)?;
                if a[0].is_str() {
                    return Err(Error::TypeMismatch);
                }
                Ok(Value::Str(to_str(&format_num(&a[0]))))
            }
            Kw::Val => {
                arity(1, 1)?;
                Ok(parse_num(&a[0].str()?).0)
            }
            Kw::StringS => {
                arity(2, 2)?;
                let k = count(&a[0])?;
                let c = match &a[1] {
                    Value::Str(s) => *s.first().ok_or(Error::IllegalFunctionCall)?,
                    v => {
                        let c = v.num()?;
                        if !(0.0..=65535.0).contains(&c) {
                            return Err(Error::IllegalFunctionCall);
                        }
                        c as u16
                    }
                };
                Ok(Value::Str(vec![c; k].into()))
            }
            Kw::Space => {
                arity(1, 1)?;
                Ok(Value::Str(vec![b' ' as u16; count(&a[0])?].into()))
            }
            Kw::Hex => {
                arity(1, 1)?;
                let v = a[0].num()?;
                let i = if (32768.0..=65535.0).contains(&v) { v as u16 } else { round_i16(v)? as u16 };
                Ok(Value::Str(to_str(&alloc::format!("{i:X}"))))
            }
            Kw::Pos => {
                arity(1, 1)?;
                Ok(Value::Int(self.text.x.min(crate::text::COLS - 1) as i16))
            }
            _ => self.screen_function(kw, &a),
        }
    }

    /// RND: x < 0 で種を設定、0 で直前の値、正で次の値
    fn rnd(&mut self, x: f32) -> f32 {
        if x < 0.0 {
            self.b.rnd = x.to_bits() | 1;
        }
        if x != 0.0 {
            let mut s = self.b.rnd;
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            self.b.rnd = s;
            self.b.last_rnd = (s >> 8) as f32 / (1u32 << 24) as f32;
        }
        self.b.last_rnd
    }

    // ---- INPUT の完了 ----

    /// 入力された文字列を代入する。正しくなければ false (Redo from start)。
    pub(crate) fn input_values(&mut self, targets: &[LValue], line_input: bool, s: &str) -> Result<bool> {
        if line_input {
            self.assign(&targets[0], Value::Str(to_str(s)))?;
            return Ok(true);
        }
        let fields = split_fields(s);
        if fields.len() != targets.len() {
            return Ok(false);
        }
        let mut vals = Vec::new();
        for (t, f) in targets.iter().zip(&fields) {
            match field_value(&lv_key(t), f) {
                Some(v) => vals.push(v),
                None => return Ok(false),
            }
        }
        for (t, v) in targets.iter().zip(vals) {
            self.assign(t, v)?;
        }
        Ok(true)
    }
}

fn negate(a: Value) -> Result<Value> {
    match a {
        Value::Int(i) => Ok(int_or_sng(-(i as i32))),
        Value::Sng(f) => Ok(Value::Sng(-f)),
        Value::Str(_) => Err(Error::TypeMismatch),
    }
}

/// 文字数・回数などの引数 (0〜32767)
fn count(v: &Value) -> Result<usize> {
    let i = v.int()?;
    if i < 0 {
        return Err(Error::IllegalFunctionCall);
    }
    Ok(i as usize)
}

pub(crate) fn lv_key(lv: &LValue) -> String {
    match lv {
        LValue::Var(k) | LValue::Elem(k, _) => k.clone(),
    }
}

/// DATA / INPUT の項目をカンマで分ける (引用符の中のカンマは区切らない)
fn split_fields(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    for c in s.chars() {
        match c {
            '"' => {
                q = !q;
                cur.push(c);
            }
            ',' if !q => out.push(core::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// 1 項目を代入先の型の値にする。数値として読めなければ None。
fn field_value(key: &str, item: &str) -> Option<Value> {
    let t = item.trim();
    if key.ends_with('$') {
        let s = t.strip_prefix('"').map(|r| r.strip_suffix('"').unwrap_or(r)).unwrap_or(t);
        return Some(Value::Str(to_str(s)));
    }
    let u = to_str(t);
    let (v, n) = parse_num(&u);
    if n != u.len() || (n == 0 && !t.is_empty()) {
        return None;
    }
    Some(v)
}
