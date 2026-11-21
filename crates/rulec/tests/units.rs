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
    let g = rulec::codegen::Gen::new(&f, &c, &src, "");
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

/// Area, volume and time (§15.83). Each is a dimension of its own, converted at literal
/// position only, exactly as mass and length always were.
#[test]
fn 面積と体積と時間が書ける() {
    assert!(check(&qty("area", "m2", "1ha")).is_empty(), "1ha は 10000m2");
    assert!(check(&qty("area", "cm2", "1m2")).is_empty(), "1m2 は 10000cm2");
    assert!(check(&qty("area", "m2", "50a")).is_empty(), "50a は 5000m2");
    assert!(check(&qty("volume", "L", "5m3")).is_empty(), "5m3 は 5000L");
    assert!(check(&qty("volume", "m3", "5000L")).is_empty());
    assert!(check(&qty("volume", "mL", "1cm3")).is_empty(), "cm3 と mL は同じ大きさ");
    assert!(check(&qty("duration", "h", "120min")).is_empty(), "120min は 2h");
    assert!(check(&qty("duration", "min", "2h")).is_empty());
    assert!(check(&qty("duration", "s", "1d")).is_empty(), "1d は 86400s");
    // A 坪 is 400/121 m² on the nose — 一間 squared, and a 尺 is 10/33 m — so it is not a
    // whole number of square metres, exactly as a pound is not a whole number of grams.
    assert!(check(&qty("area", "m2", "1坪")).contains(&"E103".to_string()));
    assert!(check(&qty("area", "坪", "1坪")).is_empty());
    // Ninety minutes is not a whole number of hours.
    assert!(check(&qty("duration", "h", "90min")).contains(&"E103".to_string()));
    // A minute is `min`, because `m` is the metre. An area is not a length.
    assert!(check(&qty("area", "m", "10m")).contains(&"E103".to_string()));
    assert!(check(&qty("duration", "m", "10m")).contains(&"E103".to_string()));
}

/// §2.1 says a compound dimension is a modeling error, and until §15.83 only money × money
/// was stopped. `縦 × 横` came back as a length and was compared against a length threshold;
/// `重さ × 長さ` came back as a mass. Division was worse: the divisor is read at **its own**
/// unit, so `重さ(mass[g]) ÷ 2kg` divided by 2 rather than by 2000 and stayed a mass.
#[test]
fn 単位どうしの掛け算と割り算は止まる() {
    // `{lo}`/`{hi}` are the derived range: it has to be written in the type the `derive`
    // declares, so it cannot be fixed in the skeleton.
    let rule = |ty: &str, expr: &str, lo: &str, hi: &str| {
        format!(
            r#"rule t(t) v1

inputs
  a(a) : mass[g]    range >=0g <=1000g
  b(b) : length[m]  range >=0m <=1000m

outputs
  r(r) : bool

derive x(x) : {ty} = {expr}  range >={lo} <={hi}

table j(j)
policy unique
| x | -> r(r) : bool |
| - | true           |
"#
        )
    };
    let g = |ty: &str, expr: &str| rule(ty, expr, "0g", "1000000g");
    let e103 = |src: &str| check(src).contains(&"E103".to_string());
    // Two lengths, two masses, and one of each: a product of units, whichever way round.
    assert!(e103(&g("mass[g]", "a × a")));
    assert!(e103(&g("mass[g]", "a × b")));
    assert!(e103(&g("mass[g]", "b × a")));
    // A quotient of two different dimensions is the same error.
    assert!(e103(&g("mass[g]", "a ÷ 2m")));
    // The same dimension in another unit: a divisor is read at the unit it is written in and
    // never converted, so it has to be written in the left side's unit.
    assert!(e103(&g("mass[g]", "a ÷ 2kg")));
    // What stays legal: a dimensionless factor, and a divisor of the same unit. (`b` goes
    // unused here, so what is asserted is that no unit is being complained about.)
    assert!(!e103(&g("mass[g]", "a × 2")));
    assert!(!e103(&rule("number", "a ÷ 100g", "0", "10")));
}

/// Temperature and sound: ordered, not arithmetic (§15.84).
///
/// ℉ is the one unit in the table whose conversion is not a scaling — 41℉ is exactly 5℃ —
/// and it is only safe because E048 makes sure no difference of two temperatures is ever
/// formed. On a difference the offset would be wrong: a rise of 9℉ is a rise of 5℃, not of
/// −27.2℃.
#[test]
fn 温度と音量は比べられるが計算できない() {
    assert!(check(&qty("temperature", "℃", "40℃")).is_empty());
    assert!(check(&qty("sound", "dB", "130dB")).is_empty());
    // 41℉ is 5℃ on the nose, and −40 is the same number in both.
    assert!(check(&qty("temperature", "℃", "41℉")).is_empty(), "41℉ は 5℃");
    assert!(check(&qty("temperature", "℉", "5℃")).is_empty(), "5℃ は 41℉");
    // 42℉ is 50/9 ℃, which is not a whole number of ℃.
    assert!(check(&qty("temperature", "℃", "42℉")).contains(&"E103".to_string()));
    // A temperature is not a sound level, and neither is a length.
    assert!(check(&qty("temperature", "dB", "1dB")).contains(&"E103".to_string()));
    assert!(check(&qty("sound", "℃", "1℃")).contains(&"E103".to_string()));
}

/// The four arithmetic operators, over the three types that are ordered and nothing more.
/// `date` is the one that was always *described* this way — §2.1 says "comparison and range
/// only, no arithmetic" — and was never checked: `甲 - 乙` between two dates typed clean and
/// only E112 ever complained, about the range.
#[test]
fn 順序だけの型は演算に使えない() {
    let rule = |ty: &str, lo: &str, hi: &str, expr: &str| {
        format!(
            r#"rule t(t) v1

inputs
  a(a) : {ty}  range >={lo} <={hi}
  b(b) : {ty}  range >={lo} <={hi}

outputs
  r(r) : bool

derive x(x) : {ty} = {expr}  range >={lo} <={hi}

table j(j)
policy unique
| x | -> r(r) : bool |
| - | true           |
"#
        )
    };
    let cases = [
        ("temperature[℃]", "0℃", "40℃"),
        ("temperature[℉]", "0℉", "100℉"),
        ("sound[dB]", "0dB", "130dB"),
        ("date", "2020-01-01", "2030-12-31"),
    ];
    for (ty, lo, hi) in cases {
        for expr in ["a - b", "a + b", "a × 2", "a ÷ 2"] {
            let ds = check(&rule(ty, lo, hi, expr));
            assert!(ds.contains(&"E048".to_string()), "{ty}: `{expr}` が止まらない: {ds:?}");
        }
        // Comparison is what these types are for, and stays.
        let src = rule(ty, lo, hi, "a").replace("| x |", "| x |");
        assert!(!check(&src).contains(&"E048".to_string()), "{ty}: 名前を置くだけで止まってはいけない");
    }
}

