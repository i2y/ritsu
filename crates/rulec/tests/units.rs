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
        let g = rulec::codegen::Gen::new(&f, &c, src);
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
    let g = rulec::codegen::Gen::new(&f, &c, &src);
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
