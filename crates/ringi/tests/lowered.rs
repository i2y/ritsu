//! What a request is lowered to (DESIGN 6.4, PLAN C1.1 and C1.6). Every request and shared route of
//! `examples/` lowers to the files of `tests/golden/lowered/<name>/`, as `ringi extract
//! examples/<name>.ringi --out tests/golden/lowered/<name>` writes them, and the files of the
//! expense request each pass the check of its language as a file of their own. `RINGI_BLESS=1
//! cargo test -p ringi --test lowered` writes them again; read the diff.

mod common;

use ringi::check::{Options, Run};
use ringi::lower;
use ritsu_base::text::Lang;
use ritsu_ports::Found;
use std::path::Path;

/// The names of the files in `dir`, in order.
fn names_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|r| r.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// A run as `ringi extract` makes it: the approvers of the organization's versions are not listed,
/// for they are not lowered.
fn run() -> Run {
    Run::with(common::joined(), Options { unlisted: true, ..Options::default() })
}

/// The files the request at `path` lowers to, written out into `dir` as `ringi extract` writes
/// them, compared with the files there (written there while blessing).
fn compare(path: &str, dir: &Path) -> Vec<String> {
    let o = run().file(path).unwrap();
    assert!(o.diags.is_empty(), "{path}:\n{}", common::shown(&o, Lang::En));
    let l = o.lowered.as_ref().unwrap_or_else(|| panic!("{path} is not lowered"));
    let files = lower::extracted(l, dir);
    let mut failures = Vec::new();
    let mut names: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    names.sort();
    if !ritsu_testkit::golden::bless() && names != names_in(dir) {
        failures.push(format!("{path} lowers to {names:?}, and {} holds {:?}", dir.display(), names_in(dir)));
    }
    for (name, text) in &files {
        if let Err(e) = ritsu_testkit::golden::check(&dir.join(name), text) {
            failures.push(format!("{path}: {e}"));
        }
    }
    failures
}

/// Each file the expense request lowers to passes its language's check as it is written out, with
/// nothing to say: rulec's for the rules, koyomi's for the dates. The calendar the deadlines count
/// in is read from `examples/shared/`.
#[test]
fn the_lowered_expense_passes_rulec_and_koyomi() {
    let suite = common::joined();
    let dir = Path::new("tests/golden/lowered/expense");
    let names = names_in(dir);
    assert_eq!(names, ["choices.rule", "form.rule", "next_up.rule", "route.rule", "terms.cal", "values.rule"]);
    for n in names {
        let p = dir.join(&n);
        let found = if n.ends_with(".rule") { suite.rules.as_ref().unwrap().findings(&p, Lang::En) } else { suite.dates.as_ref().unwrap().findings(&p, Lang::En) };
        match found {
            Ok(Found::Value(v)) => assert!(v.is_empty(), "{}:\n{}", p.display(), v.iter().map(|f| f.text.as_str()).collect::<String>()),
            Ok(Found::Undecided(why)) => panic!("{}: {}", p.display(), why.en),
            Err(e) => panic!("{}: {e:?}", p.display()),
        }
    }
}

/// The requests and the shared route, in English and in Japanese names; an organization and masters
/// lower to nothing.
#[test]
fn every_example_lowers_to_its_golden_files() {
    let mut failures = Vec::new();
    let mut lowered = Vec::new();
    for f in common::example_files() {
        let name = Path::new(&f).file_name().unwrap().to_string_lossy().trim_end_matches(".ringi").to_string();
        let o = run().file(&f).unwrap();
        if o.lowered.as_ref().is_none_or(|l| l.files.is_empty()) {
            continue;
        }
        lowered.push(name.clone());
        failures.extend(compare(&f, &Path::new("tests/golden/lowered").join(&name)));
    }
    let want = ["account", "account.ja", "contract", "contract.ja", "expense", "expense.ja", "leave", "leave.ja", "manager_approval", "purchase", "purchase.ja"];
    lowered.sort();
    assert_eq!(lowered, want);
    if !ritsu_testkit::golden::bless() {
        assert_eq!(names_in(Path::new("tests/golden/lowered")), want, "a golden directory with no example");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A stage named as a word rulec reserves (`final`) is given another name in the lowered rule, in
/// its output, its enum and its table, and the request passes; what the languages say of a renamed
/// name is said with the request's (`days` in koyomi's words, where `form.cal` has `days_value`).
#[test]
fn a_name_the_languages_reserve_is_renamed() {
    let src = "request example v1 \"Example\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n\nroute\n| amount      | -> check | final |\n| <=50_000JPY | finance  |       |\n| >50_000JPY  | finance  | heads |\n\nwho\n  finance = team finance\n  heads   = team heads\n\nticket\n  self approval  not allowed\n\nexamples\n| amount    | -> check | final |\n| 60_000JPY | finance  | heads |\n";
    let o = common::check_text("tests/mutants/final.ringi", src);
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let route = o.lowered.as_ref().unwrap().files.iter().find(|f| f.name == "route.rule").unwrap();
    assert!(route.text.contains("  final_value : final_value_by?\n"), "{}", route.text);
    assert!(route.text.contains("| amount      | -> check | final_value |\n"), "{}", route.text);
    assert_eq!(route.named_back("final_value is final_value_by, not final_values"), "final is final_value_by, not final_values");
    let src = "request example v1 \"Example\"\n\nuse calendar office from \"shared/finite.cal\"\n\nform\n  first_day : date \"First day\"  range >=2026-01-01 <=2027-06-30\n  days      : number \"Business days\"  range >=1 <=20\n  back_on   : date \"Back at work on\" = first_day + days business days in office\n\nroute\n  check  managers\n\nwho\n  managers = team managers\n\nticket\n  self approval  not allowed\n";
    let o = common::check_text("tests/mutants/days.ringi", src);
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let form = o.lowered.as_ref().unwrap().files.iter().find(|f| f.name == "form.cal").unwrap();
    assert!(form.text.contains("days_value"), "{}", form.text);
    assert_eq!(form.named_back("first_day + days_value business days, days_value 1 asks"), "first_day + days business days, days 1 asks");
}

/// A request that names its things with words rulec reads as something else: a choice named as a
/// type (`date`) whose values a cell reads otherwise (`after`, `by`), rows named `count` with a
/// field `max`, a way to choose an approver named `after`, a table named `up`, and a field `over`.
const WORDS: &str = "request example v1 \"Example\"\n\nuse organization org from \"shared/small.ringi\"\nuse calendar office from \"shared/office.cal\"\n\nform\n  date  : choice \"Kind of day\"\n    after \"After\"\n    by    \"By\"\n    usual \"Usual\"\n  over  : number \"Nights\"  range >=1 <=10\n  count : rows \"Items\"  count >=1 <=5\n    max : money[JPY] \"Price\"  range >=1JPY <=1_000JPY\n  total : money[JPY] \"Total\" = sum(count.max)  range >=1JPY <=5_000JPY\n\nroute\n| date      | total       | -> check | decide |\n| after, by | -           | finance  | after  |\n| usual     | <=1_000JPY  | finance  |        |\n| usual     | >1_000JPY   | finance  | after  |\n\nwho\n  finance = team finance\n  after   = team sales\n  top     = team company\n\nevery stage\n  due in 3 business days at end of day\n  when late escalate by up, then expire\n\ntable up\n| approver | -> goes_to |\n| finance  | after      |\n| after    | top        |\n| top      | top        |\n\nticket\n  self approval  allowed\n\nexamples\n| date  | total     | -> check | decide |\n| by    | 100JPY    | finance  | after  |\n| usual | 2_000JPY  | finance  | after  |\n\nscenario a_day_after\n  requester e03\n  form date = after, over = 2, count = [{max: 500JPY}]\n| at               | event   | by  | -> stage | status      |\n| 2026-06-01 09:00 | submit  | e03 | check    | in progress |\n| 2026-06-01 10:00 | approve | e04 | decide   | in progress |\n| 2026-06-01 11:00 | approve | e02 |          | approved    |\n";

/// What rulec reads as something else is lowered under another name (DESIGN 6.4), and the request
/// passes, its examples and its scenario as written: a choice named as a type is the enum
/// `date_value`, its values `after_value` and `by_value`, the rows and their field `count_value`
/// and `max_value`, the way `after` the value `after_approver` in the route's cells and the table's,
/// and the table `up` the file `up_value.rule`. What `simulate` says of it keeps the request's
/// names, in its text and in its JSON.
#[test]
fn names_rulec_reads_otherwise_are_lowered_under_others() {
    use ringi::check::{Options, Run};
    use ringi::simulate::{Ask, On};
    let run = Run::with(common::joined(), Options { unlisted: true, ..Options::default() });
    let o = run.text("tests/mutants/words.ringi", WORDS);
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let has = |file: &str, lines: &[&str]| {
        let text = lowered_text(&o, file);
        for l in lines {
            assert!(text.contains(l), "{l:?} is not in {file}:\n{text}");
        }
    };
    has("choices.rule", &["enum date_value = after_value \"After\" | by_value \"By\" | usual \"Usual\"\n"]);
    has("form.rule", &["elements count_value\n  max_value : money[JPY]", "sum total over count_value of max_value"]);
    has("route.rule", &["import enum date_value from \"choices.rule\"\n", "enum decide_by = after_approver \"after\"\n", "  date  : date_value\n", "| after_value, by_value | -         | finance  | after_approver |\n", "| by_value | 100JPY  | finance  | after_approver |\n"]);
    has("up_value.rule", &["table up_value\n", "| after_approver | top            |\n"]);
    // `simulate` with the values of the form, by the request's names
    let day = ritsu_ports::day_of("2026-06-01").unwrap();
    let ask = Ask { requester: Some("e03".to_string()), every: false, form: Some("date = by, over = 2, count = [{max: 700JPY}, {max: 600JPY}]".to_string()), on: On::Day(day) };
    // `simulate` reads the request where it is: here, laid over the files
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(std::path::Path::new("tests/mutants/words.ringi")), WORDS);
    let a = ritsu_base::fs::with(std::rc::Rc::new(files), || ringi::simulate::simulate(&o, &run, &ask)).unwrap_or_else(|t| panic!("{}", t.en));
    let text = ringi::simulate::render("words.ringi", a.org_version().unwrap(), &a, Lang::En);
    assert!(text.contains("  computed: total = 1300JPY\n") && text.contains("  decide  after: one of e02"), "{text}");
    let json = ringi::simulate::to_json("words.ringi", &a, Lang::En).compact();
    for lowered in ["_value", "_approver"] {
        assert!(!text.contains(lowered) && !json.contains(lowered), "{lowered}:\n{text}\n{json}");
    }
}

/// A request named with a word kaname reserves (`match`), with fields named as the names the glue
/// keeps for its own (`g`, `form`, `org`): the glue declares its module under another name with the
/// request's in brackets (`module match_value(match)`), which the generated code takes, and takes
/// those fields under other names (`form_value`); a field of a row (`y`) is read after a dot, as it
/// is. The request passes, and `check` and `api` say the request's names (DESIGN 6.4).
#[test]
fn names_kaname_reads_otherwise_are_given_others_in_the_glue() {
    let src = "request match v1 \"Match\"\n\nuse organization people from \"shared/company.ringi\"\n\nform\n  g     : text \"Destination\"  count <=100\n  form  : text \"Note\"  count <=100\n  org   : number \"Seats\"  range >=0 <=9\n  count : rows \"Items\"  count >=1 <=5\n    y   : text \"What\"  count <=50\n    max : money[JPY] \"Price\"  range >=1JPY <=1_000JPY\n  total : money[JPY] \"Total\" = sum(count.max)  range >=1JPY <=5_000JPY\n\nroute\n  check  finance\n\nwho\n  finance = team accounting\n\nticket\n  self approval  not allowed\n";
    let path = "examples/match.ringi";
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(Path::new(path)), src);
    ritsu_base::fs::with(std::rc::Rc::new(files), || {
        let engine = ringi::ports::Engine::new(common::joined());
        let glue = engine.glue(Path::new(path), Path::new("examples/.lowered/ticket.kn")).unwrap_or_else(|t| panic!("{}", t.en));
        for line in ["module match_value(match)", "form_value: form_value", "org_value: org_value", "g_value: g_value", "CountRow { y: y.y, max: y.max }"] {
            assert!(glue.text.contains(line), "{line:?} is not in the glue:\n{}", glue.text);
        }
        let ask = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
            (code, String::from_utf8(out).unwrap() + &String::from_utf8(err).unwrap())
        };
        let (code, said) = ask(&["check", path, "--format", "json", "--root", "."]);
        assert!(code == 0 && said.contains("\"ok\": true"), "{said}");
        let (code, api) = ask(&["api", path, "--root", "."]);
        assert!(code == 0 && api.contains("\"name\": \"match\""), "{api}");
        for given in ["_value"] {
            assert!(!said.contains(given) && !api.contains(given), "{given}:\n{said}\n{api}");
        }
    });
}

