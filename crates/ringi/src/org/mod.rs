//! The data of an organization (DESIGN 8): a directory that holds one directory for each version,
//! named after the day the version takes effect (`2026-04-01/`), each with `teams.csv`,
//! `people.csv` and `members.csv` (or one `organization.json` of the same shape), and the proxies
//! of every version beside them (`proxies.csv`, DESIGN 5.8).
//!
//! The data is read when the organization's file is checked, once in a run, and kept with it: a
//! request that reads the organization reads the versions from there. A mistake in the data is
//! said once, at its row (E501, E505), and the organization's file does not pass; a request that
//! reads it then waits for it (E202), and its own checks that need the data are not made.
//!
//! - [`data`]: the CSV and JSON files, and the paths they are named by.
//! - [`masters`]: the data of masters (DESIGN 4.6).
//! - [`resolve`]: who answers a stage, for a requester, in a version (DESIGN 8.5).
//! - [`listing`]: every version, every requester, every stage of a request, and what is said of
//!   them (E502, W502, E503, E504, W506, W507; W500 without data; DESIGN 8.6).
//! - [`submit`]: who may submit a request (`submitted by`; E508, E509; DESIGN 5.13).

pub mod data;
pub mod listing;
pub mod masters;
pub mod resolve;
pub mod submit;

use crate::ast::{File, Titled};
use crate::diag::Diag;
use data::{Row, Table, Unread};
use ritsu_base::text::Text;
use std::collections::HashMap;
use std::path::Path;

/// How far up the teams are followed (DESIGN 8.5): deeper is a mistake in the data (E501).
pub const DEPTH: usize = 30;

/// A team of a version.
#[derive(Clone, Debug)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub name_ja: String,
    /// The kind, as the organization's file names it.
    pub kind: String,
    pub parent: Option<String>,
    pub head: Option<String>,
    pub line: usize,
}

/// A person of a version.
#[derive(Clone, Debug)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub name_ja: String,
    pub email: String,
    /// The language they read in (`en` or `ja`, the column [`LANGUAGE`]); empty for the language of
    /// each kind.
    pub language: String,
    pub line: usize,
}

/// A row of `members.csv`: a person in a team, with a position.
#[derive(Clone, Debug)]
pub struct Member {
    pub person: String,
    pub team: String,
    /// The position, by its place among the organization's positions (0 the lowest).
    pub position: usize,
    pub primary: bool,
    pub line: usize,
}

/// One version of the organization: the day it takes effect, and its teams, people and members.
#[derive(Clone, Debug, Default)]
pub struct Version {
    pub from: i64,
    /// The version's directory, as a diagnostic names it.
    pub dir: String,
    /// The files its teams, people and members are read from, as a diagnostic names them.
    pub teams_file: String,
    pub people_file: String,
    pub members_file: String,
    pub teams: Vec<Team>,
    pub people: Vec<Person>,
    pub members: Vec<Member>,
    team_ix: HashMap<String, usize>,
    person_ix: HashMap<String, usize>,
    /// Each person's rows of `members.csv`.
    of_person: HashMap<String, Vec<usize>>,
    /// Each team's rows of `members.csv`.
    of_team: HashMap<String, Vec<usize>>,
}

impl Version {
    pub fn team(&self, id: &str) -> Option<&Team> {
        self.team_ix.get(id).map(|&i| &self.teams[i])
    }

    pub fn person(&self, id: &str) -> Option<&Person> {
        self.person_ix.get(id).map(|&i| &self.people[i])
    }

    /// The person's primary row of `members.csv`.
    pub fn primary(&self, person: &str) -> Option<&Member> {
        self.of_person.get(person)?.iter().map(|&i| &self.members[i]).find(|m| m.primary)
    }

    /// Every row of `members.csv` of the person.
    pub fn memberships(&self, person: &str) -> impl Iterator<Item = &Member> {
        self.of_person.get(person).into_iter().flatten().map(|&i| &self.members[i])
    }

    /// Every row of `members.csv` of the team.
    pub fn members(&self, team: &str) -> impl Iterator<Item = &Member> {
        self.of_team.get(team).into_iter().flatten().map(|&i| &self.members[i])
    }

    /// The person's position in the team, when they are a member of it.
    pub fn position_in(&self, person: &str, team: &str) -> Option<usize> {
        self.memberships(person).find(|m| m.team == team).map(|m| m.position)
    }

    /// The team and the teams above it, nearest first, at most [`DEPTH`] of them (the data was
    /// read with no ring and none deeper).
    pub fn up(&self, team: &str) -> Vec<&Team> {
        let mut out = Vec::new();
        let mut at = self.team(team);
        while let Some(t) = at {
            if out.len() >= DEPTH {
                break;
            }
            out.push(t);
            at = t.parent.as_deref().and_then(|p| self.team(p));
        }
        out
    }

    /// The part of the version a ticket needs (DESIGN 7.5): these people and these teams, with the
    /// rows of `members.csv` that join them, as a version of its own. A team keeps its parent and
    /// its head as the data gives them, whether they are in the part or not.
    pub fn part(&self, people: &std::collections::BTreeSet<String>, teams: &std::collections::BTreeSet<String>) -> Version {
        let mut v = Version {
            from: self.from,
            dir: self.dir.clone(),
            teams_file: self.teams_file.clone(),
            people_file: self.people_file.clone(),
            members_file: self.members_file.clone(),
            teams: self.teams.iter().filter(|t| teams.contains(&t.id)).cloned().collect(),
            people: self.people.iter().filter(|p| people.contains(&p.id)).cloned().collect(),
            members: self.members.iter().filter(|m| people.contains(&m.person) && teams.contains(&m.team)).cloned().collect(),
            ..Version::default()
        };
        v.index();
        v
    }

    /// The day it takes effect, as a date.
    pub fn day(&self) -> String {
        crate::lex::date_text(self.from)
    }

    fn index(&mut self) {
        self.team_ix = self.teams.iter().enumerate().map(|(i, t)| (t.id.clone(), i)).collect();
        self.person_ix = self.people.iter().enumerate().map(|(i, p)| (p.id.clone(), i)).collect();
        self.of_person.clear();
        self.of_team.clear();
        for (i, m) in self.members.iter().enumerate() {
            self.of_person.entry(m.person.clone()).or_default().push(i);
            self.of_team.entry(m.team.clone()).or_default().push(i);
        }
    }

    /// A person by their ID and name: `e3101 (Head of Sales Department 1)`, the Japanese name when
    /// the data has one; the ID alone for one the version does not have.
    pub fn who(&self, id: &str) -> Text {
        match self.person(id) {
            Some(p) => named(id, &p.name, &p.name_ja),
            None => Text::same(id.to_string()),
        }
    }

