//! The properties of a module (DESIGN 38): what the check holds them to (E401, E605), what it says
//! of them on their vectors (E603, W603, W604, W605), and what is written for Lean of them without
//! running Lean (the toolchain, the golden of an example); `tests/lean.rs` runs it.
//!
//! The material is `tests/properties/`, each file in English and in Japanese. It is not in
//! `tests/modules/`, whose count the generated code's tests and the models' comparisons say.

mod common;

use kaname::check::Outcome;
use kaname::value::{Program, Value};
use ritsu_base::text::Lang;
use ritsu_units::Rat;
use std::path::PathBuf;
use std::rc::Rc;

/// A file of the material, in English and in Japanese.
fn both(name: &str) -> [String; 2] {
    [format!("tests/properties/{name}.kn"), format!("tests/properties/{name}.ja.kn")]
}

/// What `kaname check` says of it, in English.
fn said(o: &Outcome) -> String {
    common::shown(o, Lang::En)
}

/// What it says of it in `lang`, a diagnostic of each `code`, in order.
fn texts(o: &Outcome, code: &str, lang: Lang) -> Vec<String> {
    o.diags.iter().filter(|d| d.code == code).map(|d| d.render(lang)).collect()
}

/// A `when` line narrows what its property's body reads as the condition of an `if` does (DESIGN
/// 38.2): divided by a value the `when` line keeps from 0, the division is no place that can fail.
#[test]
fn when_narrows_the_places_in_the_body() {
    for f in both("narrows") {
        let o = common::check(&f);
        assert_eq!(common::codes(&o), ["W603"], "{f}:\n{}", said(&o));
        let c = o.checked.as_ref().expect("it passes");
        let zeros: Vec<&kaname::ranges::Place> = c.properties[0].facts.places.iter().filter(|p| matches!(p.what, kaname::ranges::What::Zero)).collect();
        assert_eq!(zeros.len(), 1, "{f}: the division is a place");
        assert!(zeros[0].value.iv().is_some_and(|i| !i.has_zero()), "{f}: its divisor is kept from 0: {:?}", zeros[0].value);
    }
}

/// A property is total (DESIGN 38.2): one that can fail where no `when` line narrows is E401, which
/// says so and what narrows it.
#[test]
fn a_property_that_can_fail_is_e401() {
    for f in both("can_fail") {
        let o = common::check(&f);
        assert_eq!(common::codes(&o), ["E401", "E303"], "{f}:\n{}", said(&o));
        assert!(o.checked.is_none());
    }
    let [en, ja] = both("can_fail").map(|f| common::check(&f));
    let e = &texts(&en, "E401", Lang::En)[0];
    assert!(e.contains("The property `no_part_is_more_than_the_whole` can fail with `zero`, and a property is total") && e.contains("narrow the values that reach those places with `when` lines"), "{e}");
    let j = &texts(&ja, "E401", Lang::Ja)[0];
    assert!(j.contains("性質 `一つ分は全体を超えない` は、`zero` で失敗することがあります。性質は total です") && j.contains("`when` の行か引数の範囲で狭めてください"), "{j}");
}

/// A parameter of a property is never of a function type (E605): a vector cannot make a function.
#[test]
fn a_function_parameter_is_e605() {
    let expected = [("tests/properties/function_parameter.kn", 4, 27, "The parameter `step` of the property `twice_is_no_less` is of a function type"), ("tests/properties/function_parameter.ja.kn", 4, 38, "性質 `二度でも下がらない` の引数 `一歩` は関数の型です")];
    for (f, line, col, message) in expected {
        let o = common::check(f);
        assert_eq!(common::codes(&o), ["E605"], "{f}:\n{}", said(&o));
        let d = &o.diags[0];
        assert_eq!((d.line, d.col), (Some(line), Some(col)));
        let lang = if f.ends_with(".ja.kn") { Lang::Ja } else { Lang::En };
        assert!(d.render(lang).contains(message), "{}", d.render(lang));
    }
}

