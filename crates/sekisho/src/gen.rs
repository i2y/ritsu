//! `sekisho gen` (DESIGN 5, 11): what a `.gate` that passes its check generates, for a target.
//! For `--target cedar`, four files under `<out>/cedar/` (DESIGN 5.6), named by the file's alias:
//! the policies (`.cedar`), the schema (`.cedarschema`), and the JSON forms of both
//! (`.cedarschema.json`, `.policies.json`; [`crate::cedar::files`]). For `--target typescript`,
//! `python` and `go`, the code that builds the requests and asks Cedar (DESIGN 5.3; [`typescript`],
//! [`python`], [`go`]), at `<out>/typescript/authz/<alias>.ts`, `<out>/python/authz/<alias>.py` and
//! `<out>/go/authz/<package>/<package>.go`, where `ritsu gen` puts it in a package beside the rules
//! and the dates it calls. Nothing is generated from a file that does not pass; `--check` writes
//! nothing and says which file on disk differs from what would be written (for CI), as koyomi's
//! `gen --check` does.

/// The code that builds the requests, in Go (DESIGN 5.3).
pub mod go;
/// What that code needs of a gate, whatever its language (Python and Go are written from it).
pub mod plan;
/// The code that builds the requests, in Python (DESIGN 5.3).
pub mod python;
/// The code that builds the requests, in TypeScript (DESIGN 5.3).
pub mod typescript;

use crate::check::{self, Options};
use crate::cli::refuse;
use crate::suite::Suite;
use ritsu_base::cli::Args;
use ritsu_base::text::Lang;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

pub use plan::Authorizer;

/// The targets `gen` writes (DESIGN 5): Cedar, and the code that builds the requests in TypeScript,
/// Python and Go.
pub const TARGETS: [&str; 4] = ["cedar", "typescript", "python", "go"];

/// What the code of a target is written with, besides the gate: where it asks Cedar
/// (`--authorizer`), and the Go import path of the package it goes in (`--module`; a rule's package
/// is `<module>/rules/<package>`, as `ritsu gen` lays a package out, DESIGN 5.6).
#[derive(Clone, Debug)]
pub struct Target<'a> {
    pub name: &'a str,
    pub authorizer: Authorizer,
    pub go_module: String,
}

/// What koyomi says of each calendar a file reads (`today is open in <calendar>`), by the index of
/// its `use`: the code calls the module koyomi writes for it, which is named by its alias.
pub fn calendars(g: &crate::model::Gate, suite: &Suite) -> BTreeMap<usize, ritsu_ports::CalendarFacts> {
    let mut out = BTreeMap::new();
    let Some(dates) = suite.dates.as_ref() else { return out };
    for (ui, u) in g.uses.iter().enumerate() {
        if u.kind == crate::model::UseKind::Calendar
            && let Ok(c) = dates.calendar(&u.file)
        {
            out.insert(ui, c);
        }
    }
    out
}

/// The code of a file that passes, for `python` or `go`: its path under the directory written to
/// (`python/authz/<alias>.py`, `go/authz/<package>/<package>.go`) and its text.
pub fn code(o: &check::Outcome, suite: &Suite, target: &Target, shown: &str, lang: Lang) -> Result<Vec<(String, String)>, ritsu_base::cedar::Error> {
    let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { return Ok(Vec::new()) };
    let cals = calendars(&checked.gate, suite);
    let p = plan::Plan::new(scope, checked, &cals, lang)?;
    let opts = plan::Options { shown: shown.to_string(), lang, authorizer: target.authorizer, go_module: target.go_module.clone() };
    let alias = scope.file().alias().to_string();
    Ok(match target.name {
        "python" => vec![(format!("python/{}", python::path(&alias)), python::module(&p, &opts))],
        "go" => vec![(format!("go/{}", go::path(&alias)), go::module(&p, &opts))],
        _ => Vec::new(),
    })
}

/// The quotas of Amazon Verified Permissions a gate's Cedar is held to under `--authorizer avp`
/// (DESIGN 5.8, as of 2026-10): a policy's text, a schema in the JSON `PutSchema` takes, and the
/// transitive parents of an entity.
pub const POLICY_BYTES: usize = 10_000;
pub const SCHEMA_BYTES: usize = 100_000;
pub const PARENTS: usize = 100;

