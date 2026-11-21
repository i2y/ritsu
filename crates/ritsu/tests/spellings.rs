//! What a language's `check` says of a file does not turn on how its path was given (DESIGN 4.15,
//! 6.2 item 8): by its name from its directory, absolute, from the root of the tree, and from a
//! directory beside it (`ci`) climbing with `..`. Each file of the tree that says something of other
//! files is asked the four ways (a few for each language, chosen for the sentences they print); the
//! label of the file itself put back, every word that still differs is a path to one file written
//! from where the run is, the shortest way, absolute only when the file was given absolute. A path
//! a file wrote is said as it was written, and a file a reference names from the root, so neither
//! differs: each file comes with the sentences of its output that name one, which all four say word
//! for word. Asked as `./<name>` from its directory, and as `ci/../<path>` from the root, a file is
//! its name and its path from the root, folded: what it says is theirs byte for byte. The sweep of
//! every file of the tree is a script beside the work, not a test: it runs each language's check six
//! times over some thousands of files.
//!
//! A file a project reads from outside its root is named the same way, in the text and in the JSON
//! of `ritsu check`: from where the run is, never by a path of this machine the run was not given.
//!
//! A path whose `..` climbs out of a symbolic link to a directory names the file the system opens
//! there: each language reads that file, says a path that opens it from where the run is, and tells
//! it from a file the letters of the same path name. Given to a command, such a path is that file
//! as the path to it without the link is: the same root, the same name from it, the same run.
//!
//! A path a flag takes (`--out`, `--input`, `--scenario`) is one path however it is written, and is
//! said absolute only when it was given so. What `ritsu run` says of the answers a scenario leaves,
//! and `ritsu dandori scenarios` of an arm no scenario takes, is the same however the flow is given.
//! In a module, a file whose name starts with `@` is said `./@…`, the path that reads it, never the
//! dependency `@…` names.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

fn tree() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// The files, from the root of the tree, with the language that checks each, and what each of the
/// four outputs says word for word: the sentences that name a path as the file wrote it, or as a
/// reference writes it.
const FILES: &[(&str, &str, &[&str])] = &[
    // a path followed from the rule's directory, through `..`; a contribution; a copy's diff; what
    // koyomi says of a dates file it cannot read
    ("rulec", "crates/rulec/tests/mutants/m_e013enum.rule", &["Cannot read `../corpus/shared/catalogue.rule`"]),
    ("rulec", "crates/ritsu/tests/projects/product_line/rules/contrib/garden_kind.rule", &[]),
    ("rulec", "crates/rulec/tests/mutants/m_e038.rule", &[]),
    (
        "rulec",
        "crates/sakai/tests/mutants/E202_a_rule_reads_dates_inside_another_context/ordering/ship_window.rule",
        &["the days of koyomi \"../delivery/ship_date.cal\" date ship", "^ ../delivery/ship_date.cal cannot be read, or does not pass koyomi's check"],
    ),
    // a calendar a dates file uses, that cannot be read
    ("koyomi", "crates/koyomi/tests/modules/en/loose/net10.cal", &["The calendar @lib/calendars/office.cal cannot be read"]),
    ("chobo", "crates/chobo/tests/fixtures/型.book", &[]),
    // a gate read that does not pass, and the gate it reads; a rule that does not pass; a gate that
    // is not there; an enum a rule does not have
    (
        "sekisho",
        "crates/sekisho/tests/mutants/E201_a_gate_read_that_does_not_pass.gate",
        &["`read/staff_through.gate` does not pass sekisho's check", "The namespace of `staff_elsewhere.gate`, `Staff`, is not this file's"],
    ),
    ("sekisho", "crates/sekisho/tests/mutants/E201_a_rule_that_does_not_pass.gate", &["`read/broken.rule` does not pass rulec's check"]),
    ("sekisho", "crates/sekisho/tests/mutants/E201_a_gate_that_is_not_there.gate", &["Cannot read `people.gate`"]),
    ("sekisho", "crates/sekisho/tests/mutants/E101_import_enum_the_file_does_not_have.gate", &["`read/order_states.rule` has no enum `state`"]),
    // a rule that is not there, through `..`
    ("kaname", "crates/kaname/tests/modules/lift.kn", &["There is no file `../rulec/tests/corpus/member_tier_shipping.rule`"]),
    // a delta spec, a source a dates file does not declare, a message a `.proto` does not have: each
    // named by a reference, which says its file from the root
    (
        "yuen",
        "crates/yuen/tests/mutants/E104_delta_spec_named/greeter.req",
        &["The spec crates/yuen/tests/mutants/E104_delta_spec_named/openspec/changes/trim-names/specs/greeting/spec.md of the source greeting cannot be read"],
    ),
    (
        "yuen",
        "crates/yuen/tests/mutants/E106_borrows_an_undeclared_source/example.req",
        &[": crates/yuen/tests/mutants/E106_borrows_an_undeclared_source/a.cal declares no source cfr", "= crates/yuen/tests/mutants/E106_borrows_an_undeclared_source/a.cal declares no source at all."],
    ),
    ("yuen", "crates/yuen/tests/mutants/E202_no_such_message/example.req", &[": crates/yuen/tests/mutants/E202_no_such_message/a.proto has no message Orders"]),
    // paths that point at nothing, as written and where they point
    ("sakai", "crates/sakai/tests/modules/en/loose/shop.ctx", &["The path \"@lib/contexts/pricing.ctx\" is not there"]),
    // a child flow that is not there; a dates file of another context that is not there
    ("dandori", "crates/dandori/tests/fixtures/children.flow", &["could not read `children/無い.flow`", "`children/自分.flow` does not pass check"]),
    (
        "dandori",
        "crates/sakai/tests/mutants/E202_a_workflow_reads_dates_inside_another_context/ordering/ship_notice.flow",
        &["could not read the dates file `../delivery/ship_date.cal`"],
    ),
    // a request that passes, whose facts name the files of its organization and masters; a path to
    // a file that is not there, and a rule read that does not pass, as the request wrote each; a file
    // of an organization's data, which the error is at
    ("ringi", "crates/ringi/examples/expense.ringi", &[]),
    ("ringi", "crates/ringi/tests/mutants/E005_no_such_file.ringi", &["There is no file `shared/nowhere.ringi`"]),
    ("ringi", "crates/ringi/tests/mutants/E202_a_broken_rule.ringi", &["`shared/broken_authority.rule` does not pass rulec's check"]),
    ("ringi", "crates/ringi/tests/mutants/E501_a_kind_of_team_not_declared_in_the_data.ringi", &[]),
    // in each language, a mistake other lines of the file hang on: said once, where it is, with the
    // lines that write the same name too (DESIGN 8.3)
    ("rulec", "crates/rulec/tests/chains/chain_types.rule", &["`item_kind` is written on line 35 too."]),
    ("koyomi", "crates/koyomi/tests/chains/chain_names.cal", &["`received` is written on lines 12 and 20 too."]),
    ("chobo", "crates/chobo/tests/chains/chain_declarations.book", &["`USD` is written on lines 11, 12 and 15 too"]),
    ("sekisho", "crates/sekisho/tests/chains/chain_names.gate", &["`clerk` is written on line 31 too."]),
    ("kaname", "crates/kaname/tests/chains/chain_names.kn", &["`qty` is written on lines 36 and 37 too."]),
    ("geas", "crates/geas/tests/chains/chain_names.geas", &["`api` is written on lines 15, 17 and 22 too"]),
    ("yuen", "crates/yuen/tests/chains/chain_names.req", &["`accounting` is written on lines 11, 16 and 17 too."]),
    ("sakai", "crates/sakai/tests/chains/names/shop.ctx", &["`Inventory` is written on line 21 too."]),
    ("dandori", "crates/dandori/tests/fixtures/chain_names.flow", &["`notfy` is written on lines 23 and 24 too"]),
    ("ringi", "crates/ringi/tests/chains/chain_names.ringi", &["`reviewer` is written on lines 30 and 43 too."]),
];