    /// A team by its ID and name, the same way.
    pub fn team_named(&self, id: &str) -> Text {
        match self.team(id) {
            Some(t) => named(id, &t.name, &t.name_ja),
            None => Text::same(id.to_string()),
        }
    }
}

/// How many of how many in each version, as a line under `ok` says it, from each version's day,
/// the number and the whole: `19 of 24 people in the organization from 2026-04-01, and 18 of 23 from
/// 2027-04-01` (with `noun` after the first whole, its one and its many; none where the line says
/// who before), `2026-04-01 からの組織の 24 人のうち 19 人、2027-04-01 からの 23 人のうち 18 人`.
pub fn of_versions(counts: &[(String, usize, usize)], noun: Option<(&str, &str)>) -> Text {
    let en: Vec<String> = counts
        .iter()
        .enumerate()
        .map(|(i, (d, k, n))| {
            let word = noun.map(|(one, many)| format!(" {}", if *n == 1 { one } else { many })).unwrap_or_default();
            if i == 0 { format!("{k} of {n}{word} in the organization from {d}") } else { format!("{k} of {n} from {d}") }
        })
        .collect();
    let en = match en.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a}, and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    };
    let ja: Vec<String> = counts.iter().enumerate().map(|(i, (d, k, n))| if i == 0 { format!("{d} からの組織の {n} 人のうち {k} 人") } else { format!("{d} からの {n} 人のうち {k} 人") }).collect();
    Text::new(ja.join("、"), en)
}

/// A name or a list of names in a Japanese sentence, before the word that follows it: with a space
/// when it ends with a letter of ASCII (`e1001 が`), none after a closing bracket (`e1001（経理部長）が`).
pub fn ja_word(s: &str) -> String {
    if s.chars().last().is_some_and(|c| c.is_ascii()) { format!("{s} ") } else { s.to_string() }
}

/// `id (name)` in English, `id（名前）` in Japanese with the Japanese name when there is one.
pub fn named(id: &str, name: &str, name_ja: &str) -> Text {
    let ja_name = if name_ja.is_empty() { name } else { name_ja };
    let en = if name.is_empty() || name == id { id.to_string() } else { format!("{id} ({name})") };
    let ja = if ja_name.is_empty() || ja_name == id { id.to_string() } else { format!("{id}（{ja_name}）") };
    Text::new(ja, en)
}

/// What a row of `proxies.csv` lets the proxy do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyKind {
    /// Answer the person's stages for them.
    Approve,
    /// Make requests in the person's name.
    Submit,
}

/// A row of `proxies.csv` (DESIGN 5.8).
#[derive(Clone, Debug)]
pub struct Proxy {
    pub person: String,
    pub proxy: String,
    pub kind: ProxyKind,
    pub from: i64,
    /// The last day, both ends in; None for no end.
    pub to: Option<i64>,
    /// The requests it is for, by their names (`expense`); empty for every request.
    pub requests: Vec<String>,
    pub line: usize,
}

impl Proxy {
    /// Whether it is in effect on `day`, for the request named `request`.
    pub fn on(&self, day: i64, request: &str) -> bool {
        self.from <= day && self.to.is_none_or(|t| day <= t) && self.covers(request)
    }

    /// Whether it is for the request named `request`.
    pub fn covers(&self, request: &str) -> bool {
        self.requests.is_empty() || self.requests.iter().any(|r| r == request)
    }

    /// Whether it is in effect on some day of `from..=to` (to None for no end).
    pub fn meets(&self, from: i64, to: Option<i64>) -> bool {
        self.to.is_none_or(|t| t >= from) && to.is_none_or(|e| self.from <= e)
    }

    /// Its days, as a text: `2026-12-28 to 2027-01-08`, `2026-12-28 with no end`.
    pub fn days(&self) -> Text {
        let a = crate::lex::date_text(self.from);
        match self.to {
            Some(t) => {
                let b = crate::lex::date_text(t);
                Text::new(format!("{a}〜{b}"), format!("{a} to {b}"))
            }
            None => Text::new(format!("{a} から終わり無し"), format!("{a} with no end")),
        }
    }
}

/// The data of an organization, read.
#[derive(Clone, Debug, Default)]
pub struct Org {
    /// Where the data is, as the file writes it (`../org`) or as `--org` gives it.
    pub written: String,
    /// The directory, as a diagnostic names it.
    pub dir: String,
    /// The versions checked, by the day they take effect: every one, or with `--at` the one in
    /// effect on that day.
    pub versions: Vec<Version>,
    /// The day of every version there is, `--at` or not: where each version's days end.
    pub days: Vec<i64>,
    pub proxies: Vec<Proxy>,
    /// `proxies.csv`, as a diagnostic names it, when it is there.
    pub proxies_file: Option<String>,
    /// The positions, lowest first, each as written and by its ASCII name.
    pub positions: Vec<(String, String)>,
    /// The day `--at` gives, when it is given.
    pub at: Option<i64>,
}

impl Org {
    /// The version in effect on `day`, among the ones checked.
    pub fn in_effect(&self, day: i64) -> Option<&Version> {
        self.versions.iter().rev().find(|v| v.from <= day)
    }

    /// The days a version is in effect: from its day to the day before the next version's, or with
    /// no end for the last.
    pub fn period(&self, v: &Version) -> (i64, Option<i64>) {
        let next = self.days.iter().copied().filter(|d| *d > v.from).min();
        (v.from, next.map(|n| n - 1))
    }

    /// The day of the first version there is.
    pub fn first(&self) -> Option<i64> {
        self.days.iter().copied().min()
    }

    /// A position by its place: as written.
    pub fn position(&self, rank: usize) -> &str {
        self.positions.get(rank).map(|p| p.0.as_str()).unwrap_or("")
    }

    /// The place of a position, by its name as written or its ASCII name.
    pub fn rank(&self, name: &str) -> Option<usize> {
        self.positions.iter().position(|(w, a)| w == name || a == name)
    }

    /// What `ok` says of the data: the versions, the people of the latest (with `--at`, of the one
    /// in effect on its day, the only one checked), the proxies.
    pub fn facts(&self) -> Text {
        let days: Vec<String> = self.versions.iter().map(|v| v.day()).collect();
        let n = days.len();
        let (list, list_ja) = (days.join(", "), days.join("、"));
        let w = &self.written;
        let last = self.versions.last();
        let (people, teams) = last.map(|v| (v.people.len(), v.teams.len())).unwrap_or((0, 0));
        let p = self.proxies.len();
        let ps = match p {
            0 => Text::new("代理無し", "no proxies"),
            1 => Text::new("代理 1 行", "1 proxy"),
            _ => Text::new(format!("代理 {p} 行"), format!("{p} proxies")),
        };
        match (self.at, n) {
            (Some(d), _) => {
                let d = crate::lex::date_text(d);
                tr!(
                    "データ `{w}`: {d}（`--at`）に効いている {list_ja} からの版、チーム {teams} 個と {people} 人、{}",
                    "data `{w}`: the version from {list}, the one in effect on {d} (`--at`), {teams} teams and {people} people, {}",
                    ps.ja; ps.en
                )
            }
            (None, 1) => tr!(
                "データ `{w}`: 版 1 個（{list_ja}）、チーム {teams} 個と {people} 人、{}",
                "data `{w}`: 1 version ({list}), {teams} teams and {people} people, {}",
                ps.ja; ps.en
            ),
            (None, _) => tr!(
                "データ `{w}`: 版 {n} 個（{list_ja}）、最後の版にチーム {teams} 個と {people} 人、{}",
                "data `{w}`: {n} versions ({list}), {teams} teams and {people} people in the last, {}",
                ps.ja; ps.en
            ),
        }
    }
}

