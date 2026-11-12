//! 字句解析。入力行をトークン列にし、LIST 用に文字列へ戻す。
//! キーワードは語の先頭で最長一致する (FORI=1TO10 は FOR I = 1 TO 10)。

use super::value::{Value, lex_number};
use alloc::string::String;
use alloc::vec::Vec;

macro_rules! keywords {
    ($($name:literal => $kw:ident,)*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum Kw { $($kw,)* }
        /// (綴り, キーワード)。長い綴りから順に照合する。
        const TABLE: &[(&str, Kw)] = &[$(($name, Kw::$kw),)*];
    };
}

keywords! {
    "PRINT" => Print, "INPUT" => Input, "LINE" => Line, "LET" => Let,
    "IF" => If, "THEN" => Then, "ELSE" => Else, "GOTO" => Goto, "GOSUB" => Gosub,
    "RETURN" => Return, "FOR" => For, "TO" => To, "STEP" => Step, "NEXT" => Next,
    "END" => End, "STOP" => Stop, "REM" => Rem, "DIM" => Dim, "READ" => Read,
    "DATA" => Data, "RESTORE" => Restore, "LIST" => List, "RUN" => Run, "NEW" => New,
    "CONT" => Cont, "CLS" => Cls, "LOCATE" => Locate, "COLOR" => Color, "WIDTH" => Width,
    "PSET" => Pset, "PRESET" => Preset, "CIRCLE" => Circle, "PAINT" => Paint, "BEEP" => Beep,
    "EDIT" => Edit, "SAVE" => Save, "LOAD" => Load, "FILES" => Files, "KILL" => Kill,
    "AND" => And, "OR" => Or, "XOR" => Xor, "NOT" => Not, "MOD" => Mod,
    "ABS" => Abs, "INT" => Int, "FIX" => Fix, "SGN" => Sgn, "SQR" => Sqr, "SIN" => Sin,
    "COS" => Cos, "TAN" => Tan, "ATN" => Atn, "EXP" => Exp, "LOG" => Log, "RND" => Rnd,
    "LEN" => Len, "LEFT$" => Left, "RIGHT$" => Right, "MID$" => Mid, "INSTR" => Instr,
    "CHR$" => Chr, "ASC" => Asc, "STR$" => Str, "VAL" => Val, "STRING$" => StringS,
    "SPACE$" => Space, "HEX$" => Hex, "INKEY$" => Inkey, "POS" => Pos, "CSRLIN" => Csrlin,
    "POINT" => Point,
}

impl Kw {
    pub fn name(self) -> &'static str {
        TABLE.iter().find(|(_, k)| *k == self).unwrap().0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Plus,
    Minus,
    Mul,
    Div,
    IntDiv,
    Pow,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    LParen,
    RParen,
    Comma,
    Semi,
    Colon,
    Hash,
}

