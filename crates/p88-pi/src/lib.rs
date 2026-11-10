//! Pi 用: Circle シム (pi/shim) から呼ばれる staticlib
#![no_std]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};

mod ffi {
    unsafe extern "C" {
        pub fn p88_present(buf: *const u32);
        pub fn p88_millis() -> u64;
        pub fn p88_malloc(size: usize, align: usize) -> *mut u8;
        pub fn p88_free(p: *mut u8);
        pub fn p88_log(msg: *const u8, len: usize);
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
    let mut buf = alloc::string::String::new();
    let _ = write!(buf, "panic: {info}");
    log(&buf);
    loop {}
}

const W: usize = 640;
const H: usize = 400;

/// シムの初期化後に呼ばれる。戻らない。
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    // スパイク: 16 色の縦帯 (既定パレット)
    const PAL: [u32; 16] = [
        0x000000, 0x0000FF, 0xFF0000, 0xFF00FF, 0x00FF00, 0x00FFFF, 0xFFFF00, 0xFFFFFF,
        0x777777, 0x000077, 0x770000, 0x770077, 0x007700, 0x007777, 0x777700, 0x777777,
    ];
    let mut fb = alloc::vec![0u32; W * H];
    for y in 0..H {
        for x in 0..W {
            fb[y * W + x] = PAL[x * 16 / W];
        }
    }
    let t0 = unsafe { ffi::p88_millis() };
    unsafe { ffi::p88_present(fb.as_ptr()) };
    let f = 1.5f32 * (t0 as f32 + 2.0); // hard-float 演算のリンク確認
    let mut s = alloc::string::String::new();
    use core::fmt::Write;
    let _ = write!(s, "rust_main: stripes drawn, f={f}");
    log(&s);
    loop {}
}