/// The data a file of ringi's read when it was checked, kept with it for the files that read it.
#[derive(Clone, Debug)]
pub enum Data {
    /// An organization's versions and proxies.
    Org(Org),
    /// An organization with no data to read, and why: the checks that need it are not decided
    /// (W500).
    NoOrg(NoData),
    /// The rows of masters.
    Masters(masters::MastersData),
}

/// What reading an organization's data comes to.
pub struct Read {
    /// The data; None when there is none to read (no `data` line and no `--org`, a place with no
    /// version in it), or when a mistake in it is said.
    pub org: Option<Org>,
    pub diags: Vec<Diag>,
    /// Why there is no data, when there is none.
    pub none: Option<NoData>,
}

/// Why an organization has no data to check against, and how to give it some (W500).
#[derive(Clone, Debug)]
pub struct NoData {
    /// What is missing, as the headline says it: `the organization has no data`, `no version of the
    /// organization is in effect on 2025-01-01`.
    pub what: Text,
    /// Why, as a sentence.
    pub why: Text,
    /// How to give it the data.
    pub fix: Text,
    /// As the line under the organization's `ok` line says it, after `no data:`.
    pub line: Text,
}

impl NoData {
    /// The organization has no data, for `why`, and `fix` gives it some.
    fn missing(why: Text, fix: Text) -> NoData {
        NoData { what: tr!("組織のデータが無い", "the organization has no data"), line: why.clone(), why, fix }
    }
}

/// What the data would let the check say, after the way to give it.
fn with_the_data(give: Text) -> Text {
    tr!(
        "{}。データがあれば、承認者のいないステップ、届かない議決、二つのステップに当たる同じ人を言います。",
        "{}: with the data, the check says which stages have nobody, which quorums cannot be met and who answers two stages.",
        give.ja; give.en
    )
}

/// The name of the one JSON file a version may hold instead of the three CSV files.
pub const JSON_FILE: &str = "organization.json";

/// The column of `people.csv` (and the key of a person in `organization.json`) that holds the
/// language a person reads in (DESIGN 8.2): one of [`crate::screens::LANGS`], or empty for the
/// language of each kind. Its one name, which the check, the schema and the columns read.
pub const LANGUAGE: &str = "language";

const TEAMS: [&str; 5] = ["id", "name", "kind", "parent", "head"];
const PEOPLE: [&str; 2] = ["id", "name"];
const MEMBERS: [&str; 4] = ["person", "team", "position", "primary"];
const PROXIES: [&str; 5] = ["person", "proxy", "kind", "from", "to"];

fn e501(t: &Table, line: usize, msg: Text) -> Diag {
    let d = Diag::error("E501", &t.file, line.max(1), 1, msg);
    if line == 0 { d } else { d.source(&t.src) }
}

fn unread(file: &str, src: Option<&str>, u: Unread) -> Diag {
    let d = Diag::error("E501", file, u.line.max(1), 1, u.message);
    match src {
        Some(s) if u.line > 0 => d.source(s),
        _ => d,
    }
}

// ── What the values of the data are held to, as the server holds them ──

/// How many bytes an ID of a person or a team, or a master's key, may be at the most.
pub(super) const LONGEST_ID: usize = 255;

/// How many Unicode code points a name of a person or a team may be at the most.
const LONGEST_NAME: usize = 200;

/// How many bytes a person's address may be at the most.
const LONGEST_ADDRESS: usize = 254;

/// The characters besides white space an address does not hold: the specials of the mail's
/// standards.
const ADDRESS_SPECIALS: &str = "\"(),:;<>[\\]";

/// The first character of an ID, a name, an address or a master's value the data may not hold, and
/// what it is: a control character; a character that turns the direction of the text after it
/// (U+202A–U+202E, U+2066–U+2069); another formatting character, which does not show (the
/// zero-width marks, the byte order mark U+FEFF and the rest of Unicode's Cf); or a separator of
/// lines or of paragraphs (Zl, Zp). None where `s` holds none of them.
pub(super) fn unfit(s: &str) -> Option<Text> {
    for c in s.chars() {
        let u = format!("U+{:04X}", c as u32);
        if matches!(c as u32, 0x202A..=0x202E | 0x2066..=0x2069) {
            return Some(Text::new(format!("あとに続く文字の向きを変える文字（{u}）"), format!("{u}, a character that turns the direction of the text after it")));
        }
        if c.is_control() {
            return Some(Text::new(format!("制御文字（{u}）"), format!("a control character ({u})")));
        }
        if is_format(c) {
            return Some(Text::new(format!("目に見えない書式の文字（{u}）"), format!("{u}, a formatting character that does not show")));
        }
        if matches!(c as u32, 0x2028 | 0x2029) {
            return Some(Text::new(format!("行や段落を区切る文字（{u}）"), format!("{u}, a character that separates lines or paragraphs")));
        }
    }
    None
}

/// Whether `c` is a formatting character (Unicode's Cf, as Go's `unicode.Cf` holds it).
fn is_format(c: char) -> bool {
    matches!(c as u32,
        0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x0890..=0x0891 | 0x08E2 | 0x180E
        | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF
        | 0xFFF9..=0xFFFB | 0x110BD | 0x110CD | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3
        | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F)
}

