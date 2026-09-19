//! Records of past data (§10.2) and their validation.
//!
//! Extracting production logs into this shape (ETL) is deliberately the user's job; what
//! rulec takes on is only the validation of types and ranges. The format is fixed to JSONL,
//! one record per line.
//!
//! ```text
//! {"ts":"2025-08-14T09:12:33+09:00","tag":"order:1234567",
//!  "in":{"届け先":"鹿児島県","重量":800,"注文金額":4200,"会員":"一般"},
//!  "observed":{"送料":800}}
//! ```
//!
//! **Fixtures do not go into the repository.** They contain order amounts, so hand them to CI
//! as an artifact or through protected storage (§10.2).

use crate::ast::RuleFile;
use crate::eval::Val;
use crate::json::Json;
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// One record. `in` has been resolved into the form the evaluator accepts.
pub struct Record {
    pub line: usize,
    pub tag: String,
    pub ts: String,
    pub input: BTreeMap<String, Val>,
    /// The observed outputs, looked up by name rather than by declaration order (the order
    /// on the record's side is not relied on).
    pub observed: BTreeMap<String, Val>,
    /// The fields filled in with default values. If non-empty, this record is a "filled
    /// record" (§10.3).
    pub filled: Vec<String>,
    /// The rows that matched when the record was made, as `(table, 1-based row)`, when the
    /// record carries them. The generated code's record function writes them (§15.35); a
    /// record made by hand may leave them out, and then this is empty.
    pub trace: Vec<(String, usize)>,
    /// The label each entry of `trace` carried, when it did. A row keeps its label when a row
    /// is inserted above it, so `replay` matches labelled rows by label rather than by number.
    pub trace_labels: Vec<Option<String>>,
}

pub struct Problem {
    pub line: usize,
    pub tag: String,
    /// A stable identifier for the kind of problem, for `--format json`. It never changes
    /// with `--lang`, which `what` and `hint` do (docs/formats.md).
    pub kind: &'static str,
    /// The field of the record at fault, empty when the problem is about the whole record.
    pub field: String,
    pub what: String,
    pub hint: String,
}

pub struct Load {
    pub records: Vec<Record>,
    pub problems: Vec<Problem>,
    /// How many records were excluded outright because a field was missing (mode 1 of
    /// §10.3).
    pub dropped: usize,
}

impl Load {
    /// The number of observed records (records with nothing filled in). The headline
    /// agreement rate is computed from these alone.
    pub fn measured(&self) -> usize {
        self.records.iter().filter(|r| r.filled.is_empty()).count()
    }
    pub fn filled(&self) -> usize {
        self.records.len() - self.measured()
    }
}

