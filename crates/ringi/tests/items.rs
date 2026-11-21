//! What a `.ringi` holds (DESIGN 9.7), as the port `Items` hands it over and as `ringi api` writes
//! it (DESIGN 16.1): every file of `examples/` says its things, held in `tests/golden/items/`
//! (`RINGI_BLESS=1 cargo test -p ringi --test items` writes them again); a definition is the same
//! however the file is spaced and commented, and a change of meaning changes the things it is
//! written in and no other; and the command and the ports say the same of a file.

mod common;

use ringi::ports::Engine;
use ritsu_base::json::{self, Json};
use ritsu_base::naming::{self, Tool};
use ritsu_ports::{Found, Item, Items, Lined, Requests};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn items(file: &str) -> Vec<Item> {
    Engine::default().items(&root(), file).unwrap_or_else(|e| panic!("{file}: {e:?}"))
}

/// The things of a file, a line each with its lines and its naming, and its definition under it.
fn listed(file: &str) -> String {
    let mut out = String::new();
    for i in items(file) {
        out.push_str(&format!("{}-{} {}\n", i.lines.0, i.lines.1, i.naming.text()));
        for l in i.text.lines() {
            out.push_str(&format!("    {l}\n"));
        }
    }
    out
}

#[test]
fn every_file_of_the_examples_says_what_it_holds() {
    let files = common::example_files();
    assert_eq!(files.len(), 13, "{files:?}");
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let n = src.lines().count();
        let all = items(f);
        assert!(!all.is_empty(), "{f}");
        let mut seen = std::collections::BTreeSet::new();
        for i in &all {
            assert!(i.lines.0 >= 1 && i.lines.0 <= i.lines.1 && i.lines.1 <= n, "{f}: {i:?}");
            assert!(!i.text.is_empty(), "{f}: {} has no definition", i.naming.text());
            assert_eq!(naming::parse_one(&i.naming.text()).ok().as_ref(), Some(&i.naming), "{f}: the naming reads back");
            assert!(Tool::Ringi.top_kinds().contains(&i.kind()), "{f}: {} is no kind of ringi", i.kind());
            assert!(seen.insert(i.naming.text()), "{f}: {} is there twice", i.naming.text());
            // the definition starts on the first line the thing is written on, or is its column
            if i.kind() != "stage" {
                let first = ringi::items::tidy(src.lines().nth(i.lines.0 - 1).unwrap());
                assert_eq!(i.text.lines().next(), Some(first.as_str()), "{f}: {}", i.naming.text());
            }
            *kinds.entry(i.kind().to_string()).or_default() += 1;
        }
        let name = Path::new(f).strip_prefix("examples").unwrap().display().to_string().replace('/', "_");
        ritsu_testkit::golden::golden(root().join("tests/golden/items").join(format!("{name}.txt")), &listed(f));
    }
    // every kind is in the examples
    for k in ringi::items::KINDS {
        assert!(kinds.contains_key(k), "no {k} in the examples: {kinds:?}");
    }
    assert_eq!(ringi::items::KINDS.to_vec(), Tool::Ringi.top_kinds(), "the kinds are the ones a naming takes");
    // a request in Japanese names holds what the one in English names holds
    for f in files.iter().filter(|f| f.ends_with(".ja.ringi")) {
        let en = f.replace(".ja.ringi", ".ringi");
        let count = |file: &str| {
            let mut c: BTreeMap<String, usize> = BTreeMap::new();
            for i in items(file) {
                *c.entry(i.kind().to_string()).or_default() += 1;
            }
            c
        };
        assert_eq!(count(f), count(&en), "{f}");
    }
}

/// The expense request, written into a directory of its own with what it reads beside it.
fn expense(t: &ritsu_testkit::TempDir, edit: &dyn Fn(String) -> String) -> Vec<Item> {
    let src = std::fs::read_to_string("examples/expense.ringi").unwrap();
    std::fs::write(t.path().join("expense.ringi"), edit(src)).unwrap();
    Engine::default().items(t.path(), "expense.ringi").unwrap()
}

