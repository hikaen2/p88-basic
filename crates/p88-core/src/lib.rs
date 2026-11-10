//! I/O 非依存の BASIC マシン本体 (no_std + alloc)
#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod basic;
pub mod editor;
pub mod font;
pub mod key;
pub mod machine;
pub mod gfx;
pub mod hid;
pub mod screen;
pub mod text;
