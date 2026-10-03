//! SQL for PostgreSQL 14 or later (DESIGN 6.2): `sql/<alias>.sql` makes a schema named by the
//! alias and puts the functions in it, `<alias>.<date>(…) RETURNS date`, `<alias>.<date>_at(…)
//! RETURNS timestamptz`, `<alias>.is_open(date)`. Every function is `IMMUTABLE`. The public
//! functions and the helpers with a loop or a branch are PL/pgSQL; the one-expression helpers
//! (the weekday, leap years, the length of a month, whether a day is in the table) are
//! SQL-standard bodies, which the planner inlines into the expressions that call them. Errors
//! are `RAISE EXCEPTION 'koyomi <kind>: …' USING ERRCODE = '22023'`.
//!
//! A date and its day number are converted with PostgreSQL's date arithmetic (`d - DATE
//! '1970-01-01'`, `DATE '1970-01-01' + z`), which counts days and nothing else; the months
//! are the same procedures as the other targets.
//!
//! The runner (`<alias>_runner.sql`) is a psql script: it reads the vectors from psql's
//! standard input (`\copy … FROM pstdin`) and writes a line an input with `COPY … TO STDOUT`.

use ritsu_emit::header::Comment;
use super::{Call, H, Msg, Num, Piece, Unit, at_doc, date_doc, day_text, is_open_doc, message};
use crate::ast::{Kind, Ty};
use crate::naming::Target;

const T: Target = Target::Sql;

fn lit(s: &str) -> String {
    ritsu_emit::lit::sql(s)
}

/// A message as a `||` expression; the variables are the helper's locals, `_` and the name.
fn msg(u: &Unit, m: Msg) -> String {
    message(u.lang, m)
        .iter()
        .map(|p| match p {
            Piece::Lit(s) => lit(s),
            Piece::Var(v) => format!("_{v}"),
        })
        .collect::<Vec<_>>()
        .join(" || ")
}

fn raise(u: &Unit, kind: &str, m: Msg) -> String {
    format!("RAISE EXCEPTION 'koyomi {kind}: %', {} USING ERRCODE = '22023'", msg(u, m))
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
    let s = &u.alias;
    match c {
        Call::AddDays(n) => format!("{s}._add_days(day, {})", num(u, n)),
        Call::AddBusiness(n, fwd) => format!("{s}._add_business(day, {}, {fwd})", num(u, n)),
        Call::AddMonths(k, m) => format!("{s}._add_months(day, {}, {})", num(u, k), lit(m)),
        Call::DayOfMonth(n, k, m) => format!("{s}._day_of_month(day, {}, {}, {})", num(u, n), num(u, k), lit(m)),
        Call::StartOfMonth(k) => format!("{s}._start_of_month(day, {})", num(u, k)),
        Call::EndOfMonth(k) => format!("{s}._end_of_month(day, {})", num(u, k)),
        Call::CloseDay(n, m) => format!("{s}._close_day(day, {}, {})", num(u, n), lit(m)),
        Call::CloseEndOfMonth => format!("{s}._close_end_of_month(day)"),
        Call::Roll(c) => format!("{s}._roll(day, {})", lit(c)),
        Call::IfClosed(inner) => call(u, inner),
    }
}

fn doc(lines: &[String]) -> String {
    lines.iter().map(|l| format!("-- {l}\n")).collect()
}

/// `CREATE OR REPLACE FUNCTION`, PL/pgSQL.
fn plpgsql(head: &str, strict: bool, declare: &[&str], body: &str) -> String {
    let mut o = format!("CREATE OR REPLACE FUNCTION {head}\nLANGUAGE plpgsql IMMUTABLE {}PARALLEL SAFE AS $$\n", if strict { "STRICT " } else { "" });
    if !declare.is_empty() {
        o.push_str("DECLARE\n");
        for d in declare {
            o.push_str(&format!("  {d};\n"));
        }
    }
    o.push_str(&format!("BEGIN\n{body}END\n$$;\n\n"));
    o
}