#[test]
fn a_definition_is_the_same_however_it_is_spaced_and_commented() {
    let t = ritsu_testkit::TempDir::new("items");
    let plain = expense(&t, &|s| s);
    // every table lined up another way, a comment after a line and one on a line of its own
    let spaced = expense(&t, &|s| {
        s.lines()
            .map(|l| if l.starts_with('|') { l.replace(" |", "    |") } else if l.starts_with("  manager ") { format!("{l}   # the line above\n  # a comment of its own") } else { l.to_string() })
            .collect::<Vec<_>>()
            .join("\n")
    });
    assert_eq!(plain.len(), spaced.len());
    for (a, b) in plain.iter().zip(&spaced) {
        assert_eq!((a.naming.text(), &a.text), (b.naming.text(), &b.text));
    }
    // a `#` in a string is no comment, and the spaces in a string stay
    assert_eq!(ringi::items::tidy("  what  : text \"What  #1\"   count <=100  # the name"), "what : text \"What  #1\" count <=100");
}

#[test]
fn a_change_of_meaning_changes_the_things_it_is_written_in() {
    let t = ritsu_testkit::TempDir::new("items");
    let plain = expense(&t, &|s| s);
    let changed = |edit: &dyn Fn(String) -> String| -> Vec<String> {
        let now = expense(&t, edit);
        assert_eq!(plain.len(), now.len());
        plain.iter().zip(&now).filter(|(a, b)| a.text != b.text).map(|(a, _)| a.naming.items[0].clone()).map(|(k, n)| format!("{k} {n}")).collect()
    };
    // a cell of the route's table: the table, and the one stage whose column it is in
    assert_eq!(changed(&|s| s.replace("| travel   | >30_000JPY <=100_000JPY    |             |         |                          | manager       |", "| travel   | >30_000JPY <=100_000JPY    |             |         |                          | dept_head     |")), ["table route", "stage decide"]);
    // an end of a row: the table, and every stage, whose rows are read by it
    assert_eq!(changed(&|s| s.replace("| travel   | <=30_000JPY                |", "| travel   | <=20_000JPY                |").replace("| travel   | >30_000JPY <=100_000JPY    |", "| travel   | >20_000JPY <=100_000JPY    |")), ["table route", "stage precheck", "stage first", "stage second", "stage decide"]);
    // a way, a deadline, a rule of the ticket, a field of rows (with the rows it is in)
    assert_eq!(changed(&|s| s.replace("accounting    = team accounting", "accounting    = team board")), ["who accounting"]);
    assert_eq!(changed(&|s| s.replace("due in 3 business days", "due in 5 business days")), ["deadline every_stage"]);
    assert_eq!(changed(&|s| s.replace("at most 3 times", "at most 2 times")), ["ticket send_back"]);
    assert_eq!(changed(&|s| s.replace("<=1_000_000JPY\n  amount", "<=2_000_000JPY\n  amount")), ["field items", "field items.price"]);
    assert_eq!(changed(&|s| s.replace("on approved    post budget.reserve", "on approved, rejected  post budget.reserve")), ["ticket post_budget.reserve"]);
}

#[test]
fn a_file_that_does_not_parse_says_why() {
    let t = ritsu_testkit::TempDir::new("items");
    std::fs::write(t.path().join("broken.ringi"), "request broken v1 \"Broken\"\n\nform\n  amount : money[JPY \"Amount\"\n").unwrap();
    let said = Engine::default().items(t.path(), "broken.ringi").unwrap_err();
    assert_eq!(said[0].code, "E001", "{said:?}");
    assert_eq!(Engine::default().items(t.path(), "missing.ringi").unwrap_err()[0].code, "");
}

fn api(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// A place of the JSON, by its keys; null where it is not there.
fn at<'j>(j: &'j Json, keys: &[&str]) -> &'j Json {
    const NULL: Json = Json::Null;
    let mut v = j;
    for k in keys {
        v = match v.get(k) {
            Some(x) => x,
            None => return &NULL,
        };
    }
    v
}

fn text<'j>(j: &'j Json, keys: &[&str]) -> Option<&'j str> {
    at(j, keys).as_str()
}

fn int(j: &Json, keys: &[&str]) -> Option<i128> {
    at(j, keys).as_int()
}

