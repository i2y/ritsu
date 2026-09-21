//! The library as a web page (§15.48).
//!
//! `wasm32-unknown-unknown`, no bindgen and no JavaScript toolchain: six exported
//! functions and one convention — **every buffer that crosses the boundary begins with its
//! own length, as a little-endian `u32`**. The page allocates a buffer, writes the source
//! into it, calls, reads the answer out of the module's memory and frees both. Nothing else
//! is shared, so there is no glue file to keep in step with this one.
//!
//! What runs here is what the binary runs: [`crate::report_with`] for the findings,
//! [`crate::findings_text`] for the frame around them, [`crate::backend::ALL`] for the
//! files `gen` writes, and [`crate::doc::render_html`] for the page an approver reads. A
//! second implementation of any of those would be a second set of answers to keep true
//! (§15.38).
//!
//! The module is stateless. The language is passed on every call rather than set once,
//! because the page has a switch and a reload would be the alternative.

use crate::backend;
use crate::codegen::Gen;
use crate::diag::Severity;
use crate::i18n::{self, Lang};
use crate::json::{Obj, arr};
use std::alloc::{Layout, alloc, dealloc};

/// The name the findings give what the reader typed. It is a file name and not a
/// placeholder, because it appears in the frame exactly where a path appears in a terminal.
const PATH: &str = "playground.rule";

/// Four bytes of length, then the bytes. `align` is 4 for the header's sake.
fn layout(total: usize) -> Layout {
    Layout::from_size_align(total, 4).expect("a buffer this large cannot be asked for")
}

/// Allocate `len + 4` bytes and write `len` into the first four. The page writes its bytes
/// at `ptr + 4`.
///
/// # Safety
/// The pointer is owned by the caller until it is handed to [`rulec_free`].
#[unsafe(no_mangle)]
pub extern "C" fn rulec_alloc(len: usize) -> *mut u8 {
    let p = unsafe { alloc(layout(len + 4)) };
    assert!(!p.is_null(), "out of memory");
    unsafe { std::ptr::copy_nonoverlapping((len as u32).to_le_bytes().as_ptr(), p, 4) };
    p
}

/// Release a buffer from either side. The length in the header says how much to release,
/// which is why the header exists at all.
#[unsafe(no_mangle)]
pub extern "C" fn rulec_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let len = header(ptr);
    unsafe { dealloc(ptr, layout(len + 4)) };
}

fn header(ptr: *const u8) -> usize {
    let mut b = [0u8; 4];
    unsafe { std::ptr::copy_nonoverlapping(ptr, b.as_mut_ptr(), 4) };
    u32::from_le_bytes(b) as usize
}

/// The buffer as text. Invalid UTF-8 is read as an empty rule rather than a panic: the page
/// hands over whatever the textarea holds, and a trap would take the module down with it.
fn input<'a>(ptr: *const u8) -> &'a str {
    let len = header(ptr);
    let bytes = unsafe { std::slice::from_raw_parts(ptr.add(4), len) };
    std::str::from_utf8(bytes).unwrap_or("")
}

/// A string as a buffer the page can read and then free.
fn out(s: &str) -> *mut u8 {
    let b = s.as_bytes();
    let p = rulec_alloc(b.len());
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), p.add(4), b.len()) };
    p
}

fn lang(ja: u32) {
    i18n::set(if ja == 1 { Lang::Ja } else { Lang::En });
}

/// The version of the tool this module was built from. The page prints it, so a stale
/// `rulec.wasm` on the site is visible rather than silently answering an old way.
#[unsafe(no_mangle)]
pub extern "C" fn rulec_version() -> *mut u8 {
    out(env!("CARGO_PKG_VERSION"))
}