/// A value of the data as a diagnostic writes it: its first forty characters, with one that would
/// not show as itself (a control, a formatting, a line- or paragraph-separating character, a
/// backslash or a backquote) written as an escape, and `…` for what is left out: so that a
/// diagnostic stays one line, as the server's `escaped` writes it.
pub(super) fn escaped(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    for c in it.by_ref().take(40) {
        match c {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            c if c.is_control() || is_format(c) || matches!(c as u32, 0x2028 | 0x2029) => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    if it.next().is_some() {
        out.push('…');
    }
    out
}

/// The checks of an ID of a person or a team (`of`, `of_ja` as a diagnostic says it): no longer
/// than `LONGEST_ID` bytes, and no character `unfit` refuses.
fn check_id(t: &Table, line: usize, of: &str, of_ja: &str, id: &str, diags: &mut Vec<Diag>) {
    let n = id.len();
    if n > LONGEST_ID {
        let id = escaped(id);
        diags.push(e501(t, line, tr!("{of_ja} `{id}` の `id` が{n}バイトあります。ID は{LONGEST_ID}バイトまでです", "The `id` of the {of} `{id}` is {n} bytes long; an ID is {LONGEST_ID} bytes at the most")));
    }
    if let Some(what) = unfit(id) {
        let id = escaped(id);
        diags.push(e501(t, line, tr!("{of_ja} `{id}` の `id` に{}があります", "The `id` of the {of} `{id}` holds {}", what.ja; what.en)));
    }
}

/// The checks of a name of a person or a team: no longer than `LONGEST_NAME` characters, and no
/// character `unfit` refuses. `of` is what the name is a name of, in English and in Japanese.
fn check_name(t: &Table, line: usize, of: (&str, &str), id: &str, column: &str, name: &str, diags: &mut Vec<Diag>) {
    let n = name.chars().count();
    let (of, of_ja) = of;
    let id = escaped(id);
    if n > LONGEST_NAME {
        diags.push(e501(t, line, tr!("{of_ja} `{id}` の `{column}` が{n}字あります。名前は{LONGEST_NAME}字までです", "The `{column}` of the {of} `{id}` is {n} characters long; a name is {LONGEST_NAME} characters at the most")));
    }
    if let Some(what) = unfit(name) {
        diags.push(e501(t, line, tr!("{of_ja} `{id}` の `{column}` に{}があります", "The `{column}` of the {of} `{id}` holds {}", what.ja; what.en)));
    }
}

/// The checks of a person's `email`, which may be empty: no longer than `LONGEST_ADDRESS` bytes,
/// with no character `unfit` refuses, and written as one address (trimmed of the space around it,
/// one `@` with something before it and after it, no white space and none of `ADDRESS_SPECIALS`).
/// Reports whether the address passes.
fn check_address(t: &Table, line: usize, id: &str, addr: &str, diags: &mut Vec<Diag>) -> bool {
    if addr.is_empty() {
        return true;
    }
    let id = escaped(id);
    let n = addr.len();
    if n > LONGEST_ADDRESS {
        diags.push(e501(t, line, tr!("人 `{id}` の `email` が{n}バイトあります。アドレスは{LONGEST_ADDRESS}バイトまでです", "The `email` of the person `{id}` is {n} bytes long; an address is {LONGEST_ADDRESS} bytes at the most")));
        return false;
    }
    if let Some(what) = unfit(addr) {
        diags.push(e501(t, line, tr!("人 `{id}` の `email` に{}があります", "The `email` of the person `{id}` holds {}", what.ja; what.en)));
        return false;
    }
    let trimmed = addr.trim();
    let shaped = match trimmed.split_once('@') {
        Some((local, domain)) => !local.is_empty() && !domain.is_empty() && !domain.contains('@'),
        None => false,
    };
    if !shaped || trimmed.chars().any(|c| c.is_whitespace() || ADDRESS_SPECIALS.contains(c)) {
        let addr = escaped(addr);
        diags.push(e501(t, line, tr!("人 `{id}` の `email`（`{addr}`）が、アドレスの形ではありません。アドレスは、`@` が一つで、その前と後に文字がある形です。空白と `{ADDRESS_SPECIALS}` の文字は使えません", "The `email` of the person `{id}`, `{addr}`, is not written as an address: an address has one `@`, something before it and after it, and no space and none of `{ADDRESS_SPECIALS}`")));
        return false;
    }
    true
}

/// An address as two are compared where a person signs in by it (Go's `strings.EqualFold`): each
/// character as the least of the characters that fold to it, so that two addresses EqualFold takes
/// as one are one (U+212A as `k`, U+017F as `s`).
fn folded(s: &str) -> String {
    s.chars().map(fold_least).collect()
}

/// The least character of the orbit `c` is in under the simple case folding.
fn fold_least(c: char) -> char {
    let mut least = c;
    let mut f = simple_fold(c);
    while f != c {
        least = least.min(f);
        f = simple_fold(f);
    }
    least
}

/// The next code point of `c`'s orbit under the simple case folding, as Go's `unicode.SimpleFold`
/// answers it: the least code point of the orbit that is above `c`, or the orbit's least where
/// there is none.
fn simple_fold(c: char) -> char {
    let r = c as u32;
    if r < 0x80 {
        return match r {
            0x41..=0x5A => char::from_u32(r + 0x20).unwrap(),
            0x61..=0x7A => match r {
                0x6B => '\u{212A}',
                0x73 => '\u{017F}',
                _ => char::from_u32(r - 0x20).unwrap(),
            },
            _ => c,
        };
    }
    if let Ok(i) = CASE_ORBIT.binary_search_by_key(&r, |&(from, _)| from) {
        return char::from_u32(CASE_ORBIT[i].1).unwrap();
    }
    // an orbit of the rune and its simple lower and upper cases alone, as Go's `unicode.CaseRanges`
    // gives them
    let mut lower = c.to_lowercase();
    if let (Some(l), None) = (lower.next(), lower.next())
        && l != c
    {
        return l;
    }
    let mut upper = c.to_uppercase();
    if let (Some(u), None) = (upper.next(), upper.next()) { u } else { c }
}

/// The orbits of the simple case folding not reached through the lower and the upper case alone
/// (Go's `unicode.caseOrbit`, ordered by `from`).
const CASE_ORBIT: &[(u32, u32)] = &[
    (0x004B, 0x006B), (0x0053, 0x0073), (0x006B, 0x212A), (0x0073, 0x017F), (0x00B5, 0x039C), (0x00C5, 0x00E5), (0x00DF, 0x1E9E), (0x00E5, 0x212B),
    (0x0130, 0x0130), (0x0131, 0x0131), (0x017F, 0x0053), (0x01C4, 0x01C5), (0x01C5, 0x01C6), (0x01C6, 0x01C4), (0x01C7, 0x01C8), (0x01C8, 0x01C9),
    (0x01C9, 0x01C7), (0x01CA, 0x01CB), (0x01CB, 0x01CC), (0x01CC, 0x01CA), (0x01F1, 0x01F2), (0x01F2, 0x01F3), (0x01F3, 0x01F1), (0x0345, 0x0399),
    (0x0392, 0x03B2), (0x0395, 0x03B5), (0x0398, 0x03B8), (0x0399, 0x03B9), (0x039A, 0x03BA), (0x039C, 0x03BC), (0x03A0, 0x03C0), (0x03A1, 0x03C1),
    (0x03A3, 0x03C2), (0x03A6, 0x03C6), (0x03A9, 0x03C9), (0x03B2, 0x03D0), (0x03B5, 0x03F5), (0x03B8, 0x03D1), (0x03B9, 0x1FBE), (0x03BA, 0x03F0),
    (0x03BC, 0x00B5), (0x03C0, 0x03D6), (0x03C1, 0x03F1), (0x03C2, 0x03C3), (0x03C3, 0x03A3), (0x03C6, 0x03D5), (0x03C9, 0x2126), (0x03D0, 0x0392),
    (0x03D1, 0x03F4), (0x03D5, 0x03A6), (0x03D6, 0x03A0), (0x03F0, 0x039A), (0x03F1, 0x03A1), (0x03F4, 0x0398), (0x03F5, 0x0395), (0x0412, 0x0432),
    (0x0414, 0x0434), (0x041E, 0x043E), (0x0421, 0x0441), (0x0422, 0x0442), (0x042A, 0x044A), (0x0432, 0x1C80), (0x0434, 0x1C81), (0x043E, 0x1C82),
    (0x0441, 0x1C83), (0x0442, 0x1C84), (0x044A, 0x1C86), (0x0462, 0x0463), (0x0463, 0x1C87), (0x1C80, 0x0412), (0x1C81, 0x0414), (0x1C82, 0x041E),
    (0x1C83, 0x0421), (0x1C84, 0x1C85), (0x1C85, 0x0422), (0x1C86, 0x042A), (0x1C87, 0x0462), (0x1C88, 0xA64A), (0x1E60, 0x1E61), (0x1E61, 0x1E9B),
    (0x1E9B, 0x1E60), (0x1E9E, 0x00DF), (0x1FBE, 0x0345), (0x2126, 0x03A9), (0x212A, 0x004B), (0x212B, 0x00C5), (0xA64A, 0xA64B), (0xA64B, 0x1C88),
];

/// The data of the organization file `f` (at `path`, as given): read from `--org` when it is given,
/// else from the place its `data` line names; with `--at`, only the version in effect that day.
pub fn read(f: &File, path: &str, opts: &crate::check::Options) -> Read {
    let (written, dir) = match (&opts.org, &f.data) {
        (Some(o), _) => (o.clone(), data::fold(Path::new(o))),
        (None, Some((w, _))) => (w.clone(), data::beside(path, w)),
        (None, None) => {
            let none = NoData::missing(
                tr!("組織のファイルに `data` の行が無く、`--org` もありません", "the organization's file has no `data` line, and no `--org` is given"),
                with_the_data(tr!("組織のファイルに `data` の行を書くか、`--org` でデータのディレクトリを渡してください", "Write a `data` line in the organization's file, or give the data's directory with `--org`")),
            );
            return Read { org: None, diags: Vec::new(), none: Some(none) };
        }
    };
    let shown = dir.display().to_string();
    if !ritsu_base::fs::is_dir(&dir) {
        let none = NoData::missing(
            tr!("`{written}`（`{shown}`）というディレクトリがありません", "there is no directory `{written}` (`{shown}`)"),
            with_the_data(tr!("データを `{shown}` に置くか、`data` の行（`--org`）を直してください", "Put the data in `{shown}`, or correct the `data` line (`--org`)")),
        );
        return Read { org: None, diags: Vec::new(), none: Some(none) };
    }
    let positions: Vec<(String, String)> = f.positions.iter().map(|p| (p.name.text.clone(), p.name.ascii().to_string())).collect();
    let mut org = Org { written: written.clone(), dir: shown.clone(), positions, at: opts.at, ..Org::default() };
    let mut diags = Vec::new();
    // the versions, by the names of the directories
    let mut found: Vec<(i64, std::path::PathBuf)> = Vec::new();
    let entries = match ritsu_base::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            let e = e.to_string();
            return Read { org: None, diags: vec![Diag::error("E501", &shown, 1, 1, tr!("`{shown}` を読めません: {e}", "Cannot read `{shown}`: {e}"))], none: None };
        }
    };
    let mut proxies_path = None;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let p = e.path();
        if ritsu_base::fs::is_dir(&p) {
            match ritsu_ports::day_of(&name) {
                Some(d) => found.push((d, p)),
                None if name.starts_with('.') => {}
                None => {
                    let ps = data::fold(&p).display().to_string();
                    diags.push(Diag::error("E501", &ps, 1, 1, tr!("ディレクトリ `{name}` は版の名前ではありません", "The directory `{name}` is not named as a version is")).note(tr!(
                        "版のディレクトリの名前は、その版が効き始める日（`2026-04-01`）にしてください。",
                        "Name a version's directory after the day it takes effect (`2026-04-01`)."
                    )));
                }
            }
        } else if name == "proxies.csv" {
            proxies_path = Some(p);
        }
    }
    found.sort();
    org.days = found.iter().map(|(d, _)| *d).collect();
    if found.is_empty() && diags.is_empty() {
        let none = NoData::missing(
            tr!("`{written}`（`{shown}`）に版のディレクトリがありません", "`{written}` (`{shown}`) holds no version's directory"),
            with_the_data(tr!(
                "`{shown}` に、効き始める日の名前（`2026-04-01`）で版のディレクトリを置いてください",
                "Put a version's directory in `{shown}`, named after the day it takes effect (`2026-04-01`)"
            )),
        );
        return Read { org: None, diags, none: Some(none) };
    }
    let chosen: Vec<&(i64, std::path::PathBuf)> = match opts.at {
        Some(at) => found.iter().rev().find(|(d, _)| *d <= at).into_iter().collect(),
        None => found.iter().collect(),
    };
    if chosen.is_empty() && diags.is_empty() {
        let at = crate::lex::date_text(opts.at.unwrap_or_default());
        let first = found.first().map(|(d, _)| crate::lex::date_text(*d)).unwrap_or_default();
        let none = NoData {
            what: tr!("{at}（`--at`）に効いている組織の版が無い", "no version of the organization is in effect on {at}, the day `--at` gives"),
            why: tr!("最初の版は {first} から効きます", "the first version takes effect on {first}"),
            fix: tr!("`--at` に {first} 以後の日を渡すか、{at} までに効く版のディレクトリを足してください。", "Give `--at` a day on or after {first}, or add a version's directory that takes effect by {at}."),
            line: tr!(
                "{at}（`--at`）に効いている版がありません。最初の版は {first} から効きます",
                "no version is in effect on {at}, the day `--at` gives; the first takes effect on {first}"
            ),
        };
        return Read { org: None, diags, none: Some(none) };
    }
    for (day, p) in chosen {
        if let Some(v) = version(f, &org, *day, p, &mut diags) {
            org.versions.push(v);
        }
    }
    if let Some(p) = proxies_path {
        proxies(&mut org, &p, &mut diags);
    }
    if crate::diag::has_errors(&diags) {
        return Read { org: None, diags, none: None };
    }
    Read { org: Some(org), diags, none: None }
}