/// A request whose names, joined into the names the lowered files and the glue declare, come to
/// names it or the languages already have (DESIGN 6.4): a choice named as the enum of the stage
/// `check`'s output (`check_by`), a choice named as a type the code rulec generates reads (`option`,
/// Rust's `Option`), a value and a way to choose an approver that are another's name in Pascal case
/// (`xL`, `headOffice`), a way named as the stage `check` beside a way `check_approver`, and fields
/// and rows whose names make one name of the glue's (`val_x_y`, `cell_a_b_c`, `row_vals_x`).
const CLASH: &str = "request clash v1 \"Clash\"\n\nuse organization people from \"shared/company.ringi\"\n\nform\n  check_by : choice \"Checked by\"\n    desk   \"Desk\"\n    post   \"Post\"\n    portal \"Portal\"\n  option   : choice \"Option\"\n    plain \"Plain\"\n    rush  \"Rush\"\n  fit      : choice \"Fit\"\n    x_l \"XL\"\n    xL  \"xL\"\n  x_y    : text \"Note on the items\"  count <=40  optional\n  x      : rows \"Items\"  count >=1 <=3\n    y : text \"What\"  count <=40  optional\n  a      : rows \"Parts\"  count >=1 <=3\n    b_c : number \"Count\"  range >=1 <=9\n  a_b    : rows \"Boxes\"  count >=1 <=3\n    c : number \"Width\"  range >=1 <=9\n  vals_x : rows \"Values\"  count >=1 <=3\n    q : number \"Weight\"  range >=1 <=9\n  note   : text \"Note\"  count <=40  optional  shown when option is rush and fit is xL\n\nroute\n| check_by | -> check | decide         |\n| desk     | check    | head_office    |\n| post     | manager  | check_approver |\n| portal   | manager  | headOffice     |\n\nwho\n  check          = team accounting\n  check_approver = team finance\n  manager        = manager of requester\n  head_office    = team general_affairs\n  headOffice     = team legal\n\nevery stage\n  when empty escalate, then skip\n\nticket\n  self approval  not allowed\n  same approver approves once\n";

/// Each name of [`CLASH`] that would be another's is given one of its own: the stage's enum is
/// `check_by_value`, the choice's `option_value`, the value `xL` is `xL_value`, the way `headOffice`
/// the value `headOffice_approver`, the way `check` the value `check_approver2` (its
/// `check_approver` being the other way's), and the glue's second `val_x_y`, `cell_a_b_c` and
/// `row_vals_x` have `_value` after them. `check` and `api` say none of them. The code generated
/// from it builds in the four languages and answers as the reference does (ritsu's
/// `names_the_languages_read_otherwise_are_lowered_under_others`).
#[test]
fn names_made_of_two_names_are_kept_apart() {
    let path = "examples/clash.ringi";
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(Path::new(path)), CLASH);
    ritsu_base::fs::with(std::rc::Rc::new(files), || {
        let o = run().text(path, CLASH);
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        let has = |file: &str, lines: &[&str]| {
            let text = lowered_text(&o, file);
            for l in lines {
                assert!(text.contains(l), "{l:?} is not in {file}:\n{text}");
            }
        };
        has("choices.rule", &["enum check_by = desk \"Desk\" | post \"Post\" | portal \"Portal\"\n", "enum option_value = plain \"Plain\" | rush \"Rush\"\n", "enum fit = x_l \"XL\" | xL_value \"xL\"\n"]);
        has(
            "route.rule",
            &[
                "enum check_by_value = check_approver2 \"check\" | manager \"manager\"\n",
                "enum decide_by = head_office \"head_office\" | check_approver \"check_approver\" | headOffice_approver \"headOffice\"\n",
                "| desk     | check_approver2 | head_office         |\n",
            ],
        );
        let engine = ringi::ports::Engine::new(common::joined());
        let glue = engine.glue(Path::new(path), Path::new("examples/.lowered/ticket.kn")).unwrap_or_else(|t| panic!("{}", t.en));
        for name in ["val_x_y", "cell_a_b_c", "row_vals_x"] {
            for f in [format!("fn {name}("), format!("fn {name}_value(")] {
                assert!(glue.text.contains(&f), "{f:?} is not in the glue:\n{}", glue.text);
            }
        }
        let ask = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
            (code, String::from_utf8(out).unwrap() + &String::from_utf8(err).unwrap())
        };
        let (code, said) = ask(&["check", path, "--format", "json", "--root", "."]);
        assert!(code == 0 && said.contains("\"ok\": true"), "{said}");
        let (code, api) = ask(&["api", path, "--root", "."]);
        assert!(code == 0 && api.contains("\"name\": \"clash\""), "{api}");
        for given in ["check_by_value", "option_value", "xL_value", "headOffice_approver", "check_approver2"] {
            assert!(!said.contains(given) && !api.contains(given), "{given}:\n{said}\n{api}");
        }
    });
}

