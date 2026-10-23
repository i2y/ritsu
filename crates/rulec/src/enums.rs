//! Holding a declared enum to the file its values are declared in (§15.59, §15.60).
//!
//! Two kinds of file are read — a `.proto` and a JSON Schema — and what happens after they
//! are read is the same, so it lives here and the two readers stay readers.
//!
//! The shape of the check does not depend on the format. The file decides **which values
//! exist**; the rule decides **what they are called here** and what each one costs. Neither
//! can be derived from the other, so what is checked is that they agree (E032), and that no
//! value arrived without anyone deciding what the table does with it (E033).

use crate::ast::{EnumDecl, EnumFrom, EnumImport, EnumOwner, EnumSource, FileKind, Name, RuleFile};
use crate::diag::{Diag, Severity, Span};
use crate::jsonschema::{self, Found};
use crate::types::Checked;

/// An `enums` file as a rule reads it (§15.212): parsed, checked whole, and typed.
pub struct EnumsFile {
    pub file: RuleFile,
    pub checked: Checked,
    /// The source, and the path rulec reached it at.
    pub src: String,
    pub path: String,
}

/// Reads the `enums` file at `path` and checks it whole. `Err` holds the codes of the errors it
/// has, or `None` when it is not an `enums` file at all.
pub fn read_enums_file(path: &str) -> Result<EnumsFile, Option<Vec<&'static str>>> {
    let bytes = crate::vfs::read(std::path::Path::new(path)).map_err(|_| Some(Vec::new()))?;
    let src = String::from_utf8_lossy(&bytes).into_owned();
    let parsed = crate::parse::parse(&src, path);
    if parsed.file.as_ref().is_some_and(|f| f.kind != FileKind::Enums) {
        return Err(None);
    }
    let rep = crate::report(&src, path);
    let mut codes: Vec<&'static str> = rep.diags.iter().filter(|d| d.severity == Severity::Error).map(|d| d.code).collect();
    codes.sort();
    codes.dedup();
    if !codes.is_empty() {
        return Err(Some(codes));
    }
    let (file, checked) = crate::prepare(&src, path).map_err(|_| Some(Vec::new()))?;
    Ok(EnumsFile { file, checked, src, path: path.to_string() })
}

/// Whether the file at `path` is an `enums` file (§15.212): its first line that is neither blank
/// nor a comment starts with `enums`. A file that cannot be read is not one.
pub fn is_enums_file(path: &str) -> bool {
    let Ok(bytes) = crate::vfs::read(std::path::Path::new(path)) else {
        return false;
    };
    let src = String::from_utf8_lossy(&bytes);
    src.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .is_some_and(|l| l.split_whitespace().next() == Some(crate::kw::ENUMS))
}

/// The `enums` files a rule reads its enums from, as the rule's directory joined with what each
/// `import enum` line writes, each once, in the order written (§15.212).
pub fn owner_paths(f: &RuleFile) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in &f.enums {
        if let Some(o) = &e.from {
            if !out.contains(&o.path) {
                out.push(o.path.clone());
            }
        }
    }
    out
}