/// `CREATE OR REPLACE FUNCTION` with a SQL-standard body of one expression, which the
/// planner inlines into the expression that calls it. Not `STRICT`: PostgreSQL does not
/// inline a strict function whose body is not strict (a `CASE`, an `AND`), and these helpers
/// are called by the generated functions only, never with `NULL`.
fn sql_fn(head: &str, expr: &str) -> String {
    format!("CREATE OR REPLACE FUNCTION {head}\nLANGUAGE sql IMMUTABLE PARALLEL SAFE\nRETURN {expr};\n\n")
}

fn helpers(u: &Unit) -> String {
    let s = &u.alias;
    let mut o = String::new();
    o.push_str(&format!(
        "-- {}\n\n",
        u.t(tr!(
            "日付は 1970-01-01 からの通算日で計算する。どの関数も koyomi の date.rs と calendar.rs の同じ名前の手順を写したもの",
            "Dates are computed as days since 1970-01-01. Each function follows the procedure of the same name in koyomi's date.rs and calendar.rs"
        ))
    ));
    o.push_str(&plpgsql(
        &format!("{s}._days_from_civil(y integer, m integer, d integer) RETURNS integer"),
        true,
        &["yy integer := CASE WHEN m <= 2 THEN y - 1 ELSE y END", "era integer", "yoe integer", "mp integer", "doy integer", "doe integer"],
        "  era := (CASE WHEN yy >= 0 THEN yy ELSE yy - 399 END) / 400;\n  yoe := yy - era * 400;\n  mp := (m + 9) % 12;\n  doy := (153 * mp + 2) / 5 + d - 1;\n  doe := yoe * 365 + yoe / 4 - yoe / 100 + doy;\n  RETURN era * 146097 + doe - 719468;\n",
    ));
    o.push_str(&format!("-- {}\n", u.t(tr!("年・月・日の配列を返す", "Gives year, month and day as an array"))));
    o.push_str(&plpgsql(
        &format!("{s}._civil_from_days(z integer) RETURNS integer[]"),
        true,
        &["zz integer := z + 719468", "era integer", "doe integer", "yoe integer", "y integer", "doy integer", "mp integer", "d integer", "m integer"],
        "  era := (CASE WHEN zz >= 0 THEN zz ELSE zz - 146096 END) / 146097;\n  doe := zz - era * 146097;\n  yoe := (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;\n  y := yoe + era * 400;\n  doy := doe - (365 * yoe + yoe / 4 - yoe / 100);\n  mp := (5 * doy + 2) / 153;\n  d := doy - (153 * mp + 2) / 5 + 1;\n  m := CASE WHEN mp < 10 THEN mp + 3 ELSE mp - 9 END;\n  RETURN ARRAY[CASE WHEN m <= 2 THEN y + 1 ELSE y END, m, d];\n",
    ));
    o.push_str(&sql_fn(&format!("{s}._is_leap(y integer) RETURNS boolean"), "y % 4 = 0 AND (y % 100 <> 0 OR y % 400 = 0)"));
    o.push_str(&sql_fn(
        &format!("{s}._month_len(y integer, m integer) RETURNS integer"),
        &format!("CASE WHEN m = 2 THEN CASE WHEN {s}._is_leap(y) THEN 29 ELSE 28 END WHEN m IN (4, 6, 9, 11) THEN 30 ELSE 31 END"),
    ));
    o.push_str(&sql_fn(&format!("{s}._ymd(y integer, m integer, d integer) RETURNS text"), "lpad(y::text, 4, '0') || '-' || lpad(m::text, 2, '0') || '-' || lpad(d::text, 2, '0')"));
    o.push_str(&plpgsql(&format!("{s}._format_date(z integer) RETURNS text"), true, &[&format!("ymd integer[] := {s}._civil_from_days(z)")], &format!("  RETURN {s}._ymd(ymd[1], ymd[2], ymd[3]);\n")));
    o.push_str(&sql_fn(&format!("{s}._floor_div(a bigint, b bigint) RETURNS bigint"), "CASE WHEN a % b <> 0 AND (a < 0) <> (b < 0) THEN a / b - 1 ELSE a / b END"));
    if u.kind == Kind::Dates && !u.dates.is_empty() {
        o.push_str(&plpgsql(
            &format!("{s}._input_date(_d date, _name text, _from integer, _to integer) RETURNS integer"),
            false,
            &["_z integer := _d - DATE '1970-01-01'", "_value text", "_lo text", "_hi text"],
            &format!(
                "  IF _z < _from OR _z > _to THEN\n    _value := {s}._format_date(_z);\n    _lo := {s}._format_date(_from);\n    _hi := {s}._format_date(_to);\n    {};\n  END IF;\n  RETURN _z;\n",
                raise(u, "range", Msg::Range)
            ),
        ));
    }
    if u.has_ints() {
        o.push_str(&plpgsql(
            &format!("{s}._input_int(_v integer, _name text, _from integer, _to integer) RETURNS integer"),
            false,
            &["_value text := _v::text", "_lo text := _from::text", "_hi text := _to::text"],
            &format!("  IF _v < _from OR _v > _to THEN\n    {};\n  END IF;\n  RETURN _v;\n", raise(u, "range", Msg::Range)),
        ));
    }
    if u.has(H::ShiftMonth) {
        o.push_str(&format!("-- {}\n", u.t(tr!("年と月の配列を返す", "Gives year and month as an array"))));
        o.push_str(&plpgsql(
            &format!("{s}._shift_month(y integer, m integer, k integer) RETURNS integer[]"),
            true,
            &["t integer := y * 12 + (m - 1) + k", "y2 integer"],
            &format!("  y2 := {s}._floor_div(t, 12);\n  RETURN ARRAY[y2, t - y2 * 12 + 1];\n"),
        ));
    }
    if u.has(H::Place) {
        o.push_str(&plpgsql(
            &format!("{s}._place(y integer, m integer, d integer, mode text) RETURNS integer"),
            true,
            &["n integer", "ym integer[]", "_day text"],
            &format!(
                "  IF y < 1 OR y > 9999 THEN\n    {outside};\n  END IF;\n  n := {s}._month_len(y, m);\n  IF d <= n THEN\n    RETURN {s}._days_from_civil(y, m, d);\n  END IF;\n  IF mode = 'end_of_month' THEN\n    RETURN {s}._days_from_civil(y, m, n);\n  END IF;\n  IF mode = 'start_of_next_month' THEN\n    ym := {s}._shift_month(y, m, 1);\n    IF ym[1] < 1 OR ym[1] > 9999 THEN\n      {outside};\n    END IF;\n    RETURN {s}._days_from_civil(ym[1], ym[2], 1);\n  END IF;\n  _day := {s}._ymd(y, m, d);\n  IF mode = 'reject' THEN\n    {reject};\n  END IF;\n  RAISE EXCEPTION 'koyomi bug: %', {bug} USING ERRCODE = 'XX000';\n",
                outside = raise(u, "date", Msg::Outside),
                reject = raise(u, "reject", Msg::Reject),
                bug = msg(u, Msg::Bug)
            ),
        ));
    }
    if u.has(H::AddDays) {
        o.push_str(&plpgsql(
            &format!("{s}._add_days(z integer, n integer) RETURNS integer"),
            true,
            &["r integer := z + n"],
            &format!("  IF r < -719162 OR r > 2932896 THEN\n    {};\n  END IF;\n  RETURN r;\n", raise(u, "date", Msg::Outside)),
        ));
    }
    if u.has(H::AddMonths) {
        o.push_str(&plpgsql(
            &format!("{s}._add_months(z integer, k integer, mode text) RETURNS integer"),
            true,
            &[&format!("ymd integer[] := {s}._civil_from_days(z)"), &format!("ym integer[] := {s}._shift_month(ymd[1], ymd[2], k)")],
            &format!("  RETURN {s}._place(ym[1], ym[2], ymd[3], mode);\n"),
        ));
    }
    if u.has(H::DayOfMonth) {
        o.push_str(&plpgsql(
            &format!("{s}._day_of_month(z integer, n integer, k integer, mode text) RETURNS integer"),
            true,
            &[&format!("ymd integer[] := {s}._civil_from_days(z)"), &format!("ym integer[] := {s}._shift_month(ymd[1], ymd[2], k)")],
            &format!("  RETURN {s}._place(ym[1], ym[2], n, mode);\n"),
        ));
    }
    if u.has(H::StartOfMonth) {
        o.push_str(&plpgsql(
            &format!("{s}._start_of_month(z integer, k integer) RETURNS integer"),
            true,
            &[&format!("ymd integer[] := {s}._civil_from_days(z)"), &format!("ym integer[] := {s}._shift_month(ymd[1], ymd[2], k)")],
            &format!("  RETURN {s}._place(ym[1], ym[2], 1, 'none');\n"),
        ));
    }
    if u.has(H::EndOfMonth) {
        o.push_str(&plpgsql(
            &format!("{s}._end_of_month(z integer, k integer) RETURNS integer"),
            true,
            &[&format!("ymd integer[] := {s}._civil_from_days(z)"), &format!("ym integer[] := {s}._shift_month(ymd[1], ymd[2], k)")],
            &format!(
                "  IF ym[1] < 1 OR ym[1] > 9999 THEN\n    {};\n  END IF;\n  RETURN {s}._days_from_civil(ym[1], ym[2], {s}._month_len(ym[1], ym[2]));\n",
                raise(u, "date", Msg::Outside)
            ),
        ));
    }
    if u.has(H::CloseDay) {
        o.push_str(&plpgsql(
            &format!("{s}._close_day(z integer, n integer, mode text) RETURNS integer"),
            true,
            &[&format!("ymd integer[] := {s}._civil_from_days(z)"), "ym integer[]", "c integer"],
            &format!(
                "  FOR k IN CASE WHEN mode = 'start_of_next_month' THEN -1 ELSE 0 END .. 1 LOOP\n    ym := {s}._shift_month(ymd[1], ymd[2], k);\n    IF ym[1] < 1 OR ym[1] > 9999 THEN\n      IF k < 0 THEN\n        CONTINUE;\n      END IF;\n      {outside};\n    END IF;\n    c := {s}._place(ym[1], ym[2], n, mode);\n    IF c >= z THEN\n      RETURN c;\n    END IF;\n  END LOOP;\n  RAISE EXCEPTION 'koyomi bug: %', {bug} USING ERRCODE = 'XX000';\n",
                outside = raise(u, "date", Msg::Outside),
                bug = msg(u, Msg::Bug)
            ),
        ));
    }
    if u.has(H::CloseEndOfMonth) {
        o.push_str(&plpgsql(
            &format!("{s}._close_end_of_month(z integer) RETURNS integer"),
            true,
            &[&format!("ymd integer[] := {s}._civil_from_days(z)")],
            &format!("  RETURN {s}._days_from_civil(ymd[1], ymd[2], {s}._month_len(ymd[1], ymd[2]));\n"),
        ));
    }
    if u.has(H::Weekday) {
        o.push_str(&sql_fn(&format!("{s}._weekday(z integer) RETURNS integer"), "((z + 3) % 7 + 7) % 7"));
    }
    if u.has(H::IsOpen) {
        let c = u.cal.as_ref().unwrap();
        if !c.holidays.is_empty() {
            o.push_str(&format!(
                "-- {}\nCREATE OR REPLACE FUNCTION {s}._is_holiday(z integer) RETURNS boolean\nLANGUAGE sql IMMUTABLE PARALLEL SAFE\nRETURN z = ANY (ARRAY[\n",
                u.t(tr!("表に載っている日（データの範囲の中の行）", "The days of the tables (the rows inside the data range)"))
            ));
            for (i, (z, t)) in c.holidays.iter().enumerate() {
                let comma = if i + 1 < c.holidays.len() { "," } else { "" };
                o.push_str(&format!("  {z}{comma} -- {t}\n"));
            }
            o.push_str("]);\n\n");
        }
        let mut body = format!(
            "  IF z < {} OR z > {} THEN -- {}..{}\n    _day := {s}._format_date(z);\n    _lo := {s}._format_date({});\n    _hi := {s}._format_date({});\n    {};\n  END IF;\n",
            c.data.0,
            c.data.1,
            day_text(c.data.0),
            day_text(c.data.1),
            c.data.0,
            c.data.1,
            raise(u, "data", Msg::Data)
        );
        for (a, b, t) in &c.opens {
            body.push_str(&format!("  IF z BETWEEN {a} AND {b} THEN -- open {t}\n    RETURN true;\n  END IF;\n"));
        }
        if c.weekly.iter().any(|w| *w) {
            let ds: Vec<String> = (0..7).filter(|d| c.weekly[*d]).map(|d| d.to_string()).collect();
            let names: Vec<&str> = (0..7).filter(|d| c.weekly[*d]).map(|d| crate::kw::WEEKDAYS[d]).collect();
            body.push_str(&format!("  IF {s}._weekday(z) IN ({}) THEN -- {}\n    RETURN false;\n  END IF;\n", ds.join(", "), u.t(tr!("月曜が 0。closed weekly {}", "Monday is 0: closed weekly {}", names.join(", ")))));
        }
        if !c.holidays.is_empty() {
            body.push_str(&format!("  IF {s}._is_holiday(z) THEN\n    RETURN false;\n  END IF;\n"));
        }
        if !c.every.is_empty() {
            body.push_str(&format!("  ymd := {s}._civil_from_days(z);\n  md := ymd[2] * 100 + ymd[3];\n"));
            for (a, b, t) in &c.every {
                let cond = if a <= b { format!("md BETWEEN {a} AND {b}") } else { format!("md >= {a} OR md <= {b}") };
                body.push_str(&format!("  IF {cond} THEN -- closed {t}\n    RETURN false;\n  END IF;\n"));
            }
        }
        for (a, b, t) in &c.days {
            body.push_str(&format!("  IF z BETWEEN {a} AND {b} THEN -- closed {t}\n    RETURN false;\n  END IF;\n"));
        }
        body.push_str("  RETURN true;\n");
        o.push_str(&plpgsql(&format!("{s}._is_open(z integer) RETURNS boolean"), true, &["ymd integer[]", "md integer", "_day text", "_lo text", "_hi text"], &body));
    }
    if u.has(H::Seek) {
        o.push_str(&plpgsql(
            &format!("{s}._seek(z integer, forward boolean) RETURNS integer"),
            true,
            &["x integer := z"],
            &format!("  WHILE NOT {s}._is_open(x) LOOP\n    x := {s}._add_days(x, CASE WHEN forward THEN 1 ELSE -1 END);\n  END LOOP;\n  RETURN x;\n"),
        ));
    }
    if u.has(H::Roll) {
        o.push_str(&plpgsql(
            &format!("{s}._same_month(a integer, b integer) RETURNS boolean"),
            true,
            &[&format!("ya integer[] := {s}._civil_from_days(a)"), &format!("yb integer[] := {s}._civil_from_days(b)")],
            "  RETURN ya[1] = yb[1] AND ya[2] = yb[2];\n",
        ));
        o.push_str(&plpgsql(
            &format!("{s}._roll(z integer, convention text) RETURNS integer"),
            true,
            &["r integer"],
            &format!(
                "  IF convention = 'following' THEN\n    RETURN {s}._seek(z, true);\n  ELSIF convention = 'preceding' THEN\n    RETURN {s}._seek(z, false);\n  ELSIF convention = 'modified_following' THEN\n    r := {s}._seek(z, true);\n    IF {s}._same_month(r, z) THEN\n      RETURN r;\n    END IF;\n    RETURN {s}._seek(z, false);\n  END IF;\n  r := {s}._seek(z, false);\n  IF {s}._same_month(r, z) THEN\n    RETURN r;\n  END IF;\n  RETURN {s}._seek(z, true);\n"
            ),
        ));
    }
    if u.has(H::AddBusiness) {
        o.push_str(&plpgsql(
            &format!("{s}._add_business(z integer, n integer, forward boolean) RETURNS integer"),
            true,
            &["x integer := z", "k integer := 0"],
            &format!(
                "  IF n = 0 THEN\n    RETURN {s}._seek(z, forward);\n  END IF;\n  WHILE k < n LOOP\n    x := {s}._add_days(x, CASE WHEN forward THEN 1 ELSE -1 END);\n    IF {s}._is_open(x) THEN\n      k := k + 1;\n    END IF;\n  END LOOP;\n  RETURN x;\n"
            ),
        ));
    }
    if u.has(H::At) {
        o.push_str(&format!(
            "-- {}\n",
            u.t(tr!(
                "その日の 0 時から minutes 分後の時刻（UTC から off 分進んだ時刻）を timestamptz で返す",
                "Gives the time minutes after the start of the day, at off minutes east of UTC, as a timestamptz"
            ))
        ));
        o.push_str(&plpgsql(
            &format!("{s}._at(day date, minutes integer, off integer) RETURNS timestamptz"),
            true,
            &["local bigint := (day - DATE '1970-01-01')::bigint * 1440 + minutes", "utc bigint := local - off"],
            &format!(
                "  IF {s}._floor_div(local, 1440) NOT BETWEEN -719162 AND 2932896 OR {s}._floor_div(utc, 1440) NOT BETWEEN -719162 AND 2932896 THEN\n    {};\n  END IF;\n  RETURN to_timestamp(utc * 60);\n",
                raise(u, "date", Msg::Outside)
            ),
        ));
    }
    o
}

