//! rulec says a number in its sentences as the files write it (§15.333): with its unit, and with
//! its digits grouped by `_` from five on, as `ritsu_units::written` writes them (`1_000_000円`,
//! `10_000_000JPY`, `5000JPY`), never with `,` between the groups and never with 万, 億 or 兆. The
//! sweep reads the title, the labels and the notes of each diagnostic of every reproduction of the
//! ledger, each other form of an entry included, in English and in Japanese, and of every golden
//! file of what rulec prints (`tests/golden`, which the golden tests hold to the output).
//!
//! What a sentence quotes is as the file writes it, and is not read here: what is between
//! backticks (the source, or a line to write in it), and a number written as a line the diagnostic
//! shows writes it (E119's boundary, `93000円` where the cell writes `<=93000円`; E042's
//! declaration of a contribution's input, `<=5万円` where the contribution writes it so). Dates,
//! times and hashes are not numbers here.

use ritsu_testkit::TempDir;
use rulec::i18n::Lang;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The numbers of `sentence` not written as the files write them, each with what is wrong with
/// it. `shown` are the lines of the files the diagnostic shows.
fn off(sentence: &str, shown: &[String]) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for part in sentence.split('`').step_by(2) {
        let cs: Vec<char> = part.chars().collect();
        let mut i = 0;
        while i < cs.len() {
            // A digit inside a word (`E101`, `int64`), past a decimal point, or in a path, a place
            // or a fraction starts no number; one past the `..` between the ends of a range does.
            let inside = match i.checked_sub(1).map(|j| cs[j]) {
                Some(c) if c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '/' | '#') => true,
                Some('.') => i < 2 || cs[i - 2] != '.',
                _ => false,
            };
            if !cs[i].is_ascii_digit() || inside {
                i += 1;
                continue;
            }
            let begin = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || (matches!(cs[i], '_' | ',' | '.' | '-' | ':') && cs.get(i + 1).is_some_and(char::is_ascii_digit))) {
                i += 1;
            }
            let token: String = cs[begin..i].iter().collect();
            // a date, a time, a span of lines; a hash, which is seven or more hex digits and letters
            let word: String = cs[begin..].iter().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if token.contains(['-', ':']) || (word.len() >= 7 && word.chars().all(|c| c.is_ascii_hexdigit()) && word.chars().any(|c| c.is_ascii_alphabetic())) {
                continue;
            }
            let kanji = cs.get(i).copied().filter(|c| matches!(c, '万' | '億' | '兆'));
            let said = format!("{token}{}", kanji.map(String::from).unwrap_or_default());
            if shown.iter().any(|l| writes(l, &said)) {
                continue;
            }
            let whole = token.split('.').next().unwrap_or_default();
            let digits: String = whole.chars().filter(char::is_ascii_digit).collect();
            if token.contains(',') {
                out.push((token, "is grouped with `,`"));
            } else if kanji.is_some() {
                out.push((said, "is written with 万, 億 or 兆"));
            } else if (digits.len() >= 5 || whole.contains('_')) && digits.parse::<i128>().is_ok_and(|n| whole != ritsu_units::written::digits(n)) {
                out.push((token, "is not grouped by `_` in threes from five digits on"));
            }
        }
    }
    out
}

/// Whether `line` writes `number` as a number of its own, not as a part of a longer one. A digit
/// next to it goes on with it, and so does a `_`, `,` or `.` between it and a digit; the `,` that
/// ends an argument (`min(orders * 1000000, 10)`) does not.
fn writes(line: &str, number: &str) -> bool {
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    let joins = |c: Option<char>| matches!(c, Some('_' | ',' | '.'));
    line.match_indices(number).any(|(at, _)| {
        let mut after = line[at + number.len()..].chars();
        let next = after.next();
        let mut before = line[..at].chars().rev();
        let prev = before.next();
        let longer = digit(next) || (joins(next) && digit(after.next()));
        let earlier = digit(prev) || (joins(prev) && digit(before.next()));
        !longer && !earlier
    })
}