/// A request whose calendars and rules, joined into the names the glue declares for a calendar's
/// days, come to the same names: a calendar named `after` (whose business days would be counted by
/// `open_after`, the glue's own function), rules read by `open_office` and `opened_office` (the
/// names of the function and of the file of the open days of the calendar `office`), and by
/// `terms_1` and `open_1` (the files of the deadlines and of the business days of the calendar
/// read second: `after` for the deadlines, `office` for the business days, as `resubmit` is written
/// before `pause`). The amount the rules read may be left empty, so the route reads their outputs
/// as inputs the glue gives it (`fed by`), not as values it works out with `apply` (DESIGN 6.4).
const DAYS: &str = "request days v1 \"Names made of a calendar's name\"\n\nuse organization people from \"shared/company.ringi\"\nuse calendar office from \"shared/office.cal\"\nuse calendar after from \"shared/office.cal\"\nuse rule open_office from \"shared/authority.rule\"\nuse rule opened_office from \"shared/authority.rule\"\nuse rule terms_1 from \"shared/authority.rule\"\nuse rule open_1 from \"shared/authority.rule\"\n\nform\n  kind   : choice from open_office.kind \"Kind\"\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000_000JPY  optional\n  start  : date \"Start\"  range >=2026-04-01 <=2027-03-31  open in office\n\nroute\n  first    manager\n  finance  manager  when open_office.finance\n  board    manager  when opened_office.board\n  check    manager  when terms_1.finance\n    due in 2 business days in after at end of day\n    when late expire\n  last     manager  when open_1.board\n\nwho\n  manager = manager of requester\n\nevery stage\n  due in 3 business days in office at end of day\n  when late escalate, then expire\n  when empty escalate, then skip\n\nticket\n  self approval  not allowed\n  send back      to the requester, at most 2 times\n  resubmit       within 5 business days in after\n  pause          at most 5 business days in office\n";

/// Each name the glue makes of a calendar's name in [`DAYS`] that would be another's is given one of
/// its own: the rules keep the aliases the request reads them by, and the glue's `open_after_value`,
/// `open_office_value`, `opened_office_value`, `terms_1_value` and `open_1_value` have `_value` after
/// them. The code generated from it builds in the four languages and answers as the reference does
/// (ritsu's `names_the_languages_read_otherwise_are_lowered_under_others`).
#[test]
fn names_made_of_a_calendars_name_are_kept_apart() {
    let path = "examples/days.ringi";
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(Path::new(path)), DAYS);
    ritsu_base::fs::with(std::rc::Rc::new(files), || {
        let o = run().text(path, DAYS);
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        let engine = ringi::ports::Engine::new(common::joined());
        let glue = engine.glue(Path::new(path), Path::new("examples/.lowered/ticket.kn")).unwrap_or_else(|t| panic!("{}", t.en));
        for line in [
            "use rule open_office from \"../shared/authority.rule\"\n",
            "use rule opened_office from \"../shared/authority.rule\"\n",
            "use rule terms_1 from \"../shared/authority.rule\"\n",
            "use rule open_1 from \"../shared/authority.rule\"\n",
            "use dates opened_office_value from \"opened_office.cal\"\n",
            "use dates terms_1_value from \"terms_after.cal\"\n",
            "use dates open_1_value from \"open_office.cal\"\n",
            "fn open_office_value(day: date, n: number)",
            "fn open_after_value(day: date, n: number)",
            "fn open_after(key: string, day: date, n: number)",
        ] {
            assert!(glue.text.contains(line), "{line:?} is not in the glue:\n{}", glue.text);
        }
    });
}

/// A request named in capitals, with a calendar, fields and a stage named so: the names of its
/// lowered dates files, of their inputs and of their dates, which koyomi takes only in small letters
/// (`[a-z][a-z0-9_]*`, its E010).
const CAPS: &str = "request capsDays v1 \"Names in capitals\"\n\nuse organization people from \"shared/company.ringi\"\nuse calendar mainOffice from \"shared/office.cal\"\n\nform\n  firstDay : date \"First day\"  range >=2026-04-01 <=2027-03-31  open in mainOffice\n  nDays    : number \"Business days\"  range >=1 <=20\n  backOn   : date \"Back on\" = firstDay + nDays business days in mainOffice\n\nroute\n  firstB   manager\n    due in 2 business days at end of day\n    when late expire\n  secondB  manager  when nDays > 5\n\nwho\n  manager = manager of requester\n\nevery stage\n  due in 3 business days at end of day\n  when late escalate, then expire\n  when empty escalate, then skip\n\nticket\n  self approval  not allowed\n  pause          at most 5 business days\n";

/// [`CAPS`] passes the check, and each name koyomi does not take as it is is lowered so that it
/// does: the dates files are named with the name in small letters in brackets
/// (`capsDays_form(capsdays_form)`), and their inputs and dates in small letters (`firstday`,
/// `backon`, `firstb_due`). The code generated from it builds in the four languages and answers as
/// the reference does (ritsu's `names_the_languages_read_otherwise_are_lowered_under_others`).
#[test]
fn names_in_capitals_are_lowered_as_koyomi_takes_them() {
    let path = "examples/caps.ringi";
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(Path::new(path)), CAPS);
    ritsu_base::fs::with(std::rc::Rc::new(files), || {
        let o = run().text(path, CAPS);
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        let has = |text: &str, file: &str, lines: &[&str]| {
            for l in lines {
                assert!(text.contains(l), "{l:?} is not in {file}:\n{text}");
            }
        };
        has(&lowered_text(&o, "form.cal"), "form.cal", &["dates capsDays_form(capsdays_form) v1\n", "  firstday : date", "  ndays : int", "date backon = firstday\n  + ndays business days\n"]);
        has(&lowered_text(&o, "terms.cal"), "terms.cal", &["dates capsDays_terms(capsdays_terms) v1\n", "date firstb_due = started\n"]);
        let engine = ringi::ports::Engine::new(common::joined());
        let glue = engine.glue(Path::new(path), Path::new("examples/.lowered/ticket.kn")).unwrap_or_else(|t| panic!("{}", t.en));
        let file = |name: &str| glue.files.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone()).unwrap_or_else(|| panic!("the glue writes no {name}"));
        has(&file("open.cal"), "open.cal", &["dates capsDays_open(capsdays_open) v1\n"]);
        has(&file("opened_mainOffice.cal"), "opened_mainOffice.cal", &["dates capsDays_opened_mainOffice(capsdays_opened_mainoffice) v1\n"]);
        has(&glue.text, "the glue", &["module capsDays v1\n", "use dates opened_mainOffice from \"opened_mainOffice.cal\"\n", "fn open_mainOffice(day: date, n: number)"]);
    });
}

/// A request whose names start with `_`, end with `_`, or have two `_` in a row: a choice and its
/// values, computed fields, rows and their fields, dates, a field shown on a condition, stages,
/// ways to choose an approver, a named table and the alias of a rule. ritsu's
/// `names_the_languages_read_otherwise_are_lowered_under_others` generates its code in the four
/// languages and drives it.
const UNDER: &str = r#"request under v1 "Names with underscores"

use organization people from "shared/company.ringi"
use calendar office from "shared/office.cal"
use rule _auth from "shared/authority.rule"

form
  _level      : choice "Level"
    _low     "Low"
    high_    "High"
    mid__dle "Middle"
  kind        : choice from _auth.kind "Kind of purchase"
  amount      : money[JPY] "Amount"  range >=1JPY <=100_000_000JPY
  _count      : number "Count"  range >=1 <=9
  _twice      : number "Twice" = _count * 2  range >=2 <=18
  _next       : number "Next" = _count + 1  range >=2 <=10
  _items      : rows "Items"  count >=1 <=5
    _points : number "Points"  range >=1 <=9
    note_   : text "Note"  count <=10  optional
  _sum        : number "Points in all" = sum(_items._points)  range >=1 <=45
  start__on   : date "Start"  range >=2026-04-01 <=2027-03-31  open in office
  _span       : number "Business days"  range >=1 <=20
  back_on_    : date "Back on" = start__on + _span business days in office
  _remark_    : text "Remark"  count <=10  optional  shown when _level is _low
  other__note : text "Other note"  count <=10  optional