/// The first vector that breaks a property is E603, with the values of its `let` lines (DESIGN
/// 38.3): a refund of 1 cent, next to the 0 its `when` line compares with, leaves a line nothing.
#[test]
fn a_false_property_is_found_on_the_vectors() {
    let [en, ja] = both("shares").map(|f| common::check(&f));
    assert_eq!(common::codes(&en), ["E603"], "{}", said(&en));
    assert_eq!(common::codes(&ja), ["E603"], "{}", said(&ja));
    assert!(en.checked.is_none() && ja.checked.is_none(), "an E603 stops the module");
    assert_eq!(
        texts(&en, "E603", Lang::En)[0].lines().next().unwrap(),
        "error[E603]: tests/properties/shares.kn:12:1: The property `every_line_gets_a_share` is false for lines: [Line { sku: \"a\", left: 500000USDc }, Line { sku: \"a\", left: 500000USDc }], refund: 1USDc"
    );
    assert!(texts(&en, "E603", Lang::En)[0].contains("= Line 15: `shares` is [0USDc, 1USDc]"), "{}", said(&en));
    let j = &texts(&ja, "E603", Lang::Ja)[0];
    assert!(j.contains("性質 `どの明細にも配る` は、明細: [明細 { 品番: \"a\", 残り: 500000USDc }, 明細 { 品番: \"a\", 残り: 500000USDc }], 返金: 1USDc で成り立ちません") && j.contains("15 行目: `配分` は [0USDc, 1USDc]"), "{j}");
}

/// The values a `when` line compares with become vectors, as an `if`'s do (DESIGN 9): the threshold
/// and the value next to it.
#[test]
fn the_when_comparisons_become_vectors() {
    for f in both("thresholds") {
        let o = common::check(&f);
        let c = o.checked.as_ref().unwrap_or_else(|| panic!("{f}:\n{}", said(&o)));
        let full = c.lowered.as_ref().expect("the module read with its properties");
        let p = Program::of(full);
        let (vs, _) = kaname::vectors::choose(&p, full, c.properties[0].lowered);
        let xs: Vec<&Value> = vs.iter().map(|v| &v.input[0]).collect();
        for n in [0, 37, 38, 100] {
            assert!(xs.contains(&&Value::Num(Rat::int(n))), "{f}: {n} is a vector: {xs:?}");
        }
    }
}

/// A property whose `when` lines hold on none of its vectors is W605, beside its W603 (DESIGN 38.3).
#[test]
fn a_when_that_never_holds_is_w605() {
    let [en, ja] = both("thresholds").map(|f| common::check(&f));
    assert_eq!(common::codes(&en), ["W603", "W603", "W605"], "{}", said(&en));
    assert_eq!(common::codes(&ja), ["W603", "W603", "W605"], "{}", said(&ja));
    let e = &texts(&en, "W605", Lang::En)[0];
    assert!(e.contains("thresholds.kn:9:1: The `when` lines of the property `past_the_end` hold on none of its 3 vectors") && e.contains("can be proved whatever it says"), "{e}");
    let j = &texts(&ja, "W605", Lang::Ja)[0];
    assert!(j.contains("性質 `範囲の外` の `when` の行は、3 個のベクタのどれでも成り立ちません"), "{j}");
    // the W603 beside it says no vector held, not that all of none did
    assert!(texts(&en, "W603", Lang::En)[1].contains("The property `past_the_end` is not shown: there is no proof of it"), "{}", said(&en));
}

/// The vectors of a property that calls a rule run the rule itself (DESIGN 38.3), as the
/// `examples` lines do: the card fee of the example payout is more than a payment of 1 cent.
#[test]
fn a_property_on_a_rule_is_checked_with_the_rule() {
    let [en, ja] = both("fee").map(|f| common::check(&f));
    assert_eq!(common::codes(&en), ["E603", "W603"], "{}", said(&en));
    assert_eq!(common::codes(&ja), ["E603", "W603"], "{}", said(&ja));
    assert!(texts(&en, "E603", Lang::En)[0].contains("fee.kn:6:1: The property `the_fee_is_no_more_than_the_payment` is false for amount: 1USDc, international: true"), "{}", said(&en));
    assert!(texts(&ja, "E603", Lang::Ja)[0].contains("性質 `手数料は支払いを超えない` は、金額: 1USDc, 海外: true で成り立ちません"), "{}", said(&ja));
}

