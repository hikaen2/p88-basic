//! 画面とグラフィックの命令: CLS LOCATE COLOR WIDTH PSET PRESET LINE CIRCLE PAINT BEEP

use super::error::{Error, Result};
use super::interp::Flow;
use super::token::{Kw, Op, Token};
use super::value::Value;
use crate::machine::{Machine, Platform};
use crate::screen::DEFAULT_PALETTE;
use crate::text::COLS;

/// 色番号 (0〜15)
fn color(v: &Value) -> Result<u8> {
    let c = v.int()?;
    if (0..16).contains(&c) { Ok(c as u8) } else { Err(Error::IllegalFunctionCall) }
}

impl<P: Platform> Machine<P> {
    pub(crate) fn screen_statement(&mut self, kw: Kw) -> Result<Flow> {
        match kw {
            Kw::Cls => self.st_cls()?,
            Kw::Locate => self.st_locate()?,
            Kw::Color => self.st_color()?,
            Kw::Width => self.st_width()?,
            Kw::Pset | Kw::Preset => {
                let p = self.point_arg()?;
                let c = if self.accept_op(Op::Comma) {
                    color(&self.expr()?)?
                } else if kw == Kw::Pset {
                    7
                } else {
                    0
                };
                self.gfx.pset(p.0, p.1, c);
                self.gfx.last = p;
            }
            Kw::Line => self.st_line()?,
            Kw::Circle => {
                let center = self.point_arg()?;
                self.expect_op(Op::Comma)?;
                let r = self.expr()?.int()?;
                if r < 0 {
                    return Err(Error::IllegalFunctionCall);
                }
                let c = self.opt_color(7)?;
                self.gfx.circle(center, r as i32, c);
                self.gfx.last = center;
            }
            Kw::Paint => {
                let p = self.point_arg()?;
                let c = self.opt_color(7)?;
                let b = self.opt_color(c)?;
                self.gfx.paint(p, c, b);
                self.gfx.last = p;
            }
            Kw::Beep => self.platform.beep(),
            Kw::Save | Kw::Load | Kw::Files | Kw::Kill => return self.file_statement(kw),
            _ => return Err(Error::Syntax),
        }
        Ok(Flow::Next)
    }

    pub(crate) fn screen_function(&mut self, kw: Kw, a: &[Value]) -> Result<Value> {
        match (kw, a) {
            (Kw::Point, [x, y]) => Ok(Value::Int(self.gfx.point(x.int()? as i32, y.int()? as i32) as i16)),
            _ => Err(Error::Syntax),
        }
    }

    /// (x, y)
    fn point_arg(&mut self) -> Result<(i32, i32)> {
        self.expect_op(Op::LParen)?;
        let x = self.expr()?.int()? as i32;
        self.expect_op(Op::Comma)?;
        let y = self.expr()?.int()? as i32;
        self.expect_op(Op::RParen)?;
        Ok((x, y))
    }

    /// 省略できる ", 色"。省略時は def。
    fn opt_color(&mut self, def: u8) -> Result<u8> {
        if !self.accept_op(Op::Comma) || self.at_end() || self.is_comma() {
            return Ok(def);
        }
        color(&self.expr()?)
    }

    fn is_comma(&mut self) -> bool {
        matches!(self.peek(), Some(Token::Op(Op::Comma)))
    }

    fn st_cls(&mut self) -> Result<()> {
        let n = if self.at_end() { 1 } else { self.expr()?.int()? };
        match n {
            1 => self.text.clear(),
            2 => self.gfx.clear(),
            3 => {
                self.text.clear();
                self.gfx.clear();
            }
            _ => return Err(Error::IllegalFunctionCall),
        }
        Ok(())
    }

    fn st_locate(&mut self) -> Result<()> {
        let (mut x, mut y) = (self.text.x.min(COLS - 1) as i16, self.text.y as i16);
        if !self.is_comma() {
            x = self.expr()?.int()?;
        }
        if self.accept_op(Op::Comma) {
            y = self.expr()?.int()?;
        }
        if !(0..COLS as i16).contains(&x) || !(0..self.text.rows() as i16).contains(&y) {
            return Err(Error::IllegalFunctionCall);
        }
        self.text.locate(x as usize, y as usize);
        Ok(())
    }

    fn st_color(&mut self) -> Result<()> {
        if self.accept_op(Op::Eq) {
            // COLOR=(パレット番号, &HRGB) / COLOR=NEW
            if self.accept_kw(Kw::New) {
                self.palette.0 = DEFAULT_PALETTE;
                return Ok(());
            }
            self.expect_op(Op::LParen)?;
            let p = color(&self.expr()?)?;
            self.expect_op(Op::Comma)?;
            let c = self.expr()?.int()?;
            self.expect_op(Op::RParen)?;
            if !(0..=0xFFF).contains(&c) {
                return Err(Error::IllegalFunctionCall);
            }
            self.palette.0[p as usize] = c as u16;
            return Ok(());
        }
        self.text.color = color(&self.expr()?)?;
        Ok(())
    }

    fn st_width(&mut self) -> Result<()> {
        let w = self.expr()?.int()?;
        let rows = if self.accept_op(Op::Comma) { self.expr()?.int()? } else { self.text.rows() as i16 };
        match (w, rows) {
            (80, 25) | (80, 20) => {
                self.text.set_rows(rows as usize);
                Ok(())
            }
            _ => Err(Error::IllegalFunctionCall),
        }
    }

    /// LINE [(x1,y1)]-(x2,y2) [,[色][,B|BF]]
    fn st_line(&mut self) -> Result<()> {
        let from = if self.is_op_minus() { self.gfx.last } else { self.point_arg()? };
        self.expect_op(Op::Minus)?;
        let to = self.point_arg()?;
        let c = self.opt_color(7)?;
        let mode = if self.accept_op(Op::Comma) {
            match self.peek() {
                Some(Token::Ident(m)) if m == "B" || m == "BF" => {
                    self.b.pos += 1;
                    m
                }
                _ => return Err(Error::Syntax),
            }
        } else {
            alloc::string::String::new()
        };
        match mode.as_str() {
            "B" => self.gfx.draw_box(from, to, c),
            "BF" => self.gfx.fill_box(from, to, c),
            _ => self.gfx.line(from, to, c),
        }
        self.gfx.last = to;
        Ok(())
    }

    fn is_op_minus(&mut self) -> bool {
        matches!(self.peek(), Some(Token::Op(Op::Minus)))
    }
}