/// Reads every `import enum <enum> from "<file>"` of a rule and puts what it names into the rule
/// (§15.212): the enum with its values, order, display names and `default` marks, and the groups
/// of that enum, each marked with where it came from. The rule is then checked over closed
/// enums, as it always was; nothing about them is written twice.
///
/// The file is read on every check and pinned by nothing, the way `import proto` is: the path
/// holds the two together, and a value the file gains reaches every rule that reads it.
pub fn read_imports(f: &mut RuleFile, rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    if f.enum_froms.is_empty() {
        return out;
    }
    let dir = std::path::Path::new(rule_path).parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
    let mut read: Vec<(String, Result<EnumsFile, Option<Vec<&'static str>>>)> = Vec::new();
    let mut taken: Vec<crate::ast::EnumDecl> = Vec::new();
    let mut groups: Vec<crate::ast::GroupDecl> = Vec::new();
    let froms = f.enum_froms.clone();
    for im in &froms {
        let at = format!("{rule_path}:{}", im.span.line);
        let p = dir.join(&im.file);
        let ps = p.to_string_lossy().into_owned();
        if !read.iter().any(|(q, _)| *q == ps) {
            read.push((ps.clone(), read_enums_file(&ps)));
        }
        let Some((_, got)) = read.iter().find(|(q, _)| *q == ps) else { continue };
        let ef = match got {
            Ok(ef) => ef,
            Err(Some(codes)) if codes.is_empty() => {
                out.push(
                    Diag::error("E013", tr!("`{}` を読めません", "Cannot read `{}`", im.file))
                        .at(at)
                        .mark(im.span.clone(), "")
                        .note(tr!(
                            "パスは規則ファイルのある場所からたどります（探した先: {}）。",
                            "The path is followed from the directory of the rule file (looked for: {}).",
                            p.display()
                        )),
                );
                continue;
            }
            Err(Some(codes)) => {
                out.push(
                    Diag::error("E066", tr!("`{}` が check を通りません（{}）", "`{}` does not pass check ({})", im.file, codes.join(", ")))
                        .at(at)
                        .mark(im.span.clone(), "")
                        .note(tr!(
                            "先に `{}` のファイルを直してください。読めない列挙から読むと、この規則の値の集合が決まりません。",
                            "Fix the `{}` file first. An enum that does not check leaves this rule's set of values undecided.",
                            crate::kw::ENUMS
                        )),
                );
                continue;
            }
            Err(None) => {
                out.push(
                    Diag::error("E066", tr!("`{}` は `{}` のファイルではありません", "`{}` is not an `{}` file", im.file, crate::kw::ENUMS))
                        .at(at)
                        .mark(im.span.clone(), "")
                        .note(tr!(
                            "`import {} … from` が読むのは、`{} <名前> v<版>` で始まるファイルの列挙です。規則の列挙を別の規則から使うなら、その列挙を `{}` のファイルに移して、両方の規則から読んでください。",
                            "`import {} … from` reads an enum of a file that starts with `{} <name> v<version>`. To share an enum between two rules, move it into an `{}` file and read it from both.",
                            crate::kw::ENUM,
                            crate::kw::ENUMS,
                            crate::kw::ENUMS
                        )),
                );
                continue;
            }
        };
        let Some(decl) = ef.file.enums.iter().find(|e| e.name.text == im.name.text || e.name.ascii.as_deref() == Some(im.name.text.as_str())) else {
            let have: Vec<String> = ef.file.enums.iter().map(|e| e.name.text.clone()).collect();
            out.push(
                Diag::error("E013", tr!("`{}` に `{}` という列挙はありません", "`{}` has no enum named `{}`", im.file, im.name.text))
                    .at(at)
                    .mark(im.name.span.clone(), "")
                    .note(if have.is_empty() {
                        tr!("そのファイルに列挙は一つもありません。", "That file declares no enum at all.")
                    } else {
                        tr!("そのファイルにある列挙: {}", "The enums in that file: {}", have.join(", "))
                    }),
            );
            continue;
        };
        // The name as this rule's line writes it, where `->` gives none: its place is the import
        // line, not the other file's.
        let local: Name = im.local.clone().unwrap_or_else(|| Name { text: decl.name.text.clone(), ascii: decl.name.ascii.clone(), span: im.name.span.clone() });
        let sp = im.span.clone();
        let mut e = decl.clone();
        e.from = Some(EnumOwner { file: im.file.clone(), path: ps.clone(), module: ef.file.name.clone(), name: decl.name.clone() });
        // Renamed or not, it is the owner's type: the alias stays the one the file gives it, and
        // the generated code reads that type from the file's module.
        e.name = Name { text: local.text.clone(), ascii: decl.name.ascii.clone(), span: local.span.clone() };
        // The places it is reported at are this rule's line that reads it: the other file's
        // line numbers mean nothing here.
        e.span = sp.clone();
        for v in &mut e.values {
            v.span = sp.clone();
        }
        for l in e.labels.iter_mut().flatten() {
            l.1 = sp.clone();
        }
        if let Some(o) = e.open.as_mut() {
            *o = sp.clone();
        }
        for g in &ef.file.groups {
            let owner = ef.checked.groups.get(&g.name.text).map(|(o, _)| o.clone());
            if owner.as_deref() != Some(decl.name.text.as_str()) {
                continue;
            }
            let mut g = g.clone();
            g.span = sp.clone();
            g.name.span = sp.clone();
            for m in &mut g.members {
                m.span = sp.clone();
            }
            g.from = Some(im.file.clone());
            groups.push(g);
        }
        taken.push(e);
    }
    // One name, one enum: what the rule reads may not take a name the rule declares, nor one
    // another line of it reads (E034, as for row labels, tables and sources).
    let mut seen: Vec<(String, Span)> = Vec::new();
    for e in taken.iter().chain(f.enums.iter()) {
        if let Some((_, first)) = seen.iter().find(|(n, _)| *n == e.name.text) {
            out.push(
                Diag::error("E034", tr!("列挙の名前 `{}` が二度あります", "The enum name `{}` appears twice", e.name.text))
                    .at(format!("{rule_path}:{}", e.span.line))
                    .mark(first.clone(), tr!("最初の宣言", "the first declaration"))
                    .mark(e.name.span.clone(), tr!("同じ名前", "the same name"))
                    .note(tr!(
                        "`import {} … from` で読んだ列挙も、この規則の名前になります。`-> <別の名前>` で読むか、どちらかの名前を変えてください。",
                        "An enum read with `import {} … from` takes a name in this rule too. Read it with `-> <another name>`, or rename one of the two.",
                        crate::kw::ENUM
                    )),
            );
        } else {
            seen.push((e.name.text.clone(), e.name.span.clone()));
        }
    }
    // Two enums whose types take one name in the generated code (§15.263): the type of one read with
    // `import enum` is its file's, whatever this rule calls it, and the rule's module names it so.
    let type_of = |e: &EnumDecl| match &e.from {
        Some(o) => crate::codegen::spellings::enum_type(&o.name.ascii.clone().unwrap_or_else(|| o.name.text.clone())),
        None => crate::codegen::spellings::enum_type(&e.name.ascii.clone().unwrap_or_else(|| e.name.text.clone())),
    };
    // One enum of one file is one type, however many lines read it.
    let whose = |e: &EnumDecl| e.from.as_ref().map(|o| (o.path.clone(), o.name.text.clone()));
    let mut tseen: Vec<(String, &EnumDecl)> = Vec::new();
    for e in taken.iter().chain(f.enums.iter()) {
        let t = type_of(e);
        match tseen.iter().find(|(k, _)| *k == t) {
            Some((_, first)) if first.name.text != e.name.text && (whose(first).is_none() || whose(first) != whose(e)) => out.push(
                Diag::error("E034", tr!("列挙 `{}` と `{}` の型が、生成するコードで同じ名前 `{t}` になります", "The types of enums `{}` and `{}` take the one name `{t}` in the generated code", first.name.text, e.name.text))
                    .at(format!("{rule_path}:{}", e.span.line))
                    .mark(first.name.span.clone(), "")
                    .mark(e.name.span.clone(), "")
                    .note(tr!(
                        "`import {} … from` で読んだ列挙は、そのファイルの型の名前で生成するコードに入り、この規則の列挙の型と同じ名前になります。規則のモジュールに同じ名前の型が二つになり、組めないか、片方がもう片方を隠します（SQL と NumPy は型を書かず、Java は読んだ列挙をそのファイルのクラスの名前で書きます）。この規則の列挙に別の別名を付けてください。",
                        "An enum read with `import {} … from` comes into the generated code under its file's type, which is the name of this rule's enum's type too, so the rule's module holds two types of one name: it does not compile, or one hides the other (SQL and NumPy write no types, and Java writes the enum it reads by its file's class). Give this rule's enum another alias.",
                        crate::kw::ENUM
                    )),
            ),
            Some(_) => {}
            None => tseen.push((t, e)),
        }
    }
    let mut gseen: Vec<String> = f.groups.iter().map(|g| g.name.text.clone()).collect();
    for g in &groups {
        if gseen.contains(&g.name.text) {
            out.push(
                Diag::error("E034", tr!("グループの名前 `{}` が二度あります", "The group name `{}` appears twice", g.name.text))
                    .at(format!("{rule_path}:{}", g.span.line))
                    .mark(g.span.clone(), "")
                    .note(tr!(
                        "このグループは `{}` から列挙と一緒に来ました。この規則のグループの名前を変えてください。",
                        "This group came with its enum from `{}`. Rename the group this rule declares.",
                        g.from.as_deref().unwrap_or("")
                    )),
            );
        }
        gseen.push(g.name.text.clone());
    }
    // What is read comes first, the way the built-in namespaces do: a value two enums share is
    // read in the first one that has it, the same one every run (§15.150).
    taken.extend(std::mem::take(&mut f.enums));
    f.enums = taken;
    groups.extend(std::mem::take(&mut f.groups));
    f.groups = groups;
    out
}

/// The names of the enums of `f` read from `enums` files, with the file each came from.
pub fn from_files(f: &RuleFile) -> Vec<(&crate::ast::EnumDecl, &EnumOwner)> {
    f.enums.iter().filter_map(|e| e.from.as_ref().map(|o| (e, o))).collect()
}

/// The `import enum` lines of `f`, for the commands that list what a rule reads.
pub fn imports_of(f: &RuleFile) -> &[EnumFrom] {
    &f.enum_froms
}

/// Checks every enum import of a rule against the file it names.
///
/// The path is resolved against the directory of the `.rule`, which is the only place a
/// relative path in it can sensibly mean. Reading happens here and nowhere deeper: the
/// checker itself stays a function from text to diagnostics (where the file cannot be read,
/// E013 says so rather than pretending the enum agrees).
pub fn check(f: &RuleFile, c: &Checked, rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for im in &f.enum_imports {
        // Followed from the rule that wrote the line: the rule a contribution adds to, in the
        // contribution's check (§15.235).
        let p = im.file_path(rule_path);
        let at = format!("{rule_path}:{}", im.span.line);
        match ritsu_base::fs::read_to_string(&p) {
            Ok(text) => match wanted(im, &text, &at) {
                Ok(want) => out.extend(compare(f, c, im, &want, rule_path)),
                Err(d) => out.push(d),
            },
            Err(_) => out.push(
                Diag::error("E013", tr!("`{}` を読めません", "Cannot read `{}`", im.file))
                    .at(at)
                    .mark(im.span.clone(), "")
                    .note(tr!(
                        "パスは規則ファイルのある場所からたどります（探した先: {}）。",
                        "The path is followed from the directory of the rule file (looked for: {}).",
                        p.display()
                    )),
            ),
        }
    }
    out
}

/// The aliases the file asks the rule for, or why they could not be read.
fn wanted(im: &EnumImport, text: &str, at: &str) -> Result<Vec<String>, Diag> {
    let err = |title: String| Diag::error("E013", title).at(at.to_string()).mark(im.span.clone(), "");
    match im.kind {
        EnumSource::Proto => {
            // A contract that does not read is named, not read in part (§15.166).
            let found = match crate::proto::read(&im.file, text) {
                Ok(f) => f.enums,
                Err(e) => return Err(err(tr!("`{}` を読めません", "Cannot read `{}`", im.file)).note(crate::proto::unreadable(&e))),
            };
            match found.iter().find(|e| e.name == im.source) {
                Some(e) => Ok(e.aliases()),
                None => Err(err(tr!(
                    "`{}` に `{}` という列挙はありません",
                    "`{}` has no enum named `{}`",
                    im.file,
                    im.source
                ))
                .note(if found.is_empty() {
                    tr!("そのファイルに列挙は一つもありません。", "That file declares no enum at all.")
                } else {
                    tr!(
                        "そのファイルにある列挙: {}",
                        "The enums in that file: {}",
                        found.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", ")
                    )
                })),
            }
        }
        EnumSource::JsonSchema => {
            let doc = jsonschema::read(text, &im.file).map_err(|why| {
                err(tr!("`{}` を読めません", "Cannot read `{}`", im.file)).note(why)
            })?;
            match jsonschema::values(&doc, &im.source) {
                Found::Values(v) => Ok(v),
                Found::NoSuchPointer { at: reached, keys } => Err(err(tr!(
                    "`{}` の中に `{}` はありません",
                    "`{}` has nothing at `{}`",
                    im.file,
                    im.source
                ))
                .note(if keys.is_empty() {
                    tr!("`{}` までは見つかりました。", "It resolved as far as `{}`.", reached)
                } else {
                    tr!(
                        "`{}` までは見つかりました。そこにあるのは: {}",
                        "It resolved as far as `{}`, which holds: {}",
                        reached,
                        keys.join(", ")
                    )
                })),
                Found::NotAnEnum(what) => Err(err(tr!(
                    "`{}` の `{}` は列挙ではありません",
                    "`{}` in `{}` is not an enum",
                    im.source,
                    im.file
                ))
                .note(tr!(
                    "そこにあるのは {what} です。`enum` を持つスキーマか、その `enum` の配列を指してください。",
                    "What is there is {what}. Point at a schema that has an `enum`, or at that `enum` array."
                ))),
                Found::NotNames(v) => Err(err(tr!(
                    "`{}` の `{}` に、名前でない値があります",
                    "`{}` in `{}` holds a value that is not a name",
                    im.source,
                    im.file
                ))
                .note(tr!(
                    "名前でない値: {v}。表のセルに書けるのは名前だけなので、数や null の列挙は取り込めません。",
                    "The value: {v}. A cell of a table holds a name, so an enum of numbers or nulls cannot be imported."
                ))),
            }
        }
    }
}