/// W603 says why a property no vector breaks is not shown (DESIGN 38.3): no proof is beside the
/// module; the proof file beside it writes a theorem of the property, and `--lean` did not run; the
/// proof file writes none (read as text, before `--lean`); or it rests on the answers of a rule,
/// which a proof knows only as far as the port says.
#[test]
fn w603_says_why() {
    let o = common::check("tests/properties/narrows.kn");
    let w = &texts(&o, "W603", Lang::En)[0];
    assert!(w.contains("The property `no_part_is_more_than_the_whole` held on all 4 of its 6 vectors on which its `when` lines are true, but it is not shown: there is no proof of it (narrows.lean holds no theorem of it)"), "{w}");
    assert!(w.contains("`ritsu gen --target lean` writes `samples/narrows.lean`"), "{w}");
    let o = common::check("tests/properties/narrows.ja.kn");
    let w = &texts(&o, "W603", Lang::Ja)[0];
    assert!(w.contains("性質 `一つ分は全体を超えない` は、`when` の行が成り立つ 4 個のベクタ（6 個のうち）のすべてで成り立ちましたが、示されていません。証明がありません（narrows_ja.lean に、この性質の定理がありません）"), "{w}");
    // the proof beside it, which only `--lean` checks
    let o = common::check("tests/properties/in_place.kn");
    assert_eq!(common::codes(&o), ["W603"], "{}", said(&o));
    let w = &texts(&o, "W603", Lang::En)[0];
    assert!(w.contains("The property `twice_is_even` held on all 2 vectors, but it is not shown: the proof in in_place.lean is checked with `--lean`"), "{w}");
    // a proof file that writes no theorem of it, which needs no Lean to see
    let t = ritsu_testkit::TempDir::new("kaname-w603-no-theorem");
    let kn = t.write("in_place.kn", std::fs::read_to_string("tests/properties/in_place.kn").unwrap());
    t.write("in_place.lean", "import Kaname.in_place\n\nnamespace Proofs.in_place\n\n-- theorem twice_is_even : Kaname.in_place.twice_is_even := by omega\ntheorem twice_is_even_too : True := trivial\n\nend Proofs.in_place\n");
    let o = common::check(kn.to_str().unwrap());
    let w = &texts(&o, "W603", Lang::En)[0];
    assert!(w.contains("The property `twice_is_even` held on all 2 vectors, but it is not shown: in_place.lean holds no theorem `Proofs.in_place.twice_is_even`"), "{w}");
    assert!(w.contains("Write `theorem twice_is_even : Kaname.in_place.twice_is_even` in the namespace `Proofs.in_place` of in_place.lean"), "{w}");
    // the answers of a rule
    let [en, ja] = both("fee").map(|f| common::check(&f));
    let w = &texts(&en, "W603", Lang::En)[0];
    assert!(w.contains("The property `the_fee_has_its_fixed_part` held on all 4 vectors, but it is not shown: it rests on the answers of the rule `card_fee`, and a proof may use only what the port says of them"), "{w}");
    let w = &texts(&ja, "W603", Lang::Ja)[0];
    assert!(w.contains("性質 `手数料には決まった分がある` は、4 個のベクタのすべてで成り立ちましたが、示されていません。規則 `カード手数料` の答えに頼っていて、証明が使えるのは、口がその答えについて教えることだけです"), "{w}");
}