fn list<'j>(j: &'j Json, keys: &[&str]) -> &'j [Json] {
    at(j, keys).as_arr().unwrap_or_else(|| panic!("{keys:?} is no list"))
}

/// `ringi api` says what the ports say: the things of `Items`, the form of `Requests::form`, the
/// days of `Requests::usable` and the ticket of `Requests::ticket`.
#[test]
fn the_api_says_what_the_ports_say() {
    let engine = Engine::new(common::joined());
    for f in common::example_files() {
        let (code, out, err) = api(&["api", &f, "--root", "."]);
        // the contract review does not pass its check in the examples (DESIGN 19)
        if f.contains("contract") {
            assert_eq!((code, out.as_str()), (1, ""), "{f}");
            assert!(err.contains("E503") || err.contains("E504"), "{f}: {err}");
            continue;
        }
        assert_eq!((code, err.as_str()), (0, ""), "{f}");
        let j = json::parse(&out).unwrap_or_else(|e| panic!("{f}: {e:?}\n{out}"));
        let keys: Vec<&str> = j.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["ringi", "file", "kind", "name", "written", "version", "title", "description", "owner", "effective_from", "uses", "form", "items", "usable", "utc_offset_minutes", "ticket"], "{f}");
        assert_eq!((text(&j, &["ringi"]), text(&j, &["file"])), (Some(env!("CARGO_PKG_VERSION")), Some(f.as_str())), "{f}");
        // the things
        let things = engine.items(&root(), &f).unwrap();
        let listed = list(&j, &["items"]);
        assert_eq!(listed.len(), things.len(), "{f}");
        for (a, i) in listed.iter().zip(&things) {
            assert_eq!((text(a, &["kind"]), text(a, &["written"]), text(a, &["text"])), (Some(i.kind()), Some(i.name()), Some(i.text.as_str())), "{f}");
            let lines = list(a, &["lines"]);
            assert_eq!((lines[0].as_int(), lines[1].as_int()), (Some(i.lines.0 as i128), Some(i.lines.1 as i128)), "{f}");
            // a claim and a scenario have the one name, as written
            assert!(matches!(i.kind(), "claim" | "scenario") || text(a, &["name"]).unwrap().is_ascii(), "{f}: {}", a.compact());
        }
        let request = text(&j, &["kind"]) == Some("request");
        // the form, the days and the ticket
        match engine.form(Path::new(&f)) {
            Ok(form) => {
                assert!(request, "{f}");
                assert_eq!(text(&j, &["name"]), Some(form.request.as_str()), "{f}");
                let fields = list(&j, &["form"]);
                assert_eq!(fields.len(), form.fields.len(), "{f}");
                for (a, x) in fields.iter().zip(&form.fields) {
                    assert_eq!((text(a, &["name"]), text(a, &["written"]), text(a, &["type_written"]), int(a, &["line"])), (Some(x.code_name.as_str()), Some(x.name.as_str()), Some(x.ty.as_str()), Some(x.line as i128)), "{f}");
                    // the type is the type as written, but for the alias of what a choice is from, which is in ASCII
                    let ty = text(a, &["type"]).unwrap();
                    assert!(ty == x.ty || (x.ty.starts_with("choice from ") && ty.starts_with("choice from ") && ty.is_ascii()), "{f}: {ty}");
                    // the port's range, written as `types/<name>.json` writes it (DESIGN 12.2): a
                    // number's as the integers on the wire, a date's as dates, and the limits on a
                    // text's length or on the rows as `count_min` and `count_max`
                    let keys: Vec<&str> = a.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
                    assert_eq!(keys, ["name", "written", "label", "type", "type_written", "min", "max", "count_min", "count_max", "values", "optional", "computed", "approvers_only", "line", "fields"], "{f} {}", x.name);
                    let day = |d: Option<i128>| d.map(|d| ritsu_ports::day_text(i64::try_from(d).unwrap()));
                    let ends = (at(a, &["min"]), at(a, &["max"]), at(a, &["count_min"]), at(a, &["count_max"]));
                    match x.ty.as_str() {
                        "date" => assert_eq!((text(a, &["min"]).map(str::to_string), text(a, &["max"]).map(str::to_string), ends.2.is_null(), ends.3.is_null()), (day(x.lo), day(x.hi), true, true), "{f} {}", x.name),
                        "text" | "long text" | "rows" => assert_eq!((ends.0.is_null(), ends.1.is_null(), ends.2.as_int(), ends.3.as_int()), (true, true, x.lo, x.hi), "{f} {}", x.name),
                        _ => assert_eq!((ends.0.as_int(), ends.1.as_int(), ends.2.is_null(), ends.3.is_null()), (x.lo, x.hi, true, true), "{f} {}", x.name),
                    }
                    assert_eq!(list(a, &["fields"]).len(), x.fields.len(), "{f} {}", x.name);
                }
            }
            Err(_) => assert!(!request && list(&j, &["form"]).is_empty(), "{f}"),
        }
        let usable = engine.usable(Path::new(&f)).unwrap();
        assert_eq!(text(&j, &["usable", "from"]).map(str::to_string), usable.from.as_ref().map(|e| ritsu_ports::day_text(e.day)), "{f}");
        assert_eq!(text(&j, &["usable", "through"]).map(str::to_string), usable.through.as_ref().map(|e| ritsu_ports::day_text(e.day)), "{f}");
        // the offset a request's days are read in (DESIGN 7.5); null for the other files
        assert_eq!(at(&j, &["utc_offset_minutes"]).as_int().is_some(), request, "{f}");
        match engine.ticket(Path::new(&f)).unwrap() {
            Found::Value(t) => {
                let a = at(&j, &["ticket"]);
                let u = |n: Option<u64>| n.map(i128::from);
                assert_eq!(at(a, &["self_approval"]).as_bool(), Some(t.self_approval), "{f}");
                assert_eq!(list(a, &["operations"]).iter().map(|o| text(o, &["name"]).unwrap()).collect::<Vec<_>>(), t.operations.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), "{f}");
                assert_eq!(list(a, &["endings"]).len(), t.endings.len(), "{f}");
                assert_eq!((int(a, &["time", "round"]), int(a, &["time", "total"]), int(a, &["time", "held"])), (u(t.time.round), u(t.time.total), u(t.time.held)), "{f}");
                assert_eq!(list(a, &["time", "no_end"]).len(), t.time.no_end.len(), "{f}");
                assert_eq!(list(a, &["gates"]).len(), t.gates.len(), "{f}");
                assert_eq!(list(a, &["holds"]).iter().map(|h| text(h, &["transfer"]).unwrap()).collect::<Vec<_>>(), t.holds.iter().map(|h| h.transfer.as_str()).collect::<Vec<_>>(), "{f}");
                let flows = list(a, &["flows"]);
                assert_eq!(flows.len(), t.flows.len(), "{f}");
                for (x, fl) in flows.iter().zip(&t.flows) {
                    assert_eq!((text(x, &["kind"]), text(x, &["written"]), int(x, &["line"])), (Some(fl.kind.as_str()), Some(fl.alias.as_str()), Some(fl.line as i128)), "{f}");
                    assert!(text(x, &["alias"]).unwrap().is_ascii(), "{f}");
                    assert_eq!(list(x, &["given"]).iter().map(|g| text(g, &["name"]).unwrap()).collect::<Vec<_>>(), fl.given.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), "{f}");
                    // whether a value can be missing, where its range is held to less, and what may
                    // hold it to less still: what the port says of each. The first two are the words
                    // without the rows of the route, and the rows' lines apart, which the words with
                    // the lines in them (what a message says) name
                    let lined = |v: &Json, k: &str| {
                        text(v, &[k, "text"]).map(|t| (t.to_string(), list(v, &[k, "lines"]).iter().map(|n| n.as_int().unwrap() as usize).collect::<Vec<_>>()))
                    };
                    let want = |l: &Lined| {
                        assert!(!l.text.en.contains(" on line") && !l.text.ja.contains("行目"), "{f}: {}", l.text.en);
                        assert!(l.lines.iter().all(|n| l.said.en.contains(&n.to_string()) && l.said.ja.contains(&n.to_string())), "{f}: {}", l.said.en);
                        (l.text.en.clone(), l.lines.clone())
                    };
                    for (v, g) in list(x, &["given"]).iter().zip(&fl.given) {
                        match &g.none {
                            Found::Value(n) => assert_eq!(lined(v, "none"), n.as_ref().map(want), "{f} {}", g.name),
                            Found::Undecided(why) => assert_eq!(text(v, &["none", "undecided"]), Some(why.en.as_str()), "{f} {}", g.name),
                        }
                        assert_eq!(lined(v, "where"), g.narrowed.as_ref().map(want), "{f} {}", g.name);
                        assert_eq!(text(v, &["may_be_less"]), g.may_be_less.as_ref().map(|t| t.en.as_str()), "{f} {}", g.name);
                    }
                }
            }
            Found::Undecided(_) => assert!(!request && at(&j, &["ticket"]).is_null(), "{f}"),
        }
    }
    // the whole JSON of the leave request and of the organization
    for (file, name) in [("examples/leave.ringi", "leave"), ("examples/leave.ja.ringi", "leave.ja"), ("examples/shared/company.ringi", "company")] {
        let (code, out, _) = api(&["api", file, "--root", "."]);
        assert_eq!(code, 0, "{file}");
        ritsu_testkit::golden::golden(root().join("tests/golden/api").join(format!("{name}.json")), &out);
    }
    // a name in the JSON is the ASCII name, with the name as written beside it: the request in
    // Japanese names has the names of the one in English names
    let read = |file: &str| json::parse(&api(&["api", file, "--root", "."]).1).unwrap();
    let (j, e) = (read("examples/leave.ja.ringi"), read("examples/leave.ringi"));
    assert!(text(&j, &["name"]).unwrap().is_ascii() && !text(&j, &["written"]).unwrap().is_ascii());
    let names = |v: &Json, key: &str| list(v, &[key]).iter().map(|x| (text(x, &["kind"]).unwrap_or("").to_string(), text(x, &["name"]).unwrap_or("").to_string())).filter(|(k, _)| k != "claim" && k != "scenario").collect::<Vec<_>>();
    assert_eq!(names(&j, "form"), names(&e, "form"), "the fields have the same ASCII names");
    assert_eq!(names(&j, "items"), names(&e, "items"), "the things have the same ASCII names");
}

