//! The words and the syntax (DESIGN 2.1–2.3): every form of the grammar, where each thing is said to
//! be, and what cannot be read (E001–E004, E111, E602). The eight examples read as kaname.

use kaname::ast::*;
use kaname::parse::parse;
use ritsu_base::text::Lang;

fn module(src: &str) -> Module {
    let p = parse("t.kn", src);
    let said: Vec<String> = p.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(p.diags.is_empty(), "{}", said.join(""));
    p.module.expect("a module")
}

/// The codes and the places (line, column) of what the parser says.
fn said(src: &str) -> Vec<(&'static str, usize, usize)> {
    parse("t.kn", src).diags.iter().map(|d| (d.code, d.line.unwrap_or(0), d.col.unwrap_or(0))).collect()
}

fn answer(m: &Module) -> &Expr {
    &m.fns[0].body.answer
}

#[test]
fn the_head_the_uses_and_the_declarations() {
    let m = module(
        "module 支払いの遅れ(late_payment_ja) v2\ndescription \"Late payments\"\n\nuse rule charges from \"rules/charges.rule\"\nuse dates terms from \"dates/terms.cal\"\nuse book ledger from \"books/ledger.book\"\nuse module pricing from \"pricing.kn\"\n\nenum tier = basic \"Basic\" < silver < gold\nrecord Line\n  sku : string\n  qty : number  range >=1 <=20\n  lines : list[Line] count >=1 <=30\nenum channel = web | store\n",
    );
    assert_eq!(m.name.text, "支払いの遅れ");
    assert_eq!(m.name.ascii(), "late_payment_ja");
    assert_eq!(m.version, "v2");
    assert_eq!(m.description.as_ref().map(|(d, _)| d.as_str()), Some("Late payments"));
    let kinds: Vec<(UseKind, &str, &str)> = m.uses.iter().map(|u| (u.kind, u.name.as_str(), u.path.as_str())).collect();
    assert_eq!(
        kinds,
        [(UseKind::Rule, "charges", "rules/charges.rule"), (UseKind::Dates, "terms", "dates/terms.cal"), (UseKind::Book, "ledger", "books/ledger.book"), (UseKind::Module, "pricing", "pricing.kn")]
    );
    assert!(m.enums[0].ordered && !m.enums[1].ordered);
    assert_eq!(m.enums[0].values[0].label.as_deref(), Some("Basic"));
    let r = &m.records[0];
    assert_eq!(r.fields.len(), 3);
    assert!(r.fields[1].bounds.range.is_some() && r.fields[2].bounds.count.is_some());
    assert!(matches!(&r.fields[2].ty.kind, TypeKind::List(_)));
}

#[test]
fn types_as_written() {
    let m = module(
        "module m v1\nrecord R\n  a : money[USD, incl_tax]  range >=0USD <=10USD\n  b : rate[step 0.01%]\n  c : mass[oz]?\n  d : charges.band\n  e : ledger.capture.do\n  f : running[Line, money[USDc]]\n  g : list[number?]\n",
    );
    let kinds: Vec<&TypeKind> = m.records[0].fields.iter().map(|f| &f.ty.kind).collect();
    assert!(matches!(kinds[0], TypeKind::Unit { dim, unit, tax: Some(t), .. } if dim == "money" && unit == "USD" && t == "incl_tax"));
    assert!(matches!(kinds[1], TypeKind::Rate(Some(n)) if n.raw == "0.01%"));
    assert!(matches!(kinds[2], TypeKind::Unit { dim, unit, .. } if dim == "mass" && unit == "oz") && m.records[0].fields[2].ty.optional);
    assert!(matches!(kinds[3], TypeKind::Named(p) if p.len() == 2));
    assert!(matches!(kinds[4], TypeKind::Named(p) if p.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>() == ["ledger", "capture", "do"]));
    assert!(matches!(kinds[5], TypeKind::Running(..)));
    assert!(matches!(kinds[6], TypeKind::List(t) if t.optional));
}

#[test]
fn functions_with_aliases_effects_and_bounds() {
    let m = module(
        "module m v1\nfn 遅れの請求(claim)(請求書(invoice): 請求書, 支払日(paid_on): date range >=2026-01-01 <=2029-12-31) -> 請求? total\n  none\nfn a() -> number range >0 fails rule, overflow diverges\n  1\nfn b() -> number diverges\n  1\nfn c() -> number fails zero\n  1\n",
    );
    let f = &m.fns[0];
    assert_eq!((f.name.text.as_str(), f.name.ascii()), ("遅れの請求", "claim"));
    assert_eq!(f.params.iter().map(|p| p.name.ascii()).collect::<Vec<_>>(), ["invoice", "paid_on"]);
    assert!(f.effect.as_ref().is_some_and(|e| e.total));
    let a = m.fns[1].effect.as_ref().unwrap();
    assert_eq!(a.fails.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [FailKind::Rule, FailKind::Overflow]);
    assert!(a.diverges && !a.total);
    assert!(m.fns[1].ret_bounds.range.is_some());
    assert!(m.fns[2].effect.as_ref().unwrap().diverges);
    assert_eq!(m.fns[3].effect.as_ref().unwrap().fails[0].0, FailKind::Zero);
}

