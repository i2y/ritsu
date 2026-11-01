//! A Wasm module that answers one function of the component model, `call: func(input: string) ->
//! string`, with a line of JSON in and a line of JSON out (DESIGN 9.2): how it is built, the world
//! its `.wit` declares, the three functions of the canonical ABI its crate exports, and the function
//! a JavaScript host calls them through. koyomi's module of a file (koyomi's DESIGN 13), rulec's
//! module of a rule (rulec's DESIGN §15.64, §15.278) and the Wasm of a package of Rust, which
//! kaname's modules answer in (kaname's DESIGN 36.11), are written with these.
//!
//! What a module reads and answers is its language's own: the function `call` hands the input to,
//! the reader of JSON, and the sentences of the refusals.

use ritsu_base::text::Lang;

/// The flags of rustc a module is built with: for size over speed, without unwinding or symbols,
/// for the target that imports nothing.
pub const RUSTC_FLAGS: &str = "-C opt-level=s -C lto -C panic=abort -C strip=symbols --target wasm32-unknown-unknown --crate-type cdylib";

/// The same build with cargo, of the library of the crate the line runs in. cargo passes rustc
/// `-C embed-bitcode=no`, which `-C lto` after `--` does not go with, so the profile's settings are
/// given with `--config`.
pub const CARGO_LINE: &str = "cargo rustc --lib --release --target wasm32-unknown-unknown --crate-type cdylib --config 'profile.release.opt-level=\"s\"' --config 'profile.release.lto=true' --config 'profile.release.panic=\"abort\"' --config 'profile.release.strip=\"symbols\"'";

/// The one function the module exports, as its world declares it.
pub const CALL_SIGNATURE: &str = "call: func(input: string) -> string";

/// The words WIT keeps for itself (the component model's WIT.md, as wasm-tools 1.245.1 reads it):
/// a world or a package named like one is written with `%` before it.
pub const WIT_KEYWORDS: &[&str] = &[
    "as", "async", "bool", "borrow", "char", "constructor", "enum", "error-context", "export", "f32", "f64", "flags", "from", "func", "future", "import",
    "include", "interface", "list", "map", "option", "own", "package", "record", "resource", "result", "s16", "s32", "s64", "s8", "static", "stream",
    "string", "tuple", "type", "u16", "u32", "u64", "u8", "use", "variant", "with", "world",
];

/// A WIT identifier from a name of ASCII letters, digits and `_` that starts with a letter: its
/// words, lowercase, joined by `-` (`payment_terms` gives `payment-terms`; a run of `_` is one
/// break, and one at either end none, since WIT puts a letter or a digit on each side of every `-`).
/// A word after the first may start with a digit (`close-20th`), as WIT allows. One that is a
/// keyword of WIT is written `%`-escaped.
pub fn world(name: &str) -> String {
    let id = name.split('_').filter(|w| !w.is_empty()).map(str::to_ascii_lowercase).collect::<Vec<_>>().join("-");
    if WIT_KEYWORDS.contains(&id.as_str()) { format!("%{id}") } else { id }
}

/// A version of one whole number as a semver: `1` gives `1.0.0`. A number past what a semver's
/// major number holds (more than 64 bits) gives none.
pub fn semver(v: &str) -> Option<String> {
    v.parse::<u64>().ok().map(|n| format!("{n}.0.0"))
}

/// The package a `.wit` declares, `<language>:<world>@<semver>` (`koyomi:payment-terms@1.0.0`), or
/// without the version when there is none.
pub fn package(language: &str, name: &str, version: Option<&str>) -> String {
    match version {
        Some(v) => format!("{language}:{}@{v}", world(name)),
        None => format!("{language}:{}", world(name)),
    }
}

/// The two lines that make a component of `<stem>.wasm` with `<stem>.wit`, run where both are:
/// `<stem>.component.wasm`.
pub fn component_line(stem: &str) -> String {
    format!("wasm-tools component embed {stem}.wit {stem}.wasm -o {stem}.embedded.wasm && wasm-tools component new {stem}.embedded.wasm -o {stem}.component.wasm")
}

/// The `use` the three functions of the ABI read, in the file that holds them.
pub const ALLOC_USE: &str = "use std::alloc::{Layout, alloc, realloc};\n";

