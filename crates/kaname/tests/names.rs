//! What a module's names name (DESIGN 2.5, 2.6, 3.9): the files its `use` lines read (E201–E203),
//! its names (E005–E007, E101), and what its declarations declare of their values (E108). The
//! languages are joined as `ritsu kaname` joins them.

mod common;

use kaname::check::{Outcome, Run};
use kaname::suite::Suite;
use kaname::ty::{Range, Ty};
use ritsu_testkit::TempDir;
use ritsu_units::Rat;

/// The module `main.kn`, with the files beside it, checked with every language joined.
fn check_in(t: &TempDir, main: &str, suite: Suite) -> std::rc::Rc<Outcome> {
    let path = t.write("main.kn", main);
    Run::new(suite).file(path.to_str().unwrap()).unwrap()
}

fn codes(o: &Outcome) -> Vec<&'static str> {
    common::codes(o)
}

fn text(o: &Outcome) -> String {
    o.diags.iter().map(|d| d.render(ritsu_base::text::Lang::En)).collect()
}

const FEE: &str = "rule fee v1\n\ninputs\n  amount : money[USDc]  range >=1USDc <=100_000USDc\n\noutputs\n  fee : money[USDc]  round half_up(1USDc)\n\nresult fee = amount * 3%\n";

#[test]
fn the_files_a_module_reads() {
    let t = TempDir::new("kaname-uses");
    t.write("rules/fee.rule", FEE);
    t.write("rules/broken.rule", "rule broken v1\n\ninputs\n  amount : money[USDc]\n");
    t.write("notes.txt", "x");
    // a rule that passes rulec's check, read through the port
    let o = check_in(&t, "module m v1\n\nuse rule fee from \"rules/fee.rule\"\n\nfn f(a: money[USDc] range >=1USDc <=10USDc) -> money[USDc]\n  fee(amount: a).fee\n", common::joined());
    assert!(o.diags.is_empty(), "{}", text(&o));
    let c = o.checked.as_ref().unwrap();
    assert_eq!(c.uses.len(), 1);
    // a file that is not there, of the wrong kind, or that its language does not pass
    let o = check_in(&t, "module m v1\n\nuse rule fee from \"rules/none.rule\"\nuse rule x from \"notes.txt\"\nuse rule broken from \"rules/broken.rule\"\n", common::joined());
    assert_eq!(codes(&o), ["E201", "E201", "E201"]);
    let all = text(&o);
    assert!(all.contains("There is no file `"), "{all}");
    assert!(all.contains("`use rule` reads a `.rule` file"), "{all}");
    assert!(all.contains("does not pass rulec's check") && all.contains("= E"), "{all}");
    // what a function of the module says of a file it could not read is said once, at the `use`
    let o = check_in(&t, "module m v1\n\nuse rule fee from \"rules/none.rule\"\n\nfn f() -> fee.outputs\n  fee(amount: 1USDc)\n", common::joined());
    assert_eq!(codes(&o), ["E201"]);
}

#[test]
fn the_binary_of_the_crate_reads_no_other_language() {
    let t = TempDir::new("kaname-unjoined");
    t.write("rules/fee.rule", FEE);
    t.write("other.kn", "module other v1\n\nuse rule fee from \"rules/fee.rule\"\n");
    for main in ["module m v1\n\nuse rule fee from \"rules/fee.rule\"\n", "module m v1\n\nuse dates d from \"d.cal\"\n", "module m v1\n\nuse book b from \"b.book\"\n"] {
        let o = check_in(&t, main, Suite::default());
        assert_eq!((codes(&o), o.exit()), (vec!["E202"], 2), "{main}");
        assert!(text(&o).contains("ritsu kaname check <file.kn>"), "{}", text(&o));
    }
    // through a module it reads
    let o = check_in(&t, "module m v1\n\nuse module other from \"other.kn\"\n", Suite::default());
    assert_eq!((codes(&o), o.exit()), (vec!["E202"], 2));
    // a module that reads only modules needs no other language
    t.write("plain.kn", "module plain v1\n\nfn one() -> number\n  1\n");
    let o = check_in(&t, "module m v1\n\nuse module plain from \"plain.kn\"\n\nfn two() -> number\n  plain.one() + 1\n", Suite::default());
    assert!(o.diags.is_empty(), "{}", text(&o));
}

