//! Finding a value among the people, the teams or the rows of a master by words (DESIGN 13.2): what
//! a picker of a screen asks `/find` for, and an agent the operations `find_people`, `find_teams`
//! and `find_master_<file>_<master>` of the API (DESIGN 12.4). The words and the values are compared
//! folded ([`fold`]), and the values found come the closest first ([`find`]). The server finds; this
//! is the reference it is held to, through the vectors `--target requests` writes to
//! `organization/find.jsonl` ([`vectors`]).

use ritsu_base::json::Json;

/// How many values a find answers when `limit` says nothing, the most it may be asked for, and the
/// most characters (code points) its words may have.
pub const SHOWN: usize = 20;
pub const MOST: usize = 100;
pub const LONGEST: usize = 200;

/// A value to find: its ID (a person, a team) or its key (a row of a master), its name in the
/// language the find is asked in, and its name in the other ("" for none: a row of a master has one
/// name, the one its `name_field` holds).
#[derive(Clone, Copy)]
pub struct Choice<'a> {
    pub value: &'a str,
    pub name: &'a str,
    pub other: &'a str,
}

/// A text as a find compares it: in NFKC (full-width letters and digits as ASCII, half-width
/// katakana as full-width with their voiced marks joined, `①` as `1`, `㈱` as `(株)`), without
/// the characters of Unicode's `White_Space`, the hiragana U+3041–U+3096 as the katakana U+30A1–
/// U+30F6, and each character as its simple lowercase mapping (UnicodeData.txt: one character to
/// one, with no context and no language's own rules), the final sigma `ς` as `σ`.
pub fn fold(s: &str) -> String {
    let n = ritsu_emit::normal::nfkc(s);
    let mut out = String::with_capacity(n.len());
    for c in n.chars() {
        if c.is_whitespace() {
            continue;
        }
        let c = match c as u32 {
            0x3041..=0x3096 => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        };
        out.push(lower(c));
    }
    out
}

/// The simple lowercase mapping of `c`, `ς` as `σ`. Rust's `char::to_lowercase` is the full
/// mapping, which is one character for every character but `İ` (U+0130), two (`i` and U+0307);
/// its simple mapping is `i`.
fn lower(c: char) -> char {
    match c {
        '\u{130}' => 'i',
        'ς' => 'σ',
        _ => {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        }
    }
}

/// How close a value is to the query: 0 for one whose value is the query as it is typed (without
/// the white space at its ends), 1 for one whose value folds to the query folded, 2 for one with a
/// name (in either language) that folds to it, 3 for one whose value or a name begins, folded, with
/// the first word, 4 for the rest.
fn closeness(c: &Choice, typed: &str, whole: &str, words: &[String], texts: &[String; 3]) -> u8 {
    if c.value == typed {
        0
    } else if texts[0] == whole {
        1
    } else if texts[1..].iter().any(|t| !t.is_empty() && t == whole) {
        2
    } else if words.first().is_some_and(|w| texts.iter().any(|t| !t.is_empty() && t.starts_with(w.as_str()))) {
        3
    } else {
        4
    }
}

