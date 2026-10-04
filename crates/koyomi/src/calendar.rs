//! Calendars (DESIGN 1.4, 2.2, 2.4): which days are closed, the business-day conventions,
//! counting business days, and the range of days the calendar knows.
//!
//! A day is open when an `open` line names it, or when no `closed` line does. Asking about a
//! day outside the data range is an error, never a guess: the table does not say whether it
//! is a holiday (P4).

use crate::ast::{self, Conv, File, Kind, RuleKind, Span};
use crate::date::{self, Day, DateError};
use crate::diag::Diag;
use ritsu_base::text::{Text, count};
use crate::sources::{self, Law, Origin, Table};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One calendar file the calendar was made from.
#[derive(Clone, Debug)]
pub struct Info {
    pub name: String,
    pub alias: Option<String>,
    pub version: String,
    /// The path koyomi read it from.
    pub path: PathBuf,
    /// As diagnostics show it.
    pub shown: String,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct Every {
    pub from: (u32, u32),
    pub to: (u32, u32),
    pub name: Option<String>,
}

impl Every {
    pub fn covers(&self, m: u32, d: u32) -> bool {
        let x = (m, d);
        if self.from <= self.to { self.from <= x && x <= self.to } else { x >= self.from || x <= self.to }
    }

    pub fn text(&self) -> String {
        if self.from == self.to {
            format!("{:02}-{:02}", self.from.0, self.from.1)
        } else {
            format!("{:02}-{:02}..{:02}-{:02}", self.from.0, self.from.1, self.to.0, self.to.1)
        }
    }
}

/// Some days, closed or open, with a name.
#[derive(Clone, Debug)]
pub struct Stretch {
    pub from: Day,
    pub to: Day,
    pub name: Option<String>,
}

impl Stretch {
    pub fn text(&self) -> String {
        if self.from == self.to { self.from.to_string() } else { format!("{}..{}", self.from, self.to) }
    }
}

/// A calendar, with the calendars it read through `use calendar` merged into it.
#[derive(Clone, Debug)]
pub struct Calendar {
    pub info: Info,
    /// The calendars read through `use calendar`, nearest first.
    pub used: Vec<Info>,
    /// Minutes east of UTC.
    pub offset: Option<i32>,
    /// Closed days of the week, Monday first.
    pub weekly: [bool; 7],
    pub every: Vec<Every>,
    pub days: Vec<Stretch>,
    pub tables: Vec<Table>,
    pub opens: Vec<Stretch>,
    /// The days the calendar knows (DESIGN 1.4): where every table it reads overlaps.
    pub data: (Day, Day),
    pub laws: Vec<Law>,
    holidays: HashMap<Day, (usize, usize)>,
}

/// Why a day is closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    Weekday(u32),
    Holiday { table: String, name: String },
    Every { name: Option<String>, text: String },
    Day { name: Option<String>, text: String },
}

impl Reason {
    /// `日曜`, `憲法記念日`, `年末年始`: how the reason reads after a date.
    pub fn text(&self) -> Text {
        match self {
            Reason::Weekday(w) => tr!("{}曜", "{}", date::WEEKDAY_JA[*w as usize]; date::WEEKDAY_EN_LONG[*w as usize]),
            Reason::Holiday { table, name } => {
                if name.is_empty() {
                    tr!("表「{table}」の休み", "a day of the table {table}")
                } else {
                    Text::same(name.clone())
                }
            }
            Reason::Every { name: Some(n), .. } | Reason::Day { name: Some(n), .. } => Text::same(n.clone()),
            Reason::Every { name: None, text } => tr!("毎年の休み {text}", "closed every year on {text}"),
            Reason::Day { name: None, text } => tr!("休み {text}", "closed on {text}"),
        }
    }
}

/// Asking about a day the calendar does not know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outside(pub Day);

/// What can stop an operation that asks the calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalError {
    Outside(Day),
    Date(DateError),
}

impl From<Outside> for CalError {
    fn from(o: Outside) -> CalError {
        CalError::Outside(o.0)
    }
}

impl From<DateError> for CalError {
    fn from(e: DateError) -> CalError {
        CalError::Date(e)
    }
}