/// `examples` is shaped like a table but is not an `Item`, so its cells went through no type
/// check at all. §1.2 calls it an executable specification and AGENTS §1 calls it the one
/// check that can catch what the evaluator and every generated language get wrong together —
/// and an expected `800kg` under a `money[円]` output compared equal to 800円.
#[test]
fn 例のセルは列の型に照らされる() {
    let rule = |cells: &str| {
        format!(
            r#"rule t(t) v1

inputs
  重さ(w) : mass[g]  range >=0g <=10000g
  区分(k) : 会員

outputs
  料金(fee) : money[円, incl_tax]  round down(1円)

enum 会員(member) = 一般(basic) default | 上級(gold) default

table j(judge)
policy first
| 重さ    | 区分 | -> 料金(fee) : money[円, incl_tax] |
| >=1000g | -    | 800円                              |
| -       | -    | 500円                              |

examples
| 重さ | 区分 | -> 料金 |
{cells}
"#
        )
    };
    let has = |cells: &str, code: &str| check(&rule(cells)).contains(&code.to_string());

    // A literal that does not land on a whole number of the declared unit. It used to
    // produce no value, and the only thing said was E107 "no value came out".
    assert!(has("| 1lb | 一般 | 500円 |", "E103"));
    // A bare number where a unit is required.
    assert!(has("| 500 | 一般 | 500円 |", "E103"));
    // A value the enum does not have. This one passed: it landed on the `-` row, and the
    // example proved nothing while looking like it had.
    assert!(has("| 500g | 幽霊 | 500円 |", "E012"));
    // The expected value, in the wrong unit and with no unit at all. Both compared equal.
    assert!(has("| 500g | 一般 | 800kg |", "E103"));
    assert!(has("| 500g | 一般 | 500 |", "E103"));
    // A column heading that names nothing was ignored outright.
    let bad_col = rule("| 500g | 一般 | 500円 |").replace("| 重さ | 区分 | -> 料金 |", "| 幻列 | 区分 | -> 料金 |");
    assert!(check(&bad_col).contains(&"E012".to_string()));
    // What a correct example does: nothing. (Both values are marked `default`, because
    // neither has a row of its own here — an example naming one is not a row, which is why
    // this pass puts `used` back when it is done.)
    assert!(check(&rule("| 500g | 一般 | 500円 |\n| 2kg  | 上級 | 800円 |")).is_empty());
}

/// The grid an output is rounded to is a value of that output's column — read through the
/// unit, not taken as the bare number written.
///
/// It was read with `lit_value_in` and, when that said no, simply **not recorded**: the
/// generator then fell back on a grid of one unit. So `round up(10)` — the most natural way
/// to mistype `round up(10円)` — rounded to 1円, and `round up(10銭)` did too, which is ten
/// times from either of the two readings a person could have meant (§15.86).
#[test]
fn 丸めの格子は単位ごと読まれる() {
    let rule = |grid: &str| {
        format!(
            r#"rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=1000g

outputs
  fee(fee) : money[円, incl_tax]  round up({grid})

table j(j)
policy unique
| w | -> base(base) : money[円, incl_tax] |
| - | 500円                               |

define fee(fee) : money[円, incl_tax] = base × 3.3%
"#
        )
    };
    // A hundredth of the currency is a unit of the same column, so a grid written in 銭 is
    // read as the yen it is worth. This is the half that matters: the fix is not "refuse
    // what is unfamiliar" but "read the unit".
    let by_sen = rule("1000銭");
    let by_yen = rule("10円");
    for src in [&by_sen, &by_yen] {
        let (f, c) = rulec::prepare(src, "units.rule").expect("1000銭 は 10円 として通る");
        let g = rulec::codegen::Gen::new(&f, &c, src, "");
        assert!(
            g.python().contains("_round_up(raw, 10000)"),
            "10円 の格子で丸めていない:\n{}",
            g.python()
        );
    }
    // And a grid that is no value of the column at all stops, rather than becoming one unit.
    for bad in ["10g", "10", "10%", "10USD"] {
        assert!(
            check(&rule(bad)).contains(&"E103".to_string()),
            "round up({bad}) が money[円] の出力で止まらない"
        );
    }
}

/// What a `fold` answers is the rule's output, and is read in the output's unit.
///
/// `empty -> 999銭` under a `money[円]` output passed `check` and generated **99900円** —
/// 9.99円 read as a count of 銭 and then scaled as if it were yen. The same literal in a
/// table cell of that column has always been E103 (§15.86).
#[test]
fn 畳み込みの答えは出力の単位で読まれる() {
    let rule = |empty: &str| {
        format!(
            r#"rule t(t) v1

enum 採用(verdict) = 取る(take) | 送る(skip)

elements 行(rows)
  額(amount) : money[円, incl_tax]  range >=0円 <=10000円

outputs
  合計(total) : money[円, incl_tax]  round up(1円)

table 行判定(row_of)
policy unique
| 額       | -> 採用(verdict) : 採用 |
| >=1000円 | 取る                    |
| <1000円  | 送る                    |

fold 採用 over 行
  送る      -> next
  取る      -> keep_max 額 by 額
  empty     -> {empty}
  exhausted -> held

sequence 空(none)
| 額 |
"#
        )
    };
    // 1000銭 is ten yen exactly, so it is a value of the column and is read as ten.
    let src = rule("1000銭");
    let (f, c) = rulec::prepare(&src, "units.rule").expect("1000銭 は 10円 として通る");
    let g = rulec::codegen::Gen::new(&f, &c, &src, "");
    assert!(
        g.python().contains("answer: int = 10"),
        "空のときの答えが 10円 になっていない:\n{}",
        g.python()
    );
    // 999銭 is 9.99円, which the column cannot hold — as in any of its cells.
    assert!(check(&rule("999銭")).contains(&"E103".to_string()));
    // Neither can a mass, nor a bare number.
    assert!(check(&rule("0g")).contains(&"E103".to_string()));
    assert!(check(&rule("0")).contains(&"E103".to_string()));
}

