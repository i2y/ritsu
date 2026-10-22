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

/// A rule with no outputs (an `enums` file) is read for its enums (DESIGN 2.7): a module that writes
/// its enum as a type passes, and its generated code reads the rule's module of types alone. Nothing
/// to call is generated from it (rulec's §15.258), so a call of it, or its outputs written as a type,
/// stop with E201 where they are written, and once. English first, the Japanese version beside it.
#[test]
fn a_rule_with_no_outputs_is_read_for_its_enums_and_not_called() {
    let t = TempDir::new("kaname-enums-file");
    t.write("rules/regions.rule", "enums regions v1\ndescription \"The regions a parcel is delivered to\"\n\nenum region = honshu | hokkaido | okinawa\n");
    t.write("rules/配送の地域.rule", "enums 配送の地域(delivery_regions) v1\ndescription \"荷物を届ける地域\"\n\nenum 地域(region) = 本州(honshu) | 北海道(hokkaido) | 沖縄(okinawa)\n");
    // its enum as a type: the module passes
    for main in [
        "module probe v1\n\nuse rule regions from \"rules/regions.rule\"\n\nfn far_away(r: regions.region) -> bool\n  r != honshu\n",
        "module 試し(probe) v1\n\nuse rule 地域 from \"rules/配送の地域.rule\"\n\nfn 遠いか(far_away)(r: 地域.地域) -> bool\n  r != 本州\n",
    ] {
        let o = check_in(&t, main, common::joined());
        assert_eq!(codes(&o), Vec::<&str>::new(), "{main}\n{}", text(&o));
    }
    // a call of it, an output of it, its outputs as a type: E201 where each is written
    let modules = [
        ("module probe v1\n\nuse rule regions from \"rules/regions.rule\"\n\nfn f(x: bool) -> bool\n  let r = regions()\n  x\n", "regions", (6, 11)),
        ("module probe v1\n\nuse rule regions from \"rules/regions.rule\"\n\nfn in_honshu(urgent: bool) -> bool\n  regions().region = honshu\n", "regions", (6, 3)),
        ("module probe v1\n\nuse rule regions from \"rules/regions.rule\"\n\nfn g(x: regions.outputs) -> bool\n  true\n", "regions", (5, 9)),
        ("module 試し(probe) v1\n\nuse rule 地域 from \"rules/配送の地域.rule\"\n\nfn 判定(f)(x: bool) -> bool\n  let r = 地域()\n  x\n", "地域", (6, 11)),
        ("module 試し(probe) v1\n\nuse rule 地域 from \"rules/配送の地域.rule\"\n\nfn 本州か(in_honshu)(急ぎ(urgent): bool) -> bool\n  地域().地域 = 本州\n", "地域", (6, 3)),
    ];
    for (main, rule, (line, col)) in modules {
        let o = check_in(&t, main, common::joined());
        assert_eq!(codes(&o), ["E201"], "{main}\n{}", text(&o));
        let d = &o.diags[0];
        assert_eq!((d.line, d.col), (Some(line), Some(col)), "{main}");
        let en = d.render(ritsu_base::text::Lang::En);
        assert!(en.contains(&format!("The rule `{rule}` has no outputs, and is not called")), "{en}");
        assert!(en.contains(&format!("= Nothing to call is generated from a rule with no outputs (an `enums` file). What a module reads it for is its enums, written `{rule}.<enum>` where a type goes.")), "{en}");
        let ja = d.render(ritsu_base::text::Lang::Ja);
        assert!(ja.contains(&format!("規則 `{rule}` には出力が無いので、呼べません")), "{ja}");
        assert!(ja.contains(&format!("= 出力の無い規則（`enums` のファイル）からは、呼ぶためのコードが生成されません。読めるのはその列挙で、`{rule}.<列挙>` と書いて型として使います。")), "{ja}");
    }
}

