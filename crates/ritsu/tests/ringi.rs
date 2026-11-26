//! ringi in `ritsu` (ringi's PLAN B.1, B.6): `ritsu ringi` is ringi's own command with the six
//! languages a `.ringi` reads and is lowered to joined, so a request the binary of ringi's crate
//! cannot check (E008) is checked; `ritsu check` checks the `.ringi` files of a project after
//! sekisho and before yuen and sakai, says of each what `ritsu ringi check` says, and counts them as
//! `ringi N`. `ritsu --help` lists ringi, the tenth language, after kaname (ringi's PLAN F.3), and
//! `ritsu ringi`, `ritsu help ringi` and a link named `ringi` run it. A
//! requirement names what a request holds (ringi's DESIGN 9.7), and `ritsu ringi api` writes what a
//! request declares (16.1). What `ritsu check` says across a request's borders is in `tests/cross.rs`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

/// ringi's crate, whose `examples/` holds ten requests and the files they read.
fn ringi_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ringi")
}

fn run(program: &Path, dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(program);
    c.current_dir(dir).args(args);
    for v in common::LANG_VARS {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn ritsu(args: &[&str]) -> (i32, String, String) {
    run(Path::new(env!("CARGO_BIN_EXE_ritsu")), &ringi_dir(), args)
}

#[test]
fn ritsu_ringi_checks_a_request_with_every_language_joined() {
    let (code, out, err) = ritsu(&["ringi", "check", "examples/expense.ringi", "examples/shared/company.ringi"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    // the organization's data gives warnings (an empty seat in the version laid ahead, one head at
    // two stages: ringi's DESIGN 8.6) and no error; each file ends with its `ok` line, the data it
    // reads, the longest a ticket of a request takes, and what the shape of the organization does to
    // its stages
    assert!(!out.contains("error["), "{out}");
    assert!(
        out.ends_with(concat!(
            "ok examples/expense.ringi: a request, 8 fields, 4 stages\n",
            "  used from 2026-04-01: the first version of the organization `shared/company.ringi` (data `../org`)\n",
            "  used with no end: no data it reads ends\n",
            "  ends within 93 business days at the most (4 × 21 at its stages, 3 × 3 with the requester)\n",
            "  requesters whose stages (`precheck`, `first`, `second`, `decide`) go elsewhere or are skipped by the shape of the organization: 14 of 24 in the organization from 2026-04-01, and 13 of 23 from 2027-04-01\n",
            "  requesters whose late stages (`second`) have nowhere to go up to by the shape of the organization: 1 of 24 in the organization from 2026-04-01, and 1 of 23 from 2027-04-01\n",
            "  who they are, and where their stages go: `ritsu ringi simulate examples/expense.ringi --every-requester`\n",
            "ok examples/shared/company.ringi: an organization, 5 positions, 5 kinds of team\n",
            "  data `../org`: 2 versions (2026-04-01, 2027-04-01), 13 teams and 23 people in the last, 2 proxies\n",
        )),
        "{out}"
    );
    let (code, out, _) = ritsu(&["ringi", "check", "examples/purchase.ja.ringi", "--lang", "ja"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.ends_with(concat!(
            "ok examples/purchase.ja.ringi: 申請の種類、フィールド 9 個、ステップ 6 個\n",
            "  2026-04-01 から使えます: 組織 `shared/company.ringi` の最初の版（データ `../org`）\n",
            "  終わり無く使えます: 読むデータに終わりがありません\n",
            "  長くても 246 営業日で終わります（ステップに 4 × 54、申請者のもとで 3 × 10）\n",
            "  組織の形で、ステップ（`一次`、`財務`、`決裁`）がほかへ回るか飛ばされる申請者: 2026-04-01 からの組織の 24 人のうち 14 人、2027-04-01 からの 23 人のうち 13 人\n",
            "  組織の形で、期限を過ぎたステップ（`一次`、`決裁`）を上へ回せない申請者: 2026-04-01 からの組織の 24 人のうち 24 人、2027-04-01 からの 23 人のうち 23 人\n",
            "  だれの申請の、どのステップがどこへ回るか: `ritsu ringi simulate examples/purchase.ja.ringi --every-requester`\n",
        )),
        "{out}"
    );
    let (code, out, _) = ritsu(&["ringi", "--version"]);
    assert_eq!((code, out), (0, format!("ringi {}\n", env!("CARGO_PKG_VERSION"))));
    let (code, out, _) = ritsu(&["ringi", "explain", "E008"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("E008 (error) — "), "{out}");
    // the words after `ritsu ringi` are ringi's own command line; the help it points to is named the
    // way the command was run (DESIGN 6.2, item 10)
    let (code, _, err) = ritsu(&["ringi", "check", "examples/expense.ringi", "--fast"]);
    assert_eq!((code, err.as_str()), (2, "error: unknown flag `--fast` (did you mean `--at`?); run `ritsu ringi check --help`\n"));
}

/// `ritsu ringi run` is ringi's `run`: the scenarios of a request, and exit 2 for a request that
/// has none, as `ritsu ringi vectors` ends for it.
#[test]
fn ritsu_ringi_run_runs_the_scenarios_of_a_request() {
    let (code, out, err) = ritsu(&["ringi", "run", "examples/account.ringi"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(out.contains("scenario `administrator_access`: as written (5 rows)\n") && out.ends_with("scenario `the_manager_answers_once_at_security`: as written (3 rows)\n"), "{out}");
    let none = "error: `tests/machine/bare.ringi` has no scenario";
    assert_eq!(ritsu(&["ringi", "run", "tests/machine/bare.ringi"]), (2, String::new(), format!("{none}\n")));
    assert_eq!(ritsu(&["ringi", "vectors", "tests/machine/bare.ringi"]), (2, String::new(), format!("{none}: the vectors are made from the requester and the form of a scenario\n")));
}

#[test]
fn ritsu_help_lists_ringi_after_kaname() {
    let (code, out, _) = ritsu(&["--help"]);
    assert_eq!(code, 0);
    let (kaname, ringi) = (out.find("\n  ritsu kaname "), out.find("\n  ritsu ringi "));
    assert!(kaname.is_some() && ringi > kaname && out.contains("ringi's commands (applications and approvals)"), "{out}");
    // asked by name, its help is ringi's own
    let (code, out, _) = ritsu(&["help", "ringi"]);
    assert_eq!(code, 0);
    assert!(out.starts_with(&format!("ringi {}\n\nA small language for a company's applications and approvals", env!("CARGO_PKG_VERSION"))), "{out}");
    let (code, out, _) = ritsu(&["help", "ringi", "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(out.contains("\n\n会社の申請と承認を書く小さな言語。"), "{out}");
}

/// Called by a link named `ringi`, ritsu is `ritsu ringi`, as it is each language's command by
/// that language's name.
#[cfg(unix)]
#[test]
fn a_link_named_ringi_is_ritsu_ringi() {
    let t = ritsu_testkit::TempDir::new("ritsu-ringi-link");
    let link = t.path().join("ringi");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_ritsu"), &link).unwrap();
    let (code, out, err) = run(&link, &ringi_dir(), &["check", "examples/leave.ringi"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(out.contains("\nok examples/leave.ringi: a request, 6 fields, 2 stages\n"), "{out}");
    // and says what `ritsu ringi check` says, the organization's warnings and all, but for the
    // command it names to run next, named the way each was run (ritsu's DESIGN 6.2, item 10)
    let line = |run: &str| format!("  who they are, and where their stages go: `{run} simulate examples/leave.ringi --every-requester`\n");
    assert!(out.contains(&line("ringi")), "{out}");
    let (c, said, e) = ritsu(&["ringi", "check", "examples/leave.ringi"]);
    assert!(said.contains(&line("ritsu ringi")), "{said}");
    assert_eq!((c, said.replace(&line("ritsu ringi"), &line("ringi")), e), (code, out, err));
}

#[test]
fn ritsu_check_says_what_ritsu_ringi_says() {
    let files = ["examples/expense.ringi", "examples/leave.ja.ringi", "examples/shared/manager_approval.ringi", "examples/shared/masters.ringi"];
    let mut args = vec!["check"];
    args.extend(files);
    let (code, out, err) = ritsu(&args);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let mut want = String::new();
    for f in files {
        let (_, said, _) = ritsu(&["ringi", "check", f]);
        // `ritsu check` puts the language's name before each code (`warning[ringi W502]`)
        for line in said.split_inclusive('\n') {
            match ["warning[", "error["].iter().find_map(|k| line.strip_prefix(k).map(|rest| (k, rest))) {
                Some((k, rest)) => want.push_str(&format!("{k}ringi {rest}")),
                None => want.push_str(line),
            }
        }
    }
    // the organization's data gives warnings and no error (ringi's DESIGN 8.6)
    let warnings = want.lines().filter(|l| l.starts_with("warning[")).count();
    assert!(warnings > 0 && !want.contains("error["), "{want}");
    // the borders are the two requests' tickets against the gate, the book and the flows they name
    // (ringi's DESIGN 9.3): 18 of the expense request, 4 of the leave request
    want.push_str(&format!("ritsu check: 4 files (ringi 4): all pass ({warnings} warnings); borders between the languages: 22 checked, 0 undecided\n"));
    assert_eq!(out, want);
    // the command a file's check names is named the way it is run, through `ritsu` (ritsu's DESIGN
    // 6.2, item 10)
    assert!(out.contains("  who they are, and where their stages go: `ritsu ringi simulate examples/expense.ringi --every-requester`\n"), "{out}");
    // in the JSON, each file's check is what `ritsu ringi check --format json` writes of it, without
    // the file and the diagnostics, which ritsu writes itself
    let (code, out, _) = ritsu(&["check", "examples/expense.ringi", "--format", "json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let f = &v["files"][0];
    assert_eq!((f["tool"].as_str(), f["file"].as_str()), (Some("ringi"), Some("crates/ringi/examples/expense.ringi")), "{f}");
    let (_, own, _) = ritsu(&["ringi", "check", "examples/expense.ringi", "--format", "json"]);
    let mut own: serde_json::Value = serde_json::from_str(&own).unwrap();
    for k in ["file", "diagnostics"] {
        own.as_object_mut().unwrap().remove(k);
    }
    assert_eq!(f["check"], own);
}

/// The `.ringi` files are checked after sekisho's and before yuen's, and counted as `ringi N` in
/// that place (ringi's DESIGN 16.3): `examples/` holds files of eight languages.
#[test]
fn ritsu_check_counts_ringi_after_sekisho_and_before_yuen() {
    let (code, out, err) = ritsu(&["check", "examples"]);
    assert!(code < 2, "{out}{err}");
    let summary = out.lines().last().unwrap_or_default();
    assert!(summary.contains(", sekisho 1, ringi 13, yuen 1)"), "{summary}");
    // what ringi finds in the examples is what the organization's data makes of them (ringi's DESIGN
    // 8.6): in the contract, in the version laid ahead, a person who has left and a quorum out of
    // reach; stages that go elsewhere for an empty seat there, and one person at two stages. What
    // the shape of the organization does is said under `ok`
    let found: Vec<&str> = out.lines().filter(|l| l.contains("[ringi ")).collect();
    assert!(found.iter().all(|l| ["[ringi E503]", "[ringi E504]", "[ringi W502]", "[ringi W506]"].iter().any(|c| l.contains(c))), "{out}");
    assert!(found.iter().any(|l| l.starts_with("error[ringi E503]: examples/contract.ringi:")), "{out}");
    assert!(!found.iter().any(|l| l.contains("[ringi E502]")), "{out}");
    for f in ["examples/expense.ringi", "examples/shared/company.ringi", "examples/shared/masters.ringi"] {
        assert!(out.contains(&format!("ok {f}: ")), "{f}: {out}");
    }
}

/// yuen links a requirement to what a request holds (ringi's DESIGN 9.7): a stage, a way to choose
/// an approver, a rule of the ticket, and the request as what verifies it. A thing that is not
/// there is said with the ones of its kind, and a thing that checks nothing cannot verify. The
/// example's own requirements say nothing but that no one has looked at their links yet (E301).
#[test]
fn yuen_links_a_requirement_to_what_a_request_holds() {
    let t = ritsu_testkit::TempDir::new("ritsu-ringi-req");
    let ledger = ritsu_cross::codes::ledger();
    let ritsu_base::ledger::Repro::Dir { files, .. } = &ledger.find("E911").expect("E911 is in the ledger").repro else { panic!("E911 has no files") };
    for (name, body) in files {
        t.write(name, body);
    }
    let req = "requirements purchasing v1\ndescription \"Where the purchase request's route comes from (fictional)\"\nrole finance\n\nrequirement finance_checks\n  text \"Finance checks every purchase request, and no one checks their own\"\n  owner finance\n  decided 2026-11-15 by finance \"As the purchasing rules (fictional) say\"\n  satisfied by ringi \"purchase.ringi\" stage check\n  satisfied by ringi \"purchase.ringi\" who finance\n  satisfied by ringi \"purchase.ringi\" ticket self_approval\n  verified by ringi \"purchase.ringi\"\n";
    let ritsu_in = |args: &[&str]| run(Path::new(env!("CARGO_BIN_EXE_ritsu")), t.path(), args);
    t.write("purchasing.req", req);
    let (code, out, _) = ritsu_in(&["yuen", "check", "purchasing.req", "--root", "."]);
    assert_eq!(code, 1, "{out}");
    assert_eq!((out.matches("error[E301]").count(), out.matches("error[").count()), (4, 4), "the four links wait for a person to look at them: {out}");
    // a stage that is not there, with the one that is
    t.write("purchasing.req", req.replace("stage check", "stage checks"));
    let (_, out, _) = ritsu_in(&["yuen", "check", "purchasing.req", "--root", "."]);
    assert!(out.contains("error[E202]: purchasing.req:9:16: purchase.ringi has no stage checks\n"), "{out}");
    assert!(out.contains("  = Did you mean: ringi \"purchase.ringi\" stage check\n"), "{out}");
    // a way to choose an approver checks nothing
    t.write("purchasing.req", req.replace("verified by ringi \"purchase.ringi\"\n", "verified by ringi \"purchase.ringi\" who finance\n"));
    let (_, out, _) = ritsu_in(&["yuen", "check", "purchasing.req", "--root", "."]);
    assert!(out.contains("error[E403]: purchasing.req:12:15: ringi \"purchase.ringi\" who finance checks nothing, so it cannot be written after `verified by`\n"), "{out}");
    assert!(out.contains("a ringi claim, example or scenario (`claim`, `example`, `scenario`)"), "{out}");
    // a kind with no name is refused where it is written
    t.write("purchasing.req", req.replace("who finance", "who"));
    let (_, out, _) = ritsu_in(&["yuen", "check", "purchasing.req", "--root", "."]);
    assert!(out.contains("error[E012]: purchasing.req:10:42: The kind `who` has no name after it\n"), "{out}");
    // the requirements of the example
    let (code, out, _) = ritsu(&["check", "examples/shared/expense.req"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!((out.matches("error[yuen E301]").count(), out.matches("error[").count()), (21, 21), "{out}");
    for thing in ["table route", "who manager", "ticket self_approval", "deadline every_stage", "table next_up", "claim reminder_before_due", "example route"] {
        assert!(out.contains(&format!("ringi \"crates/ringi/examples/expense.ringi\" {thing} yet")), "{thing}: {out}");
    }
}

/// `ritsu ringi api` writes what a request declares, with every language joined (ringi's DESIGN
/// 16.1): what ringi's own `tests/items.rs` holds to the ports.
#[test]
fn ritsu_ringi_api_writes_what_a_request_declares() {
    let (code, out, err) = ritsu(&["ringi", "api", "examples/leave.ringi", "--root", "."]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, std::fs::read_to_string(ringi_dir().join("tests/golden/api/leave.json")).unwrap());
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!((v["kind"].as_str(), v["name"].as_str(), v["ticket"]["time"]["round"].as_u64()), (Some("request"), Some("leave"), Some(10)));
}

/// The commands ringi's pages show with what they print (its READMEs, `docs/reference.md` and the
/// skill) print that, run as `ritsu`: what ringi's own `tests/docs.rs` cannot run, since a request
/// is lowered to the languages the binary of ringi's crate does not hold (E008). Each `$ ritsu …`
/// and `$ ringi …` of a `console` block is run by a shell in ringi's crate, or in the directory of a
/// `$ cd` above it in the block (from the root of the repository), with standard error where
/// standard output is and no language asked of the environment; a line `…` of the page stands for
/// any number of lines. A command that writes (`--out <dir>`) writes into a scratch directory, and
/// what it prints is read with the page's `<dir>`, and a later command of the page reads a path
/// under that `<dir>` where it was written; `init` is run in a scratch directory. A command
/// shown with no output under it is listed only (ringi's `tests/docs.rs` holds it to the table).
#[cfg(unix)]
#[test]
fn the_commands_on_ringis_pages_print_what_they_show() {
    let pages = ["README.md", "README.ja.md", "docs/reference.md", "skills/README.md", "../../skills/ringi/SKILL.md", "../../skills/ringi/reference.md"];
    let root = ringi_dir().join("../..").canonicalize().unwrap();
    // `ritsu`, and a link named `ringi` to it, on the PATH of the shell
    let links = ritsu_testkit::TempDir::new("ritsu-ringi-pages");
    for name in ["ritsu", "ringi"] {
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_ritsu"), links.path().join(name)).unwrap();
    }
    let scratch = ritsu_testkit::TempDir::new("ritsu-ringi-pages-out");
    let (mut ran, mut wrong) = (0, Vec::new());
    for page in pages {
        let text = std::fs::read_to_string(ringi_dir().join(page)).unwrap_or_else(|e| panic!("{page}: {e}"));
        let mut block: Option<Vec<String>> = None;
        let mut blocks = Vec::new();
        for line in text.lines() {
            match (line.trim_start().strip_prefix("```"), block.take()) {
                (Some(_), Some(b)) => blocks.push(b),
                (Some(info), None) => block = Some(if info.trim() == "console" { Vec::new() } else { vec!["#not console".to_string()] }),
                (None, Some(mut b)) => {
                    b.push(line.to_string());
                    block = Some(b);
                }
                (None, None) => {}
            }
        }
        // the directories the commands above on the page wrote, as it shows them and where they are
        let mut written: Vec<(String, String)> = Vec::new();
        for b in blocks.into_iter().filter(|b| b.first().is_none_or(|l| l != "#not console")) {
            let mut runs: Vec<(String, Vec<String>)> = Vec::new();
            for l in &b {
                match l.strip_prefix("$ ") {
                    Some(cmd) => runs.push((cmd.to_string(), Vec::new())),
                    None => {
                        if let Some((_, out)) = runs.last_mut() {
                            out.push(l.clone());
                        }
                    }
                }
            }
            let mut cwd = ringi_dir();
            for (cmd, out) in runs {
                let first = cmd.split_whitespace().next().unwrap_or("");
                if first == "cd" {
                    cwd = root.join(cmd.trim_start_matches("cd").trim());
                    assert!(cwd.is_dir(), "{page}: `$ {cmd}` is no directory of the repository");
                    continue;
                }
                if !["ritsu", "ringi"].contains(&first) || out.is_empty() {
                    continue;
                }
                assert!(!cmd.contains(['|', ';', '>', '<', '`', '$']), "{page}: `$ {cmd}` is more than a command to run");
                if cmd.split(' ').any(|w| w == "wasm") && !common::wasm_is_built() {
                    continue;
                }
                // a path into what a command above wrote (`--diff-from generated/requests.zip`) is
                // read where it wrote it
                let at_out = cmd.split(' ').position(|w| w == "--out").map(|i| i + 1);
                let mut words: Vec<String> = cmd.split(' ').enumerate().map(|(i, w)| if Some(i) == at_out { w.to_string() } else { common::into_written(w, &written) }).collect();
                let mut shown_out = None;
                if let Some(i) = words.iter().position(|w| w == "--out")
                    && let Some(dir) = words.get_mut(i + 1)
                {
                    let scratch_out = scratch.path().join(format!("out{ran}")).display().to_string();
                    let shown = std::mem::replace(dir, scratch_out.clone());
                    written.retain(|(s, _)| *s != shown);
                    written.push((shown.clone(), scratch_out.clone()));
                    shown_out = Some((scratch_out, shown));
                }
                let here = if cmd.split_whitespace().take(3).any(|w| w == "init") { scratch.path().join(format!("init{ran}")) } else { cwd.clone() };
                std::fs::create_dir_all(&here).unwrap();
                let mut c = Command::new("sh");
                c.arg("-c").arg(format!("{} 2>&1", words.join(" "))).current_dir(&here).env("PATH", format!("{}:{}", links.path().display(), std::env::var("PATH").unwrap_or_default()));
                for v in common::LANG_VARS {
                    c.env_remove(v);
                }
                let mut got = String::from_utf8_lossy(&c.output().expect("could not run sh").stdout).into_owned();
                if let Some((scratch_out, shown)) = &shown_out {
                    got = got.replace(scratch_out.as_str(), shown);
                }
                let trimmed = |s: &str| {
                    let mut v: Vec<String> = s.lines().map(|l| l.trim_end().to_string()).collect();
                    while v.last().is_some_and(|l| l.is_empty()) {
                        v.pop();
                    }
                    v
                };
                if !same(&trimmed(&out.join("\n")), &trimmed(&got)) {
                    let shown: String = got.lines().take(40).map(|l| format!("{l}\n")).collect();
                    wrong.push(format!("{page}: $ {cmd}\n--- the page shows\n{}\n--- it prints\n{shown}", out.join("\n")));
                }
                ran += 1;
            }
        }
    }
    assert!(ran >= 12, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Whether `want` (a page's lines, a line `…` standing for any number of lines) is `got`.
#[cfg(unix)]
fn same(want: &[String], got: &[String]) -> bool {
    match want.split_first() {
        None => got.is_empty(),
        Some((w, rest)) if w == "…" => (0..=got.len()).any(|i| same(rest, &got[i..])),
        Some((w, rest)) => got.first() == Some(w) && same(rest, &got[1..]),
    }
}