#[test]
fn 知らない単位はその場で指摘される() {
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
        ("area[m2]", "0m2", "1000m2", "500m2"),
        ("area[坪]", "0坪", "1000坪", "500坪"),
        ("volume[L]", "0L", "1000L", "500L"),
        ("volume[m3]", "0m3", "1000m3", "500m3"),
        ("duration[min]", "0min", "1000min", "500min"),
        ("duration[h]", "0h", "1000h", "500h"),
        ("number", "0", "1000", "500"),
        ("rate[step 1%]", "0%", "100%", "50%"),
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

/// A table's rate column is bounded by its cells, not by an assumed 0..100% (§15.34). With
/// the assumption, 9 × 10¹⁸ yen times a column holding 200% "fitted" in int64 and E108 stayed
/// silent; with the cells, it does not fit and E108 says so. The same amount times a column
/// holding only 100% does fit, so the input alone is not what trips it.
#[test]
fn 率の列の範囲はセルから決まる() {
    let rule = |top: &str| {
        format!(
            "rule t(t) v1\n\ninputs\n  x(x) : money[円, incl_tax]  range >=0円 <=9_000_000_000_000_000_000円\n  k(k) : bool\n\n\
             outputs\n  y(y) : number  round down(1)\n\n\
             table j(j)\npolicy unique\n| k | -> r(r) : rate[step 100%] |\n| true | {top} |\n| false | 100% |\n\n\
             define y(y) : number = x ÷ 1円 × r\n"
        )
    };
    assert!(check(&rule("200%")).contains(&"E108".to_string()), "200% の列で 9×10¹⁸ 円が int64 に収まると言っている");
    assert!(!check(&rule("100%")).contains(&"E108".to_string()), "100% の列では収まるはず");
}

/// E108 is stated over the `result` expression too. The proof used to be called on the two
/// `Item`s alone — `define` and `derive` — and `result` is neither, so a product assembled
/// straight into the first output was never held to int64. The corpus could not notice:
/// its widest proven interval is seven orders of magnitude below i64::MAX. Written as a
/// definition the same product tripped E108; written as a result it checked `ok`, and the
/// generated Rust, Go, Wasm and SQL then disagreed with the reference evaluator on the
/// vectors, which reach both maxima.
#[test]
fn result_式も_int64_に収まることを証明する() {
    let rule = |tail: &str| {
        format!(
            "rule t(t) v1\n\ninputs\n  x(x) : money[円, incl_tax]  range >=0円 <=1_000_000_000_000円\n  \
             k(k) : number  range >=0 <=100_000_000\n\n\
             outputs\n  y(y) : money[円, incl_tax]  round down(1円)\n\n{tail}"
        )
    };
    let 定義ごし = rule("define p(p) : money[円, incl_tax] = x × k\n\nresult y = p\n");
    let 直に = rule("result y = x × k\n");
    assert!(check(&定義ごし).contains(&"E108".to_string()), "define では出ている");
    assert!(check(&直に).contains(&"E108".to_string()), "result でも出なければならない");
    // The same shape one order of magnitude down fits, so it is the magnitude that trips it
    // and not the shape of `result` itself.
    let 収まる = rule("result y = x × 1\n");
    assert!(!check(&収まる).contains(&"E108".to_string()), "収まる積で出てはいけない");
}

/// E049: a figure written the way a document writes it. `,` separates the members of a set,
/// so `<=1,000` used to pass as `<=1` in a column of numbers, and in a column of money it
/// stopped at an E103 about `1` having no unit.
#[test]
fn 桁区切りのカンマは書き直した形で止まる() {
    for (line, fixed) in [
        ("| <=1,000 | true |", "1000"),
        ("| >1,000円 | true |", "1000円"),
        ("  a(a) : money[円]  range >=0円 <=1,949,000円", "1949000円"),
        ("| >=-1,000円 | true |", "-1000円"),
        ("| 12,345.5円 | true |", "12345.5円"),
        ("| 1，000円 | true |", "1000円"),
    ] {
        let e = rulec::lex::lex_line(1, line).expect_err(line);
        assert_eq!(e.code, "E049", "{line}");
        assert_eq!(e.fix.text.as_deref(), Some(fixed), "{line}");
    }
    // A set, groups that are not three digits, a digit separator, a string and a comment are
    // what they were.
    for line in ["| 100, 200 | true |", "| 1,2,3 | true |", "| 1,0000 | true |", "| 1234,567 | true |", "| 1_000円 | true |", "  d \"1,000\"", "| 1 | true |  # 1,000円"] {
        assert!(rulec::lex::lex_line(1, line).is_ok(), "{line}");
    }
}

/// A rate is held as a fraction and written in percent. E104 used to write 12% as "0.12%",
/// call a table of whole percents "an unrounded value" and round 0.12% for its example.
#[test]
fn 率の出力のe104は百分率で言う() {
    let table = "rule r(r) v1\n\nenum 種類(kind) = 甲(a) | 乙(b)\n\ninputs\n  種類(kind) : 種類\n\n\
                 outputs\n  率(rate) : rate[step 0.1%]\n\n\
                 table 率表(t)\npolicy unique\n| 種類 | -> 率 : rate[step 0.1%] |\n| 甲 | 0.5% |\n| 乙 | 1.2% |\n";
    let ds = rulec::check_source(table, "rate.rule");
    let d = ds.iter().find(|d| d.code == "E104").expect("E104 が出る");
    let notes = d.notes.join(" ");
    assert!(!d.title.contains("unrounded") && !d.title.contains("丸めていない"), "刻みに載った率を端数と言っている: {}", d.title);
    // The column is stored in tenths of a percent (0.5% and 1.2% share that grid), and the
    // hint names that grid rather than ten of a unit.
    assert!(notes.contains("(0.1%)"), "ヒントは率の刻みで言う: {notes}");
    let ratio = "rule r(r) v1\n\ninputs\n  a(a) : money[円]  range >=1円 <=1000円\n  b(b) : money[円]  range >=1000円 <=3000円\n\n\
                 outputs\n  率(ratio) : rate\n\nresult 率 = a / b\n";
    let ds = rulec::check_source(ratio, "ratio.rule");
    let d = ds.iter().find(|d| d.code == "E104").expect("E104 が出る");
    let notes = d.notes.join(" ");
    // 500円 / 1997円 is 25.03…%: the example says so in percent, and rounds it as that.
    assert!(notes.contains("gives 25%") || notes.contains("なら 25%"), "{notes}");
}

/// The sentence that lists the units when a literal carries one that is not a unit names every
/// spelling of the table, which is ritsu's (`ritsu_units::table`) since the table moved there,
/// but for the hundredths of the codes, which it gives as the rule. Both languages spell the units
/// the same, so the language the test runs in does not matter.
#[test]
fn 単位を挙げる文は表のすべての綴りを挙げる() {
    let said = rulec::types::units();
    let words: Vec<&str> = said.split([' ', '、', '（', '）', ',', '(', ')']).collect();
    for s in ritsu_units::table::spellings() {
        if s != "USDc" && ritsu_units::table::CURRENCIES.iter().any(|c| format!("{c}c") == s) {
            continue;
        }
        assert!(words.contains(&s.as_str()), "`{s}` is not named in: {said}");
    }
    assert!(said.contains("ISO 4217"), "{said}");
}

/// The reference names the currencies of the table, in the row of money, as the message does: a
/// reader can tell a code the table does not have from a typo. `JPY` is another spelling of `円`,
/// so it has no hundredth of its own to write with a `c`.
#[test]
fn 参照は表の通貨を挙げ円の百分の一を銭と言う() {
    let reference = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/reference.md")).unwrap();
    let row = reference.lines().find(|l| l.starts_with("| money |")).expect("the row of money");
    let named: Vec<&str> = row.split('`').filter(|w| w.len() == 3 && *w != "JPY" && w.chars().all(|c| c.is_ascii_uppercase())).collect();
    assert_eq!(named, ritsu_units::table::CURRENCIES, "{row}");
    assert!(ritsu_units::table::money("JPYc").is_none() && row.contains("`銭`") && row.contains("`JPYc`"), "{row}");
}

/// A value that is no unit of the table is told what the table has, in both languages, and the
/// example of the rounding is in the column's own unit, not in yen. A type of no unit is told so
/// where it is declared, and the values written in it are not read (§15.321).
#[test]
fn 単位でない通貨の注は表の通貨を言い丸めの例は列の単位で書く() {
    let rule = |unit: &str, grid: &str| format!("rule t(t) v1\n\ninputs\n  a(a) : number  range >=0 <=9\n\noutputs\n  b(b) : {unit}  round down({grid})\n\ntable j(j)\npolicy unique\n| a | -> b(b) : {unit} |\n| - | 5{grid} |\n");
    for (lang, hundredth, example) in [(rulec::i18n::Lang::Ja, "銭（円の 1/100）", "`mass[g]` なら `round down(1g)`"), (rulec::i18n::Lang::En, "銭 (a hundredth of 円)", "`mass[g]` takes `round down(1g)`")] {
        let notes = |unit: &str, grid: &str| {
            rulec::i18n::with(lang, || rulec::check_source(&rule(unit, grid), "t.rule"))
                .iter()
                .filter(|d| d.code == "E103")
                .flat_map(|d| std::iter::once(d.title.clone()).chain(d.notes.clone()))
                .collect::<Vec<_>>()
                .join(" ")
        };
        for unknown in ["JPYc", "RUB"] {
            let said = notes(&format!("money[{unknown}]"), &format!("1{unknown}"));
            assert!(said.contains(&format!("`{unknown}`")) && said.contains(hundredth) && said.contains("USD EUR GBP"), "{said}");
            assert!(!said.split([' ', '（', '(']).any(|w| w == unknown), "`{unknown}` is among the units: {said}");
            assert!(!said.contains("round down("), "the note gives an example in `{unknown}`: {said}");
        }
        let said = notes("mass[g]", "1円");
        assert!(said.contains(example), "{said}");
    }
}

/// A type is held to the table of units where it is declared, with no literal written in its unit,
/// and what its brackets hold past what it reads is said, not dropped (§15.280). Every unit the
/// table has stays a type of its own dimension.
#[test]
fn 型の括弧は数を書かなくても単位の表で読む() {
    let rule = |ty: &str| {
        format!(
            "rule t(t) v1\n\nenum tier = gold | silver\n\ninputs\n  a(a) : {ty}  contract_only\n  n(n) : number  range >=0 <=9\n\noutputs\n  b(b) : number  round down(1)\n\ntable j(j)\npolicy unique\n| n | -> b(b) : number |\n| - | 1 |\n"
        )
    };
    let said = |ty: &str| -> Vec<String> {
        rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(&rule(ty), "t.rule")).iter().filter(|d| d.code == "E103").map(|d| d.title.clone()).collect()
    };
    for (ty, title) in [
        ("money[RUB]", "`RUB` is not a unit of money"),
        ("money[JPYc]", "`JPYc` is not a unit of money"),
        ("money[usd]", "`usd` is not a unit of money"),
        ("money[kg]", "`kg` is not a unit of money"),
        ("money[incl_tax]", "`incl_tax` is not a unit of money"),
        ("money[RUB, incl_tax]", "`RUB` is not a unit of money"),
        ("money[RUB]?", "`RUB` is not a unit of money"),
        ("mass[JPY]", "`JPY` is not a unit of mass"),
        ("mass[cm]", "`cm` is not a unit of mass"),
        ("mass[lbs]", "`lbs` is not a unit of mass"),
        ("temperature[C]", "`C` is not a unit of temperature"),
        ("money", "`money` needs a unit"),
        ("mass", "`mass` needs a unit"),
        ("sound?", "`sound` needs a unit"),
        ("money[USD, incl_tax, excl_tax]", "`money` takes no `excl_tax` in its brackets"),
        ("money[USD, step 1USD]", "`money` takes no `step 1USD` in its brackets"),
        ("mass[g, kg]", "`mass` takes no `kg` in its brackets"),
        ("mass[g, step 1g]", "`mass` takes no `step 1g` in its brackets"),
        ("rate[step 1%, step 2%]", "`rate` takes no `step 2%` in its brackets"),
        ("number[kg]", "`number` takes no `kg` in its brackets"),
        ("number[step 1]", "`number` takes no `step 1` in its brackets"),
        ("bool[x]", "`bool` takes no `x` in its brackets"),
        ("date[x]", "`date` takes no `x` in its brackets"),
        ("string[x]", "`string` takes no `x` in its brackets"),
        ("tier[x]", "`tier` takes no `x` in its brackets"),
    ] {
        let all = said(ty);
        assert!(all.iter().any(|t| t == title), "{ty}: {all:?}");
    }
    let mut good: Vec<String> = ["rate[step 1%]", "rate[step 0.1%]", "money[USD, incl_tax]", "money[JPY, excl_tax]", "money[銭, incl_tax]", "number", "bool", "date", "string", "tier", "tier?"].map(String::from).to_vec();
    for s in ritsu_units::table::spellings() {
        let (dim, _) = ritsu_units::table::unit(&s).unwrap();
        if dim != ritsu_units::Dim::Rate {
            good.push(format!("{}[{s}]", dim.word()));
        }
    }
    assert!(good.len() > 100, "{}", good.len());
    for ty in &good {
        assert_eq!(said(ty), Vec::<String>::new(), "{ty}");
    }

    // what is said, where a type is written anywhere in the rule
    let everywhere = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=9\n\nelements xs(xs)\n  k(k) : money[AAA]  range >=0USD <=9USD\n\noutputs\n  b(b) : money[BBB]  round down(1USD)\n\ndefine d(d) : money[CCC] = 1USD\n\ntable j(j)\npolicy unique\n| n | -> b(b) : money[DDD] |\n| - | 1USD |\n";
    let titles = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(everywhere, "t.rule")).iter().map(|d| d.title.clone()).collect::<Vec<_>>().join("\n");
    for u in ["AAA", "BBB", "CCC", "DDD"] {
        assert!(titles.contains(&format!("`{u}` is not a unit of money")), "{u}: {titles}");
    }

    // what is said to the reader, in both languages
    let notes = |lang, ty: &str| rulec::i18n::with(lang, || rulec::check_source(&rule(ty), "t.rule")).iter().filter(|d| d.code == "E103").flat_map(|d| d.notes.clone()).collect::<Vec<_>>().join(" ");
    assert!(notes(rulec::i18n::Lang::En, "money[RUB]").contains("The units of money are 円 (yen, the same as JPY) 銭 (a hundredth of 円), or one of the ISO 4217 codes USD EUR"), "{}", notes(rulec::i18n::Lang::En, "money[RUB]"));
    assert!(notes(rulec::i18n::Lang::Ja, "money[RUB]").contains("金額の単位は 円（JPY と同じ）"), "{}", notes(rulec::i18n::Lang::Ja, "money[RUB]"));
    assert!(notes(rulec::i18n::Lang::En, "mass[foo]").contains("The units of mass are mg g kg t oz lb."), "{}", notes(rulec::i18n::Lang::En, "mass[foo]"));
    assert!(notes(rulec::i18n::Lang::Ja, "mass[foo]").contains("mass の単位は mg g kg t oz lb です。"), "{}", notes(rulec::i18n::Lang::Ja, "mass[foo]"));
    assert!(notes(rulec::i18n::Lang::En, "mass[JPY]").contains("`JPY` is a unit of money (`money[JPY]`)."), "{}", notes(rulec::i18n::Lang::En, "mass[JPY]"));
    assert!(notes(rulec::i18n::Lang::En, "money[incl_tax]").contains("`incl_tax` is a tax, and comes after the currency (`money[USD, incl_tax]`)."), "{}", notes(rulec::i18n::Lang::En, "money[incl_tax]"));
    assert!(notes(rulec::i18n::Lang::Ja, "number[kg]").contains("`number` に単位はありません"), "{}", notes(rulec::i18n::Lang::Ja, "number[kg]"));
}

