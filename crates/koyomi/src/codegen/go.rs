//! Go (DESIGN 6.2): `go/<package>/<package>.go`, one package with a `Date{Year, Month, Day}`,
//! `ParseDate`, and functions that return `(Date, error)`; the error is a `*KoyomiError` with
//! a `Kind`. The file is what gofmt writes (`gofmt -l` prints nothing), and `go vet` passes.
//!
//! The runner is a test file of the package (`<package>_runner_test.go`): a `TestMain` that,
//! when `KOYOMI_RUNNER` is set, reads the vectors instead of running tests. Being in the
//! package, it needs no import path, so the generated directory works inside any module.

use ritsu_emit::header::Comment;
use super::{Call, H, Msg, Num, Piece, Unit, aligned, at_doc, date_doc, day_text, is_open_doc, message};
use crate::ast::{Kind, Ty};
use crate::naming::Target;

const T: Target = Target::Go;

fn lit(s: &str) -> String {
    ritsu_emit::lit::json(s)
}

/// A message as a string expression: the pieces joined with `+` (every variable is a string),
/// with no spaces around it, as gofmt writes it inside the arguments of a call.
fn msg(u: &Unit, m: Msg) -> String {
    message(u.lang, m)
        .iter()
        .map(|p| match p {
            Piece::Lit(s) => lit(s),
            Piece::Var(v) => v.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn num(u: &Unit, n: &Num) -> String {
    match n {
        Num::Lit(v) => v.to_string(),
        Num::Input(1, k) => u.inputs[*k].alias.clone(),
        Num::Input(-1, k) => format!("-{}", u.inputs[*k].alias),
        Num::Input(f, k) => format!("{f}*{}", u.inputs[*k].alias),
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
        Call::IfClosed(inner) => format!("_ifClosed(day, func(day int) (int, error) {{ return {} }})", call(u, inner)),
    }
}

fn doc(lines: &[String]) -> String {
    lines.iter().map(|l| format!("// {l}\n")).collect()
}

fn calendar(u: &Unit) -> String {
    let Some(c) = &u.cal else { return String::new() };
    let mut o = format!(
        "// {}\n",
        u.t(tr!(
            "カレンダー「{}」が知っている日: {}〜{}",
            "The days the calendar {} knows: {}..{}",
            c.name,
            day_text(c.data.0),
            day_text(c.data.1)
        ))
    );
    o.push_str(&format!("const _dataFrom, _dataTo = {}, {}\n\n", c.data.0, c.data.1));
    if c.weekly.iter().any(|w| *w) {
        let ws: Vec<String> = c.weekly.iter().map(|w| w.to_string()).collect();
        let names: Vec<&str> = (0..7).filter(|d| c.weekly[*d]).map(|d| crate::kw::WEEKDAYS[d]).collect();
        o.push_str(&format!("// {}\nvar _weekly = [7]bool{{{}}}\n\n", super::weekly_comment(u, &names), ws.join(", ")));
    }
    let pairs = |name: &str, v: &[(i64, i64, String)], o: &mut String| {
        if v.is_empty() {
            return;
        }
        o.push_str(&format!("var {name} = [][2]int{{\n"));
        let lines: Vec<(String, String)> = v.iter().map(|(a, b, t)| (format!("{{{a}, {b}}},"), t.clone())).collect();
        o.push_str(&aligned(&lines, "\t", "//"));
        o.push_str("}\n\n");
    };
    pairs("_opens", &c.opens.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("open {t}"))).collect::<Vec<_>>(), &mut o);
    pairs("_every", &c.every.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("closed {t}"))).collect::<Vec<_>>(), &mut o);
    pairs("_days", &c.days.iter().map(|(a, b, t)| (*a as i64, *b as i64, format!("closed {t}"))).collect::<Vec<_>>(), &mut o);
    if !c.holidays.is_empty() {
        o.push_str("var _holidays = []int{\n");
        let lines: Vec<(String, String)> = c.holidays.iter().map(|(z, t)| (format!("{z},"), t.clone())).collect();
        o.push_str(&aligned(&lines, "\t", "//"));
        o.push_str("}\n\n");
    }
    o
}