/// The values of `choices` the words of `q` find, the closest first, at most `limit` of them, and
/// whether more are found. The words are `q` cut at the characters of `White_Space`, each folded (a
/// word that folds to nothing is none); a value is found when each word is in its value or one of
/// its names, folded. No words find every value. Values as close come in the order of their values,
/// compared code point by code point.
pub fn find<'a>(choices: &[Choice<'a>], q: &str, limit: usize) -> (Vec<Choice<'a>>, bool) {
    let words: Vec<String> = q.split_whitespace().map(fold).filter(|w| !w.is_empty()).collect();
    let whole = fold(q);
    let typed = q.trim_matches(char::is_whitespace);
    let mut found: Vec<(u8, Choice<'a>)> = Vec::new();
    for c in choices {
        let texts = [fold(c.value), fold(c.name), fold(c.other)];
        if words.iter().all(|w| texts.iter().any(|t| !t.is_empty() && t.contains(w.as_str()))) {
            found.push((closeness(c, typed, &whole, &words, &texts), *c));
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.value.cmp(b.1.value)));
    let more = found.len() > limit;
    (found.into_iter().take(limit).map(|(_, c)| c).collect(), more)
}

/// A value found, as a find answers it: its value, its name in the language (empty for none), and
/// the label a list shows: the name and the value (`Sales staff 3 (e1203)`, `営業担当 3（e1203）`),
/// the value alone where the name is empty or the value itself.
pub fn found_json(c: &Choice, lang: &str) -> Json {
    let label = if c.name.is_empty() || c.name == c.value {
        c.value.to_string()
    } else if lang == "ja" {
        format!("{}（{}）", c.name, c.value)
    } else {
        format!("{} ({})", c.name, c.value)
    };
    Json::obj([("value", Json::str(c.value)), ("name", Json::str(c.name)), ("label", Json::str(label))])
}

/// What a find is asked: the words (`q`), how many it answers (`limit`, none for [`SHOWN`]), the
/// language of the names (`lang`), and, of the rows of a master, the day they are chosen on (`on`,
/// `YYYY-MM-DD`; none for today, and for the people and the teams).
pub struct Asked<'q> {
    pub q: &'q str,
    pub limit: Option<i64>,
    pub lang: &'q str,
    pub on: Option<&'q str>,
}

/// The answer to a find: `{"found": […], "more": …}`, or the first parameter, in the order `q`,
/// `limit`, `lang`, `on`, the server answers 400 for (`{"bad": "q"}`): words of more than
/// [`LONGEST`] characters, a `limit` outside 1–[`MOST`], a `lang` other than `en` and `ja`, an `on`
/// that is no day written `YYYY-MM-DD`. The choices are the ones that may be chosen (of a master,
/// the rows of the day `on`).
pub fn answer(choices: &[Choice], asked: &Asked) -> Json {
    if asked.q.chars().count() > LONGEST {
        return Json::obj([("bad", Json::str("q"))]);
    }
    let limit = match asked.limit {
        None => SHOWN,
        Some(n) if (1..=MOST as i64).contains(&n) => n as usize,
        Some(_) => return Json::obj([("bad", Json::str("limit"))]),
    };
    if !matches!(asked.lang, "en" | "ja") {
        return Json::obj([("bad", Json::str("lang"))]);
    }
    if asked.on.is_some_and(|d| ritsu_ports::day_of(d).is_none()) {
        return Json::obj([("bad", Json::str("on"))]);
    }
    let (found, more) = find(choices, asked.q, limit);
    Json::obj([("found", Json::arr(found.iter().map(|c| found_json(c, asked.lang)))), ("more", Json::Bool(more))])
}