fn functions(u: &Unit) -> String {
    let s = &u.alias;
    let mut o = String::new();
    for d in &u.dates {
        let params: Vec<(String, Ty)> = d.params.iter().map(|k| (u.inputs[*k].alias.clone(), u.inputs[*k].ty)).collect();
        let fname = T.function(&d.alias);
        o.push_str(&doc(&date_doc(u, d, Some(&format!("{s}.{fname}")))));
        let first = &u.inputs[u.date_input()];
        let mut body = format!("  day := {s}._input_date({}, {}, {}, {}); -- {}\n", first.alias, lit(&first.name), first.lo, first.hi, first.range_text());
        for k in &d.params {
            let i = &u.inputs[*k];
            if i.ty == Ty::Int {
                body.push_str(&format!("  {} := {s}._input_int({}, {}, {}, {}); -- {}\n", i.alias, i.alias, lit(&i.name), i.lo, i.hi, i.range_text()));
            }
        }
        for st in &d.steps {
            match &st.call {
                Call::IfClosed(inner) => body.push_str(&format!("  IF NOT {s}._is_open(day) THEN -- {}\n    day := {};\n  END IF;\n", st.comment, call(u, inner))),
                c => body.push_str(&format!("  day := {}; -- {}\n", call(u, c), st.comment)),
            }
        }
        body.push_str("  RETURN DATE '1970-01-01' + day;\n");
        o.push_str(&plpgsql(&T.signature(&u.alias, &d.alias, &params, false), true, &["day integer"], &body));
        if let Some((minutes, _)) = &d.at {
            let at = crate::naming::at_alias(&d.alias);
            let off = u.cal.as_ref().and_then(|c| c.offset).unwrap_or(0);
            o.push_str(&doc(&at_doc(u, d, Some(&format!("{s}.{}", T.function(&at))), &format!("{s}.{fname}"))));
            let args: Vec<String> = params.iter().map(|(a, _)| a.clone()).collect();
            o.push_str(&plpgsql(&T.signature(&u.alias, &at, &params, true), true, &[], &format!("  RETURN {s}._at({s}.{fname}({}), {minutes}, {off});\n", args.join(", "))));
        }
    }
    if u.cal.is_some() {
        o.push_str(&doc(&is_open_doc(u, Some(&format!("{s}.is_open")))));
        o.push_str(&plpgsql(&T.is_open(&u.alias), true, &[], &format!("  RETURN {s}._is_open(day - DATE '1970-01-01');\n")));
    }
    while o.ends_with("\n\n") {
        o.pop();
    }
    o
}