/// A property that uses what is not written for Lean yet is W604, which names it and its line
/// (DESIGN 38.4): a dict, a set, a builtin of strings, one that uses the order of keys, and a type
/// parameter held to keys, in a function the property calls. Its vectors run all the same.
#[test]
fn w604_says_what_cannot_be_written() {
    let expected = [
        (11, "The property `a_key_for_each_entry` held on all 8 vectors, but it cannot be written for Lean: it uses a dict (line 11), which is not written for Lean yet"),
        (15, "The property `no_more_members_than_names` held on all 4 vectors, but it cannot be written for Lean: it uses a set (line 17), which is not written for Lean yet"),
        (19, "The property `trimming_twice_is_trimming_once` held on all 16 vectors, but it cannot be written for Lean: it uses `trim`, a builtin of strings (line 21), which is not written for Lean yet"),
        (23, "The property `sorting_keeps_the_tasks` held on all 6 vectors, but it cannot be written for Lean: it uses `sort_by`, a builtin that uses the order of keys (line 25), which is not written for Lean yet"),
        (27, "The property `the_largest_is_one_of_them` held on all 7 vectors, but it cannot be written for Lean: it uses the type parameter held to keys `K: key` (line 8), which is not written for Lean yet"),
    ];
    let [en, ja] = both("not_in_lean").map(|f| common::check(&f));
    assert_eq!(common::codes(&en), ["W604"; 5], "{}", said(&en));
    assert_eq!(common::codes(&ja), ["W604"; 5], "{}", said(&ja));
    for (d, (line, message)) in en.diags.iter().zip(expected) {
        assert_eq!(d.line, Some(line));
        assert!(d.render(Lang::En).contains(message), "{}", d.render(Lang::En));
    }
    let j = ja.diags[2].render(Lang::Ja);
    assert!(j.contains("性質 `二度刈り込んでも同じ` は、16 個のベクタのすべてで成り立ちましたが、Lean に書けません。21 行目の文字列の組み込みの関数 `trim` は、まだ Lean に書き出しません"), "{j}");
    let j = ja.diags[0].render(Lang::Ja);
    assert!(j.contains("Lean に書けません。11 行目の辞書は、まだ Lean に書き出しません"), "{j}");
}