/// Convert a JSON value into the `Val` of the declared type. On the wire, numbers are
/// integers in the canonical unit (§10.1).
pub fn to_val(j: &Json, ty: &Ty, c: &Checked, name: &str) -> Result<Val, String> {
    let inner = match ty {
        Ty::Opt(t) => {
            if *j == Json::Null {
                return Ok(Val::Enum(crate::kw::NONE.into()));
            }
            (**t).clone()
        }
        other => other.clone(),
    };
    match (&inner, j) {
        (Ty::Enum(en), Json::Str(s)) => {
            let vs = c.enums.get(en).cloned().unwrap_or_default();
            if vs.iter().any(|v| v == s) {
                Ok(Val::Enum(s.clone()))
            } else {
                Err(tr!("`{s}` は列挙 {en} の値ではありません", "`{s}` is not a value of enum {en}"))
            }
        }
        (Ty::Bool, Json::Bool(b)) => Ok(Val::Bool(*b)),
        (Ty::Str, Json::Str(s)) => Ok(Val::Str(s.clone())),
        (Ty::Date, Json::Str(s)) => {
            let p: Vec<&str> = s.split('-').collect();
            let ok = p.len() == 3 && p[0].len() == 4 && p[1].len() == 2 && p[2].len() == 2;
            let n: Option<Vec<i64>> = p.iter().map(|x| x.parse::<i64>().ok()).collect();
            match (ok, n) {
                (true, Some(v)) if (1..=12).contains(&v[1]) && (1..=31).contains(&v[2]) => {
                    Ok(Val::Date(v[0] as i32, v[1] as u32, v[2] as u32))
                }
                _ => Err(tr!("`{s}` は YYYY-MM-DD の日付ではありません", "`{s}` is not a YYYY-MM-DD date")),
            }
        }
        (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number, Json::Int(n)) => {
            // The wire carries an integer in the canonical unit; a rate carries a count of
            // steps (§10.2). The range was declared in true values, so convert first.
            let v = crate::types::from_wire(*n, c.wire_scale(name));
            if let Some((lo, hi)) = c.ranges.get(name) {
                if lo.is_some_and(|l| v.cmp_to(l) == std::cmp::Ordering::Less)
                    || hi.is_some_and(|h| v.cmp_to(h) == std::cmp::Ordering::Greater)
                {
                    let show = |b: &Option<Rat>| b.map(|x| x.to_string()).unwrap_or_else(|| "…".into());
                    return Err(tr!(
                        "{n} は宣言範囲 {}..{} の外です",
                        "{n} is outside the declared range {}..{}",
                        show(&lo.map(|x| x)),
                        show(&hi.map(|x| x))
                    ));
                }
            }
            Ok(Val::Num(v))
        }
        (_, got) => Err(tr!("{} を期待しましたが {} でした", "expected {}, found {}", ty_word(&inner), got.kind())),
    }
}

fn ty_word(ty: &Ty) -> String {
    match ty {
        Ty::Enum(e) => tr!("列挙 {e} の値（文字列）", "value of enum {e} (string)"),
        Ty::Bool => crate::kw::BOOL.into(),
        Ty::Date => tr!("日付（YYYY-MM-DD の文字列）", "date (YYYY-MM-DD string)"),
        Ty::Str => tr!("文字列", "string"),
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => tr!("決まった単位の整数", "integer in the canonical unit"),
        _ => format!("{ty}"),
    }
}

/// The replay manifest (§10.3): a small JSON file placed next to the fixtures. **It holds
/// nothing but default values and the fields they apply to, so it contains nothing sensitive
/// and goes into the repository.**
///
/// ```json
/// {"rulec":"replay/1","rule":"送料","fills":{"会員":"一般"}}
/// ```
///
/// Default values are not baked into the rule itself because the rule is a pure function and
/// filling in is **a decision of one particular replay experiment**. Running "fill 会員 with
/// 一般" and "fill it with ゴールド to see the upper bound of the impact" separately against
/// the same rule is a legitimate use, and baking one value into the rule makes that
/// impossible. There is also the harm that an annotation with no bearing on the semantics
/// changes the rule's hash and makes `gen --check` demand a pointless regeneration.
///
/// Honesty is guaranteed not by "where the justification is written" but by "**whether the
/// justification appears where the numbers appear**": the report always stamps the default
/// values used and the number of records filled in per field.
#[derive(Default)]
pub struct Manifest {
    /// Field → default value. A record missing a field that is not listed here is excluded
    /// outright rather than filled in.
    pub fills: BTreeMap<String, Val>,
    /// The spelling exactly as written, for the stamp in the report.
    pub shown: BTreeMap<String, String>,
}

impl Manifest {
    /// Read the `会員=一般` form (a one-off override for sensitivity analysis, §10.3).
    pub fn add(&mut self, spec: &str, f: &RuleFile, c: &Checked) -> Result<(), String> {
        let (name, text) = spec.split_once('=').ok_or_else(|| {
            tr!("`{spec}` は `欄=値` の形ではありません", "`{spec}` is not of the form `field=value`")
        })?;
        let (name, text) = (name.trim(), text.trim());
        if !f.inputs.iter().any(|i| i.name.text == name) {
            return Err(tr!("`{name}` は規則の入力ではありません", "`{name}` is not an input of the rule"));
        }
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        // Quantities, money, and rates are read as integers; everything else as a string.
        let j = match &ty {
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => text
                .parse::<i128>()
                .map(Json::Int)
                .map_err(|_| tr!("`{name}` は決まった単位の整数で書いてください", "`{name}` must be written as an integer in the canonical unit"))?,
            Ty::Bool => match text {
                crate::kw::TRUE => Json::Bool(true),
                crate::kw::FALSE => Json::Bool(false),
                _ => return Err(tr!("`{name}` は {} か {} です", "`{name}` is either {} or {}", crate::kw::TRUE, crate::kw::FALSE)),
            },
            _ => Json::Str(text.into()),
        };
        let v = to_val(&j, &ty, c, name).map_err(|e| format!("`{name}`: {e}"))?;
        self.shown.insert(name.into(), text.into());
        self.fills.insert(name.into(), v);
        Ok(())
    }

