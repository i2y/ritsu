//! TypeScript (DESIGN 6.2): `typescript/<alias>.ts`, dates as `"YYYY-MM-DD"` strings, a
//! `KoyomiError` with a `kind`. Written so that Node runs it by stripping the types alone: no
//! `enum`, no `namespace` with code in it, no parameter properties (`tsc --erasableSyntaxOnly`
//! holds it to that).

use super::{Call, H, Msg, Num, Piece, Unit, at_doc, date_doc, day_text, is_open_doc, message};
use crate::ast::{Kind, Ty};
use crate::naming::Target;

const T: Target = Target::TypeScript;

fn lit(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// A message as a string expression: the pieces joined with `+`.
fn msg(u: &Unit, m: Msg) -> String {
    message(u.lang, m)
        .iter()
        .map(|p| match p {
            Piece::Lit(s) => lit(s),
            Piece::Var(v) => v.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn num(u: &Unit, n: &Num) -> String {
    match n {
        Num::Lit(v) => v.to_string(),
        Num::Input(1, k) => u.inputs[*k].alias.clone(),
        Num::Input(-1, k) => format!("-{}", u.inputs[*k].alias),
        Num::Input(f, k) => format!("{f} * {}", u.inputs[*k].alias),
    }
}

fn call(u: &Unit, c: &Call) -> String {
    match c {
        Call::AddDays(n) => format!("_addDays(day, {})", num(u, n)),
        Call::AddBusiness(n, fwd) => format!("_addBusiness(day, {}, {fwd})", num(u, n)),
        Call::AddMonths(k, m) => format!("_addMonths(day, {}, {})", num(u, k), lit(m)),
        Call::DayOfMonth(n, k, m) => format!("_dayOfMonth(day, {}, {}, {})", num(u, n), num(u, k), lit(m)),
        Call::StartOfMonth(k) => format!("_startOfMonth(day, {})", num(u, k)),
        Call::EndOfMonth(k) => format!("_endOfMonth(day, {})", num(u, k)),
        Call::CloseDay(n, m) => format!("_closeDay(day, {}, {})", num(u, n), lit(m)),
        Call::CloseEndOfMonth => "_closeEndOfMonth(day)".into(),
        Call::Roll(c) => format!("_roll(day, {})", lit(c)),
        Call::IfClosed(inner) => call(u, inner),
    }
}

fn doc(lines: &[String], indent: &str) -> String {
    if lines.len() == 1 {
        return format!("{indent}/** {} */\n", lines[0]);
    }
    let mut o = format!("{indent}/**\n");
    for l in lines {
        o.push_str(&format!("{indent} * {l}\n"));
    }
    o.push_str(&format!("{indent} */\n"));
    o
}

fn calendar(u: &Unit) -> String {
    let Some(c) = &u.cal else { return String::new() };
    let mut o = format!("// {}\n", u.t(tr!("カレンダー「{}」", "The calendar {}", c.name)));
    o.push_str(&format!("const _DATA_FROM = {}; // {}\n", c.data.0, day_text(c.data.0)));
    o.push_str(&format!("const _DATA_TO = {}; // {}\n", c.data.1, day_text(c.data.1)));
    if c.weekly.iter().any(|w| *w) {
        let ws: Vec<String> = c.weekly.iter().map(|w| w.to_string()).collect();
        let names: Vec<&str> = (0..7).filter(|d| c.weekly[*d]).map(|d| crate::kw::WEEKDAYS[d]).collect();
        o.push_str(&format!("const _WEEKLY: readonly boolean[] = [{}]; // {}\n", ws.join(", "), super::weekly_comment(u, &names)));
    }
    let pairs = |name: &str, v: &[(i64, i64, String)], o: &mut String| {
        if v.is_empty() {
            return;
        }
        o.push_str(&format!("const {name}: ReadonlyArray<readonly [number, number]> = [\n"));
        for (a, b, t) in v {
            o.push_str(&format!("  [{a}, {b}], // {t}\n"));
        }
        o.push_str("];\n");
    };
    pairs("_OPENS", &c.opens.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("open {t}"))).collect::<Vec<_>>(), &mut o);
    pairs("_EVERY", &c.every.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("closed {t}"))).collect::<Vec<_>>(), &mut o);
    pairs("_DAYS", &c.days.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("closed {t}"))).collect::<Vec<_>>(), &mut o);
    if !c.holidays.is_empty() {
        o.push_str("const _HOLIDAYS: ReadonlySet<number> = new Set([\n");
        for (z, t) in &c.holidays {
            o.push_str(&format!("  {z}, // {t}\n"));
        }
        o.push_str("]);\n");
    }
    o.push('\n');
    o
}

