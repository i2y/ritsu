//! Python (DESIGN 6.2): `python/<alias>.py`, dates as `datetime.date`, a `KoyomiError` (a
//! `ValueError`) with a `kind`. Every function is annotated; `mypy --strict` holds it to that.
//! The date type is used only to take a date in and to hand one back: year, month and day go
//! to day numbers and back by the same procedures as the other targets.

use super::{Call, H, Msg, Num, Piece, Unit, at_doc, date_doc, day_text, is_open_doc, message};
use crate::ast::{Kind, Ty};
use crate::naming::Target;

const T: Target = Target::Python;

fn lit(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// A message as an f-string, or a plain string when it has no values in it.
fn msg(u: &Unit, m: Msg) -> String {
    let pieces = message(u.lang, m);
    if pieces.iter().all(|p| matches!(p, Piece::Lit(_))) {
        let text: String = pieces.iter().map(|p| if let Piece::Lit(t) = p { *t } else { "" }).collect();
        return lit(&text);
    }
    let mut s = String::from("f\"");
    for p in message(u.lang, m) {
        match p {
            Piece::Lit(t) => {
                let q = lit(t);
                s.push_str(&q[1..q.len() - 1].replace('{', "{{").replace('}', "}}"));
            }
            Piece::Var(v) => s.push_str(&format!("{{{v}}}")),
        }
    }
    s.push('"');
    s
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
    let b = |v: bool| if v { "True" } else { "False" };
    match c {
        Call::AddDays(n) => format!("_add_days(day, {})", num(u, n)),
        Call::AddBusiness(n, fwd) => format!("_add_business(day, {}, {})", num(u, n), b(*fwd)),
        Call::AddMonths(k, m) => format!("_add_months(day, {}, {})", num(u, k), lit(m)),
        Call::DayOfMonth(n, k, m) => format!("_day_of_month(day, {}, {}, {})", num(u, n), num(u, k), lit(m)),
        Call::StartOfMonth(k) => format!("_start_of_month(day, {})", num(u, k)),
        Call::EndOfMonth(k) => format!("_end_of_month(day, {})", num(u, k)),
        Call::CloseDay(n, m) => format!("_close_day(day, {}, {})", num(u, n), lit(m)),
        Call::CloseEndOfMonth => "_close_end_of_month(day)".into(),
        Call::Roll(c) => format!("_roll(day, {})", lit(c)),
        Call::IfClosed(inner) => call(u, inner),
    }
}

fn docstring(lines: &[String], indent: &str) -> String {
    if lines.len() == 1 {
        return format!("{indent}\"\"\"{}\"\"\"\n", lines[0]);
    }
    let mut o = format!("{indent}\"\"\"{}\n\n", lines[0]);
    for l in &lines[1..] {
        o.push_str(&format!("{indent}{l}\n"));
    }
    o.push_str(&format!("{indent}\"\"\"\n"));
    o
}

fn calendar(u: &Unit) -> String {
    let Some(c) = &u.cal else { return String::new() };
    let mut o = format!("# {}\n", u.t(tr!("カレンダー「{}」", "The calendar {}", c.name)));
    o.push_str(&format!("_DATA_FROM = {}  # {}\n", c.data.0, day_text(c.data.0)));
    o.push_str(&format!("_DATA_TO = {}  # {}\n", c.data.1, day_text(c.data.1)));
    if c.weekly.iter().any(|w| *w) {
        let ws: Vec<&str> = c.weekly.iter().map(|w| if *w { "True" } else { "False" }).collect();
        let names: Vec<&str> = (0..7).filter(|d| c.weekly[*d]).map(|d| crate::kw::WEEKDAYS[d]).collect();
        o.push_str(&format!("_WEEKLY = ({})  # {}\n", ws.join(", "), super::weekly_comment(u, &names)));
    }
    let pairs = |name: &str, v: &[(i64, i64, String)], o: &mut String| {
        if v.is_empty() {
            return;
        }
        o.push_str(&format!("{name}: tuple[tuple[int, int], ...] = (\n"));
        for (a, b, t) in v {
            o.push_str(&format!("    ({a}, {b}),  # {t}\n"));
        }
        o.push_str(")\n");
    };
    pairs("_OPENS", &c.opens.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("open {t}"))).collect::<Vec<_>>(), &mut o);
    pairs("_EVERY", &c.every.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("closed {t}"))).collect::<Vec<_>>(), &mut o);
    pairs("_DAYS", &c.days.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("closed {t}"))).collect::<Vec<_>>(), &mut o);
    if !c.holidays.is_empty() {
        o.push_str("_HOLIDAYS = frozenset((\n");
        for (z, t) in &c.holidays {
            o.push_str(&format!("    {z},  # {t}\n"));
        }
        o.push_str("))\n");
    }
    o.push_str("\n\n");
    o
}