fn helpers(u: &Unit) -> String {
    let mut o = String::new();
    o.push_str(&format!(
        "// {}\n",
        u.t(tr!(
            "日付は 1970-01-01 からの通算日で計算する。どの関数も koyomi の date.rs と calendar.rs の同じ名前の手順を移植したもの",
            "Dates are computed as days since 1970-01-01. Each function follows the procedure of the same name in koyomi's date.rs and calendar.rs"
        ))
    ));
    o.push_str("const _min, _max = -719162, 2932896 // 0001-01-01, 9999-12-31\n\n");
    o.push_str(
        r#"func _error(kind, message string) error {
	return &KoyomiError{Kind: kind, Message: message}
}

func _floorDiv(a, b int) int {
	q := a / b
	if a%b != 0 && (a < 0) != (b < 0) {
		q--
	}
	return q
}

func _daysFromCivil(y, m, d int) int {
	if m <= 2 {
		y--
	}
	era := y
	if era < 0 {
		era -= 399
	}
	era /= 400
	yoe := y - era*400
	mp := (m + 9) % 12
	doy := (153*mp+2)/5 + d - 1
	doe := yoe*365 + yoe/4 - yoe/100 + doy
	return era*146097 + doe - 719468
}

func _civilFromDays(z int) (int, int, int) {
	z += 719468
	era := z
	if era < 0 {
		era -= 146096
	}
	era /= 146097
	doe := z - era*146097
	yoe := (doe - doe/1460 + doe/36524 - doe/146096) / 365
	y := yoe + era*400
	doy := doe - (365*yoe + yoe/4 - yoe/100)
	mp := (5*doy + 2) / 153
	d := doy - (153*mp+2)/5 + 1
	m := mp + 3
	if mp >= 10 {
		m = mp - 9
	}
	if m <= 2 {
		y++
	}
	return y, m, d
}

func _isLeap(y int) bool {
	return y%4 == 0 && (y%100 != 0 || y%400 == 0)
}

func _monthLen(y, m int) int {
	switch m {
	case 2:
		if _isLeap(y) {
			return 29
		}
		return 28
	case 4, 6, 9, 11:
		return 30
	}
	return 31
}

func _pad(v, width int) string {
	s := fmt.Sprint(v)
	for len(s) < width {
		s = "0" + s
	}
	return s
}

func _ymd(y, m, d int) string {
	return _pad(y, 4) + "-" + _pad(m, 2) + "-" + _pad(d, 2)
}

func _formatDate(z int) string {
	return _ymd(_civilFromDays(z))
}

func _date(z int) Date {
	y, m, d := _civilFromDays(z)
	return Date{y, m, d}
}

"#,
    );
    o.push_str(&format!(
        r#"func _dayNumber(d Date) (int, error) {{
	if d.Year < 1 || d.Year > 9999 || d.Month < 1 || d.Month > 12 || d.Day < 1 || d.Day > _monthLen(d.Year, d.Month) {{
		text := d.String()
		return 0, _error("date", {not_a_date})
	}}
	return _daysFromCivil(d.Year, d.Month, d.Day), nil
}}

func _parseDate(text string) (int, error) {{
	var p [3]int
	ok := len(text) == 10 && text[4] == '-' && text[7] == '-'
	for i := 0; ok && i < 10; i++ {{
		c := text[i]
		switch {{
		case i == 4 || i == 7:
		case c >= '0' && c <= '9' && i < 4:
			p[0] = p[0]*10 + int(c-'0')
		case c >= '0' && c <= '9' && i < 7:
			p[1] = p[1]*10 + int(c-'0')
		case c >= '0' && c <= '9':
			p[2] = p[2]*10 + int(c-'0')
		default:
			ok = false
		}}
	}}
	if !ok {{
		return 0, _error("date", {not_a_date})
	}}
	z, err := _dayNumber(Date{{p[0], p[1], p[2]}})
	if err != nil {{
		return 0, _error("date", {not_a_date})
	}}
	return z, nil
}}