fn helpers(u: &Unit) -> String {
    let mut o = String::new();
    o.push_str(&format!("// {}\n", u.t(tr!(
        "日付は 1970-01-01 からの通算日で計算する。どの関数も koyomi の date.rs と calendar.rs の同じ名前の手順を写したもの",
        "Dates are computed as days since 1970-01-01. Each function follows the procedure of the same name in koyomi's date.rs and calendar.rs"
    ))));
    o.push_str("const _MIN = -719162; // 0001-01-01\nconst _MAX = 2932896; // 9999-12-31\n\n");
    o.push_str(
        r#"function _daysFromCivil(y: number, m: number, d: number): number {
  const yy = m <= 2 ? y - 1 : y;
  const era = Math.floor(yy / 400);
  const yoe = yy - era * 400;
  const mp = (m + 9) % 12;
  const doy = Math.floor((153 * mp + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

function _civilFromDays(z: number): [number, number, number] {
  const zz = z + 719468;
  const era = Math.floor(zz / 146097);
  const doe = zz - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const y = yoe + era * 400;
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const m = mp < 10 ? mp + 3 : mp - 9;
  return [m <= 2 ? y + 1 : y, m, d];
}

function _isLeap(y: number): boolean {
  return y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);
}

function _monthLen(y: number, m: number): number {
  if (m === 2) return _isLeap(y) ? 29 : 28;
  return m === 4 || m === 6 || m === 9 || m === 11 ? 30 : 31;
}

function _ymd(y: number, m: number, d: number): string {
  return String(y).padStart(4, "0") + "-" + String(m).padStart(2, "0") + "-" + String(d).padStart(2, "0");
}

function _formatDate(z: number): string {
  const [y, m, d] = _civilFromDays(z);
  return _ymd(y, m, d);
}

"#,
    );
    o.push_str(&format!(
        r#"function _parseDate(text: string): number {{
  const p = /^(\d{{4}})-(\d{{2}})-(\d{{2}})$/.exec(text);
  if (p !== null) {{
    const y = Number(p[1]), m = Number(p[2]), d = Number(p[3]);
    if (y >= 1 && m >= 1 && m <= 12 && d >= 1 && d <= _monthLen(y, m)) return _daysFromCivil(y, m, d);
  }}
  throw new KoyomiError("date", {not_a_date});
}}

"#,
        not_a_date = msg(u, Msg::NotADate)
    ));
    if u.kind == Kind::Dates && !u.dates.is_empty() {
        o.push_str(&format!(
            r#"function _inputDate(text: string, name: string, from: number, to: number): number {{
  const z = _parseDate(text);
  if (z < from || z > to) {{
    const value = text, lo = _formatDate(from), hi = _formatDate(to);
    throw new KoyomiError("range", {range});
  }}
  return z;
}}

"#,
            range = msg(u, Msg::Range)
        ));
    }
    if u.has_ints() {
        o.push_str(&format!(
            r#"function _inputInt(v: number, name: string, from: number, to: number): void {{
  if (!Number.isInteger(v) || v < from || v > to) {{
    const value = String(v), lo = String(from), hi = String(to);
    throw new KoyomiError("range", {range});
  }}
}}

"#,
            range = msg(u, Msg::Range)
        ));
    }
    if u.has(H::ShiftMonth) {
        o.push_str(
            r#"function _shiftMonth(y: number, m: number, k: number): [number, number] {
  const t = y * 12 + (m - 1) + k;
  const y2 = Math.floor(t / 12);
  return [y2, t - y2 * 12 + 1];
}

"#,
        );
    }
    if u.has(H::Place) {
        o.push_str(&format!(
            r#"function _place(y: number, m: number, d: number, mode: string): number {{
  if (y < 1 || y > 9999) throw new KoyomiError("date", {outside});
  const len = _monthLen(y, m);
  if (d <= len) return _daysFromCivil(y, m, d);
  if (mode === "end_of_month") return _daysFromCivil(y, m, len);
  if (mode === "start_of_next_month") {{
    const [ny, nm] = _shiftMonth(y, m, 1);
    if (ny < 1 || ny > 9999) throw new KoyomiError("date", {outside});
    return _daysFromCivil(ny, nm, 1);
  }}
  const day = _ymd(y, m, d);
  if (mode === "reject") throw new KoyomiError("reject", {reject});
  throw new Error({bug});
}}

"#,
            outside = msg(u, Msg::Outside),
            reject = msg(u, Msg::Reject),
            bug = msg(u, Msg::Bug)
        ));
    }
    if u.has(H::AddDays) {
        o.push_str(&format!(
            r#"function _addDays(z: number, n: number): number {{
  const r = z + n;
  if (r < _MIN || r > _MAX) throw new KoyomiError("date", {outside});
  return r;
}}

"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    if u.has(H::AddMonths) {
        o.push_str(
            r#"function _addMonths(z: number, k: number, mode: string): number {
  const [y, m, d] = _civilFromDays(z);
  const [y2, m2] = _shiftMonth(y, m, k);
  return _place(y2, m2, d, mode);
}

"#,
        );
    }
    if u.has(H::DayOfMonth) {
        o.push_str(
            r#"function _dayOfMonth(z: number, n: number, k: number, mode: string): number {
  const [y, m] = _civilFromDays(z);
  const [y2, m2] = _shiftMonth(y, m, k);
  return _place(y2, m2, n, mode);
}

"#,
        );
    }
    if u.has(H::StartOfMonth) {
        o.push_str(
            r#"function _startOfMonth(z: number, k: number): number {
  const [y, m] = _civilFromDays(z);
  const [y2, m2] = _shiftMonth(y, m, k);
  return _place(y2, m2, 1, "none");
}

"#,
        );
    }
    if u.has(H::EndOfMonth) {
        o.push_str(&format!(
            r#"function _endOfMonth(z: number, k: number): number {{
  const [y, m] = _civilFromDays(z);
  const [y2, m2] = _shiftMonth(y, m, k);
  if (y2 < 1 || y2 > 9999) throw new KoyomiError("date", {outside});
  return _daysFromCivil(y2, m2, _monthLen(y2, m2));
}}

"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    if u.has(H::CloseDay) {
        o.push_str(&format!(
            r#"function _closeDay(z: number, n: number, mode: string): number {{
  const [y, m] = _civilFromDays(z);
  for (let k = mode === "start_of_next_month" ? -1 : 0; k <= 1; k++) {{
    const [y2, m2] = _shiftMonth(y, m, k);
    if (y2 < 1 || y2 > 9999) {{
      if (k < 0) continue;
      throw new KoyomiError("date", {outside});
    }}
    const c = _place(y2, m2, n, mode);
    if (c >= z) return c;
  }}
  throw new Error({bug});
}}

"#,
            outside = msg(u, Msg::Outside),
            bug = msg(u, Msg::Bug)
        ));
    }
    if u.has(H::CloseEndOfMonth) {
        o.push_str(
            r#"function _closeEndOfMonth(z: number): number {
  const [y, m] = _civilFromDays(z);
  return _daysFromCivil(y, m, _monthLen(y, m));
}

"#,
        );
    }
    if u.has(H::Weekday) {
        o.push_str(
            r#"function _weekday(z: number): number {
  return (((z + 3) % 7) + 7) % 7;
}

"#,
        );
    }
    if u.has(H::IsOpen) {
        let c = u.cal.as_ref().unwrap();
        let mut body = String::new();
        if !c.opens.is_empty() {
            body.push_str("  for (const [a, b] of _OPENS) if (a <= z && z <= b) return true;\n");
        }
        if c.weekly.iter().any(|w| *w) {
            body.push_str("  if (_WEEKLY[_weekday(z)]) return false;\n");
        }
        if !c.holidays.is_empty() {
            body.push_str("  if (_HOLIDAYS.has(z)) return false;\n");
        }
        if !c.every.is_empty() {
            body.push_str("  const [, m, d] = _civilFromDays(z);\n  const md = m * 100 + d;\n");
            body.push_str("  for (const [a, b] of _EVERY) if (a <= b ? a <= md && md <= b : md >= a || md <= b) return false;\n");
        }
        if !c.days.is_empty() {
            body.push_str("  for (const [a, b] of _DAYS) if (a <= z && z <= b) return false;\n");
        }
        o.push_str(&format!(
            r#"function _isOpen(z: number): boolean {{
  if (z < _DATA_FROM || z > _DATA_TO) {{
    const day = _formatDate(z), lo = _formatDate(_DATA_FROM), hi = _formatDate(_DATA_TO);
    throw new KoyomiError("data", {data});
  }}
{body}  return true;
}}

"#,
            data = msg(u, Msg::Data)
        ));
    }
    if u.has(H::Seek) {
        o.push_str(
            r#"function _seek(z: number, forward: boolean): number {
  let x = z;
  while (!_isOpen(x)) x = _addDays(x, forward ? 1 : -1);
  return x;
}

"#,
        );
    }
    if u.has(H::Roll) {
        o.push_str(
            r#"function _sameMonth(a: number, b: number): boolean {
  const [ya, ma] = _civilFromDays(a);
  const [yb, mb] = _civilFromDays(b);
  return ya === yb && ma === mb;
}

function _roll(z: number, convention: string): number {
  if (convention === "following") return _seek(z, true);
  if (convention === "preceding") return _seek(z, false);
  if (convention === "modified_following") {
    const f = _seek(z, true);
    return _sameMonth(f, z) ? f : _seek(z, false);
  }
  const p = _seek(z, false);
  return _sameMonth(p, z) ? p : _seek(z, true);
}

"#,
        );
    }
    if u.has(H::AddBusiness) {
        o.push_str(
            r#"function _addBusiness(z: number, n: number, forward: boolean): number {
  if (n === 0) return _seek(z, forward);
  let x = z;
  for (let k = 0; k < n; ) {
    x = _addDays(x, forward ? 1 : -1);
    if (_isOpen(x)) k++;
  }
  return x;
}

"#,
        );
    }
    if u.has(H::At) {
        o.push_str(&format!(
            r#"function _at(day: string, minutes: number, offset: number): string {{
  const local = _parseDate(day) * 1440 + minutes;
  const utc = local - offset;
  for (const v of [local, utc]) {{
    const d = Math.floor(v / 1440);
    if (d < _MIN || d > _MAX) throw new KoyomiError("date", {outside});
  }}
  const ud = Math.floor(utc / 1440), um = utc - ud * 1440;
  return _formatDate(ud) + "T" + String(Math.floor(um / 60)).padStart(2, "0") + ":" + String(um % 60).padStart(2, "0") + ":00Z";
}}

"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    o
}