/// The sentences of every diagnostic of every reproduction of the ledger, in English and in
/// Japanese, each with where it was said and the lines of the files the diagnostic shows. A
/// reproduction is run as the ledger's test runs it: with its files beside it, and with koyomi but
/// for E129.
fn ledger_sentences() -> Vec<(String, String, Vec<String>)> {
    let mut out = Vec::new();
    for e in rulec::codes::ledger() {
        for lang in [Lang::En, Lang::Ja] {
            let all: Vec<rulec::codes::Repro> = std::iter::once(e.repro(lang)).chain(e.also.iter().map(|a| a.repro(lang))).collect();
            for (form, r) in all.into_iter().enumerate() {
                let tmp = TempDir::new(&format!("numbers-{}", e.code));
                for (name, text) in r.files {
                    let p = tmp.path().join(name);
                    if let Some(parent) = p.parent() {
                        std::fs::create_dir_all(parent).unwrap();
                    }
                    std::fs::write(&p, text).unwrap();
                }
                let path = tmp.path().join("explain.rule").to_string_lossy().into_owned();
                let budget = e.budget.unwrap_or(rulec::region::DEFAULT_BUDGET);
                let port: Option<rulec::days::Port> = (e.code != "E129").then(|| std::sync::Arc::new(koyomi::ports::Engine) as rulec::days::Port);
                let ds = rulec::i18n::with(lang, || {
                    let mut ds = rulec::days::with(port, || rulec::report_with(r.example, &path, budget).diags);
                    if e.code == "E153" {
                        let stops = rulec::places::stops(&ds, &|_| true);
                        ds.extend(stops);
                    }
                    ds
                });
                let lines: Vec<String> = r.example.lines().map(str::to_string).collect();
                for d in &ds {
                    let shown: Vec<String> = d
                        .marks
                        .iter()
                        .filter_map(|m| m.span.file.as_ref().map(|f| f.lines.as_slice()).unwrap_or(&lines[..]).get(m.span.line.saturating_sub(1)).cloned())
                        .collect();
                    let at = format!("{} ({lang:?}, form {form}) {}", e.code, d.code);
                    for s in std::iter::once(&d.title).chain(d.marks.iter().map(|m| &m.label)).chain(&d.notes).filter(|s| !s.is_empty()) {
                        out.push((at.clone(), s.clone(), shown.clone()));
                    }
                }
            }
        }
    }
    out
}

