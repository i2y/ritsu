//! The example (DESIGN 11; PLAN C.0): its map passes check with the numbers DESIGN 3.1 shows, what
//! was copied from the suite still passes each tool's own check (rulec, koyomi, chobo, dandori;
//! a tool that is not there is told with `SKIP:`), the two protos written for it pass buf's lint,
//! and a change to it is caught. The suite's references themselves (a rule's `import proto`, a
//! calendar's `use calendar`) are read from stage C.2 on; until then the example's one crossing
//! is the proto import.

mod common;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[test]
fn the_example_passes_check() {
    let o = common::sakai(&["check", "examples/通販/通販.ctx"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(String::from_utf8_lossy(&o.stdout), "examples/通販/通販.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 1 crossing checked (proto 1)\n");
    // The one crossing: the code ordering publishes takes inventory's types, as a conformist may.
    let ex = std::fs::canonicalize(common::EXAMPLE).unwrap();
    let out = sakai::check::check_map(&ex, "通販.ctx").unwrap();
    let c = out.checked.as_ref().unwrap();
    let crossings: Vec<(String, String, String, String)> = c
        .crossings
        .iter()
        .map(|x| (x.from.clone(), x.to.clone(), c.model.contexts[x.from_ctx].name.clone(), c.model.contexts[x.to_ctx].name.clone()))
        .collect();
    assert_eq!(crossings, [("proto/shop/ordering/v1/fulfillment.proto".into(), "proto/warehouse/v1/stock.proto".into(), "受注".into(), "在庫".into())]);
    assert_eq!(c.crossings[0].allowed, Some(sakai::refs::Allowed::Upstream(0)));
}

/// `<tool> check <file>` in the file's directory, with the file's name only, as DESIGN 4.1 says
/// the suite's tools are run.
fn suite_check(tool: &str, file: &str, envs: &[(&str, &str)]) -> common::Ran {
    let p = Path::new(common::EXAMPLE).join(file);
    let mut cmd = Command::new(tool);
    cmd.arg("check").arg(p.file_name().unwrap()).current_dir(p.parent().unwrap());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    common::run(&mut cmd, Duration::from_secs(120))
}

#[test]
fn what_was_copied_passes_the_suite() {
    let mut failures = Vec::new();
    let mut ran = 0;
    let rulec = common::suite("rulec");
    let jobs: [(&str, &[&str]); 4] = [
        ("rulec", &["billing/rules/決済手数料.rule", "billing/rules/出荷の送料.rule", "billing/rules/請求の要否.rule", "delivery/rules/出荷の急ぎ.rule"]),
        ("koyomi", &["calendars/東京の営業日.cal", "billing/支払条件.cal", "delivery/出荷日.cal"]),
        ("chobo", &["inventory/在庫の引当.book"]),
        ("dandori", &["ordering/受注.flow", "delivery/配送の手配.flow"]),
    ];
    for (name, files) in jobs {
        let Some(tool) = common::suite(name) else {
            common::skip(&format!("{name} is not there (SAKAI_{} or the PATH); the copies of its files are not checked", name.to_uppercase()));
            continue;
        };
        // dandori reads the rules a workflow uses with rulec.
        let envs: Vec<(&str, &str)> = match (&rulec, name) {
            (Some(r), "dandori") => vec![("DANDORI_RULEC", r.as_str())],
            _ => vec![],
        };
        if name == "dandori" && rulec.is_none() {
            common::skip("rulec is not there, which dandori reads the rules with; the workflows are not checked");
            continue;
        }
        for f in files {
            let r = suite_check(&tool, f, &envs);
            ran += 1;
            if !r.ok {
                failures.push(format!("{name} check {f}:\n{}{}", r.stdout, r.stderr));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    println!("checked {ran} files of the example with the suite's tools");
}

#[test]
fn buf_lints_the_protos_written_for_the_example() {
    let Some(buf) = common::program("SAKAI_BUF", "", "buf", &["--version"]) else {
        common::skip("buf is not there (SAKAI_BUF or the PATH)");
        return;
    };
    // The other two import files buf would fetch from the BSR (buf/validate) or read from
    // dandori (options.proto); these two stand alone.
    let dir = common::TempDir::new();
    for f in ["shop/ordering/v1/order.proto", "warehouse/v1/stock.proto"] {
        dir.write(f, &std::fs::read_to_string(Path::new(common::EXAMPLE).join("proto").join(f)).unwrap());
    }
    let r = common::run(Command::new(&buf).arg("lint").current_dir(dir.path()).env("BUF_CACHE_DIR", dir.path().join(".cache")), Duration::from_secs(120));
    assert!(r.ok, "{}{}", r.stdout, r.stderr);
}

fn changed(edits: &[(&str, &str, &str)]) -> Vec<sakai::check::Outcome> {
    let dir = common::TempDir::new();
    common::copy_dir(Path::new(common::EXAMPLE), dir.path());
    for (f, old, new) in edits {
        let p = dir.path().join(f);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(old), "{f} has no {old:?}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
    }
    sakai::check::check_args(dir.path(), &["通販.ctx".to_string()]).unwrap()
}

/// PLAN C.15: inventory adds a value to the packing status (E401), ordering's glossary gains a
/// 引当 of another meaning (E406), billing's side of the shared kernel is gone (E307; the
/// calendar read across the boundary, E202, is told once koyomi's references are read).
#[test]
fn a_change_to_the_example_is_caught() {
    let os = changed(&[("proto/warehouse/v1/stock.proto", "  PACKING_STATUS_SHORT = 3;\n", "  PACKING_STATUS_SHORT = 3;\n  PACKING_STATUS_DAMAGED = 4;\n")]);
    assert_eq!(common::codes(&os), ["E401"]);
    assert!(os[0].diags[0].message.en.contains("PACKING_STATUS_DAMAGED"));
    let os = changed(&[("contexts/受注.ctx", "\nupstream 在庫 conformist", "  引当 \"客の注文の一行に、届ける日を割り当てること\"\n\nupstream 在庫 conformist")]);
    assert_eq!(common::codes(&os), ["E406"]);
    let os = changed(&[(
        "contexts/請求.ctx",
        "\nshared kernel with 配送\n  koyomi \"../calendars/東京の営業日.cal\"\n  dir \"../py/calendars\", \"../ts/calendars\", \"../java/src/main/java/calendars\", \"../go/calendars\"\n",
        "\n",
    )]);
    assert_eq!(common::codes(&os), ["E307"]);
}