/// Every number a rule of the corpus declares is a unit of ritsu's table (ritsu's DESIGN 5.2),
/// and spelled back it is the type the certificate prints; a rate carries the step its integer
/// counts on the wire.
#[test]
fn コーパスのどの数の型も単位の表で書ける() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut seen = 0;
    let mut rules: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "rule")).collect();
    rules.sort();
    for p in rules {
        let src = std::fs::read_to_string(&p).unwrap();
        let path = p.to_string_lossy().to_string();
        let Ok((_, c)) = rulec::prepare(&src, &path) else { continue };
        for (name, sym) in &c.syms {
            if !sym.ty.is_numeric() {
                continue;
            }
            let step = matches!(sym.ty, rulec::types::Ty::Rate).then(|| rulec::num::Rat::new(1, c.wire_scale(name)));
            let u = sym.ty.unit(step).unwrap_or_else(|| panic!("{path}: {name} : {} is not in the table", sym.ty));
            match sym.ty {
                rulec::types::Ty::Rate => assert_eq!(u.step, step, "{path}: {name}"),
                _ => assert_eq!(u.to_string(), sym.ty.to_string(), "{path}: {name}"),
            }
            seen += 1;
        }
    }
    assert!(seen > 150, "{seen} numbers");
}

/// The tax of a money type is `incl_tax` or `excl_tax`, or not written at all (§15.169). Any other
/// word passed in silence, as a tax of its own that no table knows. It is said where the type is
/// first written, and the note names the other lines that write the type, an output's, a table's
/// column's and a definition's alike (§15.321).
#[test]
fn 税区分はincl_taxかexcl_taxだけ() {
    let rule = |tax: &str| {
        let t = if tax.is_empty() { "money[円]".to_string() } else { format!("money[円, {tax}]") };
        format!(
            "rule t(t) v1\n\ninputs\n  p(p) : {t}  range >=0円 <=1万円\n\n\
             outputs\n  o(o) : {t}  round down(1円)\n\n\
             define 差額(gap) : {t} = p - 100円\n\n\
             table j(j)\npolicy unique\n| 差額 | -> o(o) : {t} |\n| <5000円 | 100円 |\n| >=5000円 | 0円 |\n"
        )
    };
    for ok in ["incl_tax", "excl_tax", ""] {
        assert!(check(&rule(ok)).is_empty(), "{ok}: {:?}", check(&rule(ok)));
    }
    let said: Vec<(String, String, usize, Vec<String>)> = rulec::i18n::with(rulec::i18n::Lang::Ja, || {
        rulec::check_source(&rule("foo"), "units.rule")
            .iter()
            .map(|d| (d.code.to_string(), d.title.clone(), d.marks.first().map(|m| m.span.line).unwrap_or(0), d.notes.clone()))
            .collect()
    });
    assert_eq!(said.iter().map(|(c, t, l, _)| (c.as_str(), t.as_str(), *l)).collect::<Vec<_>>(), [("E103", "税区分 `foo` は incl_tax でも excl_tax でもありません", 4)]);
    assert!(said[0].3.iter().any(|n| n == "`money[円, foo]` は 7、9、13 行目にも書かれています。"), "{:?}", said[0].3);
    let en: Vec<rulec::diag::Diag> = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(&rule("税込"), "units.rule"));
    assert!(en.len() == 1 && en[0].title == "The tax `税込` is neither incl_tax nor excl_tax", "{:?}", en.iter().map(|d| &d.title).collect::<Vec<_>>());
    assert!(en[0].notes.iter().any(|n| n == "`money[円, 税込]` is written on lines 7, 9 and 13 too."), "{:?}", en[0].notes);
}

