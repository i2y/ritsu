//! What a form works out, decided from outside the ticket machine (DESIGN 13.7, 4.9): the
//! reference's answers of the entry `worked`, case by case. A condition that holds, one that does
//! not and one that is not decided: a field it reads that the requester has to fill in left empty, a
//! value it reads outside its field's range or none of its choices, rows past the most their field
//! takes. A field hidden is read as empty, by a condition and through a field shown on a condition,
//! and so is text of spaces alone. A total is worked out from the rows there so far, fewer than the
//! field asks for too; each row's own computation and the day the form counts are worked out as the
//! machine works them out. What an approver gives is read in place of the form's values, of the
//! fields they give alone. What is no call of the contract is refused, and a kind whose form works
//! nothing out answers no field. The vectors `--target requests` writes of the entry hold each of
//! these cases.

mod common;

use ringi::check::Run;
use ritsu_base::json::{self, Json};
use std::path::PathBuf;

/// What the reference answers the entry `worked` of the request at `path` for each of `inputs`: the
/// answer as compact JSON, or `error: ` and why it is no call of the contract.
fn answers(path: &str, inputs: &[String]) -> Vec<String> {
    let run = Run::new(common::joined());
    let o = run.file(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    ringi::engine::with(&o, &run, |m, _| {
        inputs
            .iter()
            .map(|i| {
                let input = json::parse(i).unwrap_or_else(|e| panic!("{i}: {e:?}"));
                match ringi::engine::worked::answer(m, &input) {
                    Ok(a) => a.compact(),
                    Err(t) => format!("error: {}", t.en),
                }
            })
            .collect()
    })
    .unwrap_or_else(|t| panic!("{path}: {}", t.en))
}

fn strings(inputs: &[&str]) -> Vec<String> {
    inputs.iter().map(|s| s.to_string()).collect()
}

/// Each case of `cases` (an input and the answer) answered so, each said by its input.
fn assert_answers(path: &str, cases: &[(String, &str)]) {
    let inputs: Vec<String> = cases.iter().map(|(i, _)| i.clone()).collect();
    let got = answers(path, &inputs);
    let wrong: Vec<String> = cases.iter().zip(&got).filter(|((_, want), got)| got != want).map(|((i, want), got)| format!("{i}\n  expected: {want}\n  answered: {got}")).collect();
    assert!(wrong.is_empty(), "{path}:\n{}", wrong.join("\n"));
}

fn case(input: &str, answer: &'static str) -> (String, &'static str) {
    (input.to_string(), answer)
}

/// A condition holds, does not, or is not decided: while a field it reads that the requester has to
/// fill in is empty, a value it reads is none of its field's choices or outside its range, or the
/// rows it reads are more than their field takes (as the screens' `outputOf` gives no answer).
#[test]
fn a_condition_holds_does_not_or_is_not_decided() {
    let pens: Vec<String> = (0..21).map(|_| r#"{"what":"Pen","price":100}"#.to_string()).collect();
    assert_answers(
        "examples/expense.ringi",
        &[
            // a trip: the destination is shown, and a receipt is not asked for at 1,200 yen
            case(
                r#"{"form":{"category":"travel","items":[{"what":"Train","price":1200}],"purpose":"A visit","used_on":"2026-11-16","trip_to":"Osaka"},"edited":null}"#,
                r#"{"fields":{"amount":{"value":1200},"trip_to":{"shown":true},"receipt":{"required":false}}}"#,
            ),
            // 35,000 yen of expenses: no destination, and a receipt asked for
            case(
                r#"{"form":{"category":"expense","items":[{"what":"Paper","price":20000},{"what":"Ink","price":15000}]}}"#,
                r#"{"fields":{"amount":{"value":35000},"trip_to":{"shown":false},"receipt":{"required":true}}}"#,
            ),
            // no category, and one none of its choices: whether the destination is shown is not decided
            case(r#"{"form":{"items":[{"what":"Paper","price":100}]}}"#, r#"{"fields":{"amount":{"value":100},"trip_to":{"shown":null},"receipt":{"required":false}}}"#),
            case(r#"{"form":{"category":"none_of_them","items":[{"price":100}]}}"#, r#"{"fields":{"amount":{"value":100},"trip_to":{"shown":null},"receipt":{"required":false}}}"#),
            // a price outside its range: the total, and whether a receipt is asked for, are not decided
            case(r#"{"form":{"category":"travel","items":[{"what":"Paper","price":2000000}]}}"#, r#"{"fields":{"amount":{"value":null},"trip_to":{"shown":true},"receipt":{"required":null}}}"#),
            // 21 rows, past the 20 the field takes: neither are they
            (format!(r#"{{"form":{{"category":"travel","items":[{}]}}}}"#, pens.join(",")), r#"{"fields":{"amount":{"value":null},"trip_to":{"shown":true},"receipt":{"required":null}}}"#),
        ],
    );
    // a day of leave: the reason is asked for a special leave alone, and not decided with no kind
    assert_answers(
        "examples/leave.ringi",
        &[
            case(r#"{"form":{"kind":"special","first_day":"2026-11-20","days":1}}"#, r#"{"fields":{"back_on":{"value":"2026-11-23"},"reason":{"required":true}}}"#),
            case(r#"{"form":{"kind":"paid","first_day":"2026-11-16","days":3}}"#, r#"{"fields":{"back_on":{"value":"2026-11-19"},"reason":{"required":false}}}"#),
            case(r#"{"form":{"days":3}}"#, r#"{"fields":{"back_on":{"value":null},"reason":{"required":null}}}"#),
        ],
    );
}

/// A field hidden is read as empty, whatever value it holds (DESIGN 4.9): a vendor given while the
/// kind is a service, and a vendor whose own condition is not decided, show no contact, which is
/// shown when there is a vendor. Text of spaces alone is empty too.
#[test]
fn a_field_hidden_is_read_as_empty() {
    assert_answers(
        "tests/machine/supplies.ringi",
        &[
            case(r#"{"form":{"kind":"goods","vendor":"Acme"}}"#, r#"{"fields":{"total":{"value":null},"vendor":{"shown":true},"contact":{"shown":true},"label":{"shown":true}}}"#),
            case(r#"{"form":{"kind":"service","vendor":"Acme","price":100,"quantity":3}}"#, r#"{"fields":{"total":{"value":300},"vendor":{"shown":false},"contact":{"shown":false},"label":{"shown":false}}}"#),
            case(r#"{"form":{"vendor":"Acme"}}"#, r#"{"fields":{"total":{"value":null},"vendor":{"shown":null},"contact":{"shown":false},"label":{"shown":null}}}"#),
            case(r#"{"form":{"kind":"goods","vendor":"   "}}"#, r#"{"fields":{"total":{"value":null},"vendor":{"shown":true},"contact":{"shown":false},"label":{"shown":true}}}"#),
        ],
    );
    // a field that may be left empty, left so: a computation that reads it shows no value, and a
    // condition reads it as none
    assert_answers(
        "tests/machine/absent.ringi",
        &[
            case(
                r#"{"form":{"kind":"goods","asked":3,"lines":[{"what":"A"}]}}"#,
                r#"{"fields":{"total":{"value":0},"over":{"value":null},"note":{"required":false},"detail":{"shown":false},"plain":{"shown":true},"why":{"required":true},"hurry":{"shown":false},"many":{"shown":true},"soon":{"required":false},"costly":{"required":false},"refill":{"required":false},"where":{"shown":false}}}"#,
            ),
            case(
                r#"{"form":{"kind":"goods","size":"large","asked":3,"left":1,"urgent":true,"stock":4,"lines":[{"what":"A","price":400}]}}"#,
                r#"{"fields":{"total":{"value":400},"over":{"value":2},"note":{"required":true},"detail":{"shown":true},"plain":{"shown":false},"why":{"required":false},"hurry":{"shown":true},"many":{"shown":false},"soon":{"required":true},"costly":{"required":true},"refill":{"required":true},"where":{"shown":true}}}"#,
            ),
        ],
    );
}

/// A total is worked out from the rows there so far, fewer than the field asks for too, as the
/// screens show it: a row whose value the total reads is empty is not there yet. Each row's own
/// computation is worked out as the machine works it out, none where a value it reads is empty or
/// outside its range, in the rows' order; a total that reads it then is not decided.
#[test]
fn the_rows_are_worked_out_as_they_are_there() {
    assert_answers(
        "examples/expense.ringi",
        &[
            case(r#"{"form":{"items":[{"what":"Paper","price":null}]}}"#, r#"{"fields":{"amount":{"value":0},"trip_to":{"shown":null},"receipt":{"required":false}}}"#),
            case(r#"{"form":{"category":"travel","items":[]}}"#, r#"{"fields":{"amount":{"value":0},"trip_to":{"shown":true},"receipt":{"required":false}}}"#),
        ],
    );
    assert_answers(
        "examples/purchase.ringi",
        &[
            case(
                r#"{"form":{"kind":"goods","lines":[{"item":"Paper","quantity":2,"price":600},{"item":"Pens","quantity":null,"price":100}]}}"#,
                r#"{"fields":{"lines":{"rows":[{"subtotal":{"value":1200}},{"subtotal":{"value":null}}]},"amount":{"value":1200}}}"#,
            ),
            case(r#"{"form":{"lines":[{"item":"Desk","quantity":10001,"price":600}]}}"#, r#"{"fields":{"lines":{"rows":[{"subtotal":{"value":null}}]},"amount":{"value":null}}}"#),
        ],
    );
    // one row of the two to three the field asks for, and three
    assert_answers(
        "tests/machine/ranges.ringi",
        &[
            case(r#"{"form":{"rate":5,"lines":[{"price":5}]}}"#, r#"{"fields":{"lines":{"rows":[{"fee":{"value":10}}]},"total":{"value":5},"double":{"value":10}}}"#),
            case(r#"{"form":{"rate":10,"lines":[{"price":5},{"price":6}]}}"#, r#"{"fields":{"lines":{"rows":[{"fee":{"value":10}},{"fee":{"value":12}}]},"total":{"value":11},"double":{"value":22}}}"#),
        ],
    );
}

/// The day the form counts is worked out as the machine counts it, in its calendar's business days:
/// none while a value it reads is empty or outside its range.
#[test]
fn a_day_is_counted_as_the_machine_counts_it() {
    assert_answers(
        "examples/leave.ringi",
        &[
            // Monday and three business days; Friday and one, over a weekend the calendar closes
            case(r#"{"form":{"kind":"paid","first_day":"2026-11-16","days":3}}"#, r#"{"fields":{"back_on":{"value":"2026-11-19"},"reason":{"required":false}}}"#),
            case(r#"{"form":{"kind":"special","first_day":"2026-11-20","days":1}}"#, r#"{"fields":{"back_on":{"value":"2026-11-23"},"reason":{"required":true}}}"#),
            case(r#"{"form":{"kind":"sick","first_day":"2026-11-16","days":21}}"#, r#"{"fields":{"back_on":{"value":null},"reason":{"required":false}}}"#),
            case(r#"{"form":{"first_day":"2025-12-31","days":2}}"#, r#"{"fields":{"back_on":{"value":null},"reason":{"required":null}}}"#),
            case(r#"{"form":{"kind":"special","reason":"   "}}"#, r#"{"fields":{"back_on":{"value":null},"reason":{"required":true}}}"#),
        ],
    );
}

/// What an approver gives is read in place of the form's values: of the fields they give (their own,
/// and those `approvers edit` names) alone, a value of null keeping the form's.
#[test]
fn what_an_approver_gives_is_read_in_place_of_the_form() {
    assert_answers(
        "tests/machine/supplies.ringi",
        &[
            case(r#"{"form":{"kind":"service","vendor":"Acme"},"edited":{"kind":"goods"}}"#, r#"{"fields":{"total":{"value":null},"vendor":{"shown":true},"contact":{"shown":true},"label":{"shown":true}}}"#),
            // the price is no approver's to give
            case(r#"{"form":{"kind":"goods","vendor":"Acme"},"edited":{"vendor":null,"price":5}}"#, r#"{"fields":{"total":{"value":null},"vendor":{"shown":true},"contact":{"shown":true},"label":{"shown":true}}}"#),
        ],
    );
    assert_answers(
        "tests/machine/steps.ringi",
        &[
            case(r#"{"form":{"share":8},"edited":null}"#, r#"{"fields":{"double":{"value":16},"cap":{"shown":false}}}"#),
            case(r#"{"form":{"share":8},"edited":{"share":12}}"#, r#"{"fields":{"double":{"value":24},"cap":{"shown":false}}}"#),
            case(r#"{"form":{"share":8},"edited":{"tenth":5}}"#, r#"{"fields":{"double":{"value":16},"cap":{"shown":true}}}"#),
        ],
    );
}

/// What is no call of the contract is refused, with why: no form, or a value of no field's type. A
/// key of no field the person gives is not read, and a kind whose form works nothing out answers no
/// field.
#[test]
fn what_is_no_call_of_the_contract_is_refused() {
    let got = answers(
        "examples/expense.ringi",
        &strings(&[
            r#"{}"#,
            r#"{"form":null}"#,
            r#"{"form":5}"#,
            r#"{"form":{"category":5}}"#,
            r#"{"form":{"items":{}}}"#,
            r#"{"form":{"items":[5]}}"#,
            r#"{"form":{},"edited":5}"#,
            r#"{"form":{"amount":5,"category":"travel"}}"#,
        ]),
    );
    assert_eq!(
        got,
        [
            "error: the input has no `form`",
            "error: the input has no `form`",
            "error: `form` is not an object",
            "error: the value of `form.category` does not fit the field's type",
            "error: `form.items` is not a list of rows",
            "error: `form.items[0]` is not an object",
            "error: `edited` is not an object",
            r#"{"fields":{"amount":{"value":0},"trip_to":{"shown":true},"receipt":{"required":false}}}"#,
        ]
    );
    assert_eq!(answers("examples/account.ringi", &strings(&[r#"{"form":{}}"#])), [r#"{"fields":{}}"#]);
}

/// The vectors of the entry `worked` (`vectors/worked/<name>.jsonl`) hold each case: a condition that
/// holds, one that does not and one not decided; a value worked out and one not; the rows of a field;
/// what an approver gives changing the answer; and a kind whose form works nothing out.
#[test]
fn the_vectors_of_worked_hold_each_case() {
    let mut paths: Vec<PathBuf> = common::example_files().into_iter().filter(|f| !f.starts_with("examples/shared/")).map(PathBuf::from).collect();
    paths.extend(["tests/machine/absent.ringi", "tests/machine/ranges.ringi", "tests/machine/steps.ringi", "tests/machine/supplies.ringi"].map(PathBuf::from));
    let engine = ringi::ports::Engine::new(common::joined());
    // as `ritsu gen` runs it (DESIGN 12.3)
    engine.keep_listing_apart();
    let kinds = engine.kinds(&paths, &PathBuf::from(".")).unwrap_or_else(|t| panic!("{}", t.en));
    let mut seen: Vec<String> = Vec::new();
    let mut table = Vec::new();
    for k in &kinds {
        let lines: Vec<Json> = k.worked.lines().map(|l| json::parse(l).unwrap_or_else(|e| panic!("{}: {e:?}: {l}", k.named.name))).collect();
        assert!(!lines.is_empty(), "{} has no vector of `worked`", k.named.name);
        table.push(format!("{} {}", k.named.name, lines.len()));
        for v in &lines {
            let (Some(from), Some(input), Some(answer)) = (v.get("from").and_then(Json::as_str), v.get("input"), v.get("answer")) else { panic!("{}: {}", k.named.name, v.compact()) };
            assert!(!from.is_empty() && input.get("form").is_some() && input.get("edited").is_some(), "{}: {}", k.named.name, v.compact());
            let fields = answer.get("fields").and_then(Json::as_obj).unwrap_or_else(|| panic!("{}: {}", k.named.name, v.compact()));
            for (_, said) in fields {
                for (what, x) in said.as_obj().unwrap_or_default() {
                    let x = match (what.as_str(), x) {
                        ("rows", _) => "rows".to_string(),
                        ("value", Json::Null) => "value null".to_string(),
                        ("value", _) => "value".to_string(),
                        (w, x) => format!("{w} {}", x.compact()),
                    };
                    if !seen.contains(&x) {
                        seen.push(x);
                    }
                }
            }
            if fields.is_empty() && !seen.iter().any(|s| s == "no field") {
                seen.push("no field".to_string());
            }
            // what an approver gives changes the answer of the same form with none
            if input.get("edited").is_some_and(|e| *e != Json::Null) {
                let plain = Json::obj([("form", input.get("form").cloned().unwrap()), ("edited", Json::Null)]);
                let without = lines.iter().find(|w| w.get("input") == Some(&plain)).and_then(|w| w.get("answer"));
                if without.is_some_and(|w| w != answer) && !seen.iter().any(|s| s == "an edit") {
                    seen.push("an edit".to_string());
                }
            }
        }
    }
    println!("{}", table.join(", "));
    seen.sort();
    assert_eq!(seen, ["an edit", "no field", "required false", "required null", "required true", "rows", "shown false", "shown null", "shown true", "value", "value null"]);
}