route
  first       _boss
  check__two  _boss  when _twice > 10
    due in 2 business days at end of day
    when late expire
  _third      _boss  when _sum > 5
    due in 4 business days at 17:00
    when late expire
  finance     _boss  when _auth.finance

who
  _boss = manager of requester
  _head = head of requester.division

every stage
  due in 3 business days at end of day
  when late escalate by _climb, then expire
  when empty escalate, then skip

table _climb
| approver | -> goes_to |
| _boss    | _head      |
| _head    | _head      |

ticket
  self approval  not allowed
  pause          at most 5 business days
"#;

/// [`UNDER`] passes the check, and each name a language takes only under another is lowered under
/// one it takes (DESIGN 6.4): in the files of rulec's, a name that starts with `_` has `n` before it
/// (the code rulec generates for Python holds an output and an element's field in a `NamedTuple`,
/// which takes no field that starts with `_`); in the files of koyomi's, it has `n` before it too,
/// and two `_` in a row are one (`start__on` is `start_on`), which rustc would warn of. What
/// `check`, `simulate` and `api` say keeps the request's names.
#[test]
fn names_with_underscores_are_lowered_as_each_language_takes_them() {
    let path = "examples/under.ringi";
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(Path::new(path)), UNDER);
    ritsu_base::fs::with(std::rc::Rc::new(files), || {
        let o = run().text(path, UNDER);
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        let has = |file: &str, lines: &[&str]| {
            let text = lowered_text(&o, file);
            for l in lines {
                assert!(text.contains(l), "{l:?} is not in {file}:\n{text}");
            }
        };
        has("choices.rule", &["enum n_level = n_low \"Low\" | high_ \"High\" | mid__dle \"Middle\"\n"]);
        has("form.rule", &["  n_count : number  range >=1 <=9\n", "elements n_items\n  n_points : number  range >=1 <=9\n", "  n_twice ", "  n_remark__shown : bool\n"]);
        // the route reads one output of the rule `_auth` of three: an input fed by it, as the code
        // rulec writes of a rule that applies another works out every output of it (DESIGN 6.4)
        has("route.rule", &["enum n_third_by = n_boss \"_boss\"\n", "  n_auth_finance : bool  fed by rulec \"../shared/authority.rule\" output finance\n", "| >5    | n_boss     |\n"]);
        assert!(!lowered_text(&o, "route.rule").contains("\napply "), "{}", lowered_text(&o, "route.rule"));
        has("n_climb.rule", &["table n_climb\n", "| n_boss   | n_head     |\n"]);
        has("form.cal", &["  start_on : date  range >=2026-04-01 <=2027-03-31\n", "  n_span : int  range >=1 <=20\n", "date back_on_ = start_on\n  + n_span business days\n"]);
        has("terms.cal", &["date check_two_due = started\n", "date n_third_due = started\n"]);
        let ask = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
            (code, String::from_utf8(out).unwrap() + &String::from_utf8(err).unwrap())
        };
        let (code, said) = ask(&["check", path, "--format", "json", "--root", "."]);
        assert!(code == 0 && said.contains("\"ok\": true"), "{said}");
        let (code, api) = ask(&["api", path, "--root", "."]);
        assert!(code == 0 && api.contains("\"name\": \"under\""), "{api}");
        let form = "_level = _low, kind = goods, amount = 2_000_000JPY, _count = 6, _items = [{_points: 3}, {_points: 4}], start__on = 2026-11-16, _span = 2, _remark_ = \"low\"";
        let (code, simulated) = ask(&["simulate", path, "--requester", "e1201", "--form", form, "--on", "2026-11-16", "--format", "json"]);
        assert!(code == 0 && simulated.contains("\"name\": \"_twice\"") && simulated.contains("\"stage\": \"_third\""), "{simulated}");
        // a name the lowered files give, as a word of its own (`\n_low` in a JSON string is the
        // value `_low` after a line's end)
        let says = |text: &str, name: &str| {
            text.match_indices(name).any(|(i, _)| {
                let before = text[..i].chars().next_back().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\'));
                let after = text[i + name.len()..].chars().next().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
                before && after
            })
        };
        for given in ["n_level", "n_low", "n_twice", "n_items", "n_points", "n_remark_", "n_remark__shown", "n_boss", "n_head", "n_climb", "n_third", "n_auth_finance", "start_on", "check_two", "n_span"] {
            assert!(!says(&said, given) && !says(&api, given) && !says(&simulated, given), "{given}:\n{said}\n{api}\n{simulated}");
        }
    });
}

/// A request whose calendars are named so that the files of their days, in the code generated from
/// them, would be one module or one package: `ab`, `aB` and `a_b` (one name in small letters with
/// no `_`), `x` and `ed_x` (`opened_x` and `open_ed_x` are one package of Go's), one that starts
/// with `_`, one that ends with `_`, and one with two `_` in a row.
const CALS: &str = r#"request cals v1 "Calendars named alike"

use organization people from "shared/company.ringi"
use calendar ab from "shared/office.cal"
use calendar aB from "shared/office.cal"
use calendar a_b from "shared/office.cal"
use calendar x from "shared/office.cal"
use calendar ed_x from "shared/office.cal"
use calendar _office from "shared/office.cal"
use calendar office_ from "shared/office.cal"
use calendar c__d from "shared/office.cal"

form
  day1 : date "Day 1"  range >=2026-04-01 <=2027-03-31  open in ab
  day2 : date "Day 2"  range >=2026-04-01 <=2027-03-31  open in aB
  day3 : date "Day 3"  range >=2026-04-01 <=2027-03-31  open in a_b
  day4 : date "Day 4"  range >=2026-04-01 <=2027-03-31  open in x
  day5 : date "Day 5"  range >=2026-04-01 <=2027-03-31  open in _office
  day6 : date "Day 6"  range >=2026-04-01 <=2027-03-31  open in c__d
  span : number "Business days"  range >=1 <=20
  back : date "Back" = day1 + span business days in office_

route
  first   manager
  second  manager  when day1 > 2026-12-01
    due in 2 business days in aB at end of day
    when late expire
  third   manager  when day2 > 2026-12-01
    due in 2 business days in a_b at end of day
    when late expire
  fourth  manager  when day3 > 2026-12-01
    due in 2 business days in office_ at end of day
    when late expire
  fifth   manager  when day4 > 2026-12-01
    due in 2 business days in _office at end of day
    when late expire

who
  manager = manager of requester

every stage
  due in 3 business days in ab at end of day
  when late escalate, then expire
  when empty escalate, then skip

ticket
  self approval  not allowed
  send back      to the requester, at most 2 times
  pause          at most 5 business days in x
  resubmit       within 5 business days in ed_x
"#;

/// [`CALS`] passes the check, and the files of its calendars' days are named by a word of each
/// calendar's (DESIGN 6.4) so that no two are one module or one package in any language the code
/// is generated in: in small letters with no `_`, as Go names a package, every file the request is
/// lowered to has a name of its own. A calendar that starts with `_` has `n` before its word, and
/// two `_` in a row are one. ritsu's `names_the_languages_read_otherwise_are_lowered_under_others`
/// generates its code in the four languages and drives it.
#[test]
fn a_calendars_files_are_named_apart_in_every_language() {
    let path = "examples/cals.ringi";
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(Path::new(path)), CALS);
    ritsu_base::fs::with(std::rc::Rc::new(files), || {
        let o = run().text(path, CALS);
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        let engine = ringi::ports::Engine::new(common::joined());
        let glue = engine.glue(Path::new(path), Path::new("examples/.lowered/ticket.kn")).unwrap_or_else(|t| panic!("{}", t.en));
        let mut names: Vec<String> = o.lowered.as_ref().unwrap().files.iter().map(|f| f.name.clone()).chain(glue.files.iter().map(|(n, _)| n.clone())).filter(|n| n.ends_with(".cal")).collect();
        names.sort();
        for name in [
            "terms.cal", "terms_aB_2.cal", "terms_a_b_3.cal", "terms_office_.cal", "terms_n_office.cal", "open.cal", "open_ed_x_2.cal",
            "opened_ab.cal", "opened_aB_2.cal", "opened_a_b_3.cal", "opened_x.cal", "opened_n_office.cal", "opened_c_d.cal",
        ] {
            assert!(names.iter().any(|n| n == name), "{name} is not among {names:?}");
        }
        let go = |n: &str| n.trim_end_matches(".cal").chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect::<String>();
        let mut spelled: Vec<String> = names.iter().map(|n| go(n)).collect();
        spelled.sort();
        spelled.dedup();
        assert_eq!(spelled.len(), names.len(), "two files of {names:?} are one package of Go's");
        for line in ["use dates opened_x from \"opened_x.cal\"\n", "use dates open_1 from \"open_ed_x_2.cal\"\n", "use dates opened_aB_2 from \"opened_aB_2.cal\"\n", "fn open_ed_x_2(day: date, n: number)", "fn open_x(day: date, n: number)"] {
            assert!(glue.text.contains(line), "{line:?} is not in the glue:\n{}", glue.text);
        }
    });
}