/// A proof file is read as text before anything is built (DESIGN 38.5): each form a proof file may
/// not hold is found, with its kind and the words written; a name escaped with `«»` is read as the
/// name it is; nothing in a comment, a string or a character is read; and the forms the proofs use
/// pass.
#[test]
fn the_scan_finds_each_form() {
    use kaname::lean::{BreachKind, scan};
    let found: [(&str, BreachKind, &str); 55] = [
        // imports of anything but the modules written from the `.kn` files
        ("import Lean", BreachKind::Import, "import Lean"),
        ("import Std.Data.HashMap", BreachKind::Import, "import Std.Data.HashMap"),
        ("public meta import Lean.Elab", BreachKind::Import, "import Lean.Elab"),
        ("import all Init.Prelude", BreachKind::Import, "import Init.Prelude"),
        ("import KanameX", BreachKind::Import, "import KanameX"),
        // `#` commands that run code
        ("#eval IO.FS.writeFile \"ran.txt\" \"ran\"", BreachKind::Command, "#eval"),
        ("#eval! (1 : Nat)", BreachKind::Command, "#eval!"),
        ("#guard 1 + 1 = 2", BreachKind::Command, "#guard"),
        ("#exit", BreachKind::Command, "#exit"),
        // commands, tactics and terms that run code
        ("run_cmd pure ()", BreachKind::Command, "run_cmd"),
        ("run_elab pure ()", BreachKind::Command, "run_elab"),
        ("run_meta pure ()", BreachKind::Command, "run_meta"),
        ("example : True := by run_tac pure ()", BreachKind::Command, "run_tac"),
        ("example : True := by_elab pure ()", BreachKind::Command, "by_elab"),
        ("initialize pure ()", BreachKind::Command, "initialize"),
        ("builtin_initialize pure ()", BreachKind::Command, "builtin_initialize"),
        ("def secret : String := include_str \"/etc/hosts\"", BreachKind::Command, "include_str"),
        ("unsafe def peek : Nat := 0", BreachKind::Command, "unsafe"),
        // commands that add syntax, which can make commands that are not in the text
        ("syntax \"go\" : tactic", BreachKind::Command, "syntax"),
        ("macro \"go\" : tactic => `(tactic| sorry)", BreachKind::Command, "macro"),
        ("macro_rules | `(tactic| decide) => `(tactic| sorry)", BreachKind::Command, "macro_rules"),
        ("elab \"go\" : tactic => pure ()", BreachKind::Command, "elab"),
        ("elab_rules : tactic | `(tactic| go) => pure ()", BreachKind::Command, "elab_rules"),
        ("declare_syntax_cat go", BreachKind::Command, "declare_syntax_cat"),
        ("local notation \"T\" => True", BreachKind::Command, "notation"),
        ("infix:65 \" +' \" => Nat.add", BreachKind::Command, "infix"),
        ("infixl:65 \" +' \" => Nat.add", BreachKind::Command, "infixl"),
        ("infixr:65 \" +' \" => Nat.add", BreachKind::Command, "infixr"),
        ("prefix:max \"√\" => Nat.succ", BreachKind::Command, "prefix"),
        ("postfix:max \"⁺\" => Nat.succ", BreachKind::Command, "postfix"),
        ("binder_predicate x \" > \" y:term => `($x > $y)", BreachKind::Command, "binder_predicate"),
        ("simproc reduceGo (_) := fun _ => pure .continue", BreachKind::Command, "simproc"),
        ("cbv_simproc reduceGo (_) := fun _ => pure .continue", BreachKind::Command, "cbv_simproc"),
        ("grind_propagator ↓Nat.succ (e) := do pure ()", BreachKind::Command, "grind_propagator"),
        ("register_error_explanation lean.x {}", BreachKind::Command, "register_error_explanation"),
        ("register_simp_attr go_simp", BreachKind::Command, "register_simp_attr"),
        // what has compiled code run in the build
        ("example : 1 = 1 := by native_decide", BreachKind::Native, "native_decide"),
        ("example : 1 = 1 := by «native_decide»", BreachKind::Native, "native_decide"),
        ("example : 1 = 1 := by decide +native", BreachKind::Native, "native"),
        ("example : 1 = 1 := by decide (config := { native := true })", BreachKind::Native, "decide (config := …)"),
        ("example : 1 = 1 := by decide (config := {})", BreachKind::Native, "decide (config := …)"),
        ("theorem t : True := Lean.ofReduceBool _ _ rfl", BreachKind::Native, "Lean.ofReduceBool"),
        ("open Lean in example := reduceBool true", BreachKind::Native, "reduceBool"),
        ("example := Lean.reduceNat 1", BreachKind::Native, "Lean.reduceNat"),
        ("example := Lean.ofReduceNat", BreachKind::Native, "Lean.ofReduceNat"),
        ("example := Lean.trustCompiler", BreachKind::Native, "Lean.trustCompiler"),
        // attributes that install code
        ("@[implemented_by fake] def real : Bool := false", BreachKind::Attribute, "@[implemented_by]"),
        ("@[macro Lean.Parser.Tactic.decide] def go : Lean.Macro := fun _ => `(tactic| sorry)", BreachKind::Attribute, "@[macro]"),
        ("@[simp, extern \"c_go\"] def go : Nat := 0", BreachKind::Attribute, "@[extern]"),
        ("attribute [local tactic Lean.Parser.Tactic.decide] go", BreachKind::Attribute, "attribute [tactic]"),
        // options other than those of time, depth, printing and lints
        ("set_option debug.skipKernelTC true", BreachKind::Option, "set_option debug.skipKernelTC"),
        ("set_option trace.profiler.output \"out.json\" in", BreachKind::Option, "set_option trace.profiler.output"),
        ("set_option doc.verso true", BreachKind::Option, "set_option doc.verso"),
        // a string with `{`, whose braces in an interpolated string hold code
        ("example : String := s!\"{(1 : Nat)}\"", BreachKind::Interpolation, "{"),
        ("example : String := r#\"a { b\"#", BreachKind::Interpolation, "{"),
    ];
    for (text, kind, what) in found {
        let got = scan(text);
        let first = got.first().unwrap_or_else(|| panic!("nothing found in {text:?}"));
        assert_eq!((first.kind, first.what.as_str()), (kind, what), "{text:?}: {got:?}");
    }
    // where each is, in the order written: the column of the attribute is that of its name
    let text = "import Kaname.in_place\n-- #eval 1\nimport Lean\n\ntheorem t : True := by\n  native_decide\n@[simp, implemented_by f] def g := 1\n";
    let got: Vec<(usize, usize, BreachKind)> = scan(text).into_iter().map(|b| (b.line, b.col, b.kind)).collect();
    assert_eq!(got, [(3, 1, BreachKind::Import), (6, 3, BreachKind::Native), (7, 9, BreachKind::Attribute)]);
    // what a proof file holds: comments, strings and characters read past, and the forms it uses
    let pass = [
        "import Kaname.refund_split\nimport all Kaname.Prelude",
        "-- import Lean\n/- #eval 1 /- nested -/ run_cmd -/\n/-- `native_decide` -/ theorem t : True := trivial",
        "example : String := \"import Lean #eval native_decide set_option debug.skipKernelTC\"",
        "example : String := \"a \\\" macro\"\nexample : String := r#\"run_cmd \" #eval\"#",
        "example : Char := '\"'\nexample : Char := '\\''\nexample : Char := '\\x41'\nexample : String := \"#exit\"",
        "theorem t : 1 = 1 := by decide\ntheorem u : 1 = 1 := by decide +kernel",
        "@[simp] theorem t : True := trivial\n@[local simp, -grind] theorem u : True := trivial\nattribute [-simp] t\nattribute [local instance] i\n@[reducible, scoped instance 100] def k := 1",
        "set_option maxHeartbeats 400000 in\nset_option maxRecDepth 2000\nset_option pp.all true\nset_option linter.unusedVariables false\nset_option trace.Meta.Tactic.simp true\nset_option synthInstance.maxHeartbeats 0",
        "#check Nat\n#print axioms t\n#reduce 1 + 1\n#guard_msgs in #check Nat",
        "theorem native' : True := trivial\ndef nativeLike := 1\ntheorem t (macroX : Nat) : macroX = macroX := rfl",
        "theorem «a theorem» : True := trivial\nexample := x.1.2",
    ];
    for text in pass {
        assert_eq!(scan(text), [], "{text:?}");
    }
}