/// The four ways, each the directory the check runs in and the path it is given.
fn ways(root: &Path, rel: &str) -> [(PathBuf, String); 4] {
    let file = root.join(rel);
    let dir = file.parent().unwrap().to_path_buf();
    let name = file.file_name().unwrap().to_string_lossy().to_string();
    [(dir.clone(), name), (dir, file.to_string_lossy().to_string()), (root.to_path_buf(), rel.to_string()), (root.join("ci"), format!("../{rel}"))]
}

const SPELLED: [&str; 4] = ["name", "absolute", "from the root", "from beside the root"];

/// `.` and `..` folded by their letters.
fn fold(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// A word of a line: what lies between spaces, quotes, brackets and the marks that end a path.
fn words(line: &str) -> Vec<&str> {
    line.split(|c: char| c.is_whitespace() || "`'\"()[],;:".contains(c)).filter(|w| !w.is_empty()).collect()
}

/// A word as a path: without the full stop that ends a sentence.
fn as_path(w: &str) -> &str {
    match w.strip_suffix('.') {
        Some(p) if !p.is_empty() && !p.chars().all(|c| c == '.') => p,
        _ => w,
    }
}

/// The one a run writes `target` (absolute, folded) from `cwd`: the shortest relative path, or the
/// path itself for the run given an absolute path.
fn run_way(k: usize, cwd: &Path, target: &Path) -> String {
    if k == 1 { target.to_string_lossy().to_string() } else { ritsu_base::paths::between(cwd, target) }
}

/// A line with a group `(…)` at its end taken off, when it has one: what koyomi's E015 adds where
/// the calendar it looked for is not the path the file wrote, which the name of the file in its own
/// directory never is.
fn without_last_group(line: &str) -> Option<(&str, &str)> {
    let inner = line.strip_suffix(')')?;
    let open = inner.rfind(" (")?;
    Some((&line[..open], &inner[open + 2..]))
}

#[test]
fn a_check_says_the_same_however_the_file_is_given() {
    let root = tree();
    let bin = env!("CARGO_BIN_EXE_ritsu");
    let mut wrong = Vec::new();
    for (tool, rel, written) in FILES {
        let ways = ways(&root, rel);
        let said = |cwd: &Path, given: &str| -> String {
            let o = Command::new(bin).args([tool, "check", given, "--lang", "en"]).current_dir(cwd).env("NO_COLOR", "1").output().unwrap();
            format!("{}{}exit {}\n", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr), o.status.code().unwrap_or(-1))
        };
        let texts: Vec<String> = ways.iter().map(|(cwd, given)| said(cwd, given)).collect();
        // `./<name>` from the file's directory, and `ci/../<path>` from the root (`ci` is a directory,
        // no link): the name and the path from the root, folded, which the run holds as those, so
        // each says what they say byte for byte, with nothing put back
        for (k, (cwd, given)) in [(0, (&ways[0].0, format!("./{}", ways[0].1))), (2, (&ways[2].0, format!("ci/../{rel}")))] {
            let text = said(cwd, &given);
            if text != texts[k] {
                wrong.push(format!("{rel} (`{given}`): not what {} says:\n{text}\n--- {}:\n{}", SPELLED[k], SPELLED[k], texts[k]));
            }
        }
        let outs: Vec<Vec<String>> = ways
            .iter()
            .zip(&texts)
            .map(|((_, given), text)| {
                // the file itself, as it was given, put back to its name
                let name = &ways[0].1;
                text.replace(given.as_str(), name).lines().map(str::to_string).collect()
            })
            .collect();
        for (k, out) in outs.iter().enumerate() {
            if k != 1 && out.iter().any(|l| l.contains(root.to_string_lossy().as_ref())) {
                wrong.push(format!("{rel} ({}): a path of this machine where none was given:\n{}", SPELLED[k], out.join("\n")));
            }
            for w in *written {
                if !out.iter().any(|l| l.contains(w)) {
                    wrong.push(format!("{rel} ({}): it does not say `{w}`:\n{}", SPELLED[k], out.join("\n")));
                }
            }
        }
        if outs.iter().any(|o| o.len() != outs[0].len()) {
            wrong.push(format!("{rel}: the outputs have {:?} lines", outs.iter().map(Vec::len).collect::<Vec<_>>()));
            continue;
        }
        for i in 0..outs[0].len() {
            let mut lines: Vec<String> = outs.iter().map(|o| o[i].clone()).collect();
            if lines.iter().all(|l| *l == lines[0]) {
                continue;
            }
            // koyomi's E015: the place looked at, after the path as written, where the two differ
            let counts: Vec<usize> = lines.iter().map(|l| words(l).len()).collect();
            if counts.iter().any(|c| *c != counts[0]) {
                let fewest = counts.iter().copied().min().unwrap_or(0);
                let mut taken = Vec::new();
                for (k, l) in lines.iter_mut().enumerate() {
                    if counts[k] == fewest {
                        continue;
                    }
                    if let Some((head, group)) = without_last_group(l).map(|(h, g)| (h.to_string(), g.to_string())) {
                        taken.push((k, group));
                        *l = head;
                    }
                }
                for (k, group) in &taken {
                    let (cwd, _) = &ways[*k];
                    let target = fold(&cwd.join(group));
                    if *group != run_way(*k, cwd, &target) {
                        wrong.push(format!("{rel} ({}): the place looked at, `{group}`, is not written from where the run is", SPELLED[*k]));
                    }
                }
            }
            let split: Vec<Vec<&str>> = lines.iter().map(|l| words(l)).collect();
            if split.iter().any(|w| w.len() != split[0].len()) {
                wrong.push(format!("{rel}: line {} differs in its words:\n{}", i + 1, lines.join("\n")));
                continue;
            }
            for j in 0..split[0].len() {
                let w: Vec<&str> = split.iter().map(|s| as_path(s[j])).collect();
                if w.iter().all(|x| *x == w[0]) {
                    continue;
                }
                let targets: Vec<PathBuf> = ways.iter().zip(&w).map(|((cwd, _), x)| fold(&cwd.join(x))).collect();
                let one = targets.iter().all(|t| *t == targets[0]);
                let runs = (0..4).all(|k| w[k] == run_way(k, &ways[k].0, &targets[k]));
                if !(one && runs) {
                    wrong.push(format!("{rel}: line {}, `{}` is not one file written from where each run is:\n{}", i + 1, w.join("` `"), lines.join("\n")));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// The project of X4's reproduction (ritsu E203), its rule moved beside the project's root: the
/// note of E203 names the rule from where `ritsu check` runs, `.` and `..` folded, in both languages
/// and in the JSON, and absolute only where the project was given absolute.
#[test]
fn a_file_outside_the_root_is_named_from_where_the_run_is() {
    let ledger = ritsu_cross::codes::ledger();
    let ritsu_base::ledger::Repro::Dir { files, .. } = &ledger.find("E203").expect("E203 is in the ledger").repro else { panic!("E203 has no files") };
    let t = ritsu_testkit::TempDir::new("outside-root");
    // the project's root, by the file a worktree has for `.git`
    t.write("project/.git", "gitdir: elsewhere\n");
    for (name, body) in files.iter().filter(|(n, _)| !n.ends_with(".kn")) {
        match *name {
            "seats.rule" => t.write("shared/seats.rule", body),
            "booking.flow" => t.write("project/booking.flow", &body.replace("from \"seats.rule\"", "from \"../shared/seats.rule\"")),
            other => t.write(&format!("project/{other}"), body),
        };
    }
    let machine = t.path().to_string_lossy().to_string();
    let project = t.path().join("project");
    let run = |dir: &Path, args: &[&str]| -> (i32, String) {
        let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).args(args).current_dir(dir).env("NO_COLOR", "1").env_remove("RITSU_LANG").output().unwrap();
        (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
    };
    let mut wrong = Vec::new();
    for (dir, given, named) in [(project.as_path(), ".", "../shared/seats.rule"), (t.path(), "project", "shared/seats.rule")] {
        for lang in ["en", "ja"] {
            let (code, out) = run(dir, &["check", given, "--lang", lang]);
            if code != 1 || !out.contains(&format!("\"{named}\"")) || out.contains(&machine) {
                wrong.push(format!("`ritsu check {given} --lang {lang}` (exit {code}) does not name the rule `{named}`, or names a path of this machine:\n{out}"));
            }
        }
        let (code, out) = run(dir, &["check", given, "--format", "json"]);
        if code != 1 || !out.contains(&format!("\\\"{named}\\\"")) || out.contains(&machine) {
            wrong.push(format!("`ritsu check {given} --format json` (exit {code}) does not name the rule `{named}`, or names a path of this machine:\n{out}"));
        }
    }
    let given = project.to_string_lossy().to_string();
    let (_, out) = run(&project, &["check", &given, "--lang", "en"]);
    let folded = t.path().join("shared/seats.rule").to_string_lossy().to_string();
    if !out.contains(&format!("\"{folded}\"")) {
        wrong.push(format!("`ritsu check` of the project given absolute does not name the rule `{folded}`:\n{out}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// A request of ringi's that reads a rule from outside the project's root, which does not pass
/// rulec's check (ringi's E202): the line says the path as the request wrote it, and what rulec says
/// of the rule names it from where `ritsu check` runs, `.` and `..` folded, in both languages and in
/// the JSON, and absolute only where the project was given absolute.
#[test]
fn a_rule_a_request_reads_outside_the_root_is_named_from_where_the_run_is() {
    let mutants = tree().join("crates/ringi/tests/mutants");
    let t = ritsu_testkit::TempDir::new("request-outside-root");
    t.write("project/.git", "gitdir: elsewhere\n");
    let request = std::fs::read_to_string(mutants.join("E202_a_broken_rule.ringi")).unwrap();
    t.write("project/example.ringi", request.replace("\"shared/broken_authority.rule\"", "\"../shared/broken_authority.rule\""));
    t.write("shared/broken_authority.rule", std::fs::read_to_string(mutants.join("shared/broken_authority.rule")).unwrap());
    let machine = t.path().to_string_lossy().to_string();
    let project = t.path().join("project");
    let run = |dir: &Path, args: &[&str]| -> (i32, String) {
        let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).args(args).current_dir(dir).env("NO_COLOR", "1").env_remove("RITSU_LANG").output().unwrap();
        (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
    };
    let mut wrong = Vec::new();
    for (dir, given, named) in [(project.as_path(), ".", "../shared/broken_authority.rule"), (t.path(), "project", "shared/broken_authority.rule")] {
        for (args, written) in [
            (vec!["check", given, "--lang", "en"], "`../shared/broken_authority.rule` does not pass rulec's check"),
            (vec!["check", given, "--lang", "ja"], "`../shared/broken_authority.rule` は rulec の検査を通りません"),
            (vec!["check", given, "--format", "json"], "`../shared/broken_authority.rule` does not pass rulec's check"),
        ] {
            let (code, out) = run(dir, &args);
            // the place in rulec's words: `(…)` in English, `（`…`）` in Japanese
            let names = out.contains(&format!("({named}:12)")) || out.contains(&format!("`{named}:12`"));
            if code != 1 || !out.contains(written) || !names || out.contains(&machine) {
                wrong.push(format!("`ritsu {}` (exit {code}) does not say `{written}`, does not name the rule `{named}`, or names a path of this machine:\n{out}", args.join(" ")));
            }
        }
    }
    let given = project.to_string_lossy().to_string();
    let (_, out) = run(&project, &["check", &given, "--lang", "en"]);
    let folded = t.path().join("shared/broken_authority.rule").to_string_lossy().to_string();
    if !out.contains(&format!("({folded}:12)")) {
        wrong.push(format!("`ritsu check` of the project given absolute does not name the rule `{folded}`:\n{out}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// The files that read others by paths they write, one for each way of reading of each language
/// that has one: the command, the directory of the example from the root of the tree, and the file
/// in it.
const THROUGH_A_LINK: &[(&[&str], &str, &str)] = &[
    // a rule and an OpenAPI document; a book and a dates file; a child flow
    (&["dandori", "check"], "crates/dandori/examples/hotel", "temporal/hotel.flow"),
    (&["dandori", "check"], "crates/dandori/examples/invoice", "invoice.flow"),
    (&["dandori", "check"], "crates/dandori/tests/fixtures", "children.flow"),
    // a rule and a dates file; an enum a rule has, and a workflow; a gate
    (&["sekisho", "check"], "crates/sekisho/examples/refunds", "refunds.gate"),
    (&["sekisho", "check"], "crates/sekisho/examples/order_states", "orders.gate"),
    (&["sekisho", "check"], "crates/sekisho/tests/gen", "papers.gate"),
    // a rule and a book; a dates file
    (&["kaname", "check"], "crates/kaname/examples/payout", "payout.kn"),
    (&["kaname", "check"], "crates/kaname/examples/late_payment", "late_payment.kn"),
    // a calendar a dates file uses
    (&["koyomi", "check"], "crates/dandori/examples/invoice", "dates/payment_terms.cal"),
    // the rule a contribution adds to, and the contribution it comes after
    (&["rulec", "check"], "crates/ritsu/tests/projects/product_line/rules", "contrib/garden_price.rule"),
    // an organization, a calendar, a book, a gate and a shared route
    (&["ringi", "check"], "crates/ringi/examples", "leave.ringi"),
    // the same, through `ritsu check`
    (&["check"], "crates/dandori/examples/hotel", "temporal/hotel.flow"),
    (&["check"], "crates/kaname/examples/payout", "payout.kn"),
    (&["check"], "crates/sekisho/examples/refunds", "refunds.gate"),
    (&["check"], "crates/ritsu/tests/projects/product_line/rules", "contrib/garden_price.rule"),
    (&["check"], "crates/ringi/examples", "leave.ringi"),
];

/// A file of each kind that does not pass, `{m}` where it says which copy it is.
fn broken(ext: &str) -> &'static str {
    match ext {
        "rule" => "rule broken v1\n{m} this is not a rule\n",
        "cal" => "calendar broken\n{m} this is not a calendar\n",
        "book" => "book broken v1\n{m} this is not a book\n",
        "kn" => "module broken v1\n{m} this is not a module\n",
        "gate" => "gate broken v1\n{m} this is not a gate\n",
        "flow" => "workflow broken v1\n{m} this is not a flow\n",
        "json" => "{ \"{m}\": this is not json\n",
        "yaml" | "yml" => "{m}: [ this is not yaml\n",
        _ => "{m} broken copy\n",
    }
}

#[cfg(unix)]
fn link(to: &str, at: &Path) {
    std::os::unix::fs::symlink(to, at).unwrap_or_else(|e| panic!("cannot link {}: {e}", at.display()));
}

#[cfg(not(unix))]
fn link(_: &str, _: &Path) {
    unreachable!("links are made on Unix only")
}

/// Under `top`: `common/ex`, a copy of the example's directory, its file moved to `in/` with each
/// path it wrote to a file of the copy written again from there; `project/in`, a link to it; and in
/// the project, for each file read, a copy that does not pass where the letters of `in/../<file>`
/// lead, marked `LETTERS`. With `say`, the files read do not pass either, and are marked `REAL`.
/// The path to give, and the files read, from the copy's top.
fn through_a_link(top: &Path, example: &Path, file: &str, say: bool) -> (String, Vec<String>) {
    let ex = top.join("common/ex");
    ritsu_testkit::tmp::copy_dir(example, &ex);
    let here = ex.join(file).parent().unwrap().to_path_buf();
    let text = std::fs::read_to_string(ex.join(file)).unwrap();
    let mut read = Vec::new();
    let mut moved = String::new();
    for line in text.split_inclusive('\n') {
        let parts: Vec<&str> = line.split('"').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                moved.push('"');
            }
            let target = fold(&here.join(part));
            let rel = target.strip_prefix(&ex).ok().map(|r| r.to_string_lossy().to_string());
            match rel {
                Some(rel) if i % 2 == 1 && i + 1 < parts.len() && !part.is_empty() && target.is_file() && rel != file => {
                    moved.push_str(&format!("../{rel}"));
                    read.push(rel);
                }
                _ => moved.push_str(part),
            }
        }
    }
    read.sort();
    read.dedup();
    let name = Path::new(file).file_name().unwrap().to_string_lossy().to_string();
    std::fs::create_dir_all(ex.join("in")).unwrap();
    std::fs::write(ex.join("in").join(&name), moved).unwrap();
    std::fs::remove_file(ex.join(file)).unwrap();
    std::fs::create_dir_all(top.join("project")).unwrap();
    link("../common/ex/in", &top.join("project/in"));
    for rel in &read {
        let ext = Path::new(rel).extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        let at = top.join("project").join(rel);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, broken(&ext).replace("{m}", "LETTERS")).unwrap();
        if say {
            std::fs::write(ex.join(rel), broken(&ext).replace("{m}", "REAL")).unwrap();
        }
    }
    (format!("in/{name}"), read)
}

/// The exit and what a command of ritsu printed, run in `cwd`.
fn printed(cwd: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).args(args).args(["--lang", "en"]).current_dir(cwd).env("NO_COLOR", "1").env_remove("RITSU_LANG").output().unwrap();
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

/// A file read through a path whose `..` climbs out of a symbolic link to a directory is the file
/// the system opens there (DESIGN 4.15). For each language that reads files by the paths it writes,
/// an example's file is moved to `in/` of a copy of its directory and reached from a project beside
/// the copy through a link ([`through_a_link`]); where the letters of each path it wrote lead from
/// the project, a copy that does not pass stands. Run in the project, the check says what it says
/// run in the copy, its paths written from the project, and reads none of those copies. With the
/// files read made not to pass too, every path it says of one opens, from the project, the file it
/// read, or is the path as the file wrote it, which opens it from `in/`.
#[test]
fn a_path_that_climbs_out_of_a_link_reads_the_file_the_system_opens() {
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    let t = ritsu_testkit::TempDir::new("through-a-link");
    let top = std::fs::canonicalize(t.path()).unwrap();
    let mut wrong = Vec::new();
    for (k, (command, example, file)) in THROUGH_A_LINK.iter().enumerate() {
        let case = top.join(format!("{k}"));
        let (given, read) = through_a_link(&case.join("read"), &tree().join(example), file, false);
        assert!(!read.is_empty(), "{example}/{file} reads no file of its directory by a path it writes");
        let args: Vec<&str> = command.iter().copied().chain([given.as_str()]).collect();
        let shown = format!("`ritsu {}` of {example}/{file}", args.join(" "));
        let (plain_code, plain) = printed(&case.join("read/common/ex"), &args);
        let (code, out) = printed(&case.join("read/project"), &args);
        let unplaced = |s: &str| s.replace(&case.join("read/project").to_string_lossy().to_string(), "<top>").replace(&case.join("read/common/ex").to_string_lossy().to_string(), "<top>");
        if out.contains("LETTERS") || code != plain_code || unplaced(&out.replace("../common/ex/", "")) != unplaced(&plain) {
            wrong.push(format!("{shown}, through the link, does not read what it reads beside it (exit {code}, beside it {plain_code}):\n{out}\n-- beside it:\n{plain}"));
        }
        let (given, read) = through_a_link(&case.join("say"), &tree().join(example), file, true);
        let project = case.join("say/project");
        let real: Vec<PathBuf> = read.iter().map(|r| std::fs::canonicalize(case.join("say/common/ex").join(r)).unwrap()).collect();
        let letters: Vec<PathBuf> = read.iter().map(|r| std::fs::canonicalize(project.join(r)).unwrap()).collect();
        let args: Vec<&str> = command.iter().copied().chain([given.as_str()]).collect();
        let (_, out) = printed(&project, &args);
        if out.contains("LETTERS") {
            wrong.push(format!("{shown}, through the link, reads a copy the letters of a path lead to:\n{out}"));
        }
        let names: Vec<String> = read.iter().map(|r| Path::new(r).file_name().unwrap().to_string_lossy().to_string()).collect();
        for w in out.split(|c: char| c.is_whitespace() || "`'\"(),:".contains(c)).map(as_path) {
            if !names.iter().any(|n| w.ends_with(n.as_str())) {
                continue;
            }
            let from = |base: &Path| std::fs::canonicalize(base.join(w)).ok();
            let opens = |p: Option<PathBuf>, set: &[PathBuf]| p.is_some_and(|p| set.contains(&p));
            if opens(from(&project), &letters) || !(opens(from(&project), &real) || opens(from(&project.join("in")), &real)) {
                wrong.push(format!("{shown}, through the link, says `{w}`, which does not open the file it read from the project:\n{out}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// Two files the letters of one path name, one of them reached by climbing out of a link, are two
/// files to the ports a run joins (DESIGN 4.15): each port answers for the file the system opens at
/// the path it is asked by, whichever of the two is asked first.
#[test]
fn two_files_the_letters_of_one_path_name_are_two_files() {
    use ritsu_ports::{Books, Dates, Rules};
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    let t = ritsu_testkit::TempDir::new("two-files");
    let top = std::fs::canonicalize(t.path()).unwrap();
    for (from, to) in [("crates/kaname/examples/payout/rules", "rules"), ("crates/kaname/examples/payout/books", "books"), ("crates/dandori/examples/invoice/dates", "dates"), ("crates/dandori/examples/invoice/calendars", "calendars")] {
        ritsu_testkit::tmp::copy_dir(&tree().join(from), &top.join("common/ex").join(to));
        ritsu_testkit::tmp::copy_dir(&tree().join(from), &top.join("project").join(to));
    }
    std::fs::create_dir_all(top.join("common/ex/in")).unwrap();
    link("../common/ex/in", &top.join("project/in"));
    let files = ["rules/card_fee.rule", "books/ledger.book", "dates/payment_terms.cal"];
    for f in files {
        let p = top.join("project").join(f);
        let text = std::fs::read_to_string(&p).unwrap().replacen("description \"", "description \"Another copy. ", 1);
        std::fs::write(&p, text).unwrap();
    }
    let sha = |p: &Path| ritsu_base::sha256::hex(&std::fs::read(p).unwrap());
    let mut wrong = Vec::new();
    for real_first in [true, false] {
        let joined = ritsu_project::Joined::new();
        for f in files {
            let (real, letters) = (top.join("project/in/..").join(f), top.join("project").join(f));
            assert_ne!(sha(&real), sha(&letters), "{f}: the two copies read the same");
            let asked = if real_first { [real, letters] } else { [letters, real] };
            for p in asked {
                let got = match p.extension().and_then(|e| e.to_str()) {
                    Some("rule") => joined.rulec.facts(&p).map(|x| x.sha256),
                    Some("book") => joined.chobo.facts(&p).map(|x| x.sha256),
                    _ => joined.koyomi.facts(&p).map(|x| x.sha256),
                };
                if got.as_ref().ok() != Some(&sha(&p)) {
                    wrong.push(format!("{} (asked {}): the port answers for another file: {got:?}", p.display(), if real_first { "real first" } else { "letters first" }));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The commands given a path whose `..` climbs out of a symbolic link, `{f}` where it goes: ritsu's
/// own (`{gen}` an output directory), and those of the languages that find a root above the file or
/// name it from there; each with the directory of an example from the root of the tree, and the
/// file in it.
const GIVEN_THROUGH_A_LINK: &[(&[&str], &str, &str)] = &[
    (&["check", "{f}"], "crates/kaname/examples/payout", "rules/card_fee.rule"),
    (&["check", "{f}", "--format", "json"], "crates/kaname/examples/payout", "payout.kn"),
    (&["check", "{f}"], "crates/dandori/examples/hotel", "temporal/hotel.flow"),
    (&["check", "{f}"], "crates/sekisho/examples/refunds", "refunds.gate"),
    (&["gen", "{f}", "--target", "typescript", "--out", "{gen}", "--format", "json"], "crates/kaname/examples/payout", "rules/card_fee.rule"),
    (&["rulec", "check", "{f}"], "crates/kaname/examples/payout", "rules/card_fee.rule"),
    (&["kaname", "check", "{f}", "--format", "json"], "crates/kaname/examples/payout", "payout.kn"),
    (&["sekisho", "check", "{f}"], "crates/sekisho/examples/refunds", "refunds.gate"),
    (&["yuen", "check", "{f}"], "crates/yuen/examples/greeter", "greeter.req"),
    (&["sakai", "check", "{f}"], "crates/sakai/examples/shop", "shop.ctx"),
    (&["check", "{f}"], "crates/ringi/examples", "leave.ringi"),
    (&["ringi", "check", "{f}", "--format", "json"], "crates/ringi/examples", "leave.ringi"),
    (&["ringi", "doc", "{f}"], "crates/ringi/examples", "leave.ringi"),
    (&["ringi", "diff", "{f}", "{f}"], "crates/ringi/examples", "leave.ringi"),
];

/// Under `top`: a copy of the example's directory with an empty `in/`, and a project with a `.git`
/// of its own whose `in` is a link to that `in/`. Where the letters of `in/../<file>` lead from the
/// project, a copy of the file that does not pass, marked `LETTERS`. With `inside`, the copy is under
/// the project's root (`P/common/ex`); else it is beside the project, with a `.git` of its own
/// (`common/ex`, `project`). The project, and the path from it to the file without the link.
fn given_through_a_link(top: &Path, example: &Path, file: &str, inside: bool) -> (PathBuf, String) {
    let (project, ex, to, real) = if inside {
        (top.join("P"), top.join("P/common/ex"), "common/ex/in", format!("common/ex/{file}"))
    } else {
        (top.join("project"), top.join("common/ex"), "../common/ex/in", format!("../common/ex/{file}"))
    };
    ritsu_testkit::tmp::copy_dir(example, &ex);
    std::fs::create_dir_all(ex.join("in")).unwrap();
    if !inside {
        std::fs::write(ex.join(".git"), "gitdir: elsewhere\n").unwrap();
    }
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join(".git"), "gitdir: elsewhere\n").unwrap();
    link(to, &project.join("in"));
    let ext = Path::new(file).extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
    let at = project.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(&at, broken(&ext).replace("{m}", "LETTERS")).unwrap();
    (project, real)
}

/// Where the link leads, as a case says it.
fn led(inside: bool) -> &'static str {
    if inside { "the link leads inside the root" } else { "the link leads outside the root" }
}

/// A path given to a command whose `..` climbs out of a symbolic link is the file the system opens
/// there (DESIGN 4.15, 6.2): the command finds the root above that file and names the file from it,
/// so it does what it does given the path to the file without the link, whether the link leads
/// inside the project's root or out of it ([`given_through_a_link`]). It reads no copy the letters of
/// the path lead to. A path the command prints as it was given is put in place of the other.
#[test]
fn a_path_given_through_a_link_is_the_file_the_system_opens() {
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    let t = ritsu_testkit::TempDir::new("given-through-a-link");
    let top = std::fs::canonicalize(t.path()).unwrap();
    let mut wrong = Vec::new();
    for (k, (command, example, file)) in GIVEN_THROUGH_A_LINK.iter().enumerate() {
        for inside in [false, true] {
            let case = top.join(format!("{k}-{inside}"));
            let (project, real) = given_through_a_link(&case, &tree().join(example), file, inside);
            let given = format!("in/../{file}");
            let (gen_link, gen_real) = (case.join("gen/link").to_string_lossy().to_string(), case.join("gen/real").to_string_lossy().to_string());
            let args = |path: &str, out_dir: &str| -> Vec<String> { command.iter().map(|a| a.replace("{f}", path).replace("{gen}", out_dir)).collect() };
            let (link_args, real_args) = (args(&given, &gen_link), args(&real, &gen_real));
            let (code, out) = printed(&project, &link_args.iter().map(String::as_str).collect::<Vec<_>>());
            let (real_code, real_out) = printed(&project, &real_args.iter().map(String::as_str).collect::<Vec<_>>());
            if out.contains("LETTERS") || code != real_code || out.replace(&gen_link, "<out>").replace(&given, &real) != real_out.replace(&gen_real, "<out>") {
                wrong.push(format!("`ritsu {}` ({}) does not do what `ritsu {}` does (exit {code}, {real_code}):\n{out}\n-- without the link:\n{real_out}", link_args.join(" "), led(inside), real_args.join(" ")));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// Two files the letters of one path name, one given through a link, are two files to `ritsu check`
/// and `ritsu gen`, and one file given by two paths is one (DESIGN 4.15): each run does what it does
/// given the path to the file without the link. Where the link leads inside the root, the two are
/// checked together; where it leads out of it, the root is found above the file the first path opens,
/// as for the path without the link, and a root that holds both (`--root ..`) checks the two. `ritsu
/// gen` refuses the file outside the root it is given (`--root .`) as it refuses the path to it
/// without the link.
#[test]
fn two_files_given_by_the_letters_of_one_path_are_two_files() {
    if !cfg!(unix) {
        ritsu_testkit::skip("the test makes symbolic links, which it makes on Unix only");
        return;
    }
    let t = ritsu_testkit::TempDir::new("two-given");
    let top = std::fs::canonicalize(t.path()).unwrap();
    let rule = std::fs::read_to_string(tree().join("crates/rulec/tests/corpus/average_unit_price.rule")).unwrap();
    let mut wrong = Vec::new();
    for inside in [false, true] {
        let case = top.join(format!("{inside}"));
        let example = case.join("example");
        std::fs::create_dir_all(example.join("rules")).unwrap();
        std::fs::write(example.join("rules/fee.rule"), &rule).unwrap();
        let (project, real) = given_through_a_link(&case, &example, "rules/fee.rule", inside);
        let given = "in/../rules/fee.rule";
        let out_dir = case.join("gen").to_string_lossy().to_string();
        // the arguments through the link; each `{f}` is the path given, the same without the link
        let runs: Vec<(Vec<&str>, Option<&str>)> = vec![
            (vec!["check", "{f}"], Some("ritsu check: 1 file")),
            (vec!["check", "{f}", "rules/fee.rule"], if inside { Some("ritsu check: 2 files") } else { Some("is outside the root") }),
            (vec!["check", "{f}", "{f}"], Some("ritsu check: 1 file")),
            (vec!["check", "--root", "..", "{f}", "rules/fee.rule"], (!inside).then_some("ritsu check: 2 files")),
            (vec!["gen", "{f}", "--root", ".", "--target", "typescript", "--out", &out_dir], Some(if inside { "generated:" } else { "is outside the root" })),
        ];
        for (args, says) in runs {
            if inside && args.contains(&"--root") && args.contains(&"..") {
                continue;
            }
            // `{f}` twice: the second time, the path without the link
            let mut seen = 0;
            let link_args: Vec<String> = args
                .iter()
                .map(|a| {
                    if *a != "{f}" {
                        return a.to_string();
                    }
                    seen += 1;
                    if seen == 1 { given.to_string() } else { real.clone() }
                })
                .collect();
            let real_args: Vec<String> = args.iter().map(|a| if *a == "{f}" { real.clone() } else { a.to_string() }).collect();
            let _ = std::fs::remove_dir_all(&out_dir);
            let (code, out) = printed(&project, &link_args.iter().map(String::as_str).collect::<Vec<_>>());
            let _ = std::fs::remove_dir_all(&out_dir);
            let (real_code, real_out) = printed(&project, &real_args.iter().map(String::as_str).collect::<Vec<_>>());
            let twice = real_args.iter().filter(|a| **a == real).count() > 1;
            // the path without the link given twice is one file, and said once
            let expected = if twice { printed(&project, &["check", &real]) } else { (real_code, real_out.clone()) };
            // the copy the letters lead to is read only where it is given by its own path
            if (out.contains("LETTERS") && !expected.1.contains("LETTERS")) || code != expected.0 || out.replace(given, &real) != expected.1 || says.is_some_and(|s| !out.contains(s)) {
                wrong.push(format!(
                    "`ritsu {}` ({}, exit {code}) does not do what `ritsu {}` does (exit {}){}:\n{out}\n-- without the link:\n{}",
                    link_args.join(" "),
                    led(inside),
                    if twice { format!("check {real}") } else { real_args.join(" ") },
                    expected.0,
                    says.map(|s| format!(", or does not say `{s}`")).unwrap_or_default(),
                    expected.1
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// Commands whose flags take a path (DESIGN 4.15, 6.2 item 8): each run in a copy of an example's
/// directory, `{o}` the path the flag takes, the path, and what the run says of it.
const FLAG_PATHS: &[(&[&str], &str, &str, &str)] = &[
    (&["rulec", "gen", "site_capacity.rule", "--target", "typescript", "--out", "{o}"], "crates/rulec/tests/merging/sum", "out", "generated: out/typescript/site_capacity.ts"),
    (&["rulec", "run", "site_capacity.rule", "--input", "{o}"], "crates/rulec/tests/merging/sum", "no.json", "error: `no.json` is not there"),
    (&["koyomi", "gen", "late_fee.cal", "--target", "typescript", "--out", "{o}"], "crates/koyomi/examples", "out", "generated: out/typescript/late_fee.ts"),
    (&["chobo", "scenarios", "inventory.book", "--out", "{o}"], "crates/chobo/examples/inventory", "out", "written to out"),
    (&["dandori", "doc", "temporal/hotel.flow", "--out", "{o}"], "crates/dandori/examples/hotel", "out", "out/hotel.md"),
    // with the note of an arm no scenario takes after it (dandori's DESIGN 36)
    (&["dandori", "scenarios", "tool_orders.flow", "--out", "{o}"], "crates/ritsu/tests/projects/rule_arms", "out", "7 scenario(s) written to out\nnote: no scenario takes the arm `review`"),
    (&["sekisho", "gen", "refunds.gate", "--target", "cedar", "--out", "{o}"], "crates/sekisho/examples/refunds", "out", "generated: out/cedar/refunds.cedar"),
    (&["yuen", "doc", "refunds.req", "--out", "{o}"], "crates/yuen/examples/refunds", "out", "wrote: out/refunds.md"),
    (&["sakai", "doc", "shop.ctx", "--out", "{o}"], "crates/sakai/examples/shop", "out", "out/shop.md"),
    (&["run", "temporal/hotel.flow", "--scenario", "{o}"], "crates/dandori/examples/hotel", "no.json", "error: `no.json` is not there"),
    (&["ringi", "extract", "expense.ringi", "--out", "{o}"], "crates/ringi/examples", "out", "wrote out/choices.rule"),
    (&["ringi", "check", "leave.ringi", "--org", "{o}"], "crates/ringi/examples", "org", "(data `org`)"),
    (&["ringi", "doc", "leave.ringi", "--org", "{o}"], "crates/ringi/examples", "org", "(data `org`)"),
    (&["ringi", "diff", "leave.ringi", "leave.ringi", "--org", "{o}"], "crates/ringi/examples", "org", "The two say the same and answer alike."),
];

/// A path a flag takes is said as one path however it is written (DESIGN 4.15, 6.2 item 8):
/// `--out out`, `./out`, `out/` and `x/../out` (`x` a directory, no link) write the same files and
/// say the same, byte for byte, and say `out`; a path given absolute is said absolute, folded. (A
/// path whose `x` is not there is kept as written, as the system takes it: `tests/paths.rs` of
/// ritsu-base.)
#[test]
fn a_path_a_flag_takes_is_one_path_however_it_is_written() {
    let t = ritsu_testkit::TempDir::new("flag-paths");
    let mut wrong = Vec::new();
    for (i, (args, example, path, says)) in FLAG_PATHS.iter().enumerate() {
        let mut first: Option<(i32, String)> = None;
        // `out/` for a directory; a file is no directory
        let dir = !path.contains('.');
        let spellings: Vec<String> = [Some(path.to_string()), Some(format!("./{path}")), dir.then(|| format!("{path}/")), Some(format!("x/../{path}"))].into_iter().flatten().collect();
        for (k, spelled) in spellings.iter().enumerate() {
            let ex = t.path().join(format!("{i}/{k}"));
            ritsu_testkit::tmp::copy_dir(&tree().join(example), &ex);
            std::fs::write(ex.join(".git"), "gitdir: elsewhere\n").unwrap();
            std::fs::create_dir_all(ex.join("x")).unwrap();
            let given: Vec<&str> = args.iter().map(|a| if *a == "{o}" { spelled.as_str() } else { a }).collect();
            let said = printed(&ex, &given);
            match &first {
                None => {
                    if !said.1.contains(says) {
                        wrong.push(format!("`ritsu {}` does not say `{says}`:\n{}", given.join(" "), said.1));
                    }
                    first = Some(said);
                }
                Some(f) if *f != said => wrong.push(format!("`ritsu {}` (exit {}) does not say what `{path}` says (exit {}):\n{}\n-- `{path}`:\n{}", given.join(" "), said.0, f.0, said.1, f.1)),
                Some(_) => {}
            }
        }
    }
    let ex = t.path().join("absolute");
    ritsu_testkit::tmp::copy_dir(&tree().join("crates/rulec/tests/merging/sum"), &ex);
    std::fs::write(ex.join(".git"), "gitdir: elsewhere\n").unwrap();
    // `x` is there: a path through a directory that is not there is kept as written
    std::fs::create_dir_all(ex.join("x")).unwrap();
    let out = std::fs::canonicalize(&ex).unwrap().join("out");
    let given = format!("{}/x/../out", out.parent().unwrap().display());
    let (_, said) = printed(&ex, &["rulec", "gen", "site_capacity.rule", "--target", "typescript", "--out", &given]);
    if !said.contains(&format!("generated: {}/typescript/site_capacity.ts", out.display())) {
        wrong.push(format!("`--out {given}` is not said absolute and folded:\n{said}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// What `ritsu run` says after the run of the answers a scenario leaves, and what `ritsu dandori
/// scenarios` says after the scenarios of an arm no scenario takes (dandori's DESIGN 36), name no
/// path: the flow given the four ways (the scenario absolute), each says the same, the flow put back
/// to its name.
#[test]
fn what_is_said_after_a_run_and_after_the_scenarios_does_not_turn_on_how_the_flow_is_given() {
    let root = tree();
    let t = ritsu_testkit::TempDir::new("said-after");
    let sc = t.path().join("left_over.json");
    std::fs::write(&sc, r#"{"input": {"order": "A-1", "tier": "basic"}, "answers": [{"ok": 1}, {"ok": null}, {"ok": 2}]}"#).unwrap();
    let sc = sc.to_string_lossy().to_string();
    let rel = "crates/ritsu/tests/projects/rule_arms/tool_orders.flow";
    let ways = ways(&root, rel);
    let name = &ways[0].1;
    let runs: [(&[&str], &str); 2] = [
        (&["run", "{f}", "--scenario", &sc], "error: the run did not take answer 3 of the scenario: {\"ok\":2}\n"),
        (&["dandori", "scenarios", "{f}"], "note: no scenario takes the arm `review` of the `match` on line 26: rulec says the rule tool_check never comes to `route` = review with what the workflow can give it on line 25\n"),
    ];
    let mut wrong = Vec::new();
    for (args, says) in runs {
        let outs: Vec<(i32, String)> = ways
            .iter()
            .map(|(cwd, given)| {
                let args: Vec<&str> = args.iter().map(|a| if *a == "{f}" { given.as_str() } else { a }).collect();
                let (code, out) = printed(cwd, &args);
                (code, out.replace(given.as_str(), name))
            })
            .collect();
        if !outs[0].1.contains(says) {
            wrong.push(format!("`ritsu {}` does not say `{says}`:\n{}", args.join(" "), outs[0].1));
        }
        for (k, out) in outs.iter().enumerate().skip(1) {
            if *out != outs[0] {
                wrong.push(format!("`ritsu {}`, the flow given {} (exit {}), does not say what it says given its name (exit {}):\n{}\n-- by its name:\n{}", args.join(" "), SPELLED[k], out.0, outs[0].0, out.1, outs[0].1));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// In a module, a path whose first part starts with `@` names a dependency from the module's root
/// (DESIGN 17.6), so a file below the working directory whose name starts with `@` is said
/// `./@notes.rule` (DESIGN 4.15): given so, and found by a walk of `.`. That is the path a command run
/// there reads it by; `@notes.rule` is refused as no dependency.
#[test]
fn a_file_whose_name_starts_with_an_at_is_said_from_where_the_run_is() {
    let t = ritsu_testkit::TempDir::new("at-name");
    t.write("shop/ritsu.mod", "module example.com/acme/shop\nversion 0.1.0\n");
    t.write("shop/.git", "gitdir: elsewhere\n");
    t.write("shop/rules/@notes.rule", &std::fs::read_to_string(tree().join("crates/rulec/tests/corpus/province_growth.rule")).unwrap());
    let rules = t.path().join("shop/rules");
    std::fs::create_dir_all(rules.join("x")).unwrap();
    for given in ["./@notes.rule", "x/../@notes.rule", "."] {
        let (code, out) = printed(&rules, &["rulec", "check", given]);
        assert!(code == 0 && out.contains("ok ./@notes.rule") && !out.contains(" @notes.rule"), "`rulec check {given}` (exit {code}):\n{out}");
    }
    let (code, out) = printed(&rules, &["rulec", "check", "@notes.rule"]);
    assert!(code == 2 && out.contains("is no dependency"), "`rulec check @notes.rule` (exit {code}):\n{out}");
}