/// A condition, or a column of the route's table, that reads a rule's output that may be none
/// (`decider : decider?`) reads `none` as a value of its own (DESIGN 4.9): the column may be none,
/// `is cfo` does not hold where it is, a table that writes no row for it leaves those requests to
/// no row (E301, with the row to add), and a row `none` takes them.
#[test]
fn a_rules_output_that_may_be_none_is_read_as_none() {
    let uses = "use rule authority from \"shared/optional_decider.rule\"\n\n";
    let who = "\n\nwho\n  managers = team managers\n  cfos     = team cfos";
    // a stage's condition
    let o = common::check_text("tests/mutants/optional.ringi", &route_request(false, uses, "", &format!("\n  check  managers\n  audit  cfos  when authority.decider is cfo{who}"), ""));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let route = lowered_text(&o, "route.rule");
    for line in ["apply authority = \"../shared/optional_decider.rule\" meaning:", "  decider -> authority_decider\n", "| not: cfo          | none     |", "| cfo               | cfos     |"] {
        assert!(route.contains(line), "{line:?} is not in:\n{route}");
    }
    // a table that writes no row for none, then one that does
    let table = "\n| authority.decider | -> check | audit |\n| manager           | managers |       |\n| cfo               | managers | cfos  |";
    let o = common::check_text("tests/mutants/optional.ringi", &route_request(false, uses, "", &format!("{table}{who}"), ""));
    assert_eq!(common::codes(&o), ["E301"], "{}", common::shown(&o, Lang::En));
    let shown = common::shown(&o, Lang::En);
    assert!(shown.contains("A request no row takes: authority.decider = none") && shown.contains("| none | ? | ? |"), "{shown}");
    let o = common::check_text("tests/mutants/optional.ringi", &route_request(false, uses, "", &format!("{table}\n| none              | managers |       |{who}"), ""));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
}

/// The text of the lowered file `name` of an outcome.
fn lowered_text(o: &ringi::check::Outcome, name: &str) -> String {
    o.lowered.as_ref().and_then(|l| l.files.iter().find(|f| f.name == name)).map(|f| f.text.clone()).unwrap_or_else(|| panic!("no {name}"))
}

/// A request with a fee of 3% of an amount (DESIGN 4.1, 4.4), the route as given.
fn fee_request(fee_range: &str, conditions: &str, route: &str) -> String {
    format!(
        "request example v1 \"Example\"\n\nform\n  amount  : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY\n  fee     : money[JPY] \"Fee\" = amount * 3%  round down(1JPY){fee_range}\n{conditions}\nroute\n{route}\nwho\n  finance = team finance\n\nticket\n  self approval  not allowed\n"
    )
}

/// A rate is the share it is: `amount * 3%` is lowered as it is written, and what the fee reaches
/// is 3% of the amount's range (the route reads it as `>=0JPY <=30_000JPY`).
#[test]
fn a_rate_is_lowered_as_the_share_it_is() {
    let src = fee_request("", "", "| fee | -> check |\n| -   | finance  |\n");
    let o = common::check_text("tests/mutants/rate.ringi", &src);
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let form = lowered_text(&o, "form.rule");
    assert!(form.contains("define fee : money[JPY] = down(amount * 3%, 1JPY)\n"), "{form}");
    let route = lowered_text(&o, "route.rule");
    assert!(route.contains("  fee : money[JPY]  range >=0JPY <=30_000JPY\n"), "{route}");
}

/// A computed field is read with what its computation reaches, within the range it writes: a range
/// wider than the fee reaches costs nothing, and a route that takes every fee the fee comes to
/// passes (DESIGN 4.1).
#[test]
fn a_computed_field_is_read_within_what_it_reaches() {
    let src = fee_request("  range >=0JPY <=1_000_000JPY", "", "| fee          | -> check |\n| <=30_000JPY  | finance  |\n");
    let o = common::check_text("tests/mutants/wide.ringi", &src);
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let route = lowered_text(&o, "route.rule");
    assert!(route.contains("  fee : money[JPY]  range >=0JPY <=30_000JPY\n"), "{route}");
    // a range narrower than the fee reaches is the part the route reads
    let src = fee_request("  range >=0JPY <=1_000JPY", "", "| fee        | -> check |\n| <=1_000JPY | finance  |\n");
    let o = common::check_text("tests/mutants/narrow.ringi", &src);
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let route = lowered_text(&o, "route.rule");
    assert!(route.contains("  fee : money[JPY]  range >=0JPY <=1000JPY\n"), "{route}");
}

/// The question of E105 keeps how a computation follows from what it reads: a fee over 900 yen is
/// an amount over 30,000 yen, so a receipt asked for over that fee and shown over that amount is
/// never hidden, and never said to be (no E105 with an example that cannot happen). rulec reads the
/// fee and the amount together, and shows that no request reaches the row of the question where the
/// receipt is asked for and hidden: the row is left out, and the question comes to no request
/// (DESIGN 4.4, 6.4). Written over the amount alone, it is settled the same. Shown over 40,000 yen
/// the receipt is hidden, and the example says the fee the amount comes to. A fee that writes a
/// range is asked about within it.
#[test]
fn the_question_of_e105_keeps_how_a_computation_follows() {
    let route = "| -> check |\n| finance  |\n";
    let receipt = |shown: &str| format!("  receipt : attachment \"Receipt\"  required when fee > 900JPY  shown when {shown}\n");
    let o = common::check_text("tests/mutants/fee.ringi", &fee_request("", &receipt("amount > 30_000JPY"), route));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let question = lowered_text(&o, "receipt.hidden.rule");
    assert!(question.contains("define fee : money[JPY] = down(amount * 3%, 1JPY)\n") && !question.contains("| true"), "{question}");
    let same = "  receipt : attachment \"Receipt\"  required when amount > 30_033JPY  shown when amount > 30_000JPY\n";
    let o = common::check_text("tests/mutants/fee.ringi", &fee_request("", same, route));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let o = common::check_text("tests/mutants/fee.ringi", &fee_request("", &receipt("amount > 40_000JPY"), route));
    let d = o.diags.iter().find(|d| d.code == "E105").unwrap_or_else(|| panic!("{}", common::shown(&o, Lang::En)));
    let ex = d.notes.first().map(|n| n.en.clone()).unwrap_or_default();
    let fee: i64 = ex.rsplit("fee = ").next().unwrap().trim_end_matches("JPY").replace('_', "").parse().unwrap_or_else(|_| panic!("{ex}"));
    let amount: i64 = ex.trim_start_matches("Such a request: amount = ").split("JPY").next().unwrap().replace('_', "").parse().unwrap_or_else(|_| panic!("{ex}"));
    assert!(ex.starts_with("Such a request: amount = ") && fee == amount * 3 / 100 && fee > 900 && amount <= 40_000, "{ex}");
    assert!(d.notes[0].ja.starts_with("その申請: amount = ") && d.notes[0].ja.contains("、fee = "), "{}", d.notes[0].ja);
    // within the range the fee writes
    let shown = "  category : choice \"Category\"\n    travel  \"Travel\"\n    expense \"Expenses\"\n  receipt : attachment \"Receipt\"  required when fee > 900JPY  shown when category is travel\n";
    let o = common::check_text("tests/mutants/fee.ringi", &fee_request("  range >=0JPY <=1_000JPY", shown, route));
    let d = o.diags.iter().find(|d| d.code == "E105").unwrap_or_else(|| panic!("{}", common::shown(&o, Lang::En)));
    let ex = d.notes.first().map(|n| n.en.clone()).unwrap_or_default();
    let fee: i64 = ex.split("fee = ").nth(1).unwrap().split("JPY").next().unwrap().replace('_', "").parse().unwrap_or_else(|_| panic!("{ex}"));
    assert!((901..=1_000).contains(&fee), "{ex}");
}