/// One version's directory, read and checked (DESIGN 8.4): None when it does not read (said).
fn version(f: &File, org: &Org, day: i64, dir: &Path, diags: &mut Vec<Diag>) -> Option<Version> {
    let shown = data::fold(dir).display().to_string();
    let (teams, people, members) = (dir.join("teams.csv"), dir.join("people.csv"), dir.join("members.csv"));
    let json = dir.join(JSON_FILE);
    let csvs = [&teams, &people, &members].iter().filter(|p| ritsu_base::fs::is_file(p)).count();
    let has_json = ritsu_base::fs::is_file(&json);
    let before = diags.len();
    let tables: Vec<Table> = if has_json && csvs > 0 {
        diags.push(Diag::error("E501", &data::fold(&json).display().to_string(), 1, 1, tr!(
            "版 `{shown}` に、`{JSON_FILE}` と CSV のファイルの両方があります",
            "The version `{shown}` holds both `{JSON_FILE}` and CSV files"
        )).note(tr!("どちらか一方にしてください。", "Keep one of them.")));
        return None;
    } else if has_json {
        let jp = data::fold(&json);
        match data::read_json(&jp, &[("teams", &["id", "name", "name_ja", "kind", "parent", "head"]), ("people", &["id", "name", "name_ja", "email", LANGUAGE]), ("members", &MEMBERS)], &["primary"]) {
            Ok(t) => t,
            Err(us) => {
                let file = jp.display().to_string();
                for u in us {
                    diags.push(unread(&file, None, u));
                }
                return None;
            }
        }
    } else {
        let mut out = Vec::new();
        for p in [&teams, &people, &members] {
            let fp = data::fold(p);
            if !ritsu_base::fs::is_file(&fp) {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                diags.push(Diag::error("E501", &shown, 1, 1, tr!("版 `{shown}` に `{name}` がありません", "The version `{shown}` has no `{name}`")).note(tr!(
                    "版のディレクトリには、`teams.csv`、`people.csv`、`members.csv`（または同じ形の `{JSON_FILE}` 一つ）を置いてください。",
                    "A version's directory holds `teams.csv`, `people.csv` and `members.csv` (or one `{JSON_FILE}` of the same shape)."
                )));
                continue;
            }
            match data::read_csv(&fp) {
                Ok(t) => out.push(t),
                Err(u) => {
                    let file = fp.display().to_string();
                    let src = ritsu_base::fs::read_to_string(&fp).ok();
                    diags.push(unread(&file, src.as_deref(), u));
                }
            }
        }
        if out.len() < 3 {
            return None;
        }
        out
    };
    let [tt, pt, mt] = [&tables[0], &tables[1], &tables[2]];
    for (t, needed, known) in [(tt, &TEAMS[..], &["name_ja"][..]), (pt, &PEOPLE[..], &["name_ja", "email", LANGUAGE][..]), (mt, &MEMBERS[..], &[][..])] {
        for u in data::columns(t, needed, known) {
            diags.push(e501(t, u.line, u.message));
        }
    }
    if diags.len() > before {
        return None;
    }
    let mut v = Version { from: day, dir: shown, teams_file: tt.file.clone(), people_file: pt.file.clone(), members_file: mt.file.clone(), ..Version::default() };
    let kinds: Vec<&Titled> = f.team_kinds.iter().collect();
    // a name as written, with its ASCII name when that is another (`部(department)`): the data may
    // write either
    let names = |all: Vec<(String, String)>| {
        let items: Vec<String> = all.iter().map(|(w, a)| if w == a { w.clone() } else { format!("{w}({a})") }).collect();
        Text::new(items.join("、"), items.join(", "))
    };
    let kind_names = || names(kinds.iter().map(|k| (k.name.text.clone(), k.name.ascii().to_string())).collect());
    let pos_names = || names(org.positions.clone());
    // people
    // the addresses, as two are compared where a person signs in by one (Go's
    // `strings.EqualFold`): two people of a version may not share one
    let mut addresses: HashMap<String, usize> = HashMap::new();
    for r in &pt.rows {
        let id = pt.get(r, "id").to_string();
        if id.is_empty() {
            diags.push(e501(pt, r.line, tr!("人の `id` が空です", "A person's `id` is empty")));
            continue;
        }
        check_id(pt, r.line, "person", "人", &id, diags);
        if let Some(o) = v.people.iter().find(|p| p.id == id) {
            let l = o.line;
            diags.push(e501(pt, r.line, tr!("人 `{id}` が二度あります（{l} 行目にも）", "The person `{id}` is there twice (line {l} too)")));
            continue;
        }
        let name = pt.get(r, "name").to_string();
        if name.is_empty() {
            diags.push(e501(pt, r.line, tr!("人 `{id}` の `name` が空です", "The `name` of the person `{id}` is empty")));
        }
        check_name(pt, r.line, ("person", "人"), &id, "name", &name, diags);
        let name_ja = pt.get(r, "name_ja").to_string();
        check_name(pt, r.line, ("person", "人"), &id, "name_ja", &name_ja, diags);
        let language = pt.get(r, LANGUAGE).to_string();
        if !language.is_empty() && !crate::screens::LANGS.contains(&language.as_str()) {
            let all = crate::screens::LANGS.map(|l| format!("`{l}`"));
            let (en, ja) = (all.join(" or "), all.join(" か "));
            diags.push(e501(pt, r.line, tr!("人 `{id}` の `{LANGUAGE}` は {ja} で書きます。`{language}` ではありません", "The `{LANGUAGE}` of the person `{id}` is {en}, not `{language}`")).note(tr!(
                "その人が知らせと画面を読む言語です。空なら、申請の種類ごとに、その種類の言語で読みます。",
                "It is the language the person reads notices and screens in; left empty, each kind's own language is read."
            )));
        }
        let email = pt.get(r, "email").to_string();
        if check_address(pt, r.line, &id, &email, diags) && !email.trim().is_empty() {
            let key = folded(email.trim());
            if let Some(&j) = addresses.get(&key) {
                let (o, l) = (v.people[j].id.clone(), v.people[j].line);
                let addr = escaped(email.trim());
                diags.push(e501(pt, r.line, tr!("人 `{id}` の `email`（`{addr}`）が、人 `{o}` のアドレスと同じです（{l} 行目にも）。アドレスでサインインするので、二人で同じアドレスは使えません", "The `email` of the person `{id}` (`{addr}`) is the address of the person `{o}` (line {l} too): people sign in by their addresses, and two may not share one")));
            } else {
                addresses.insert(key, v.people.len());
            }
        }
        v.people.push(Person { id, name, name_ja, email, language, line: r.line });
    }
    // teams
    for r in &tt.rows {
        let id = tt.get(r, "id").to_string();
        if id.is_empty() {
            diags.push(e501(tt, r.line, tr!("チームの `id` が空です", "A team's `id` is empty")));
            continue;
        }
        check_id(tt, r.line, "team", "チーム", &id, diags);
        if let Some(o) = v.teams.iter().find(|t| t.id == id) {
            let l = o.line;
            diags.push(e501(tt, r.line, tr!("チーム `{id}` が二度あります（{l} 行目にも）", "The team `{id}` is there twice (line {l} too)")));
            continue;
        }
        check_name(tt, r.line, ("team", "チーム"), &id, "name", tt.get(r, "name"), diags);
        check_name(tt, r.line, ("team", "チーム"), &id, "name_ja", tt.get(r, "name_ja"), diags);
        let kind_w = tt.get(r, "kind");
        let kind = match kinds.iter().find(|k| k.name.text == kind_w || k.name.ascii() == kind_w) {
            Some(k) => k.name.text.clone(),
            None => {
                let ks = kind_names();
                diags.push(e501(tt, r.line, tr!("チーム `{id}` の種類 `{kind_w}` は、組織のファイルが宣言するチームの種類ではありません", "The kind `{kind_w}` of the team `{id}` is not a kind of team the organization's file declares"))
                    .note(tr!("宣言したチームの種類: {}", "The kinds of team declared: {}", ks.ja; ks.en)));
                String::new()
            }
        };
        let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
        v.teams.push(Team { id, name: tt.get(r, "name").to_string(), name_ja: tt.get(r, "name_ja").to_string(), kind, parent: opt(tt.get(r, "parent")), head: opt(tt.get(r, "head")), line: r.line });
    }
    // members
    for r in &mt.rows {
        let (person, team, pos_w, primary_w) = (mt.get(r, "person"), mt.get(r, "team"), mt.get(r, "position"), mt.get(r, "primary"));
        let mut ok = true;
        if !v.people.iter().any(|p| p.id == person) {
            let pf = &pt.file;
            diags.push(e501(mt, r.line, tr!("`{person}` は、この版の人（`{pf}`）にいません", "`{person}` is not a person of this version (`{pf}`)")));
            ok = false;
        }
        if !v.teams.iter().any(|t| t.id == team) {
            let tf = &tt.file;
            diags.push(e501(mt, r.line, tr!("`{team}` は、この版のチーム（`{tf}`）にありません", "`{team}` is not a team of this version (`{tf}`)")));
            ok = false;
        }
        let position = org.rank(pos_w);
        if position.is_none() {
            let ps = pos_names();
            diags.push(e501(mt, r.line, tr!("役職 `{pos_w}` は、組織のファイルが宣言する役職ではありません", "The position `{pos_w}` is not a position the organization's file declares"))
                .note(tr!("宣言した役職（低い順）: {}", "The positions declared, lowest first: {}", ps.ja; ps.en)));
            ok = false;
        }
        let primary = match primary_w {
            "yes" => true,
            "no" | "" => false,
            other => {
                diags.push(e501(mt, r.line, tr!("`primary` は `yes` か `no` で書きます。`{other}` ではありません", "`primary` is `yes` or `no`, not `{other}`")));
                ok = false;
                false
            }
        };
        if let Some(o) = v.members.iter().find(|m| m.person == person && m.team == team) {
            let l = o.line;
            diags.push(e501(mt, r.line, tr!("`{person}` の `{team}` の行が二つあります（{l} 行目にも）", "There are two rows of `{person}` in `{team}` (line {l} too)")));
            ok = false;
        }
        if ok {
            v.members.push(Member { person: person.to_string(), team: team.to_string(), position: position.unwrap_or(0), primary, line: r.line });
        }
    }
    v.index();
    if diags.len() > before {
        return None;
    }
    // a primary team for each person
    for p in &v.people {
        let prim: Vec<&Member> = v.memberships(&p.id).filter(|m| m.primary).collect();
        let id = &p.id;
        let mf = &v.members_file;
        match prim.len() {
            1 => {}
            0 => diags.push(e501(pt, p.line, tr!("人 `{id}` に、主の所属がありません", "The person `{id}` has no primary team")).note(tr!(
                "`{mf}` の `{id}` の行の一つの `primary` を `yes` にしてください（人は、主の所属のチームから上長と部長をたどります）。",
                "Mark one of the rows of `{id}` in `{mf}` `yes` in `primary`: a requester's manager and heads are found from their primary team."
            ))),
            n => {
                let lines = Text::new(
                    prim.iter().map(|m| format!("{}（{} 行目）", m.team, m.line)).collect::<Vec<_>>().join("、"),
                    prim.iter().map(|m| format!("{} (line {})", m.team, m.line)).collect::<Vec<_>>().join(", "),
                );
                let first = prim[1].line;
                diags.push(e501(mt, first, tr!("人 `{id}` に、主の所属が {n} 個あります: {}", "The person `{id}` has {n} primary teams: {}", lines.ja; lines.en)).note(tr!(
                    "主の所属は一人に一つです。ほかの行の `primary` を `no` にしてください（兼務になります）。",
                    "A person has one primary team: make `primary` `no` on the other rows (they are then also members of those teams)."
                )));
            }
        }
    }
    // parents, heads, rings and depth
    for t in &v.teams {
        let id = &t.id;
        if let Some(p) = &t.parent
            && v.team(p).is_none()
        {
            diags.push(e501(tt, t.line, tr!("チーム `{id}` の親 `{p}` は、この版のチームにありません", "The parent `{p}` of the team `{id}` is not a team of this version")));
        }
        if let Some(h) = &t.head {
            if v.person(h).is_none() {
                let pf = &v.people_file;
                diags.push(e501(tt, t.line, tr!("チーム `{id}` の長 `{h}` は、この版の人（`{pf}`）にいません", "The head `{h}` of the team `{id}` is not a person of this version (`{pf}`)")));
            } else if v.position_in(h, id).is_none() {
                let mf = &v.members_file;
                diags.push(e501(tt, t.line, tr!("チーム `{id}` の長 `{h}` は、そのチームの人ではありません", "The head `{h}` of the team `{id}` is not a member of it")).note(tr!(
                    "`{mf}` に `{h},{id},<役職>,no` の行を足すか、長を直してください。",
                    "Add the row `{h},{id},<position>,no` to `{mf}`, or correct the head."
                )));
            }
        }
    }
    if diags.len() > before {
        return None;
    }
    // a ring of parents, said once, at the team of the ring that comes first in the file; a team
    // deeper than DEPTH, said once
    let mut rings: Vec<Vec<String>> = Vec::new();
    let mut deep = false;
    for t in &v.teams {
        let mut chain: Vec<&str> = vec![&t.id];
        let mut at = t.parent.as_deref();
        while let Some(p) = at {
            if let Some(i) = chain.iter().position(|c| *c == p) {
                let mut key: Vec<String> = chain[i..].iter().map(|s| s.to_string()).collect();
                key.sort();
                if !rings.contains(&key) {
                    rings.push(key);
                    let first = chain[i..].iter().filter_map(|id| v.team(id)).min_by_key(|x| x.line).unwrap_or(t);
                    let mut around = vec![first.id.as_str()];
                    let mut next = first.parent.as_deref();
                    while let Some(n) = next {
                        around.push(n);
                        if n == first.id || around.len() > DEPTH + 1 {
                            break;
                        }
                        next = v.team(n).and_then(|x| x.parent.as_deref());
                    }
                    let ring = around.join(" → ");
                    let id = &first.id;
                    diags.push(e501(tt, first.line, tr!("チーム `{id}` の親をたどると、`{id}` に戻ります（{ring}）", "The parents of the team `{id}` lead back to it ({ring})")).note(tr!(
                        "どれかのチームの `parent` を直して、輪を切ってください。",
                        "Correct the `parent` of one of these teams to break the ring."
                    )));
                }
                break;
            }
            chain.push(p);
            if chain.len() > DEPTH {
                if !deep {
                    deep = true;
                    let id = &t.id;
                    diags.push(e501(tt, t.line, tr!("チーム `{id}` の上に、チームが {DEPTH} 段以上あります", "The team `{id}` has {DEPTH} or more teams above it")).note(tr!(
                        "上へたどるのは {DEPTH} 段までです。チームの親を直してください。",
                        "Teams are followed up {DEPTH} levels at most: correct the parents of the teams."
                    )));
                }
                break;
            }
            at = v.team(p).and_then(|x| x.parent.as_deref());
        }
    }
    if diags.len() > before {
        return None;
    }
    Some(v)
}