/// The sentences of the diagnostics in a text rulec printed, each with the lines of the file the
/// diagnostic shows: the title, the label past the carets, and the notes, as `diag::render` writes
/// them.
fn printed_sentences(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut cur: Option<(Vec<String>, Vec<String>)> = None;
    let flush = |cur: &mut Option<(Vec<String>, Vec<String>)>, out: &mut Vec<(String, Vec<String>)>| {
        if let Some((said, shown)) = cur.take() {
            out.extend(said.into_iter().map(|s| (s, shown.clone())));
        }
    };
    for line in text.lines() {
        let head = line.split_once('[').filter(|(h, _)| ["error", "warning", "エラー", "警告"].contains(h)).and_then(|(_, r)| r.split_once("]: "));
        if let Some((_, title)) = head {
            flush(&mut cur, &mut out);
            cur = Some((vec![title.to_string()], Vec::new()));
            continue;
        }
        let Some((said, shown)) = cur.as_mut() else { continue };
        if line.starts_with("  --> ") {
            continue;
        }
        let t = line.trim_start();
        let n = t.chars().take_while(char::is_ascii_digit).count();
        let gutter = if n == 0 { t.strip_prefix('|') } else { t[n..].strip_prefix(" |") };
        if let Some(g) = gutter {
            match g.trim_start().strip_prefix('^') {
                Some(carets) if n == 0 => {
                    let label = carets.trim_start_matches('^').trim_start();
                    if !label.is_empty() {
                        said.push(label.to_string());
                    }
                }
                _ if n > 0 => shown.push(g.strip_prefix(' ').unwrap_or(g).to_string()),
                _ => {}
            }
        } else if let Some(note) = line.strip_prefix(' ').filter(|x| !x.trim().is_empty()) {
            said.push(note.to_string());
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

fn under(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            under(&p, out);
        } else if p.extension().is_some_and(|x| x == "txt") {
            out.push(p);
        }
    }
}

#[test]
fn every_sentence_says_a_number_as_the_files_write_it() {
    let mut wrong = Vec::new();
    let said = ledger_sentences();
    for (at, s, shown) in &said {
        for (n, why) in off(s, shown) {
            wrong.push(format!("{at}: `{n}` {why}: {s}"));
        }
    }
    let golden = root().join("tests/golden");
    let mut files = Vec::new();
    under(&golden, &mut files);
    files.sort();
    let mut printed = 0;
    for p in &files {
        let rel = p.strip_prefix(&golden).unwrap().display().to_string();
        for (s, shown) in printed_sentences(&std::fs::read_to_string(p).unwrap()) {
            printed += 1;
            for (n, why) in off(&s, &shown) {
                wrong.push(format!("golden/{rel}: `{n}` {why}: {s}"));
            }
        }
    }
    eprintln!("read: {} sentences of the ledger's reproductions, and {printed} of {} golden files", said.len(), files.len());
    // 1,744 and 3,684 when this was written
    assert!(said.len() >= 1500 && printed >= 3300, "{} sentences of the ledger's reproductions and {printed} of the golden files read: were the outputs or their form changed?", said.len());
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The page says an answer as rulec says it: the try box of the page `rulec doc --format html`
/// writes (the one `--extra mcp` writes beside the code too) shows an answer with the script's
/// `shown`, which groups the digits as `types::written_num` does. The two functions are taken from
/// a page and run with node over values on either side of five digits, below zero, and as rates.
#[test]
fn the_page_says_an_answer_as_rulec_does() {
    let node = || std::process::Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success());
    if !ritsu_testkit::ready(ritsu_testkit::Need::Node, node, "node is not on the machine") {
        return;
    }
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec"))
        .args(["doc", "tests/corpus/member_shipping_fee.rule", "--format", "html", "--lang", "en"])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let page = String::from_utf8(o.stdout).unwrap();
    let take = |name: &str| -> String {
        let at = page.find(&format!("\nfunction {name}(")).unwrap_or_else(|| panic!("the page has no function {name}"));
        let end = page[at + 1..].find("\n}\n").unwrap();
        page[at + 1..at + 1 + end + 2].to_string()
    };
    // (the value's kind, its unit, the wire scale of a rate, the value on the wire)
    let cases: [(&str, &str, i64, i64); 9] = [
        ("number", "JPY", 1, 1234),
        ("number", "JPY", 1, 12345),
        ("number", "JPY", 1, 1234567),
        ("number", "円", 1, -12345),
        ("number", "", 1, 99999),
        ("number", "g", 1, 0),
        ("rate", "%", 100, 1234567),
        ("rate", "%", 1000, 125),
        ("rate", "%", 1000, 12345678),
    ];
    let objs: Vec<String> = cases.iter().map(|(k, u, s, v)| format!("[{{\"kind\":\"{k}\",\"unit\":\"{u}\",\"scale\":{s}}},{v}]")).collect();
    let script = format!(
        "const dateOf = (d) => String(d);\n{}{}console.log(JSON.stringify([{}].map(([o, v]) => shown(o, v))));\n",
        take("grouped"),
        take("shown"),
        objs.join(",")
    );
    let o = std::process::Command::new("node").args(["-e", &script]).output().unwrap();
    assert!(o.status.success(), "{}\n{script}", String::from_utf8_lossy(&o.stderr));
    let said = String::from_utf8(o.stdout).unwrap();
    assert_eq!(said.trim(), r#"["1234JPY","12_345JPY","1_234_567JPY","-12_345円","99_999","0g","1_234_567%","12.5%","1_234_567.8%"]"#, "{script}");
    // and so does rulec
    for (k, u, s, v) in cases {
        let r = rulec::num::Rat::new(i128::from(v), i128::from(s));
        let ty = match (k, u) {
            ("rate", _) => rulec::types::Ty::Rate,
            (_, "") => rulec::types::Ty::Number,
            _ => rulec::types::Ty::Money { cur: u.to_string(), tax: None },
        };
        assert!(said.contains(&format!("\"{}\"", rulec::types::fmt_val(r, &ty))), "{k} {v}: rulec writes {}, the page {said}", rulec::types::fmt_val(r, &ty));
    }
}

/// `rulec diff` says the ends of a range that moved as the file writes a value of its type: an
/// input's, a field of an element's and a definition's, in English and in Japanese. Its JSON keeps
/// the plain numbers the two versions are compared on.
#[test]
fn the_diff_says_each_range_as_the_file_writes_it() {
    let before = "rule parcel_fee v1\n\
                  description \"The fee for a parcel: the weight it is charged on, and the price of each line it carries\"\n\n\
                  elements lines\n  price : money[JPY]  range >=0JPY <=10_000JPY\n\n\
                  sum subtotal over lines of price  range >=0JPY <=100_000JPY\n\n\
                  inputs\n  weight : mass[g]  range >=1g <=30_000g\n\n\
                  define charged : mass[g] = weight * 2  range >=2g <=60_000g\n\n\
                  outputs\n  fee : money[JPY]  round down(1JPY)\n\n\
                  table fee_of\npolicy first\n\
                  | charged   | subtotal    | -> fee   |\n\
                  | -         | >=50_000JPY | 0JPY     |\n\
                  | <=20_000g | <50_000JPY  | 800JPY   |\n\
                  | -         | -           | 1_200JPY |\n";
    let now = before
        .replace("range >=0JPY <=10_000JPY", "range >=0JPY <=20_000JPY")
        .replace("range >=1g <=30_000g", "range >=1g <=20_000g")
        .replace("range >=2g <=60_000g", "range >=2g <=30_000g");
    let t = TempDir::new("numbers-diff");
    std::fs::write(t.path().join("before.rule"), before).unwrap();
    std::fs::write(t.path().join("now.rule"), &now).unwrap();
    let diff = |lang: &str, more: &[&str]| -> String {
        let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec"))
            .env("RULEC_LANG", lang)
            .current_dir(t.path())
            .args(["diff", "before.rule", "now.rule"])
            .args(more)
            .args(["--lang", lang])
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
    };
    for (lang, lines) in [
        (
            "en",
            [
                "an input changed range: weight  1g..30_000g → 1g..20_000g",
                "a field of an element changed range: lines[].price  0JPY..10_000JPY → 0JPY..20_000JPY",
                "a definition changed the range it accepts: charged  2g..60_000g → 2g..30_000g",
            ],
        ),
        (
            "ja",
            [
                "入力の範囲が変わった: weight  1g..30_000g → 1g..20_000g",
                "要素のフィールドの範囲が変わった: lines[].price  0JPY..10_000JPY → 0JPY..20_000JPY",
                "定義が受け付ける範囲が変わった: charged  2g..60_000g → 2g..30_000g",
            ],
        ),
    ] {
        let said = diff(lang, &[]);
        for l in lines {
            assert!(said.lines().any(|x| x.trim() == l), "{lang}: no line `{l}`:\n{said}");
        }
    }
    let json = diff("en", &["--format", "json"]);
    for d in [
        r#""what":"input_range","name":"weight","old":"1..30000","new":"1..20000""#,
        r#""what":"field_range","name":"lines[].price","old":"0..10000","new":"0..20000""#,
        r#""what":"define_range","name":"charged","old":"2..60000","new":"2..30000""#,
    ] {
        assert!(json.contains(d), "the JSON has no {d}:\n{json}");
    }
}

/// The sweep finds each way of writing a number the files do not write, and passes over what a
/// sentence quotes, dates, times and hashes.
#[test]
fn the_sweep_finds_each_wrong_form() {
    let none: [String; 0] = [];
    let found = |s: &str| off(s, &none).into_iter().map(|(n, _)| n).collect::<Vec<_>>();
    assert_eq!(found("the stored integer reaches 10000000000000000000."), ["10000000000000000000"]);
    assert_eq!(found("とりうる値は >=0円 <=100万円 です"), ["100万"]);
    assert_eq!(found("an amount of 1,000,000JPY"), ["1,000,000"]);
    assert_eq!(found("a sum changed range: amount  0JPY..100000JPY → 500JPY..100_000JPY"), ["100000"]);
    assert_eq!(found("it ranges from -1000000 to 10_000_000.5"), ["1000000"]);
    assert_eq!(found("a total of 10_00JPY"), ["10_00"]);
    let fine = [
        "the stored integer reaches 10_000_000_000_000_000_000, past int64",
        "pay_day = 2026-07-10 at 09:33, line 12:5, sha256:5ac7630d669d, 511051f7, version 0.55.2",
        "`,` separates the members of a set, so `2,000g` reads as two values; write `range <=10000000円`",
        "E10001, x100000, 1/100000, 5000JPY, 1000, 0.25%",
    ];
    for s in fine {
        assert!(found(s).is_empty(), "{s}: {:?}", found(s));
    }
    // a number written as a line the diagnostic shows writes it is a quote of that line
    let shown = ["| <=93000円 | 0円 |".to_string(), "  小計(subtotal)   : money[円]  range >=0円 <=5万円".to_string()];
    assert!(off("At exactly 93000円, the copy's column takes it with the larger side", &shown).is_empty());
    assert!(off("money[円] >=0円 <=5万円", &shown).is_empty());
    assert_eq!(off("At exactly 9300円 or 930000円", &shown).into_iter().map(|(n, _)| n).collect::<Vec<_>>(), ["930000"]);
    // the `,` past an argument ends a number; a number the line writes only inside longer ones is
    // not written by it
    let call = ["result points = min(orders * 1000000000000000000, 10)".to_string(), "| 1,100000 | 9_100000 |".to_string()];
    assert!(off("the value orders * 1000000000000000000 on the way", &call).is_empty());
    assert_eq!(off("a cell of 100000", &call).into_iter().map(|(n, _)| n).collect::<Vec<_>>(), ["100000"]);
    // the sentences of a printed diagnostic, and the lines it shows
    let text = "エラー[E042]: 寄与の入力 小計 の型か範囲が、元の入力と違います\n  --> 寄与/離島.rule:6\n  |\n6 |   小計(subtotal)   : money[円]  range >=0円 <=5万円\n  |   ^^^^ money[円] >=0円 <=5万円\n  |\n 同じ名前の入力は同じ入力で、元の規則では `money[JPY] >=0JPY <=100_000JPY` です。\n\n";
    let s = printed_sentences(text);
    assert_eq!(s.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["寄与の入力 小計 の型か範囲が、元の入力と違います", "money[円] >=0円 <=5万円", "同じ名前の入力は同じ入力で、元の規則では `money[JPY] >=0JPY <=100_000JPY` です。"]);
    assert!(s.iter().all(|(_, shown)| shown == &["  小計(subtotal)   : money[円]  range >=0円 <=5万円".to_string()]), "{s:?}");
}
