//! The data of masters (DESIGN 4.6): one CSV file for each master, with a column for each of its
//! fields and `valid_from` and `valid_to`, the first day and the last day a row may be chosen on
//! (both in; an empty `valid_from` from any day, an empty `valid_to` with no end).
//!
//! The data is read when the masters' file is checked, once in a run, and kept with it. A row that
//! does not read is said at its row (E501), and a person of a row who is not in the organization
//! on some day the row is in effect (E505), when the file reads the organization. A request that
//! chooses from a master reads its rows from there: the days a row may be chosen on decide the days
//! the request can be used on (`crate::usable`, E107).

use super::Org;
use super::data::{self, Table};
use crate::ast::{Field, File, TypeExpr};
use crate::diag::Diag;
use ritsu_base::text::Text;

/// A row of a master.
#[derive(Clone, Debug)]
pub struct MRow {
    pub line: usize,
    pub key: String,
    /// The values, in the order of the master's fields.
    pub values: Vec<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

impl MRow {
    /// Whether it may be chosen on `day`.
    pub fn on(&self, day: i64) -> bool {
        self.from.is_none_or(|f| f <= day) && self.to.is_none_or(|t| day <= t)
    }
}

/// A master's data.
#[derive(Clone, Debug)]
pub struct MasterData {
    /// The master's name, as written.
    pub name: String,
    /// Where the data is, as the file writes it (`../masters/accounts.csv`).
    pub written: String,
    /// The file, as a diagnostic names it.
    pub file: String,
    /// The fields, by their names as written, in their order.
    pub fields: Vec<String>,
    pub rows: Vec<MRow>,
}

impl MasterData {
    /// The value of the field `field` (its name as written or its ASCII name) in `row`.
    pub fn value<'r>(&self, row: &'r MRow, field: &str) -> Option<&'r str> {
        self.fields.iter().position(|f| f == field).and_then(|i| row.values.get(i)).map(|s| s.as_str())
    }

    /// The first day some row may be chosen on; None when a row may be chosen from any day.
    pub fn first(&self) -> Option<(i64, &MRow)> {
        if self.rows.is_empty() || self.rows.iter().any(|r| r.from.is_none()) {
            return None;
        }
        self.rows.iter().filter_map(|r| r.from.map(|f| (f, r))).min_by_key(|(f, r)| (*f, r.line))
    }

    /// The last day some row may be chosen on, with the row; None when a row has no end.
    pub fn last(&self) -> Option<(i64, &MRow)> {
        if self.rows.iter().any(|r| r.to.is_none()) {
            return None;
        }
        self.rows.iter().filter_map(|r| r.to.map(|t| (t, r))).max_by_key(|(t, r)| (*t, std::cmp::Reverse(r.line)))
    }

    /// The days of `from..=to` (to None: no end) no row may be chosen on, as runs, each with the
    /// row that ends just before it and the row that starts just after it.
    pub fn holes(&self, from: i64, to: Option<i64>) -> Vec<Hole<'_>> {
        // the days rows cover, merged
        let mut spans: Vec<(i64, Option<i64>, &MRow)> = self.rows.iter().map(|r| (r.from.unwrap_or(i64::MIN), r.to, r)).collect();
        spans.sort_by_key(|(a, ..)| *a);
        let mut out = Vec::new();
        let mut at = from;
        let mut before: Option<&MRow> = None;
        for (a, b, r) in &spans {
            if to.is_some_and(|t| at > t) {
                break;
            }
            if *a > at {
                let end = match to {
                    Some(t) => (*a - 1).min(t),
                    None => *a - 1,
                };
                out.push(Hole { from: at, to: Some(end), before, after: Some(r) });
            }
            match b {
                None => return out,
                Some(b) => {
                    if b + 1 > at {
                        at = b + 1;
                        before = Some(r);
                    }
                }
            }
        }
        // past the last row's end: a hole only within the days asked about
        if let Some(t) = to
            && at <= t
        {
            out.push(Hole { from: at, to: Some(t), before, after: None });
        }
        out
    }
}

