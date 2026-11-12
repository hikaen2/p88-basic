//! Pi 用: Circle シム (pi/shim) から呼ばれる staticlib。Machine を動かすメインループ。
#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::ffi::{c_char, c_void};
use alloc::collections::VecDeque;
use p88_core::gfx::{HEIGHT, WIDTH};
use p88_core::hid::{HidKeyboard, Layout as KeyLayout};
use p88_core::key::Key;
use p88_core::machine::{FileError, Machine, Platform};

mod ffi {
    use core::ffi::{c_char, c_void};
    unsafe extern "C" {
        pub fn p88_present(buf: *const u32);
        pub fn p88_millis() -> u64;
        pub fn p88_update();
        pub fn p88_kbd_poll(mods: *mut u8, keys: *mut u8) -> i32;
        pub fn p88_keymap_us() -> i32;
        pub fn p88_malloc(size: usize, align: usize) -> *mut u8;
        pub fn p88_free(p: *mut u8);
        pub fn p88_log(msg: *const u8, len: usize);
        pub fn p88_beep();
        pub fn p88_file_read(name: *const c_char, data: *mut *mut u8, len: *mut usize) -> i32;
        pub fn p88_file_write(name: *const c_char, data: *const u8, len: usize) -> i32;
        pub fn p88_file_list(cb: extern "C" fn(*mut c_void, *const c_char, u64), ctx: *mut c_void) -> i32;
        pub fn p88_file_delete(name: *const c_char) -> i32;
    }
}

struct CircleAlloc;

unsafe impl GlobalAlloc for CircleAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { ffi::p88_malloc(layout.size(), layout.align()) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { ffi::p88_free(ptr) }
    }
}

#[global_allocator]
static ALLOC: CircleAlloc = CircleAlloc;

fn log(s: &str) {
    unsafe { ffi::p88_log(s.as_ptr(), s.len()) }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    let mut buf = String::new();
    let _ = write!(buf, "panic: {info}");
    log(&buf);
    loop {}
}

fn result(r: i32) -> Result<(), FileError> {
    match r {
        0 => Ok(()),
        1 => Err(FileError::NotFound),
        _ => Err(FileError::Io),
    }
}

/// NUL 終端の C 文字列にする
fn c_name(name: &str) -> Vec<u8> {
    let mut v: Vec<u8> = name.bytes().filter(|&b| b != 0).collect();
    v.push(0);
    v
}

struct Pi {
    kbd: HidKeyboard,
    keys: VecDeque<Key>,
}

impl Pi {
    /// キーボードの報告を読んでキー列にする (リピートを含む)
    fn read_keyboard(&mut self) {
        let now = self.millis();
        let mut out = Vec::new();
        let (mut mods, mut keys) = (0u8, [0u8; 6]);
        while unsafe { ffi::p88_kbd_poll(&mut mods, keys.as_mut_ptr()) } != 0 {
            self.kbd.report(mods, keys, now, &mut out);
        }
        self.kbd.tick(now, &mut out);
        self.keys.extend(out);
    }
}

impl Platform for Pi {
    fn poll_key(&mut self) -> Option<Key> {
        if self.keys.is_empty() {
            self.read_keyboard();
        }
        self.keys.pop_front()
    }

    fn millis(&self) -> u64 {
        unsafe { ffi::p88_millis() }
    }

    fn beep(&mut self) {
        unsafe { ffi::p88_beep() }
    }

    fn log(&mut self, msg: &str) {
        log(msg);
    }

    fn read_file(&mut self, name: &str) -> Result<Vec<u8>, FileError> {
        let n = c_name(name);
        let (mut p, mut len) = (core::ptr::null_mut(), 0usize);
        result(unsafe { ffi::p88_file_read(n.as_ptr() as *const c_char, &mut p, &mut len) })?;
        let v = unsafe { core::slice::from_raw_parts(p, len) }.to_vec();
        unsafe { ffi::p88_free(p) };
        Ok(v)
    }

    fn write_file(&mut self, name: &str, data: &[u8]) -> Result<(), FileError> {
        let n = c_name(name);
        result(unsafe { ffi::p88_file_write(n.as_ptr() as *const c_char, data.as_ptr(), data.len()) })
    }

    fn list_files(&mut self) -> Result<Vec<(String, u64)>, FileError> {
        extern "C" fn cb(ctx: *mut c_void, name: *const c_char, size: u64) {
            let v = unsafe { &mut *(ctx as *mut Vec<(String, u64)>) };
            let s = unsafe { core::ffi::CStr::from_ptr(name) };
            v.push((String::from(s.to_string_lossy().as_ref()).to_uppercase(), size));
        }
        let mut v: Vec<(String, u64)> = Vec::new();
        result(unsafe { ffi::p88_file_list(cb, &mut v as *mut _ as *mut c_void) })?;
        Ok(v)
    }

    fn delete_file(&mut self, name: &str) -> Result<(), FileError> {
        let n = c_name(name);
        result(unsafe { ffi::p88_file_delete(n.as_ptr() as *const c_char) })
    }
}

/// 画面の書き換え間隔 (ミリ秒)
const FRAME_MS: u64 = 33;

/// シムの初期化後に呼ばれる。戻らない。
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    log("rust_main: starting machine");
    let layout = if unsafe { ffi::p88_keymap_us() } != 0 { KeyLayout::Us } else { KeyLayout::Jis };
    let mut m = Machine::new(Pi { kbd: HidKeyboard::new(layout), keys: VecDeque::new() });
    let mut buf = alloc::vec![0u32; WIDTH * HEIGHT];
    let mut last = 0u64;
    loop {
        unsafe { ffi::p88_update() };
        m.step();
        let now = m.platform.millis();
        if now.wrapping_sub(last) >= FRAME_MS {
            last = now;
            m.render(&mut buf);
            unsafe { ffi::p88_present(buf.as_ptr()) };
        }
    }
}