impl Calendar {
    pub fn is_open(&self, d: Day) -> Result<bool, Outside> {
        if d < self.data.0 || d > self.data.1 {
            return Err(Outside(d));
        }
        if self.opens.iter().any(|o| o.from <= d && d <= o.to) {
            return Ok(true);
        }
        Ok(!self.closed_by_rules(d))
    }

    fn closed_by_rules(&self, d: Day) -> bool {
        if self.weekly[d.weekday() as usize] {
            return true;
        }
        if self.holidays.contains_key(&d) {
            return true;
        }
        if !self.every.is_empty() {
            let (_, m, dd) = d.ymd();
            if self.every.iter().any(|e| e.covers(m, dd)) {
                return true;
            }
        }
        self.days.iter().any(|s| s.from <= d && d <= s.to)
    }

    /// Why a day is closed: every `closed` line that names it. Empty for an open day.
    pub fn reasons(&self, d: Day) -> Vec<Reason> {
        if self.opens.iter().any(|o| o.from <= d && d <= o.to) {
            return vec![];
        }
        let mut out = Vec::new();
        if self.weekly[d.weekday() as usize] {
            out.push(Reason::Weekday(d.weekday()));
        }
        if let Some((t, r)) = self.holidays.get(&d) {
            let t = &self.tables[*t];
            out.push(Reason::Holiday { table: t.name.clone(), name: t.rows[*r].name.clone() });
        }
        let (_, m, dd) = d.ymd();
        for e in &self.every {
            if e.covers(m, dd) {
                out.push(Reason::Every { name: e.name.clone(), text: e.text() });
            }
        }
        for s in &self.days {
            if s.from <= d && d <= s.to {
                out.push(Reason::Day { name: s.name.clone(), text: s.text() });
            }
        }
        out
    }

    /// The `open` line that opens a day the rules would close.
    pub fn opened_by(&self, d: Day) -> Option<&Stretch> {
        self.opens.iter().find(|o| o.from <= d && d <= o.to && self.closed_by_rules(d))
    }

    /// The first business day on or after `d`, or on or before it.
    fn seek(&self, d: Day, forward: bool) -> Result<Day, CalError> {
        let mut x = d;
        loop {
            if self.is_open(x)? {
                return Ok(x);
            }
            x = x.plus(if forward { 1 } else { -1 })?;
        }
    }

    /// The four conventions (DESIGN 1.8, 2.2).
    pub fn roll(&self, d: Day, c: Conv) -> Result<Day, CalError> {
        match c {
            Conv::Following => self.seek(d, true),
            Conv::Preceding => self.seek(d, false),
            Conv::ModifiedFollowing => {
                let f = self.seek(d, true)?;
                if same_month(f, d) { Ok(f) } else { self.seek(d, false) }
            }
            Conv::ModifiedPreceding => {
                let p = self.seek(d, false)?;
                if same_month(p, d) { Ok(p) } else { self.seek(d, true) }
            }
        }
    }

    /// `+ n business days` (`forward`) and `- n business days`: the `n`-th business day after
    /// (before) `d`, counting from the day after (before) it whether `d` is open or not. With
    /// `n` = 0, `d` rolled to a business day in the same direction (DESIGN 1.8).
    pub fn add_business(&self, d: Day, n: i64, forward: bool) -> Result<Day, CalError> {
        if n == 0 {
            return self.seek(d, forward);
        }
        let step = if forward { 1 } else { -1 };
        let mut x = d;
        let mut k = 0;
        loop {
            x = x.plus(step)?;
            if self.is_open(x)? {
                k += 1;
                if k == n {
                    return Ok(x);
                }
            }
        }
    }

    /// The business days of a year, when the calendar knows the whole year.
    pub fn open_days_in_year(&self, y: i32) -> Result<u32, Outside> {
        let a = Day::from_ymd(y as i64, 1, 1).unwrap();
        let b = Day::from_ymd(y as i64, 12, 31).unwrap();
        let mut n = 0;
        let mut d = a;
        while d <= b {
            if self.is_open(d)? {
                n += 1;
            }
            d = Day(d.0 + 1);
        }
        Ok(n)
    }