/// Days no row of a master may be chosen on.
#[derive(Clone, Debug)]
pub struct Hole<'m> {
    pub from: i64,
    pub to: Option<i64>,
    /// The row that ends the day before, when one does.
    pub before: Option<&'m MRow>,
    /// The row that starts the day after, when one does.
    pub after: Option<&'m MRow>,
}

/// The data of a masters' file, master by master; a master with no data (no `data` line) is not in
/// it.
#[derive(Clone, Debug, Default)]
pub struct MastersData {
    pub masters: Vec<MasterData>,
}

impl MastersData {
    pub fn master(&self, name: &str) -> Option<&MasterData> {
        self.masters.iter().find(|m| m.name == name)
    }

    /// What `ok` says of the data: the rows of each master, and what the file reads them against.
    pub fn facts(&self) -> Text {
        if self.masters.is_empty() {
            return tr!("データ: どのマスタにも `data` の行がありません", "data: no master has a `data` line");
        }
        let each: Vec<(String, usize)> = self.masters.iter().map(|m| (m.name.clone(), m.rows.len())).collect();
        let ja = each.iter().map(|(n, r)| format!("{n} {r} 行")).collect::<Vec<_>>().join("、");
        let en = each.iter().map(|(n, r)| if *r == 1 { format!("{n} 1 row") } else { format!("{n} {r} rows") }).collect::<Vec<_>>().join(", ");
        tr!("データ: {ja}", "data: {en}")
    }
}

/// How many Unicode code points a value of a master may be at the most (the key's is not held to
/// this: its length is its field's own).
const LONGEST_VALUE: usize = 1000;

fn e501(t: &Table, line: usize, msg: Text) -> Diag {
    Diag::error("E501", &t.file, line.max(1), 1, msg).source(&t.src)
}

/// The data of each master of `f` (at `path`, as given), read and checked: E501 where it does not
/// read, E505 for a person of a row who is not in `org` on some day the row is in effect.
pub fn read(f: &File, path: &str, src: &str, org: Option<&Org>, diags: &mut Vec<Diag>) -> MastersData {
    let mut out = MastersData::default();
    for m in &f.masters {
        let Some((written, span)) = &m.data else { continue };
        let fp = data::beside(path, written);
        let file = fp.display().to_string();
        if !ritsu_base::fs::is_file(&fp) {
            diags.push(Diag::error("E501", path, span.line, span.col, tr!("マスタ `{}` のデータ `{written}`（`{file}`）がありません", "The data `{written}` of the master `{}` (`{file}`) is not there", m.name.text; m.name.text)).source(src).note(tr!(
                "パスは、このファイルのあるディレクトリから書きます。",
                "A path is written from the directory of the file it is in."
            )));
            continue;
        }
        let t = match data::read_csv(&fp) {
            Ok(t) => t,
            Err(u) => {
                let text = ritsu_base::fs::read_to_string(&fp).unwrap_or_default();
                diags.push(Diag::error("E501", &file, u.line.max(1), 1, u.message).source(&text));
                continue;
            }
        };
        if let Some(md) = master(m, &t, org, diags) {
            out.masters.push(MasterData { written: written.clone(), ..md });
        }
    }
    out
}

/// A column of the table for the field: by its name as written, else its ASCII name (a table with
/// both is refused, E501); None for an optional field with no column, whose values are empty.
fn column(t: &Table, f: &Field) -> Option<usize> {
    t.col(&f.name.text).or_else(|| t.col(f.name.ascii()))
}