/// E101 for a field a value of a sum with no fields does not have says it has none, rather than
/// listing nothing.
#[test]
fn a_value_with_no_fields_says_it_has_none() {
    for (src, lang, note) in [
        (
            "module m v1\n\nenum Order\n  placed\n    on : date\n  cancelled\n\nfn why(c: Order.cancelled) -> string\n  c.reason\n",
            ritsu_base::text::Lang::En,
            "  = Order.cancelled has no fields.\n",
        ),
        (
            "module 例(m) v1\n\nenum 注文(Order)\n  受付(placed)\n    日(on) : date\n  取消(cancelled)\n\nfn 理由(why)(c: 注文.取消) -> string\n  c.理由\n",
            ritsu_base::text::Lang::Ja,
            "  = 注文.取消 にはフィールドがありません。\n",
        ),
    ] {
        let o = common::check_text("t.kn", src);
        assert_eq!(codes(&o), ["E101"], "{}", text(&o));
        let said = o.diags[0].render(lang);
        assert!(said.contains(note), "{said}");
    }
    // a record's fields are still listed
    let o = common::check_text("t.kn", "module m v1\n\nrecord Line\n  sku : string\n  qty : number\n\nfn f(l: Line) -> number\n  l.price\n");
    assert!(text(&o).contains("  = The fields of Line are `sku`, `qty`.\n"), "{}", text(&o));
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
fn a_range_and_a_count_may_be_left_out_and_a_rate_declares_its_step() {
    let check = |src: &str| -> Vec<&'static str> { codes(&common::check_text("t.kn", src)) };
    // a parameter's numbers, dates and lists, and what they reach, take the whole of their types
    // where no range and no count are written (DESIGN 22.1)
    assert_eq!(check("module m v1\nfn f(n: number) -> number\n  n\n"), Vec::<&str>::new());
    assert_eq!(check("module m v1\nfn f(d: date) -> date\n  d\n"), Vec::<&str>::new());
    assert_eq!(check("module m v1\nfn f(xs: list[string]) -> list[string]\n  xs\n"), Vec::<&str>::new());
    assert_eq!(check("module m v1\nfn f(xs: list[string] count >=1) -> list[string]\n  xs\n"), Vec::<&str>::new());
    assert_eq!(check("module m v1\nfn f(n: number?) -> number?\n  n\n"), Vec::<&str>::new());
    assert_eq!(check("module m v1\nfn f(xss: list[list[number]]) -> list[list[number]]\n  xss\n"), Vec::<&str>::new());
    assert_eq!(check("module m v1\nrecord Inner\n  qty : number\nrecord Outer\n  inner : Inner?\nfn f(o: list[Outer] count <=3) -> number\n  1\n"), Vec::<&str>::new());
    let t = TempDir::new("kaname-reach");
    t.write("types.kn", "module types v1\nrecord Line\n  qty : number\n");
    let o = check_in(&t, "module m v1\nuse module types from \"types.kn\"\nfn f(l: types.Line) -> number\n  1\n", common::joined());
    assert_eq!(codes(&o), Vec::<&str>::new());
    // a rate declares its step where it comes in and goes out: a parameter, an answer, a field
    let o = common::check_text("t.kn", "module m v1\nfn f(r: rate) -> number\n  0\n");
    assert_eq!(codes(&o), ["E108"]);
    assert!(text(&o).contains("The parameter `r` of `f` declares no step of the rate (`rate[step 0.01%]`)"), "{}", text(&o));
    assert_eq!(check("module m v1\nfn f(r: rate[step 1%]) -> rate\n  r\n"), ["E108"]);
    assert_eq!(check("module m v1\nrecord R\n  share : rate\nfn f(r: rate[step 1%]) -> R\n  R { share: r }\n"), ["E108"]);
    // a rate with its step may leave its range out, as rulec's inputs may
    assert_eq!(check("module m v1\nfn f(r: rate[step 1%]) -> rate[step 1%]\n  r\n"), Vec::<&str>::new());
    // a record only answered with leaves its ranges out as it did
    assert_eq!(
        check("module m v1\nrecord Claim\n  days : number\n  due  : date\nfn f(n: number range >=1 <=9, d: date range >=2026-01-01 <=2026-12-31) -> Claim\n  Claim { days: n, due: d }\n"),
        Vec::<&str>::new()
    );
}

#[test]
fn ranges_that_cannot_hold() {
    let check = |src: &str| -> Vec<&'static str> { codes(&common::check_text("t.kn", src)) };
    assert_eq!(check("module m v1\nfn f(n: number range >=9 <=1) -> number\n  n\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(n: number range >=1 >=2) -> number\n  n\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(s: string range >=1) -> string\n  s\n"), ["E108"]);
    assert_eq!(check("module m v1\nfn f(n: number count <=3 range >=0 <=1) -> number\n  n\n"), ["E108"]);
    // a record that holds itself through a list that cannot be empty: no value of it can be made
    // (DESIGN 28.1); one whose list may be empty can
    assert_eq!(check("module m v1\nrecord Node\n  kids : list[Node] count >=1 <=3\n"), ["E108"]);
    assert_eq!(check("module m v1\nrecord Node\n  kids : list[Node] count <=3\n"), Vec::<&str>::new());
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

/// What is `private` (DESIGN 28.2): used in its own module, and from another module nowhere (E101,
/// with a note that says it is `private`); what is not `private` of the same module is used.
#[test]
fn what_is_private_is_its_modules_alone() {
    let t = TempDir::new("kaname-private");
    t.write(
        "pricing.kn",
        "module pricing v1\n\nprivate record Bounds\n  low : number  range >=0 <=9\n\nprivate enum side = below | above\n\nprivate fn base(n: number range >=0 <=9) -> number\n  n + 1\n\nprivate fn side_of(b: Bounds) -> side\n  if b.low < 5 then side.below else side.above\n\nfn price(n: number range >=0 <=9) -> number\n  base(n: n)\n",
    );
    let o = check_in(&t, "module m v1\n\nuse module pricing from \"pricing.kn\"\n\nfn f(n: number range >=0 <=9) -> number\n  pricing.price(n: n)\n", common::joined());
    assert!(o.diags.is_empty(), "{}", text(&o));
    for body in [
        "fn f(n: number range >=0 <=9) -> number\n  pricing.base(n: n)\n",
        "fn f(b: pricing.Bounds) -> number\n  1\n",
        "fn f(s: pricing.side) -> number\n  1\n",
    ] {
        let o = check_in(&t, &format!("module m v1\n\nuse module pricing from \"pricing.kn\"\n\n{body}"), common::joined());
        assert_eq!(codes(&o), ["E101"], "{body}");
        assert!(text(&o).contains("`private`"), "{}", text(&o));
    }
}
