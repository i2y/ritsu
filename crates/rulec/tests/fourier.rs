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
fn 起きない重なりは消え_起きる重なりは残る() {
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

/// Two boolean definitions off one input: the thresholds inside them come into the system
/// and settle the pair (§15.127).
#[test]
fn 真偽の定義の中の閾値も消去に入る() {
    let src = "rule t(t) v1\n\n\
        inputs\n  a(a) : number  range >=0 <=100\n\n\
        outputs\n  可否(v) : bool\n\n\
        define 大(big) : bool = a >= 90\n\
        define 小(small) : bool = a <= 10\n\n\
        table 表(t1)\npolicy unique\n\
        | 大    | 小    | -> 可否(v) : bool |\n\
        | true  | -     | true              |\n\
        | -     | true  | false             |\n\
        | false | false | false             |\n";
    let ds = codes(src);
    assert!(!ds.iter().any(|c| c == "W114"), "閾値が入るので決まるはず: {ds:?}");
    assert!(!ds.iter().any(|c| c == "E105"), "起きない重なりをエラーにしてはいけない: {ds:?}");

    // And the other way: two definitions that really can hold together are left alone.
    let both = src.replace("a <= 10", "a <= 95");
    let ds = codes(&both);
    assert!(
        ds.iter().any(|c| c == "W114" || c == "E105"),
        "本当に重なる対を黙って消してはいけない: {ds:?}"
    );
}

/// What the elimination cannot decide it does not claim: it works over the rationals, so a
/// pair kept apart only by the values being whole stays unconfirmed.
#[test]
fn 有理数で解く限界は主張しない() {
    let src = "rule t(t) v1\n\n\
        inputs\n  a(a) : money[円]  range >=0円 <=10万円\n\n\
        outputs\n  可否(v) : bool\n\n\
        derive 倍(d) : money[円] = a + a  range >=0円 <=20万円\n\n\
        define 上(up) : bool = 倍 >= 5円\n\
        define 下(dn) : bool = 倍 <= 5円\n\n\
        table 表(t1)\npolicy unique\n\
        | 上    | 下    | -> 可否(v) : bool |\n\
        | true  | -     | true              |\n\
        | -     | true  | false             |\n\
        | false | false | false             |\n";
    let ds = codes(src);
    assert!(ds.iter().any(|c| c == "W114"), "決められないものは W114 のまま: {ds:?}");
    assert!(!ds.iter().any(|c| c == "E105"), "{ds:?}");
    // On an even boundary there is an integer and the pair really can meet; the sieve
    // cannot build it either, so it stays a warning rather than becoming an error.
    let even = src.replace("5円", "6円");
    assert!(codes(&even).iter().any(|c| c == "W114" || c == "E105"), "偶数の境界では重なる");
}

/// A column of another type in the table no longer switches the elimination off. §15.126
/// gave up on the whole question as soon as two types met; each type now gets a system of
/// its own, and the one that holds the correlation still decides the pair (§15.139).
#[test]
fn 型の違う列があっても消去は効く() {
    let dated = |a: i64, b: i64| {
        rule(a, b)
            .replace("  割引B(d2) : money[円]  range >=0円 <=10万円\n", "  割引B(d2) : money[円]  range >=0円 <=10万円\n  注文日(day) : date  range >=2026-01-01 <=2026-12-31\n")
            .replace("| 残高A   | 残高B   | -> 可否(v) : 判定 |", "| 残高A   | 残高B   | 注文日 | -> 可否(v) : 判定 |")
            .replace(&format!("| <={a}円 | -       | 外                |"), &format!("| <={a}円 | -       | -      | 外                |"))
            .replace(&format!("| -       | >={b}円 | 内                |"), &format!("| -       | >={b}円 | -      | 内                |"))
            .replace(&format!("| >{a}円  | <{b}円  | 外                |"), &format!("| >{a}円  | <{b}円  | -      | 外                |"))
    };
    let ds = codes(&dated(1_000, 50_000));
    assert!(!ds.iter().any(|c| c == "W114" || c == "E105"), "起きない重なりが残っている: {ds:?}");
    // And the direction that must never flip: where the rows really meet, they still do.
    let ds = codes(&dated(50_000, 1_000));
    assert!(ds.iter().any(|c| c == "W114" || c == "E105"), "起きる重なりが黙って消えた: {ds:?}");
}

/// A chain of constraints through an input no table has a column for (§15.141). `a <= b` and
/// `b <= x` say `a <= x`, but only with `b` in the system: the sieve reads each constraint on
/// its own against the declared ranges, and reported a gap at `a = 51, x = 0` and an overlap
/// at the same place — inputs the door refuses. The gap and the overlap that do exist stay.
pub fn chained(rows: &str) -> String {
    format!(
        "rule 連なる制約(chain) v1\n\n\
         inputs\n  \
           a(a) : money[円]  range >=0円 <=100円\n  \
           b(b) : money[円]  range >=0円 <=100円\n  \
           x(x) : money[円]  range >=0円 <=100円\n\n\
         constraint a <= b\nconstraint b <= x\n\n\
         outputs\n  y(y) : bool\n\n\
         table 表(t)\npolicy unique\n| a | x | -> y |\n{rows}"
    )
}

#[test]
fn 列でない入力を経て連なる制約で_穴も重なりも決まる() {
    // Complete only with the chain: a > 50 forces x > 50.
    let gap = chained("| <=50円 | -      | true  |\n| >50円  | >50円  | false |\n");
    assert!(codes(&gap).is_empty(), "起きない穴を出した: {:?}", codes(&gap));
    // Unique only with the chain: rows 1 and 2 meet where a > 50 and x < 50.
    let overlap = chained("| >50円  | -      | true  |\n| -      | <50円  | false |\n| <=50円 | >=50円 | false |\n");
    assert!(codes(&overlap).is_empty(), "起きない重なりを出した: {:?}", codes(&overlap));
    // Take the chain's second link away and both are real.
    let open = |src: String| src.replace("constraint b <= x\n", "");
    assert!(codes(&open(gap)).contains(&"E101".to_string()), "起きる穴が黙って消えた");
    assert!(codes(&open(overlap)).contains(&"E105".to_string()), "起きる重なりが黙って消えた");
}
