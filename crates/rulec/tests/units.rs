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
