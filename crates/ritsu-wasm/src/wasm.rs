//! The page's side of the boundary, for `wasm32-unknown-unknown` with no bindgen and no JavaScript
//! toolchain: six exported functions and the convention rulec's and dandori's playgrounds kept.
//! **Every buffer that crosses the boundary begins with its own length, as a little-endian `u32`.**
//! The page allocates a buffer, writes its request into it, calls, reads the answer out of the
//! module's memory and frees both. Everything else is in crate::playground, which the tests run
//! natively.
//!
//! The module keeps nothing between calls: every request carries the whole project, which is a few
//! files the reader is editing.

use std::alloc::{Layout, alloc, dealloc};

/// Four bytes of length, then the bytes. `align` is 4 for the header's sake.
fn layout(total: usize) -> Layout {
    Layout::from_size_align(total, 4).expect("a buffer this large cannot be asked for")
}

/// Allocate `len + 4` bytes and write `len` into the first four. The page writes its bytes at
/// `ptr + 4`, and hands the pointer to [`ritsu_free`] when it is done with it.
#[unsafe(no_mangle)]
pub extern "C" fn ritsu_alloc(len: usize) -> *mut u8 {
    let p = unsafe { alloc(layout(len + 4)) };
    assert!(!p.is_null(), "out of memory");
    unsafe { std::ptr::copy_nonoverlapping((len as u32).to_le_bytes().as_ptr(), p, 4) };
    p
}

/// Release a buffer from either side. The length in the header says how much to release.
///
/// # Safety
/// `ptr` is null, or a buffer from [`ritsu_alloc`] or an answer of this module, not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ritsu_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let len = unsafe { header(ptr) };
    unsafe { dealloc(ptr, layout(len + 4)) };
}

/// # Safety
/// `ptr` points at four readable bytes.
unsafe fn header(ptr: *const u8) -> usize {
    let mut b = [0u8; 4];
    unsafe { std::ptr::copy_nonoverlapping(ptr, b.as_mut_ptr(), 4) };
    u32::from_le_bytes(b) as usize
}

/// The buffer as text. Bytes that are not UTF-8 read as nothing rather than a panic, which would
/// take the module down with it.
///
/// # Safety
/// `ptr` is a buffer from [`ritsu_alloc`], with the page's bytes written after its header.
unsafe fn input<'a>(ptr: *const u8) -> &'a str {
    let len = unsafe { header(ptr) };
    let bytes = unsafe { std::slice::from_raw_parts(ptr.add(4), len) };
    std::str::from_utf8(bytes).unwrap_or("")
}

/// A string as a buffer the page reads and then frees.
fn out(s: &str) -> *mut u8 {
    let b = s.as_bytes();
    let p = ritsu_alloc(b.len());
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), p.add(4), b.len()) };
    p
}

/// The version of ritsu this module was built from. The page shows it, so that a stale
/// `ritsu.wasm` is seen rather than answering the old way.
#[unsafe(no_mangle)]
pub extern "C" fn ritsu_version() -> *mut u8 {
    out(env!("CARGO_PKG_VERSION"))
}

/// `ritsu check .` on the project: crate::playground::check.
///
/// # Safety
/// `ptr` is a buffer from [`ritsu_alloc`], with a request written after its header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ritsu_check(ptr: *const u8) -> *mut u8 {
    out(&crate::playground::answer("check", unsafe { input(ptr) }))
}

/// One file's generator: crate::playground::generate.
///
/// # Safety
/// `ptr` is a buffer from [`ritsu_alloc`], with a request written after its header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ritsu_gen(ptr: *const u8) -> *mut u8 {
    out(&crate::playground::answer("gen", unsafe { input(ptr) }))
}

/// One file's page: crate::playground::doc.
///
/// # Safety
/// `ptr` is a buffer from [`ritsu_alloc`], with a request written after its header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ritsu_doc(ptr: *const u8) -> *mut u8 {
    out(&crate::playground::answer("doc", unsafe { input(ptr) }))
}