/// `rulec check`, as data and as the text a terminal would print.
///
/// `{"ok":…,"errors":…,"warnings":…,"text":…,"diags":[…]}` — `text` is exactly what the
/// command prints for one file, and `diags` is `--format json`, one object per finding.
#[unsafe(no_mangle)]
pub extern "C" fn rulec_check(src: *const u8, ja: u32) -> *mut u8 {
    lang(ja);
    let src = input(src);
    let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
    let r = crate::report_with(src, PATH, crate::region::DEFAULT_BUDGET);
    let errors = r.diags.iter().filter(|d| d.severity == Severity::Error).count();
    let warnings = r.diags.len() - errors;
    let text =
        crate::findings_text(&r.diags, &lines) + &crate::check_tail(&r.shadow, &r.diags, PATH, 0);
    let diags: Vec<String> =
        r.diags.iter().map(|d| crate::diag::render_json(d, PATH)).collect();
    out(&Obj::new()
        .bool("ok", errors == 0)
        .int("errors", errors as i128)
        .int("warnings", warnings as i128)
        .str("text", &text)
        .raw("diags", arr(&diags))
        .finish())
}

/// `rulec gen`, in memory: every file every backend writes, the `.proto` beside them, and the
/// vectors with their expected records, in the order `rulec api` lists them.
///
/// Nothing is generated for a rule that does not pass (§1.6), and `ok` is false with the
/// findings in `text`, as the command prints them.
#[unsafe(no_mangle)]
pub extern "C" fn rulec_gen(src: *const u8, ja: u32) -> *mut u8 {
    lang(ja);
    let src = input(src);
    let Some((f, c)) = passing(src) else { return out(&refused(src)) };
    let g = Gen::new(&f, &c, src).at(PATH);
    let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
    let pkg = alias.replace('_', "").to_lowercase();
    let mut files: Vec<String> = Vec::new();
    for b in backend::ALL {
        for (rel, body) in (b.files)(&g, &alias, &pkg) {
            files.push(Obj::new().str("path", &rel).str("body", &body).finish());
        }
    }
    // The contract of the rule as a service, which is one file for every language and so is
    // not in the registry (§15.112).
    files.push(
        Obj::new()
            .str("path", format!("proto/{}", g.proto_path()))
            .str("body", &g.proto())
            .finish(),
    );
    files.push(Obj::new().str("path", "proto/buf.yaml").str("body", &g.buf_yaml()).finish());
    files.push(Obj::new().str("path", "proto/buf.gen.yaml").str("body", &g.buf_gen_yaml()).finish());
    let vs = crate::vectors::generate(&f, &c);
    let join = |v: Vec<String>| v.join("\n") + "\n";
    files.push(
        Obj::new()
            .str("path", format!("vectors/{alias}.jsonl"))
            .str("body", join(vs.iter().map(|v| crate::vectors::to_json(&f, &c, v)).collect()))
            .finish(),
    );
    files.push(
        Obj::new()
            .str("path", format!("vectors/{alias}.expected.jsonl"))
            .str(
                "body",
                join(vs.iter().map(|v| crate::vectors::expected_json(&f, &c, v)).collect()),
            )
            .finish(),
    );
    out(&Obj::new().bool("ok", true).raw("files", arr(&files)).finish())
}

/// `rulec doc --format html`: the page an approver reads, running the generated JavaScript,
/// as one self-contained document the playground can put in an iframe.
#[unsafe(no_mangle)]
pub extern "C" fn rulec_doc(src: *const u8, ja: u32) -> *mut u8 {
    lang(ja);
    let src = input(src);
    let Some((f, c)) = passing(src) else { return out(&refused(src)) };
    let js = Gen::new(&f, &c, src).javascript();
    let html = crate::doc::render_html(&f, &c, src, PATH, &js);
    out(&Obj::new().bool("ok", true).str("html", &html).finish())
}

/// The rule, when it passes. `gen` and `doc` both refuse the same way and for the same
/// reason, so they ask the same question.
fn passing(src: &str) -> Option<(crate::ast::RuleFile, crate::types::Checked)> {
    if crate::has_error(&crate::report_with(src, PATH, crate::region::DEFAULT_BUDGET).diags) {
        return None;
    }
    crate::prepare(src, PATH).ok()
}

fn refused(src: &str) -> String {
    let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
    let r = crate::report_with(src, PATH, crate::region::DEFAULT_BUDGET);
    Obj::new()
        .bool("ok", false)
        .str("text", crate::findings_text(&r.diags, &lines))
        .finish()
}