    /// Read the manifest JSON.
    pub fn load(src: &str, f: &RuleFile, c: &Checked) -> Result<Manifest, String> {
        let j = crate::json::parse(src.trim()).map_err(|e| tr!("マニフェストが読めません: {e}", "Cannot read the manifest: {e}"))?;
        if let Some(r) = j.get("rule").and_then(|x| x.as_str()) {
            if r != f.name.text {
                return Err(tr!(
                    "マニフェストは規則 `{r}` のものです（いま見ているのは `{}`）",
                    "The manifest belongs to rule `{r}` (the current rule is `{}`)",
                    f.name.text
                ));
            }
        }
        let mut m = Manifest::default();
        let Some(fills) = j.get("fills").and_then(|x| x.as_obj()) else {
            return Ok(m);
        };
        for (name, v) in fills {
            if !f.inputs.iter().any(|i| i.name.text == *name) {
                return Err(tr!("`{name}` は規則の入力ではありません", "`{name}` is not an input of the rule"));
            }
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            let val = to_val(v, &ty, c, name).map_err(|e| tr!("既定値 `{name}`: {e}", "default value `{name}`: {e}"))?;
            m.shown.insert(name.clone(), format!("{v}"));
            m.fills.insert(name.clone(), val);
        }
        Ok(m)
    }
}

