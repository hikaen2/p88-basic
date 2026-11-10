//! 値と数値の書式

use super::error::{Error, Result};
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

/// 文字列は BMP コードポイント (u16) の列
pub type Str = Rc<[u16]>;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i16),
    Sng(f32),
    Str(Str),
}

pub fn to_str(s: &str) -> Str {
    s.chars().map(|c| if (c as u32) <= 0xFFFF { c as u32 as u16 } else { crate::font::REPLACEMENT }).collect()
}

pub fn from_str(s: &[u16]) -> String {
    s.iter().map(|&c| char::from_u32(c as u32).unwrap_or('\u{25A1}')).collect()
}

impl Value {
    pub fn num(&self) -> Result<f32> {
        match self {
            Value::Int(i) => Ok(*i as f32),
            Value::Sng(f) => Ok(*f),
            Value::Str(_) => Err(Error::TypeMismatch),
        }
    }

    pub fn str(&self) -> Result<Str> {
        match self {
            Value::Str(s) => Ok(s.clone()),
            _ => Err(Error::TypeMismatch),
        }
    }

    /// 整数へ丸める (四捨五入、-32768〜32767)
    pub fn int(&self) -> Result<i16> {
        match self {
            Value::Int(i) => Ok(*i),
            Value::Sng(f) => round_i16(*f),
            Value::Str(_) => Err(Error::TypeMismatch),
        }
    }

    pub fn is_str(&self) -> bool {
        matches!(self, Value::Str(_))
    }
}

pub fn round_i16(f: f32) -> Result<i16> {
    let r = libm::floorf(f + 0.5);
    if (-32768.0..=32767.0).contains(&r) { Ok(r as i16) } else { Err(Error::Overflow) }
}

/// 整数の結果: i16 に収まれば Int、収まらなければ Sng
pub fn int_or_sng(v: i32) -> Value {
    if (-32768..=32767).contains(&v) { Value::Int(v as i16) } else { Value::Sng(v as f32) }
}

/// 数値を PRINT と同じ形で文字列にする (正なら先頭に空白、末尾の空白なし)
pub fn format_num(v: &Value) -> String {
    match v {
        Value::Int(i) => {
            if *i >= 0 {
                alloc::format!(" {i}")
            } else {
                alloc::format!("{i}")
            }
        }
        Value::Sng(f) => format_sng(*f),
        Value::Str(_) => String::new(),
    }
}