fn master(m: &crate::ast::Master, t: &Table, org: Option<&Org>, diags: &mut Vec<Diag>) -> Option<MasterData> {
    let before = diags.len();
    let names: Vec<String> = m.fields.iter().map(|f| f.name.text.clone()).collect();
    let all = || names.iter().map(|n| format!("`{n}`")).chain(["`valid_from`".to_string(), "`valid_to`".to_string()]).collect::<Vec<_>>().join(", ");
    for f in &m.fields {
        let n = &f.name.text;
        // one field in two columns: which one holds its values would be a guess
        if f.name.ascii() != n.as_str()
            && t.col(n).is_some()
            && t.col(f.name.ascii()).is_some()
        {
            let ascii = f.name.ascii();
            diags.push(e501(t, 1, tr!(
                "マスタ `{}` のフィールド `{n}` の列が、`{n}` と `{ascii}` の二つあります",
                "The field `{n}` of the master `{}` has two columns, `{n}` and `{ascii}`",
                m.name.text; m.name.text
            )).note(tr!("どちらか一つの名前の列だけにしてください。", "Keep one column, by either name.")));
        }
        // an optional field may have no column: each row's value is then empty (null)
        if column(t, f).is_none() && f.optional.is_none() {
            let a = all();
            diags.push(e501(t, 1, tr!("マスタ `{}` のフィールド `{n}` の列がありません", "There is no column for the field `{n}` of the master `{}`", m.name.text; m.name.text)).note(tr!("列は {a} です。", "The columns are {a}.")));
        }
    }
    for c in ["valid_from", "valid_to"] {
        if t.col(c).is_none() {
            diags.push(e501(t, 1, tr!("列 `{c}` がありません", "The column `{c}` is missing")).note(tr!(
                "マスタの行は、選べる最初の日と最後の日（`valid_from`、`valid_to`。空なら始まり無し、終わり無し）を持ちます。",
                "A master's row has the first and the last day it may be chosen on (`valid_from`, `valid_to`; empty for no start, no end)."
            )));
        }
    }
    for c in &t.columns {
        if c != "valid_from" && c != "valid_to" && !m.fields.iter().any(|f| &f.name.text == c || f.name.ascii() == c) {
            let a = all();
            diags.push(e501(t, 1, tr!("列 `{c}` は、マスタ `{}` のフィールドではありません", "The column `{c}` is not a field of the master `{}`", m.name.text; m.name.text)).note(tr!("列は {a} です。", "The columns are {a}.")));
        }
    }
    if diags.len() > before {
        return None;
    }
    let key = m.key.as_ref().and_then(|k| m.fields.iter().position(|f| f.name.text == k.text));
    let mut rows: Vec<MRow> = Vec::new();
    for r in &t.rows {
        if r.cells.len() != t.columns.len() {
            let (n, c) = (r.cells.len(), t.columns.len());
            diags.push(e501(t, r.line, tr!("この行の値は {n} 個で、列は {c} 個です", "The row has {n} values, and there are {c} columns")));
            continue;
        }
        let mut values = Vec::new();
        let row_before = diags.len();
        for f in &m.fields {
            let v = column(t, f).and_then(|i| r.cells.get(i)).cloned().unwrap_or_default();
            let n = &f.name.text;
            if let Some(why) = misfit(f, &v) {
                diags.push(e501(t, r.line, tr!("フィールド `{n}` の値 `{v}` が合いません: {}", "The value `{v}` of the field `{n}` does not fit: {}", why.ja; why.en)));
            }
            if let Some(what) = super::unfit(&v) {
                diags.push(e501(t, r.line, tr!("行の `{n}` に{}があります", "The `{n}` of the row holds {}", what.ja; what.en)));
            }
            if m.key.as_ref().is_some_and(|k| f.name.text == k.text) {
                if v.is_empty() {
                    diags.push(e501(t, r.line, tr!("行のキー `{n}` が空です", "The key `{n}` of the row is empty")));
                } else if v.len() > super::LONGEST_ID {
                    let (b, k, m) = (v.len(), super::escaped(&v), &m.name.text);
                    diags.push(e501(t, r.line, tr!("マスタ `{m}` のキー `{k}` が{b}バイトあります。キーは{}バイトまでです", "The key `{k}` of the master `{m}` is {b} bytes long; a key is {} bytes at the most", super::LONGEST_ID)));
                }
            } else {
                let c = v.chars().count();
                if c > LONGEST_VALUE {
                    diags.push(e501(t, r.line, tr!("行の `{n}` が{c}字あります。マスタの値は{LONGEST_VALUE}字までです", "The `{n}` of the row is {c} characters long; a value of a master is {LONGEST_VALUE} characters at the most")));
                }
            }
            values.push(v);
        }
        let date = |c: &str| -> Result<Option<i64>, ()> {
            let w = t.get(r, c);
            if w.is_empty() {
                return Ok(None);
            }
            ritsu_ports::day_of(w).map(Some).ok_or(())
        };
        let from = date("valid_from").unwrap_or_else(|_| {
            let w = t.get(r, "valid_from");
            diags.push(e501(t, r.line, tr!("`valid_from` は `2026-01-01` の形の日付か、空です。`{w}` ではありません", "`valid_from` is a date such as `2026-01-01`, or empty; not `{w}`")));
            None
        });
        let to = date("valid_to").unwrap_or_else(|_| {
            let w = t.get(r, "valid_to");
            diags.push(e501(t, r.line, tr!("`valid_to` は `2026-12-31` の形の日付か、空です。`{w}` ではありません", "`valid_to` is a date such as `2026-12-31`, or empty; not `{w}`")));
            None
        });
        if let (Some(a), Some(b)) = (from, to)
            && b < a
        {
            let (a, b) = (crate::lex::date_text(a), crate::lex::date_text(b));
            diags.push(e501(t, r.line, tr!("`valid_to`（{b}）が `valid_from`（{a}）より前です", "`valid_to` ({b}) is before `valid_from` ({a})")));
        }
        if diags.len() > row_before {
            continue;
        }
        let k = key.and_then(|i| values.get(i)).cloned().unwrap_or_default();
        let row = MRow { line: r.line, key: k, values, from, to };
        // two rows of one key in effect on one day
        if let Some(o) = rows.iter().find(|o| o.key == row.key && overlaps(o, &row)) {
            let (k, l) = (&row.key, o.line);
            diags.push(e501(t, r.line, tr!("キー `{k}` の行が、{l} 行目の行と同じ日に効きます", "The row of the key `{k}` is in effect on days the row of line {l} is too")).note(tr!(
                "一つのキーには、一つの日に一つの行だけが効くようにしてください（前の行の `valid_to` を、この行の `valid_from` の前の日にします）。",
                "On one day, one row of a key is in effect: end the earlier row (`valid_to`) the day before this one starts (`valid_from`)."
            )));
            continue;
        }
        rows.push(row);
    }
    // a person of a row who is not in the organization on some day the row is in effect
    if let Some(org) = org {
        for (i, f) in m.fields.iter().enumerate() {
            if f.ty != TypeExpr::Person {
                continue;
            }
            for row in &rows {
                let Some(id) = row.values.get(i).filter(|v| !v.is_empty()) else { continue };
                for v in &org.versions {
                    let (a, b) = org.period(v);
                    let meets = row.to.is_none_or(|t| t >= a) && b.is_none_or(|e| row.from.is_none_or(|f| f <= e));
                    if !meets || v.person(id).is_some() {
                        continue;
                    }
                    let (n, day, key) = (&f.name.text, v.day(), &row.key);
                    // ending the row the day before the version keeps its days before it; a row
                    // that starts in the version has none to keep
                    let fix = if row.from.is_none_or(|f| f < v.from) {
                        let before = crate::lex::date_text(v.from - 1);
                        tr!(
                            "人が辞めたなら、この行の `valid_to` を、{day} からの組織の前の日の {before} にして、{day} からの新しい人の行を足してください。ID の誤りなら、ID を直してください。",
                            "If the person has left, make this row's `valid_to` {before}, the day before the organization from {day}, and add a row with the new person from {day}; if the ID is wrong, correct it."
                        )
                    } else {
                        tr!("人が辞めたなら、この行に新しい人を書いてください。ID の誤りなら、ID を直してください。", "If the person has left, write the new person in this row; if the ID is wrong, correct it.")
                    };
                    diags.push(
                        Diag::error("E505", &t.file, row.line, 1, tr!(
                            "マスタ `{}` の行 `{key}` の `{n}` の `{id}` は、{day} からの組織にいません",
                            "`{id}`, the `{n}` of the row `{key}` of the master `{}`, is not in the organization from {day}",
                            m.name.text; m.name.text
                        ))
                        .source(&t.src)
                        .note(tr!(
                            "この行は{}選べ、{day} からの組織が効く日と重なります。",
                            "The row may be chosen {}, which overlaps the days of the organization from {day}.",
                            row_days_ja_adverb(row); row_days(row).en
                        ))
                        .note(fix),
                    );
                }
            }
        }
    }
    if diags.len() > before {
        return None;
    }
    Some(MasterData { name: m.name.text.clone(), written: String::new(), file: t.file.clone(), fields: names, rows })
}

