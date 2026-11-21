//! A value written as the bare name of an enum's value where nothing says its type (DESIGN 29): the
//! list a `for` goes through, a list or a value put in a variable with no type written. An item of
//! the list that is no bare word says the list's type; else the name is read as a value of the one
//! enum the flow sees that has it, all the names of a list together; when no enum has it, or two
//! enums or more do, E002 says which, and how to write the type. A variable of the same name comes
//! first, as everywhere, and `{…}` in a string reads variables alone.

use dandori::check::Checked;
use dandori::diag::Lang;
use dandori::model::Ty;
use std::path::PathBuf;

fn check(src: &str) -> Checked {
    dandori::check::check_source(src, &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/values.flow"))
}

/// What check says, each finding with its notes, in `lang`.
fn said(c: &Checked, lang: Lang) -> Vec<(&'static str, usize, usize, String, Vec<String>)> {
    let pick = |t: &dandori::diag::Text| if lang == Lang::Ja { t.ja.clone() } else { t.en.clone() };
    c.diags.iter().map(|d| (d.code, d.line, d.col, if lang == Lang::Ja { d.ja.clone() } else { d.en.clone() }, d.notes.iter().map(pick).collect())).collect()
}

/// A flow with two enums, `role` and `desk`, that share the value `manager`, and the statements `body`.
fn flow(body: &str) -> String {
    format!("workflow asks v1\ndescription \"Asks the roles\"\n\nenum role = manager | accounting\nenum desk = manager | front\n\ninputs\n  id : string\n\noutputs\n  id : string\n\ntask ask(id: string, role: role)\ntask note(text: string)\n\nflow\n{body}  succeed id = id\n")
}

/// The form of the report: a `for` over a list of values of `role`, and a value put in a variable
/// with no type, each passes where one enum has the words.
#[test]
fn the_values_of_the_one_enum_that_has_them_are_read() {
    let c = check(&flow("  for r in [manager, accounting] at most 2\n    ask(id: id, role: r)\n  let x = accounting\n  ask(id: id, role: x)\n").replace("enum desk = manager | front\n", ""));
    assert!(c.model.is_some(), "{:?}", said(&c, Lang::En));
    // the report's own flow: the list goes to `ask`, whose `role` takes a value of `role`
    let report = "workflow lists_for v1\ndescription \"Ask the manager and accounting, one after the other\"\n\nenum role = manager | accounting\n\ninputs\n  id : string\n\noutputs\n  id : string\n\ntask ask(id: string, role: role)\n\nflow\n  for r in [manager, accounting] at most 2\n    ask(id: id, role: r)\n  succeed id = id\n";
    let c = check(report);
    assert!(c.model.is_some(), "{:?}", said(&c, Lang::En));
    // a value of `desk` alone, in a list put in a variable
    let c = check(&flow("  let desks = [front, manager]\n  for d in desks at most 2\n    note(text: id)\n"));
    assert!(c.model.is_some(), "{:?}", said(&c, Lang::En));
    let m = c.model.unwrap();
    let desk = m.enums.iter().position(|e| e.name == "desk").unwrap();
    assert!(m.all_stmts().iter().any(|s| matches!(&s.kind, dandori::model::TK::Assign { name, expr } if name == "desks" && expr.ty() == Ty::List(Box::new(Ty::Enum(desk))))), "the list is not a list of `desk`");
    // an item that is no bare word says the type of the list, wherever it is: `manager` of both
    // enums, after it a value of `role`
    let with_r = |body: &str| flow(body).replace("  id : string\n\noutputs", "  id : string\n  r  : role\n\noutputs");
    let c = check(&with_r("  let xs = [manager, r]\n  for v in xs at most 2\n    ask(id: id, role: v)\n"));
    assert!(c.model.is_some(), "{:?}", said(&c, Lang::En));
    let c = check(&with_r("  let ys = [front, r]\n"));
    assert_eq!(said(&c, Lang::En), [("E002", 18, 13, "there is no variable or value `front`".to_string(), vec!["the values of `role` are manager, accounting".to_string()])]);
}

/// A name no enum has: E002 at that name, with the values of the enums the list's other names are
/// values of. Names each of some enum, of no one enum together: E002 at the list, with what each
/// enum has and lacks.
#[test]
fn a_name_no_enum_has_is_said_where_it_is_written() {
    let c = check(&flow("  for r in [manager, acounting] at most 2\n    note(text: id)\n"));
    assert_eq!(
        said(&c, Lang::En),
        [("E002", 17, 22, "there is no variable or value `acounting`".to_string(), vec!["no enum this flow sees has the value `acounting`; the values of `role` are manager, accounting; the values of `desk` are manager, front".to_string()])]
    );
    assert_eq!(said(&c, Lang::Ja)[0].3, "変数や値 `acounting` はありません");
    assert_eq!(said(&c, Lang::Ja)[0].4, ["このフローから見える列挙に、値 `acounting` はありません。`role` の値は manager・accounting です。`desk` の値は manager・front です"]);
    let c = check(&flow("  let x = managr\n"));
    assert_eq!(
        said(&c, Lang::En),
        [("E002", 17, 11, "there is no variable or value `managr`".to_string(), vec!["a value is a variable, a field of one, a string, a number, true, false or a value of an enum, and no enum this flow sees has the value `managr`".to_string()])]
    );
    assert_eq!(said(&c, Lang::Ja)[0].4, ["値は、変数・そのフィールド・文字列・数・true・false・列挙の値のどれかです。このフローから見える列挙に、値 `managr` はありません"]);
    let c = check(&flow("  let xs = [accounting, front]\n"));
    assert_eq!(
        said(&c, Lang::En),
        [(
            "E002",
            17,
            13,
            "no enum this flow sees has all of `accounting`, `front` as values".to_string(),
            vec!["the values of a list are of one type; `role` has `accounting` but not `front`; `desk` has `front` but not `accounting`".to_string()]
        )]
    );
    assert_eq!(said(&c, Lang::Ja)[0].3, "このフローから見える列挙に、`accounting`・`front` のすべてを値に持つものはありません");
    assert_eq!(said(&c, Lang::Ja)[0].4, ["並びの値は、一つの型の値です。`role` には `accounting` がありますが、`front` はありません。`desk` には `front` がありますが、`accounting` はありません"]);
}

/// A name two enums have: E002 says which enums, and how to write the type, of the list or of
/// where the value goes.
#[test]
fn a_name_two_enums_have_says_which_and_how_to_write_the_type() {
    let c = check(&flow("  for r in [manager] at most 1\n    note(text: id)\n"));
    assert_eq!(
        said(&c, Lang::En),
        [("E002", 17, 13, "which enum `manager` is a value of is not known here".to_string(), vec!["`manager` is a value of `role` and of `desk` alike; write the type of the list (`let xs : list[role] = [manager]`)".to_string()])]
    );
    assert_eq!(said(&c, Lang::Ja)[0].3, "`manager` がどの列挙の値か、ここでは決まりません");
    assert_eq!(said(&c, Lang::Ja)[0].4, ["`manager` は `role` と `desk` のどちらの値でもあります。並びに型を書いてください（`let xs : list[role] = [manager]`）"]);
    let c = check(&flow("  let x = manager\n"));
    assert_eq!(said(&c, Lang::En)[0].4, ["`manager` is a value of `role` and of `desk` alike; write the type where the value goes (`let x : role = manager`)"]);
    assert_eq!(said(&c, Lang::Ja)[0].4, ["`manager` は `role` と `desk` のどちらの値でもあります。値を入れるところに型を書いてください（`let x : role = manager`）"]);
    // the type written, the value is read as before
    let c = check(&flow("  let xs : list[role] = [manager]\n  for r in xs at most 1\n    ask(id: id, role: r)\n"));
    assert!(c.model.is_some(), "{:?}", said(&c, Lang::En));
}

/// A variable of the same name comes first, as everywhere: a list that starts with it is a list of
/// its type. `{…}` in a string puts a variable in, and a word in it is no value of an enum.
#[test]
fn a_variable_comes_first_and_a_string_reads_variables_alone() {
    let c = check(&flow("  let manager = \"boss\"\n  for r in [manager] at most 1\n    note(text: r)\n"));
    assert!(c.model.is_some(), "{:?}", said(&c, Lang::En));
    let c = check(&flow("  note(text: \"{accounting}\")\n").replace("enum desk = manager | front\n", ""));
    assert_eq!(
        said(&c, Lang::En),
        [("E002", 16, 14, "there is no variable `accounting`".to_string(), vec!["`{…}` in a string puts in a variable or a field of one; a word that is always the same is written in the string as it is".to_string()])]
    );
    assert_eq!(said(&c, Lang::Ja)[0].4, ["文字列の `{…}` に入れられるのは、変数かそのフィールドです。決まった語は、文字列にそのまま書いてください"]);
}
