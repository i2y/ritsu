//! The values of the examples (DESIGN 5.3). A row of the examples is a request: a value the form
//! does not take is said at its cell, whether the route reads the column or not. A request with two
//! fields declared alike, `left` and `right`, whose route reads one of them, is told the same thing
//! of the same cells whichever it reads; with the cells corrected, it passes either way. A column the
//! route cannot read (a text, a choice from a master) is held to the form the same way. The values
//! of a rule's enum are held to the enum where the route's table and its conditions write them too.

mod common;

use ringi::check::{Options, Outcome, Run};
use ritsu_base::text::Lang;

/// The check of `src` as a file of `examples/`, in the organization's version of 2026-11-16.
fn check(src: &str) -> Outcome {
    let at = ritsu_ports::day_of("2026-11-16");
    Run::with(common::joined(), Options { at, ..Options::default() }).text("examples/values.ringi", src)
}

/// What the check prints of the diagnostics, in `lang`.
fn said(o: &Outcome, lang: Lang) -> String {
    o.diags.iter().map(|d| d.render(lang)).collect()
}

/// A kind of field, declared twice alike.
struct Kind {
    what: &'static str,
    /// The `use` lines it needs, after the organization's.
    uses: &'static str,
    /// The fields the two read, before them.
    before: &'static str,
    /// The declaration after `left  : ` and `right : `, `{f}` standing for the name.
    decl: &'static str,
    /// The condition of the stage `audit`, `{f}` standing for the field it reads.
    when: &'static str,
    /// The cells of `left` and `right`, the value the form does not take.
    bad: [&'static str; 2],
    /// The cells corrected, which the condition takes: the row comes to `audit`.
    good: &'static str,
    /// What is said of each, in order, `{f}` standing for the field.
    codes: [&'static str; 2],
    said: [&'static str; 2],
}

fn kinds() -> Vec<Kind> {
    vec![
        Kind {
            what: "a number outside its range",
            uses: "",
            before: "",
            decl: r#"number "Count"  range >=1 <=10"#,
            when: "{f} > 5",
            bad: ["99", "0"],
            good: "7",
            codes: ["E601", "E601"],
            said: ["`99` is outside the range of `left` (`>=1 <=10`)", "`0` is outside the range of `right` (`>=1 <=10`)"],
        },
        Kind {
            what: "an amount outside its range",
            uses: "",
            before: "",
            decl: r#"money[JPY] "Amount"  range >=1JPY <=100_000JPY"#,
            when: "{f} > 50_000JPY",
            bad: ["100_001JPY", "0JPY"],
            good: "60_000JPY",
            codes: ["E601", "E601"],
            said: ["`100_001JPY` is outside the range of `left` (`>=1JPY <=100_000JPY`)", "`0JPY` is outside the range of `right` (`>=1JPY <=100_000JPY`)"],
        },
        Kind {
            what: "an amount off its step",
            uses: "",
            before: "",
            decl: r#"money[JPY] "Amount"  range >=1JPY <=100_000JPY"#,
            when: "{f} > 50_000JPY",
            bad: ["60_000.5JPY", "40_000.5JPY"],
            good: "60_000JPY",
            codes: ["E601", "E601"],
            said: ["`60_000.5JPY` is not a multiple of the step of `left` (`1JPY`)", "`40_000.5JPY` is not a multiple of the step of `right` (`1JPY`)"],
        },
        Kind {
            what: "a rate off its step",
            uses: "",
            before: "",
            decl: r#"rate[step 0.1%] "Share"  range >=0% <=100%"#,
            when: "{f} > 50%",
            bad: ["60.05%", "0.15%"],
            good: "60%",
            codes: ["E601", "E601"],
            said: ["`60.05%` is not a multiple of the step of `left` (`0.1%`)", "`0.15%` is not a multiple of the step of `right` (`0.1%`)"],
        },
        Kind {
            what: "a date outside its range",
            uses: "",
            before: "",
            decl: r#"date "Day"  range >=2026-01-01 <=2027-12-31"#,
            when: "{f} > 2026-06-30",
            bad: ["2028-01-01", "2025-12-31"],
            good: "2026-07-01",
            codes: ["E601", "E601"],
            said: ["`2028-01-01` is outside the range of `left` (`>=2026-01-01 <=2027-12-31`)", "`2025-12-31` is outside the range of `right` (`>=2026-01-01 <=2027-12-31`)"],
        },
        Kind {
            what: "a computed amount outside what its computation comes to",
            uses: "",
            before: "  price    : money[JPY] \"Price\"  range >=1JPY <=100_000JPY\n  quantity : number \"Quantity\"  range >=1 <=10\n",
            decl: r#"money[JPY] "Total" = price * quantity  range >=1JPY <=10_000_000JPY"#,
            when: "{f} > 500_000JPY",
            bad: ["5_000_000JPY", "2_000_000JPY"],
            good: "600_000JPY",
            codes: ["E601", "E601"],
            said: ["`5_000_000JPY` is outside what the computation of `left` can come to (`>=1JPY <=1_000_000JPY`)", "`2_000_000JPY` is outside what the computation of `right` can come to (`>=1JPY <=1_000_000JPY`)"],
        },
        Kind {
            what: "a value a choice does not have",
            uses: "",
            before: "",
            decl: "choice \"Size\"\n    small \"Small\"\n    large \"Large\"",
            when: "{f} is large",
            bad: ["huge", "\"large\""],
            good: "large",
            codes: ["E004", "E009"],
            said: ["`huge` is not one of the values to choose from", "`\"large\"` is `text`, and the column `right` is `choice`"],
        },
        Kind {
            what: "a value a rule's enum does not have",
            uses: "use rule authority from \"shared/authority.rule\"\n",
            before: "",
            decl: r#"choice from authority.kind "Kind""#,
            when: "{f} is capex",
            bad: ["bogus", "\"capex\""],
            good: "capex",
            codes: ["E004", "E009"],
            said: ["`bogus` is not one of the values to choose from", "`\"capex\"` is `text`, and the column `right` is `choice`"],
        },
    ]
}

/// The request of a kind, its route reading `reads`, its examples one row of `cells`.
fn request(k: &Kind, reads: &str, cells: [&str; 2]) -> String {
    let decl = |f: &str| k.decl.replace("{f}", f);
    let when = k.when.replace("{f}", reads);
    let w = cells.iter().map(|c| c.chars().count()).max().unwrap_or(0).max(5);
    let pad = |c: &str| format!("{c}{}", " ".repeat(w - c.chars().count()));
    format!(
        "request example v1 \"Example\"\n\nuse organization org from \"shared/company.ringi\"\n{}\nform\n{}  left  : {}\n  right : {}\n\nroute\n  first   manager\n  audit   board_member  when {when}\n\nwho\n  manager      = manager of requester\n  board_member = team board\n\nevery stage\n  when empty escalate, then board_member\n\nticket\n  self approval  not allowed\n\nexamples\n| {} | {} | -> first | audit        |\n| {} | {} | manager  | board_member |\n",
        k.uses,
        k.before,
        decl("left"),
        decl("right"),
        pad("left"),
        pad("right"),
        pad(cells[0]),
        pad(cells[1]),
    )
}

/// Each cell is said of, at the cell, the same whichever of the two columns the route reads; the
/// cells corrected pass either way.
#[test]
fn a_column_the_route_reads_and_one_it_does_not_are_told_the_same() {
    let mut failures = Vec::new();
    for k in kinds() {
        let mut told = Vec::new();
        for reads in ["left", "right"] {
            let src = request(&k, reads, k.bad);
            let o = check(&src);
            let codes = common::codes(&o);
            if codes != k.codes {
                failures.push(format!("{} (the route reads `{reads}`): {codes:?}, not {:?}\n{}", k.what, k.codes, said(&o, Lang::En)));
                continue;
            }
            // at the cells: the line of the row, the column of each value
            let row = src.lines().count();
            let cols: Vec<usize> = o.diags.iter().map(|d| d.col.unwrap_or(0)).collect();
            let line = src.lines().last().unwrap_or_default();
            let want: Vec<usize> = k.bad.iter().map(|c| line.find(c).map(|i| line[..i].chars().count() + 1).unwrap_or(0)).collect();
            if o.diags.iter().any(|d| d.line != Some(row)) || cols != want {
                failures.push(format!("{} (the route reads `{reads}`): said at {:?}, not at line {row}, columns {want:?}", k.what, o.diags.iter().map(|d| (d.line, d.col)).collect::<Vec<_>>()));
            }
            let en = said(&o, Lang::En);
            for s in k.said {
                if !en.contains(s) {
                    failures.push(format!("{} (the route reads `{reads}`): `{s}` is not said\n{en}", k.what));
                }
            }
            told.push((en, said(&o, Lang::Ja)));
            let fixed = check(&request(&k, reads, [k.good, k.good]));
            if !fixed.diags.is_empty() {
                failures.push(format!("{} corrected (the route reads `{reads}`):\n{}", k.what, said(&fixed, Lang::En)));
            }
        }
        if let [a, b] = told.as_slice()
            && a != b
        {
            failures.push(format!("{}: the route reading `left` is told\n{}\nand reading `right`\n{}", k.what, a.0, b.0));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A text and a choice from a master, which the route cannot read, are held to the form too: a text
/// past its `count` or short of it, a key no row of the master has (in any of its days).
#[test]
fn a_column_the_route_cannot_read_is_held_to_the_form() {
    let src = |note: &str, vendor: &str| {
        format!(
            "request example v1 \"Example\"\n\nuse organization org from \"shared/company.ringi\"\nuse masters m from \"shared/masters.ringi\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n  note   : text \"Note\"  count >=2 <=20\n  vendor : choice from m.vendors \"Vendor\"\n\nroute\n  first   manager\n  audit   board_member  when amount > 50_000JPY\n\nwho\n  manager      = manager of requester\n  board_member = team board\n\nevery stage\n  when empty escalate, then board_member\n\nticket\n  self approval  not allowed\n\nexamples\n| amount    | note | vendor | -> first | audit        |\n| 60_000JPY | {note} | {vendor} | manager  | board_member |\n"
        )
    };
    let o = check(&src("\"a note longer than twenty\"", "\"V-9999\""));
    assert_eq!(common::codes(&o), ["E601", "E601"], "{}", said(&o, Lang::En));
    let en = said(&o, Lang::En);
    assert!(en.contains("`\"a note longer than twenty\"` is 25 characters long, past the limit of `note` (`count >=2 <=20`)"), "{en}");
    assert!(en.contains("`\"V-9999\"` is not a row of the master `vendors`, which `vendor` chooses from"), "{en}");
    assert!(en.contains("A row of the examples is a request: write values the form takes, whether the route reads the column or not."), "{en}");
    let ja = said(&o, Lang::Ja);
    assert!(ja.contains("`\"a note longer than twenty\"` は25文字で、`note` の上限（`count >=2 <=20`）を超えます"), "{ja}");
    assert!(ja.contains("`\"V-9999\"` は、`vendor` が選ぶマスタ `vendors` の行ではありません"), "{ja}");
    let o = check(&src("\"a\"", "\"V-0012\""));
    assert_eq!(common::codes(&o), ["E601"], "{}", said(&o, Lang::En));
    assert!(said(&o, Lang::En).contains("`\"a\"` is 1 character long, short of the least of `note` (`count >=2 <=20`)"), "{}", said(&o, Lang::En));
    // a row of the master that may be chosen on some days only is a row
    let o = check(&src("\"two\"", "\"V-0044\""));
    assert!(o.diags.is_empty(), "{}", said(&o, Lang::En));
}

/// A value a rule's enum does not have is E004 where the route's table and its conditions write it,
/// as for a `choice` of the request's own values, with the values it has.
#[test]
fn the_values_of_a_rules_enum_are_held_to_it_in_the_route() {
    let head = "request example v1 \"Example\"\n\nuse organization org from \"shared/company.ringi\"\nuse rule authority from \"shared/authority.rule\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n  kind   : choice from authority.kind \"Kind\"\n\n";
    let tail = "\nwho\n  manager      = manager of requester\n  board_member = team board\n\nevery stage\n  when empty escalate, then board_member\n\nticket\n  self approval  not allowed\n";
    let table = format!("{head}route\n| kind     | -> first | audit        |\n| goods    | manager  |              |\n| bogus    | manager  | board_member |\n| capex    | manager  | board_member |\n| software | manager  | board_member |\n{tail}");
    let o = check(&table);
    assert_eq!(common::codes(&o), ["E004"], "{}", said(&o, Lang::En));
    let d = &o.diags[0];
    assert_eq!((d.line, d.col), (Some(13), Some(3)));
    assert!(said(&o, Lang::En).contains("`bogus` is not one of the values to choose from"), "{}", said(&o, Lang::En));
    assert!(said(&o, Lang::En).contains("`goods`, `capex` and `software`"), "{}", said(&o, Lang::En));
    let cond = format!("{head}route\n  first   manager\n  audit   board_member  when kind is bogus\n{tail}");
    let o = check(&cond);
    assert_eq!(common::codes(&o), ["E004"], "{}", said(&o, Lang::En));
    let o = check(&cond.replace("kind is bogus", "kind is \"capex\""));
    assert_eq!(common::codes(&o), ["E009"], "{}", said(&o, Lang::En));
    let o = check(&cond.replace("kind is bogus", "kind is capex"));
    assert!(o.diags.is_empty(), "{}", said(&o, Lang::En));
}