/// `proxies.csv`, read and checked (DESIGN 5.8): E501 for a row that does not read, E505 for a
/// person or a proxy not in the organization on some day of the row.
fn proxies(org: &mut Org, p: &Path, diags: &mut Vec<Diag>) {
    let fp = data::fold(p);
    let file = fp.display().to_string();
    org.proxies_file = Some(file.clone());
    let t = match data::read_csv(&fp) {
        Ok(t) => t,
        Err(u) => {
            let src = ritsu_base::fs::read_to_string(&fp).ok();
            diags.push(unread(&file, src.as_deref(), u));
            return;
        }
    };
    let cols = data::columns(&t, &PROXIES, &["requests"]);
    if !cols.is_empty() {
        for u in cols {
            diags.push(e501(&t, u.line, u.message));
        }
        return;
    }
    for r in &t.rows {
        if let Some(x) = proxy(&t, r, diags) {
            org.proxies.push(x);
        }
    }
    // a person or a proxy who is not in a version on some day of the row
    for x in &org.proxies {
        for v in &org.versions {
            let (a, b) = org.period(v);
            if !x.meets(a, b) {
                continue;
            }
            for (id, stands_in) in [(&x.person, false), (&x.proxy, true)] {
                if v.person(id).is_some() {
                    continue;
                }
                let day = v.day();
                let days = x.days();
                let head = if stands_in {
                    tr!("代理の行の代理人 `{id}` は、{day} からの組織にいません", "`{id}`, the proxy of this row, is not in the organization from {day}")
                } else {
                    tr!("代理の行の本来の承認者 `{id}` は、{day} からの組織にいません", "`{id}`, whom this row's proxy stands in for, is not in the organization from {day}")
                };
                // ending the row the day before the version keeps its days before it; a row that
                // starts in the version has none to keep
                let fix = if x.from < v.from {
                    let before = crate::lex::date_text(v.from - 1);
                    tr!(
                        "人が辞めたなら、この行の `to` を、{day} からの組織の前の日の {before} にしてください。ID の誤りなら、ID を直してください。",
                        "If the person has left, make this row's `to` {before}, the day before the organization from {day}; if the ID is wrong, correct it."
                    )
                } else {
                    tr!("人が辞めたなら、この行を消してください。ID の誤りなら、ID を直してください。", "If the person has left, take this row out; if the ID is wrong, correct it.")
                };
                diags.push(
                    Diag::error("E505", &t.file, x.line, 1, head)
                        .source(&t.src)
                        .note(tr!(
                            "この代理は {} に効き、{day} からの組織が効く日と重なります。",
                            "The proxy is in effect {}, which overlaps the days of the organization from {day}.",
                            days.ja; days.en
                        ))
                        .note(fix),
                );
            }
        }
    }
}