fn functions(u: &Unit) -> String {
    let mut o = String::new();
    for d in &u.dates {
        let params: Vec<(String, Ty)> = d.params.iter().map(|k| (u.inputs[*k].alias.clone(), u.inputs[*k].ty)).collect();
        let fname = T.function(&d.alias);
        o.push_str(&doc(&date_doc(u, d, None), ""));
        o.push_str(&format!("{} {{\n", T.signature(&u.alias, &d.alias, &params, false)));
        let mut lines: Vec<(String, String)> = Vec::new();
        let di = u.date_input();
        let first = &u.inputs[di];
        let mutable = !d.steps.is_empty();
        lines.push((
            format!("{} day = _inputDate({}, {}, {}, {});", if mutable { "let" } else { "const" }, first.alias, lit(&first.name), first.lo, first.hi),
            first.range_text(),
        ));
        for k in &d.params {
            let i = &u.inputs[*k];
            if i.ty == Ty::Int {
                lines.push((format!("_inputInt({}, {}, {}, {});", i.alias, lit(&i.name), i.lo, i.hi), i.range_text()));
            }
        }
        for s in &d.steps {
            match &s.call {
                Call::IfClosed(inner) => lines.push((format!("if (!_isOpen(day)) day = {};", call(u, inner)), s.comment.clone())),
                c => lines.push((format!("day = {};", call(u, c)), s.comment.clone())),
            }
        }
        for (code, comment) in &lines {
            o.push_str(&format!("  {code} // {comment}\n"));
        }
        o.push_str("  return _formatDate(day);\n}\n\n");
        if let Some((minutes, _)) = &d.at {
            let at = crate::naming::at_alias(&d.alias);
            let off = u.cal.as_ref().and_then(|c| c.offset).unwrap_or(0);
            o.push_str(&doc(&at_doc(u, d, None, &fname), ""));
            o.push_str(&format!("{} {{\n", T.signature(&u.alias, &at, &params, true)));
            let args: Vec<String> = params.iter().map(|(a, _)| a.clone()).collect();
            o.push_str(&format!("  return _at({fname}({}), {minutes}, {off});\n}}\n\n", args.join(", ")));
        }
    }
    if u.cal.is_some() {
        o.push_str(&doc(&is_open_doc(u, None), ""));
        o.push_str(&format!("{} {{\n  return _isOpen(_parseDate(day));\n}}\n", T.is_open(&u.alias)));
    }
    o
}