/// `sql/<alias>.sql`.
pub fn module(u: &Unit) -> String {
    let mut o = String::new();
    for h in &u.header {
        o.push_str(&Comment::Dashes.line(h));
    }
    o.push('\n');
    let what = match u.kind {
        Kind::Dates => {
            let n = super::named(&u.name, &u.alias, false);
            u.t(tr!("スキーマ {} に {}の日付の関数を作る。", "Makes the functions of the dates of {} in the schema {}.", u.alias, n.ja; n.en, u.alias))
        }
        Kind::Calendar => u.t(tr!("スキーマ {} に、カレンダー「{}」の営業日の関数を作る。", "Makes the function of the business days of the calendar {} in the schema {}.", u.alias, u.name; u.name, u.alias)),
    };
    o.push_str(&format!("-- {what}\n"));
    o.push_str(&format!(
        "-- {}\n",
        u.t(tr!(
            "エラーは SQLSTATE 22023 で、メッセージは koyomi <種類>: で始まる。種類は range（入力が範囲の外）、data（カレンダーが知らない日）、reject（else reject の無い日）、date（0001-01-01〜9999-12-31 の外）",
            "Errors are SQLSTATE 22023, and the message starts with koyomi <kind>:, the kind being range (an input outside its range), data (a day the calendar does not know), reject (a day the month does not have under else reject) or date (outside 0001-01-01..9999-12-31)"
        ))
    ));
    o.push_str(&format!("CREATE SCHEMA IF NOT EXISTS {};\n\n", u.alias));
    o.push_str(&helpers(u));
    o.push_str(&functions(u));
    o
}