#[test]
fn modules_that_read_each_other_in_a_ring() {
    let t = TempDir::new("kaname-ring");
    t.write("a.kn", "module a v1\n\nuse module b from \"b.kn\"\n");
    t.write("b.kn", "module b v1\n\nuse module a from \"a.kn\"\n");
    t.write("self.kn", "module self_ v1\n\nuse module me from \"self.kn\"\n");
    let run = Run::new(common::joined());
    let a = run.file(t.path().join("a.kn").to_str().unwrap()).unwrap();
    assert_eq!(codes(&a), ["E203"]);
    let b = run.file(t.path().join("b.kn").to_str().unwrap()).unwrap();
    assert_eq!(codes(&b), ["E203"]);
    let s = run.file(t.path().join("self.kn").to_str().unwrap()).unwrap();
    assert_eq!(codes(&s), ["E203"]);
    assert!(text(&s).contains("The ring: self.kn → self.kn"), "{}", text(&s));
    // a module that does not pass kaname's check is E201, with what it says
    t.write("bad.kn", "module bad v1\n\nfn f() -> number\n  x\n");
    let o = check_in(&t, "module m v1\n\nuse module bad from \"bad.kn\"\n", common::joined());
    assert_eq!(codes(&o), ["E201"]);
    assert!(text(&o).contains("E101: There is no `x`"), "{}", text(&o));
}

#[test]
fn types_and_functions_of_another_module() {
    let t = TempDir::new("kaname-modules");
    t.write(
        "pricing.kn",
        "module pricing v1\n\nenum tier = basic < gold\n\nrecord Line\n  qty   : number  range >=1 <=9\n  price : money[USDc]  range >=0USDc <=100USDc\n\nfn total(lines: list[Line] count >=1 <=9) -> money[USDc]\n  sum(lines, l => l.price * l.qty)\n",
    );
    let o = check_in(
        &t,
        "module m v1\n\nuse module pricing from \"pricing.kn\"\n\nfn f(lines: list[pricing.Line] count >=1 <=9, t: pricing.tier) -> money[USDc]\n  let best = max(t, pricing.tier.basic)\n  if best = pricing.tier.gold then pricing.total(lines: lines) else pricing.total(lines: lines) * 2\n",
        common::joined(),
    );
    assert!(o.diags.is_empty(), "{}", text(&o));
    let o = check_in(&t, "module m v1\n\nuse module pricing from \"pricing.kn\"\n\nfn f() -> pricing.Lines\n  pricing.nothing()\n", common::joined());
    assert_eq!(codes(&o), ["E101", "E101"]);
}

#[test]
fn names_given_twice_or_not_ascii() {
    let check = |src: &str| -> Vec<&'static str> { codes(&common::check_text("t.kn", src)) };
    // one space for the `use` names and the declarations, by name and by alias
    assert_eq!(check("module m v1\n\nrecord Line\n  a : string\n\nenum Line = x | y\n"), ["E006"]);
    assert_eq!(check("module m v1\n\nrecord 明細(Line)\n  a : string\n\nrecord 行(Line)\n  b : string\n"), ["E006"]);
    assert_eq!(check("module m v1\n\nfn sum() -> number\n  1\n"), ["E006"]);
    assert_eq!(check("module m v1\n\nrecord R\n  a : string\n  a : bool\n"), ["E006"]);
    assert_eq!(check("module m v1\n\nenum e = a | a\n"), ["E006"]);
    assert_eq!(check("module m v1\n\nfn f(x: bool, x: bool) -> bool\n  true\n"), ["E006"]);
    // a name the generated code takes: ASCII, or an alias (E005), and a word it can take (E007)
    assert_eq!(check("module 例 v1\n"), ["E005"]);
    assert_eq!(check("module m v1\n\nrecord R\n  名前 : string\n"), ["E005"]);
    assert_eq!(check("module m v1\n\nrecord R\n  名前(名前) : string\n"), ["E005"]);
    assert_eq!(check("module m v1\n\nenum e = 金 | silver\n"), ["E005"]);
    assert_eq!(check("module m v1\n\nfn def() -> number\n  1\n"), ["E007"]);
    assert_eq!(check("module m v1\n\nrecord R\n  名前(func) : string\n"), ["E007"]);
    assert_eq!(check("module m v1\n\nrecord KanameError\n  a : string\n"), ["E007"]);
    // a `let` needs no alias, as rulec's internal names need none
    assert_eq!(check("module m v1\n\nfn f() -> number\n  let 合計 = 1\n  合計\n"), Vec::<&str>::new());
    // a type that names nothing
    assert_eq!(check("module m v1\n\nrecord R\n  a : Nothing\n"), ["E101"]);
    assert_eq!(check("module m v1\n\nrecord R\n  a : money[XYZ]\n"), ["E101"]);
    assert_eq!(check("module m v1\n\nrecord R\n  a : mass[cm]\n"), ["E101"]);
}