/// `ringi api` and the description of a kind that `ritsu gen --target requests` writes
/// (`types/<name>.json`, DESIGN 12.2) say a field of the form alike, in every key the two have:
/// the names and the type, the range of its value (a date's as dates) and the limits on its length
/// or its rows, and the fields of its rows in the same way.
#[test]
fn the_api_and_the_description_of_a_kind_say_a_field_alike() {
    const SHARED: [&str; 12] = ["name", "written", "label", "type", "type_written", "min", "max", "count_min", "count_max", "optional", "computed", "approvers_only"];
    fn alike(f: &str, a: &Json, t: &Json) {
        for k in SHARED {
            assert_eq!(at(a, &[k]), at(t, &[k]), "{f}: `{k}` of `{}`", text(a, &["name"]).unwrap_or_default());
        }
        let (rows, of_types) = (list(a, &["fields"]), list(t, &["fields"]));
        assert_eq!(rows.len(), of_types.len(), "{f}");
        rows.iter().zip(of_types).for_each(|(x, y)| alike(f, x, y));
    }
    let engine = Engine::new(common::joined());
    let mut dates = 0;
    // the purchase's share paid in advance, and a request of the machine's tests with rates of each
    // step: both say a rate's step, the one written with none too, as the integer counts it
    // (`rate[step 1%]`. DESIGN 12.2, 13.1)
    let mut files = common::example_files();
    files.push("tests/machine/steps.ringi".to_string());
    let mut rates = Vec::new();
    for f in files {
        let (code, out, _) = api(&["api", &f, "--root", "."]);
        // the contract review does not pass its check in the examples (DESIGN 19)
        if code != 0 && f.contains("contract") {
            continue;
        }
        let j = json::parse(&out).unwrap_or_else(|e| panic!("{f}: {e:?}"));
        if text(&j, &["kind"]) != Some("request") {
            continue;
        }
        let files = engine.requests(&[PathBuf::from(&f)], &root()).unwrap_or_else(|t| panic!("{f}: {}", t.en));
        let (_, described, _) = files.iter().find(|(p, _, _)| p.starts_with("types/")).unwrap_or_else(|| panic!("{f}: no types/"));
        let t = json::parse(described).unwrap();
        let (fields, of_types) = (list(&j, &["form"]), list(&t, &["form"]));
        assert_eq!(fields.len(), of_types.len(), "{f}");
        fields.iter().zip(of_types).for_each(|(a, x)| alike(&f, a, x));
        dates += fields.iter().filter(|a| text(a, &["type"]) == Some("date") && text(a, &["min"]).is_some()).count();
        for a in fields.iter().chain(fields.iter().flat_map(|a| list(a, &["fields"]))) {
            if let (Some(ty), Some(written)) = (text(a, &["type"]), text(a, &["type_written"])) {
                if written.starts_with("rate") {
                    rates.push(format!("{ty} ({written})"));
                }
            }
        }
    }
    // the examples have dates with a range, which both write as dates
    assert!(dates >= 4, "{dates}");
    assert_eq!(
        rates,
        [
            "rate[step 5%] (rate[step 5%])",
            "rate[step 5%] (rate[step 5%])",
            "rate[step 0.25%] (rate[step 0.25%])",
            "rate[step 0.25%] (rate[step 0.25%])",
            "rate[step 2.5%] (rate[step 2.5%])",
            "rate[step 1%] (rate)",
            "rate[step 0.1%] (rate[step 0.1%])",
            "rate[step 0.25%] (rate[step 0.25%])",
            "rate[step 0.5%] (rate[step 0.5%])"
        ]
    );
}