/// The toolchain of the Lean kaname writes is the one `proofs/` builds with (DESIGN 38.4).
#[test]
fn the_lean_toolchain_is_the_one_proofs_use() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../proofs/lean-toolchain");
    let proofs = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(proofs.trim(), kaname::r#gen::lean::TOOLCHAIN);
}

/// The Lean module of the example `refund_split` (DESIGN 38.4), English and Japanese, as its golden
/// file holds it (`KANAME_BLESS=1` writes them again), kaname's version as `<version>`.
#[test]
fn the_lean_module_of_an_example_is_its_golden() {
    for (kn, golden) in [("examples/refund_split/refund_split.kn", "refund_split.lean"), ("examples/refund_split/refund_split.ja.kn", "refund_split_ja.lean")] {
        let o = common::check(kn);
        let c: Rc<kaname::check::Checked> = o.checked.clone().unwrap_or_else(|| panic!("{kn}:\n{}", said(&o)));
        let (files, _) = kaname::r#gen::lean::package(&[c], &kaname::r#gen::lean::Layout::Temp);
        let text = &files.iter().find(|(p, _)| p.to_string_lossy() == format!("Kaname/{golden}")).unwrap_or_else(|| panic!("no Kaname/{golden}")).1;
        ritsu_testkit::golden(format!("tests/golden/lean/{golden}"), &text.replace(env!("CARGO_PKG_VERSION"), "<version>"));
    }
}
