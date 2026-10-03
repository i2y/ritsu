//! A number's unit is ritsu's one type of units (ritsu-units; ritsu's DESIGN 5.3): two spellings of
//! one unit are the same unit, two units of a dimension are not, and a bound of a range may carry a
//! unit, counted in the unit of its type as rulec counts one. The rules are read through rulec's own
//! answer to ritsu's port of rules.

use dandori::check::Checked;
use dandori::model::{Range, Ty};
use std::path::PathBuf;
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A flow checked as if it were the file `tests/fixtures/<name>`, with rulec's port.
fn check(name: &str, src: &str) -> Checked {
    dandori::sources::with_rules(Rc::new(rulec::ports::Engine::new()), || dandori::check::check_source(src, &root().join("tests/fixtures").join(name)))
}

fn said(c: &Checked) -> Vec<(String, String, Vec<String>)> {
    c.diags.iter().map(|d| (d.code.to_string(), d.en.clone(), d.notes.iter().map(|n| n.en.clone()).collect())).collect()
}

/// The workflow of ritsu's DESIGN 1.4: the hotel's rule answers `money[JPY, incl_tax]`, and the task
/// takes `money[円, incl_tax]`. `JPY` is another spelling of `円`, so the amount goes as it is.
#[test]
fn an_amount_in_jpy_goes_where_yen_is_taken() {
    let c = check(
        "hold.flow",
        "workflow hold v1\n\nuse rule hold from \"../../examples/hotel/rules/hold_amount.rule\"\n\ninputs\n  room   : hold.room\n  nights : int  range >=1 <=30\n\ntask authorize(amount: money[円, incl_tax])\n\nflow\n  let quote = hold(room: room, nights: nights)\n  authorize(amount: quote.amount)\n",
    );
    assert!(c.model.is_some() && c.diags.is_empty(), "{:?}", said(&c));
    // and the other way round: a task's yen into an input of JPY
    let c = check("pay.flow", "workflow pay v1\n\ninputs\n  amount : money[円, incl_tax]\n\ntask charge(amount: money[JPY, incl_tax])\n\nflow\n  charge(amount: amount)\n");
    assert!(c.model.is_some() && c.diags.is_empty(), "{:?}", said(&c));
}

/// Two units of one dimension are two units: dandori computes nothing (P1), so it does not convert
/// a value where it goes, and a rule says so if one is to be converted.
#[test]
fn a_mass_in_kg_does_not_go_where_g_is_taken() {
    let c = check("weigh.flow", "workflow weigh v1\n\ninputs\n  w : mass[kg]\n\ntask ship(weight: mass[g])\n\nflow\n  ship(weight: w)\n");
    assert_eq!(
        said(&c).iter().map(|(code, en, _)| (code.as_str(), en.as_str())).collect::<Vec<_>>(),
        [("E003", "expected `mass[g]` here, but this is `mass[kg]`")]
    );
    // nor tax in where it is left out
    let c = check("tax.flow", "workflow tax v1\n\ninputs\n  a : money[円, incl_tax]\n\ntask book(amount: money[円, excl_tax])\n\nflow\n  book(amount: a)\n");
    assert_eq!(said(&c).iter().map(|(code, _, _)| code.as_str()).collect::<Vec<_>>(), ["E003"]);
}

/// A bound written with a unit is counted in the unit of the type: what goes on the wire.
#[test]
fn a_bound_with_a_unit_is_counted_in_the_type() {
    let c = check(
        "bounds.flow",
        "workflow bounds v1\n\nrecord 請求\n  額 : money[円, incl_tax]  range >=0円 <=100万円\n  重さ : mass[g]  range >=1kg <=40kg\n  割引 : rate[step 0.1%]  range <=50%\n  税抜 : money[JPY, excl_tax]  range >=100銭\n  温度 : temperature[℃]  range >=41℉\n\nflow\n  pass\n",
    );
    let m = c.model.as_ref().unwrap_or_else(|| panic!("{:?}", said(&c)));
    let rec = m.records.iter().find(|r| r.name == "請求").unwrap();
    let r = |lo: Option<i64>, hi: Option<i64>| Range { lo, hi };
    assert_eq!(rec.ranges["額"], r(Some(0), Some(1_000_000)));
    assert_eq!(rec.ranges["重さ"], r(Some(1_000), Some(40_000)));
    assert_eq!(rec.ranges["割引"], r(None, Some(500)));
    assert_eq!(rec.ranges["税抜"], r(Some(1), None));
    assert_eq!(rec.ranges["温度"], r(Some(5), None));
    // a sign of temperature is a unit, never part of a name the code dandori writes could not spell
    for src in ["workflow w v1\n\ninputs\n  温度℃ : int\n\nflow\n  pass\n", "workflow w v1\n\ninputs\n  t : int  range >=41 <=50 ℉\n\nflow\n  pass\n"] {
        let c = check("bad.flow", src);
        assert_eq!(said(&c).iter().map(|(code, _, _)| code.as_str()).collect::<Vec<_>>(), ["E001"], "{src}");
    }
    // the types keep their spelling
    assert_eq!(m.ty_name(&rec.fields.iter().find(|(n, _)| n == "割引").unwrap().1), "rate[step 0.1%]");
    assert!(matches!(&rec.fields[0].1, Ty::Num(u) if u.to_string() == "money[円, incl_tax]"));
}

/// A spelling the table of units does not have is no type, and says why.
#[test]
fn a_unit_the_table_does_not_have_is_no_type() {
    for (ty, note) in [("mass[foo]", "`foo` is not a unit"), ("money[円, foo]", "`foo` is neither incl_tax nor excl_tax"), ("length[kg]", "`kg` is not a unit of length")] {
        let c = check("bad.flow", &format!("workflow bad v1\n\ninputs\n  x : {ty}\n\nflow\n  pass\n"));
        assert_eq!(said(&c), [("E002".to_string(), format!("there is no type `{ty}`"), vec![note.to_string()])], "{ty}");
    }
    // a word that is no dimension says what a type is, as before
    let c = check("bad.flow", "workflow bad v1\n\ninputs\n  x : weight[kg]\n\nflow\n  pass\n");
    assert_eq!(said(&c)[0].0, "E002");
    assert!(said(&c)[0].2[0].starts_with("a type is int, string, bool"), "{:?}", said(&c));
}
