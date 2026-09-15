//! The unit table (§2.1, DESIGN §15.18).
//!
//! The one property worth more than all the others here: **two currencies never convert.**
//! There is no exchange rate in this tool, so a dollar written in a yen column has to stop.
//! Everything else — the hundredth of a currency, the imperial units — is the same relation
//! `mass[kg]` has always had to `mass[g]`, and is tested the same way: a literal in one unit
//! lands on the right integer in a column declared in another, or is refused for not landing
//! on a whole one.

fn check(src: &str) -> Vec<String> {
    rulec::check_source(src, "units.rule").iter().map(|d| d.code.to_string()).collect()
}

/// A rule whose single output column is money in `cur`, with `lit` written into it.
fn money(cur: &str, lit: &str) -> String {
    format!(
        "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  \
         fee(fee) : money[{cur}, incl_tax]  round up(1{cur})\n\n\
         table j(j)\npolicy unique\n| x | -> fee(fee) : money[{cur}, incl_tax] |\n\
         | true | {lit} |\n| false | 0{cur} |\n"
    )
}

/// The same for a quantity: an input declared in `unit`, with `lit` as its upper bound.
fn qty(dim: &str, unit: &str, lit: &str) -> String {
    format!(
        "rule t(t) v1\n\ninputs\n  w(w) : {dim}[{unit}]  range >=0{unit} <={lit}\n\n\
         outputs\n  r(r) : bool\n\n\
         table j(j)\npolicy unique\n| w | -> r(r) : bool |\n| - | true |\n"
    )
}

#[test]
fn 通貨は互いに換算されない() {
    // The whole point. A rate would make this a conversion; there is no rate here, and a
    // silent one would be the exact failure this tool exists to refuse.
    assert!(check(&money("USD", "100円")).contains(&"E103".to_string()));
    assert!(check(&money("円", "5USD")).contains(&"E103".to_string()));
    assert!(check(&money("EUR", "5USD")).contains(&"E103".to_string()));
    // And the same currency in the same unit is fine.
    assert!(check(&money("USD", "5USD")).is_empty());
}

#[test]
fn 通貨の主単位と百分の一は換算される() {
    // `USDc` is to `USD` what `銭` is to `円` and `g` is to `kg`.
    assert!(check(&money("USDc", "5USD")).is_empty(), "5USD は 500USDc");
    assert!(check(&money("USD", "500USDc")).is_empty(), "500USDc は 5USD");
    // 5.99 dollars is not a whole number of dollars, so the dollar column refuses it.
    assert!(check(&money("USD", "599USDc")).contains(&"E103".to_string()));
    assert!(check(&money("USDc", "599USDc")).is_empty());
}

/// Before the currency became the dimension, the declared unit's factor was never divided
/// out for money, so a yen literal in a 銭 column read as itself rather than as 100 times
/// itself. It was invisible because nothing in the corpus declares 銭.
#[test]
fn 銭の列に書いた円は百倍になる() {
    let src = money("銭", "5円");
    assert!(check(&src).is_empty(), "5円 は 500銭 として通る");
    let (f, c) = rulec::prepare(&src, "units.rule").expect("通る");
    let g = rulec::codegen::Gen::new(&f, &c, &src);
    assert!(g.python().contains("fee = 500"), "500 でなければ換算していない:\n{}", g.python());
}

#[test]
fn ヤードポンド法が書ける() {
    assert!(check(&qty("mass", "lb", "50lb")).is_empty());
    assert!(check(&qty("length", "in", "36in")).is_empty());
    assert!(check(&qty("length", "mi", "100mi")).is_empty());
    // A pound is 453.59237 g exactly, so it is not a whole number of grams and cannot be
    // written in a column that counts them.
    assert!(check(&qty("mass", "g", "1lb")).contains(&"E103".to_string()));
    // A kilogram is, and always could be.
    assert!(check(&qty("mass", "g", "2kg")).is_empty());
}

#[test]
fn 知らない単位はその場で名指しされる() {
    let ds = rulec::check_source(&money("USD", "100zzz"), "units.rule");
    let d = ds.iter().find(|d| d.code == "E103").expect("E103 が出る");
    let notes = d.notes.join(" ");
    assert!(notes.contains("zzz"), "その綴りを言う: {notes}");
    assert!(notes.contains("USD"), "書ける単位を言う: {notes}");
}