impl Op {
    fn text(self) -> &'static str {
        match self {
            Op::Plus => "+",
            Op::Minus => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::IntDiv => "\\",
            Op::Pow => "^",
            Op::Eq => "=",
            Op::Ne => "<>",
            Op::Lt => "<",
            Op::Gt => ">",
            Op::Le => "<=",
            Op::Ge => ">=",
            Op::LParen => "(",
            Op::RParen => ")",
            Op::Comma => ",",
            Op::Semi => ";",
            Op::Colon => ":",
            Op::Hash => "#",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Kw(Kw),
    /// 変数名 (大文字、型接尾辞 % ! # $ を含む)
    Ident(String),
    /// 数値 (値と入力された綴り)
    Num(Value, String),
    /// 文字列リテラル (引用符の中身)。閉じていなければ closed = false
    Str(String, bool),
    Op(Op),
    /// 空白 (LIST で再現するため保持する)
    Ws(String),
    /// REM / ' 以降の文字列 (apos: ' で書かれた)
    Rem(bool, String),
    /// DATA 以降、行末か (引用符外の) : までの生の文字列
    Data(String),
    /// 解釈できない文字
    Other(char),
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.'
}

/// 語全体がキーワードならそれを返す (TOTAL は TO + TAL ではなく変数)
fn match_keyword(rest: &str) -> Option<(Kw, usize)> {
    let mut best: Option<(Kw, usize)> = None;
    for (name, kw) in TABLE {
        let n = name.len();
        if rest.len() >= n && rest.is_char_boundary(n) && rest[..n].eq_ignore_ascii_case(name) {
            if best.is_none_or(|(_, m)| n > m) {
                best = Some((*kw, n));
            }
        }
    }
    best.filter(|&(_, n)| !rest[n..].starts_with(is_ident_char))
}

pub fn tokenize(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    let s = line;
    while i < s.len() {
        let rest = &s[i..];
        let c = rest.chars().next().unwrap();
        // 空白
        if c == ' ' || c == '\t' {
            let n = rest.chars().take_while(|&c| c == ' ' || c == '\t').count();
            out.push(Token::Ws(String::from(&rest[..n])));
            i += n;
            continue;
        }
        // 文字列
        if c == '"' {
            let body = &rest[1..];
            match body.find('"') {
                Some(e) => {
                    out.push(Token::Str(String::from(&body[..e]), true));
                    i += e + 2;
                }
                None => {
                    out.push(Token::Str(String::from(body), false));
                    i = s.len();
                }
            }
            continue;
        }
        // コメント
        if c == '\'' {
            out.push(Token::Rem(true, String::from(&rest[1..])));
            break;
        }
        // 数値 (&H を含む)
        if c.is_ascii_digit() || c == '.' || (c == '&' && rest.len() > 1 && rest.as_bytes()[1].eq_ignore_ascii_case(&b'H'))
        {
            if let Some((v, n)) = lex_number(rest) {
                out.push(Token::Num(v, rest[..n].to_ascii_uppercase()));
                i += n;
                continue;
            }
        }
        // キーワードと変数名
        if c.is_ascii_alphabetic() {
            if let Some((kw, n)) = match_keyword(rest) {
                i += n;
                out.push(Token::Kw(kw));
                if kw == Kw::Rem {
                    out.push(Token::Rem(false, String::from(&s[i..])));
                    break;
                }
                if kw == Kw::Data {
                    let d = &s[i..];
                    let mut q = false;
                    let end = d.char_indices().find(|&(_, c)| {
                        if c == '"' {
                            q = !q;
                        }
                        c == ':' && !q
                    });
                    let e = end.map_or(d.len(), |(e, _)| e);
                    out.push(Token::Data(String::from(&d[..e])));
                    i += e;
                }
                continue;
            }
            let n = rest.chars().take_while(|&c| is_ident_char(c)).count();
            let mut name = rest[..n].to_ascii_uppercase();
            let mut len = n;
            if let Some(suf @ ('%' | '!' | '#' | '$')) = rest[n..].chars().next() {
                name.push(suf);
                len += 1;
            }
            out.push(Token::Ident(name));
            i += len;
            continue;
        }
        // 演算子
        let two = rest.get(..2).unwrap_or("");
        let (op, n) = match two {
            "<>" | "><" => (Some(Op::Ne), 2),
            "<=" | "=<" => (Some(Op::Le), 2),
            ">=" | "=>" => (Some(Op::Ge), 2),
            _ => (
                match c {
                    '+' => Some(Op::Plus),
                    '-' => Some(Op::Minus),
                    '*' => Some(Op::Mul),
                    '/' => Some(Op::Div),
                    '\\' | '¥' => Some(Op::IntDiv),
                    '^' => Some(Op::Pow),
                    '=' => Some(Op::Eq),
                    '<' => Some(Op::Lt),
                    '>' => Some(Op::Gt),
                    '(' => Some(Op::LParen),
                    ')' => Some(Op::RParen),
                    ',' => Some(Op::Comma),
                    ';' => Some(Op::Semi),
                    ':' => Some(Op::Colon),
                    '#' => Some(Op::Hash),
                    _ => None,
                },
                c.len_utf8(),
            ),
        };
        match op {
            Some(op) => out.push(Token::Op(op)),
            None if c == '?' => out.push(Token::Kw(Kw::Print)),
            None => out.push(Token::Other(c)),
        }
        i += n;
    }
    out
}

/// トークン列を LIST 用の文字列にする
pub fn detokenize(toks: &[Token]) -> String {
    let mut s = String::new();
    for t in toks {
        match t {
            Token::Kw(k) => s.push_str(k.name()),
            Token::Ident(n) => s.push_str(n),
            Token::Num(_, text) => s.push_str(text),
            Token::Str(b, closed) => {
                s.push('"');
                s.push_str(b);
                if *closed {
                    s.push('"');
                }
            }
            Token::Op(o) => s.push_str(o.text()),
            Token::Ws(w) => s.push_str(w),
            Token::Rem(apos, r) => {
                if *apos {
                    s.push('\'');
                }
                s.push_str(r);
            }
            Token::Data(d) => s.push_str(d),
            Token::Other(c) => s.push(*c),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(s: &str) -> String {
        detokenize(&tokenize(s))
    }

    #[test]
    fn uppercases_keywords_and_names_only() {
        assert_eq!(round("print \"abc\";a"), "PRINT \"abc\";A");
        assert_eq!(round("rem hello World"), "REM hello World");
        assert_eq!(round("a=1 ' note"), "A=1 ' note");
        assert_eq!(round("data abc, \"x:y\" :print"), "DATA abc, \"x:y\" :PRINT");
        assert_eq!(round("? left$(a$,2)"), "PRINT LEFT$(A$,2)");
    }

    #[test]
    fn keyword_is_whole_word() {
        assert_eq!(tokenize("TOTAL")[0], Token::Ident("TOTAL".into()));
        assert_eq!(tokenize("FORI")[0], Token::Ident("FORI".into()));
        assert_eq!(tokenize("CHR$(65)")[0], Token::Kw(Kw::Chr));
    }

    #[test]
    fn longest_keyword_wins() {
        assert_eq!(tokenize("PRESET")[0], Token::Kw(Kw::Preset));
        assert_eq!(tokenize("STRING$(3,\"a\")")[0], Token::Kw(Kw::StringS));
        assert_eq!(tokenize("INPUT")[0], Token::Kw(Kw::Input));
    }

    #[test]
    fn operators() {
        let t = tokenize("a<>b>=c");
        assert_eq!(t[1], Token::Op(Op::Ne));
        assert_eq!(t[3], Token::Op(Op::Ge));
    }

    #[test]
    fn unicode_in_strings() {
        assert_eq!(round("print \"こんにちは\""), "PRINT \"こんにちは\"");
    }
}