/// E032: the set the file has and the set the rule declares are not the same.
fn compare(f: &RuleFile, c: &Checked, im: &EnumImport, want: &[String], rule_path: &str) -> Vec<Diag> {
    let Some(target) = f.enums.iter().find(|e| e.name.text == im.target.text) else {
        // The rule names an enum it does not declare. E012 has already said so; saying it
        // twice from here would only bury it.
        return Vec::new();
    };
    let have: Vec<String> = target
        .values
        .iter()
        .map(|v| v.ascii.clone().unwrap_or_else(|| v.text.clone()))
        .collect();
    let only_file: Vec<&String> = want.iter().filter(|a| !have.contains(a)).collect();
    let only_rule: Vec<&String> = have.iter().filter(|a| !want.contains(a)).collect();
    if only_file.is_empty() && only_rule.is_empty() {
        // A file of enums has no rows to decide anything in; each rule that reads the enum is
        // held to it (§15.212).
        if f.kind == FileKind::Enums {
            return Vec::new();
        }
        return undecided(target, c, im, rule_path);
    }

    let mut d = Diag::error(
        "E032",
        tr!(
            "列挙 {0} が {1} の {2} と一致していません",
            "Enum {0} does not agree with {2} in {1}",
            target.name.text,
            im.file,
            im.source
        ),
    )
    .at(format!("{rule_path}:{}", target.span.line))
    .mark(target.name.span.clone(), "");
    if !only_file.is_empty() {
        d = d.note(tr!(
            "{} にあって、この列挙に無い値: {}",
            "In {} but not in this enum: {}",
            im.file,
            only_file.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" / ")
        ));
        // The form follows how the enum is written. Where its values are named in ASCII, the
        // contract's value can be added as it is; where they are named in another script (and so
        // carry an ASCII alias), the alias is the contract's value and the name is the writer's.
        let named_apart = target.values.iter().any(|v| !v.text.is_ascii());
        d = if named_apart {
            d.note(tr!(
                "この列挙は値の名前に別名を付けて書いているので、`{}` の形で足してください。`<名前>` の部分は自分で決めてください。",
                "This enum gives its values names with aliases, so add it as `{}`, with a name of your choosing.",
                only_file
                    .iter()
                    .map(|a| format!("{}({a})", tr!("<名前>", "<name>")))
                    .collect::<Vec<_>>()
                    .join(" | ")
            ))
        } else {
            d.note(tr!(
                "この列挙に `{}` を足してください。",
                "Add `{}` to this enum.",
                only_file.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" | ")
            ))
        };
        d = d.note(tr!(
            "契約（{}）に値が増えました。この規則がその値をどう扱うかは、まだ決まっていません。",
            "The contract ({}) has gained a value. How this rule treats it has not been decided yet.",
            im.file
        ));
    }
    if !only_rule.is_empty() {
        d = d.note(tr!(
            "この列挙にあって、{} に無い値: {}",
            "In this enum but not in {}: {}",
            im.file,
            only_rule.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" / ")
        ));
        d = d.note(tr!(
            "契約から消えた値なら、この規則からも消してください。綴りの誤りなら、綴りを直してください。",
            "A value the contract dropped goes from the rule too; a misspelling is corrected."
        ));
    }
    vec![d]
}