/// A person or a team of the vectors: its ID, its name and its Japanese name (empty for none).
type Named = (&'static str, &'static str, &'static str);

/// The people of the vectors (fictional): IDs, names in English and in Japanese, among them names in
/// the forms the folding reads alike.
const PEOPLE: [Named; 14] = [
    ("e1101", "Head of Sales", "営業部長"),
    ("e1201", "Sales staff 1", "営業担当 1"),
    ("e1202", "Sales staff 2", "営業担当 2"),
    ("e1203", "Sales staff 3", "営業担当 3"),
    ("e2001", "Sato Ichiro", "佐藤 一郎"),
    ("e2002", "Saito Hanako", "斎藤　花子"),
    ("e2003", "Misaki Ren", "みさき 蓮"),
    ("e2004", "Kato Jiro", "ｶﾄｳ ｼﾞﾛｳ"),
    ("e2005", "ＫＩＭＵＲＡ Ｙｕｉ", "木村 結衣"),
    ("e4001", "ΑΛΕΞΗΣ Papas", ""),
    ("e4002", "İlker Demir", ""),
    ("e4003", "Anna GROẞ", ""),
    ("e4004", "Jonas Straße", ""),
    ("e9001", "", ""),
];

/// The teams of the vectors (fictional).
const TEAMS: [Named; 5] = [
    ("t-abc", "ABC Promotion Office", "ＡＢＣ推進室"),
    ("t-gas", "Gas Section", "ｶﾞｽ課"),
    ("t-midori", "Midori Co. liaison", "㈱ミドリ窓口"),
    ("t-sales1", "Sales Section 1", "第①営業課"),
    ("t-sales2", "Sales Section 2", "第②営業課"),
];

/// A row of a master of the vectors (fictional): its key, the name its `name_field` holds, and the
/// first and the last day it may be chosen on (`valid_from`, `valid_to`; "" for none).
type Row = (&'static str, &'static str, &'static str, &'static str);

/// The rows of a master of the vectors, each of them chosen on every day.
const ROWS: [Row; 4] = [("V-001", "Kumo Trading", "", ""), ("V-002", "ﾐﾄﾞﾘ商会", "", ""), ("V-010", "Sora Supplies", "", ""), ("V-011", "", "", "")];

/// Rows held in an order other than the order of their keys, which is another order in UTF-16
/// units: `ｚ` (U+FF5A) comes before `𠮷` (U+20BB7) by code points, and after it by UTF-16 units,
/// as a surrogate (U+D842) is less than U+FF5A.
const ORDERED: [Row; 4] = [("𠮷-01", "𠮷田商店", "", ""), ("ｚ-01", "Zumi Supplies", "", ""), ("あ-01", "あおば商会", "", ""), ("z-01", "Zelk Parts", "", "")];

/// Rows that may be chosen on some days: one to the end of June 2026, one from July, one with no
/// ends.
const DATED: [Row; 3] = [("A-100", "Travel", "", "2026-06-30"), ("A-101", "Travel and lodging", "2026-07-01", ""), ("A-102", "Meals", "", "")];

/// People whose name is their ID: e5001 in either language, e5002 in Japanese alone.
const SAME: [Named; 2] = [("e5001", "e5001", ""), ("e5002", "Ito Mei", "e5002")];

/// What one names a value by on a ranking of the vectors: the values the ranks of [`closeness`] tell
/// apart, each with a name, against the order of the values.
const RANKED: [Named; 6] = [("a0", "my x9", ""), ("a1", "x9", ""), ("a2", "x9 team", ""), ("a3", "the x9", ""), ("X9", "Other", ""), ("x9", "Another", "")];

/// The choices of people or teams, in the language `lang`: the name in it (the Japanese name in
/// Japanese where there is one), and the other.
fn named_in(values: &[Named], lang: &str) -> Vec<(String, String, String)> {
    values
        .iter()
        .map(|(id, en, ja)| {
            let in_ja = if ja.is_empty() { en.to_string() } else { ja.to_string() };
            if lang == "ja" { (id.to_string(), in_ja, en.to_string()) } else { (id.to_string(), en.to_string(), in_ja) }
        })
        .collect()
}

/// One line of the vectors: what is found among (`among`: `people`, `teams` or `master`), the
/// values as the data holds them, what the find is asked, and the reference's answer.
fn line(case: &str, among: &str, values: &[Json], choices: &[(String, String, String)], asked: &Asked) -> Json {
    let cs: Vec<Choice> = choices.iter().map(|(v, n, o)| Choice { value: v, name: n, other: o }).collect();
    Json::obj([
        ("case", Json::str(case)),
        ("among", Json::str(among)),
        ("values", Json::arr(values.iter().cloned())),
        ("q", Json::str(asked.q)),
        ("limit", asked.limit.map_or(Json::Null, |n| Json::int(i128::from(n)))),
        ("lang", Json::str(asked.lang)),
        ("on", asked.on.map_or(Json::Null, Json::str)),
        ("answer", answer(&cs, asked)),
    ])
}

fn named_json(values: &[Named]) -> Vec<Json> {
    values.iter().map(|&(id, en, ja)| Json::obj([("value", Json::str(id)), ("name", Json::str(en)), ("name_ja", Json::str(ja))])).collect()
}

fn rows_json(rows: &[Row]) -> Vec<Json> {
    rows.iter().map(|&(k, n, from, to)| Json::obj([("value", Json::str(k)), ("name", Json::str(n)), ("valid_from", Json::str(from)), ("valid_to", Json::str(to))])).collect()
}

/// The rows a find chooses among on the day `on`, as choices: every row when no day is asked (the
/// rows the vectors ask so have no ends, and are chosen on any day), none for a day that is no date
/// (the find is refused).
fn rows_on(rows: &[Row], on: Option<&str>) -> Vec<(String, String, String)> {
    let day = on.map(ritsu_ports::day_of);
    let holds = |from: &str, to: &str, d: ritsu_ports::Day| (from.is_empty() || ritsu_ports::day_of(from).is_some_and(|f| f <= d)) && (to.is_empty() || ritsu_ports::day_of(to).is_some_and(|t| d <= t));
    rows.iter()
        .filter(|&&(_, _, from, to)| match day {
            None => true,
            Some(None) => false,
            Some(Some(d)) => holds(from, to, d),
        })
        .map(|&(k, n, _, _)| (k.to_string(), n.to_string(), String::new()))
        .collect()
}

/// The vectors of finding (`organization/find.jsonl`): one line for each case, each with the values
/// it finds among, so that a server's find is held to them alone. They show each step of the
/// folding, the ranks and their order (the order of the values against the order the data holds
/// them in), the labels, the limits of `q` and `limit`, the languages, the day the rows of a master
/// are chosen on, and which parameter is named where two are wrong.
pub fn vectors() -> String {
    let mut out = Vec::new();
    let people = named_json(&PEOPLE);
    let teams = named_json(&TEAMS);
    let rows = rows_json(&ROWS);
    let row_choices = rows_on(&ROWS, None);
    let many: Vec<(String, String, String)> = (1..=25).map(|i| (format!("p{i:02}"), format!("Person {i}"), format!("人 {i}"))).collect();
    let many_json: Vec<Json> = many.iter().map(|(v, n, j)| Json::obj([("value", Json::str(v)), ("name", Json::str(n)), ("name_ja", Json::str(j))])).collect();
    let ranked = named_json(&RANKED);
    // words of 200 characters and of 201, each a character past the Basic Multilingual Plane: the
    // limit counts code points, not bytes nor UTF-16 units
    let (long, longer) = ("𠮷".repeat(LONGEST), "𠮷".repeat(LONGEST + 1));
    let mut people_case = |case: &str, q: &str, lang: &str| out.push(line(case, "people", &people, &named_in(&PEOPLE, lang), &Asked { q, limit: None, lang, on: None }));
    people_case("no words: every value, in the order of the values", "", "en");
    people_case("only white space is no words", " \u{3000}\t", "en");
    // what the standard functions of some languages read otherwise: U+0085 is white space, and
    // U+FEFF and U+001C are none
    people_case("U+0085 is white space between the words", "sales\u{85}1201", "en");
    people_case("U+FEFF is no white space: one word, which no value holds", "sales\u{feff}1201", "en");
    people_case("U+001C is no white space: one word, which no value holds", "sales\u{1c}1201", "en");
    people_case("a word in a name", "sales", "en");
    people_case("every word is found: in the name and in the ID", "sales 1201", "en");
    people_case("words found in the name of the other language", "営業 3", "en");
    people_case("the names in Japanese", "営業", "ja");
    people_case("a Japanese name with no space, found with one", "佐藤一郎", "ja");
    people_case("an ideographic space between the words", "斎藤　花子", "ja");
    people_case("hiragana found as katakana", "かとう", "ja");
    people_case("hiragana in a name, found by katakana", "ミサキ", "en");
    people_case("half-width katakana and its voiced mark, found by full-width", "ジロウ", "ja");
    people_case("full-width letters, found in lower case", "kimura", "en");
    people_case("words in full-width letters", "ＳＡＴＯ", "en");
    people_case("a final sigma, found by a sigma in capitals", "αλεξης", "en");
    people_case("a capital I with a dot above, found by i", "ilker", "en");
    people_case("a capital sharp s, found by ß", "groß", "en");
    people_case("ß is not ss", "strasse", "en");
    people_case("an ID typed as it is comes first", "e1203", "en");
    people_case("an ID in capitals", "E1203", "en");
    people_case("no value holds the word", "nobody", "en");
    people_case("a person with no name: the label is the ID", "e9001", "ja");
    let mut team_case = |case: &str, q: &str, lang: &str| out.push(line(case, "teams", &teams, &named_in(&TEAMS, lang), &Asked { q, limit: None, lang, on: None }));
    team_case("full-width letters of a Japanese name", "abc推進", "en");
    team_case("a circled digit as a digit", "第1", "ja");
    team_case("a parenthesized ideograph", "(株)ミドリ", "ja");
    team_case("the ideograph alone", "株", "ja");
    team_case("half-width katakana found by hiragana", "がす", "ja");
    let mut row_case = |case: &str, q: &str, lang: &str| out.push(line(case, "master", &rows, &row_choices, &Asked { q, limit: None, lang, on: None }));
    row_case("the rows of a master: one name, in either language", "kumo", "ja");
    row_case("a key's part", "v-01", "en");
    row_case("half-width katakana in a row's name", "ミドリ", "en");
    row_case("a row with no name: the label is the key", "V-011", "en");
    let ordered = rows_json(&ORDERED);
    out.push(line("values as close in the order of their code points, not in the order of the data nor of UTF-16 units", "master", &ordered, &rows_on(&ORDERED, None), &Asked { q: "01", limit: None, lang: "en", on: None }));
    let dated = rows_json(&DATED);
    let mut day_case = |case: &str, lang: &str, on: &str| out.push(line(case, "master", &dated, &rows_on(&DATED, Some(on)), &Asked { q: "", limit: None, lang, on: Some(on) }));
    day_case("the rows of the day: a row on its last day", "en", "2026-06-30");
    day_case("the rows of the day: a row on its first day", "en", "2026-07-01");
    day_case("a day that is no date", "en", "2026-02-29");
    day_case("a day not written YYYY-MM-DD", "en", "2026-7-1");
    day_case("a language of neither and a day that is no date: the language is named", "fr", "2026-02-29");
    let same = named_json(&SAME);
    let mut same_case = |case: &str, lang: &str| out.push(line(case, "people", &same, &named_in(&SAME, lang), &Asked { q: "e500", limit: None, lang, on: None }));
    same_case("a name that is the ID: the label is the ID", "en");
    same_case("a Japanese name that is the ID: the label is the ID", "ja");
    let mut rank_case = |case: &str, q: &str| out.push(line(case, "people", &ranked, &named_in(&RANKED, "en"), &Asked { q, limit: None, lang: "en", on: None }));
    rank_case("the ranks: the ID as typed, the ID folded, a name, a name that begins with the word, the rest", "x9");
    rank_case("the ranks, the ID typed in capitals", "X9");
    rank_case("the ranks, the ID typed after U+0085, which is white space at an end", "\u{85}x9");
    let mut limit_case = |case: &str, q: &str, limit: Option<i64>, lang: &str| out.push(line(case, "people", &many_json, &many, &Asked { q, limit, lang, on: None }));
    limit_case("no limit: 20, and more", "", None, "en");
    limit_case("the least limit", "", Some(1), "en");
    limit_case("as many as there are: no more", "", Some(25), "en");
    limit_case("one fewer than there are: more", "", Some(24), "en");
    limit_case("the most limit", "", Some(100), "en");
    limit_case("a limit of 0", "", Some(0), "en");
    limit_case("a limit past the most", "", Some(101), "en");
    limit_case("words of 200 characters", &long, None, "en");
    limit_case("words of 201 characters", &longer, None, "en");
    limit_case("a language of neither", "person", None, "fr");
    // two parameters wrong at once: the first of `q`, `limit`, `lang` is named
    limit_case("words of 201 characters and a limit of 0: the words are named", &longer, Some(0), "en");
    limit_case("a limit of 0 and a language of neither: the limit is named", "", Some(0), "fr");
    out.iter().map(|j| format!("{}\n", j.compact())).collect()
}