#[test]
fn blocks_lets_ifs_and_matches() {
    let m = module(
        "module m v1\nfn f(x: number range >=0 <=9, t: tier) -> number\n  description \"d\"\n  let y =\n    let z = x + 1\n    z * 2\n  let w = if y > 3 then 1 else 2\n  match t\n    a, b => w\n    c =>\n      let v = 3\n      v\n",
    );
    let f = &m.fns[0];
    assert_eq!(f.description.as_ref().map(|(d, _)| d.as_str()), Some("d"));
    assert_eq!(f.body.lets.len(), 2);
    assert!(matches!(&f.body.lets[0].value.kind, ExprKind::Block(b) if b.lets.len() == 1));
    assert!(matches!(&f.body.lets[1].value.kind, ExprKind::If(..)));
    let ExprKind::Match(_, arms) = &f.body.answer.kind else { panic!("a match") };
    assert_eq!(arms.len(), 2);
    assert_eq!(arms[0].patterns.len(), 2);
    assert_eq!(arms[1].body.lets.len(), 1);
    // `else if` is an `if` in the `else`
    let m = module("module m v1\nfn f(x: number range >=0 <=9) -> number\n  if x > 5\n    1\n  else if x > 2\n    2\n  else\n    3\n");
    let ExprKind::If(_, _, e) = &answer(&m).kind else { panic!("an if") };
    assert!(matches!(e.answer.kind, ExprKind::If(..)));
}

#[test]
fn the_precedence_of_the_operators() {
    let m = module("module m v1\nfn f(a: bool, b: bool, x: number range >=0 <=9) -> bool\n  not a or b and x + 2 * 3 >= -x in kg\n");
    // `or` is the loosest: (not a) or (b and ((x + (2 * 3)) >= ((-x) in kg)))
    let ExprKind::Bin(l, BinOp::Or, r) = &answer(&m).kind else { panic!("or") };
    assert!(matches!(l.kind, ExprKind::Not(_)));
    let ExprKind::Bin(_, BinOp::And, c) = &r.kind else { panic!("and") };
    let ExprKind::Bin(sum, BinOp::Ge, conv) = &c.kind else { panic!(">=") };
    assert!(matches!(&sum.kind, ExprKind::Bin(_, BinOp::Add, p) if matches!(p.kind, ExprKind::Bin(_, BinOp::Mul, _))));
    assert!(matches!(&conv.kind, ExprKind::In(x, u, _) if u == "kg" && matches!(x.kind, ExprKind::Neg(_))));
}

#[test]
fn calls_records_lists_and_functions_given_to_builtins() {
    let m = module(
        "module m v1\nfn f(xs: list[Line] count <=3) -> list[Share]\n  map(running_totals(xs, l => l.amount), r =>\n    Share { label: r.item.label, share: rule(discount: d, before: r.before,) })\n",
    );
    let ExprKind::Call(callee, args) = &answer(&m).kind else { panic!("a call") };
    assert!(matches!(&callee.kind, ExprKind::Var(w) if w == "map"));
    assert!(matches!(&args[0], Arg::Pos(x) if matches!(x.kind, ExprKind::Call(..))));
    let Arg::Pos(lam) = &args[1] else { panic!("positional") };
    let ExprKind::Lambda(ps, body) = &lam.kind else { panic!("a function") };
    assert_eq!(ps[0].0, "r");
    let ExprKind::Record(path, fields) = &body.kind else { panic!("a record") };
    assert_eq!(path[0].0, "Share");
    assert_eq!(fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["label", "share"]);
    assert!(matches!(&fields[1].value.kind, ExprKind::Call(_, a) if a.len() == 2 && matches!(a[0], Arg::Named(..))));
    let m = module("module m v1\nfn f() -> number\n  reduce([1, 2, 3], 0, (acc, x) => acc + x)\n");
    let ExprKind::Call(_, args) = &answer(&m).kind else { panic!("a call") };
    assert!(matches!(&args[0], Arg::Pos(x) if matches!(&x.kind, ExprKind::List(xs) if xs.len() == 3)));
    assert!(matches!(&args[2], Arg::Pos(x) if matches!(&x.kind, ExprKind::Lambda(ps, _) if ps.len() == 2)));
    // a dotted name called, as a date or a transfer is
    let m = module("module m v1\nfn f() -> number\n  ledger.capture.do(payment: \"p\")\n");
    let ExprKind::Call(callee, _) = &answer(&m).kind else { panic!("a call") };
    assert_eq!(callee.path().unwrap().iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>(), ["ledger", "capture", "do"]);
}

