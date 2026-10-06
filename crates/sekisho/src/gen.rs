//! `sekisho gen` (DESIGN 5, 11): what a `.gate` that passes its check generates, for a target.
//! For `--target cedar`, four files under `<out>/cedar/` (DESIGN 5.6), named by the file's alias:
//! the policies (`.cedar`), the schema (`.cedarschema`), and the JSON forms of both
//! (`.cedarschema.json`, `.policies.json`; [`crate::cedar::files`]). Nothing is generated from a
//! file that does not pass; `--check` writes nothing and says which file on disk differs from
//! what would be written (for CI), as koyomi's `gen --check` does.

use crate::check::{self, Options};
use crate::cli::refuse;
use crate::suite::Suite;
use ritsu_base::cli::Args;
use ritsu_base::text::Lang;
use std::io::Write;
use std::path::PathBuf;

/// The targets `gen` writes (DESIGN 5): Cedar now; the code that builds the requests comes with
/// stage C.
pub const TARGETS: [&str; 1] = ["cedar"];

/// What `gen` writes for one file that passes, for a target: each file's path under the directory
/// written to, and its text. `shown` is the path the heads name (the file's name, for sekisho's own
/// command). Err when the Cedar writer does not read what it is given (a bug of sekisho's).
pub fn files(o: &check::Outcome, target: &str, shown: &str, lang: Lang) -> Result<Vec<(String, String)>, ritsu_base::cedar::Error> {
    let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { return Ok(Vec::new()) };
    match target {
        "cedar" => Ok(crate::cedar::files(scope, checked, shown, lang)?.paths(scope.file().alias())),
        _ => Ok(Vec::new()),
    }
}

/// Say why a file is not generated from: its diagnostics on standard output, as `check` prints
/// them, and one line on standard error.
fn not_generated(o: &check::Outcome, why: ritsu_base::text::Text, lang: Lang, out: &mut dyn Write, err: &mut dyn Write) {
    let _ = write!(out, "{}", check::render(o, lang));
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    let _ = writeln!(err, "{head}: {}", why.get(lang));
}

/// `sekisho gen <file.gate>... --target cedar [--out <dir>] [--check]`: the exit code.
pub fn run(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.is_empty() {
        return refuse(err, tr!("`sekisho gen` には .gate のファイルが要ります", "`sekisho gen` needs .gate files"), lang);
    }
    let Some(target) = a.get("--target") else {
        let t = TARGETS.join("|");
        return refuse(err, tr!("`sekisho gen` には `--target {t}` が要ります", "`sekisho gen` needs `--target {t}`"), lang);
    };
    let out_dir = PathBuf::from(a.get("--out").unwrap_or("generated"));
    let check_only = a.has("--check");
    // the root the references of `@guards` are written from, found once for the run
    let opts = match crate::cli::root(a, &a.pos[0], lang, err) {
        Ok(root) => Options { root: Some(root), ..Options::default() },
        Err(code) => return code,
    };
    let mut worst = 0u8;
    // every file the run writes, and the .gate it comes from: two files of one alias would write
    // over each other
    let mut planned: Vec<(String, String, String)> = Vec::new();
    for f in &a.pos {
        let o = match check::check_file(f, suite, &opts) {
            Ok(o) => o,
            Err(e) => return refuse(err, tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang),
        };
        if o.unjoined() {
            // E209: the file reads a language this sekisho does not hold
            let _ = write!(out, "{}", check::render(&o, lang));
            worst = 2;
            continue;
        }
        if o.has_errors() || o.walked.is_none() {
            not_generated(&o, tr!("`{f}` は検査を通らないので、生成しません", "`{f}` does not pass check, so nothing is generated from it"), lang, out, err);
            worst = worst.max(1);
            continue;
        }
        let shown = ritsu_emit::header::file_name(f);
        let made = match files(&o, target, &shown, lang) {
            Ok(m) => m,
            Err(e) => {
                let (l, c) = (e.line, e.col);
                return refuse(
                    err,
                    tr!(
                        "`{f}` から生成した Cedar を読めません（{l}:{c}: {}）。sekisho の不具合です。報告してください",
                        "the Cedar generated from `{f}` does not read ({l}:{c}: {}): a bug in sekisho, please report it",
                        e.message.ja; e.message.en
                    ),
                    lang,
                );
            }
        };
        for (rel, body) in made {
            if let Some((_, _, from)) = planned.iter().find(|(p, _, _)| *p == rel) {
                return refuse(err, tr!("`{f}` と `{from}` が同じ {rel} を書き出します。どちらかの別名を変えてください", "`{f}` and `{from}` both write {rel}; give one of them another alias"), lang);
            }
            planned.push((rel, body, f.clone()));
        }
    }
    let mut dirty = false;
    for (rel, body, _) in &planned {
        let p = out_dir.join(rel);
        let shown = p.to_string_lossy().to_string();
        let existing = std::fs::read(&p).ok();
        if existing.as_deref() == Some(body.as_bytes()) {
            continue;
        }
        if check_only {
            dirty = true;
            let msg = match existing {
                None => tr!("ありません: {shown}", "missing: {shown}"),
                Some(_) => tr!("生成し直すと変わります: {shown}", "differs from what gen writes: {shown}"),
            };
            let _ = writeln!(out, "{}", msg.get(lang));
            continue;
        }
        if let Some(dir) = p.parent()
            && std::fs::create_dir_all(dir).is_err()
        {
            return refuse(err, tr!("`{}` を作れません", "cannot create `{}`", dir.display()), lang);
        }
        if std::fs::write(&p, body).is_err() {
            return refuse(err, tr!("`{shown}` に書けません", "cannot write `{shown}`"), lang);
        }
        let _ = writeln!(out, "{}", tr!("生成しました: {shown}", "generated: {shown}").get(lang));
    }
    if dirty {
        worst = worst.max(1);
    }
    worst
}