"#,
        not_a_date = msg(u, Msg::NotADate)
    ));
    if u.kind == Kind::Dates && !u.dates.is_empty() {
        o.push_str(&format!(
            r#"func _inputDate(d Date, name string, from, to int) (int, error) {{
	z, err := _dayNumber(d)
	if err != nil {{
		return 0, err
	}}
	if z < from || z > to {{
		value, lo, hi := _formatDate(z), _formatDate(from), _formatDate(to)
		return 0, _error("range", {range})
	}}
	return z, nil
}}

"#,
            range = msg(u, Msg::Range)
        ));
    }
    if u.has_ints() {
        o.push_str(&format!(
            r#"func _inputInt(v int, name string, from, to int) error {{
	if v < from || v > to {{
		value, lo, hi := fmt.Sprint(v), fmt.Sprint(from), fmt.Sprint(to)
		return _error("range", {range})
	}}
	return nil
}}

"#,
            range = msg(u, Msg::Range)
        ));
    }
    if u.has(H::ShiftMonth) {
        o.push_str(
            r#"func _shiftMonth(y, m, k int) (int, int) {
	t := y*12 + (m - 1) + k
	y2 := _floorDiv(t, 12)
	return y2, t - y2*12 + 1
}

"#,
        );
    }
    if u.has(H::Place) {
        o.push_str(&format!(
            r#"func _place(y, m, d int, mode string) (int, error) {{
	if y < 1 || y > 9999 {{
		return 0, _error("date", {outside})
	}}
	n := _monthLen(y, m)
	if d <= n {{
		return _daysFromCivil(y, m, d), nil
	}}
	switch mode {{
	case "end_of_month":
		return _daysFromCivil(y, m, n), nil
	case "start_of_next_month":
		ny, nm := _shiftMonth(y, m, 1)
		if ny < 1 || ny > 9999 {{
			return 0, _error("date", {outside})
		}}
		return _daysFromCivil(ny, nm, 1), nil
	case "reject":
		day := _ymd(y, m, d)
		return 0, _error("reject", {reject})
	}}
	panic({bug})
}}

"#,
            outside = msg(u, Msg::Outside),
            reject = msg(u, Msg::Reject),
            bug = msg(u, Msg::Bug)
        ));
    }
    if u.has(H::AddDays) {
        o.push_str(&format!(
            r#"func _addDays(z, n int) (int, error) {{
	r := z + n
	if r < _min || r > _max {{
		return 0, _error("date", {outside})
	}}
	return r, nil
}}

"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    if u.has(H::AddMonths) {
        o.push_str(
            r#"func _addMonths(z, k int, mode string) (int, error) {
	y, m, d := _civilFromDays(z)
	y2, m2 := _shiftMonth(y, m, k)
	return _place(y2, m2, d, mode)
}

"#,
        );
    }
    if u.has(H::DayOfMonth) {
        o.push_str(
            r#"func _dayOfMonth(z, n, k int, mode string) (int, error) {
	y, m, _ := _civilFromDays(z)
	y2, m2 := _shiftMonth(y, m, k)
	return _place(y2, m2, n, mode)
}

"#,
        );
    }
    if u.has(H::StartOfMonth) {
        o.push_str(
            r#"func _startOfMonth(z, k int) (int, error) {
	y, m, _ := _civilFromDays(z)
	y2, m2 := _shiftMonth(y, m, k)
	return _place(y2, m2, 1, "none")
}

"#,
        );
    }
    if u.has(H::EndOfMonth) {
        o.push_str(&format!(
            r#"func _endOfMonth(z, k int) (int, error) {{
	y, m, _ := _civilFromDays(z)
	y2, m2 := _shiftMonth(y, m, k)
	if y2 < 1 || y2 > 9999 {{
		return 0, _error("date", {outside})
	}}
	return _daysFromCivil(y2, m2, _monthLen(y2, m2)), nil
}}

"#,
            outside = msg(u, Msg::Outside)
        ));
    }
    if u.has(H::CloseDay) {
        o.push_str(&format!(
            r#"func _closeDay(z, n int, mode string) (int, error) {{
	y, m, _ := _civilFromDays(z)
	k := 0
	if mode == "start_of_next_month" {{
		k = -1
	}}
	for ; k <= 1; k++ {{
		y2, m2 := _shiftMonth(y, m, k)
		if y2 < 1 || y2 > 9999 {{
			if k < 0 {{
				continue
			}}
			return 0, _error("date", {outside})
		}}
		c, err := _place(y2, m2, n, mode)
		if err != nil {{
			return 0, err
		}}
		if c >= z {{
			return c, nil
		}}
	}}
	panic({bug})
}}

"#,
            outside = msg(u, Msg::Outside),
            bug = msg(u, Msg::Bug)
        ));
    }
    if u.has(H::CloseEndOfMonth) {
        o.push_str(
            r#"func _closeEndOfMonth(z int) (int, error) {
	y, m, _ := _civilFromDays(z)
	return _daysFromCivil(y, m, _monthLen(y, m)), nil
}

"#,
        );
    }
    if u.has(H::Weekday) {
        o.push_str(
            r#"func _weekday(z int) int {
	return ((z+3)%7 + 7) % 7
}

"#,
        );
    }
    if u.has(H::IsOpen) {
        let c = u.cal.as_ref().unwrap();
        let mut body = String::new();
        if !c.opens.is_empty() {
            body.push_str("\tfor _, o := range _opens {\n\t\tif o[0] <= z && z <= o[1] {\n\t\t\treturn true, nil\n\t\t}\n\t}\n");
        }
        if c.weekly.iter().any(|w| *w) {
            body.push_str("\tif _weekly[_weekday(z)] {\n\t\treturn false, nil\n\t}\n");
        }
        if !c.holidays.is_empty() {
            body.push_str(
                "\tlo, hi := 0, len(_holidays)\n\tfor lo < hi {\n\t\tmid := (lo + hi) / 2\n\t\tif _holidays[mid] < z {\n\t\t\tlo = mid + 1\n\t\t} else {\n\t\t\thi = mid\n\t\t}\n\t}\n\tif lo < len(_holidays) && _holidays[lo] == z {\n\t\treturn false, nil\n\t}\n",
            );
        }
        if !c.every.is_empty() {
            body.push_str("\t_, m, d := _civilFromDays(z)\n\tmd := m*100 + d\n");
            body.push_str("\tfor _, e := range _every {\n\t\tif e[0] <= e[1] && e[0] <= md && md <= e[1] || e[0] > e[1] && (md >= e[0] || md <= e[1]) {\n\t\t\treturn false, nil\n\t\t}\n\t}\n");
        }
        if !c.days.is_empty() {
            body.push_str("\tfor _, s := range _days {\n\t\tif s[0] <= z && z <= s[1] {\n\t\t\treturn false, nil\n\t\t}\n\t}\n");
        }
        o.push_str(&format!(
            r#"func _isOpen(z int) (bool, error) {{
	if z < _dataFrom || z > _dataTo {{
		day, lo, hi := _formatDate(z), _formatDate(_dataFrom), _formatDate(_dataTo)
		return false, _error("data", {data})
	}}
{body}	return true, nil
}}

"#,
            data = msg(u, Msg::Data)
        ));
    }
    if u.dates.iter().any(|d| d.steps.iter().any(|s| matches!(s.call, Call::IfClosed(_)))) {
        o.push_str(
            r#"func _ifClosed(z int, then func(int) (int, error)) (int, error) {
	open, err := _isOpen(z)
	if err != nil || open {
		return z, err
	}
	return then(z)
}

"#,
        );
    }
    if u.has(H::Seek) {
        o.push_str(
            r#"func _seek(z int, forward bool) (int, error) {
	step := -1
	if forward {
		step = 1
	}
	for x := z; ; {
		open, err := _isOpen(x)
		if err != nil {
			return 0, err
		}
		if open {
			return x, nil
		}
		if x, err = _addDays(x, step); err != nil {
			return 0, err
		}
	}
}

"#,
        );
    }
    if u.has(H::Roll) {
        o.push_str(
            r#"func _sameMonth(a, b int) bool {
	ya, ma, _ := _civilFromDays(a)
	yb, mb, _ := _civilFromDays(b)
	return ya == yb && ma == mb
}

func _roll(z int, convention string) (int, error) {
	switch convention {
	case "following":
		return _seek(z, true)
	case "preceding":
		return _seek(z, false)
	case "modified_following":
		f, err := _seek(z, true)
		if err != nil || _sameMonth(f, z) {
			return f, err
		}
		return _seek(z, false)
	}
	p, err := _seek(z, false)
	if err != nil || _sameMonth(p, z) {
		return p, err
	}
	return _seek(z, true)
}

"#,
        );
    }
    if u.has(H::AddBusiness) {
        o.push_str(
            r#"func _addBusiness(z, n int, forward bool) (int, error) {
	if n == 0 {
		return _seek(z, forward)
	}
	step := -1
	if forward {
		step = 1
	}
	x := z
	for k := 0; k < n; {
		var err error
		if x, err = _addDays(x, step); err != nil {
			return 0, err
		}
		open, err := _isOpen(x)
		if err != nil {
			return 0, err
		}
		if open {
			k++
		}
	}
	return x, nil
}

"#,
        );
    }
    if u.has(H::At) {
        o.push_str(&format!(
            r#"func _at(day Date, minutes, offset int) (string, error) {{
	z, err := _dayNumber(day)
	if err != nil {{
		return "", err
	}}
	local := z*1440 + minutes
	utc := local - offset
	for _, v := range []int{{local, utc}} {{
		if d := _floorDiv(v, 1440); d < _min || d > _max {{
			return "", _error("date", {outside})
		}}
	}}
	ud := _floorDiv(utc, 1440)
	um := utc - ud*1440
	return _formatDate(ud) + "T" + _pad(um/60, 2) + ":" + _pad(um%60, 2) + ":00Z", nil
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
        o.push_str(&doc(&date_doc(u, d, Some(&fname))));
        o.push_str(&format!("{} {{\n", T.signature(&u.alias, &d.alias, &params, false)));
        let first = &u.inputs[u.date_input()];
        o.push_str(&format!(
            "\tday, err := _inputDate({}, {}, {}, {}) // {}\n\tif err != nil {{\n\t\treturn Date{{}}, err\n\t}}\n",
            first.alias,
            lit(&first.name),
            first.lo,
            first.hi,
            first.range_text()
        ));
        for k in &d.params {
            let i = &u.inputs[*k];
            if i.ty == Ty::Int {
                o.push_str(&format!(
                    "\tif err := _inputInt({}, {}, {}, {}); err != nil {{ // {}\n\t\treturn Date{{}}, err\n\t}}\n",
                    i.alias,
                    lit(&i.name),
                    i.lo,
                    i.hi,
                    i.range_text()
                ));
            }
        }
        for s in &d.steps {
            o.push_str(&format!("\tday, err = {} // {}\n\tif err != nil {{\n\t\treturn Date{{}}, err\n\t}}\n", call(u, &s.call), s.comment));
        }
        o.push_str("\treturn _date(day), nil\n}\n\n");
        if let Some((minutes, _)) = &d.at {
            let at = crate::naming::at_alias(&d.alias);
            let af = T.function(&at);
            let off = u.cal.as_ref().and_then(|c| c.offset).unwrap_or(0);
            o.push_str(&doc(&at_doc(u, d, Some(&af), &fname)));
            o.push_str(&format!("{} {{\n", T.signature(&u.alias, &at, &params, true)));
            let args: Vec<String> = params.iter().map(|(a, _)| a.clone()).collect();
            o.push_str(&format!("\tday, err := {fname}({})\n\tif err != nil {{\n\t\treturn \"\", err\n\t}}\n\treturn _at(day, {minutes}, {off})\n}}\n\n", args.join(", ")));
        }
    }
    if u.cal.is_some() {
        o.push_str(&doc(&is_open_doc(u, Some("IsOpen"))));
        o.push_str(&format!("{} {{\n\tz, err := _dayNumber(day)\n\tif err != nil {{\n\t\treturn false, err\n\t}}\n\treturn _isOpen(z)\n}}\n\n", T.is_open(&u.alias)));
    }
    while o.ends_with("\n\n") {
        o.pop();
    }
    o
}

/// `go/<package>/<package>.go`.
pub fn module(u: &Unit) -> String {
    let pkg = T.module(&u.alias);
    let mut o = String::new();
    for h in &u.header {
        o.push_str(&Comment::Slashes.line(h));
    }
    o.push('\n');
    let what = match u.kind {
        Kind::Dates => {
            let n = super::named(&u.name, &u.alias, false);
            u.t(tr!("Package {pkg} は {}の日付を計算する。", "Package {pkg} computes the dates of {}.", n.ja; n.en))
        }
        Kind::Calendar => {
            let n = super::named(&u.name, &u.alias, true);
            u.t(tr!("Package {pkg} は、ある日がカレンダー{}の営業日かを返す。", "Package {pkg} says which days are business days of the calendar {}.", n.ja; n.en))
        }
    };
    o.push_str(&format!("// {what}\npackage {pkg}\n\nimport \"fmt\"\n\n"));
    o.push_str(&doc(&[u.t(tr!(
        "Date は先発グレゴリオ暦の日付（0001-01-01〜9999-12-31。タイムゾーンを持たない）。",
        "Date is a day of the proleptic Gregorian calendar, 0001-01-01..9999-12-31, with no time zone."
    ))]));
    o.push_str("type Date struct {\n\tYear, Month, Day int\n}\n\n");
    o.push_str(&doc(&[u.t(tr!("ParseDate は YYYY-MM-DD を読む。", "ParseDate reads YYYY-MM-DD."))]));
    o.push_str("func ParseDate(s string) (Date, error) {\n\tz, err := _parseDate(s)\n\tif err != nil {\n\t\treturn Date{}, err\n\t}\n\treturn _date(z), nil\n}\n\n");
    o.push_str(&doc(&[u.t(tr!("String は日付を YYYY-MM-DD で書く。", "String writes the date as YYYY-MM-DD."))]));
    o.push_str("func (d Date) String() string {\n\treturn _ymd(d.Year, d.Month, d.Day)\n}\n\n");
    o.push_str(&doc(&[u.t(tr!(
        "KoyomiError はこのパッケージの関数が返すエラー。Kind は range（入力が範囲の外）、data（カレンダーが知らない日）、reject（else reject の無い日）、date（0001-01-01〜9999-12-31 の外か日付でない値）。",
        "KoyomiError is what the functions of this package return. Kind is range (an input outside its range), data (a day the calendar does not know), reject (a day the month does not have under else reject) or date (outside 0001-01-01..9999-12-31, or not a date)."
    ))]));
    o.push_str("type KoyomiError struct {\n\tKind    string\n\tMessage string\n}\n\n");
    o.push_str("func (e *KoyomiError) Error() string {\n\treturn e.Message\n}\n\n");
    o.push_str(&calendar(u));
    o.push_str(&helpers(u));
    o.push_str(&functions(u));
    o
}

/// `go/<package>/<package>_runner_test.go`.
pub fn runner(u: &Unit) -> String {
    let pkg = T.module(&u.alias);
    let mut o = String::new();
    o.push_str(&Comment::Slashes.line(&u.header[0]));
    o.push('\n');
    o.push_str(&format!("package {pkg}\n\n"));
    let strings = if u.kind == Kind::Dates { "\t\"strings\"\n" } else { "" };
    o.push_str(&format!("import (\n\t\"bufio\"\n\t\"encoding/json\"\n\t\"errors\"\n\t\"os\"\n{strings}\t\"testing\"\n)\n\n"));
    o.push_str(&doc(&[
        u.t(tr!(
            "TestMain は、環境変数 KOYOMI_RUNNER があれば、koyomi vectors の行を標準入力から読み、関数の結果を一行ずつ書く（空白で区切る。エラーなら error <種類>）。無ければふつうにテストを走らせる。",
            "TestMain, when KOYOMI_RUNNER is set, reads the lines of koyomi vectors on standard input and writes what the functions give, a line each (separated by a space; error <kind> for an error). Otherwise it runs the tests."
        )),
        u.t(tr!(
            "go test -c で作ったテストのプログラムを、KOYOMI_RUNNER=1 で走らせる。",
            "Build the test program with go test -c and run it with KOYOMI_RUNNER=1."
        )),
    ]));
    o.push_str(
        r#"func TestMain(m *testing.M) {
	if os.Getenv("KOYOMI_RUNNER") == "" {
		os.Exit(m.Run())
	}
	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 1<<20), 1<<20)
	out := bufio.NewWriter(os.Stdout)
	for in.Scan() {
		if len(in.Bytes()) == 0 {
			continue
		}
		var line struct {
			In map[string]any `json:"in"`
		}
		if err := json.Unmarshal(in.Bytes(), &line); err != nil {
			panic(err)
		}
		got, err := _runOne(line.In)
		var ke *KoyomiError
		if errors.As(err, &ke) {
			got = "error " + ke.Kind
		} else if err != nil {
			panic(err)
		}
		out.WriteString(got + "\n")
	}
	if err := out.Flush(); err != nil {
		panic(err)
	}
	os.Exit(0)
}

