//! The elimination that decides overlaps across correlated derivations (§15.126).
//!
//! One direction of this is dangerous and the other is not. Failing to prove that a pair
//! cannot overlap costs a W114 that was going to be printed anyway. **Proving it when the
//! pair really can overlap turns an error into silence**, which is the one outcome this tool
//! must never produce — so what is swept here is a grid of threshold pairs whose answer is
//! known in advance, and both halves of it are asserted.

fn codes(src: &str) -> Vec<String> {
    rulec::check_source(src, "inline.rule").iter().map(|d| d.code.to_string()).collect()
}

/// Two derived values off one input, with the thresholds as parameters.
///
/// `残高B = 合計 − 割引A − 割引B` and `割引B >= 0`, so `残高B <= 残高A` always. Row 1 takes
/// `残高A <= a` and row 2 takes `残高B >= b`, and the two can meet exactly when `b <= a`.
fn rule(a: i64, b: i64) -> String {
    format!(
        "rule t(t) v1\n\n\
         enum 判定(v) = 外(no) default | 内(yes)\n\n\
         inputs\n  \
           合計(total) : money[円]  range >=0円 <=100万円\n  \
           割引A(d1) : money[円]  range >=0円 <=10万円\n  \
           割引B(d2) : money[円]  range >=0円 <=10万円\n\n\
         outputs\n  可否(v) : 判定\n\n\
         derive 残高A(ra) : money[円] = 合計 - 割引A            range >=-10万円 <=100万円\n\
         derive 残高B(rb) : money[円] = 合計 - 割引A - 割引B    range >=-20万円 <=100万円\n\n\
         table 表(t1)\npolicy unique\n\
         | 残高A   | 残高B   | -> 可否(v) : 判定 |\n\
         | <={a}円 | -       | 外                |\n\
         | -       | >={b}円 | 内                |\n\
         | >{a}円  | <{b}円  | 外                |\n"
    )
}

#[test]
fn 起きない重なりは静かに落ち_起きる重なりは残る() {
    let mut decided = 0;
    let mut kept = 0;
    for a in [0i64, 1_000, 5_000, 50_000, 300_000] {
        for b in [0i64, 1_000, 5_000, 50_000, 300_000] {
            let ds = codes(&rule(a, b));
            let overlap = ds.iter().any(|c| c == "W114" || c == "E105");
            if b > a {
                // 残高B <= 残高A, so `残高A <= a` and `残高B >= b` cannot both hold.
                assert!(!overlap, "a={a} b={b}: 起きない重なりが残っている: {ds:?}");
                decided += 1;
            } else {
                // They can: `割引B = 0` makes 残高B = 残高A, and any value in [b, a] does it.
                assert!(overlap, "a={a} b={b}: 起きる重なりが黙って消えた: {ds:?}");
                kept += 1;
            }
            assert!(!ds.iter().any(|c| c == "E101"), "a={a} b={b}: 表は完全なはず: {ds:?}");
        }
    }
    assert!(decided >= 10 && kept >= 10, "両側とも実際に通っていること: {decided} / {kept}");
}

/// The elimination reads a `constraint` too, which is the other place a rule says a
/// combination cannot happen (§15.55) — and here it is the only thing that decides the pair.
#[test]
fn 制約も消去に入る() {
    let src = |k: &str| {
        format!(
            "rule t(t) v1\n\n\
             inputs\n  \
               a(a) : money[円]  range >=0円 <=100万円\n  \
               b(b) : money[円]  range >=0円 <=100万円\n\n\
             {k}\
             outputs\n  可否(v) : bool\n\n\
             derive p(p) : money[円] = a + a  range >=0円 <=200万円\n\
             derive q(q) : money[円] = b + b  range >=0円 <=200万円\n\n\
             table 表(t1)\npolicy unique\n\
             | p      | q     | -> 可否(v) : bool |\n\
             | >=10円 | -     | true              |\n\
             | -      | <=8円 | false             |\n\
             | <10円  | >8円  | false             |\n"
        )
    };
    // With `a <= b`: `p >= 10円` wants `a >= 5円` and `q <= 8円` wants `b <= 4円`, which the
    // constraint forbids. Nothing else in the rule does — the ranges allow both.
    let with = codes(&src("constraint a <= b\n\n"));
    assert!(!with.iter().any(|c| c == "W114" || c == "E105"), "制約が決めるはず: {with:?}");
    let without = codes(&src(""));
    assert!(
        without.iter().any(|c| c == "W114" || c == "E105"),
        "制約が無ければ重なりは残るはず（決めたのが制約だったことの裏）: {without:?}"
    );
}

/// A product of two names is not a linear form, and nothing is claimed about it.
#[test]
fn 非線形の導出には何も言わない() {
    let src = "rule t(t) v1\n\n\
        inputs\n  \
          a(a) : number  range >=0 <=100\n  \
          b(b) : number  range >=0 <=100\n\n\
        outputs\n  可否(v) : bool\n\n\
        define 大(big) : bool = a >= 90\n\
        define 小(small) : bool = a <= 10\n\n\
        table 表(t1)\npolicy unique\n\
        | 大    | 小    | -> 可否(v) : bool |\n\
        | true  | -     | true              |\n\
        | -     | true  | false             |\n\
        | false | false | false             |\n";
    // Two boolean definitions off one input: the elimination has no equation for a `define`
    // body, so the pair stays unconfirmed rather than being claimed either way.
    let ds = codes(src);
    assert!(ds.iter().any(|c| c == "W114"), "決められないものは W114 のまま: {ds:?}");
    assert!(!ds.iter().any(|c| c == "E105"), "{ds:?}");
}