/// `5USD` used to lex as two tokens, because the lexer took a whitelist of the seven
/// characters the metric units happened to need.
#[test]
fn 通貨コードは一語として読まれる() {
    let toks = rulec::lex::lex_line(1, "| true | 1234EUR |").expect("読める");
    assert!(
        toks.iter().any(|t| matches!(&t.kind, rulec::lex::Kind::Num(n)
            if n.unit.as_deref() == Some("EUR") && n.digits == "1234")),
        "EUR が単位として付いていない: {toks:?}"
    );
}

/// A range bound the checker cannot read used to be skipped, leaving the range open on that
/// side — and that side is both the universe the completeness proof quantifies over and the
/// entry guard of the generated code. It answered "ok" for a rule with no upper bound at all.
#[test]
fn 読めない範囲の境界は黙って落ちない() {
    let src = "rule t(t) v1\n\ninputs\n  w(w) : mass[g]  range >=0g <=1lb\n\n\
               outputs\n  r(r) : bool\n\n\
               table j(j)\npolicy unique\n| w | -> r(r) : bool |\n| - | true |\n";
    let ds = rulec::check_source(src, "units.rule");
    assert!(rulec::has_error(&ds), "上限が読めないのに通ってはいけない");
    let d = ds.iter().find(|d| d.code == "E103").expect("E103 が出る");
    assert!(d.title.contains("1lb"), "どの境界かを言う: {}", d.title);
}

/// A literal inside an **expression** has to evaluate in every unit, not just the three the
/// evaluator used to try.
///
/// `Expr::Lit` guessed: 円, then a rate, then a plain number. Every other unit fell through
/// all three and the expression returned *nothing* — so a `define` over `重量 >= 500g`, or
/// over `額 >= 250EUR`, had no value, no table fired, and the vectors came out empty or
/// `null` while the generated code was right the whole time. `money[銭]` was worse: `500銭`
/// matched the 円 arm and came back as 5, so the oracle answered a hundredfold low and
/// `499銭 >= 500銭` evaluated to true.
///
/// This walks the whole unit table, because the bug was invisible for exactly as long as the
/// corpus happened to write its expressions in 円. Each rule puts the literal through **both**
/// callers of `Expr::Lit` that a rule can reach — a `define`'s comparison and a `derive`'s
/// arithmetic — since a fix that reached only one of them would look like a fix.
#[test]
fn 式の中のリテラルはどの単位でも評価される() {
    // (declared type, low, high, the literal). The high is twice the literal, so the derived
    // `x - lit` spans exactly [-lit, lit] and needs no range of its own to be worked out.
    let cases: &[(&str, &str, &str, &str)] = &[
        ("mass[mg]", "0mg", "1000mg", "500mg"),
        ("mass[g]", "0g", "1000g", "500g"),
        ("mass[kg]", "0kg", "1000kg", "500kg"),
        ("mass[t]", "0t", "1000t", "500t"),
        ("mass[lb]", "0lb", "1000lb", "500lb"),
        ("length[mm]", "0mm", "1000mm", "500mm"),
        ("length[cm]", "0cm", "1000cm", "500cm"),
        ("length[m]", "0m", "1000m", "500m"),
        ("length[km]", "0km", "1000km", "500km"),
        ("money[円, incl_tax]", "0円", "1000円", "500円"),
        ("money[銭, incl_tax]", "0銭", "1000銭", "500銭"),
        ("money[EUR, incl_tax]", "0EUR", "1000EUR", "500EUR"),
        ("money[USD, incl_tax]", "0USD", "1000USD", "500USD"),
        ("number", "0", "1000", "500"),
        ("rate", "0%", "100%", "50%"),
    ];
    for (ty, lo, hi, lit) in cases {
        let src = format!(
            "rule t(t) v1\n\ninputs\n  x(x) : {ty}  range >={lo} <={hi}\n\n\
             outputs\n  fee(fee) : money[円, incl_tax]  round down(1円)\n\n\
             derive gap(gap) : {ty} = x - {lit}  range >=-{lit} <={lit}\n\
             define big(big) : bool = x >= {lit}\n\n\
             table j(j)\npolicy first\n\
             | big | gap | -> fee(fee) : money[円, incl_tax] |\n\
             | true | >{lo} | 100円 |\n| - | - | 0円 |\n\nresult fee = fee\n"
        );
        let (f, c) = rulec::prepare(&src, "units.rule")
            .unwrap_or_else(|d| panic!("{ty}: 検査を通らない: {:?}", d.iter().map(|x| x.code.to_string()).collect::<Vec<_>>()));
        let vs = rulec::vectors::generate(&f, &c);
        assert!(!vs.is_empty(), "{ty}: ベクタが 0 件。式の中の `{lit}` が評価できていない");
        for v in &vs {
            for (name, val) in &v.outputs {
                assert!(val.is_some(), "{ty}: 出力 {name} に値が無い（表が一行も発火していない）");
            }
        }
        // The define's own boundary has to be reachable from both sides, or the rule is only
        // half exercised — which is how the 銭 case would still have slipped through.
        let hits = |want: &str| vs.iter().any(|v| v.trace.iter().any(|t| t.ends_with(want)));
        assert!(hits("1"), "{ty}: 1 行目に当たるベクタが無い");
        assert!(hits("2"), "{ty}: 2 行目に当たるベクタが無い");
    }
}