/// A row no request reaches is left out of a table ringi cuts a condition into, and the rest is
/// asked (DESIGN 6.4): a receipt asked for over a fee over 900 yen or an amount over 500,000 yen, and
/// shown between 30,000 and 600,000 yen, is never hidden over the fee (that fee is an amount over
/// 30,000 yen), and is hidden from 600,000 yen; the question says so, with that example, where it
/// was E201 while the row of the fee was in it. A `shown when` whose table has such a row is lowered
/// without it, in `form.rule` and the copy the generated code reads, and passes: the receipt is
/// shown for travel, and over the fee only where the amount is up to 30,000 yen, which no request
/// is. (A row of a table the request writes is the request's, and is said: E303.)
#[test]
fn a_row_no_request_reaches_is_left_out_of_a_condition() {
    let route = "| -> check |\n| finance  |\n";
    let mixed = "  receipt : attachment \"Receipt\"  required when fee > 900JPY or amount > 500_000JPY  shown when amount > 30_000JPY and amount < 600_000JPY\n";
    let o = common::check_text("tests/mutants/fee.ringi", &fee_request("", mixed, route));
    let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["E105"], "{}", common::shown(&o, Lang::En));
    let ex = o.diags[0].notes.first().map(|n| n.en.clone()).unwrap_or_default();
    let amount: i64 = ex.trim_start_matches("Such a request: amount = ").split("JPY").next().unwrap().replace('_', "").parse().unwrap_or_else(|_| panic!("{ex}"));
    assert!(amount >= 600_000, "{ex}");
    let question = lowered_text(&o, "receipt.hidden.rule");
    assert_eq!(question.lines().filter(|l| l.contains("| true")).count(), 1, "{question}");
    let shown = "  category : choice \"Category\"\n    travel  \"Travel\"\n    expense \"Expenses\"\n  receipt : attachment \"Receipt\"  shown when category is travel or (fee > 900JPY and amount <= 30_000JPY)\n";
    let o = common::check_text("tests/mutants/fee.ringi", &fee_request("", shown, route));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    for file in ["form.rule", "values.rule"] {
        let t = lowered_text(&o, file);
        let table: Vec<&str> = t.lines().skip_while(|l| !l.starts_with("table receipt_shown")).skip(2).collect();
        assert_eq!(table.len(), 4, "{t}");
        assert!(table.iter().all(|l| !(l.contains(">900JPY") && l.contains("<=30_000JPY"))), "{t}");
    }
}

/// A rate times a number is a rate, whichever is written first, as rulec types it: a computed rate
/// may double another (`share * 2`), and an amount may be worked out from it.
#[test]
fn a_rate_times_a_number_is_a_rate() {
    let form = "  share  : rate[step 1%] \"Share\"  range >=0% <=50%\n  double : rate[step 1%] \"Double\" = share * 2  range >=0% <=100%\n  twice  : rate[step 1%] \"Twice\" = 2 * share  range >=0% <=100%\n  part   : money[JPY] \"Part\" = amount * double  round down(1JPY)\n";
    let route = "\n  check  finance\n\nwho\n  finance = team finance";
    let o = common::check_text("tests/mutants/rates.ringi", &route_request(false, "", form, route, ""));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let lowered = lowered_text(&o, "form.rule");
    for line in ["define double : rate[step 1%] = share * 2  range >=0% <=100%\n", "define twice : rate[step 1%] = 2 * share  range >=0% <=100%\n", "define part : money[JPY] = down(amount * double, 1JPY)\n"] {
        assert!(lowered.contains(line), "{line:?} is not in\n{lowered}");
    }
}

/// A temperature, whose units `℃` and `℉` Unicode files as symbols, is written in a type's brackets
/// as in a number (`temperature[℃]`, `30℃`), as rulec and kaname read it, and lowered as it is.
#[test]
fn a_temperature_is_written_in_degrees() {
    for (unit, top) in [("℃", "40℃"), ("℉", "104℉")] {
        let form = format!("  heat   : temperature[{unit}] \"Heat\"  range >=0{unit} <={top}\n");
        let route = format!("\n  check  finance\n  audit  board  when heat > 30{unit}\n\nwho\n  finance = team finance\n  board   = team board");
        let o = common::check_text("tests/mutants/degrees.ringi", &route_request(false, "", &form, &route, ""));
        assert!(o.diags.is_empty(), "{unit}: {}", common::shown(&o, Lang::En));
        let lowered = lowered_text(&o, "route.rule");
        assert!(lowered.contains(&format!("  heat : temperature[{unit}]  range >=0{unit} <={top}\n")) && lowered.contains(&format!("| >30{unit}")), "{lowered}");
    }
}

/// A number written in another unit of a field's dimension is lowered as the same amount in the
/// field's unit: the ends of its range, a value a condition compares it with, a cell of the route's
/// table and of the examples, the step it rounds to (`500_000g` for a field of `kg` is `500kg`, not
/// `500_000kg`).
#[test]
fn a_number_in_another_unit_is_lowered_in_the_fields() {
    let request = |route: &str| {
        format!(
            "request example v1 \"Example\"\n\nform\n  weight : mass[kg] \"Weight\"  range >=1kg <=500_000g\n  price  : money[JPY] \"Price\"  range >=1JPY <=1_000JPY\n  half   : money[JPY] \"Half\" = price / 2  round down(100銭)\n\nroute\n{route}\nwho\n  finance = team finance\n  board   = team board\n\nticket\n  self approval  not allowed\n\nexamples\n| weight   | -> check | audit |\n| 20_000g  | finance  |       |\n| 150_000g | finance  | board |\n"
        )
    };
    // a condition
    let o = common::check_text("tests/mutants/units.ringi", &request("  check  finance\n  audit  board  when weight > 100_000g\n"));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let route = lowered_text(&o, "route.rule");
    for line in ["  weight : mass[kg]  range >=1kg <=500kg\n", "| >100kg  | board", "| 20kg   | finance", "| 150kg  | finance"] {
        assert!(route.contains(line), "{line:?} is not in\n{route}");
    }
    let form = lowered_text(&o, "form.rule");
    assert!(form.contains("down(price / 2, 1JPY)"), "{form}");
    // a cell of the route's table
    let o = common::check_text("tests/mutants/units.ringi", &request("| weight     | -> check | audit |\n| <=100_000g | finance  |       |\n| >100_000g  | finance  | board |\n"));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let route = lowered_text(&o, "route.rule");
    for line in ["| <=100kg | finance", "| >100kg  | finance"] {
        assert!(route.contains(line), "{line:?} is not in\n{route}");
    }
}

/// A table that chooses the shared route (`-> route`) is lowered as a table of one output, whose
/// values are the routes' names, so that rulec checks it as any route (DESIGN 5.3): a gap is E301
/// (`tests/mutants/E301_a_table_of_routes_with_a_gap.ringi`).
#[test]
fn a_table_that_chooses_the_route_is_lowered() {
    let src = "request example v1 \"Example\"\n\nuse route simple from \"shared/simple.ringi\"\nuse route strict from \"shared/strict.ringi\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY\n\nroute\n| amount       | -> route |\n| <=100_000JPY | simple   |\n| >100_000JPY  | strict   |\n\nticket\n  self approval  not allowed\n";
    let o = common::check_text("tests/mutants/routes.ringi", src);
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let route = lowered_text(&o, "route.rule");
    for line in ["enum route_by = simple \"simple\" | strict \"strict\"\n", "  route : route_by\n", "| >100_000JPY  | strict   |\n"] {
        assert!(route.contains(line), "{route}");
    }
}