/// The days a row may be chosen on, as a text.
pub fn row_days(r: &MRow) -> Text {
    let a = r.from.map(crate::lex::date_text);
    let b = r.to.map(crate::lex::date_text);
    match (a, b) {
        (Some(a), Some(b)) => Text::new(format!("{a}〜{b}"), format!("from {a} to {b}")),
        (Some(a), None) => Text::new(format!("{a} から終わり無し"), format!("from {a} with no end")),
        (None, Some(b)) => Text::new(format!("{b} まで"), format!("up to {b}")),
        (None, None) => Text::new("いつでも", "on any day"),
    }
}

/// The days a row may be chosen on, as the Japanese that goes before 選べ (may be chosen), with the
/// space before a date: ` 2026-04-01〜2026-06-30 に`, ` 2026-04-01 から終わり無く`, `いつでも`.
fn row_days_ja_adverb(r: &MRow) -> String {
    let a = r.from.map(crate::lex::date_text);
    let b = r.to.map(crate::lex::date_text);
    match (a, b) {
        (Some(a), Some(b)) => format!(" {a}〜{b} に"),
        (Some(a), None) => format!(" {a} から終わり無く"),
        (None, Some(b)) => format!(" {b} まで"),
        (None, None) => "いつでも".to_string(),
    }
}