#[test]
fn the_full_width_signs_and_digits_are_read() {
    let m = module("module m v1\nfn f(x: number range >=0 <=９) -> bool\n  x × 2 ÷ 1 − 3 ≦ 4 and x ≧ 0\n");
    let ExprKind::Bin(l, BinOp::And, r) = &answer(&m).kind else { panic!("and") };
    assert!(matches!(&l.kind, ExprKind::Bin(_, BinOp::Le, _)));
    assert!(matches!(&r.kind, ExprKind::Bin(_, BinOp::Ge, _)));
}

#[test]
fn examples_go_on_on_deeper_lines() {
    let m = module("module m v1\nfn f(x: number range >=0 <=9) -> R\n  R { a: x }\nexamples\n  f(x: 1)\n    = R {\n        a: 1,\n      }\n  f(x: 2) = R { a: 2 }\n  f(x: 3) fails range\n");
    assert_eq!(m.examples.len(), 3);
    assert!(matches!(m.examples[0].expect, Expect::Value(_)));
    assert!(matches!(m.examples[2].expect, Expect::Fails(FailKind::Range, _)));
}

#[test]
fn what_cannot_be_read_and_where() {
    // E001: the words, a second comparison, a sign where a value goes
    assert_eq!(said("module m v1\nfn f(a: bool, b: bool) -> bool\n  a && b\n"), [("E001", 3, 5)]);
    assert_eq!(said("module m v1\nfn f(a: number range >=0 <=1) -> bool\n  0 < a < 1\n"), [("E001", 3, 9)]);
    assert_eq!(said("module m v1\nfn f() -> number\n  1,000\n"), [("E001", 3, 3)]);
    assert_eq!(said("module m v1\nfn f() -> number\n  (1 + 2\n"), [("E001", 3, 3)]);
    assert_eq!(said("module m v1\nfn f() -> number\n  1 +\n"), [("E001", 3, 6)]);
    // E002: a keyword for a name
    assert_eq!(said("module m v1\nrecord R\n  if : number range >=0 <=1\n"), [("E002", 3, 3)]);
    // E003: the head, the order, the lines once
    assert_eq!(said("record R\n  a : string\n"), [("E003", 1, 1)]);
    assert_eq!(said("module m\n"), [("E003", 1, 8)]);
    assert_eq!(said("module m v1\ndescription \"a\"\ndescription \"b\"\n"), [("E003", 3, 1)]);
    assert_eq!(said("module m v1\nexamples\nexamples\n"), [("E003", 3, 1)]);
    assert_eq!(said("module m v1\nrule r v1\n"), [("E003", 2, 1)]);
    assert_eq!(said("module m v1\nuse table t from \"t.rule\"\n"), [("E003", 2, 5)]);
    // E004: the lines of a block that do not line up, and a line under nothing
    assert_eq!(said("module m v1\nfn f() -> number\n  let x = 1\n   x\n"), [("E004", 4, 4)]);
    assert_eq!(said("module m v1\n  fn f() -> number\n"), [("E004", 2, 3)]);
    // E111: no answer, no else, lines after the answer
    assert_eq!(said("module m v1\nfn f() -> number\n"), [("E111", 2, 1)]);
    assert_eq!(said("module m v1\nfn f() -> number\n  1\n  2\n"), [("E111", 4, 3)]);
    assert_eq!(said("module m v1\nfn f(a: bool) -> number\n  if a then 1\n"), [("E111", 3, 14)]);
    // E602: an example that is no call, or has no answer
    assert_eq!(said("module m v1\nfn f() -> number\n  1\nexamples\n  f()\n"), [("E602", 5, 6)]);
}

#[test]
fn the_examples_read_as_kaname() {
    for f in [
        "examples/payout/payout.kn",
        "examples/payout/payout.ja.kn",
        "examples/late_payment/late_payment.kn",
        "examples/late_payment/late_payment.ja.kn",
        "examples/media_mail/media_mail.kn",
        "examples/media_mail/media_mail.ja.kn",
        "examples/discount_split/discount_split.kn",
        "examples/discount_split/discount_split.ja.kn",
    ] {
        let src = std::fs::read_to_string(f).unwrap();
        let p = parse(f, &src);
        let said: Vec<String> = p.diags.iter().map(|d| d.render(Lang::En)).collect();
        assert!(p.diags.is_empty(), "{f}: {}", said.join(""));
        let m = p.module.unwrap();
        assert!(!m.fns.is_empty() && !m.examples.is_empty(), "{f}");
    }
}