// ── Two spellings of one unit (§15.332) ──────────────────────────────────────────────────────────

/// A rule whose output is `money[円]` and whose table and examples write it `money[JPY]`.
const HEADING: &str = "rule heading v1\n\ninputs\n  price : money[円]  range >=0円 <=10_000円\n\noutputs\n  fee : money[円]  round down(1円)\n\ntable fee\n| price     | -> fee : money[JPY] |\n| <1_000円  | 100JPY              |\n| >=1_000円 | 0JPY                |\n\nexamples\n| price    | -> fee : money[JPY] |\n| 999円    | 100JPY              |\n| 1_000円  | 0JPY                |\n";

/// `JPY` is another spelling of `円`, `mL` of `cm3` and `kL` of `m3`, and two types are compared as
/// the units they are, not as they are spelled (§15.332). The heading of a table and of the
/// examples, the two sides of a sum or a `max`, a divisor and the answer of a fold read the
/// spelling as the unit: a rule that wrote `円` beside `JPY` mixed units (E103), had a compound
/// dimension, or gave its fold an answer of another type (E021). A hundredth is a unit of its own,
/// as a gram is beside a kilogram and a tax beside the other.
#[test]
fn 一つの単位の二つの綴りは同じ単位() {
    let passes = |what: &str, src: &str| assert!(check(src).is_empty(), "{what}: {:?}", check(src));
    passes("見出しの型", HEADING);
    passes(
        "セルの名前",
        "rule assign v1\n\ninputs\n  price : money[JPY]  range >=0JPY <=10_000JPY\n\noutputs\n  fee : money[円]  round down(1円)\n\ntable fee\n| price     | -> fee |\n| <1000JPY  | price  |\n| >=1000JPY | 0円    |\n",
    );
    passes(
        "引き算",
        "rule volume v1\n\ninputs\n  capacity : volume[mL]  range >=0mL <=2_000mL\n\noutputs\n  left : volume[cm3]  round down(1cm3)\n\ndefine after_cup : volume[cm3] = capacity - 200cm3\n\ntable left\n| after_cup  | -> left    |\n| <0cm3      | 0cm3       |\n| >=0cm3     | after_cup  |\n",
    );
    passes(
        "割り算",
        "rule grid v1\n\ninputs\n  price : money[円]  range >=0円 <=10_000円\n\noutputs\n  points : number  round down(1)\n\ndefine per : number = down(price / 100JPY, 1)\n\ntable points\n| -> points |\n| per       |\n",
    );
    passes(
        "max",
        "rule larger v1\n\ninputs\n  price  : money[JPY]  range >=0JPY <=10_000JPY\n  budget : money[円]   range >=0円 <=10_000円\n\noutputs\n  larger : money[円]  round down(1円)\n\ndefine top : money[円] = max(price, budget)\n\ntable larger\n| -> larger |\n| top       |\n",
    );
    passes(
        "畳み込みの答え",
        "rule first_price v1\n\nenum mark = skip | pick\n\nelements lines\n  price   : money[JPY]  range >=0JPY <=10_000JPY\n  flagged : bool\n\noutputs\n  chosen : money[円]  round down(1円)\n\ntable verdict_by_flag\npolicy unique\n| flagged | -> verdict : mark |\n| true    | pick              |\n| false   | skip              |\n\nfold verdict over lines -> chosen : money[JPY]\n  pick      -> take_first price\n  skip      -> next\n  empty     -> 0円\n  exhausted -> held\n",
    );
    // other units, and the other tax, stay other types
    for (written, code) in [("money[USD]", "E103"), ("money[銭]", "E103"), ("money[円, incl_tax]", "E103")] {
        let src = HEADING.replace("-> fee : money[JPY] |", &format!("-> fee : {written} |"));
        assert!(check(&src).contains(&code.to_string()), "{written}: {:?}", check(&src));
    }
    let kg = "rule heavy v1\n\ninputs\n  w : mass[g]  range >=0g <=9_000g\n\noutputs\n  left : mass[g]  round down(1g)\n\ndefine limit : mass[kg] = 1kg\ndefine rest : mass[g] = w - limit\n\ntable left\n| -> left |\n| rest    |\n";
    assert!(check(kg).contains(&"E103".to_string()), "{:?}", check(kg));
    // a hundredth of the yen is of the yen's dimension: the divisor is in another unit of it, not a
    // compound dimension
    let sen = "rule sen v1\n\ninputs\n  price : money[円]  range >=0円 <=10_000円\n\noutputs\n  points : number  round down(1)\n\ndefine per : number = down(price / 10銭, 1)\n\ntable points\n| -> points |\n| per       |\n";
    let said = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(sen, "units.rule"));
    assert_eq!(said.iter().map(|d| (d.code, d.title.as_str())).collect::<Vec<_>>(), [("E103", "The divisor's unit does not match: money[円] divided by money[銭]")]);
}