/// `typescript/<alias>.ts`.
pub fn module(u: &Unit) -> String {
    let mut o = String::new();
    for h in &u.header {
        o.push_str(&format!("// {h}\n"));
    }
    o.push('\n');
    o.push_str(&doc(
        &[u.t(tr!(
            "エラーの種類。range は入力が範囲の外、data はカレンダーが知らない日、reject は else reject の無い日、date は 0001-01-01〜9999-12-31 の外か日付でない値",
            "The kinds of KoyomiError: range, an input outside its range; data, a day the calendar does not know; reject, a day the month does not have under else reject; date, outside 0001-01-01..9999-12-31 or not a date"
        ))],
        "",
    ));
    o.push_str("export type KoyomiErrorKind = \"range\" | \"data\" | \"reject\" | \"date\";\n\n");
    o.push_str(&doc(&[u.t(tr!("この生成物の関数が投げるエラー", "What the functions of this file throw"))], ""));
    o.push_str(
        r#"export class KoyomiError extends Error {
  readonly kind: KoyomiErrorKind;
  constructor(kind: KoyomiErrorKind, message: string) {
    super(message);
    this.name = "KoyomiError";
    this.kind = kind;
  }
}

"#,
    );
    o.push_str(&calendar(u));
    o.push_str(&helpers(u));
    o.push_str(&functions(u));
    o
}

