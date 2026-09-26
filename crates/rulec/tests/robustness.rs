//! A broken rule is answered: never a crash, never a wait without end (§15.156).
//!
//! Before 1.0 the corpus was cut up at random and fed to `rulec check`, and three kinds of
//! input took the tool down: a malformed `derive` line was read again for ever, `÷ 0`
//! panicked the evaluator, and a `where` with nothing to compare panicked the parser. Each
//! was one line away from a rule someone could be in the middle of typing — in the
//! playground, the check runs on every pause. The same cutting is done here, the same way
//! every time: every declaration line of every rule in the corpus is cut short after each of
//! its words, deleted, doubled, and has each number in it turned to zero. The checker must
//! come back from every one, without a panic and in good time.
//!
//! What it says about them is not held here. The regressions below hold that for the cases
//! that were found, and the ledger's own examples hold it for every code.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Far above what any variant needs, even unoptimized: the slowest rule in the corpus checks
/// in a fraction of a second. A case past this is not slow, it is stuck.
const LIMIT: Duration = Duration::from_secs(60);

fn corpus() -> Vec<(String, String)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/corpus");
    let mut out: Vec<(String, String)> = std::fs::read_dir(dir)
        .expect("コーパスを読めない")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(&p).unwrap()))
        .collect();
    out.sort();
    out
}

/// Every variant of every rule, labelled with what was done to which line.
fn variants() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, src) in corpus() {
        let lines: Vec<&str> = src.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            // Table rows are many and alike; the declarations are where the parser branches.
            if t.is_empty() || t.starts_with('#') || t.starts_with('|') {
                continue;
            }
            let with = |replacement: Option<String>| -> String {
                let mut v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
                match replacement {
                    Some(r) => v[i] = r,
                    None => {
                        v.remove(i);
                    }
                }
                v.join("\n")
            };
            let indent = &line[..line.len() - line.trim_start().len()];
            let words: Vec<&str> = t.split_whitespace().collect();
            for k in 0..words.len() {
                out.push((format!("{name}:{} cut after {k} words", i + 1), with(Some(format!("{indent}{}", words[..k].join(" "))))));
            }
            out.push((format!("{name}:{} deleted", i + 1), with(None)));
            out.push((format!("{name}:{} doubled", i + 1), with(Some(format!("{line}\n{line}")))));
            // Each run of digits in turn becomes 0: a divisor, a rounding grid, a step, a bound.
            let bytes = line.as_bytes();
            let mut j = 0;
            while j < bytes.len() {
                if bytes[j].is_ascii_digit() {
                    let start = j;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    let zeroed = format!("{}0{}", &line[..start], &line[j..]);
                    out.push((format!("{name}:{} digits at {start} as 0", i + 1), with(Some(zeroed))));
                } else {
                    j += 1;
                }
            }
        }
    }
    out
}