#[test]
fn what_a_parameter_reaches_declares_its_range() {
    let check = |src: &str| -> Vec<&'static str> { codes(&common::check_text("t.kn", src)) };
    // a parameter's numbers and dates, its list, its rate
    assert_eq!(check("module m v1\nfn f(n: number) -> number\n  n\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(d: date) -> date\n  d\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(xs: list[string]) -> number\n  count(xs)\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(xs: list[string] count >=1) -> number\n  count(xs)\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(r: rate) -> number\n  0\n"), ["E108"]);
    // a rate that goes out of a function declares its step too: its answer, a field of a record
    assert_eq!(check("module m v1\nfn f(r: rate[step 1%]) -> rate\n  r\n"), ["E108"]);
    assert_eq!(check("module m v1\nrecord R\n  share : rate\nfn f(r: rate[step 1%]) -> R\n  R { share: r }\n"), ["E108"]);
    // a rate with its step may leave its range out, as rulec's inputs may
    assert_eq!(check("module m v1\nfn f(r: rate[step 1%]) -> rate[step 1%]\n  r\n"), Vec::<&str>::new());
    // through optionals, lists and records
    assert_eq!(check("module m v1\nfn f(n: number?) -> number?\n  n\n"), ["E108"]);
    assert_eq!(check("module m v1\nrecord Inner\n  qty : number\nrecord Outer\n  inner : Inner?\nfn f(o: list[Outer] count <=3) -> number\n  1\n"), ["E108"]);
    // a record only answered with may leave its ranges out (DESIGN 3.9)
    assert_eq!(
        check("module m v1\nrecord Claim\n  days : number\n  due  : date\nfn f(n: number range >=1 <=9, d: date range >=2026-01-01 <=2026-12-31) -> Claim\n  Claim { days: n, due: d }\n"),
        Vec::<&str>::new()
    );
    // one diagnostic for a field, however many parameters reach it, at the field, naming the parameter
    let o = common::check_text("t.kn", "module m v1\nrecord Line\n  qty : number\nfn f(a: Line, b: Line) -> number\n  1\n");
    assert_eq!(codes(&o), ["E108"]);
    assert_eq!(o.diags[0].line, Some(3));
    assert!(text(&o).contains("The parameter `a` of `f` (line 4) reaches this field."), "{}", text(&o));
    // the field of another module's record: said at the parameter, naming the field
    let t = TempDir::new("kaname-reach");
    t.write("types.kn", "module types v1\nrecord Line\n  qty : number\n");
    let o = check_in(&t, "module m v1\nuse module types from \"types.kn\"\nfn f(l: types.Line) -> number\n  1\n", common::joined());
    assert_eq!(codes(&o), ["E108"]);
    assert_eq!(o.diags[0].line, Some(3));
    assert!(text(&o).contains("types.kn:3."), "{}", text(&o));
}

#[test]
fn ranges_that_cannot_hold() {
    let check = |src: &str| -> Vec<&'static str> { codes(&common::check_text("t.kn", src)) };
    assert_eq!(check("module m v1\nfn f(n: number range >=9 <=1) -> number\n  n\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(n: number range >=1 >=2) -> number\n  n\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(s: string range >=1) -> string\n  s\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(n: number count <=3 range >=0 <=1) -> number\n  n\n"), ["E108"]);
    assert_eq!(check("module m v1\nrecord Node\n  kids : list[Node] count <=3\n"), ["E108"]);
    // an end that is not a value of the type
    assert_eq!(check("module m v1\nfn f(n: number range >=1USD <=2USD) -> number\n  n\n"), ["E102"]);
    assert_eq!(check("module m v1\nfn f(w: mass[g] range >=0g <=1lb) -> mass[g]\n  w\n"), ["E104"]);
    assert_eq!(check("module m v1\nfn f(r: rate[step 1%] range >=0% <=0.5%) -> rate[step 1%]\n  r\n"), ["E104"]);
    assert_eq!(check("module m v1\nfn f(r: rate[step 0%]) -> number\n  1\n"), ["E104"]);
}

#[test]
fn the_ends_are_read_in_the_unit_of_the_place() {
    let o = common::check_text(
        "t.kn",
        "module m v1\nrecord P\n  w : mass[oz]  range >0oz <=15lb\n  r : rate[step 0.01%]  range >=0% <15%\n  d : date  range >2026-01-01 <=2026-12-31\n  xs : list[number]  count <=3  range >=0 <=9\nfn f(p: P) -> number\n  1\n",
    );
    assert!(o.diags.is_empty(), "{}", text(&o));
    let c = o.checked.as_ref().unwrap();
    let fields = &c.decls.records[0].fields;
    let range = |i: usize| fields[i].slot.range.clone().unwrap();
    // 15lb is 240oz; `>0` on whole ounces is `>=1`
    assert_eq!(range(0), Range { min: Some(Rat::int(1)), max: Some(Rat::int(240)) });
    // `<15%` on the step of 0.01% is `<=14.99%`
    assert_eq!(range(1), Range { min: Some(Rat::zero()), max: Some(Rat::new(1499, 10_000)) });
    // a day after 2026-01-01
    assert_eq!(range(2).min, Some(Rat::int(20_455)));
    assert_eq!(fields[3].slot.count, Some(Range { min: None, max: Some(Rat::int(3)) }));
    assert!(matches!(&fields[3].slot.ty, Ty::List(_)));
}