/// 単精度: 有効数字 7 桁。整数部が 0 なら先頭の 0 を省く (例: .5)。
fn format_sng(f: f32) -> String {
    let sign = if f < 0.0 { "-" } else { " " };
    let a = libm::fabsf(f);
    if a == 0.0 {
        return String::from(" 0");
    }
    // "d.dddddde±x" から仮数の数字列と指数を取り出す
    let e_str = alloc::format!("{a:.6e}");
    let (mant, exp) = e_str.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let body = if (-2..7).contains(&exp) {
        let point = exp + 1; // 小数点の位置 (数字列の先頭から)
        if point <= 0 {
            alloc::format!(".{}{}", "0".repeat((-point) as usize), digits)
        } else if point as usize >= digits.len() {
            alloc::format!("{}{}", digits, "0".repeat(point as usize - digits.len()))
        } else {
            alloc::format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
        }
    } else {
        let frac = &digits[1..];
        let m = if frac.is_empty() { String::from(&digits[..1]) } else { alloc::format!("{}.{}", &digits[..1], frac) };
        alloc::format!("{m}E{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
    };
    alloc::format!("{sign}{body}")
}

/// 文字列の先頭から数値を読む (VAL / INPUT / DATA 用)。読めた文字数も返す。
/// 先頭の空白は飛ばす。数字がなければ 0。
pub fn parse_num(s: &[u16]) -> (Value, usize) {
    let mut i = 0;
    while i < s.len() && s[i] == b' ' as u16 {
        i += 1;
    }
    let text: Vec<u8> = s[i..].iter().take_while(|&&c| c < 0x80).map(|&c| c as u8).collect();
    let text = core::str::from_utf8(&text).unwrap_or("");
    let (v, n) = lex_number(text).unwrap_or((Value::Int(0), 0));
    (v, i + n)
}

/// 数値リテラルを読む。&H 形式、小数点、指数 (E)、型接尾辞 (% ! #) に対応。
/// 戻り値は (値, 読んだバイト数)。数値で始まらなければ None。
pub fn lex_number(s: &str) -> Option<(Value, usize)> {
    let b = s.as_bytes();
    let (neg, start) = match b.first() {
        Some(b'-') => (true, 1),
        Some(b'+') => (false, 1),
        _ => (false, 0),
    };
    let r = lex_unsigned(&s[start..])?;
    let v = if neg {
        match r.0 {
            Value::Int(i) => int_or_sng(-(i as i32)),
            Value::Sng(f) => Value::Sng(-f),
            v => v,
        }
    } else {
        r.0
    };
    Some((v, start + r.1))
}

fn lex_unsigned(s: &str) -> Option<(Value, usize)> {
    let b = s.as_bytes();
    if b.len() >= 2 && b[0] == b'&' && (b[1] == b'H' || b[1] == b'h') {
        let n = b[2..].iter().take_while(|c| c.is_ascii_hexdigit()).count();
        if n == 0 {
            return None;
        }
        let v = u32::from_str_radix(&s[2..2 + n], 16).ok()?;
        if v > 0xFFFF {
            return Some((Value::Sng(v as f32), 2 + n));
        }
        return Some((Value::Int(v as u16 as i16), 2 + n));
    }
    let mut i = 0;
    let mut is_int = true;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i < b.len() && b[i] == b'.' {
        is_int = false;
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i == 0 || (i == 1 && b[0] == b'.') {
        return None;
    }
    let mant_end = i;
    if i < b.len() && (b[i] == b'E' || b[i] == b'e' || b[i] == b'D' || b[i] == b'd') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let k = b[j..].iter().take_while(|c| c.is_ascii_digit()).count();
        if k > 0 {
            is_int = false;
            i = j + k;
        }
    }
    let mut text = String::from(&s[..i]);
    if mant_end < i {
        // D 指数は E に
        text = text.replace(['D', 'd'], "E");
    }
    let mut force_sng = false;
    if i < b.len() && matches!(b[i], b'!' | b'#') {
        force_sng = true;
        i += 1;
    } else if i < b.len() && b[i] == b'%' {
        let f: f32 = text.parse().ok()?;
        return Some((Value::Int(round_i16(f).ok()?), i + 1));
    }
    let f: f32 = text.parse().ok()?;
    if is_int && !force_sng && f <= 32767.0 {
        Some((Value::Int(f as i16), i))
    } else {
        Some((Value::Sng(f), i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(x: f32) -> String {
        format_num(&Value::Sng(x))
    }

    #[test]
    fn formats() {
        assert_eq!(format_num(&Value::Int(3)), " 3");
        assert_eq!(format_num(&Value::Int(-3)), "-3");
        assert_eq!(f(2.5), " 2.5");
        assert_eq!(f(-2.5), "-2.5");
        assert_eq!(f(1.0 / 3.0), " .3333333");
        assert_eq!(f(40000.0), " 40000");
        assert_eq!(f(0.01), " .01");
        assert_eq!(f(0.001), " 1E-03");
        assert_eq!(f(12345678.0), " 1.234568E+07");
        assert_eq!(f(0.0), " 0");
    }

    #[test]
    fn lexes() {
        assert_eq!(lex_number("12AB"), Some((Value::Int(12), 2)));
        assert_eq!(lex_number("40000"), Some((Value::Sng(40000.0), 5)));
        assert_eq!(lex_number("1.5E2"), Some((Value::Sng(150.0), 5)));
        assert_eq!(lex_number("&H3042"), Some((Value::Int(0x3042), 6)));
        assert_eq!(lex_number("&HFFFF"), Some((Value::Int(-1), 6)));
        assert_eq!(lex_number("7.6%"), Some((Value::Int(8), 4)));
        assert_eq!(lex_number("X"), None);
    }
}