/// Check every case on as many threads as the machine has, and watch them. A panic is
/// caught and reported with its case; a case that runs past `LIMIT` fails the test at once
/// — its thread cannot be stopped, so it is left behind rather than waited for.
fn check_all(cases: Vec<(String, String)>) -> Vec<String> {
    let cases = Arc::new(cases);
    let next = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(Mutex::new(Vec::new()));
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 8);
    let slots: Arc<Vec<Mutex<Option<(usize, Instant)>>>> = Arc::new((0..workers).map(|_| Mutex::new(None)).collect());
    for w in 0..workers {
        let (cases, next, done, failures, slots) = (cases.clone(), next.clone(), done.clone(), failures.clone(), slots.clone());
        std::thread::spawn(move || loop {
            let i = next.fetch_add(1, Ordering::SeqCst);
            let Some((label, src)) = cases.get(i) else { break };
            *slots[w].lock().unwrap() = Some((i, Instant::now()));
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rulec::report(src, "robust.rule")));
            if r.is_err() {
                failures.lock().unwrap().push(format!("{label}: panicked\n{src}"));
            }
            *slots[w].lock().unwrap() = None;
            done.fetch_add(1, Ordering::SeqCst);
        });
    }
    while done.load(Ordering::SeqCst) < cases.len() {
        for s in slots.iter() {
            if let Some((i, t)) = *s.lock().unwrap() {
                assert!(t.elapsed() < LIMIT, "{}: {:?} たっても答えない\n{}", cases[i].0, LIMIT, cases[i].1);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Arc::try_unwrap(failures).map(|m| m.into_inner().unwrap()).unwrap_or_default()
}

#[test]
fn 崩した規則にも_落ちずに時間内に答える() {
    let cases = variants();
    assert!(cases.len() > 5000, "崩し方が少なすぎる: {}", cases.len());
    let failures = check_all(cases);
    assert!(failures.is_empty(), "{} 件で落ちた。最初の一件:\n{}", failures.len(), failures[0]);
}

fn codes_of(src: &str) -> Vec<&'static str> {
    rulec::report(src, "t.rule").diags.iter().map(|d| d.code).collect()
}

const HEAD: &str = "rule t(t) v1\n\ninputs\n  x(x) : money[円]  range >=0円 <=100円\n\noutputs\n  y(y) : money[円]  round down(1円)\n\n";

/// The cases that were found, each held to the code that now names it. Every one of them
/// used to crash, hang, or pass with nothing said.
#[test]
fn 見つかった壊れ方は_それぞれのコードで止まる() {
    let cases: &[(&str, &str, &str)] = &[
        // Read again for ever.
        ("derive だけの行", "derive\nresult y = x\n", "E058"),
        ("演算子で終わる derive", "derive z(z) : money[円] = x +  range >=0円 <=100円\nresult y = z\n", "E059"),
        ("名前の壊れた derive", "derive z) : money[円] = x  range >=0円 <=100円\nresult y = x\n", "E058"),
        ("= の無い derive", "derive z(z) : money[円] x  range >=0円 <=100円\nresult y = x\n", "E006"),
        // Dropped with nothing said, and the check passed.
        ("演算子の抜けた define", "define z(z) : money[円] = x x\nresult y = z\n", "E059"),
        ("符号がくっついた引き算", "define z(z) : money[円] = x -1円\nresult y = z\n", "E059"),
        ("閉じない括弧", "define z(z) : money[円] = (x + 1円\nresult y = z\n", "E059"),
        ("抜けた引数", "define z(z) : money[円] = min(x, , 1円)\nresult y = z\n", "E059"),
        ("型の無い define", "define z(z) = x\nresult y = x\n", "E057"),
        ("= の前に語がある define", "define z(z) : money[円] range = x\nresult y = x\n", "E058"),
        ("名前の無い enum", "enum = a(a) | b(b)\nresult y = x\n", "E058"),
        ("名前の無い group", "group = a\nresult y = x\n", "E058"),
        ("形の崩れた result", "result y x\n", "E006"),
        ("中身の無い result", "result\n", "E058"),
        ("文字列の無い description", "description 説明\nresult y = x\n", "E058"),
        // Panicked.
        ("0 で割る", "define z(z) : money[円] = x ÷ 0\nresult y = z\n", "E115"),
        ("0円 で割る", "define z(z) : number = x ÷ 0円\nresult y = x\n", "E115"),
        // Passed, and the generated code divided by zero at run time.
        ("刻み 0 の丸めの呼び出し", "define z(z) : money[円] = down(x, 0円)\nresult y = z\n", "E060"),
        ("刻みが変数の丸めの呼び出し", "define z(z) : money[円] = down(x, x)\nresult y = z\n", "E118"),
    ];
    for (what, body, code) in cases {
        let src = format!("{HEAD}{body}");
        let got = codes_of(&src);
        assert!(got.contains(code), "{what}: {code} が出ない（出たのは {got:?}）\n{src}");
    }
    // An output's rounding grid and a type's step, which sit on the declarations themselves.
    let grid0 = "rule t(t) v1\n\ninputs\n  x(x) : money[円]  range >=0円 <=100円\n\noutputs\n  y(y) : money[円]  round up(0円)\n\nresult y = x\n";
    assert!(codes_of(grid0).contains(&"E060"), "round up(0円) が通る");
    let step0 = "rule t(t) v1\n\ninputs\n  r(r) : rate[step 0%]  range >=0% <=10%\n\noutputs\n  y(y) : bool\n\ntable j(j)\npolicy unique\n| r    | -> y(y) : bool |\n| <=5% | true           |\n| >5%  | false          |\n";
    assert!(codes_of(step0).contains(&"E060"), "rate[step 0%] が通る");
    // The `where` of a projection with nothing to compare — the rule does not need its contract
    // on disk for the parser to see it.
    let whr = "rule t(t) v1\n\nshape o(o) = jsonschema \"o.json\" \"#/$defs/O\"\n\ninputs\n  c(c) : bool  from any o.lines where chilled\n\noutputs\n  y(y) : bool\n\ntable j(j)\npolicy unique\n| c     | -> y(y) : bool |\n| true  | true           |\n| false | false          |\n";
    assert!(codes_of(whr).contains(&"E013"), "where の後が空のまま通る");
    // A `rule` line with no name used to leave a file with nothing in it, and pass.
    assert!(codes_of("rule\n\ninputs\n  x(x) : bool\n").contains(&"E003"), "名前の無い rule が通る");
    // A range upside down passed with nothing in it; under `allocate` it also panicked.
    let upside = "rule t(t) v1\n\ninputs\n  x(x) : money[円]  range >=10円 <=0円\n\noutputs\n  y(y) : money[円]  round down(1円)\n\nresult y = x\n";
    assert!(codes_of(upside).contains(&"E061"), "逆さの範囲が通る");
    let share = "rule t(t) v1\n\ninputs\n  off(off) : money[円]  range >=0円 <=1000円\n  upto(upto) : money[円]  range >=0円 <=1000円\n  base(base) : money[円]  range >=1円 <=0円\n\noutputs\n  o(o) : money[円]  round down(1円)\n\nconstraint upto <= base\n\nderive s(s) : money[円] = allocate(off, upto, base)  range >=0円 <=1000円\n\nresult o = s\n";
    assert!(codes_of(share).contains(&"E061"), "allocate の全体の範囲が逆さのまま通る");
}

/// Rules that passed the check and whose generated code then failed: found by generating code
/// for cut-up rules that still passed, and running it against their own vectors.
#[test]
fn 検査を通して生成コードが食い違っていた形は止まる() {
    // A table with no name was read as an anonymous one, and the code generated for it lost
    // the scale of its output.
    let anonymous = "rule t(t) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=1000円\n  r(r) : rate[step 0.1%]  range >=0% <=10%\n\noutputs\n  o(o) : money[円]  round up(1円)\n\ndefine f(f) : money[円] = a × r\n\ntable\npolicy unique\n| r      | -> o |\n| <=0.5% | 0円  |\n| >0.5%  | f    |\n";
    assert!(codes_of(anonymous).contains(&"E058"), "名前の無い表が通る");
    let junk = anonymous.replace("\ntable\n", "\ntable -\n");
    assert!(codes_of(&junk).contains(&"E058"), "名前の代わりの `-` が黙って捨てられる");
    // A date with the shape of one and no such day.
    let day = "rule t(t) v1\n\ninputs\n  d(d) : date  range >=2026-01-01 <=2026-12-31\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| d            | -> r(r) : bool |\n| <=2026-01-99 | true           |\n| >2026-01-99  | false          |\n";
    assert!(codes_of(day).contains(&"E062"), "1 月 99 日が通る");
    // An example outside the declared range, which the generated code refuses at its door.
    let example = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=2 <=5\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| n    | -> r(r) : bool |\n| <=3 | true           |\n| >3   | false          |\n\nexamples\n| n | -> r  |\n| 1 | true |\n";
    assert!(codes_of(example).contains(&"E019"), "範囲の外の例が通る");
    // The same for a field of an element of a sequence an example walks.
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/corpus/shipment_surcharge.rule")).unwrap();
    let narrowed = src.replacen("range >=1lb <=150lb", "range >=99lb <=150lb", 1);
    assert_ne!(narrowed, src, "コーパスの規則の形が変わっていて、範囲を狭められない");
    assert!(codes_of(&narrowed).contains(&"E019"), "範囲の外の要素を持つ例が通る");
    // A row whose closing bar was left off lost its last cell, and a cell with words after its
    // comparison lost them; the evaluator and the generated code read such a row apart.
    let base = "rule t(t) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=1000円\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| a      | -> r(r) : bool |\n| <=100円 | true           |\n| >100円  | false          |\n";
    let open_row = base.replace("| >100円  | false          |", "| >100円  | false");
    assert!(codes_of(&open_row).contains(&"E064"), "閉じていない行が通る");
    let short_row = base.replace("| >100円  | false          |", "| false |");
    assert!(codes_of(&short_row).contains(&"E064"), "セルの足りない行が通る");
    let junk = base.replace("| <=100円 | true ", "| <=100円 + 1円 | true ");
    assert!(codes_of(&junk).contains(&"E063"), "比較の後ろの語が捨てられる");
    let word = base.replace("| >100円  | false          |", "| >100円  | false 0円 |");
    assert!(codes_of(&word).contains(&"E014"), "出力のセルの二語が通る");
    let extra = format!("{base}\nexamples\n| a    | -> r |\n| 50円 | true | false |\n");
    assert!(codes_of(&extra).contains(&"E064"), "例の行の余分なセルが黙って捨てられる");
    let two = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\ninputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x   | -> r(r) : bool |\n| a b | true           |\n| b   | false          |\n";
    assert!(codes_of(two).contains(&"E063"), "コンマの無い二語のセルが一語目だけで読まれる");
}

#[test]
fn 深すぎるjsonは断る() {
    let deep = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
    assert!(rulec::json::parse(&deep(rulec::json::MAX_DEPTH)).is_ok(), "上限ちょうどは読める");
    let e = rulec::json::parse(&deep(rulec::json::MAX_DEPTH + 1)).expect_err("上限を超えたら断る");
    assert!(e.contains(&rulec::json::MAX_DEPTH.to_string()), "上限を言わない: {e}");
    // Some twenty thousand levels ran the reader out of stack and aborted the process.
    assert!(rulec::json::parse(&deep(100_000)).is_err());
    assert!(rulec::json::parse(&format!("{{\"a\":{}}}", deep(100_000))).is_err());
}
