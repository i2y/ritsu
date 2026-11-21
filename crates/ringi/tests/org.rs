//! The organization's data against the requests (DESIGN 8, PLAN C2): every version is checked,
//! the ones laid ahead too, and `--at` checks the one in effect on a day; the data that does not
//! read (a ring of parents, a person with no primary team or two) is E501, said once at its row,
//! and a request that reads it waits for it (E202); a proxy is checked on the days of each version
//! it meets (E505), and is for the requests its row names; a master's person is checked against
//! every version (E505); `effective from` before the first version is E502; `when empty escalate`
//! where nobody is above waits (E502) unless it says `then skip` or `then <way>`, for whoever is at
//! the top of each version; what the shape of the organization does to a stage is counted under
//! `ok`, with its cases in the JSON, and what a gap in the data does is W502; a request no stage of
//! which has anyone to answer it is W510; a stage late with nowhere to go up to is counted under
//! `ok`, or W507 where a gap in the data leaves it so, and `simulate` says what it does; `same
//! approver approves once` does a stage with the approval of the one right before it, and W506 says
//! the ones it does not do, and the ones it would; the data is named as the files write it. Each
//! organization is written into a directory of its own.

mod common;

use ringi::check::{Options, Run};
use ritsu_base::text::Lang;
use std::path::{Path, PathBuf};

const COMPANY: &str = "organization company v1 \"Company\"
description \"A small company (fictional)\"

positions
  staff     \"Staff\"
  manager   \"Manager\"
  dept_head \"Head of department\"
  president \"President\"

team kinds
  company    \"Company\"
  department \"Department\"
  section    \"Section\"

data \"org\"
";

/// The version of 2026-04-01: a sales department with a section, and finance, whose accountant p06
/// works in finance and p03 in both sales and finance.
const TEAMS_2026: &str = "id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,p01
sales,Sales,営業部,department,company,p02
sales1,Sales 1,営業一課,section,sales,p03
finance,Finance,財務部,department,company,p05
";
const PEOPLE_2026: &str = "id,name,name_ja
p01,President (fictional),社長（架空）
p02,Head of Sales,営業部長
p03,Manager of Sales 1,営業一課長
p04,Sales staff,営業担当
p05,Head of Finance,財務部長
p06,Accountant,経理担当
";
const MEMBERS_2026: &str = "person,team,position,primary
p01,company,president,yes
p02,sales,dept_head,yes
p03,sales1,manager,yes
p03,finance,staff,no
p04,sales1,staff,yes
p05,finance,dept_head,yes
p06,finance,staff,yes
";

/// The version of 2027-04-01, laid ahead: the section is gone, p04 is in sales, and p03 and p06
/// have left.
const TEAMS_2027: &str = "id,name,name_ja,kind,parent,head
company,Example Co. (fictional),株式会社サンプル（架空）,company,,p01
sales,Sales,営業部,department,company,p02
finance,Finance,財務部,department,company,p05
";
const PEOPLE_2027: &str = "id,name,name_ja
p01,President (fictional),社長（架空）
p02,Head of Sales,営業部長
p04,Sales staff,営業担当
p05,Head of Finance,財務部長
";
const MEMBERS_2027: &str = "person,team,position,primary
p01,company,president,yes
p02,sales,dept_head,yes
p04,sales,staff,yes
p05,finance,dept_head,yes
";

/// A request whose one stage is the requester's manager: the president has none (E502, as the
/// stage is `when empty stay`).
const LEAVE: &str = "request leave v1 \"Leave\"

use organization org from \"company.ringi\"

form
  days : number \"Days\"  range >=1 <=20

route
  check  manager

who
  manager = manager of requester

ticket
  self approval  not allowed
";

/// A directory holding `company.ringi`, the data of both versions in `org/` with `proxies` as its
/// `proxies.csv` (none when empty), and the files given.
struct Company {
    tmp: ritsu_testkit::TempDir,
}

impl Company {
    fn new(name: &str, proxies: &str, files: &[(&str, &str)]) -> Company {
        let tmp = ritsu_testkit::TempDir::new(name);
        let c = Company { tmp };
        c.write("company.ringi", COMPANY);
        for (day, t, p, m) in [("2026-04-01", TEAMS_2026, PEOPLE_2026, MEMBERS_2026), ("2027-04-01", TEAMS_2027, PEOPLE_2027, MEMBERS_2027)] {
            c.write(&format!("org/{day}/teams.csv"), t);
            c.write(&format!("org/{day}/people.csv"), p);
            c.write(&format!("org/{day}/members.csv"), m);
        }
        if !proxies.is_empty() {
            c.write("org/proxies.csv", proxies);
        }
        for (n, t) in files {
            c.write(n, t);
        }
        c
    }