"#,
    );
    o.push_str("func _runOne(i map[string]any) (string, error) {\n");
    if u.kind == Kind::Calendar {
        o.push_str("\tday, err := ParseDate(i[\"date\"].(string))\n\tif err != nil {\n\t\treturn \"\", err\n\t}\n\topen, err := IsOpen(day)\n\tif err != nil {\n\t\treturn \"\", err\n\t}\n\tif open {\n\t\treturn \"true\", nil\n\t}\n\treturn \"false\", nil\n}\n");
        return o;
    }
    // The inputs, read once each.
    for (k, inp) in u.inputs.iter().enumerate() {
        if !u.dates.iter().any(|d| d.params.contains(&k)) {
            continue;
        }
        match inp.ty {
            Ty::Date => o.push_str(&format!("\tin{k}, err := ParseDate(i[{}].(string))\n\tif err != nil {{\n\t\treturn \"\", err\n\t}}\n", lit(&inp.name))),
            Ty::Int => o.push_str(&format!("\tin{k} := int(i[{}].(float64))\n", lit(&inp.name))),
        }
    }
    o.push_str("\tvar out []string\n");
    for (k, at) in u.outputs() {
        let d = &u.dates[k];
        let f = if at { T.function(&crate::naming::at_alias(&d.alias)) } else { T.function(&d.alias) };
        let args: Vec<String> = d.params.iter().map(|p| format!("in{p}")).collect();
        let value = if at { "v" } else { "v.String()" };
        o.push_str(&format!("\tif v, err := {f}({}); err != nil {{\n\t\treturn \"\", err\n\t}} else {{\n\t\tout = append(out, {value})\n\t}}\n", args.join(", ")));
    }
    o.push_str("\treturn strings.Join(out, \" \"), nil\n}\n");
    o
}