/// The UTC offset a kind's days are read in (DESIGN 7.5) and whether its requester may approve their
/// own request (DESIGN 5.5), which a server needs of each kind (DESIGN 12.4), as the description of
/// the kind says them (`types/<name>.json`: `utc_offset_minutes`, `self_approval`) and as `ringi api`
/// does (`utc_offset_minutes`, `ticket.self_approval`): what the request and its first calendar
/// write (`self approval allowed`, `offset +09:00`), for each kind of the examples, in English and
/// in Japanese; and, as they write otherwise, other values: the leave written to allow it, and the
/// leave read in a calendar of another offset.
#[test]
fn the_api_and_the_description_of_a_kind_say_its_offset_and_self_approval_as_written() {
    // what the request at `f` writes: whether `self approval allowed` (`not allowed` when it writes
    // neither), and the offset of the first calendar it uses that writes one, in minutes east of
    // UTC (UTC when none does)
    fn written(f: &str) -> (i128, bool) {
        let src = ritsu_base::fs::read_to_string(Path::new(f)).unwrap();
        let allowed = src.lines().any(|l| l.split_whitespace().collect::<Vec<_>>() == ["self", "approval", "allowed"]);
        let dir = Path::new(f).parent().unwrap();
        let offset = src.lines().filter(|l| l.starts_with("use calendar ")).find_map(|l| {
            let cal = ritsu_base::fs::read_to_string(&dir.join(l.split('"').nth(1)?)).unwrap();
            let o = cal.lines().find_map(|x| x.strip_prefix("offset "))?.trim();
            let (sign, hm) = o.split_at(1);
            let (h, m) = hm.split_once(':')?;
            let minutes = h.parse::<i128>().unwrap() * 60 + m.parse::<i128>().unwrap();
            Some(if sign == "-" { -minutes } else { minutes })
        });
        (offset.unwrap_or(0), allowed)
    }
    // what the description and `ringi api` say of the request at `f`; the api's None where the
    // examples' contract review does not pass its check alone (DESIGN 19)
    fn said(f: &str) -> ((Option<i128>, Option<bool>), Option<(Option<i128>, Option<bool>)>) {
        let engine = Engine::new(common::joined());
        // as `ritsu gen` runs it (DESIGN 12.3)
        engine.keep_listing_apart();
        let files = engine.requests(&[PathBuf::from(f)], Path::new(".")).unwrap_or_else(|t| panic!("{f}: {}", t.en));
        let (_, described, _) = files.iter().find(|(p, _, _)| p.starts_with("types/")).unwrap_or_else(|| panic!("{f}: no types/"));
        let t = json::parse(described).unwrap();
        let keys: Vec<&str> = t.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
        let at_key = |k: &str| keys.iter().position(|x| *x == k).unwrap_or_else(|| panic!("{f}: no `{k}` in {keys:?}"));
        // each beside the key it goes with: the days it can be used on, and who may submit it
        assert_eq!((at_key("utc_offset_minutes"), at_key("self_approval")), (at_key("usable") + 1, at_key("submitted_by") + 1), "{f}");
        let described = (int(&t, &["utc_offset_minutes"]), at(&t, &["self_approval"]).as_bool());
        let (code, out, err) = api(&["api", f, "--root", "."]);
        if code != 0 {
            assert!(f.contains("contract") && (err.contains("E503") || err.contains("E504")), "{f}: {err}");
            return (described, None);
        }
        let j = json::parse(&out).unwrap();
        (described, Some((int(&j, &["utc_offset_minutes"]), at(&j, &["ticket", "self_approval"]).as_bool())))
    }
    let requests: Vec<String> = common::example_files().into_iter().filter(|f| !f.starts_with("examples/shared/")).collect();
    assert_eq!(requests.len(), 10, "{requests:?}");
    let mut apis = 0;
    for f in &requests {
        let (offset, allowed) = written(f);
        let (described, api) = said(f);
        assert_eq!(described, (Some(offset), Some(allowed)), "{f}: the description");
        if let Some(a) = api {
            assert_eq!(a, (Some(offset), Some(allowed)), "{f}: ringi api");
            apis += 1;
        }
    }
    // the examples read their days at +09:00 and do not let a requester approve their own request;
    // `ringi api` says so of the eight that pass their check alone
    assert!(requests.iter().all(|f| written(f) == (540, false)), "{requests:?}");
    assert_eq!(apis, 8);
    // written otherwise, said otherwise: the leave that lets its requester approve, and the leave
    // read in a calendar half an hour off the hour west of UTC
    let leave = std::fs::read_to_string("examples/leave.ringi").unwrap();
    let office = std::fs::read_to_string("examples/shared/office.cal").unwrap();
    let mut laid = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    let own = leave.replacen("  self approval  not allowed\n", "  self approval  allowed\n", 1);
    assert_ne!(own, leave);
    laid.put(&ritsu_base::paths::absolute(Path::new("examples/own.ringi")), own);
    let west = leave.replacen("use calendar office from \"shared/office.cal\"", "use calendar office from \"shared/west.cal\"", 1);
    assert_ne!(west, leave);
    laid.put(&ritsu_base::paths::absolute(Path::new("examples/west.ringi")), west);
    let cal = office.replacen("offset +09:00", "offset -03:30", 1);
    assert_ne!(cal, office);
    laid.put(&ritsu_base::paths::absolute(Path::new("examples/shared/west.cal")), cal);
    ritsu_base::fs::with(std::rc::Rc::new(laid), || {
        for (f, want) in [("examples/own.ringi", (540, true)), ("examples/west.ringi", (-210, false))] {
            assert_eq!(written(f), want, "{f}");
            let (described, api) = said(f);
            assert_eq!(described, (Some(want.0), Some(want.1)), "{f}: the description");
            assert_eq!(api, Some((Some(want.0), Some(want.1))), "{f}: ringi api");
        }
    });
}

