//! The characters a unit is written with (DESIGN 5.1): every language that writes a unit reads every
//! spelling of the table of units as rulec reads it, in a type's brackets and after the numbers of a
//! range — the signs ℃, ℉ and % as the letters of `kg` and `USD`, and the full-width `％` as `%`.
//! ritsu-units says which characters those are (`ritsu_units::chars`); the lexers of sekisho,
//! dandori and chobo read a unit with them, and rulec's and kaname's read the same ones.
//!
//! One project holds a rule of rulec's for each spelling, a gate of sekisho's and a flow of
//! dandori's with an attribute and an input in each, a module of kaname's with a function on each,
//! and a book of chobo's with a unit of each that a word spells (a rate is no unit of a book). A
//! rate's type writes its step (`rate[step 1%]`), and `%` is the unit of its range. `ritsu check`
//! passes every file, with nothing to say but that the book's units and the module's comparisons
//! are left unused.

use ritsu_testkit::TempDir;
use ritsu_units::{Dim, table};
use std::path::Path;
use std::process::Command;

fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).current_dir(dir).args(args).env_remove("RITSU_LANG").output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every spelling of the table with the type that takes it, and the full-width `％` with a rate's.
fn spellings() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = table::spellings()
        .into_iter()
        .map(|s| {
            let ty = match table::unit(&s).map(|(d, _)| d) {
                Some(Dim::Rate) => "rate[step 1%]".to_string(),
                Some(Dim::Money(_)) => format!("money[{s}]"),
                Some(d) => format!("{}[{s}]", d.word()),
                None => panic!("{s} is a spelling of the table"),
            };
            (s, ty)
        })
        .collect();
    out.push(("％".into(), "rate[step 1%]".into()));
    out
}

#[test]
fn every_language_reads_every_spelling_of_the_table() {
    let all = spellings();
    assert!(all.iter().any(|(s, _)| s == "℃") && all.iter().any(|(s, _)| s == "℉") && all.iter().any(|(s, _)| s == "%"), "the signs are spellings of the table");
    let t = TempDir::new("spellings");
    let (mut attributes, mut inputs, mut functions, mut units) = (String::new(), String::new(), String::new(), String::new());
    for (i, (s, ty)) in all.iter().enumerate() {
        let range = format!("range >=1{s} <=2{s}");
        t.write(
            &format!("rules/u{i}.rule"),
            &format!("rule u{i} v1\ndescription \"A rule on an input in {s}\"\n\ninputs\n  x : {ty}  {range}\n\noutputs\n  low : bool\n\ntable t\npolicy unique\n| x | -> low : bool |\n| <=1{s} | true |\n| >1{s} | false |\n"),
        );
        attributes.push_str(&format!("    a{i} : {ty}  {range}\n"));
        inputs.push_str(&format!("  a{i} : {ty}  {range}\n"));
        functions.push_str(&format!("\nfn f{i}(x: {ty} {range}) -> bool\n  x > 1{s}\n"));
        if !ty.starts_with("rate") {
            units.push_str(&format!("unit {s}\n"));
        }
    }
    t.write(
        "spellings.gate",
        &format!("gate spellings v1\ndescription \"An attribute in each spelling of the table of units\"\nnamespace Units\n\nrole reader\n  description \"Reads the measures\"\n  can read\n\nprincipal User\n  description \"A person\"\n  roles reader\n\nresource Measure\n  description \"The measures\"\n  attributes\n{attributes}\naction read\n  description \"Read the measures\"\n  principal User\n  resource Measure from measureId\n\npermit readers_read\n  description \"A reader reads them\"\n  principal in reader\n  action read\n"),
    );
    t.write("spellings.flow", &format!("workflow spellings v1\ndescription \"An input in each spelling of the table of units\"\n\ninputs\n{inputs}\nflow\n  pass\n"));
    t.write("spellings.kn", &format!("module spellings v1\ndescription \"A function on each spelling of the table of units\"\n{functions}"));
    t.write("spellings.book", &format!("book spellings v1\ndescription \"A unit of each spelling of the table of units that a word spells\"\n\n{units}"));
    let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}{err}"));
    let said: Vec<String> = v["diagnostics"].as_array().unwrap().iter().filter(|d| !matches!((d["tool"].as_str(), d["code"].as_str()), (Some("chobo"), Some("W105")) | (Some("kaname"), Some("W501")))).map(|d| format!("{} {} {}:{} {}", d["tool"], d["code"], d["file"], d["line"], d["message"])).collect();
    assert!(code == 0 && said.is_empty(), "{}", said.join("\n"));
    let files = v["files"].as_array().unwrap();
    assert_eq!(files.len(), all.len() + 4);
    assert!(files.iter().all(|f| f["ok"] == true));
    println!("rulec, sekisho, dandori, kaname and chobo read the {} spellings of the table and `％`", all.len() - 1);
}