/// The second gate (E033). The sets agree, so every value the contract has is written here —
/// and a value written here that no row names and no `default` marks is a change nobody has
/// read. A table with a `-` row passes the completeness check with it, which is precisely why
/// the warning W111 would be enough for a value you wrote and is not enough for this one.
fn undecided(e: &crate::ast::EnumDecl, c: &Checked, im: &EnumImport, rule_path: &str) -> Vec<Diag> {
    let names: Vec<String> = e
        .values
        .iter()
        .enumerate()
        .filter(|(i, v)| {
            !c.used_values.contains(&(e.name.text.clone(), v.text.clone())) && !e.default_marks.get(*i).copied().unwrap_or(false)
        })
        .map(|(_, v)| v.text.clone())
        .collect();
    if names.is_empty() {
        return Vec::new();
    }
    vec![
        Diag::error(
            "E033",
            tr!(
                "取り込んだ列挙 {} の値に、行も `default` もありません",
                "A value of the imported enum {} has neither a row nor `default`",
                e.name.text
            ),
        )
        .at(format!("{rule_path}:{}", e.span.line))
        .fix(crate::diag::FixKind::MarkDefault, crate::kw::DEFAULT)
        .mark(e.name.span.clone(), "")
        .note(tr!("決まっていない値: {}", "Values with nothing decided: {}", names.join(" / ")))
        .note(tr!(
            "この列挙の値は {1} の {0} が決めています。増えた値をどう扱うかは、この規則の側で決めてください。",
            "The values of this enum are decided by {0} in {1}. What happens to a new one is decided here.",
            im.source,
            im.file
        ))
        .note(tr!(
            "表に行を足すか、その値に `default` を付けて「既定の行に落ちるのが意図です」と書いてください。",
            "Add a row to the table, or mark the value `default` to declare that falling through to the default row is what is meant."
        )),
    ]
}