/// The three functions of the canonical ABI, each with its doc comment in `lang`: [`REALLOC`],
/// [`call`] of `answer` and `not_utf8`, and [`POST_CALL`].
pub fn abi(lang: Lang, answer: &str, not_utf8: &str) -> String {
    let (realloc, called, freed) = match lang {
        Lang::Ja => (
            "canonical ABI のアロケーター。ホストは入力の文字列をここに置き、call が読み終えて解放する。",
            "`call: func(input: string) -> string`。入力は、このモジュールのメモリにある UTF-8 のポインターと長さ。返す値は、答えのポインターと長さの組を指す。その組は cabi_post_call が解放する。",
            "call が返した答えを解放する。",
        ),
        Lang::En => (
            "The allocator of the canonical ABI: the host puts the input string here, and call frees it once read.",
            "`call: func(input: string) -> string`: the input is the pointer and the length of UTF-8 in this module's memory, and what comes back points at the pointer and the length of the answer, which cabi_post_call frees.",
            "Frees the answer call gave.",
        ),
    };
    format!("/// {realloc}\n{REALLOC}/// {called}\n{}/// {freed}\n{POST_CALL}", call(answer, not_utf8))
}

/// The allocator of the canonical ABI, `cabi_realloc`: the host puts the input here, and `call` frees
/// it once read.
pub const REALLOC: &str = r#"#[unsafe(no_mangle)]
pub unsafe extern "C" fn cabi_realloc(old: *mut u8, old_size: usize, align: usize, new_size: usize) -> *mut u8 {
    if new_size == 0 {
        return align as *mut u8;
    }
    let p = unsafe {
        if old_size == 0 {
            alloc(Layout::from_size_align_unchecked(new_size, align))
        } else {
            realloc(old, Layout::from_size_align_unchecked(old_size, align), new_size)
        }
    };
    if p.is_null() {
        std::process::abort();
    }
    p
}

"#;

/// `call`: the input is the pointer and the length of UTF-8 in the module's memory, and what comes
/// back points at the pointer and the length of the answer, which `cabi_post_call` frees. `answer`
/// is the expression of the answer to `text: &str`, a `String`, and `not_utf8` that of the answer to
/// bytes that are not UTF-8: the module answers whatever bytes it is given, an empty input among
/// them, and never reads them as a string without looking.
pub fn call(answer: &str, not_utf8: &str) -> String {
    format!(
        r#"#[unsafe(no_mangle)]
pub unsafe extern "C" fn call(ptr: *mut u8, len: usize) -> *mut [u32; 2] {{
    let bytes = if len == 0 {{ Vec::new() }} else {{ unsafe {{ Vec::from_raw_parts(ptr, len, len) }} }};
    let out = match std::str::from_utf8(&bytes) {{
        Ok(text) => {answer},
        Err(_) => {not_utf8},
    }};
    drop(bytes);
    let s = out.into_boxed_str();
    let n = s.len();
    let p = Box::into_raw(s) as *mut u8;
    Box::into_raw(Box::new([p as u32, n as u32]))
}}

"#
    )
}

/// `cabi_post_call`, which frees what `call` answered.
pub const POST_CALL: &str = r#"#[unsafe(no_mangle)]
pub unsafe extern "C" fn cabi_post_call(ret: *mut [u32; 2]) {
    let r = unsafe { Box::from_raw(ret) };
    let (p, n) = (r[0] as *mut u8, r[1] as usize);
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, n) as *mut str) });
}

"#;

/// The function a JavaScript host calls `call` through, with the module's exports in `ex`, a
/// `TextEncoder` in `enc` and a `TextDecoder` in `dec`: the views of the memory are made after the
/// call, since the memory may grow during it.
pub const NODE_CALL: &str = r#"function call(text) {
  const bytes = enc.encode(text);
  const ptr = ex.cabi_realloc(0, 0, 1, bytes.length);
  new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes);
  const ret = ex.call(ptr, bytes.length);
  const [p, n] = new Uint32Array(ex.memory.buffer, ret, 2);
  const out = dec.decode(new Uint8Array(ex.memory.buffer, p, n));
  ex.cabi_post_call(ret);
  return out;
}
"#;