/// The linear model reads two spellings of one unit as one unit too (§15.332): `excess` is
/// `money[円]`, computed from `added` in `JPY` and `limit` in `円`, and the row that asks
/// `excess >= 1円` with `added <= 1JPY` is reached by no input, as it is when the three are
/// spelled alike. The equation is one fact of one system, and the sentence says it as it says the
/// rule written in one spelling, each value in the spelling its name was declared in. So is what
/// a computed value comes to while a row of a table above fires (E102's third form, §15.195).
#[test]
fn 線形の式も綴りの違う一つの単位を一つの系で読む() {
    let same = "rule excess_flag v1\n\ninputs\n  added : money[JPY]  range >=0JPY <=10_000JPY\n  limit : money[JPY]  range >=1JPY <=5_000JPY\n\noutputs\n  flag : bool\n\ndefine excess : money[JPY] = added - limit\n\ntable flag\n| excess  | added   | -> flag |\n| >=1JPY  | <=1JPY  | true    |\n| >=1JPY  | >1JPY   | true    |\n| <1JPY   | -       | false   |\n";
    let mixed = same
        .replace("limit : money[JPY]  range >=1JPY <=5_000JPY", "limit : money[円]  range >=1円 <=5_000円")
        .replace("define excess : money[JPY]", "define excess : money[円]")
        .replace("| >=1JPY  |", "| >=1円   |")
        .replace("| <1JPY   |", "| <1円    |");
    assert_eq!(mixed.matches('円').count(), 7, "{mixed}");
    for (src, unit) in [(same, "JPY"), (mixed.as_str(), "円")] {
        let said = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(src, "units.rule"));
        assert_eq!(said.iter().map(|d| d.code).collect::<Vec<_>>(), ["E102"], "{src}");
        let notes = &said[0].notes;
        assert!(notes.iter().any(|n| n == "`excess = added - limit` holds for every input, and with it no values meet all of this row's conditions."), "{unit}: {notes:?}");
        assert!(notes.iter().any(|n| n == &format!("Over the ranges of the inputs, `excess` can only come to >=-5000{unit} <=9999{unit}.")), "{unit}: {notes:?}");
    }
    // the same while a row of a table above fires: `net` is `money[円]` beside `subtotal` in `JPY`,
    // which the table reads first
    let same = "rule tiered_flag v1\n\ninputs\n  subtotal : money[JPY]  range >=0JPY <=10_000JPY\n  credit   : money[JPY]  range >=0JPY <=1_000JPY\n\noutputs\n  flag : bool\n\nenum band = low | high\n\ntable band_of\npolicy unique\n| subtotal    | -> band : band |\n| <5_000JPY   | low            |\n| >=5_000JPY  | high           |\n\ndefine net : money[JPY] = subtotal - credit\n\ntable flag\n| band | subtotal | net         | -> flag |\n| low  | -        | >=5_000JPY  | true    |\n| low  | -        | <5_000JPY   | false   |\n| high | -        | -           | true    |\n";
    let mixed = same
        .replace("credit   : money[JPY]  range >=0JPY <=1_000JPY", "credit   : money[円]   range >=0円 <=1_000円")
        .replace("define net : money[JPY]", "define net : money[円]")
        .replace("| >=5_000JPY  | true    |", "| >=5_000円   | true    |")
        .replace("| <5_000JPY   | false   |", "| <5_000円    | false   |");
    assert_eq!(mixed.matches('円').count(), 6, "{mixed}");
    for (src, unit) in [(same, "JPY"), (mixed.as_str(), "円")] {
        let said = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(src, "units.rule"));
        assert_eq!(said.iter().map(|d| d.code).collect::<Vec<_>>(), ["E102"], "{src}");
        let want = format!("`band` is low only where row 1 of table band_of (subtotal < 5_000JPY) fires. There `net` can only come to >=-1000{unit} <=4999{unit}, and none of those values meets this row's conditions.");
        assert!(said[0].notes.iter().any(|n| *n == want), "{unit}: {:?}", said[0].notes);
    }
}