/// The words ringi keeps the lowered names out of the way of are the words rulec, koyomi and kaname
/// reserve, with the types rulec reads and the names of kaname's helpers, copied (ringi does not
/// depend on them): a word any of them adds is kept out of the way too.
#[test]
fn the_words_kept_out_of_the_way_are_the_languages_own() {
    use std::collections::BTreeSet;
    let set = |w: &[&'static str]| w.iter().copied().collect::<BTreeSet<&str>>();
    assert_eq!(set(lower::words::RULEC_RESERVED), set(rulec::kw::RESERVED));
    assert_eq!(set(lower::words::RULEC_VALUE_RESERVED), set(rulec::kw::VALUE_RESERVED));
    assert_eq!(set(lower::words::KOYOMI_RESERVED), set(koyomi::kw::RESERVED));
    use rulec::kw as r;
    let types = [r::MONEY, r::MASS, r::LENGTH, r::AREA, r::VOLUME, r::DURATION, r::TEMPERATURE, r::SOUND, r::RATE, r::NUMBER, r::BOOL, r::STRING, r::DATE, r::INCL_TAX, r::EXCL_TAX];
    assert_eq!(set(lower::words::RULEC_TYPES), set(&types));
    assert_eq!(set(lower::words::KANAME_RESERVED), set(&kaname::kw::RESERVED));
    assert_eq!(set(lower::words::KANAME_HELPERS), set(&kaname::kw::HELPERS));
    // a name asked for that is one of them is given another
    let mut r = lower::words::Names::rulec();
    assert_eq!(r.take("next"), "next_value");
    assert_eq!(r.take("next"), "next_value2");
    let mut k = lower::words::Names::koyomi();
    assert_eq!(k.take("due"), "due");
    assert_eq!(k.take("start"), "start_value");
    let mut e = lower::words::Names::enums();
    assert_eq!(e.take("date"), "date_value");
    let mut v = lower::words::Names::values();
    assert_eq!(v.take("after"), "after_value");
    assert_eq!(v.take("then"), "then");
    let mut g = lower::words::Names::kaname();
    assert_eq!(g.take("match"), "match_value");
}

/// A request with the route given, checked as a file of `tests/mutants/` (whose shared routes it
/// reads), in English names or in Japanese ones.
fn route_request(ja: bool, uses: &str, form: &str, route: &str, after: &str) -> String {
    if ja {
        format!("request 例(example) v1 \"例\"\n\n{uses}form\n  金額(amount) : money[円] \"金額\"  range >=1円 <=100_000円\n  日数(days)   : number \"日数\"  range >=1 <=20\n{form}\nroute{route}\n\nticket\n  self approval  not allowed\n{after}")
    } else {
        format!("request example v1 \"Example\"\n\n{uses}form\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n  days   : number \"Days\"  range >=1 <=20\n{form}\nroute{route}\n\nticket\n  self approval  not allowed\n{after}")
    }
}

/// A value written before the name it is compared with is read the other way round:
/// `50_000JPY < amount` is `amount > 50_000JPY` (DESIGN 5.3).
#[test]
fn a_value_written_first_is_read_the_other_way_round() {
    for (ja, route) in [
        (false, "\n  check  finance\n  audit  board  when 50_000JPY < amount\n\nwho\n  finance = team finance\n  board   = team board"),
        (true, "\n  確かめ(check)  経理\n  監査(audit)    取締役会  when 50_000円 < 金額\n\nwho\n  経理(finance)   = team finance\n  取締役会(board) = team board"),
    ] {
        let o = common::check_text("tests/mutants/flipped.ringi", &route_request(ja, "", "", route, ""));
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        let text = lowered_text(&o, "route.rule");
        let row = |from: &str, then: &str| text.lines().any(|l| l.starts_with(from) && l.trim_end_matches('|').trim_end().ends_with(then));
        assert!(row("| >50_000", "board") && row("| <=50_000", "none"), "{text}");
    }
}

/// A shared route written as a table is lowered with what the request gives it: each row holds on
/// its tests of the inputs, as the request gives them (`long: days > 5`), and the request's table
/// has the cells where it holds (DESIGN 5.11).
#[test]
fn a_shared_route_written_as_a_table_is_lowered_with_what_the_request_gives_it() {
    for (ja, uses, route) in [
        (false, "use route by_table from \"shared/by_table.ringi\"\n\n", " by_table(long: days > 5)"),
        (true, "use route 表で決める(by_table) from \"shared/表で決める.ringi\"\n\n", " 表で決める(長い: 日数 > 5)"),
    ] {
        let o = common::check_text("tests/mutants/shared_table.ringi", &route_request(ja, uses, "", route, ""));
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        let text = lowered_text(&o, "route.rule");
        for line in ["| days | -> check | head  |\n", "| >5   | finance  | heads |\n", "| <=5  | finance  | none  |\n"] {
            assert!(text.contains(line), "{text}");
        }
    }
}

/// An optional input of the shared route the request does not give has no value (DESIGN 4.9): a
/// comparison of it with a value does not hold, so `long or cap > 50_000JPY` is `long`, and the table
/// reads no column of it; given, it is read.
#[test]
fn an_input_the_request_does_not_give_has_no_value() {
    for (ja, uses, route) in [
        (false, "use route capped from \"shared/capped.ringi\"\n\n", " capped(long: days > 5)"),
        (true, "use route 上限つき(capped) from \"shared/上限つき.ringi\"\n\n", " 上限つき(長い: 日数 > 5)"),
    ] {
        let o = common::check_text("tests/mutants/not_given.ringi", &route_request(ja, uses, "", route, ""));
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        let text = lowered_text(&o, "route.rule");
        assert!(!text.contains("cap"), "{text}");
        // given, it is read
        let given = if ja { route.replace(" > 5)", " > 5, 上限: 金額)") } else { route.replace(" > 5)", " > 5, cap: amount)") };
        let o = common::check_text("tests/mutants/not_given.ringi", &route_request(ja, uses, "", &given, ""));
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
        assert!(lowered_text(&o, "route.rule").contains("amount"), "{}", lowered_text(&o, "route.rule"));
    }
}

/// The conditions of the form (`shown when`, `required when`) and of the route's claims (`only
/// when`) are lowered as the route's are: what their tables cannot hold is E309 where it is
/// written, every one of them (DESIGN 5.3).
#[test]
fn the_conditions_of_the_form_and_the_claims_that_cannot_be_read_are_said() {
    for (ja, form, route, after) in [
        (
            false,
            "  place  : text \"Place\"  count <=20  shown when place is \"Osaka\"\n  note   : text \"Note\"  count <=20  required when ticket.round > 1\n",
            "\n  check  finance\n\nwho\n  finance = team finance",
            "\nclaims\n  small : check is finance only when requester.id is \"e01\"\n",
        ),
        (
            true,
            "  場所(place) : text \"場所\"  count <=20  shown when 場所 is \"大阪\"\n  メモ(note)  : text \"メモ\"  count <=20  required when ticket.round > 1\n",
            "\n  確かめ(check)  経理\n\nwho\n  経理(finance) = team finance",
            "\nclaims\n  小さいもの : 確かめ is 経理 only when requester.id is \"e01\"\n",
        ),
    ] {
        let o = common::check_text("tests/mutants/form_conditions.ringi", &route_request(ja, "", form, route, after));
        let shown = common::shown(&o, Lang::En);
        assert_eq!(common::codes(&o), ["E309", "E309", "E309"], "{shown}");
        let lines: Vec<Option<usize>> = o.diags.iter().map(|d| d.line).collect();
        assert_eq!(lines, [Some(6), Some(7), Some(19)], "{shown}");
        assert!(o.diags[0].message.en.starts_with("The form cannot read `"), "{shown}");
        assert!(o.diags[2].message.en.starts_with("The route cannot read `requester.id`"), "{shown}");
    }
}

/// Every request and shared route of the tree that passes its check has its route lowered (DESIGN
/// 5.3): what a route's table cannot hold is E309, and nothing that passes is left with no
/// `route.rule` (the files of `examples/`, the mutants that pass, and the projects of ritsu's tests).
#[test]
fn every_route_that_passes_is_lowered() {
    fn collect(dir: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                collect(&p, out);
            } else if p.extension().is_some_and(|x| x == "ringi") {
                out.push(p.display().to_string());
            }
        }
    }
    let mut files = Vec::new();
    for dir in ["examples", "tests", "../ritsu/tests/projects"] {
        collect(Path::new(dir), &mut files);
    }
    files.sort();
    let (mut checked, mut failures) = (0, Vec::new());
    for path in &files {
        let o = common::check(path);
        let Some(f) = &o.file else { continue };
        if !matches!(f.kind, ringi::ast::FileKind::Request | ringi::ast::FileKind::Route) || f.route.is_none() {
            continue;
        }
        checked += 1;
        if !o.lowered.as_ref().is_some_and(|l| l.files.iter().any(|x| x.part == lower::Part::Route)) {
            failures.push(path.clone());
        }
    }
    assert!(checked >= 40, "{checked} files with a route pass");
    assert!(failures.is_empty(), "{failures:?}");
}