    /// The longest run of closed days that lies within `from..=to` (the first, when two are
    /// as long).
    pub fn longest_closed_run(&self, from: Day, to: Day) -> Option<(Day, Day)> {
        let mut best: Option<(Day, Day)> = None;
        let mut start: Option<Day> = None;
        let mut d = from;
        while d <= to {
            let closed = !self.is_open(d).unwrap_or(true);
            match (closed, start) {
                (true, None) => start = Some(d),
                (false, Some(s)) => {
                    let run = (s, Day(d.0 - 1));
                    if best.is_none_or(|b| run.1.0 - run.0.0 > b.1.0 - b.0.0) {
                        best = Some(run);
                    }
                    start = None;
                }
                _ => {}
            }
            d = Day(d.0 + 1);
        }
        if let Some(s) = start {
            let run = (s, to);
            if best.is_none_or(|b| run.1.0 - run.0.0 > b.1.0 - b.0.0) {
                best = Some(run);
            }
        }
        best
    }

    /// The offset as `+09:00`.
    pub fn offset_text(&self) -> Option<String> {
        self.offset.map(offset_text)
    }
}

pub fn same_month(a: Day, b: Day) -> bool {
    let (ya, ma, _) = a.ymd();
    let (yb, mb, _) = b.ymd();
    ya == yb && ma == mb
}

pub fn offset_text(m: i32) -> String {
    let sign = if m < 0 { '-' } else { '+' };
    let a = m.abs();
    format!("{sign}{:02}:{:02}", a / 60, a % 60)
}

/// Lexically clean a path: drop `.`, fold `..` into the component before it.
pub fn clean(p: &Path) -> PathBuf {
    let mut out: Vec<std::path::Component> = Vec::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => match out.last() {
                Some(std::path::Component::Normal(_)) => {
                    out.pop();
                }
                _ => out.push(c),
            },
            _ => out.push(c),
        }
    }
    out.iter().collect()
}

/// Reads calendar files once each, however many `.cal` files use them.
#[derive(Default)]
pub struct Loader {
    done: HashMap<PathBuf, Result<Calendar, ()>>,
    /// The calendars being read, for a `use calendar` that comes back round (E015).
    stack: Vec<PathBuf>,
}

/// What reading a calendar said.
pub struct Loaded {
    pub calendar: Option<Calendar>,
    pub diags: Vec<Diag>,
}