fn materials() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/unit_spellings")
}

/// `rulec <args>` from `dir`, and what it prints.
fn rulec_in(dir: &std::path::Path, args: &[&str]) -> (i32, String) {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

fn golden(name: &str, got: &str, failures: &mut Vec<String>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/unit_spellings").join(name);
    if let Err(e) = ritsu_testkit::golden::check(&path, got) {
        failures.push(e);
    }
}

/// The materials of `tests/unit_spellings`, English first with the Japanese version beside each
/// (§15.332). A rule that writes `JPY` applies a shared rule that writes `円`: it passes its subtotal
/// in, and adds the fee that comes out to it (`order_total`, and `注文の合計`, which writes the two the
/// other way round). Another binds its lines to the lines of a shared rule whose price is spelled
/// the other way (`order_lines`, `注文の明細`). A contribution declares the input of the rule it adds
/// to again, in the other spelling, and adds a fee of its spelling to the rule's
/// (`contrib/remote_island`, `寄与/離島`). Each was stopped by E042 (and the sum by E103), and passes
/// now, in both languages. Passed more than the shared rule takes, the binding stops at E043 as its
/// golden files hold, and a contribution whose range is another range stops at E042 however it is
/// spelled.
#[test]
fn 綴りの違う準用と寄与も通る() {
    let dir = materials();
    for f in ["order_total.rule", "注文の合計.rule", "order_lines.rule", "注文の明細.rule", "contrib/remote_island.rule", "寄与/離島.rule"] {
        for lang in ["en", "ja"] {
            let (code, out) = rulec_in(&dir, &["check", f, "--lang", lang]);
            assert_eq!(code, 0, "{f} ({lang}):\n{out}");
        }
    }
    let mut failures = Vec::new();
    for (f, name) in [("E043_order_total_wide.rule", "E043_order_total_wide"), ("E043_注文の合計_広い.rule", "E043_注文の合計_広い")] {
        for lang in ["en", "ja"] {
            let (code, out) = rulec_in(&dir, &["check", f, "--lang", lang]);
            assert_eq!(code, 1, "{f} ({lang}):\n{out}");
            golden(&format!("{name}.{lang}.txt"), &out, &mut failures);
        }
    }
    // a contribution's range is read by value: written another way it is the same range, and another
    // range is another, however its unit is spelled
    for (base, contrib, written, same, other) in [
        ("delivery_charge.rule", "contrib/remote_island.rule", "range >=0JPY <=100_000JPY", "range >=0円 <=100000JPY", "range >=0JPY <=50_000JPY"),
        ("配送料.rule", "寄与/離島.rule", "range >=0円 <=100_000円", "range >=0JPY <=10万円", "range >=0円 <=5万円"),
    ] {
        let t = ritsu_testkit::TempDir::new("unit-spellings-contrib");
        let lay = |range: &str| {
            std::fs::create_dir_all(t.path().join(contrib).parent().unwrap()).unwrap();
            std::fs::copy(dir.join(base), t.path().join(base)).unwrap();
            let body = std::fs::read_to_string(dir.join(contrib)).unwrap();
            assert_eq!(body.matches(written).count(), 1, "{contrib}");
            std::fs::write(t.path().join(contrib), body.replace(written, range)).unwrap();
        };
        lay(same);
        let (code, out) = rulec_in(t.path(), &["check", contrib]);
        assert_eq!(code, 0, "{contrib} with {same}:\n{out}");
        lay(other);
        for lang in ["en", "ja"] {
            let (code, out) = rulec_in(t.path(), &["check", contrib, "--lang", lang]);
            assert_eq!(code, 1, "{contrib} with {other} ({lang}):\n{out}");
            let stem = contrib.trim_end_matches(".rule").replace('/', "_");
            golden(&format!("E042_{stem}.{lang}.txt"), &out, &mut failures);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The certificate of a rule that writes one unit two ways passes both re-checkers (§15.332). For
/// each pair of spellings ritsu's table of units reads as one unit, a value in one spelling is added
/// to a value in the other, a table reads the sum through the linear model, and an output in the
/// other spelling answers it. The re-checkers compare units as `check` does from a list of their
/// own (`ONE_UNIT` in `tools/recheck.py`, `oneUnit` in ritsu's `proofs/RulecCert/Read.lean`), and a
/// pair the table gains that the lists lack fails here. The Lean one runs where `proofs/` is built.
#[test]
fn 綴りの違う一つの単位の証明書も二つの再検査を通る() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let python = || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success());
    if !ritsu_testkit::ready(ritsu_testkit::Need::Python, python, "python3 is not found") {
        return;
    }
    let all = ritsu_units::table::spellings();
    let pairs: Vec<(&String, &String)> = all.iter().enumerate().flat_map(|(i, a)| all[i + 1..].iter().filter(move |b| rulec::types::one_unit(a, b)).map(move |b| (a, b))).collect();
    // `cm3` and `mL`, `m3` and `kL`, `円` and `JPY` when this was written
    assert!(pairs.len() >= 3, "{pairs:?}");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let built = root.join("../../proofs/.lake/build/bin/rulec-recheck");
    let lean = ritsu_testkit::ready(ritsu_testkit::Need::Lean, || built.exists(), "ritsu's proofs/ is not built (lake build there makes rulec-recheck)").then_some(built);
    let recheck = root.join("tools/recheck.py");
    let t = ritsu_testkit::TempDir::new("unit-spellings-recheck");
    let mut failures = Vec::new();
    for (i, (a, b)) in pairs.into_iter().enumerate() {
        let k = ritsu_units::table::unit(a).map(|(d, _)| d.word().to_string()).unwrap();
        let file = format!("pair_{i}.rule");
        let rule = format!(
            "rule spellings v1\ndescription \"One unit written two ways: a value in {a} added to one in {b}, and the band it falls in\"\n\n\
             enum band = small | large\n\ninputs\n  first  : {k}[{a}]  range >=0{a} <=1000{a}\n  second : {k}[{b}]  range >=0{b} <=1000{b}\n\n\
             outputs\n  total : {k}[{b}]  round down(1{b})\n  size  : band\n\ndefine combined : {k}[{a}] = first + second\n\n\
             table size_of\npolicy first\n| combined  | -> size |\n| <=500{a} | small   |\n| -         | large   |\n\nresult total = combined\n"
        );
        std::fs::write(t.path().join(&file), &rule).unwrap();
        let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(t.path()).args(["certificate", &file]).output().expect("cannot start rulec");
        assert!(o.status.success(), "{a} and {b}: no certificate:\n{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let mut programs = vec![("tools/recheck.py", Command::new("python3"))];
        programs[0].1.arg(&recheck);
        if let Some(bin) = &lean {
            programs.push(("rulec-recheck", Command::new(bin)));
        }
        for (who, mut cmd) in programs {
            let mut p = cmd.current_dir(t.path()).args(["--rule", &file]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("cannot start a re-checker");
            p.stdin.as_mut().unwrap().write_all(&o.stdout).unwrap();
            let q = p.wait_with_output().unwrap();
            let said = String::from_utf8_lossy(&q.stdout).into_owned();
            if !q.status.success() || said.contains("FAILED") {
                failures.push(format!("{who}, {a} and {b}:\n{said}{}", String::from_utf8_lossy(&q.stderr)));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// E103 names the operation the expression was doing — a comparison says "comparing", not
/// "adding" — and its hint about dimensions is written only when the two are of different
/// dimensions: `mass[g]` and `mass[kg]` are one dimension in two spellings, and a number
/// against an enum carries no dimension at all (§15.350).
#[test]
fn 単位の混同は演算の言葉で言う() {
    use rulec::i18n::Lang::*;
    let rule = |expr: &str| {
        format!("rule t(t) v1\n\nenum tier = a | b\n\ninputs\n  g(g) : mass[g]  range >=0g <=2000g\n  k(k) : mass[kg]  range >=0kg <=2kg\n  n(n) : number  range >=0 <=9\n  t(t) : tier\n\noutputs\n  r(r) : bool\n\nderive z(z) : bool = {expr}\n\nresult r = z\n")
    };
    let e103 = |lang, expr: &str| {
        rulec::i18n::with(lang, || rulec::check_source(&rule(expr), "t.rule")).into_iter().find(|d| d.code == "E103").unwrap_or_else(|| panic!("{expr}: no E103"))
    };
    // The title names the operation.
    assert!(e103(En, "g < k").title.contains("comparing mass[g] and mass[kg]"), "{}", e103(En, "g < k").title);
    assert!(e103(En, "g = k").title.contains("comparing"), "{}", e103(En, "g = k").title);
    assert!(e103(En, "g + k < 0g").title.contains("adding mass[kg] to mass[g]"), "{}", e103(En, "g + k < 0g").title);
    assert!(e103(En, "g - k < 0g").title.contains("subtracting mass[kg] from mass[g]"), "{}", e103(En, "g - k < 0g").title);
    assert!(e103(Ja, "g < k").title.contains("比べています"), "{}", e103(Ja, "g < k").title);
    assert!(e103(Ja, "g - k < 0g").title.contains("引いています"), "{}", e103(Ja, "g - k < 0g").title);
    // The hint: different dimensions (money is a dimension of its own), same dimension in
    // another unit, and no dimension at all.
    let notes = e103(En, "g + k < 0g").notes.join(" ");
    assert!(notes.contains("one dimension") && !notes.contains("different dimensions"), "{notes}");
    let notes = e103(En, "g + n < 0g").notes.join(" ");
    assert!(notes.contains("different dimensions"), "{notes}");
    let notes = e103(En, "n < t").notes.join(" ");
    assert!(!notes.contains("dimension"), "{notes}");
}

/// An answer cell that writes one unit's other spelling — `800JPY` under `money[円]`,
/// `5mL` under `volume[cm3]` — is read back by its number by both re-checkers, not counted
/// among the literals they cannot convert (§15.351).
#[test]
fn 答えのセルの別の綴りは値で読み戻される() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let python = || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success());
    if !ritsu_testkit::ready(ritsu_testkit::Need::Python, python, "python3 is not found") {
        return;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let built = root.join("../../proofs/.lake/build/bin/rulec-recheck");
    let lean = ritsu_testkit::ready(ritsu_testkit::Need::Lean, || built.exists(), "ritsu's proofs/ is not built (lake build there makes rulec-recheck)").then_some(built);
    let recheck = root.join("tools/recheck.py");
    let t = ritsu_testkit::TempDir::new("answer-spellings-recheck");
    let file = "spellings.rule";
    let rule = "rule spellings v1\n\ninputs\n  n : number  range >=0 <=100\n\noutputs\n  fee : money[円]  round down(1円)\n  vol : volume[cm3]  round down(1cm3)\n\ntable t\npolicy first\n| n    | -> fee   | -> vol   |\n| <=50 | 800JPY   | 5mL      |\n| -    | 0円      | 0cm3     |\n";
    std::fs::write(t.path().join(file), rule).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(t.path()).args(["certificate", file]).output().expect("cannot start rulec");
    assert!(o.status.success(), "no certificate:\n{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    let mut programs = vec![("tools/recheck.py", Command::new("python3"))];
    programs[0].1.arg(&recheck);
    if let Some(bin) = &lean {
        programs.push(("rulec-recheck", Command::new(bin)));
    }
    for (who, mut cmd) in programs {
        let mut p = cmd.current_dir(t.path()).args(["--rule", file]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("cannot start a re-checker");
        p.stdin.as_mut().unwrap().write_all(&o.stdout).unwrap();
        let q = p.wait_with_output().unwrap();
        let said = String::from_utf8_lossy(&q.stdout).into_owned();
        assert!(q.status.success() && !said.contains("FAILED"), "{who}:\n{said}{}", String::from_utf8_lossy(&q.stderr));
        assert!(!said.contains("cannot read") && !said.contains("not pinned") && !said.contains("does not convert"), "{who} counted a same-unit spelling as unconvertible:\n{said}");
    }
}
