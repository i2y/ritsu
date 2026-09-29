//! The playground's side of the boundary, for `wasm32-unknown-unknown` with no bindgen and no
//! JavaScript toolchain: eight exported functions and one convention, the one rulec's playground
//! keeps. **Every buffer that crosses the boundary begins with its own length, as a little-endian
//! `u32`.** The page allocates a buffer, writes its request into it, calls, reads the answer out
//! of the module's memory and frees both.
//!
//! The page hands over the bundle once (`dandori_bundle`), and the module keeps it: it is the
//! same for every request, and reading it again at each keystroke would be most of the work.
//! Everything else is in crate::playground, which the tests run natively too.

use crate::sources::Bundle;
use std::alloc::{alloc, dealloc, Layout};
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static BUNDLE: RefCell<Option<Rc<Bundle>>> = const { RefCell::new(None) };
}

/// Four bytes of length, then the bytes. `align` is 4 for the header's sake.
fn layout(total: usize) -> Layout {
    Layout::from_size_align(total, 4).expect("a buffer this large cannot be asked for")
}

/// Allocate `len + 4` bytes and write `len` into the first four. The page writes its bytes at
/// `ptr + 4`, and hands the pointer to [`dandori_free`] when it is done with it.
#[no_mangle]
pub extern "C" fn dandori_alloc(len: usize) -> *mut u8 {
    let p = unsafe { alloc(layout(len + 4)) };
    assert!(!p.is_null(), "out of memory");
    unsafe { std::ptr::copy_nonoverlapping((len as u32).to_le_bytes().as_ptr(), p, 4) };
    p
}

/// Release a buffer from either side. The length in the header says how much to release.
///
/// # Safety
/// `ptr` is null, or a buffer from [`dandori_alloc`] or an answer of this module, not yet freed.
#[no_mangle]
pub unsafe extern "C" fn dandori_free(ptr: *mut u8) {
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
/// `ptr` is a buffer from [`dandori_alloc`], with the page's bytes written after its header.
unsafe fn input<'a>(ptr: *const u8) -> &'a str {
    let len = unsafe { header(ptr) };
    let bytes = unsafe { std::slice::from_raw_parts(ptr.add(4), len) };
    std::str::from_utf8(bytes).unwrap_or("")
}

/// A string as a buffer the page reads and then frees.
fn out(s: &str) -> *mut u8 {
    let b = s.as_bytes();
    let p = dandori_alloc(b.len());
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), p.add(4), b.len()) };
    p
}

/// The version of dandori this module was built from. The page shows it, so that a stale
/// `dandori.wasm` is seen rather than answering the old way.
#[no_mangle]
pub extern "C" fn dandori_version() -> *mut u8 {
    out(env!("CARGO_PKG_VERSION"))
}

/// Keep the bundle the other files are read from (`presets.json`). Answers nothing when it is
/// kept, and why not when it cannot be.
///
/// # Safety
/// `ptr` is a buffer from [`dandori_alloc`], with the bundle written after its header.
#[no_mangle]
pub unsafe extern "C" fn dandori_bundle(ptr: *const u8) -> *mut u8 {
    let read = serde_json::from_str(unsafe { input(ptr) }).map_err(|e| e.to_string()).and_then(|v| Bundle::from_json(&v));
    match read {
        Ok(b) => {
            BUNDLE.with(|k| *k.borrow_mut() = Some(Rc::new(b)));
            out("")
        }
        Err(e) => out(&e),
    }
}

/// # Safety
/// `ptr` is a buffer from [`dandori_alloc`], with a request written after its header.
unsafe fn answer(what: &str, ptr: *const u8) -> *mut u8 {
    match BUNDLE.with(|k| k.borrow().clone()) {
        Some(b) => out(&crate::playground::answer(&b, what, unsafe { input(ptr) })),
        None => out(r#"{"error":"the bundle has not been handed over"}"#),
    }
}

/// `dandori check`: crate::playground::check.
///
/// # Safety
/// `ptr` is a buffer from [`dandori_alloc`], with a request written after its header.
#[no_mangle]
pub unsafe extern "C" fn dandori_check(ptr: *const u8) -> *mut u8 {
    unsafe { answer("check", ptr) }
}

/// `dandori build`: crate::playground::build.
///
/// # Safety
/// `ptr` is a buffer from [`dandori_alloc`], with a request written after its header.
#[no_mangle]
pub unsafe extern "C" fn dandori_build(ptr: *const u8) -> *mut u8 {
    unsafe { answer("build", ptr) }
}

/// `dandori doc`: crate::playground::doc.
///
/// # Safety
/// `ptr` is a buffer from [`dandori_alloc`], with a request written after its header.
#[no_mangle]
pub unsafe extern "C" fn dandori_doc(ptr: *const u8) -> *mut u8 {
    unsafe { answer("doc", ptr) }
}

/// The rules the flow calls: crate::playground::rules.
///
/// # Safety
/// `ptr` is a buffer from [`dandori_alloc`], with a request written after its header.
#[no_mangle]
pub unsafe extern "C" fn dandori_rules(ptr: *const u8) -> *mut u8 {
    unsafe { answer("rules", ptr) }
}