impl Loader {
    /// Read the calendar at `path` (`shown` is how diagnostics name it).
    pub fn load(&mut self, path: &Path, shown: &str) -> Loaded {
        let key = clean(path);
        if let Some(r) = self.done.get(&key) {
            return Loaded { calendar: r.clone().ok(), diags: vec![] };
        }
        let src = match ritsu_base::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Loaded { calendar: None, diags: vec![] },
        };
        let Ok(text) = String::from_utf8(src.clone()) else {
            return Loaded { calendar: None, diags: vec![] };
        };
        let parsed = crate::parse::parse(shown, &text);
        let mut diags = parsed.diags;
        let Some(f) = parsed.file else {
            self.done.insert(key, Err(()));
            return Loaded { calendar: None, diags };
        };
        self.stack.push(key.clone());
        let (cal, mut more) = self.build(&f, path, &src);
        self.stack.pop();
        diags.append(&mut more);
        let ok = !diags.iter().any(|d| d.is_error());
        let cal = if ok { cal } else { None };
        self.done.insert(key, cal.clone().ok_or(()));
        Loaded { calendar: cal, diags }
    }

    /// Whether `path` is a calendar being read right now (a `use calendar` cycle).
    pub fn in_progress(&self, path: &Path) -> bool {
        self.stack.contains(&clean(path))
    }

    /// Stages 2 and 3 of a calendar file (DESIGN 3.1).
    pub fn build(&mut self, f: &File, path: &Path, bytes: &[u8]) -> (Option<Calendar>, Vec<Diag>) {
        let mut diags = Vec::new();
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
        let err = |code: &'static str, s: Span, msg: Text| Diag::error(code, &f.path, s.line, s.col, msg).source(&f.src);
        let info = Info {
            name: f.name.text.clone(),
            alias: f.name.ascii().map(|s| s.to_string()),
            version: f.version.clone(),
            path: clean(path),
            shown: f.path.clone(),
            sha256: ritsu_base::sha256::hex(bytes),
        };
        // Names (stage 2).
        let mut seen: HashMap<&str, Span> = HashMap::new();
        for s in &f.sources {
            if let Some(prev) = seen.get(s.name.as_str()) {
                diags.push(err("E007", s.span, tr!("出典「{}」が二度宣言されています（{} 行目にもあります）", "The source {} is declared twice (line {} too)", s.name, prev.line)));
            } else {
                seen.insert(&s.name, s.span);
            }
        }
        diags.extend(crate::resolve::check_header(f));
        for r in &f.rules {
            if let RuleKind::Table(name, sp) = &r.what {
                match f.sources.iter().find(|s| &s.name == name) {
                    None => diags.push(err("E008", *sp, tr!("表「{name}」は宣言されていません", "The table {name} is not declared")).note(tr!(
                        "`closed <名前>` の名前は、同じファイルの `source <名前> = file …` で宣言した表です。",
                        "The name in `closed <name>` is a table declared in the same file with `source <name> = file …`."
                    ))),
                    Some(s) if matches!(s.kind, ast::SourceKind::Law { .. }) => diags.push(err("E011", *sp, tr!(
                        "「{name}」は法令で、休みの表ではありません。`closed` に書けるのは `= file` の表です",
                        "{name} is a law, not a table of closed days; `closed` takes a `= file` table"
                    ))),
                    _ => {}
                }
            }
        }
        // `use calendar` (E015).
        let mut base: Option<Calendar> = None;
        if let Some((p, sp)) = &f.use_calendar {
            match self.use_calendar(f, &dir, p, *sp) {
                Ok((c, mut ds)) => {
                    diags.append(&mut ds);
                    base = c;
                }
                Err(d) => diags.push(*d),
            }
        }
        if diags.iter().any(|d| d.is_error()) {
            return (None, diags);
        }
        // The calendar and its sources (stage 3).
        let origin = Origin { file: f, dir: &dir };
        let mut tables = Vec::new();
        for s in &f.sources {
            if matches!(s.kind, ast::SourceKind::File { .. }) {
                match sources::load_table(&origin, s) {
                    Ok(t) => tables.push(t),
                    Err(mut ds) => diags.append(&mut ds),
                }
            }
        }
        let (laws, mut ds) = sources::check_laws(&origin);
        diags.append(&mut ds);
        let offset = match &f.offset {
            Some((text, sp)) => match parse_offset(text) {
                Ok(m) => Some(m),
                Err(d) => {
                    let mut x = err(d.0, *sp, d.1);
                    x.notes = d.2;
                    diags.push(x);
                    None
                }
            },
            None => None,
        };
        if diags.iter().any(|d| d.is_error()) {
            return (None, diags);
        }
        let mut cal = match base {
            Some(b) => {
                let mut used = vec![b.info.clone()];
                used.extend(b.used.iter().cloned());
                Calendar { info: info.clone(), used, laws: vec![], ..b }
            }
            None => Calendar {
                info: info.clone(),
                used: vec![],
                offset: None,
                weekly: [false; 7],
                every: vec![],
                days: vec![],
                tables: vec![],
                opens: vec![],
                data: (date::MIN, date::MAX),
                laws: vec![],
                holidays: HashMap::new(),
            },
        };
        if let (Some(mine), Some(theirs)) = (offset, cal.offset)
            && mine != theirs
        {
            let (_, sp) = f.offset.as_ref().unwrap();
            let used = &cal.used[0].shown;
            diags.push(err("E015", *sp, tr!(
                "オフセット {} が、use calendar で読んだ {used} のオフセット {} と違います",
                "The offset {} differs from {}, the offset of {used}, read with use calendar",
                offset_text(mine),
                offset_text(theirs)
            ))
            .note(tr!("オフセットは一つのカレンダーに一つです。どちらかに合わせます。", "A calendar has one offset; make the two agree.")));
            return (None, diags);
        }
        if offset.is_some() {
            cal.offset = offset;
        }
        cal.laws = laws;
        for r in &f.rules {
            match &r.what {
                RuleKind::Weekly(ds) => {
                    for d in ds {
                        cal.weekly[*d as usize] = true;
                    }
                }
                RuleKind::Table(name, _) => {
                    if let Some(t) = tables.iter().find(|t| &t.name == name) {
                        cal.tables.push(t.clone());
                    }
                }
                RuleKind::Every { from, to, name } => cal.every.push(Every { from: *from, to: *to, name: name.clone() }),
                RuleKind::Days { from, to, name } => cal.days.push(Stretch { from: *from, to: *to, name: name.clone() }),
                RuleKind::Open { from, to, name } => cal.opens.push(Stretch { from: *from, to: *to, name: name.clone() }),
            }
        }
        cal.holidays.clear();
        for (ti, t) in cal.tables.iter().enumerate() {
            for (ri, r) in t.rows.iter().enumerate() {
                cal.holidays.entry(r.day).or_insert((ti, ri));
            }
        }
        cal.data = cal.tables.iter().fold((date::MIN, date::MAX), |(a, b), t| (a.max(t.covers.0), b.min(t.covers.1)));
        // E108: no business day at all.
        let any_open = if cal.data.0 > cal.data.1 {
            false
        } else {
            let mut d = cal.data.0;
            let mut found = false;
            loop {
                if cal.is_open(d) == Ok(true) {
                    found = true;
                    break;
                }
                if d >= cal.data.1 {
                    break;
                }
                d = Day(d.0 + 1);
            }
            found
        };
        if !any_open {
            let sp = Span { line: f.name.span.line, col: 1 };
            let mut d = err("E108", sp, tr!("このカレンダーには営業日が一日もありません", "This calendar has no business day at all"));
            d = if cal.data.0 > cal.data.1 {
                d.note(tr!(
                    "表の `covers` が重なる日がありません。データの範囲は、どの表も知っている日です。",
                    "The tables' `covers` have no day in common, and the data range is the days every table knows."
                ))
            } else {
                d.note(tr!(
                    "データの範囲 {}〜{} のどの日も休みです。翌営業日を探す計算が終わらなくなります。",
                    "Every day of the data range {}..{} is closed, so looking for the next business day would never end.",
                    cal.data.0,
                    cal.data.1
                ))
            };
            diags.push(d);
            return (None, diags);
        }
        // W101: an open day that the rules leave open anyway.
        for r in &f.rules {
            if let RuleKind::Open { from, to, .. } = &r.what {
                let mut already = Vec::new();
                let mut d = *from;
                while d <= *to {
                    if d >= cal.data.0 && d <= cal.data.1 && !cal.closed_by_rules(d) {
                        already.push(d);
                    }
                    d = Day(d.0 + 1);
                }
                if !already.is_empty() {
                    let shown: Vec<String> = already.iter().take(6).map(|d| crate::diag::day_with_weekday(*d, ritsu_base::text::Lang::Ja)).collect();
                    let shown_en: Vec<String> = already.iter().take(6).map(|d| crate::diag::day_with_weekday(*d, ritsu_base::text::Lang::En)).collect();
                    diags.push(
                        Diag::warning("W101", &f.path, r.span.line, r.span.col, Text::new(
                            format!("`open` に書いた {}は、もともと営業日です", shown.join("、")),
                            format!("{} written under `open` is a business day anyway", shown_en.join(", ")),
                        ))
                        .source(&f.src)
                        .note(tr!(
                            "どの `closed` にも当たらない日です。日付の書き違いでなければ、この行は要りません。",
                            "No `closed` line names it. Unless the date is a slip, the line is not needed."
                        )),
                    );
                }
            }
        }
        let _ = count;
        (Some(cal), diags)
    }

    /// `use calendar "<file>"`: read it, or say why it cannot be used (E015).
    pub fn use_calendar(&mut self, f: &File, dir: &Path, p: &str, sp: Span) -> Result<(Option<Calendar>, Vec<Diag>), Box<Diag>> {
        let err = |msg: Text| Box::new(Diag::error("E015", &f.path, sp.line, sp.col, msg).source(&f.src));
        let path = dir.join(p);
        let shown = clean(&Path::new(&f.path).parent().unwrap_or(Path::new("")).join(p)).to_string_lossy().to_string();
        let where_ = if shown == p { String::new() } else { format!("（{shown}）") };
        let where_en = if shown == p { String::new() } else { format!(" ({shown})") };
        if self.in_progress(&path) {
            return Err(err(tr!("{p} を読むと、このファイルに戻ってきます（use calendar が循環しています）", "Reading {p} leads back to this file (the use calendar lines go round)")));
        }
        let Ok(bytes) = ritsu_base::fs::read(&path) else {
            return Err(Box::new(err(tr!("カレンダー {p} が読めません{where_}", "The calendar {p} cannot be read{where_en}"))
                .note(tr!("パスは、この .cal のあるディレクトリから数えます。", "The path is relative to the directory this .cal is in."))));
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return Err(err(tr!("カレンダー {p} が UTF-8 ではありません", "The calendar {p} is not UTF-8")));
        };
        let first = text.lines().map(|l| l.trim()).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("");
        if !first.starts_with(&format!("{} ", crate::kw::CALENDAR)) {
            return Err(err(tr!(
                "{p} はカレンダーのファイルではありません（一行目が `calendar …` ではありません）",
                "{p} is not a calendar file (its first line is not `calendar …`)"
            )));
        }
        let loaded = self.load(&path, &shown);
        if loaded.calendar.is_none() && loaded.diags.is_empty() {
            // Read before and refused then; say so here, where it is used.
            return Err(err(tr!("カレンダー {p} にエラーがあるので使えません", "The calendar {p} has errors, so it cannot be used")));
        }
        Ok((loaded.calendar, loaded.diags))
    }
}