/// `sql/<alias>_runner.sql`, for `psql -f`: the vectors come on psql's standard input.
pub fn runner(u: &Unit) -> String {
    let s = &u.alias;
    let mut o = String::new();
    o.push_str(&Comment::Dashes.line(&u.header[0]));
    o.push_str(&format!(
        "-- {}\n",
        u.t(tr!(
            "koyomi vectors の行を psql の標準入力から読み、関数の結果を一行ずつ書く（空白で区切る。エラーなら error <種類>）。psql -f で走らせる",
            "Reads the lines of koyomi vectors on psql's standard input and writes what the functions give, a line each (separated by a space; error <kind> for an error). Run it with psql -f"
        ))
    ));
    o.push_str("\\set ON_ERROR_STOP on\nSET client_min_messages = warning;\n");
    o.push_str("CREATE TEMP TABLE _koyomi_vectors (n bigint GENERATED ALWAYS AS IDENTITY, line jsonb);\n");
    o.push_str("\\copy _koyomi_vectors (line) FROM pstdin\n");
    o.push_str("CREATE FUNCTION pg_temp._koyomi_run(i jsonb) RETURNS text LANGUAGE plpgsql AS $$\n");
    if u.kind == Kind::Calendar {
        o.push_str(&format!(
            "BEGIN\n  RETURN CASE WHEN {s}.is_open((i->>'date')::date) THEN 'true' ELSE 'false' END;\nEXCEPTION WHEN SQLSTATE '22023' THEN\n  RETURN 'error ' || substring(SQLERRM FROM '^koyomi ([a-z]+):');\nEND\n$$;\n"
        ));
    } else {
        let mut declare = Vec::new();
        let mut assign = String::new();
        for (k, inp) in u.inputs.iter().enumerate() {
            if !u.dates.iter().any(|d| d.params.contains(&k)) {
                continue;
            }
            let ty = if inp.ty == Ty::Date { "date" } else { "integer" };
            declare.push(format!("  in{k} {ty};\n"));
            assign.push_str(&format!("  in{k} := (i->>{})::{ty};\n", lit(&inp.name)));
        }
        o.push_str("DECLARE\n");
        for d in &declare {
            o.push_str(d);
        }
        o.push_str("BEGIN\n");
        o.push_str(&assign);
        let mut parts = Vec::new();
        for (k, at) in u.outputs() {
            let d = &u.dates[k];
            let args: Vec<String> = d.params.iter().map(|p| format!("in{p}")).collect();
            if at {
                let f = T.function(&crate::naming::at_alias(&d.alias));
                parts.push(format!("to_char({s}.{f}({}) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')", args.join(", ")));
            } else {
                parts.push(format!("to_char({s}.{}({}), 'YYYY-MM-DD')", T.function(&d.alias), args.join(", ")));
            }
        }
        if parts.is_empty() {
            o.push_str("  RETURN '';\n");
        } else {
            o.push_str(&format!("  RETURN concat_ws(' ',\n    {});\n", parts.join(",\n    ")));
        }
        o.push_str("EXCEPTION WHEN SQLSTATE '22023' THEN\n  RETURN 'error ' || substring(SQLERRM FROM '^koyomi ([a-z]+):');\nEND\n$$;\n");
    }
    o.push_str("COPY (SELECT pg_temp._koyomi_run(line->'in') FROM _koyomi_vectors ORDER BY n) TO STDOUT;\n");
    o
}