/// Read and validate the JSONL. **Broken records are reported, not discarded.** Dropping
/// them silently shrinks the denominator, which makes the agreement rate look higher (§10.3).
pub fn load(src: &str, f: &RuleFile, c: &Checked, m: &Manifest) -> Load {
    let mut out = Load { records: Vec::new(), problems: Vec::new(), dropped: 0 };
    for (li, raw) in src.lines().enumerate() {
        let line = li + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let j = match crate::json::parse(raw) {
            Ok(j) => j,
            Err(e) => {
                out.problems.push(Problem {
                    line,
                    tag: String::new(),
                    kind: "not_json",
                    field: String::new(),
                    what: tr!("JSON として読めません: {e}", "Not readable as JSON: {e}"),
                    hint: tr!("1 件 1 行の JSON Lines です（§10.2）。", "The format is JSON Lines, one record per line (§10.2)."),
                });
                continue;
            }
        };
        let tag = j.get("tag").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let ts = j.get("ts").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let mut bad = |kind: &'static str, field: &str, what: String, hint: &str| {
            out.problems.push(Problem {
                line,
                tag: tag.clone(),
                kind,
                field: field.to_string(),
                what,
                hint: hint.into(),
            });
        };

        let Some(ins) = j.get("in").and_then(|x| x.as_obj()) else {
            bad("no_in", "", tr!("`in` がありません", "`in` is missing"), &tr!("入力は `in` の下に、規則の和名で置きます。", "Inputs go under `in`, keyed by the names used in the rule."));
            continue;
        };
        let Some(obs) = j.get("observed").and_then(|x| x.as_obj()) else {
            bad("no_observed", "", tr!("`observed` がありません", "`observed` is missing"), &tr!("そのとき実際に出た値を `observed` に置きます。", "Put the values that actually came out at the time under `observed`."));
            continue;
        };

        // A field the rule does not know is reported as an error. Discarding it silently turns
        // a spelling mistake into "filled in with the default value", and only the agreement
        // rate moves.
        let known: Vec<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).collect();
        let mut extra: Vec<&String> = ins.keys().filter(|k| !known.contains(&k.as_str())).collect();
        extra.sort();
        if !extra.is_empty() {
            bad(
                "unknown_field",
                extra[0],
                tr!("`in` に規則が知らない欄があります: {}", "`in` has fields the rule does not know: {}", extra.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
                &tr!("規則の入力の和名と綴りを合わせてください。", "Match the spelling of the rule's input names."),
            );
            continue;
        }

        let mut input: BTreeMap<String, Val> = BTreeMap::new();
        let mut filled: Vec<String> = Vec::new();
        let mut broken = false;
        for i in &f.inputs {
            let name = &i.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            match ins.get(name) {
                Some(v) => match to_val(v, &ty, c, name) {
                    Ok(v) => {
                        input.insert(name.clone(), v);
                    }
                    Err(e) => {
                        bad("bad_input", name, format!("`in.{name}`: {e}"), &tr!("型か範囲が宣言と食い違っています。", "The type or range disagrees with the declaration."));
                        broken = true;
                    }
                },
                // §10.3: there are only two modes per field. If a default value is declared,
                // fill it in and label the record "filled"; if not, **exclude the whole
                // record**. No inference backwards. A missing field is an absent key, while
                // the `none` of an optional is `null` ("known to be absent" is not something
                // to fill in).
                None => match m.fills.get(name) {
                    Some(v) => {
                        input.insert(name.clone(), v.clone());
                        filled.push(name.clone());
                    }
                    None => {
                        broken = true;
                        out.dropped += 1;
                        break;
                    }
                },
            }
        }
        if broken {
            continue;
        }

        let mut observed: BTreeMap<String, Val> = BTreeMap::new();
        for o in &f.outputs {
            let name = &o.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            match obs.get(name) {
                Some(v) => match to_val(v, &ty, c, name) {
                    Ok(v) => {
                        observed.insert(name.clone(), v);
                    }
                    Err(e) => {
                        bad("bad_observed", name, format!("`observed.{name}`: {e}"), &tr!("そのとき出た値を、決まった単位で書いてください。", "Write the value that came out at the time, in the canonical unit."));
                        broken = true;
                    }
                },
                None => {
                    bad(
                        "missing_observed",
                        name,
                        tr!("`observed.{name}` がありません", "`observed.{name}` is missing"),
                        &tr!("出力は全部要ります。片方だけ比べると、比べなかった側の食い違いが緑になります。", "Every output is required. Comparing only one side turns a mismatch on the other side green."),
                    );
                    broken = true;
                }
            }
        }
        if broken {
            continue;
        }

        // `trace` is optional. When it is there, every table must be one of the rule's and
        // every row one the table has — a trace that names nothing real is a record from
        // some other version of the rule, and it is reported rather than read.
        let mut trace: Vec<(String, usize)> = Vec::new();
        let mut trace_labels: Vec<Option<String>> = Vec::new();
        if let Some(t) = j.get("trace") {
            let hint = tr!(
                "`trace` は `{{\"table\":表名,\"row\":行番号}}` の並びです。生成コードの record 関数が書きます。",
                "`trace` is a list of `{{\"table\":name,\"row\":number}}`; the generated code's record function writes it."
            );
            let mut ok = true;
            match t {
                Json::Arr(items) => {
                    for it in items {
                        let table = it.get("table").and_then(|x| x.as_str());
                        let row = it.get("row").and_then(|x| x.as_int());
                        let rows = table.and_then(|name| {
                            f.items.iter().find_map(|i| match i {
                                crate::ast::Item::Table(tb) if tb.name.as_ref().is_some_and(|n| n.text == name) => Some(tb.rows.len()),
                                _ => None,
                            })
                        });
                        match (table, row, rows) {
                            (Some(name), Some(r), Some(n)) if r >= 1 && (r as usize) <= n => {
                                trace.push((name.to_string(), r as usize));
                                trace_labels.push(it.get("label").and_then(|x| x.as_str()).map(|l| l.to_string()));
                            }
                            (Some(name), _, None) => {
                                bad("bad_trace", "trace", tr!("`trace`: 表 {name} はこの規則にありません", "`trace`: table {name} is not in this rule"), &hint);
                                ok = false;
                            }
                            (Some(name), Some(r), Some(n)) => {
                                bad("bad_trace", "trace", tr!("`trace`: 表 {name} に 行{r} はありません（{n} 行）", "`trace`: table {name} has no row {r} (it has {n})"), &hint);
                                ok = false;
                            }
                            _ => {
                                bad("bad_trace", "trace", tr!("`trace` の要素が `table` と `row` を持っていません", "an entry of `trace` lacks `table` or `row`"), &hint);
                                ok = false;
                            }
                        }
                        if !ok {
                            break;
                        }
                    }
                }
                _ => {
                    bad("bad_trace", "trace", tr!("`trace` が配列ではありません", "`trace` is not an array"), &hint);
                    ok = false;
                }
            }
            if !ok {
                continue;
            }
        }
        out.records.push(Record { line, tag, ts, input, observed, filled, trace, trace_labels });
    }
    out
}