    fn write(&self, name: &str, text: &str) {
        let p = self.tmp.path().join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn path(&self, name: &str) -> PathBuf {
        self.tmp.path().join(name)
    }

    /// The file `name` checked with every language joined, with `opts`.
    fn check(&self, name: &str, opts: Options) -> std::rc::Rc<ringi::check::Outcome> {
        Run::with(common::joined(), opts).file(&self.path(name).display().to_string()).unwrap()
    }
}

fn at(day: &str) -> Options {
    Options { at: ritsu_ports::day_of(day), ..Options::default() }
}

fn messages(o: &ringi::check::Outcome) -> Vec<(&'static str, String)> {
    o.diags.iter().map(|d| (d.code, d.message.en.clone())).collect()
}

/// The notes of a diagnostic, in English.
fn notes(d: &ringi::diag::Diag) -> Vec<String> {
    d.notes.iter().map(|n| n.en.clone()).collect()
}

/// `PEOPLE_2026` with a column (`email`, `language`) added: `cells` gives each row's value by ID,
/// an empty one the rows it leaves out (a row writes a cell for every column — one of another
/// number does not read).
fn people_with(col: &str, cells: &[(&str, &str)]) -> String {
    let mut out = format!("id,name,name_ja,{col}\n");
    for row in PEOPLE_2026.lines().skip(1) {
        let id = row.split(',').next().unwrap();
        let v = cells.iter().find(|(i, _)| *i == id).map(|(_, v)| *v).unwrap_or("");
        out.push_str(&format!("{row},{v}\n"));
    }
    out
}

/// One case of what the shape of the organization does, as the JSON's `facts` lists it: what
/// happens, the versions, the cause, the notes, and every requester with the one it came to.
#[derive(Debug, PartialEq)]
struct Case {
    what: String,
    versions: Vec<String>,
    cause: String,
    notes: Vec<String>,
    requesters: Vec<(String, Option<String>)>,
}

/// The cases of the JSON's `facts`, in order.
fn cases(o: &ringi::check::Outcome) -> Vec<Case> {
    let v = ringi::check::to_json(o, Path::new("."), Lang::En);
    let strs = |x: Option<&ritsu_base::json::Json>| -> Vec<String> { x.and_then(|x| x.as_arr()).unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect() };
    let mut out = Vec::new();
    for f in v.get("facts").and_then(|x| x.as_arr()).unwrap() {
        for c in f.get("cases").and_then(|x| x.as_arr()).unwrap() {
            let s = |k: &str| c.get(k).and_then(|x| x.as_str()).unwrap().to_string();
            let requesters = c.get("requesters").and_then(|x| x.as_arr()).unwrap().iter().map(|r| (r.get("id").and_then(|x| x.as_str()).unwrap().to_string(), r.get("to").and_then(|x| x.as_str()).map(str::to_string))).collect();
            out.push(Case { what: s("what"), versions: strs(c.get("versions")), cause: s("cause"), notes: strs(c.get("notes")), requesters });
        }
    }
    out
}

/// The cases of the JSON's `facts` of late stages with nowhere to go up to.
fn late_cases(o: &ringi::check::Outcome) -> Vec<Case> {
    cases(o).into_iter().filter(|c| c.what == "late").collect()
}

/// The lines under `ok` that count what the shape of the organization does.
fn shaped(o: &ringi::check::Outcome) -> Vec<String> {
    o.facts.iter().filter(|f| !f.cases.is_empty()).map(|f| f.text.en.clone()).collect()
}

/// The requesters of a case, by ID, with the one it came to.
fn to(rs: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
    rs.iter().map(|(r, t)| (r.to_string(), t.map(str::to_string))).collect()
}

/// Every version is checked, the one laid ahead too; a finding the same in both is said once,
/// naming both; `--at` checks the version in effect that day.
#[test]
fn every_version_is_checked_and_at_picks_one() {
    let c = Company::new("ringi-org-versions", "", &[("leave.ringi", LEAVE)]);
    let o = c.check("leave.ringi", Options::default());
    assert_eq!(messages(&o), [("E502", "In the versions of the organization from 2026-04-01 and 2027-04-01, the stage `check` has no approver".to_string())], "{}", common::shown(&o, Lang::En));
    assert!(o.diags[0].notes.iter().any(|n| n.en == "For 1 requester: p01 (President (fictional))."), "{}", common::shown(&o, Lang::En));
    for (day, version) in [("2026-06-01", "2026-04-01"), ("2027-04-01", "2027-04-01"), ("2030-01-01", "2027-04-01")] {
        let o = c.check("leave.ringi", at(day));
        assert_eq!(messages(&o), [("E502", format!("In the organization from {version}, the stage `check` has no approver"))], "{day}");
    }
    // the organization's own line: both versions, and what the last holds
    let o = c.check("company.ringi", Options::default());
    assert_eq!(o.facts[0].text.en, "data `org`: 2 versions (2026-04-01, 2027-04-01), 3 teams and 4 people in the last, no proxies");
    let o = c.check("company.ringi", at("2026-06-01"));
    assert!(o.facts[0].text.en.starts_with("data `org`: "), "{}", o.facts[0].text.en);
}

/// A team or a person the organization does not have is one E503 where the way names it, whatever
/// the versions it is missing from: the versions are named in the message and in the cause, all of
/// them when none has it, and only the ones without it when some have it. Each version checked
/// alone says it once, so the check of every version says them together, not once for each.
#[test]
fn someone_missing_from_several_versions_is_said_once() {
    let with = |way: &str| LEAVE.replace("  check  manager\n", "  check  manager\n  review  books\n").replace("ticket\n", "every stage\n  when empty escalate\n\nticket\n").replace("  manager = manager of requester\n", &format!("  manager = manager of requester\n  books   = {way}\n"));
    let c = Company::new("ringi-org-missing", "", &[("none.ringi", &with("team financ")), ("gone.ringi", &with("team sales1")), ("left.ringi", &with("person \"p06\""))]);
    for (file, said, cause) in [
        ("none.ringi", "In the versions of the organization from 2026-04-01 and 2027-04-01, `books` names someone who is not there", "`books` names the team `financ`, which is not in the versions of the organization from 2026-04-01 and 2027-04-01"),
        ("gone.ringi", "In the organization from 2027-04-01, `books` names someone who is not there", "`books` names the team `sales1`, which is not in the organization from 2027-04-01"),
        ("left.ringi", "In the organization from 2027-04-01, `books` names someone who is not there", "`books` names p06, who is not in the organization from 2027-04-01"),
    ] {
        let o = c.check(file, Options::default());
        let e503: Vec<&ringi::diag::Diag> = o.diags.iter().filter(|d| d.code == "E503").collect();
        assert_eq!(e503.len(), 1, "{file}: {}", common::shown(&o, Lang::En));
        assert_eq!(e503[0].message.en, said, "{file}");
        assert!(notes(e503[0]).iter().any(|n| n.starts_with(cause)), "{file}: {:?}", notes(e503[0]));
        // each version alone, as `--at` checks it: the same finding, once in each that lacks it
        let alone: usize = ["2026-06-01", "2027-06-01"].iter().map(|day| c.check(file, at(day)).diags.iter().filter(|d| d.code == "E503").count()).sum();
        assert_eq!(alone, if file == "none.ringi" { 2 } else { 1 }, "{file}");
    }
    let o = c.check("none.ringi", Options::default());
    let d = o.diags.iter().find(|d| d.code == "E503").unwrap();
    assert_eq!(d.message.ja, "2026-04-01 からと 2027-04-01 からの組織では、`books` が指す人かチームがありません");
    assert!(d.notes.iter().any(|n| n.ja.starts_with("`books` が指すチーム `financ` は、2026-04-01 からと 2027-04-01 からの組織にありません")), "{:?}", d.notes.iter().map(|n| &n.ja).collect::<Vec<_>>());
}

/// `manager of requester` is the head of the requester's primary team, or the head above when
/// that is the requester; a team of a second membership is not the requester's.
#[test]
fn the_manager_is_found_from_the_primary_team() {
    let c = Company::new("ringi-org-manager", "", &[("leave.ringi", LEAVE)]);
    let leave = c.path("leave.ringi").display().to_string();
    let run = |args: &[&str]| {
        let mut all = vec!["simulate".to_string(), leave.clone()];
        all.extend(args.iter().map(|s| s.to_string()));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = ringi::run::run(&all, common::joined(), &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    };
    let (code, out, err) = run(&["--requester", "p03", "--on", "2026-06-01"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("  check  manager: p02 (Head of Sales)\n"), "{out}");
    let (_, out, _) = run(&["--requester", "p04", "--on", "2026-06-01"]);
    assert!(out.contains("  check  manager: p03 (Manager of Sales 1)\n"), "{out}");
    // after the switch, the section is gone and p04 is in sales
    let (_, out, _) = run(&["--requester", "p04", "--on", "2027-04-01"]);
    assert!(out.starts_with(&format!("{leave} on 2027-04-01, in the organization from 2027-04-01\n")), "{out}");
    assert!(out.contains("  check  manager: p02 (Head of Sales)\n"), "{out}");
    // p03 has left by then
    let (code, _, err) = run(&["--requester", "p03", "--on", "2027-04-01"]);
    assert_eq!((code, err.as_str()), (2, "error: p03 is not in the organization from 2027-04-01\n"));
}

/// A ring of parents, a person with no primary team and one with two are E501 at their rows; the
/// request that reads the organization waits for it (E202), and says nothing of its own approvers.
#[test]
fn data_that_does_not_read_is_said_once_at_its_row() {
    // in the version of 2026, the memberships; in the one of 2027, a ring of parents (a ring is
    // looked for once the memberships and the heads of a version read)
    let members = MEMBERS_2026.replace("p06,finance,staff,yes", "p06,finance,staff,no").replace("p04,sales1,staff,yes", "p04,sales1,staff,yes\np04,finance,staff,yes");
    let ring = TEAMS_2027.replace("sales,Sales,営業部,department,company,p02", "sales,Sales,営業部,department,finance,p02").replace("finance,Finance,財務部,department,company,p05", "finance,Finance,財務部,department,sales,p05");
    let c = Company::new("ringi-org-unread", "", &[("leave.ringi", LEAVE), ("org/2026-04-01/members.csv", &members), ("org/2027-04-01/teams.csv", &ring)]);
    let o = c.check("company.ringi", Options::default());
    let said: Vec<(String, Option<usize>, String)> = o.diags.iter().map(|d| (Path::new(&d.file).components().rev().take(2).map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("<"), d.line, d.message.en.clone())).collect();
    assert!(o.diags.iter().all(|d| d.code == "E501"), "{}", common::shown(&o, Lang::En));
    assert_eq!(
        said,
        [
            ("members.csv<2026-04-01".to_string(), Some(7), "The person `p04` has 2 primary teams: sales1 (line 6), finance (line 7)".to_string()),
            ("people.csv<2026-04-01".to_string(), Some(7), "The person `p06` has no primary team".to_string()),
            ("teams.csv<2027-04-01".to_string(), Some(3), "The parents of the team `sales` lead back to it (sales → finance → sales)".to_string()),
        ],
        "{}",
        common::shown(&o, Lang::En)
    );
    // the request: E202 at its `use` line, and no finding of the approvers
    let o = c.check("leave.ringi", Options::default());
    assert_eq!(common::codes(&o), ["E202"], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[0].line, Some(3));
}

/// A proxy is checked on the days of each version it meets: across the switch, the proxy who has
/// left is E505 for the version of 2027 alone, with the day to end the row on.
#[test]
fn a_proxy_across_the_switch() {
    let proxies = "person,proxy,kind,from,to,requests\np02,p03,approve,2027-03-20,2027-04-10,\n";
    let c = Company::new("ringi-org-proxy", proxies, &[("leave.ringi", LEAVE)]);
    let o = c.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E505", "`p03`, the proxy of this row, is not in the organization from 2027-04-01".to_string())], "{}", common::shown(&o, Lang::En));
    let notes: Vec<&str> = o.diags[0].notes.iter().map(|n| n.en.as_str()).collect();
    assert_eq!(
        notes,
        [
            "The proxy is in effect 2027-03-20 to 2027-04-10, which overlaps the days of the organization from 2027-04-01.",
            "If the person has left, make this row's `to` 2027-03-31, the day before the organization from 2027-04-01; if the ID is wrong, correct it."
        ]
    );
    assert_eq!((o.diags[0].line, Path::new(&o.diags[0].file).file_name().unwrap().to_str()), (Some(2), Some("proxies.csv")));
    // with `--at` before the switch, the version of 2026 alone is checked, and the row passes
    let o = c.check("company.ringi", at("2026-06-01"));
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    // the request waits for the organization
    let o = c.check("leave.ringi", Options::default());
    assert_eq!(common::codes(&o), ["E202"], "{}", common::shown(&o, Lang::En));
}

/// A proxy is for the requests its row names: an approve proxy for `expense` stands in on an
/// expense, not on a leave; a submit proxy for `leave` makes leave requests for the person, and
/// the listing counts the person made for as a requester too.
#[test]
fn the_requests_a_proxy_is_for() {
    let proxies = "person,proxy,kind,from,to,requests\np03,p02,approve,2026-12-01,2026-12-31,expense\np04,p06,submit,2026-12-01,2026-12-31,leave\n";
    let expense = LEAVE.replace("request leave v1 \"Leave\"", "request expense v1 \"Expense\"");
    let c = Company::new("ringi-org-filters", proxies, &[("leave.ringi", LEAVE), ("expense.ringi", &expense)]);
    let json = |file: &str| {
        let f = c.path(file).display().to_string();
        let args: Vec<String> = ["simulate", &f, "--requester", "p04", "--on", "2026-12-10", "--format", "json"].iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        ritsu_base::json::parse(&String::from_utf8(out).unwrap()).unwrap()
    };
    let proxies_of = |v: &ritsu_base::json::Json| -> Vec<String> {
        let r = &v.get("requesters").and_then(|x| x.as_arr()).unwrap()[0];
        let w = &r.get("stages").and_then(|x| x.as_arr()).unwrap()[0].get("ways").and_then(|x| x.as_arr()).unwrap()[0];
        w.get("proxies").and_then(|x| x.as_arr()).unwrap().iter().map(|p| p.to_string()).collect()
    };
    // p04's manager is p03, whose proxy p02 stands in on an expense alone
    assert!(proxies_of(&json("expense.ringi")).iter().any(|p| p.contains("p02")), "{:?}", proxies_of(&json("expense.ringi")));
    assert!(proxies_of(&json("leave.ringi")).is_empty(), "{:?}", proxies_of(&json("leave.ringi")));
    // every requester of leave: p04 made for by p06 too; of expense, not
    let every = |file: &str| {
        let f = c.path(file).display().to_string();
        let args: Vec<String> = ["simulate", &f, "--every-requester", "--on", "2026-12-10"].iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        ringi::run::run(&args, common::joined(), &mut out, &mut err);
        String::from_utf8(out).unwrap()
    };
    assert!(every("leave.ringi").contains(", p04 (Sales staff) [made by p06 (Accountant)] (2 requesters)\n"), "{}", every("leave.ringi"));
    assert!(!every("expense.ringi").contains("made by"), "{}", every("expense.ringi"));
    assert!(every("expense.ringi").contains("  check  manager: p03 (Manager of Sales 1) [p02 (Head of Sales) too, as the proxy of p03]\n"), "{}", every("expense.ringi"));
}

/// A master's person is checked against every version whose days its row meets: the owner who has
/// left by 2027 is E505 at the row, with the day to end it on and a row to add.
#[test]
fn a_masters_person_who_has_left() {
    let masters = "masters company_masters v1\n\nuse organization org from \"company.ringi\"\n\nmaster centres \"Cost centres\"\n  key centre\n  fields\n    centre : text \"Centre\"  count <=10\n    owner  : person \"Owner\"\n  data \"centres.csv\"\n";
    let c = Company::new("ringi-org-masters", "", &[("masters.ringi", masters), ("centres.csv", "centre,owner,valid_from,valid_to\nC1,p03,2026-04-01,\nC2,p05,2026-04-01,\n")]);
    let o = c.check("masters.ringi", Options::default());
    assert_eq!(messages(&o), [("E505", "`p03`, the `owner` of the row `C1` of the master `centres`, is not in the organization from 2027-04-01".to_string())], "{}", common::shown(&o, Lang::En));
    let notes: Vec<&str> = o.diags[0].notes.iter().map(|n| n.en.as_str()).collect();
    assert_eq!(notes[1], "If the person has left, make this row's `valid_to` 2027-03-31, the day before the organization from 2027-04-01, and add a row with the new person from 2027-04-01; if the ID is wrong, correct it.");
    let ja: Vec<&str> = o.diags[0].notes.iter().map(|n| n.ja.as_str()).collect();
    assert_eq!(ja[0], "この行は 2026-04-01 から終わり無く選べ、2027-04-01 からの組織が効く日と重なります。");
}

/// `effective from` before the first version: E502 at the line, for no stage has an approver on
/// those days; on the day of the second, only that version is listed: the president's request is
/// skipped at its one stage, and nobody answers it (W510).
#[test]
fn effective_from_and_the_versions() {
    let early = LEAVE.replace("request leave v1 \"Leave\"\n", "request leave v1 \"Leave\"\neffective from 2026-01-01\n");
    let late = LEAVE.replace("request leave v1 \"Leave\"\n", "request leave v1 \"Leave\"\neffective from 2027-04-01\n").replace("ticket\n", "every stage\n  when empty escalate, then skip\n\nticket\n");
    let c = Company::new("ringi-org-effective", "", &[("early.ringi", &early), ("late.ringi", &late)]);
    let o = c.check("early.ringi", Options::default());
    let said = messages(&o);
    assert!(said.contains(&("E502", "From 2026-01-01 (`effective from`) to 2026-03-31, no version of the organization is in effect: no stage has an approver".to_string())), "{}", common::shown(&o, Lang::En));
    let o = c.check("late.ringi", Options::default());
    assert_eq!(messages(&o), [("W510", "In the organization from 2027-04-01, no stage of the request has anyone to answer it for some requesters".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.facts[0].text.en, "used from 2027-04-01: `effective from`");
    assert_eq!(shaped(&o), ["requesters whose stages (`check`) go elsewhere or are skipped by the shape of the organization: 1 of 4 in the organization from 2027-04-01"]);
}

/// `when empty escalate` where nobody is above waits (E502), for whoever is at the top in each
/// version: with the president gone from the version of 2027 and the company's head empty, the
/// heads of the departments are at the top there. `then stay` is what nothing written is; `then
/// skip` skips the stage; `then board` goes to the way `board`, which answers or finds nobody
/// either (E502). For the president of 2026, at the top of the organization, that is what the
/// request says: `ok` counts them, and the JSON lists the case; in 2027 it is for the empty seat
/// (W502). Where the one stage is skipped, nobody answers the request (W510). `simulate` says
/// which, for the person in the version.
#[test]
fn escalate_at_the_top_of_each_version() {
    let teams = TEAMS_2027.replace(",company,,p01\n", ",company,,\n");
    let people = PEOPLE_2027.replace("p01,President (fictional),社長（架空）\n", "");
    let members = MEMBERS_2027.replace("p01,company,president,yes\n", "");
    let with = |line: &str| LEAVE.replace("ticket\n", &format!("every stage\n  {line}\n\nticket\n"));
    let (bare, stay, skip) = (with("when empty escalate"), with("when empty escalate, then stay"), with("when empty escalate, then skip"));
    let board = with("when empty escalate, then board").replace("  manager = manager of requester\n", "  manager = manager of requester\n  board   = team finance\n");
    let c = Company::new(
        "ringi-org-top",
        "",
        &[
            ("org/2027-04-01/teams.csv", &teams),
            ("org/2027-04-01/people.csv", &people),
            ("org/2027-04-01/members.csv", &members),
            ("bare.ringi", &bare),
            ("stay.ringi", &stay),
            ("skip.ringi", &skip),
            ("board.ringi", &board),
        ],
    );
    let whom = |o: &ringi::check::Outcome| -> Vec<String> { o.diags.iter().map(|d| d.notes.iter().find(|n| n.en.starts_with("For ")).map(|n| n.en.clone()).unwrap_or_default()).collect() };
    let tops = ["For 1 requester: p01 (President (fictional)).", "For 2 requesters: p02 (Head of Sales) and p05 (Head of Finance)."];
    for (file, written) in [("bare.ringi", "when empty escalate"), ("stay.ringi", "when empty escalate, then stay")] {
        let o = c.check(file, Options::default());
        assert_eq!(
            messages(&o),
            [
                ("E502", "In the organization from 2026-04-01, the stage `check` has no approver".to_string()),
                ("E502", "In the organization from 2027-04-01, the stage `check` has no approver".to_string())
            ],
            "{}",
            common::shown(&o, Lang::En)
        );
        assert_eq!(whom(&o), tops);
        let notes: Vec<&str> = o.diags[0].notes.iter().map(|n| n.en.as_str()).collect();
        assert_eq!(notes[2], format!("The stage is `{written}`, and nobody is above (the top of the organization): a ticket that comes to it waits there until an administrator adds an approver."));
        assert!(notes[3].starts_with("If the stage may be skipped for them, write `when empty escalate, then skip`; "), "{}", notes[3]);
    }
    let o = c.check("skip.ringi", Options::default());
    let shown = common::shown(&o, Lang::En);
    let nobody = |v: &str| ("W510", format!("In the organization from {v}, no stage of the request has anyone to answer it for some requesters"));
    assert_eq!(
        messages(&o),
        [("W502", "In the organization from 2027-04-01, the stage `check` goes elsewhere or is skipped for lack of an approver".to_string()), nobody("2026-04-01"), nobody("2027-04-01")],
        "{shown}"
    );
    assert_eq!(whom(&o), [tops[1], tops[0], tops[1]]);
    assert_eq!(
        notes(&o.diags[0])[1..3],
        ["The stage is `when empty escalate, then skip`, and nobody is above: it is skipped.", "The head of the team company (Example Co. (fictional)) is empty."],
        "{shown}"
    );
    assert_eq!(notes(&o.diags[1])[..2], ["`check` is skipped for lack of an approver, and no other stage has anyone to answer it.", "A ticket of theirs is approved as soon as it is submitted, with nobody answering it."]);
    // the president of 2026: what the request says, counted under `ok`, the case in the JSON
    assert_eq!(shaped(&o), ["requesters whose stages (`check`) go elsewhere or are skipped by the shape of the organization: 1 of 6 in the organization from 2026-04-01"]);
    assert_eq!(
        cases(&o),
        [Case {
            what: "skipped".to_string(),
            versions: vec!["2026-04-01".to_string()],
            cause: "`manager` finds no head above the requester.".to_string(),
            notes: vec!["The stage is `when empty escalate, then skip`, and nobody is above (the top of the organization): it is skipped.".to_string()],
            requesters: to(&[("p01", None)]),
        }]
    );
    let f = c.path("skip.ringi").display().to_string();
    assert_eq!(o.facts.last().unwrap().text.en, format!("who they are, and where their stages go: `ringi simulate {f} --every-requester`"));
    assert_eq!(o.facts.last().unwrap().text.ja, format!("だれの申請の、どのステップがどこへ回るか: `ringi simulate {f} --every-requester`"));
    // `simulate`: the head of sales is under the president in 2026, and at the top in 2027
    let simulate = |file: &str, on: &str| {
        let f = c.path(file).display().to_string();
        let args: Vec<String> = ["simulate", &f, "--requester", "p02", "--on", on].iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(ringi::run::run(&args, common::joined(), &mut out, &mut err), 0, "{}", String::from_utf8_lossy(&err));
        String::from_utf8(out).unwrap()
    };
    assert!(simulate("bare.ringi", "2026-06-01").contains("  check  manager: p01 (President (fictional))\n"));
    assert!(simulate("bare.ringi", "2027-04-01").contains("  check  manager: no head is above the requester; the ticket waits until an administrator adds an approver\n"));
    assert!(simulate("skip.ringi", "2027-04-01").contains("  check  manager: no head is above the requester; the stage is skipped\n"));
    // `then board`: finance answers the president's request in 2026 (what the request says), and in
    // 2027 the head of sales' (for the empty seat); the head of finance is all finance has in 2027,
    // and their own request waits
    let o = c.check("board.ringi", Options::default());
    let shown = common::shown(&o, Lang::En);
    assert_eq!(common::codes(&o), ["E502", "W502"], "{shown}");
    // the president of 2026: what the request says, counted under `ok` where the version passes
    let then = c.check("board.ringi", at("2026-06-01"));
    assert!(then.diags.is_empty(), "{}", common::shown(&then, Lang::En));
    assert_eq!(
        cases(&then),
        [Case {
            what: "then".to_string(),
            versions: vec!["2026-04-01".to_string()],
            cause: "`manager` finds no head above the requester.".to_string(),
            notes: vec!["The stage is `when empty escalate, then board`, and nobody is above (the top of the organization): it goes to `board`, where one of p03 (Manager of Sales 1), p05 (Head of Finance) and p06 (Accountant) answers.".to_string()],
            requesters: to(&[("p01", None)]),
        }]
    );
    assert_eq!(o.diags[1].message.en, "In the organization from 2027-04-01, the stage `check` goes elsewhere or is skipped for lack of an approver");
    assert_eq!(
        notes(&o.diags[1])[1..],
        [
            "The stage is `when empty escalate, then board`, and nobody is above: it goes to `board`.".to_string(),
            "The head of the team company (Example Co. (fictional)) is empty.".to_string(),
            "It goes to p05 (Head of Finance) for 1 requester: p02 (Head of Sales).".to_string(),
            "Correct the data of the version, or make sure this is meant.".to_string()
        ],
        "{shown}"
    );
    assert_eq!(o.diags[0].message.en, "In the organization from 2027-04-01, the stage `check` has no approver");
    assert_eq!(
        notes(&o.diags[0])[1..],
        [
            "For 1 requester: p05 (Head of Finance).".to_string(),
            "The stage is `when empty escalate, then board`, and nobody is above (the top of the organization): it goes to `board`, but `board` comes only to the requester, and self approval is not allowed. A ticket that comes to it waits there until an administrator adds an approver.".to_string(),
            "Correct the data of the version, or write after `then` a way that has someone in every version.".to_string()
        ],
        "{shown}"
    );
    let ask = |who: &str, on: &str| {
        let f = c.path("board.ringi").display().to_string();
        let args: Vec<String> = ["simulate", &f, "--requester", who, "--on", on].iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(ringi::run::run(&args, common::joined(), &mut out, &mut err), 0, "{}", String::from_utf8_lossy(&err));
        String::from_utf8(out).unwrap()
    };
    let said = ask("p02", "2027-04-01");
    assert!(said.contains("  check  manager: no head is above the requester; it goes to `board`: p05 (Head of Finance)\n"), "{said}");
    let said = ask("p05", "2027-04-01");
    assert!(said.contains("  check  manager: no head is above the requester; it goes to `board`, but it comes to the requester alone; the ticket waits until an administrator adds an approver\n"), "{said}");
}

/// A request with the office's calendar, whose stage is `route` with the ways `who`, and every
/// stage `late` and `when empty escalate, then skip`, with the lines `rest` after.
fn late_request(route: &str, who: &str, late: &str, rest: &str) -> String {
    format!(
        "request leave v1 \"Leave\"\n\nuse organization org from \"company.ringi\"\nuse calendar office from \"office.cal\"\n\nform\n  days : number \"Days\"  range >=1 <=20\n\nroute\n  {route}\n\nwho\n{who}\nevery stage\n  due in 3 business days at end of day\n  {late}\n  when empty escalate, then skip\n{rest}\nticket\n  self approval  not allowed\n"
    )
}

/// A stage that goes up when it is late, with nowhere to go up to. Without a table, from the one who
/// is late: the president has nobody above, either time (`then escalate`), and the second time from
/// the president it went to; from a team, above the team. With a table, the way it sends the stage
/// to, solved for the requester: a way that finds nobody goes up from where it stopped, and the
/// president's own request has nobody above that. At the top of the organization, that is what the
/// request says: `ok` counts the requesters, the JSON lists the cases with the one who is late, and
/// `simulate` says what the stage does when it is late. Where the seat of the head above is empty,
/// it is W507.
#[test]
fn late_with_nowhere_to_go_up_to() {
    let office = "calendar office v1\ndescription \"Saturdays and Sundays are closed (fictional)\"\noffset +09:00\n\nclosed weekly sat, sun\n";
    let twice = late_request("check  manager", "  manager = manager of requester\n", "when late escalate, then escalate", "");
    let team = late_request("check  board", "  board = team company\n", "when late escalate, then expire", "");
    let table = late_request(
        "check  finance",
        "  finance = team finance\n  top     = nearest president above requester\n",
        "when late escalate by next_up, then expire",
        "\ntable next_up\n| approver | -> goes_to |\n| finance  | top        |\n| top      | top        |\n",
    );
    let c = Company::new("ringi-org-late", "", &[("office.cal", office), ("twice.ringi", &twice), ("team.ringi", &team), ("table.ringi", &table)]);
    let both = vec!["2026-04-01".to_string(), "2027-04-01".to_string()];
    let top = "the one who is late is at the top of the organization, with nobody above to go up to.";
    // without a table: the president's reports, either time; the second time, from the president.
    // The president's own request is skipped, and nobody answers it (W510).
    let o = c.check("twice.ringi", Options::default());
    let shown = common::shown(&o, Lang::En);
    assert_eq!(common::codes(&o), ["W510"], "{shown}");
    assert_eq!(
        shaped(&o),
        [
            "requesters whose stages (`check`) go elsewhere or are skipped by the shape of the organization: 1 of 6 in the organization from 2026-04-01, and 1 of 4 from 2027-04-01",
            "requesters whose late stages (`check`) have nowhere to go up to by the shape of the organization: 4 of 6 in the organization from 2026-04-01, and 3 of 4 from 2027-04-01"
        ],
        "{shown}"
    );
    let second = "The stage is `when late escalate, then escalate`: the second time, it cannot go up, and expires at the second deadline.".to_string();
    assert_eq!(
        late_cases(&o),
        [
            Case {
                what: "late".to_string(),
                versions: both.clone(),
                cause: ritsu_base::text::capitalize(top),
                notes: vec!["The stage is `when late escalate, then escalate`: it cannot go up either time, and expires at the deadline.".to_string()],
                requesters: to(&[("p02", Some("p01")), ("p05", Some("p01"))]),
            },
            Case {
                what: "late".to_string(),
                versions: vec!["2026-04-01".to_string()],
                cause: format!("Going up the second time, {top}"),
                notes: vec![second.clone()],
                requesters: to(&[("p03", Some("p01")), ("p06", Some("p01"))]),
            },
            Case { what: "late".to_string(), versions: vec!["2027-04-01".to_string()], cause: format!("Going up the second time, {top}"), notes: vec![second], requesters: to(&[("p04", Some("p01"))]) },
        ],
        "{shown}"
    );
    // `simulate` says what the stage does when it is late, for the one it comes to
    let simulate = |file: &str, who: &str| {
        let f = c.path(file).display().to_string();
        let args: Vec<String> = ["simulate", &f, "--requester", who, "--on", "2026-06-01"].iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(ringi::run::run(&args, common::joined(), &mut out, &mut err), 0, "{}", String::from_utf8_lossy(&err));
        String::from_utf8(out).unwrap()
    };
    let said = simulate("twice.ringi", "p02");
    assert!(said.ends_with("  check  manager: p01 (President (fictional)); when it is late, it cannot go up either time, and expires at the deadline\n"), "{said}");
    let said = simulate("twice.ringi", "p03");
    assert!(said.ends_with("  check  manager: p02 (Head of Sales); when it is late, the second time, it cannot go up, and expires at the second deadline\n"), "{said}");
    let said = simulate("twice.ringi", "p04");
    assert!(said.ends_with("  check  manager: p03 (Manager of Sales 1)\n"), "{said}");
    // a team: above the team, for every requester but the president (whose stage is skipped)
    let o = c.check("team.ringi", Options::default());
    assert_eq!(common::codes(&o), ["W510"], "{}", common::shown(&o, Lang::En));
    let cs = late_cases(&o);
    let above = "Nobody is above the team company (Example Co. (fictional)) to go up to.";
    assert_eq!(
        cs.iter().map(|c| (c.cause.as_str(), c.versions.clone(), c.requesters.clone())).collect::<Vec<_>>(),
        [
            (above, vec!["2026-04-01".to_string()], to(&[("p02", None), ("p03", None), ("p04", None), ("p05", None), ("p06", None)])),
            (above, vec!["2027-04-01".to_string()], to(&[("p02", None), ("p04", None), ("p05", None)]))
        ],
        "{}",
        common::shown(&o, Lang::En)
    );
    // a table: the way it sends the stage to finds nobody for the president, and nobody is above
    let o = c.check("table.ringi", Options::default());
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    assert_eq!(
        late_cases(&o),
        [Case {
            what: "late".to_string(),
            versions: both.clone(),
            cause: "The table `next_up` sends a late `finance` to `top`, and `top` finds no head above the requester; nobody is above that either.".to_string(),
            notes: vec!["The stage is `when late escalate by next_up, then expire`: it cannot go up, and expires at the deadline.".to_string()],
            requesters: to(&[("p01", None)]),
        }]
    );
    let said = simulate("table.ringi", "p01");
    assert!(said.ends_with("; when it is late, it cannot go up, and expires at the deadline\n"), "{said}");
    // the seat of the head above the one who is late is empty in 2027: W507 for the staff of sales,
    // whose manager is the head of sales; the heads of the departments are skipped (W502), and
    // nobody answers them (W510)
    let vacant = TEAMS_2027.replace(",company,,p01\n", ",company,,\n");
    let c = Company::new("ringi-org-late-vacant", "", &[("office.cal", office), ("org/2027-04-01/teams.csv", &vacant), ("twice.ringi", &twice)]);
    let o = c.check("twice.ringi", Options::default());
    let shown = common::shown(&o, Lang::En);
    assert_eq!(common::codes(&o), ["W502", "W507", "W510", "W510"], "{shown}");
    assert_eq!(o.diags[1].message.en, "In the organization from 2027-04-01, the stage `check` has nowhere to go up to when it is late");
    assert_eq!(
        notes(&o.diags[1]),
        [
            "nobody is above the one who is late to go up to.",
            "The head of the team company (Example Co. (fictional)) is empty.",
            "The stage is `when late escalate, then escalate`: it cannot go up either time, and expires at the deadline.",
            "Late with p02 (Head of Sales) for 1 requester: p04 (Sales staff).",
            "Correct the data of the version; for it to go to someone else, say where it goes in the table of `when late escalate by <table>`."
        ],
        "{shown}"
    );
    // 2026 is as it was: what the request says, counted under `ok`
    assert_eq!(late_cases(&o).iter().map(|c| c.versions.clone()).collect::<Vec<_>>(), [vec!["2026-04-01".to_string()], vec!["2026-04-01".to_string()]], "{shown}");
}

/// A late stage for whoever a stage comes to where its own way finds nobody (DESIGN 5.4, 5.10): the
/// head it goes up to, and at the top the people of the way `then` names. The stage is the head of
/// the requester's department: with the head of sales empty in 2026, it goes up to the president for
/// the people of sales (W502), and the president has nobody above (W507, for the empty seat sent the
/// stage there). For the heads of the departments it goes up to the president too, and the
/// president's own request goes to finance, and nobody is above finance but the president: that is
/// what the request says, counted under `ok`. A table reads the way `then` names for its people, and
/// the stage's way for the head it went up to.
#[test]
fn late_where_the_stage_went_elsewhere() {
    let office = "calendar office v1\ndescription \"Saturdays and Sundays are closed (fictional)\"\noffset +09:00\n\nclosed weekly sat, sun\n";
    let teams = TEAMS_2026.replace(",department,company,p02\n", ",department,company,\n");
    let who = "  dept_head = head of requester.department\n  board     = team finance\n  top       = nearest president above requester\n";
    let up = late_request("check  dept_head", who, "when late escalate, then expire", "").replace("then skip", "then board");
    let table = late_request(
        "check  dept_head",
        who,
        "when late escalate by next_up, then expire",
        "\ntable next_up\n| approver  | -> goes_to |\n| dept_head | board      |\n| board     | top        |\n| top       | top        |\n",
    )
    .replace("then skip", "then board");
    let c = Company::new("ringi-org-late-elsewhere", "", &[("office.cal", office), ("org/2026-04-01/teams.csv", &teams), ("up.ringi", &up), ("table.ringi", &table)]);
    let o = c.check("up.ringi", Options::default());
    let shown = common::shown(&o, Lang::En);
    let expire = "The stage is `when late escalate, then expire`: it cannot go up, and expires at the deadline.";
    let top = "the one who is late is at the top of the organization, with nobody above to go up to.";
    let went_up = "The stage is `when empty escalate, then board`, and it goes up for lack of an approver.";
    let to_board = "The stage is `when empty escalate, then board`, and nobody is above (the top of the organization): it goes to `board`.";
    let both = vec!["2026-04-01".to_string(), "2027-04-01".to_string()];
    // the empty seat of the head of sales: what it sends the stage to is said with the warning
    assert_eq!(common::codes(&o), ["W502", "W507"], "{shown}");
    assert_eq!(o.diags[1].message.en, "In the organization from 2026-04-01, the stage `check` has nowhere to go up to when it is late");
    assert_eq!(
        notes(&o.diags[1]),
        [
            top,
            "`dept_head` comes to the team sales (Sales), whose head is empty; the stage is `when empty escalate, then board`, and it goes up.",
            expire,
            "Late with p01 (President (fictional)) for 3 requesters: p02 (Head of Sales), p03 (Manager of Sales 1) and p04 (Sales staff).",
            "Correct the data of the version; for it to go to someone else, say where it goes in the table of `when late escalate by <table>`."
        ],
        "{shown}"
    );
    // the shape of the organization: under `ok`
    let case = |versions: Vec<String>, cause: &str, notes: [&str; 2], requesters: &[(&str, Option<&str>)]| Case {
        what: "late".to_string(),
        versions,
        cause: cause.to_string(),
        notes: notes.iter().map(|n| n.to_string()).collect(),
        requesters: to(requesters),
    };
    let top = ritsu_base::text::capitalize(top);
    assert_eq!(
        late_cases(&o),
        [
            case(both.clone(), "Nobody is above the team finance (Finance) to go up to.", [to_board, expire], &[("p01", None)]),
            case(vec!["2026-04-01".to_string()], &top, [went_up, expire], &[("p05", Some("p01"))]),
            case(vec!["2027-04-01".to_string()], &top, [went_up, expire], &[("p02", Some("p01")), ("p05", Some("p01"))]),
        ],
        "{shown}"
    );
    // a table: `board` for the people of `board`, which sends the stage to `top`, and the president
    // has no president above; for the head the stage went up to it reads `dept_head`, which sends
    // the stage to finance
    let o = c.check("table.ringi", Options::default());
    let shown = common::shown(&o, Lang::En);
    assert_eq!(common::codes(&o), ["W502"], "{shown}");
    assert_eq!(
        late_cases(&o),
        [case(
            both,
            "The table `next_up` sends a late `board` to `top`, and `top` finds no head above the requester; nobody is above that either.",
            [to_board, "The stage is `when late escalate by next_up, then expire`: it cannot go up, and expires at the deadline."],
            &[("p01", None)]
        )],
        "{shown}"
    );
}

/// A request whose stages are `route`, with the ways `who`, `every stage` going up and skipped at
/// the top, the lines `ticket` under `ticket`, and the fields of `form`.
fn once_request(form: &str, uses: &str, route: &str, who: &str, ticket: &str) -> String {
    format!(
        "request leave v1 \"Leave\"\n\nuse organization org from \"company.ringi\"\n{uses}\nform\n  days : number \"Days\"  range >=1 <=20\n{form}\nroute\n{route}\nwho\n{who}\nevery stage\n  when empty escalate, then skip\n\nticket\n  self approval  not allowed\n{ticket}"
    )
}

const MANAGER_HEAD: &str = "  first   manager\n  second  head\n";
const MANAGER_HEAD_WHO: &str = "  manager = manager of requester\n  head    = head of requester.department\n  finance = team finance\n";
const ONCE: &str = "  same approver  approves once\n";

/// The W506 of an outcome, each with its versions and its notes.
fn w506(o: &ringi::check::Outcome) -> Vec<(String, Vec<String>)> {
    o.diags.iter().filter(|d| d.code == "W506").map(|d| (d.message.en.clone(), d.notes.iter().map(|n| n.en.clone()).collect())).collect()
}

/// `same approver approves once` (DESIGN 5.12): a stage whose one approver approved the stage right
/// before it is done with that approval, and W506 does not say the two. Not written, W506 says how
/// to write it where it would do the later stage. The manager of p03 (the head of the section) and
/// of p06 is the head of their department in 2026, and of p04 in 2027, when the section is gone; p02
/// and p05 head their departments themselves, and the stage goes up to the president, as the shape
/// of the organization does (counted under `ok` either way: `same approver approves once` does not
/// fill a stage).
#[test]
fn the_same_approver_approves_once() {
    let c = Company::new(
        "ringi-org-once",
        "",
        &[("bare.ringi", &once_request("", "", MANAGER_HEAD, MANAGER_HEAD_WHO, "")), ("once.ringi", &once_request("", "", MANAGER_HEAD, MANAGER_HEAD_WHO, ONCE))],
    );
    let bare = c.check("bare.ringi", Options::default());
    let said = w506(&bare);
    assert_eq!(said.len(), 2, "{}", common::shown(&bare, Lang::En));
    assert_eq!(said[0].0, "In the organization from 2026-04-01, one person answers two stages of a ticket");
    assert_eq!(
        said[0].1,
        [
            "the stage `first` is `manager` and the stage `second` is `head`, and both come to one person.",
            "For 1 requester, p02 (Head of Sales) answers both: p03 (Manager of Sales 1).",
            "For 1 requester, p05 (Head of Finance) answers both: p06 (Accountant).",
            "If one approval is enough, write `same approver approves once` under `ticket`: `second` is then done with the approval of `first`.",
            "One person who checks twice is not two pairs of eyes: change the way to choose one of the stages, or make sure it is meant.",
        ]
    );
    assert_eq!(said[1].1[1], "For 1 requester, p02 (Head of Sales) answers both: p04 (Sales staff).");
    let once = c.check("once.ringi", Options::default());
    assert!(w506(&once).is_empty(), "{}", common::shown(&once, Lang::En));
    // the stages that go up for lack of an approver are counted either way
    assert_eq!(shaped(&bare), shaped(&once));
    assert!(!shaped(&once).is_empty());
    // the Japanese of the note
    let ja = ringi::check::render(&bare, Lang::Ja);
    assert!(ja.contains("  = 一度の承認でよいなら、`ticket` に `same approver approves once` と書いてください。`second` は `first` の承認で済みます。\n"), "{ja}");
}

/// What `same approver approves once` does not do, and W506 says with why: a stage between that
/// someone else answers (a team of several, a person who is not the one), even one whose condition
/// may keep it from coming (the conditions are not read); another part of the earlier stage (`manager
/// and finance`); a later stage an agent answers. A circulation between is passed over.
#[test]
fn what_the_same_approver_does_not_do() {
    let between = once_request("", "", "  first   manager\n  check   finance\n  second  head\n", MANAGER_HEAD_WHO, ONCE);
    let when = once_request("", "", "  first   manager\n  check   finance  when days > 5\n  second  head\n", MANAGER_HEAD_WHO, ONCE);
    let told = once_request("", "", "  first   manager\n  tell    circulate to finance\n  second  head\n", MANAGER_HEAD_WHO, ONCE);
    let parts = once_request("", "", "  first   manager and finance\n  second  head\n", MANAGER_HEAD_WHO, ONCE);
    let agent = once_request(
        "  category : choice \"Category\"\n    travel   \"Travel\"\n    expense  \"Expense\"\n    purchase \"Purchase\"\n  amount   : money[JPY] \"Amount\"  range >=1JPY <=10_000_000JPY\n  purpose  : text \"Purpose\"  count >=1 <=100\n",
        "use flow precheck_agent from \"precheck.flow\"\n",
        "  first   manager\n  second  check\n",
        &format!("{MANAGER_HEAD_WHO}  check   = agent precheck_agent, else head\n"),
        ONCE,
    );
    let flow = std::fs::read_to_string("examples/shared/precheck.flow").unwrap();
    let c = Company::new(
        "ringi-org-once-not",
        "",
        &[("between.ringi", &between), ("when.ringi", &when), ("told.ringi", &told), ("parts.ringi", &parts), ("agent.ringi", &agent), ("precheck.flow", &flow)],
    );
    let why = |file: &str| -> Vec<String> {
        let o = c.check(file, Options::default());
        w506(&o).iter().map(|(_, notes)| notes.iter().find(|n| n.starts_with("`same approver approves once`")).cloned().unwrap_or_default()).collect()
    };
    let check_between = "`same approver approves once` does not do `second` with the approval of `first`: `check` comes between them.".to_string();
    assert_eq!(why("between.ringi"), [check_between.clone(), check_between.clone()]);
    assert_eq!(why("when.ringi"), [check_between.clone(), check_between]);
    assert!(why("told.ringi").is_empty());
    let first_waits = "`same approver approves once` does not do `second` with the approval of `first`: `first` waits for `finance` too.".to_string();
    assert_eq!(why("parts.ringi"), [first_waits.clone(), first_waits]);
    assert_eq!(why("agent.ringi"), ["`same approver approves once` does not do `second` with the approval of `first`: `second` is a stage an agent answers."; 2]);
    let ja = ringi::check::render(&c.check("between.ringi", Options::default()), Lang::Ja);
    assert!(ja.contains("  = `same approver approves once` でも、`second` は `first` の承認では済みません。あいだに `check` があります。\n"), "{ja}");
}

/// `simulate` says the stages `same approver approves once` does: on the route the form comes to, as
/// it is; without the form, sure where it is so on the route, and else where no other stage comes
/// between them. A stage that goes up to the one who approved the stage before is done so too. The
/// JSON names the stage whose approval does it.
#[test]
fn simulate_says_what_the_same_approver_does() {
    let when = once_request("", "", "  first   manager\n  check   finance  when days > 5\n  second  head\n", MANAGER_HEAD_WHO, ONCE);
    let c = Company::new("ringi-org-once-simulate", "", &[("once.ringi", &once_request("", "", MANAGER_HEAD, MANAGER_HEAD_WHO, ONCE)), ("when.ringi", &when)]);
    let simulate = |file: &str, who: &str, more: &[&str]| {
        let f = c.path(file).display().to_string();
        let mut args: Vec<String> = ["simulate", &f, "--requester", who, "--on", "2026-06-01"].iter().map(|s| s.to_string()).collect();
        args.extend(more.iter().map(|s| s.to_string()));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(ringi::run::run(&args, common::joined(), &mut out, &mut err), 0, "{}", String::from_utf8_lossy(&err));
        String::from_utf8(out).unwrap()
    };
    let done = "  second  head: p02 (Head of Sales); done with the approval of `first` (`same approver approves once`)\n";
    assert!(simulate("once.ringi", "p03", &[]).contains(done));
    let out = simulate("once.ringi", "p03", &["--form", "days = 3"]);
    assert!(out.ends_with(&format!("  first   manager: p02 (Head of Sales)\n{done}")), "{out}");
    // the stage that goes up: p02 heads the department, and it goes up to the president, who is
    // p02's manager
    assert!(simulate("once.ringi", "p02", &[]).contains("  second  head: it comes to the requester alone; it goes up to p01 (President (fictional)); done with the approval of `first` (`same approver approves once`)\n"));
    // a stage whose condition decides it between them
    let maybe = "  second  head: p02 (Head of Sales); done with the approval of `first` where no other stage comes between them\n";
    assert!(simulate("when.ringi", "p03", &[]).contains(maybe), "{}", simulate("when.ringi", "p03", &[]));
    assert!(simulate("when.ringi", "p03", &["--form", "days = 3"]).contains(done));
    assert!(simulate("when.ringi", "p03", &["--form", "days = 8"]).contains("  second  head: p02 (Head of Sales)\n"));
    // the JSON
    let out = simulate("when.ringi", "p03", &["--format", "json"]);
    let v = ritsu_base::json::parse(&out).unwrap();
    let stages = v.get("requesters").and_then(|x| x.as_arr()).unwrap()[0].get("stages").and_then(|x| x.as_arr()).unwrap();
    let second = stages.iter().find(|s| s.get("stage").and_then(|x| x.as_str()) == Some("second")).unwrap();
    let way = &second.get("ways").and_then(|x| x.as_arr()).unwrap()[0];
    assert_eq!(way.get("once_with").and_then(|x| x.as_str()), Some("first"));
    assert_eq!(way.get("once_if_no_stage_between").and_then(|x| x.as_bool()), Some(true));
}

const LAST: &str = "  same approver  approves once, at the last stage\n";

/// `same approver approves once, at the last stage` (DESIGN 5.12): a stage one person alone answers
/// waits for the last later stage the same person alone answers, whatever comes between them, and
/// W506 does not say the two; a way of a stage of several ways waits by itself. An agent's stage
/// is answered where it is, and W506 says the two with why. Where only `same approver approves
/// once` is written and does not do the later stage, or neither is, W506 says how the two are
/// answered once: the earlier stage waits for the later.
#[test]
fn the_same_approver_approves_once_at_the_last_stage() {
    const BETWEEN: &str = "  first   manager\n  check   finance\n  second  head\n";
    const PARTS: &str = "  first   manager and finance\n  second  head\n";
    const THREE: &str = "  first   manager\n  check   finance\n  second  head\n  third   boss\n";
    let boss = format!("{MANAGER_HEAD_WHO}  boss    = manager of requester\n");
    let agent = |ticket: &str| {
        once_request(
            "  category : choice \"Category\"\n    travel   \"Travel\"\n    expense  \"Expense\"\n    purchase \"Purchase\"\n  amount   : money[JPY] \"Amount\"  range >=1JPY <=10_000_000JPY\n  purpose  : text \"Purpose\"  count >=1 <=100\n",
            "use flow precheck_agent from \"precheck.flow\"\n",
            "  first   manager\n  second  check\n",
            &format!("{MANAGER_HEAD_WHO}  check   = agent precheck_agent, else head\n"),
            ticket,
        )
    };
    let flow = std::fs::read_to_string("examples/shared/precheck.flow").unwrap();
    let c = Company::new(
        "ringi-org-last",
        "",
        &[
            ("next_last.ringi", &once_request("", "", MANAGER_HEAD, MANAGER_HEAD_WHO, LAST)),
            ("between_last.ringi", &once_request("", "", BETWEEN, MANAGER_HEAD_WHO, LAST)),
            ("between_once.ringi", &once_request("", "", BETWEEN, MANAGER_HEAD_WHO, ONCE)),
            ("between_bare.ringi", &once_request("", "", BETWEEN, MANAGER_HEAD_WHO, "")),
            ("parts_last.ringi", &once_request("", "", PARTS, MANAGER_HEAD_WHO, LAST)),
            ("parts_once.ringi", &once_request("", "", PARTS, MANAGER_HEAD_WHO, ONCE)),
            ("three_once.ringi", &once_request("", "", THREE, &boss, ONCE)),
            ("agent_last.ringi", &agent(LAST)),
            ("agent_once.ringi", &agent(ONCE)),
            ("agent_bare.ringi", &agent("")),
            ("precheck.flow", &flow),
        ],
    );
    // the notes of each W506 after its cause and its requesters, and before the last
    let said = |file: &str| -> Vec<Vec<String>> {
        let o = c.check(file, Options::default());
        w506(&o).iter().map(|(_, notes)| notes.iter().filter(|n| n.contains("same approver approves once")).cloned().collect()).collect()
    };
    // written: the two are answered once, right after one another, with a stage between, and
    // where the earlier stage has another way
    for file in ["next_last.ringi", "between_last.ringi", "parts_last.ringi"] {
        assert!(said(file).is_empty(), "{file}: {:?}", said(file));
    }
    // `same approver approves once` alone does not do it: why, and the writing that does
    let not_done = "`same approver approves once` does not do `second` with the approval of `first`: `check` comes between them.";
    let once_at_second = "With `same approver approves once, at the last stage`, they answer once: `first` waits for the approval of `second`, after the stages between them.";
    assert_eq!(said("between_once.ringi"), [[not_done, once_at_second]; 2]);
    let first_waits = "`same approver approves once` does not do `second` with the approval of `first`: `first` waits for `finance` too.";
    assert_eq!(said("parts_once.ringi"), [[first_waits, once_at_second]; 2]);
    // neither is written
    assert_eq!(said("between_bare.ringi"), [["If one approval is enough, write `same approver approves once, at the last stage` under `ticket`: `first` then waits for the approval of `second`, after the stages between them."]; 2]);
    // three stages of one person: the first two wait for the third, and the two of them are said
    // with the last stage the person answers
    let three = c.check("three_once.ringi", Options::default());
    let notes: Vec<String> = w506(&three).iter().flat_map(|(_, notes)| notes.iter().filter(|n| n.starts_with("With `same approver")).cloned()).collect();
    assert!(notes.contains(&"With `same approver approves once, at the last stage`, they answer once: `first` waits for the approval of the last stage they answer.".to_string()), "{}", common::shown(&three, Lang::En));
    // an agent answers the later stage first: no writing makes the earlier stage wait for it
    assert_eq!(said("agent_last.ringi"), [["`same approver approves once, at the last stage` does not make `first` wait for `second`: `second` is a stage an agent answers first."]; 2]);
    assert_eq!(said("agent_once.ringi"), [["`same approver approves once` does not do `second` with the approval of `first`: `second` is a stage an agent answers."]; 2]);
    assert!(said("agent_bare.ringi").iter().all(|notes| notes.is_empty()), "{:?}", said("agent_bare.ringi"));
    // the Japanese of the notes
    let ja = ringi::check::render(&c.check("between_once.ringi", Options::default()), Lang::Ja);
    assert!(ja.contains("  = `same approver approves once, at the last stage` と書けば、一度の承認で済みます。`first` は、あいだのステップのあとの `second` の承認を待ちます。\n"), "{ja}");
    let ja = ringi::check::render(&c.check("between_bare.ringi", Options::default()), Lang::Ja);
    assert!(ja.contains("  = 一度の承認でよいなら、`ticket` に `same approver approves once, at the last stage` と書いてください。`first` は、あいだのステップのあとの `second` の承認を待ちます。\n"), "{ja}");
    let ja = ringi::check::render(&c.check("agent_last.ringi", Options::default()), Lang::Ja);
    assert!(ja.contains("  = `same approver approves once, at the last stage` でも、`first` は `second` を待ちません。`second` は、エージェントが先に答えるステップです。\n"), "{ja}");
    // the stages that go up for lack of an approver are counted either way
    assert_eq!(shaped(&c.check("between_last.ringi", Options::default())), shaped(&c.check("between_bare.ringi", Options::default())));
}

/// `simulate` says where a way answers once (DESIGN 10.3): on the route the form comes to, the
/// stage; without the form, sure where the later stage comes whatever the form, and else the
/// stages it can be. The way that waits has no deadline of its own. The JSON names the later stage
/// in `once_with`, and says whether it is so only where the stage comes.
#[test]
fn simulate_says_where_the_same_approver_answers_once() {
    let when = once_request("", "", "  first   manager\n  check   finance\n  second  head  when days > 5\n", MANAGER_HEAD_WHO, LAST);
    let two = once_request("", "", "  first   manager\n  second  head  when days > 5\n  check   finance\n  third   boss  when days > 10\n", &format!("{MANAGER_HEAD_WHO}  boss    = manager of requester\n"), LAST);
    let between = once_request("", "", "  first   manager\n  check   finance\n  second  head\n", MANAGER_HEAD_WHO, LAST);
    let c = Company::new("ringi-org-last-simulate", "", &[("between.ringi", &between), ("when.ringi", &when), ("two.ringi", &two)]);
    let simulate = |file: &str, who: &str, more: &[&str]| {
        let f = c.path(file).display().to_string();
        let mut args: Vec<String> = ["simulate", &f, "--requester", who, "--on", "2026-06-01"].iter().map(|s| s.to_string()).collect();
        args.extend(more.iter().map(|s| s.to_string()));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(ringi::run::run(&args, common::joined(), &mut out, &mut err), 0, "{}", String::from_utf8_lossy(&err));
        String::from_utf8(out).unwrap()
    };
    // sure: with the form and without it
    let once = "  first   manager: p02 (Head of Sales); answers once, at `second` (`same approver approves once, at the last stage`)\n";
    assert!(simulate("between.ringi", "p03", &[]).contains(once), "{}", simulate("between.ringi", "p03", &[]));
    assert!(simulate("between.ringi", "p03", &["--form", "days = 3"]).contains(once));
    assert!(simulate("between.ringi", "p03", &["--lang", "ja"]).contains("  first   manager：p02（営業部長）。`second` で一度だけ答えます（`same approver approves once, at the last stage`）\n"), "{}", simulate("between.ringi", "p03", &["--lang", "ja"]));
    // the later stage comes on a condition: said so without the form, and decided with it
    let maybe = "  first   manager: p02 (Head of Sales); answers once, at `second`, where it comes, and to them\n";
    assert!(simulate("when.ringi", "p03", &[]).contains(maybe), "{}", simulate("when.ringi", "p03", &[]));
    assert!(simulate("when.ringi", "p03", &["--lang", "ja"]).contains("  first   manager：p02（営業部長）。`second` が来て同じ人に当たるなら、そこで一度だけ答えます\n"));
    assert!(simulate("when.ringi", "p03", &["--form", "days = 8"]).contains(once));
    let out = simulate("when.ringi", "p03", &["--form", "days = 3"]);
    assert!(out.contains("  first  manager: p02 (Head of Sales)\n"), "{out}");
    // two later stages, each on a condition: the last of them that comes
    let among = "  first   manager: p02 (Head of Sales); answers once, at the last of `second` and `third` that comes, and to them\n";
    assert!(simulate("two.ringi", "p03", &[]).contains(among), "{}", simulate("two.ringi", "p03", &[]));
    assert!(simulate("two.ringi", "p03", &["--lang", "ja"]).contains("  first   manager：p02（営業部長）。`second`、`third` のうち、来て同じ人に当たる最後のステップで、一度だけ答えます\n"), "{}", simulate("two.ringi", "p03", &["--lang", "ja"]));
    let out = simulate("two.ringi", "p03", &["--form", "days = 12"]);
    assert!(out.contains("  first   manager: p02 (Head of Sales); answers once, at `third` (`same approver approves once, at the last stage`)\n") && out.contains("  second  head: p02 (Head of Sales); answers once, at `third` (`same approver approves once, at the last stage`)\n"), "{out}");
    // the JSON
    let ways = |file: &str, more: &[&str], stage: &str| {
        let mut args = vec!["--format", "json"];
        args.extend(more);
        let v = ritsu_base::json::parse(&simulate(file, "p03", &args)).unwrap();
        let stages = v.get("requesters").and_then(|x| x.as_arr()).unwrap()[0].get("stages").and_then(|x| x.as_arr()).unwrap().to_vec();
        let st = stages.iter().find(|s| s.get("stage").and_then(|x| x.as_str()) == Some(stage)).unwrap().clone();
        st.get("ways").and_then(|x| x.as_arr()).unwrap()[0].clone()
    };
    let keys = |w: &ritsu_base::json::Json| ["once_with", "once_with_written", "once_if_no_stage_between", "once_if_it_comes", "once_among", "once_among_written"].map(|k| w.get(k).map(|x| x.compact()).unwrap_or_default()).join(" ");
    assert_eq!(keys(&ways("between.ringi", &[], "first")), r#""second" "second" false false [] []"#);
    assert_eq!(keys(&ways("when.ringi", &[], "first")), r#""second" "second" false true ["second"] ["second"]"#);
    assert_eq!(keys(&ways("two.ringi", &[], "first")), r#""third" "third" false true ["second","third"] ["second","third"]"#);
    assert_eq!(keys(&ways("two.ringi", &["--form", "days = 8"], "first")), r#""second" "second" false false [] []"#);
    assert_eq!(keys(&ways("between.ringi", &[], "second")), "null null false false [] []");
}

/// The JSON of `simulate` says the same keys of a way whatever the request writes under `ticket`
/// (DESIGN 16.2): with `same approver approves once`, and with no line of the same approver, a way
/// has `once_if_it_comes`, `once_among` and `once_among_written` too, as `false` and two empty
/// lists, since only `, at the last stage` makes a way wait for a stage that may not come.
#[test]
fn the_json_of_simulate_has_the_same_keys_of_a_way_whatever_the_ticket_says() {
    const NEXT: &str = "  first   manager\n  second  head\n";
    const BETWEEN: &str = "  first   manager\n  check   finance\n  second  head  when days > 5\n";
    let files: Vec<(String, String)> = [("last", LAST), ("once", ONCE), ("bare", "")]
        .iter()
        .flat_map(|(name, line)| [(format!("next_{name}.ringi"), once_request("", "", NEXT, MANAGER_HEAD_WHO, line)), (format!("between_{name}.ringi"), once_request("", "", BETWEEN, MANAGER_HEAD_WHO, line))])
        .collect();
    let listed: Vec<(&str, &str)> = files.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let c = Company::new("ringi-org-simulate-keys", "", &listed);
    // the ways of every stage, in the order of the route, with the form or without it
    let ways = |file: &str, more: &[&str]| -> Vec<ritsu_base::json::Json> {
        let f = c.path(file).display().to_string();
        let mut args: Vec<String> = ["simulate", &f, "--requester", "p03", "--on", "2026-06-01", "--format", "json"].iter().map(|s| s.to_string()).collect();
        args.extend(more.iter().map(|s| s.to_string()));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(ringi::run::run(&args, common::joined(), &mut out, &mut err), 0, "{}", String::from_utf8_lossy(&err));
        let v = ritsu_base::json::parse(&String::from_utf8(out).unwrap()).unwrap();
        let stages = v.get("requesters").and_then(|x| x.as_arr()).unwrap()[0].get("stages").and_then(|x| x.as_arr()).unwrap().to_vec();
        stages.iter().flat_map(|st| st.get("ways").and_then(|x| x.as_arr()).unwrap().to_vec()).collect()
    };
    let names = |w: &ritsu_base::json::Json| w.as_obj().unwrap().iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
    let once = |w: &ritsu_base::json::Json| ["once_with", "once_with_written", "once_if_no_stage_between"].map(|k| w.get(k).map(|x| x.compact()).unwrap_or_default()).join(" ");
    let more = |w: &ritsu_base::json::Json| ["once_if_it_comes", "once_among", "once_among_written"].map(|k| w.get(k).map(|x| x.compact()).unwrap_or_default()).join(" ");
    // one list of keys, in one order, in every file
    let all = names(&ways("next_last.ringi", &[])[0]);
    assert_eq!(all, ["way", "way_written", "case", "people", "needed", "proxies", "nobody", "once_with", "once_with_written", "once_if_no_stage_between", "once_if_it_comes", "once_among", "once_among_written", "late"]);
    for (file, _) in &files {
        for form in [&[][..], &["--form", "days = 3"][..], &["--form", "days = 8"][..]] {
            let ws = ways(file, form);
            assert!(!ws.is_empty(), "{file}");
            for w in &ws {
                assert_eq!(names(w), all, "{file} {form:?}");
            }
            // without `, at the last stage`, the three are `false` and empty in every way
            if !file.ends_with("_last.ringi") {
                for w in &ws {
                    assert_eq!(more(w), "false [] []", "{file} {form:?}: {}", w.compact());
                }
            }
        }
    }
    // what the three keys that were there say is what they said: the line without `, at the last
    // stage` does the later of two stages next to one another, and nothing with a stage between
    assert_eq!(ways("next_once.ringi", &[]).iter().map(once).collect::<Vec<_>>(), ["null null false", r#""first" "first" false"#]);
    assert_eq!(ways("between_once.ringi", &[]).iter().map(once).collect::<Vec<_>>(), ["null null false", "null null false", "null null false"]);
    assert_eq!(ways("next_bare.ringi", &[]).iter().map(once).collect::<Vec<_>>(), ["null null false", "null null false"]);
    assert_eq!(ways("between_bare.ringi", &[]).iter().map(once).collect::<Vec<_>>(), ["null null false", "null null false", "null null false"]);
    // and `, at the last stage` says them where a way waits for a stage that may not come
    assert_eq!(ways("between_last.ringi", &[]).iter().map(more).collect::<Vec<_>>(), [r#"true ["second"] ["second"]"#, "false [] []", "false [] []"]);
    assert_eq!(ways("next_last.ringi", &[]).iter().map(more).collect::<Vec<_>>(), ["false [] []", "false [] []"]);
}

/// A request whose one stage is `team finance`, which has people for every requester in both
/// versions, with `submitted by <line>` under `ticket`.
fn submitted(line: &str) -> String {
    once_request("", "", "  check   finance\n", MANAGER_HEAD_WHO, &format!("  submitted by   {line}\n"))
}

/// `simulate --every-requester --format json` of `file` on `day`: the people who may not submit, and
/// every requester with the one who makes the request for them.
fn every_requester(c: &Company, file: &str, day: &str) -> (Vec<String>, Vec<(String, Option<String>)>) {
    let f = c.path(file).display().to_string();
    let args: Vec<String> = ["simulate", &f, "--every-requester", "--on", day, "--format", "json"].iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(ringi::run::run(&args, common::joined(), &mut out, &mut err), 0, "{}{}", String::from_utf8_lossy(&out), String::from_utf8_lossy(&err));
    let v = ritsu_base::json::parse(&String::from_utf8(out).unwrap()).unwrap();
    let not = v.get("may_not_submit").and_then(|x| x.as_arr()).unwrap().iter().filter_map(|x| x.as_str()).map(str::to_string).collect();
    let rs = v.get("requesters").and_then(|x| x.as_arr()).unwrap().iter().map(|r| (r.get("requester").and_then(|x| x.as_str()).unwrap().to_string(), r.get("made_by").and_then(|x| x.as_str()).map(str::to_string))).collect();
    (not, rs)
}

/// `submitted by` (DESIGN 5.13), each kind of group in each version: `everyone in` takes the teams
/// below, `team` the members of the team itself (a second membership too), a position the one in
/// the primary team, `or higher` the positions above it, `in` the teams the primary team is in, and
/// `except` leaves people out. The people of 2026: p01 the president, p02 the head of sales, p03 the
/// manager of its section (and staff in finance), p04 staff of the section, p05 the head of finance,
/// p06 its staff; in 2027 the section is gone, p04 is in sales, and p03 and p06 have left.
#[test]
fn who_may_submit() {
    let cases: [(&str, &[&str], &[&str]); 6] = [
        ("everyone except team finance", &["p03", "p05", "p06"], &["p05"]),
        ("everyone in sales", &["p01", "p05", "p06"], &["p01", "p05"]),
        ("team sales", &["p01", "p03", "p04", "p05", "p06"], &["p01", "p05"]),
        ("position manager or higher", &["p04", "p06"], &["p04"]),
        ("position manager or higher in sales", &["p01", "p04", "p05", "p06"], &["p01", "p04", "p05"]),
        ("person \"p04\", position president", &["p02", "p03", "p05", "p06"], &["p02", "p05"]),
    ];
    let files: Vec<(String, String)> = cases.iter().enumerate().map(|(i, (line, ..))| (format!("r{i}.ringi"), submitted(line))).collect();
    let refs: Vec<(&str, &str)> = files.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let c = Company::new("ringi-org-submit", "", &refs);
    for (i, (line, not_2026, not_2027)) in cases.iter().enumerate() {
        let file = format!("r{i}.ringi");
        let o = c.check(&file, Options::default());
        assert!(!o.has_errors(), "{line}: {}", common::shown(&o, Lang::En));
        for (day, want) in [("2026-06-01", not_2026), ("2027-06-01", not_2027)] {
            let (not, _) = every_requester(&c, &file, day);
            assert_eq!(not, *want, "{line} on {day}");
        }
    }
    // what `ok` says, in both languages and in the JSON
    let o = c.check("r0.ringi", Options::default());
    let fact = "submitted by 3 of 6 people in the organization from 2026-04-01, and 3 of 4 from 2027-04-01 (`everyone except team finance`)";
    assert_eq!(o.facts.last().map(|f| f.text.en.as_str()), Some(fact));
    assert_eq!(o.facts.last().map(|f| f.text.ja.as_str()), Some("出せる人: 2026-04-01 からの組織の 6 人のうち 3 人、2027-04-01 からの 4 人のうち 3 人（`everyone except team finance`）"));
    let o = c.check("r0.ringi", at("2027-04-01"));
    assert_eq!(o.facts.last().map(|f| f.text.en.as_str()), Some("submitted by 3 of 4 people in the organization from 2027-04-01 (`everyone except team finance`)"));
}

/// The listing says what comes to the requesters who may submit alone: the president, who has no
/// manager, is left out, and the request passes; `simulate` refuses them, and a request made for
/// someone is theirs (the proxy is not looked at, and makes none for someone who may not submit).
#[test]
fn the_requesters_are_the_ones_who_may_submit() {
    let staff = LEAVE.replace("ticket\n", "ticket\n  submitted by   everyone except position president\n");
    let proxies = "person,proxy,kind,from,to,requests\np04,p06,submit,2026-04-01,2027-03-31,\np05,p04,submit,2026-04-01,,\n";
    let c = Company::new("ringi-org-submit-listing", proxies, &[("leave.ringi", LEAVE), ("staff.ringi", &staff), ("finance.ringi", &submitted("everyone except team finance"))]);
    assert_eq!(common::codes(&c.check("leave.ringi", Options::default())), ["E502"]);
    let o = c.check("staff.ringi", Options::default());
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    // simulate: refused, as one the organization does not have is
    let f = c.path("staff.ringi").display().to_string();
    let args: Vec<String> = ["simulate", &f, "--requester", "p01", "--on", "2026-06-01"].iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(ringi::run::run(&args, common::joined(), &mut out, &mut err), 2);
    assert_eq!(String::from_utf8(err).unwrap(), "error: p01 (President (fictional)) may not submit staff.ringi in the organization from 2026-04-01 (`submitted by everyone except position president`)\n");
    // the proxies: p06 makes requests for p04, who may submit; p04's for p05 are not made
    let (_, rs) = every_requester(&c, "finance.ringi", "2026-06-01");
    let made: Vec<(String, Option<String>)> = rs.into_iter().filter(|(_, by)| by.is_some()).collect();
    assert_eq!(made, [("p04".to_string(), Some("p06".to_string()))]);
}

/// A team or a person `submitted by` names that a version does not have is E508 at where it is
/// written, and that version has no E509; a version no one may submit in is E509 at the line, the
/// versions together; `--at` a day of the version that has them passes.
#[test]
fn what_submitted_by_names_is_in_each_version() {
    let c = Company::new(
        "ringi-org-submit-gone",
        "",
        &[("section.ringi", &submitted("team sales1")), ("p03.ringi", &submitted("person \"p03\", person \"p02\"")), ("manager.ringi", &submitted("position manager")), ("nobody.ringi", &submitted("everyone except everyone"))],
    );
    let o = c.check("section.ringi", Options::default());
    assert_eq!(messages(&o), [("E508", "In the organization from 2027-04-01, `submitted by` names the team `sales1`, which is not there".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!((o.diags[0].line, o.diags[0].col), (Some(21), Some(23)));
    let o = c.check("p03.ringi", Options::default());
    assert_eq!(messages(&o), [("E508", "In the organization from 2027-04-01, `submitted by` names p03, who is not there".to_string())]);
    let o = c.check("manager.ringi", Options::default());
    assert_eq!(messages(&o), [("E509", "In the organization from 2027-04-01, no one may submit the request".to_string())]);
    assert_eq!(o.diags[0].notes[0].en, "`position manager` comes to nobody in the organization from 2027-04-01.");
    assert_eq!(o.diags[0].notes[0].ja, "2027-04-01 からの組織では、`position manager` に当たる人がいません。");
    let o = c.check("nobody.ringi", Options::default());
    assert_eq!(messages(&o), [("E509", "In the versions of the organization from 2026-04-01 and 2027-04-01, no one may submit the request".to_string())]);
    // the version in effect now passes: what `ok` says of it
    for f in ["section.ringi", "p03.ringi", "manager.ringi"] {
        let o = c.check(f, at("2026-11-16"));
        assert!(o.diags.is_empty(), "{f}: {}", common::shown(&o, Lang::En));
    }
    assert_eq!(c.check("manager.ringi", at("2026-11-16")).facts.last().map(|f| f.text.en.clone()), Some("submitted by 1 of 6 people in the organization from 2026-04-01 (`position manager`)".to_string()));
    // the JSON's witness: the versions and the group
    let o = c.check("section.ringi", Options::default());
    let j = ringi::check::to_json(&o, c.path("").as_path(), Lang::En);
    let w = j.get("diagnostics").and_then(|x| x.as_arr()).unwrap()[0].get("witness").unwrap().compact();
    assert_eq!(w, r#"{"versions":["2027-04-01"],"group":"team sales1"}"#);
}

/// The data is named as the file writes it, or as `--org` gives it; a finding in the data is said
/// at the data's file, as the path the organization's file was given by reaches it.
#[test]
fn the_data_is_named_as_written() {
    let c = Company::new("ringi-org-paths", "", &[]);
    let o = c.check("company.ringi", Options::default());
    assert!(o.facts[0].text.en.starts_with("data `org`: "), "{}", o.facts[0].text.en);
    let dir = c.path("org").display().to_string();
    let o = c.check("company.ringi", Options { org: Some(dir.clone()), ..Options::default() });
    assert!(o.facts[0].text.en.starts_with(&format!("data `{dir}`: ")), "{}", o.facts[0].text.en);
    // relative to where the check runs: the same findings, the files named from there
    let bad = Company::new("ringi-org-paths-bad", "", &[("org/2026-04-01/teams.csv", &TEAMS_2026.replace(",section,", ",unit,"))]);
    let abs = bad.check("company.ringi", Options::default());
    let rel_path = ritsu_base::paths::between(&std::env::current_dir().unwrap(), &bad.path("company.ringi"));
    let rel = Run::new(common::joined()).file(&rel_path).unwrap();
    assert_eq!(common::codes(&abs), ["E501"]);
    assert_eq!(common::codes(&rel), ["E501"]);
    assert_eq!(abs.diags[0].message.en, rel.diags[0].message.en);
    assert_eq!(abs.diags[0].file, bad.path("org/2026-04-01/teams.csv").display().to_string());
    assert!(rel.diags[0].file.ends_with("org/2026-04-01/teams.csv") && !rel.diags[0].file.starts_with('/'), "{}", rel.diags[0].file);
}

/// A person's language is `en`, `ja` or empty, in `people.csv` and in `organization.json` alike;
/// any other value is E501 at its row, in English and in Japanese, and the version is not read.
#[test]
fn a_persons_language_is_en_ja_or_none() {
    let people = people_with("language", &[("p01", "ja"), ("p02", "en"), ("p03", "")]);
    let c = Company::new("ringi-org-language", "", &[("leave.ringi", LEAVE), ("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    // the same in the one JSON file of a version
    let json = r#"{"teams": [{"id": "company", "name": "Example Co. (fictional)", "kind": "company", "parent": null, "head": "p01"}],
"people": [{"id": "p01", "name": "President (fictional)", "language": "ja"}, {"id": "p02", "name": "Head of Sales", "language": null}],
"members": [{"person": "p01", "team": "company", "position": "president", "primary": true}, {"person": "p02", "team": "company", "position": "dept_head", "primary": true}]}
"#;
    let j = Company::new("ringi-org-language-json", "", &[]);
    for f in ["teams.csv", "people.csv", "members.csv"] {
        std::fs::remove_file(j.path(&format!("org/2027-04-01/{f}"))).unwrap();
    }
    j.write("org/2027-04-01/organization.json", json);
    let o = j.check("company.ringi", Options::default());
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    j.write("org/2027-04-01/organization.json", &json.replace(r#""language": "ja""#, r#""language": "en-GB""#));
    let o = j.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The `language` of the person `p01` is `en` or `ja`, not `en-GB`".to_string())], "{}", common::shown(&o, Lang::En));
    // a value of neither: at its row, the version left unread
    let bad = Company::new("ringi-org-language-bad", "", &[("leave.ringi", LEAVE), ("org/2026-04-01/people.csv", &people.replace("p02,Head of Sales,営業部長,en\n", "p02,Head of Sales,営業部長,fr\n"))]);
    let o = bad.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The `language` of the person `p02` is `en` or `ja`, not `fr`".to_string())], "{}", common::shown(&o, Lang::En));
    let d = &o.diags[0];
    assert_eq!(d.message.ja, "人 `p02` の `language` は `en` か `ja` で書きます。`fr` ではありません");
    assert_eq!(d.line, Some(3));
    assert!(d.file.ends_with("people.csv"), "{}", d.file);
    assert_eq!(notes(d), ["It is the language the person reads notices and screens in; left empty, each kind's own language is read."]);
    assert_eq!(d.notes.iter().map(|n| n.ja.as_str()).collect::<Vec<_>>(), ["その人が知らせと画面を読む言語です。空なら、申請の種類ごとに、その種類の言語で読みます。"]);
    let o = bad.check("leave.ringi", Options::default());
    assert_eq!(common::codes(&o), ["E202"], "{}", common::shown(&o, Lang::En));
}

/// A value of `organization.json` is a string or null, as `organization/schema.json` says, and
/// `primary` a truth value too (DESIGN 8.2): a number, or a truth value of another key, is E501 at
/// its element, in English and in Japanese, and the version is not read.
#[test]
fn a_value_of_organization_json_is_a_string_or_null() {
    let json = r#"{"teams": [{"id": "company", "name": "Example Co. (fictional)", "kind": "company", "parent": null, "head": "p01"}],
"people": [{"id": "p01", "name": "President (fictional)"}, {"id": "p02", "name": "Head of Sales", "email": null}],
"members": [{"person": "p01", "team": "company", "position": "president", "primary": true}, {"person": "p02", "team": "company", "position": "dept_head", "primary": "yes"}]}
"#;
    let j = Company::new("ringi-org-json-values", "", &[]);
    for f in ["teams.csv", "people.csv", "members.csv"] {
        std::fs::remove_file(j.path(&format!("org/2027-04-01/{f}"))).unwrap();
    }
    let check = |text: &str| {
        j.write("org/2027-04-01/organization.json", text);
        j.check("company.ringi", Options::default())
    };
    let o = check(json);
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    for (from, to, said) in [
        (r#""head": "p01""#, r#""head": 1"#, "The `head` of the element 1 of `teams` is to be a string or null, not `1`"),
        (r#""email": null"#, r#""email": false"#, "The `email` of the element 2 of `people` is to be a string or null, not `false`"),
        (r#""primary": "yes""#, r#""primary": 1"#, "The `primary` of the element 2 of `members` is to be a truth value, a string or null, not `1`"),
    ] {
        let o = check(&json.replace(from, to));
        assert_eq!(messages(&o), [("E501", said.to_string())], "{}", common::shown(&o, Lang::En));
    }
    let o = check(&json.replace(r#""head": "p01""#, r#""head": 1"#));
    let d = &o.diags[0];
    assert_eq!(d.message.ja, "`teams` の 1 番目の `head` は、文字列か null にしてください。`1` ではありません");
    assert!(d.file.ends_with("organization.json") && d.line == Some(1), "{} {:?}", d.file, d.line);
}

/// The column of a master's field is named as the field is written or by its ASCII name (DESIGN
/// 4.6): an optional field may have no column, and each row's value is then empty; a field in two
/// columns, one of each name, is E501 naming both, and the data is not read.
#[test]
fn a_masters_columns_are_its_fields() {
    let masters = "masters company_masters v1\n\nmaster centres \"Cost centres\"\n  key centre\n  fields\n    centre : text \"Centre\"  count <=10\n    担当(owner) : text \"Owner\"  count <=10  optional\n  data \"centres.csv\"\n";
    let c = Company::new("ringi-org-master-columns", "", &[("masters.ringi", masters), ("centres.csv", "centre,valid_from,valid_to\nC1,2026-04-01,\nC2,,\n")]);
    let o = c.check("masters.ringi", Options::default());
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let Some(ringi::org::Data::Masters(m)) = o.data.as_deref() else { panic!("{}", common::shown(&o, Lang::En)) };
    let data = m.master("centres").unwrap();
    assert_eq!(data.rows.iter().map(|r| (r.key.as_str(), data.value(r, "担当"))).collect::<Vec<_>>(), [("C1", Some("")), ("C2", Some(""))]);
    // a field that is not optional needs its column
    let c = Company::new("ringi-org-master-columns-needed", "", &[("masters.ringi", &masters.replace("  optional\n", "\n")), ("centres.csv", "centre,valid_from,valid_to\nC1,2026-04-01,\n")]);
    let o = c.check("masters.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "There is no column for the field `担当` of the master `centres`".to_string())], "{}", common::shown(&o, Lang::En));
    // either name is read; both are refused
    for head in ["centre,担当,valid_from,valid_to", "centre,owner,valid_from,valid_to"] {
        let c = Company::new("ringi-org-master-columns-one", "", &[("masters.ringi", masters), ("centres.csv", &format!("{head}\nC1,Kato,2026-04-01,\n"))]);
        let o = c.check("masters.ringi", Options::default());
        assert!(o.diags.is_empty(), "{head}: {}", common::shown(&o, Lang::En));
        let Some(ringi::org::Data::Masters(m)) = o.data.as_deref() else { panic!("{head}") };
        let data = m.master("centres").unwrap();
        assert_eq!(data.value(&data.rows[0], "担当"), Some("Kato"), "{head}");
    }
    let c = Company::new("ringi-org-master-columns-two", "", &[("masters.ringi", masters), ("centres.csv", "centre,担当,owner,valid_from,valid_to\nC1,Kato,Sato,2026-04-01,\n")]);
    let o = c.check("masters.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The field `担当` of the master `centres` has two columns, `担当` and `owner`".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[0].message.ja, "マスタ `centres` のフィールド `担当` の列が、`担当` と `owner` の二つあります");
    assert_eq!(notes(&o.diags[0]), ["Keep one column, by either name."]);
}

/// A person's `email` may be empty, or is at most 254 bytes and, trimmed of the space around it,
/// one `@` with something before it and after it, and no white space and none of `"(),:;<>[\]`, as
/// the server's checks hold it (DESIGN 8.4): a value that is not one is E501 at the row, in English
/// and in Japanese, and the version is not read.
#[test]
fn a_persons_email_is_one_address() {
    let people = |email: &str| people_with("email", &[("p02", email)]);
    let check = |name: &str, email: &str| {
        let c = Company::new(name, "", &[("org/2026-04-01/people.csv", &people(email))]);
        c.check("company.ringi", Options::default())
    };
    // empty, and one address (the space around it, kept by a quoted field, is trimmed)
    let o = check("ringi-org-address-empty", "");
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    let o = check("ringi-org-address-one", "\" p02@example.com \"");
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    // every shape that is not one address
    let no_address = "an address has one `@`, something before it and after it, and no space and none of `\"(),:;<>[\\]`";
    // (`"a"b@c"` — a character after a closing quote — the file does not read as CSV at all)
    for (i, email) in ["no-at.example.com", "@example.com", "p02@", "a@@b", "\"a b@example.com\"", "a(b@c", "a)b@c", "\"a,b@c\"", "a:b@c", "a;b@c", "a<b@c", "a>b@c", "a[b@c", "a]b@c", "a\\b@c"].iter().enumerate() {
        let o = check(&format!("ringi-org-address-no-{i}"), email);
        let [("E501", ref m)] = messages(&o)[..] else { panic!("{email}: {}", common::shown(&o, Lang::En)) };
        assert!(m.starts_with("The `email` of the person `p02`, `") && m.contains("`, is not written as an address: ") && m.ends_with(no_address), "{email}: {m}");
    }
    // a character the data may not hold is said by its own check, before the address's shape
    let o = check("ringi-org-address-unfit", "\"a\tb@example.com\"");
    assert_eq!(messages(&o), [("E501", "The `email` of the person `p02` holds a control character (U+0009)".to_string())], "{}", common::shown(&o, Lang::En));
    let o = check("ringi-org-address-shape", "\"a b@example.com\"");
    let d = &o.diags[0];
    assert_eq!(d.message.en, "The `email` of the person `p02`, `a b@example.com`, is not written as an address: an address has one `@`, something before it and after it, and no space and none of `\"(),:;<>[\\]`");
    assert_eq!(d.message.ja, "人 `p02` の `email`（`a b@example.com`）が、アドレスの形ではありません。アドレスは、`@` が一つで、その前と後に文字がある形です。空白と `\"(),:;<>[\\]` の文字は使えません");
    assert_eq!(d.line, Some(3));
    assert!(d.file.ends_with("people.csv"), "{}", d.file);
    // 255 bytes is one past the most (the count is of bytes, not of characters)
    let long = format!("{}@x.co", "a".repeat(250));
    assert_eq!(long.len(), 255);
    let o = check("ringi-org-address-long", &format!("\"{long}\""));
    assert_eq!(messages(&o), [("E501", "The `email` of the person `p02` is 255 bytes long; an address is 254 bytes at the most".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[0].message.ja, "人 `p02` の `email` が255バイトあります。アドレスは254バイトまでです");
}

/// Two people of a version may not share an address, compared as a person signs in by it: trimmed
/// of the space around it, and folded the way Go's `strings.EqualFold` folds (U+212A is `k`,
/// U+017F is `s`): the later row is E501 naming the earlier one.
#[test]
fn two_people_may_not_share_an_address() {
    // p02 `p02@example.com` and p03 `" P02@EXAMPLE.COM "`: trimmed and folded, one address
    let people = people_with("email", &[("p02", "p02@example.com"), ("p03", "\" P02@EXAMPLE.COM \"")]);
    let c = Company::new("ringi-org-address-two", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The `email` of the person `p03` (`P02@EXAMPLE.COM`) is the address of the person `p02` (line 3 too): people sign in by their addresses, and two may not share one".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[0].message.ja, "人 `p03` の `email`（`P02@EXAMPLE.COM`）が、人 `p02` のアドレスと同じです（3 行目にも）。アドレスでサインインするので、二人で同じアドレスは使えません");
    assert_eq!(o.diags[0].line, Some(4));
    // the Kelvin sign folds to `k`, the long s to `s`: `K…` and `K…` are one, `s…` and `ſ…` are one
    let people = people.replace("\" P02@EXAMPLE.COM \"", "\u{212a}02@example.com").replace("p02@example.com", "K02@example.com");
    let c = Company::new("ringi-org-address-kelvin", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    let [("E501", ref m)] = messages(&o)[..] else { panic!("{}", common::shown(&o, Lang::En)) };
    assert!(m.contains("is the address of the person `p02`"), "{m}");
    // two different addresses, however like, pass
    let people = people.replace("\u{212a}02@example.com", "k03@example.com");
    let c = Company::new("ringi-org-address-other", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
}

/// An ID, a name, an address or a master's value holds none of the characters the server refuses
/// (`UnfitChar`: Unicode's Cc, Cf — ZWJ, ZWNJ, the byte order mark among them —, Zl and Zp): the
/// first such character of a value is E501 at the row, named by what it is, in English and in
/// Japanese.
#[test]
fn the_data_holds_no_character_it_may_not() {
    // an ID with the zero-width joiner, a name with a control character and one that turns the
    // direction of the text, a `name_ja` that separates lines or paragraphs
    let people = PEOPLE_2026
        .replace("p02,Head of Sales,営業部長\n", "p\u{200d}02,Head of Sales,営業部長\n")
        .replace("p04,Sales staff,営業担当\n", "p04,Sales \u{7}staff,営業\u{2029}担当\n")
        .replace("p05,Head of Finance,財務部長\n", "p05,Head \u{202e}of Finance,財務部長\n");
    let teams = TEAMS_2026.replace("sales,Sales,営業部,department,company,p02", "sales,Sales,営業\u{feff}部,department,company,p02");
    let c = Company::new("ringi-org-unfit", "", &[("org/2026-04-01/people.csv", &people), ("org/2026-04-01/teams.csv", &teams)]);
    let o = c.check("company.ringi", Options::default());
    let got: Vec<&str> = o.diags.iter().map(|d| d.message.en.as_str()).collect();
    for want in [
        // the ID the diagnostic writes is escaped, so the zero-width joiner shows as `\u200D`
        "The `id` of the person `p\\u200D02` holds U+200D, a formatting character that does not show",
        "The `name` of the person `p04` holds a control character (U+0007)",
        "The `name_ja` of the person `p04` holds U+2029, a character that separates lines or paragraphs",
        "The `name` of the person `p05` holds U+202E, a character that turns the direction of the text after it",
        "The `name_ja` of the team `sales` holds U+FEFF, a formatting character that does not show",
    ] {
        assert!(got.contains(&want), "not found: {want}\n{}", got.join("\n"));
    }
    let jas: Vec<&str> = o.diags.iter().map(|d| d.message.ja.as_str()).collect();
    for want in ["目に見えない書式の文字（U+200D）", "制御文字（U+0007）", "行や段落を区切る文字（U+2029）", "あとに続く文字の向きを変える文字（U+202E）", "目に見えない書式の文字（U+FEFF）"] {
        assert!(jas.iter().any(|j| j.contains(want)), "not found: {want}\n{}", jas.join("\n"));
    }
    // a character of the same kinds in an address is E501, not one of an address's shape
    let people = people_with("email", &[("p02", "a\u{200c}b@example.com")]);
    let c = Company::new("ringi-org-unfit-email", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The `email` of the person `p02` holds U+200C, a formatting character that does not show".to_string())], "{}", common::shown(&o, Lang::En));
    // a master's value with one
    let masters = "masters company_masters v1\n\nmaster centres \"Cost centres\"\n  key centre\n  fields\n    centre : text \"Centre\"  count <=10\n    owner : text \"Owner\"  count <=10\n  data \"centres.csv\"\n";
    let c = Company::new("ringi-org-unfit-master", "", &[("masters.ringi", masters), ("centres.csv", "centre,owner,valid_from,valid_to\nC1,K\u{200b}ato,,\n")]);
    let o = c.check("masters.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The `owner` of the row holds U+200B, a formatting character that does not show".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[0].message.ja, "行の `owner` に目に見えない書式の文字（U+200B）があります");
}

/// A name of a person or a team is 200 characters at the most, counted in Unicode's code points:
/// a longer one is E501 at the row.
#[test]
fn a_name_is_two_hundred_characters_at_the_most() {
    let name = "x".repeat(201);
    let people = PEOPLE_2026.replace("Head of Sales", &name);
    let teams = TEAMS_2026.replace("財務部", &"財".repeat(201));
    let c = Company::new("ringi-org-long-names", "", &[("org/2026-04-01/people.csv", &people), ("org/2026-04-01/teams.csv", &teams)]);
    let o = c.check("company.ringi", Options::default());
    let got: Vec<&str> = o.diags.iter().map(|d| d.message.en.as_str()).collect();
    assert!(got.contains(&"The `name` of the person `p02` is 201 characters long; a name is 200 characters at the most"), "{}", got.join("\n"));
    assert!(got.contains(&"The `name_ja` of the team `finance` is 201 characters long; a name is 200 characters at the most"), "{}", got.join("\n"));
    // 200 characters pass, a name of two bytes a character counted once each
    let people = people.replace(&name, &"é".repeat(200));
    let teams = teams.replace(&"財".repeat(201), &"財".repeat(200));
    let c = Company::new("ringi-org-names-at-most", "", &[("org/2026-04-01/people.csv", &people), ("org/2026-04-01/teams.csv", &teams)]);
    let o = c.check("company.ringi", Options::default());
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
}

/// A value of a master's row, its key excepted, is 1,000 characters at the most, counted in
/// Unicode's code points: a longer one is E501 at the row. The key's own most is 255 bytes, as an
/// ID's is (the server's `longestID`).
#[test]
fn a_value_of_a_master_is_a_thousand_characters_at_the_most() {
    let masters = "masters company_masters v1\n\nmaster centres \"Cost centres\"\n  key centre\n  fields\n    centre : text \"Centre\"  count <=1500\n    owner : text \"Owner\"  count <=1500\n  data \"centres.csv\"\n";
    let value = "x".repeat(1001);
    let c = Company::new("ringi-org-long-value", "", &[("masters.ringi", masters), ("centres.csv", &format!("centre,owner,valid_from,valid_to\nC1,{value},,\n"))]);
    let o = c.check("masters.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The `owner` of the row is 1001 characters long; a value of a master is 1000 characters at the most".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[0].message.ja, "行の `owner` が1001字あります。マスタの値は1000字までです");
    // the key's own most is 255 bytes, not 1,000 characters: a key of 1,001 bytes is E501 (said
    // cut to forty characters)
    let key = "K".repeat(1001);
    let shown = format!("{}…", "K".repeat(40));
    let c = Company::new("ringi-org-long-key", "", &[("masters.ringi", masters), ("centres.csv", &format!("centre,owner,valid_from,valid_to\n{key},Kato,,\n"))]);
    let o = c.check("masters.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", format!("The key `{shown}` of the master `centres` is 1001 bytes long; a key is 255 bytes at the most"))], "{}", common::shown(&o, Lang::En));
    // an empty key is E501 too (beside the field's own "does not fit")
    let c = Company::new("ringi-org-empty-key", "", &[("masters.ringi", masters), ("centres.csv", "centre,owner,valid_from,valid_to\n,Kato,,\n")]);
    let o = c.check("masters.ringi", Options::default());
    assert!(messages(&o).contains(&("E501", "The key `centre` of the row is empty".to_string())), "{}", common::shown(&o, Lang::En));
    // 1,000 characters pass
    let c = Company::new("ringi-org-value-at-most", "", &[("masters.ringi", masters), ("centres.csv", &format!("centre,owner,valid_from,valid_to\nC1,{},,\n", "x".repeat(1000)))]);
    let o = c.check("masters.ringi", Options::default());
    assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
}

/// The top object of `organization.json` reads its three keys alone (`teams`, `people`, `members`,
/// in this case): another key is E501 at the key's line, every one of them said, and the version
/// is not read.
#[test]
fn the_top_object_of_organization_json_reads_its_keys_alone() {
    let json = r#"{"teams": [{"id": "company", "name": "Example Co. (fictional)", "kind": "company", "parent": null, "head": "p01"}],
"Junk": [],
"people": [{"id": "p01", "name": "President (fictional)"}],
"Members": [{"person": "p01", "team": "company", "position": "president", "primary": true}]}
"#;
    let j = Company::new("ringi-org-json-keys", "", &[]);
    for f in ["teams.csv", "people.csv", "members.csv"] {
        std::fs::remove_file(j.path(&format!("org/2027-04-01/{f}"))).unwrap();
    }
    j.write("org/2027-04-01/organization.json", json);
    let o = j.check("company.ringi", Options::default());
    let got: Vec<(&str, &str, Option<usize>)> = o.diags.iter().map(|d| (d.code, d.message.en.as_str(), d.line)).collect();
    assert_eq!(got.len(), 3, "{}", common::shown(&o, Lang::En));
    // the missing array is said at the file's first line, the keys it does not read at their lines
    assert!(got[0].1.ends_with("has no array `members`") && got[0].2 == Some(1), "{}", common::shown(&o, Lang::En));
    assert_eq!(
        got[1..],
        [
            ("E501", "`Junk` is not a key of this file (the keys read are `teams`, `people`, `members`)", Some(2)),
            ("E501", "`Members` is not a key of this file (the keys read are `teams`, `people`, `members`)", Some(4)),
        ],
        "{}",
        common::shown(&o, Lang::En)
    );
    assert_eq!(o.diags[1].message.ja, "`Junk` は、このファイルのキーではありません（読むキーは `teams`, `people`, `members`）");
}

/// An object of an array of `organization.json` reads its keys alone, as the file's top object
/// does: another key — `Name`, written otherwise in only case, counts — is E501 at the object's
/// line; and a key an object, or the top object, writes twice the file does not read.
#[test]
fn an_object_of_organization_json_reads_its_keys_alone() {
    let json = r#"{"teams": [{"id": "company", "name": "Example Co. (fictional)", "kind": "company", "parent": null, "head": "p01"}],
"people": [{"id": "p01", "name": "President (fictional)", "Name": "x"}],
"members": [{"person": "p01", "team": "company", "position": "president", "primary": true}]}
"#;
    let j = Company::new("ringi-org-json-el-keys", "", &[]);
    for f in ["teams.csv", "people.csv", "members.csv"] {
        std::fs::remove_file(j.path(&format!("org/2027-04-01/{f}"))).unwrap();
    }
    j.write("org/2027-04-01/organization.json", json);
    let o = j.check("company.ringi", Options::default());
    let got: Vec<(&str, &str, Option<usize>)> = o.diags.iter().map(|d| (d.code, d.message.en.as_str(), d.line)).collect();
    assert_eq!(
        got,
        [("E501", "`Name` is not a key of `people` (the keys read are `id`, `name`, `name_ja`, `email`, `language`)", Some(2))],
        "{}",
        common::shown(&o, Lang::En)
    );
    assert_eq!(o.diags[0].message.ja, "`Name` は `people` のキーではありません（読むキーは `id`, `name`, `name_ja`, `email`, `language`）");
    // a key written twice — in an object, or at the top — the file does not read, said at its line
    for (json, line) in [
        (json.replace(r#""name": "President (fictional)", "Name": "x""#, r#""name": "President (fictional)", "name": "x""#), 2),
        (json.replace(r#""members": [{"person": "p01", "team": "company", "position": "president", "primary": true}]}"#, "\"members\": [],\n\"members\": []"), 4),
    ] {
        j.write("org/2027-04-01/organization.json", &json);
        let o = j.check("company.ringi", Options::default());
        let [("E501", ref m)] = messages(&o)[..] else { panic!("{}", common::shown(&o, Lang::En)) };
        assert!(m.contains("does not read as JSON (the key `name` appears twice)") || m.contains("does not read as JSON (the key `members` appears twice)"), "{m}");
        assert_eq!(o.diags[0].line, Some(line));
    }
}

/// An ID of a person or a team is 255 bytes at the most, as the server's `longestID` holds it: a
/// longer one is E501 at the row, the ID said cut to forty characters.
#[test]
fn an_id_is_255_bytes_at_the_most() {
    let long = "x".repeat(256);
    let shown = format!("{}…", "x".repeat(40));
    let people = PEOPLE_2026.replace("p02,Head of Sales", &format!("{long},Head of Sales"));
    let teams = TEAMS_2026.replace("finance,Finance", &format!("{long},Finance"));
    let c = Company::new("ringi-org-long-ids", "", &[("org/2026-04-01/people.csv", &people), ("org/2026-04-01/teams.csv", &teams)]);
    let o = c.check("company.ringi", Options::default());
    let got: Vec<&str> = o.diags.iter().map(|d| d.message.en.as_str()).collect();
    for want in [
        format!("The `id` of the person `{shown}` is 256 bytes long; an ID is 255 bytes at the most"),
        format!("The `id` of the team `{shown}` is 256 bytes long; an ID is 255 bytes at the most"),
    ] {
        assert!(got.contains(&want.as_str()), "not found: {want}\n{}", got.join("\n"));
    }
    assert!(o.diags.iter().any(|d| d.message.ja.contains("ID は255バイトまでです")), "{}", common::shown(&o, Lang::En));
    // 255 bytes pass the check (a person of only `x`s still says its other problems, not this one)
    let people = PEOPLE_2026.replace("p02,Head of Sales", &format!("{},Head of Sales", "x".repeat(255)));
    let c = Company::new("ringi-org-id-at-most", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert!(!o.diags.iter().any(|d| d.message.en.contains("bytes long")), "{}", common::shown(&o, Lang::En));
}

/// A row of a CSV file is at most 1 MiB and of at most 1,000 values, a row of another number of
/// values than there are columns does not read either way, and a quote inside a field not written
/// in quotes — or a character after a field's closing quote — the file does not read (as the
/// server's CSV reader refuses them).
#[test]
fn a_row_of_a_csv_reads_within_the_servers_limits() {
    // a row longer than 1 MiB does not read, said at the line it starts on
    let people = format!("id,name,name_ja\np01,{},x\n", "x".repeat(1 << 20));
    let c = Company::new("ringi-org-row-bytes", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The row is longer than 1048576 bytes, the most a row may be".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[0].line, Some(2));
    assert_eq!(o.diags[0].message.ja, "この行は1048576バイトを超えます。一つの行は1048576バイトまでです");
    // a row of more than 1,000 values does not read
    let people = format!("id,name,name_ja\n{}\n", "a,".repeat(1001));
    let c = Company::new("ringi-org-row-values", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "The row has more than 1000 values, the most a row may have".to_string())], "{}", common::shown(&o, Lang::En));
    // a row of fewer values than there are columns is E501 too
    let people = format!("{PEOPLE_2026}p07,Contractor\n");
    let c = Company::new("ringi-org-row-short", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert!(messages(&o).contains(&("E501", "The row has 2 values, and there are 3 columns".to_string())), "{}", common::shown(&o, Lang::En));
    // a quote inside a field not written in quotes, and a character after a closing quote
    let people = format!("{PEOPLE_2026}p07,Contractor\"x\",z\n");
    let c = Company::new("ringi-org-bare-quote", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "a quote `\"` is inside a cell not written in quotes".to_string())], "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[0].message.ja, "引用符で囲んでいない値の中に `\"` があります");
    let people = format!("{PEOPLE_2026}p07,\"Contractor\"x,z\n");
    let c = Company::new("ringi-org-after-quote", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert_eq!(messages(&o), [("E501", "a quote `\"` is not closed, or more follows it in its cell".to_string())], "{}", common::shown(&o, Lang::En));
}

/// Every problem of the organization's data is said: the check does not stop at 1,000 (the
/// server's `MaxOrgProblems`), and says the 1,001st at its row too.
#[test]
fn every_problem_of_the_data_is_said() {
    let mut people = PEOPLE_2026.to_string();
    for i in 1..=1001 {
        people.push_str(&format!(",No ID {i},\n"));
    }
    let c = Company::new("ringi-org-all-problems", "", &[("org/2026-04-01/people.csv", &people)]);
    let o = c.check("company.ringi", Options::default());
    assert_eq!(o.diags.len(), 1001, "{}", common::shown(&o, Lang::En));
    assert!(o.diags.iter().all(|d| d.code == "E501" && d.message.en == "A person's `id` is empty"), "{}", common::shown(&o, Lang::En));
    assert_eq!(o.diags[1000].line, Some(1008), "{:?}", o.diags[1000]);
}