fn proxy(t: &Table, r: &Row, diags: &mut Vec<Diag>) -> Option<Proxy> {
    let (person, px, kind_w, from_w, to_w) = (t.get(r, "person"), t.get(r, "proxy"), t.get(r, "kind"), t.get(r, "from"), t.get(r, "to"));
    let before = diags.len();
    if person.is_empty() || px.is_empty() {
        diags.push(e501(t, r.line, tr!("代理の行には、`person` と `proxy` の両方が要ります", "A proxy's row needs both `person` and `proxy`")));
    } else if person == px {
        diags.push(e501(t, r.line, tr!("`{person}` が自分の代理になっています", "`{person}` is their own proxy")));
    }
    let kind = match kind_w {
        "approve" => Some(ProxyKind::Approve),
        "submit" => Some(ProxyKind::Submit),
        other => {
            diags.push(e501(t, r.line, tr!("代理の `kind` は `approve` か `submit` です。`{other}` ではありません", "A proxy's `kind` is `approve` or `submit`, not `{other}`")));
            None
        }
    };
    let from = ritsu_ports::day_of(from_w);
    if from.is_none() {
        diags.push(e501(t, r.line, tr!("代理の `from` は `2026-12-28` の形の日付です。`{from_w}` ではありません", "A proxy's `from` is a date such as `2026-12-28`, not `{from_w}`")));
    }
    let to = if to_w.is_empty() {
        None
    } else {
        let d = ritsu_ports::day_of(to_w);
        if d.is_none() {
            diags.push(e501(t, r.line, tr!("代理の `to` は `2027-01-08` の形の日付か、空（終わり無し）です。`{to_w}` ではありません", "A proxy's `to` is a date such as `2027-01-08`, or empty for no end; not `{to_w}`")));
        }
        d
    };
    if let (Some(a), Some(b)) = (from, to)
        && b < a
    {
        diags.push(e501(t, r.line, tr!("代理の `to`（{to_w}）が `from`（{from_w}）より前です", "The proxy's `to` ({to_w}) is before its `from` ({from_w})")));
    }
    if diags.len() > before {
        return None;
    }
    let requests = t.get(r, "requests").split_whitespace().map(str::to_string).collect();
    Some(Proxy { person: person.to_string(), proxy: px.to_string(), kind: kind?, from: from?, to, requests, line: r.line })
}