/// `money[銭]` specifically: the arm that used to be *wrong* rather than missing.
#[test]
fn 銭のリテラルは式の中でも銭のまま読まれる() {
    let src = "rule t(t) v1\n\ninputs\n  x(x) : money[銭, incl_tax]  range >=0銭 <=1000銭\n\n\
               outputs\n  fee(fee) : money[円, incl_tax]  round down(1円)\n\n\
               define big(big) : bool = x >= 500銭\n\n\
               table j(j)\npolicy first\n\
               | big | -> fee(fee) : money[円, incl_tax] |\n\
               | true | 100円 |\n| - | 0円 |\n\nresult fee = fee\n";
    let (f, c) = rulec::prepare(src, "units.rule").expect("検査を通る");
    let vs = rulec::vectors::generate(&f, &c);
    // 499銭 is below the threshold. Read as 5円 it was above it, and the oracle said 100円.
    for v in &vs {
        let Some(rulec::eval::Val::Num(x)) = v.input.get("x") else { continue };
        if x.cmp_to(rulec::num::Rat::int(499)) != std::cmp::Ordering::Greater {
            assert!(
                v.trace.iter().any(|t| t.ends_with('2')),
                "x={x}銭 は 500銭 未満なのに 1 行目に当たっている: {:?}",
                v.trace
            );
        }
    }
}

/// `number` has no unit, and a diagnostic must not invent one for it.
///
/// The region axis carried `unit: String`, where an empty one meant "this is a date" — so
/// `number`, the one numeric type with nothing to write after the digits, fell into the `_` arm
/// beside a rate and was given `%`. A witness on a count read `閾値 = 1%`, and worse, the row
/// `fix.text` offers ready to paste came out `| 1% | 0% | true |`, which is E103 in a `number`
/// column. The JSON `witness` was right the whole time, which is why it read as cosmetic.
#[test]
fn numberの診断は単位を付けない() {
    let src = "\
rule t(t) v1

inputs
  n(n) : number range >=0 <=10

outputs
  ok(ok) : bool

derive g(g) : number = n - n  range >=-10 <=10

table j(j)
policy unique
| n   | g | -> ok(ok) : bool |
| >=2 | - | true             |
| 0   | - | false            |
";
    let ds = rulec::check_source(src, "units.rule");
    let d = ds.iter().find(|d| d.code == "E101").expect("完全性の欠落が出る");

    for (name, v) in &d.witness.inputs {
        let shown = format!("{v:?}");
        assert!(!shown.contains('%'), "{name} の witness に % が付いている: {shown}");
    }
    let fix = d.fix.text.as_deref().expect("E101 は貼れる行を出す");
    assert!(!fix.contains('%'), "fix.text に % が付いている: {fix}");

    // The row it hands over has to parse in a `number` column. E103 there means it handed over
    // something the rule cannot hold.
    let patched = src.replace("| 0   | - | false            |\n", "| 0   | - | false            |\n| 1 | 0 | true |\n");
    assert!(patched != src, "貼り付け位置が見つからない");
    let after = rulec::check_source(&patched, "units.rule");
    assert!(
        !after.iter().any(|d| d.code == "E103"),
        "fix.text を貼ると単位の誤りになる: {:?}",
        after.iter().map(|d| d.code).collect::<Vec<_>>()
    );
}