fn helpers(u: &Unit) -> String {
    let mut o = String::new();
    o.push_str(&format!(
        "# {}\n",
        u.t(tr!(
            "日付は 1970-01-01 からの通算日で計算する。どの関数も koyomi の date.rs と calendar.rs の同じ名前の手順を写したもの",
            "Dates are computed as days since 1970-01-01. Each function follows the procedure of the same name in koyomi's date.rs and calendar.rs"
        ))
    ));
    o.push_str("_MIN = -719162  # 0001-01-01\n_MAX = 2932896  # 9999-12-31\n\n\n");
    o.push_str(
        r#"def _days_from_civil(y: int, m: int, d: int) -> int:
    yy = y - 1 if m <= 2 else y
    era = yy // 400
    yoe = yy - era * 400
    mp = (m + 9) % 12
    doy = (153 * mp + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468


def _civil_from_days(z: int) -> tuple[int, int, int]:
    zz = z + 719468
    era = zz // 146097
    doe = zz - era * 146097
    yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
    y = yoe + era * 400
    doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
    mp = (5 * doy + 2) // 153
    d = doy - (153 * mp + 2) // 5 + 1
    m = mp + 3 if mp < 10 else mp - 9
    return (y + 1 if m <= 2 else y, m, d)


def _is_leap(y: int) -> bool:
    return y % 4 == 0 and (y % 100 != 0 or y % 400 == 0)


def _month_len(y: int, m: int) -> int:
    if m == 2:
        return 29 if _is_leap(y) else 28
    return 30 if m in (4, 6, 9, 11) else 31


def _ymd(y: int, m: int, d: int) -> str:
    return f"{y:04d}-{m:02d}-{d:02d}"


def _from_date(day: date) -> int:
    return _days_from_civil(day.year, day.month, day.day)


def _to_date(z: int) -> date:
    y, m, d = _civil_from_days(z)
    return date(y, m, d)


def _format_date(z: int) -> str:
    return _ymd(*_civil_from_days(z))


"#,
    );
    if u.kind == Kind::Dates && !u.dates.is_empty() {
        o.push_str(&format!(
            r#"def _input_date(day: date, name: str, start: int, end: int) -> int:
    z = _from_date(day)
    if z < start or z > end:
        value, lo, hi = _format_date(z), _format_date(start), _format_date(end)
        raise KoyomiError("range", {range})
    return z


"#,
            range = msg(u, Msg::Range)
        ));
    }
    if u.has_ints() {
        o.push_str(&format!(
            r#"def _input_int(v: int, name: str, start: int, end: int) -> None:
    if isinstance(v, bool) or not isinstance(v, int) or v < start or v > end:
        value, lo, hi = str(v), str(start), str(end)
        raise KoyomiError("range", {range})


"#,
            range = msg(u, Msg::Range)
        ));
    }
    if u.has(H::ShiftMonth) {
        o.push_str(
            r#"def _shift_month(y: int, m: int, k: int) -> tuple[int, int]:
    t = y * 12 + (m - 1) + k
    y2 = t // 12
    return (y2, t - y2 * 12 + 1)


"#,
        );
    }
    if u.has(H::Place) {
        o.push_str(&format!(
            r#"def _place(y: int, m: int, d: int, mode: str) -> int:
    if y < 1 or y > 9999:
        raise KoyomiError("date", {outside})
    n = _month_len(y, m)
    if d <= n:
        return _days_from_civil(y, m, d)
    if mode == "end_of_month":
        return _days_from_civil(y, m, n)
    if mode == "start_of_next_month":
        ny, nm = _shift_month(y, m, 1)
        if ny < 1 or ny > 9999:
            raise KoyomiError("date", {outside})
        return _days_from_civil(ny, nm, 1)
    day = _ymd(y, m, d)
    if mode == "reject":
        raise KoyomiError("reject", {reject})
    raise AssertionError({bug})


"#,
            outside = msg(u, Msg::Outside),
            reject = msg(u, Msg::Reject),
            bug = msg(u, Msg::Bug)
        ));
    }
    if u.has(H::AddDays) {
        o.push_str(&format!(
            r#"def _add_days(z: int, n: int) -> int:
    r = z + n
    if r < _MIN or r > _MAX:
        raise KoyomiError("date", {outside})
    return r


"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    if u.has(H::AddMonths) {
        o.push_str(
            r#"def _add_months(z: int, k: int, mode: str) -> int:
    y, m, d = _civil_from_days(z)
    y2, m2 = _shift_month(y, m, k)
    return _place(y2, m2, d, mode)


"#,
        );
    }
    if u.has(H::DayOfMonth) {
        o.push_str(
            r#"def _day_of_month(z: int, n: int, k: int, mode: str) -> int:
    y, m, _ = _civil_from_days(z)
    y2, m2 = _shift_month(y, m, k)
    return _place(y2, m2, n, mode)


"#,
        );
    }
    if u.has(H::StartOfMonth) {
        o.push_str(
            r#"def _start_of_month(z: int, k: int) -> int:
    y, m, _ = _civil_from_days(z)
    y2, m2 = _shift_month(y, m, k)
    return _place(y2, m2, 1, "none")


"#,
        );
    }
    if u.has(H::EndOfMonth) {
        o.push_str(&format!(
            r#"def _end_of_month(z: int, k: int) -> int:
    y, m, _ = _civil_from_days(z)
    y2, m2 = _shift_month(y, m, k)
    if y2 < 1 or y2 > 9999:
        raise KoyomiError("date", {outside})
    return _days_from_civil(y2, m2, _month_len(y2, m2))


"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    if u.has(H::CloseDay) {
        o.push_str(&format!(
            r#"def _close_day(z: int, n: int, mode: str) -> int:
    y, m, _ = _civil_from_days(z)
    for k in range(-1 if mode == "start_of_next_month" else 0, 2):
        y2, m2 = _shift_month(y, m, k)
        if y2 < 1 or y2 > 9999:
            if k < 0:
                continue
            raise KoyomiError("date", {outside})
        c = _place(y2, m2, n, mode)
        if c >= z:
            return c
    raise AssertionError({bug})


"#,
            outside = msg(u, Msg::Outside),
            bug = msg(u, Msg::Bug)
        ));
    }
    if u.has(H::CloseEndOfMonth) {
        o.push_str(
            r#"def _close_end_of_month(z: int) -> int:
    y, m, _ = _civil_from_days(z)
    return _days_from_civil(y, m, _month_len(y, m))


"#,
        );
    }
    if u.has(H::Weekday) {
        o.push_str(
            r#"def _weekday(z: int) -> int:
    return (z + 3) % 7


"#,
        );
    }
    if u.has(H::IsOpen) {
        let c = u.cal.as_ref().unwrap();
        let mut body = String::new();
        if !c.opens.is_empty() {
            body.push_str("    for a, b in _OPENS:\n        if a <= z <= b:\n            return True\n");
        }
        if c.weekly.iter().any(|w| *w) {
            body.push_str("    if _WEEKLY[_weekday(z)]:\n        return False\n");
        }
        if !c.holidays.is_empty() {
            body.push_str("    if z in _HOLIDAYS:\n        return False\n");
        }
        if !c.every.is_empty() {
            body.push_str("    _, m, d = _civil_from_days(z)\n    md = m * 100 + d\n");
            body.push_str("    for a, b in _EVERY:\n        if (a <= md <= b) if a <= b else (md >= a or md <= b):\n            return False\n");
        }
        if !c.days.is_empty() {
            body.push_str("    for a, b in _DAYS:\n        if a <= z <= b:\n            return False\n");
        }
        o.push_str(&format!(
            r#"def _is_open(z: int) -> bool:
    if z < _DATA_FROM or z > _DATA_TO:
        day, lo, hi = _format_date(z), _format_date(_DATA_FROM), _format_date(_DATA_TO)
        raise KoyomiError("data", {data})
{body}    return True


"#,
            data = msg(u, Msg::Data)
        ));
    }
    if u.has(H::Seek) {
        o.push_str(
            r#"def _seek(z: int, forward: bool) -> int:
    x = z
    while not _is_open(x):
        x = _add_days(x, 1 if forward else -1)
    return x


"#,
        );
    }
    if u.has(H::Roll) {
        o.push_str(
            r#"def _same_month(a: int, b: int) -> bool:
    ya, ma, _ = _civil_from_days(a)
    yb, mb, _ = _civil_from_days(b)
    return ya == yb and ma == mb


def _roll(z: int, convention: str) -> int:
    if convention == "following":
        return _seek(z, True)
    if convention == "preceding":
        return _seek(z, False)
    if convention == "modified_following":
        f = _seek(z, True)
        return f if _same_month(f, z) else _seek(z, False)
    p = _seek(z, False)
    return p if _same_month(p, z) else _seek(z, True)


"#,
        );
    }
    if u.has(H::AddBusiness) {
        o.push_str(
            r#"def _add_business(z: int, n: int, forward: bool) -> int:
    if n == 0:
        return _seek(z, forward)
    x = z
    k = 0
    while k < n:
        x = _add_days(x, 1 if forward else -1)
        if _is_open(x):
            k += 1
    return x


"#,
        );
    }
    if u.has(H::At) {
        o.push_str(&format!(
            r#"def _at(day: date, minutes: int, offset: int) -> str:
    local = _from_date(day) * 1440 + minutes
    utc = local - offset
    for v in (local, utc):
        if v // 1440 < _MIN or v // 1440 > _MAX:
            raise KoyomiError("date", {outside})
    um = utc % 1440
    return f"{{_format_date(utc // 1440)}}T{{um // 60:02d}}:{{um % 60:02d}}:00Z"


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
        o.push_str(&format!("{}:\n", T.signature(&u.alias, &d.alias, &params, false)));
        o.push_str(&docstring(&date_doc(u, d, None), "    "));
        let first = &u.inputs[u.date_input()];
        o.push_str(&format!("    day = _input_date({}, {}, {}, {})  # {}\n", first.alias, lit(&first.name), first.lo, first.hi, first.range_text()));
        for k in &d.params {
            let i = &u.inputs[*k];
            if i.ty == Ty::Int {
                o.push_str(&format!("    _input_int({}, {}, {}, {})  # {}\n", i.alias, lit(&i.name), i.lo, i.hi, i.range_text()));
            }
        }
        for s in &d.steps {
            match &s.call {
                Call::IfClosed(inner) => {
                    o.push_str(&format!("    if not _is_open(day):  # {}\n        day = {}\n", s.comment, call(u, inner)));
                }
                c => o.push_str(&format!("    day = {}  # {}\n", call(u, c), s.comment)),
            }
        }
        o.push_str("    return _to_date(day)\n\n\n");
        if let Some((minutes, _)) = &d.at {
            let at = crate::naming::at_alias(&d.alias);
            let off = u.cal.as_ref().and_then(|c| c.offset).unwrap_or(0);
            o.push_str(&format!("{}:\n", T.signature(&u.alias, &at, &params, true)));
            o.push_str(&docstring(&at_doc(u, d, None, &fname), "    "));
            let args: Vec<String> = params.iter().map(|(a, _)| a.clone()).collect();
            o.push_str(&format!("    return _at({fname}({}), {minutes}, {off})\n\n\n", args.join(", ")));
        }
    }
    if u.cal.is_some() {
        o.push_str(&format!("{}:\n", T.is_open(&u.alias)));
        o.push_str(&docstring(&is_open_doc(u, None), "    "));
        o.push_str("    return _is_open(_from_date(day))\n");
    }
    while o.ends_with("\n\n") {
        o.pop();
    }
    o
}

/// `python/<alias>.py`.
pub fn module(u: &Unit) -> String {
    let mut o = String::new();
    for h in &u.header {
        o.push_str(&format!("# {h}\n"));
    }
    let what = match u.kind {
        Kind::Dates => {
            let n = super::named(&u.name, &u.alias, false);
            u.t(tr!("{}の日付を計算する。", "The dates of {}.", n.ja; n.en))
        }
        Kind::Calendar => {
            let n = super::named(&u.name, &u.alias, true);
            u.t(tr!("ある日がカレンダー{}の営業日かを返す。", "Whether a day is a business day of the calendar {}.", n.ja; n.en))
        }
    };
    o.push_str(&format!("\"\"\"{what}\"\"\"\n\nfrom datetime import date\n\n\n"));
    o.push_str(&format!(
        "class KoyomiError(ValueError):\n{}",
        docstring(
            &[u.t(tr!(
                "この生成物の関数が投げるエラー。kind は range（入力が範囲の外）、data（カレンダーが知らない日）、reject（else reject の無い日）、date（0001-01-01〜9999-12-31 の外）",
                "What the functions of this file raise. kind is range (an input outside its range), data (a day the calendar does not know), reject (a day the month does not have under else reject) or date (outside 0001-01-01..9999-12-31)"
            ))],
            "    "
        )
    ));
    o.push_str("\n    def __init__(self, kind: str, message: str) -> None:\n        super().__init__(message)\n        self.kind = kind\n\n\n");
    o.push_str(&calendar(u));
    o.push_str(&helpers(u));
    o.push_str(&functions(u));
    o.push('\n');
    o
}

/// `python/<alias>_runner.py`.
pub fn runner(u: &Unit) -> String {
    let mut o = String::new();
    o.push_str(&format!("# {}\n", u.header[0]));
    o.push_str(&format!(
        "# {}\n",
        u.t(tr!(
            "koyomi vectors の行を標準入力から読み、関数の結果を一行ずつ書く（空白で区切る。エラーなら error <種類>）",
            "Reads the lines of koyomi vectors on standard input and writes what the functions give, a line each (separated by a space; error <kind> for an error)"
        ))
    ));
    o.push_str("import json\nimport sys\nfrom datetime import date\nfrom typing import Any\n\n");
    o.push_str(&format!("import {} as m\n\n\n", u.alias));
    o.push_str("def run(i: dict[str, Any]) -> str:\n");
    if u.kind == Kind::Calendar {
        o.push_str("    return \"true\" if m.is_open(date.fromisoformat(i[\"date\"])) else \"false\"\n");
    } else {
        o.push_str("    out: list[str] = []\n");
        for (k, at) in u.outputs() {
            let d = &u.dates[k];
            let f = if at { T.function(&crate::naming::at_alias(&d.alias)) } else { T.function(&d.alias) };
            let args: Vec<String> = d
                .params
                .iter()
                .map(|p| {
                    let i = &u.inputs[*p];
                    match i.ty {
                        Ty::Date => format!("date.fromisoformat(i[{}])", lit(&i.name)),
                        Ty::Int => format!("i[{}]", lit(&i.name)),
                    }
                })
                .collect();
            if at {
                o.push_str(&format!("    out.append(m.{f}({}))\n", args.join(", ")));
            } else {
                o.push_str(&format!("    out.append(m.{f}({}).isoformat())\n", args.join(", ")));
            }
        }
        o.push_str("    return \" \".join(out)\n");
    }
    o.push_str(
        r#"

def main() -> None:
    lines: list[str] = []
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            lines.append(run(json.loads(line)["in"]))
        except m.KoyomiError as e:
            lines.append("error " + e.kind)
        if len(lines) >= 4096:
            sys.stdout.write("\n".join(lines) + "\n")
            lines = []
    if lines:
        sys.stdout.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
"#,
    );
    o
}