/// `+09:00` as minutes east of UTC, or why it is not one (E107, E006).
pub fn parse_offset(text: &str) -> Result<i32, (&'static str, Text, Vec<Text>)> {
    let t = text.trim();
    let b = t.as_bytes();
    if b.len() == 6 && (b[0] == b'+' || b[0] == b'-') && b[1].is_ascii_digit() && b[2].is_ascii_digit() && b[3] == b':' && b[4].is_ascii_digit() && b[5].is_ascii_digit() {
        let h: i32 = t[1..3].parse().unwrap();
        let m: i32 = t[4..6].parse().unwrap();
        if h > 23 || m > 59 {
            return Err(("E006", tr!("{t} というオフセットはありません", "There is no offset {t}"), vec![]));
        }
        let v = h * 60 + m;
        return Ok(if b[0] == b'-' { -v } else { v });
    }
    let upper = t.to_ascii_uppercase();
    if upper == "UTC" || upper == "GMT" || upper == "Z" {
        return Err((
            "E107",
            tr!("オフセットは `±HH:MM` の形で書きます（`{t}` ではなく `+00:00`）", "Write the offset as `±HH:MM`: `+00:00`, not `{t}`"),
            vec![],
        ));
    }
    let looks_like_zone = t.contains('/') || t.chars().any(|c| c.is_ascii_alphabetic());
    if looks_like_zone {
        return Err((
            "E107",
            tr!("オフセットにタイムゾーンの名前 `{t}` が書かれています。koyomi は固定の UTC オフセットだけを扱います", "The offset is a time zone name, `{t}`; koyomi takes only a fixed UTC offset"),
            vec![
                tr!(
                    "夏時間のあるタイムゾーンでは、同じ 09:00 でも季節でオフセットが変わり、固定のオフセットでは一年の半分で一時間ずれます。どのタイムゾーンに夏時間があるかを知るには tz データベースが要るので、名前は全部断ります（DESIGN 1.9）。",
                    "In a time zone with daylight saving time, the same 09:00 has a different offset in summer, and a fixed offset is an hour off for half the year. Knowing which zones have it takes the tz database, so every name is refused (DESIGN 1.9)."
                ),
                tr!(
                    "夏時間の無い地域なら、`offset +09:00` のように数で書けば足ります。夏時間のある地域では、時刻を出さず日付だけにします。",
                    "For a place without daylight saving time, write the number, like `offset +09:00`. For a place with it, leave times out and give dates only."
                ),
            ],
        ));
    }
    Err(("E107", tr!("オフセット `{t}` は `±HH:MM` の形ではありません（`+09:00` のように書きます）", "The offset `{t}` is not `±HH:MM` (write it like `+09:00`)"), vec![]))
}

/// The calendar a dates file is checked against has a kind: a calendar file checked on its own
/// is `Kind::Calendar`.
pub fn is_calendar(f: &File) -> bool {
    f.kind == Kind::Calendar
}
