//! The scope and who owns what (PLAN B.4).

mod common;

use common::{check_dir, codes, variant};

fn owner_of(os: &[sakai::check::Outcome], path: &str) -> String {
    let c = os[0].checked.as_ref().unwrap();
    let a = c.artifacts.iter().find(|a| a.path == path).unwrap_or_else(|| panic!("{path} is not an artifact"));
    c.model.contexts[a.ctx().unwrap()].name.clone()
}

#[test]
fn the_deepest_entry_wins() {
    // 受注 owns the whole repository; 在庫 and 請求 cut their directories out of it.
    let dir = variant("基本", &[("ctx/受注.ctx", "dir \"../proto/shop/ordering\", \"../proto/shop/common\", \"../py/ordering\"", "dir \"..\"")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), Vec::<&str>::new());
    assert_eq!(owner_of(&os, "proto/warehouse/v1/stock.proto"), "在庫");
    assert_eq!(owner_of(&os, "proto/shop/billing/v1/billing.proto"), "請求");
    assert_eq!(owner_of(&os, "proto/shop/ordering/v1/order.proto"), "受注");
    assert_eq!(owner_of(&os, "py/ordering/fulfill.py"), "受注");
    // A file named is deeper than any directory.
    let dir = variant("基本", &[("ctx/受注.ctx", "dir \"../proto/shop/ordering\", \"../proto/shop/common\", \"../py/ordering\"", "dir \"../proto/shop/ordering\", \"../proto/shop/common\", \"../py/ordering\"\n  file \"../py/inventory/service.py\"")]);
    let os = check_dir(dir.path());
    assert_eq!(owner_of(&os, "py/inventory/service.py"), "受注");
}

#[test]
fn what_the_scope_leaves_out() {
    let dir = variant(
        "基本",
        &[
            (".venv/lib/site.py", "", "x = 1\n"),
            ("py/node_modules/left.py", "", "x = 1\n"),
            ("py/ordering/__pycache__/fulfill.py", "", "x = 1\n"),
            ("target/out.proto", "", "syntax = \"proto3\";\n"),
            ("proto/google/protobuf/empty.proto", "", "syntax = \"proto3\";\npackage google.protobuf;\nmessage Empty {}\n"),
            ("proto/buf/validate/validate.proto", "", "syntax = \"proto3\";\npackage buf.validate;\n"),
            ("proto/dandori/v1/options.proto", "", "syntax = \"proto3\";\npackage dandori.v1;\n"),
            ("tools/gen.py", "", "x = 1\n"),
            ("基本.ctx", "covers \".\"\n", "covers \".\"\nexcept \"tools\"\n"),
            ("notes/readme.txt", "", "not an artifact\n"),
        ],
    );
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), Vec::<&str>::new());
    assert_eq!(os[0].checked.as_ref().unwrap().artifacts.len(), 9);
}

#[test]
fn files_no_context_owns_are_told_once_a_directory() {
    let mut edits: Vec<(String, String)> = (1..=12).map(|i| (format!("py/scripts/s{i:02}.py"), "x = 1\n".to_string())).collect();
    edits.push(("py/ordering/stray/one.proto".into(), "syntax = \"proto3\";\n".into()));
    let e: Vec<(&str, &str, &str)> = edits.iter().map(|(p, b)| (p.as_str(), "", b.as_str())).collect();
    let dir = variant("基本", &e);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E101"], "{}", common::text(&os, sakai::i18n::Lang::En));
    let d = &os[0].diags[0];
    assert_eq!(d.file, "py/scripts/");
    assert!(d.message.ja.contains("ファイル 12 件"), "{}", d.message.ja);
    // A directory with owned files beside the stray one is not taken for it: the stray file is in
    // 受注's directory, so it is owned and nothing is said.
    assert!(os[0].diags.iter().all(|d| !d.file.contains("stray")));
}

#[test]
fn two_contexts_writing_one_entry() {
    let dir = common::mutant("E102_同じ深さで二つが持つ");
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E102"]);
    assert_eq!(os[0].diags[0].refs.len(), 2);
}
