//! Holding a declared enum to the file its values are declared in (§15.59, §15.60).
//!
//! Two kinds of file are read — a `.proto` and a JSON Schema — and what happens after they
//! are read is the same, so it lives here and the two readers stay readers.
//!
//! The shape of the check does not depend on the format. The file decides **which values
//! exist**; the rule decides **what they are called here** and what each one costs. Neither
//! can be derived from the other, so what is checked is that they agree (E032), and that no
//! value arrived without anyone deciding what the table does with it (E033).

use crate::ast::{EnumImport, EnumSource, RuleFile};
use crate::diag::Diag;
use crate::jsonschema::{self, Found};
use crate::types::Checked;

/// Checks every enum import of a rule against the file it names.
///
/// The path is resolved against the directory of the `.rule`, which is the only place a
/// relative path in it can sensibly mean. Reading happens here and nowhere deeper: the
/// checker itself stays a function from text to diagnostics (the playground runs it as wasm,
/// where there is no file to read, and says so rather than pretending the enum agrees).
pub fn check(f: &RuleFile, c: &Checked, rule_path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for im in &f.enum_imports {
        let p = std::path::Path::new(rule_path)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(&im.file);
        let at = format!("{rule_path}:{}", im.span.line);
        match std::fs::read_to_string(&p) {
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
            let found = crate::proto::enums(text);
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
                    tr!("`{}` までは届きました。", "It resolved as far as `{}`.", reached)
                } else {
                    tr!(
                        "`{}` までは届きました。そこにあるのは: {}",
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
        d = d.note(tr!(
            "足す形: `{}`。名前は自分で決めます — 日本語はそのファイルに入っていません。",
            "The form to add: `{}`. The name is yours to decide — the file carries no Japanese.",
            only_file
                .iter()
                .map(|a| format!("{}({a})", tr!("<名前>", "<name>")))
                .collect::<Vec<_>>()
                .join(" | ")
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
            "契約から消えた値なら、この規則からも消します。綴り違いなら直します。",
            "A value the contract dropped goes from the rule too; a misspelling is corrected."
        ));
    }
    vec![d.note(tr!(
        "値が増えたのは、この規則ではなく契約の側の変更です。増えた値にいくら付けるかは、まだ誰も決めていません（§15.59）。",
        "A value appeared through the contract, not through this rule. What the new value costs is a decision nobody has made yet (§15.59)."
    ))]
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
            !c.used_values.contains(&v.text) && !e.default_marks.get(*i).copied().unwrap_or(false)
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
            "この列挙の値は {1} の {0} が決めています。増えた値をどう扱うかを決めるのは、この規則のほうです。",
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