/// The report of `rulec fixtures lint`.
pub fn render_lint(l: &Load, path: &str) -> String {
    let mut o = tr!(
        "{path}: 記録 {} 件（そのまま {}、補った分 {}）\n",
        "{path}: {} records ({} observed, {} filled)\n",
        l.records.len(),
        l.measured(),
        l.filled()
    );
    if l.dropped > 0 {
        o.push_str(&tr!("欄が欠けていたので外した記録: {} 件\n", "Records excluded because a field was missing: {}\n", l.dropped));
    }
    if l.problems.is_empty() {
        o.push_str(&tr!("形式の問題はありません。\n", "No format problems.\n"));
        return o;
    }
    o.push_str(&tr!("\n問題 {} 件:\n", "\n{} problems:\n", l.problems.len()));
    // Problems of the same shape are grouped. In a 10,000-line extract, the same misspelling
    // listed 10,000 times is unreadable.
    let mut by: BTreeMap<&str, Vec<&Problem>> = BTreeMap::new();
    for p in &l.problems {
        by.entry(p.what.as_str()).or_default().push(p);
    }
    for (what, ps) in &by {
        let ex = ps[0];
        let where_ = if ex.tag.is_empty() {
            tr!("{} 行目", "line {}", ex.line)
        } else {
            tr!("{} 行目 ({})", "line {} ({})", ex.line, ex.tag)
        };
        o.push_str(&tr!("  {what}\n    {} 件。例: {where_}\n    {}\n", "  {what}\n    {} record(s). Example: {where_}\n    {}\n", ps.len(), ex.hint));
    }
    o
}

/// `--format json` for `fixtures lint` (docs/formats.md). Problems of the same shape are
/// grouped exactly as in the text rendering: in a 10,000-line extract, the same misspelling
/// listed 10,000 times is unreadable in either form.
pub fn render_lint_json(l: &Load, path: &str) -> String {
    let mut by: BTreeMap<&str, Vec<&Problem>> = BTreeMap::new();
    for p in &l.problems {
        by.entry(p.what.as_str()).or_default().push(p);
    }
    let problems: Vec<String> = by
        .values()
        .map(|ps| {
            let ex = ps[0];
            let example = crate::json::Obj::new()
                .int("line", ex.line as i128)
                .str("tag", &ex.tag)
                .finish();
            let mut o = crate::json::Obj::new().str("kind", ex.kind);
            if !ex.field.is_empty() {
                o = o.str("field", &ex.field);
            }
            o.int("count", ps.len() as i128)
                .raw("example", example)
                .str("what", &ex.what)
                .str("hint", &ex.hint)
                .finish()
        })
        .collect();
    crate::json::Obj::new()
        .str("file", path)
        .int("records", l.records.len() as i128)
        .int("observed", l.measured() as i128)
        .int("filled", l.filled() as i128)
        .int("dropped", l.dropped as i128)
        .raw("problems", crate::json::arr(&problems))
        .finish()
}