/// `typescript/<alias>_runner.ts`: reads `koyomi vectors` on standard input and prints, a line
/// an input, the values the functions give, separated by a space, or `error <kind>`. The module
/// is imported as a namespace, so no function of it can meet a name of the runner.
pub fn runner(u: &Unit) -> String {
    let mut o = String::new();
    o.push_str(&format!("// {}\n", u.header[0]));
    o.push_str(&format!("// {}\n", u.t(tr!(
        "koyomi vectors の行を標準入力から読み、関数の結果を一行ずつ書く（空白で区切る。エラーなら error <種類>）",
        "Reads the lines of koyomi vectors on standard input and writes what the functions give, a line each (separated by a space; error <kind> for an error)"
    ))));
    let mut body = String::new();
    if u.kind == Kind::Calendar {
        body.push_str("  return String(m.is_open(input[\"date\"] as string));\n");
    } else {
        body.push_str("  const out: string[] = [];\n");
        for (k, at) in u.outputs() {
            let d = &u.dates[k];
            let f = if at { T.function(&crate::naming::at_alias(&d.alias)) } else { T.function(&d.alias) };
            let args: Vec<String> = d
                .params
                .iter()
                .map(|p| {
                    let i = &u.inputs[*p];
                    match i.ty {
                        Ty::Date => format!("input[{}] as string", lit(&i.name)),
                        Ty::Int => format!("input[{}] as number", lit(&i.name)),
                    }
                })
                .collect();
            body.push_str(&format!("  out.push(m.{f}({}));\n", args.join(", ")));
        }
        body.push_str("  return out.join(\" \");\n");
    }
    o.push_str(&format!(
        r#"import {{ createInterface }} from "node:readline";
import * as m from "./{}.ts";

function run(input: Record<string, unknown>): string {{
{body}}}

let buf: string[] = [];
for await (const line of createInterface({{ input: process.stdin, crlfDelay: Infinity }})) {{
  if (line === "") continue;
  const input = (JSON.parse(line) as {{ in: Record<string, unknown> }}).in;
  try {{
    buf.push(run(input));
  }} catch (e) {{
    if (!(e instanceof m.KoyomiError)) throw e;
    buf.push("error " + e.kind);
  }}
  if (buf.length >= 4096) {{
    process.stdout.write(buf.join("\n") + "\n");
    buf = [];
  }}
}}
if (buf.length > 0) process.stdout.write(buf.join("\n") + "\n");
"#,
        u.alias
    ));
    o
}