fn overlaps(a: &MRow, b: &MRow) -> bool {
    let (a0, a1) = (a.from.unwrap_or(i64::MIN), a.to.unwrap_or(i64::MAX));
    let (b0, b1) = (b.from.unwrap_or(i64::MIN), b.to.unwrap_or(i64::MAX));
    a0 <= b1 && b0 <= a1
}

/// Why a value of a row does not fit its field, when it does not.
fn misfit(f: &Field, v: &str) -> Option<Text> {
    if v.is_empty() {
        return (f.optional.is_none()).then(|| tr!("空です", "it is empty"));
    }
    match &f.ty {
        TypeExpr::Text | TypeExpr::LongText => {
            let n = v.chars().count() as i128;
            let b = f.count.as_ref()?;
            let end = |x: &Option<crate::ast::Bound>| x.as_ref().and_then(|b| crate::lower::words::constant(&b.value).map(|c| (c.rat(), b.inclusive)));
            if let Some((hi, inc)) = end(&b.hi)
                && hi.is_int()
                && (n > hi.num || (!inc && n == hi.num))
            {
                let h = hi.num;
                return Some(tr!("{n} 文字で、上限（{h}）を超えます", "it is {n} characters long, past the limit ({h})"));
            }
            if let Some((lo, inc)) = end(&b.lo)
                && lo.is_int()
                && (n < lo.num || (!inc && n == lo.num))
            {
                let l = lo.num;
                return Some(tr!("{n} 文字で、下限（{l}）に足りません", "it is {n} characters long, short of the least ({l})"));
            }
            None
        }
        TypeExpr::Date => ritsu_ports::day_of(v).is_none().then(|| tr!("`2026-01-01` の形の日付ではありません", "it is not a date such as `2026-01-01`")),
        TypeExpr::Bool => (!matches!(v, "yes" | "no" | "true" | "false")).then(|| tr!("`yes` か `no` ではありません", "it is not `yes` or `no`")),
        TypeExpr::Choice => (!f.options.iter().any(|o| o.name.text == v || o.name.ascii() == v)).then(|| {
            let vs = f.options.iter().map(|o| o.name.text.clone()).collect::<Vec<_>>().join(", ");
            tr!("選べる値（{vs}）のどれでもありません", "it is none of the values ({vs})")
        }),
        _ => None,
    }
}