/// koyomi counts the days of a request's deadlines once in a run of the program: the check asks
/// koyomi of the lowered `terms.cal` in both languages and for its facts, and what `run` and
/// `simulate` ask after it (the deadline of a stage, the calendar's days) is answered from the
/// check koyomi keeps, while the files read the same. What is checked is the calendar the request
/// uses and the one lowered file.
#[test]
fn koyomi_checks_the_lowered_dates_once() {
    use ringi::simulate::{Ask, On, simulate};
    let koyomi = koyomi::ports::Engine;
    let before = koyomi.checks();
    let checked = Run::new(common::joined());
    let o = checked.file("examples/expense.ringi").unwrap();
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    assert_eq!(koyomi.checks() - before, 2, "`shared/office.cal` and the lowered `terms.cal`");
    // the same run again, and a new one (as each command makes its own)
    let _ = checked.file("examples/expense.ringi").unwrap();
    let again = Run::with(common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
    let o = again.file("examples/expense.ringi").unwrap();
    // the machine and `simulate` on it
    ringi::engine::with(&o, &again, |m, org| m.s.file.scenarios.iter().for_each(|sc| assert!(ringi::scenario::run(m, org, sc).stop.is_none()))).unwrap();
    let day = ritsu_ports::day_of("2026-11-16").unwrap();
    simulate(&o, &again, &Ask { requester: Some("e1201".to_string()), every: false, form: None, on: On::Day(day) }).unwrap();
    assert_eq!(koyomi.checks() - before, 2, "nothing is counted a second time");
}

/// The copy of a form's rule the generated code reads, `values.rule`, is the rule the check and the
/// reference interpreter read, `form.rule`, but for what the copy is for (DESIGN 4.4, 12.1): its
/// name and description, no range on a computation (`define … range`), a total's range what it can
/// reach, the most and the least of a computation of a row that writes a range, with their outputs;
/// and a condition's table that reads a computed field, which the copy writes over all the values the
/// field can come to. The spaces that line up the names of a block are not compared. Every request
/// of `examples/` and of `tests/machine/` that computes something or asks for a field on a condition
/// has both.
#[test]
fn the_copy_of_the_form_differs_only_where_it_is_for() {
    let mut files = common::example_files();
    let mut machine: Vec<String> = std::fs::read_dir("tests/machine").unwrap().filter_map(|e| e.ok()).map(|e| e.path().display().to_string()).filter(|p| p.ends_with(".ringi")).collect();
    machine.sort();
    files.extend(machine);
    let mut compared = Vec::new();
    for f in files {
        let o = run().file(&f).unwrap();
        let Some(l) = o.lowered.as_ref() else { continue };
        let of = |part: lower::Part| l.files.iter().find(|x| x.part == part).map(|x| x.text.clone());
        let (form, values) = (of(lower::Part::Form), of(lower::Part::Values));
        assert_eq!(form.is_some(), values.is_some(), "{f}: form.rule and values.rule come together");
        let (Some(form), Some(values)) = (form, values) else { continue };
        // the computed fields a table may read
        let s = std::fs::read_to_string(&f).unwrap();
        let computed: Vec<String> = s.lines().filter(|l| l.contains("\" = ")).filter_map(|l| l.split_whitespace().next()).map(|w| w.split('(').nth(1).map(|a| a.trim_end_matches(')').to_string()).unwrap_or_else(|| w.to_string())).collect();
        let extremes: Vec<String> = values.lines().filter(|l| l.starts_with("max ") || l.starts_with("min ")).filter_map(|l| l.split_whitespace().nth(1)).map(str::to_string).collect();
        let norm = |text: &str| -> Vec<String> {
            let mut out = Vec::new();
            let mut skip_table = false;
            for line in text.lines().skip(2) {
                if line.is_empty() {
                    skip_table = false;
                    continue;
                }
                if line.starts_with("table ") {
                    out.push(line.to_string());
                    skip_table = false;
                    continue;
                }
                if line.starts_with('|') {
                    // the header row of a table says what it reads
                    if out.last().is_some_and(|l| l.starts_with("table ")) {
                        skip_table = line.split('|').map(str::trim).any(|c| computed.iter().any(|n| n == c));
                    }
                    if skip_table {
                        continue;
                    }
                }
                if line.starts_with("max ") || line.starts_with("min ") || extremes.iter().any(|e| line.trim_start().starts_with(&format!("{e} "))) {
                    continue;
                }
                let kept = if line.starts_with("define ") || line.starts_with("sum ") { line.split("  range ").next().unwrap() } else { line };
                // the spaces that line up the names of a block (the copy's outputs are more)
                let lead = if kept.starts_with(' ') { "  " } else { "" };
                out.push(format!("{lead}{}", kept.split_whitespace().collect::<Vec<_>>().join(" ")));
            }
            out
        };
        assert_eq!(norm(&form), norm(&values), "{f}:\n{form}\n----\n{values}");
        assert!(values.starts_with("rule ") && values.lines().next().unwrap().ends_with("_values v1"), "{f}: {values}");
        compared.push(f);
    }
    assert!(compared.len() >= 6, "{compared:?}");
}

/// What kaname says of the code ringi lays for a request is said as ringi's E201 at the request's
/// first line (DESIGN 9.2): ringi's bug, with kaname's codes and lines and none of its words, in
/// the request's glue or in the machine of the tickets, a function that is not total too.
#[test]
fn what_kaname_says_of_the_laid_code_is_ringis_e201() {
    use ringi::ports::{Engine, GlueSaid};
    let t = ritsu_testkit::TempDir::new("ringi-glue-said");
    let file = t.write("r.ringi", "request r v1 \"R\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100JPY\n\nroute\n  check  finance\n\nwho\n  finance = team finance\n");
    let file = file.to_string_lossy().into_owned();
    let said = [
        GlueSaid { code: "W501".into(), line: Some(12), machine: false, not_total: false },
        GlueSaid { code: "fails range".into(), line: Some(40), machine: true, not_total: true },
    ];
    let u = Engine::new(common::joined()).glue_refused(t.path(), &file, &said, Lang::En);
    assert_eq!(u.verdict, ritsu_ports::Verdict::Fails);
    let f = u.findings().next().expect("one finding");
    assert_eq!((f.code.as_str(), f.line, f.file.as_deref()), ("E201", Some(1), Some("r.ringi")));
    for line in [
        "The code ringi writes for this request does not pass kaname's check: a bug in ringi",
        "= From kaname: W501 at line 12 of the request's code",
        "= A function at line 40 of the machine of the tickets is not total (fails range)",
        "= Nothing in the request is to be corrected for it: report it as a bug in ringi",
    ] {
        assert!(f.text.contains(line), "{line}:\n{}", f.text);
    }
    assert_eq!(f.json.get("from").and_then(|j| j.as_str()), Some("kaname W501"));
    let ja = Engine::new(common::joined()).glue_refused(t.path(), &file, &said, Lang::Ja);
    let text = &ja.findings().next().unwrap().text;
    assert!(text.contains("ringi の不具合です") && text.contains("kaname の W501（申請のコードの 12 行目）"), "{text}");
}

/// A request whose route brings a rule in with `apply`, while a condition of the form
/// (`shown when`) reads an output of the same rule (DESIGN 6.4): the output's enum is one
/// `choices.rule` holds for the input the condition is `fed by`, and the `apply` brings the
/// rule's own enum into the route's rule — the same enum by two paths, which rulec declares
/// once (its §15.355), so the check says nothing where it said W201 (rulec's W121 it passed
/// through) before. The route's own read of a field's choice is what puts the module of
/// `choices.rule` in the route's module at all.
#[test]
fn the_enum_an_apply_and_a_condition_share_is_declared_once() {
    for (ja, uses, form, route, who) in [
        (
            false,
            "use rule authority from \"shared/authority.rule\"\n\n",
            "  kind   : choice \"Kind\"\n    goods    \"Goods\"\n    capex    \"Capital\"\n  note   : text \"Note\"  count <=100  optional  shown when authority.decider is cfo\n",
            "\n  first  finance  when kind is goods\n  decide authority.decider",
            "\n\nwho\n  finance = team finance\n  manager = team managers\n  cfo     = team cfos",
        ),
        (
            true,
            "use rule 権限(authority) from \"shared/権限.rule\"\n\n",
            "  種類(kind)   : choice \"種類\"\n    goods    \"物品\"\n    capex    \"設備\"\n  備考(note)   : text \"備考\"  count <=100  optional  shown when 権限.decider is cfo\n",
            "\n  一次(first)   経理   when 種類 is goods\n  決裁(decide)  権限.decider",
            "\n\nwho\n  経理(finance) = team finance\n  部長(manager) = team managers\n  財務部長(cfo)  = team cfos",
        ),
    ] {
        let src = if ja {
            format!("request 購買(share) v1 \"購買\"\n\n{uses}form\n  金額(amount) : money[JPY] \"金額\"  range >=1JPY <=100_000JPY\n{form}\nroute{route}{who}\n\nticket\n  self approval  not allowed\n")
        } else {
            format!("request share v1 \"Share\"\n\n{uses}form\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n{form}\nroute{route}{who}\n\nticket\n  self approval  not allowed\n")
        };
        let o = common::check_text("tests/mutants/share.ringi", &src);
        // the two paths are there: the route applies the rule and reads the file of the choices
        let route_text = lowered_text(&o, "route.rule");
        assert!(route_text.contains("\napply authority = "), "{ja}: {route_text}");
        assert!(route_text.contains("import enum kind from \"choices.rule\""), "{ja}: {route_text}");
        assert!(lowered_text(&o, "choices.rule").contains("enum authority_decider = manager | cfo"), "{ja}");
        // and the one enum is declared once: no W201 (rulec's W121) of the lowered files
        assert!(o.diags.is_empty(), "{ja}: {}", common::shown(&o, Lang::En));
    }
}