/// A kind's description says the language its request is written in (`language`, DESIGN 12.2),
/// after its version: the one of the screens' two (`en`, `ja`) its labels do not give. Each example
/// is in English and its version in Japanese in Japanese, and the labels of its title, its fields,
/// its stages and its ways are in the other language. A request that gives no labels is in
/// Japanese where it names itself in other than ASCII, as the versions in Japanese do, and in
/// English otherwise: the leave and its version in Japanese, with no labels, whose stages the shared
/// route titles.
#[test]
fn the_description_of_a_kind_says_the_language_it_is_written_in() {
    fn described(f: &str) -> Json {
        let engine = Engine::new(common::joined());
        engine.keep_listing_apart();
        let files = engine.requests(&[PathBuf::from(f)], Path::new(".")).unwrap_or_else(|t| panic!("{f}: {}", t.en));
        let (_, d, _) = files.iter().find(|(p, _, _)| p.starts_with("types/")).unwrap_or_else(|| panic!("{f}: no types/"));
        json::parse(d).unwrap()
    }
    // the languages of the labels of a thing of the description
    fn langs(j: &Json) -> Vec<&str> {
        list(j, &["labels"]).iter().map(|l| text(l, &["lang"]).unwrap()).collect()
    }
    let requests: Vec<String> = common::example_files().into_iter().filter(|f| !f.starts_with("examples/shared/")).collect();
    assert_eq!(requests.len(), 10, "{requests:?}");
    for f in &requests {
        let t = described(f);
        let keys: Vec<&str> = t.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys.iter().position(|k| *k == "language"), keys.iter().position(|k| *k == "version").map(|v| v + 1), "{f}: {keys:?}");
        let (lang, other) = if f.ends_with(".ja.ringi") { ("ja", "en") } else { ("en", "ja") };
        assert_eq!(text(&t, &["language"]), Some(lang), "{f}");
        let mut things = vec![&t];
        let mut fields: Vec<&Json> = list(&t, &["form"]).iter().collect();
        while let Some(x) = fields.pop() {
            fields.extend(list(x, &["fields"]));
            things.push(x);
        }
        things.extend(list(&t, &["stages"]));
        things.extend(list(&t, &["ways"]));
        for x in things {
            assert_eq!(langs(x), [other], "{f}: {}", x.compact());
        }
    }
    // with no labels, by the name the request gives itself
    let mut laid = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    for (f, bare) in [("examples/leave.ringi", "examples/bare.ringi"), ("examples/leave.ja.ringi", "examples/bare.ja.ringi")] {
        let src = std::fs::read_to_string(f).unwrap();
        let (kept, labels) = src.split_once("\nlabels ").unwrap_or_else(|| panic!("{f} gives no labels"));
        assert!(labels.lines().skip(1).all(|l| l.starts_with("  ")), "{f}: the labels are not the last of the file");
        laid.put(&ritsu_base::paths::absolute(Path::new(bare)), format!("{}\n", kept.trim_end()));
    }
    ritsu_base::fs::with(std::rc::Rc::new(laid), || {
        for (f, lang) in [("examples/bare.ringi", "en"), ("examples/bare.ja.ringi", "ja")] {
            let t = described(f);
            assert_eq!(text(&t, &["language"]), Some(lang), "{f}");
            assert!(list(&t, &["labels"]).is_empty(), "{f}");
        }
        // the stages are the shared route's, written in English and titled in Japanese by the
        // route's labels (DESIGN 4.8, 5.11): the leave takes those as its labels, and its version in
        // Japanese as its titles
        let stages = |f: &str| -> Vec<(String, Vec<String>)> { list(&described(f), &["stages"]).iter().map(|s| (text(s, &["label"]).unwrap().to_string(), langs(s).iter().map(|l| l.to_string()).collect())).collect() };
        let one = |title: &str, langs: &[&str]| (title.to_string(), langs.iter().map(|l| l.to_string()).collect::<Vec<_>>());
        assert_eq!(stages("examples/bare.ringi"), [one("manager", &["ja"]), one("head", &["ja"])]);
        assert_eq!(stages("examples/bare.ja.ringi"), [one("上長", &[]), one("部長", &[])]);
    });
}

#[test]
fn what_stops_the_api() {
    let (code, out, err) = api(&["api"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.contains("`ringi api` takes <file.ringi>\nusage: ringi api <file.ringi> "), "{err}");
    let (code, out, err) = api(&["api", "examples/missing.ringi"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: `examples/missing.ringi` is not there\n");
    // with no other language joined, a request needs them (E008)
    let args: Vec<String> = ["api", "examples/leave.ringi"].iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ringi::run::run(&args, ringi::suite::Suite::default(), &mut out, &mut err);
    assert_eq!((code, out.len()), (2, 0), "{}", String::from_utf8_lossy(&err));
    assert!(String::from_utf8_lossy(&err).contains("E008"));
}
