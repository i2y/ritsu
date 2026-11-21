//! The screens work out what the entry `worked` answers (DESIGN 13.7): for every kind of the
//! examples, and for the machine's tests whose conditions and computations read fields that may be
//! empty, rows, rates and a field shown on a field shown on a condition (`tests/machine/`), each
//! vector of the entry (`vectors/worked/<name>.jsonl`) is given to the functions of the screens, the
//! form with what an approver gives laid over it (of the fields they give alone, a value of null
//! keeping the form's). A field the vector says is shown is shown, and one it says is hidden or not
//! decided is hidden; so with a field asked for; and a value the screens work out is the vector's,
//! none while it is not decided. The screens work out no computation of a row and no day: those are
//! the machine's alone, and the reference's tests hold them.
//!
//! Forms with more rows than their field takes are compared too, and counted: the screens and the
//! entry both leave what reads those rows not decided (the code the entry is generated as gives the
//! rule no more rows than the field takes, and the check of the rows' count stops the form).

use super::forms::{fields_of, held};
use super::{file, made_of, node};
use ritsu_base::json::{self, Json};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// The machine's tests the vectors are run on beside the examples.
const MACHINE: [&str; 4] = ["tests/machine/absent.ringi", "tests/machine/ranges.ringi", "tests/machine/steps.ringi", "tests/machine/supplies.ringi"];

/// What runs the functions over the vectors: each function's answer for the form, against what the
/// vector says it is.
const DRIVER: &str = r#"import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const dir = process.argv[2];
const { functions } = await import(pathToFileURL(join(dir, "functions.js")).href);
const cases = JSON.parse(readFileSync(join(dir, "cases.json"), "utf8"));
const canon = (v) => JSON.stringify(v === undefined ? null : v);
const failures = [];
let compared = 0;
for (const c of cases) {
  for (const [name, want] of c.expect) {
    const f = functions[name];
    if (typeof f !== "function") {
      failures.push(`${c.from}: ${name} is not defined`);
      continue;
    }
    compared++;
    const got = f({ form: c.form });
    if (canon(got) !== canon(want)) failures.push(`${c.from}: ${name} is ${canon(got)}, and the vector says ${canon(want)}`);
  }
}
console.log(JSON.stringify({ cases: cases.length, compared, failures: failures.length, first: failures.slice(0, 10) }));
"#;

#[test]
fn the_screens_work_out_what_the_entry_worked_answers() {
    if !node() {
        return;
    }
    let mut paths: Vec<PathBuf> = crate::common::example_files().into_iter().filter(|f| !f.starts_with("examples/shared/")).map(PathBuf::from).collect();
    paths.extend(MACHINE.iter().map(PathBuf::from));
    let m = made_of(&paths);
    let mut cases = Vec::new();
    let mut table = Vec::new();
    let mut over = 0;
    for k in &m.kinds {
        let name = &k.named.name;
        let types = json::parse(file(&m, &format!("types/{name}.json"))).unwrap();
        let fields = fields_of(types.get("form").unwrap());
        // the fields the screens work out the value of
        let worked: Vec<&str> = types.get("form").and_then(Json::as_arr).unwrap_or_default().iter().filter(|f| f.get("worked_on_screen") == Some(&Json::Bool(true))).filter_map(|f| f.get("name").and_then(Json::as_str)).collect();
        let editable: Vec<&str> = k.edited().map(|f| f.named.name.as_str()).collect();
        // the most rows each field of rows takes
        let most: Vec<(&str, usize)> = types
            .get("form")
            .and_then(Json::as_arr)
            .unwrap_or_default()
            .iter()
            .filter_map(|f| Some((f.get("name").and_then(Json::as_str)?, usize::try_from(f.get("count_max").and_then(Json::as_int)?).ok()?)))
            .collect();
        let before = cases.len();
        for line in file(&m, &format!("vectors/worked/{name}.jsonl")).lines() {
            let v = json::parse(line).unwrap();
            let input = v.get("input").unwrap();
            // the form with what the approver gives laid over it
            let mut form: Vec<(String, Json)> = input.get("form").and_then(Json::as_obj).unwrap_or_default().to_vec();
            for (f, x) in input.get("edited").and_then(Json::as_obj).unwrap_or_default() {
                if *x == Json::Null || !editable.contains(&f.as_str()) {
                    continue;
                }
                form.retain(|(n, _)| n != f);
                form.push((f.clone(), x.clone()));
            }
            let past = form.iter().any(|(n, x)| most.iter().any(|(m, hi)| m == n && x.as_arr().is_some_and(|rows| rows.len() > *hi)));
            let shown = Json::obj(form.iter().map(|(n, x)| (n.clone(), fields.iter().find(|f| f.name == *n).map(|f| held(f, x)).unwrap_or_else(|| x.clone()))));
            let mut expect = Vec::new();
            for (f, said) in v.get("answer").and_then(|a| a.get("fields")).and_then(Json::as_obj).unwrap_or_default() {
                if let Some(s) = said.get("shown") {
                    expect.push(Json::arr([Json::str(format!("{name}.shown_{f}")), Json::Bool(*s == Json::Bool(true))]));
                }
                if let Some(r) = said.get("required") {
                    expect.push(Json::arr([Json::str(format!("{name}.required_{f}")), Json::Bool(*r == Json::Bool(true))]));
                }
                if let Some(x) = said.get("value").filter(|_| worked.contains(&f.as_str())) {
                    expect.push(Json::arr([Json::str(format!("{name}.{f}")), x.clone()]));
                }
            }
            if expect.is_empty() {
                continue;
            }
            over += usize::from(past);
            let from = format!("{name}, {}", v.get("from").and_then(Json::as_str).unwrap_or_default());
            cases.push(Json::obj([("from", Json::str(from)), ("form", shown), ("expect", Json::Arr(expect))]));
        }
        table.push(format!("{name} {}", cases.len() - before));
    }
    let t = ritsu_testkit::TempDir::new("ringi-a2ui-worked");
    t.write("functions.js", file(&m, "a2ui/functions.js"));
    t.write("package.json", "{\"type\": \"module\"}\n");
    t.write("cases.json", Json::Arr(cases).compact());
    t.write("driver.mjs", DRIVER);
    let r = ritsu_testkit::run::finish(Command::new("node").arg(t.path().join("driver.mjs")).arg(t.path()), Duration::from_secs(300), "node, running the functions over the vectors of `worked`");
    assert!(r.ok, "{}", r.both());
    let got = json::parse(r.stdout.trim()).unwrap_or_else(|e| panic!("{e:?}: {}", r.stdout));
    let n = |k: &str| got.get(k).and_then(Json::as_int).unwrap_or(-1);
    println!("forms of the vectors of `worked` by kind: {}\n{} answers of the screens' functions compared on {} forms, {over} of them with more rows than their field takes", table.join(", "), n("compared"), n("cases"));
    assert_eq!(n("failures"), 0, "{}", got.get("first").map(Json::pretty).unwrap_or_default());
    assert!(n("compared") > 500 && over > 0, "{}", got.compact());
}