/// W401 (DESIGN 5.8): what of the Cedar of a file that passes is over a quota of Verified
/// Permissions — a policy longer than its quota, as `cedar format` writes it alone (as it is put in
/// the policy store), the schema's JSON, and the roles a principal of a type can hold, with what
/// they include, past the parents an entity can have.
pub fn quotas(o: &check::Outcome, lang: Lang) -> Vec<crate::diag::Diag> {
    use crate::diag::Diag;
    use ritsu_base::text::count;
    let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { return Vec::new() };
    let g = &checked.gate;
    let mut out = Vec::new();
    let shape = crate::cedar::shape(g, scope, checked);
    let set = crate::cedar::policies(g, scope, &shape);
    for p in &set.policies {
        let Ok(text) = ritsu_base::cedar::write_policy(p).and_then(|t| ritsu_base::cedar::format_policies(&t, 80, 2)) else { continue };
        if text.len() <= POLICY_BYTES {
            continue;
        }
        let at = (0..g.policies.len()).find(|&i| crate::cedar::policy_id(g, i) == p.id && g.policies[i].from.is_none()).map(|i| g.policies[i].named.line).unwrap_or(1);
        let (id, n, most) = (&p.id, count(text.len() as u64), count(POLICY_BYTES as u64));
        out.push(
            Diag::at("W401", &g.file, at, 1, tr!("ポリシー `{id}` は {n} バイトで、Verified Permissions の上限の {most} バイトを超えます", "The policy `{id}` is {n} bytes, past the quota of Verified Permissions of {most} bytes a policy"))
                .source(&g.src)
                .note(tr!("ポリシーを二つ以上に分けてください。", "Split the policy in two or more.")),
        );
    }
    if let Ok(files) = crate::cedar::files(scope, checked, &ritsu_emit::header::file_name(&g.file), lang)
        && files.schema_json.len() > SCHEMA_BYTES
    {
        let (n, most) = (count(files.schema_json.len() as u64), count(SCHEMA_BYTES as u64));
        out.push(
            Diag::at("W401", &g.file, 1, 1, tr!("スキーマは JSON で {n} バイトで、Verified Permissions の上限の {most} バイトを超えます", "The schema is {n} bytes as JSON, past the quota of Verified Permissions of {most} bytes a schema"))
                .source(&g.src)
                .note(tr!("型、属性、action を、いくつかのゲートとポリシーストアに分けてください。", "Spread the types, the attributes and the actions over gates of their own, each with a policy store.")),
        );
    }
    for t in g.types.iter().filter(|t| !t.roles.is_empty()) {
        let mut held: Vec<usize> = Vec::new();
        for &r in &t.roles {
            for x in g.closure(r) {
                if !held.contains(&x) {
                    held.push(x);
                }
            }
        }
        let n = held.len();
        if n <= PARENTS {
            continue;
        }
        let ty = &t.named.name;
        out.push(
            Diag::at("W401", &g.file, t.named.line, 1, tr!("{ty} が持てる役割は、含む役割と合わせて {n} 個で、Verified Permissions がエンティティに許す推移的な親の {PARENTS} 個を超えます", "A {ty} can hold {n} roles with the roles they include, past the {PARENTS} transitive parents Verified Permissions lets an entity have"))
                .source(&g.src)
                .note(tr!("役割の入れ子を浅くするか、型の roles を分けてください。", "Make the roles nest less deep, or spread the type's roles over types of their own.")),
        );
    }
    crate::diag::sort(&mut out);
    out
}

/// What `gen` writes for one file that passes, for any target: each file's path under the
/// directory written to, and its text. The code of the requests reads the facts of the rules, the
/// dates and the calendars the gate computes with through `suite`. Err with what went wrong (a bug
/// of sekisho's: the file passed its check).
pub fn generate(o: &check::Outcome, target: &Target, shown: &str, lang: Lang, suite: &Suite) -> Result<Vec<(String, String)>, String> {
    let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { return Ok(Vec::new()) };
    let cedar_error = |e: ritsu_base::cedar::Error| format!("{}:{}: {}", e.line, e.col, e.message.en);
    match target.name {
        "cedar" => files(o, "cedar", shown, lang).map_err(cedar_error),
        "typescript" => Ok(vec![(typescript::path(scope.file().alias()), typescript::module(scope, checked, suite, shown, lang, target.authorizer)?)]),
        "python" | "go" => code(o, suite, target, shown, lang).map_err(cedar_error),
        _ => Ok(Vec::new()),
    }
}

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

/// `sekisho gen <file.gate>... --target cedar|typescript|python|go [--authorizer cedar|avp] [--module
/// <path>] [--out <dir>] [--check]`: the exit code.
pub fn run(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.is_empty() {
        return refuse(err, tr!("`sekisho gen` には .gate のファイルが要ります", "`sekisho gen` needs .gate files"), lang);
    }
    let Some(target) = a.get("--target") else {
        let t = TARGETS.join("|");
        return refuse(err, tr!("`sekisho gen` には `--target {t}` が要ります", "`sekisho gen` needs `--target {t}`"), lang);
    };
    let authorizer = a.get("--authorizer").and_then(Authorizer::parse).unwrap_or(Authorizer::Cedar);
    let go_module = a.get("--module").unwrap_or("generated").to_string();
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
        // what is put in Verified Permissions is held to its quotas
        if authorizer == Authorizer::Avp {
            for d in quotas(&o, lang) {
                let _ = write!(out, "{}", d.render(lang));
            }
        }
        let made = match generate(&o, &Target { name: target, authorizer, go_module: go_module.clone() }, &shown, lang, suite) {
            Ok(m) => m,
            Err(e) => {
                return refuse(err, tr!("`{f}` から生成できません（{e}）。sekisho の不具合です。報告してください", "cannot generate from `{f}` ({e}): a bug in sekisho, please report it"), lang);
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
        let existing = ritsu_base::fs::read(&p).ok();
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
            && ritsu_base::fs::create_dir_all(dir).is_err()
        {
            return refuse(err, tr!("`{}` を作れません", "cannot create `{}`", dir.display()), lang);
        }
        if ritsu_base::fs::write(&p, body).is_err() {
            return refuse(err, tr!("`{shown}` に書けません", "cannot write `{shown}`"), lang);
        }
        let _ = writeln!(out, "{}", tr!("生成しました: {shown}", "generated: {shown}").get(lang));
    }
    if dirty {
        worst = worst.max(1);
    }
    worst
}
