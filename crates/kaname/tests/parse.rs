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
    assert!(matches!(kinds[3], TypeKind::Named(p, _) if p.len() == 2));
    assert!(matches!(kinds[4], TypeKind::Named(p, _) if p.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>() == ["ledger", "capture", "do"]));
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

/// An enum with its values listed under it, values with fields, a `match` that names the value it
/// takes, `else` and literals, and a record made from another (DESIGN 23.1, 23.3, 23.4).
#[test]
fn values_with_fields_else_literals_and_records_made_from_another() {
    let m = module(
        "module m v1\nenum Order\n  draft \"Draft\"\n    items : list[Item]  count <=20\n  placed\n    items : list[Item]\n    on    : date  range >=2026-01-01 <=2027-12-31\n  cancelled\nenum tier = basic | gold\n",
    );
    let e = &m.enums[0];
    assert!(e.listed && !e.ordered);
    assert_eq!(e.values.iter().map(|v| (v.name.text.as_str(), v.fields.len())).collect::<Vec<_>>(), [("draft", 1), ("placed", 2), ("cancelled", 0)]);
    assert_eq!(e.values[0].label.as_deref(), Some("Draft"));
    assert!(e.values[1].fields[1].bounds.range.is_some() && e.values[0].fields[0].bounds.count.is_some());
    assert_eq!(e.values.iter().map(|v| v.line).collect::<Vec<_>>(), [3, 5, 8]);
    assert!(!m.enums[1].listed);
    let m = module(
        "module m v1\nfn f(o: Order, n: number range >=-9 <=9) -> number\n  match o\n    placed p => 1\n    Order.draft d => 2\n    cancelled => 3\n    else =>\n      match n\n        -1, 0 => 4\n        2026-01-01 => 5\n        \"A\" => 6\n        else => 7\n",
    );
    let ExprKind::Match(_, arms) = &answer(&m).kind else { panic!("a match") };
    assert!(matches!(&arms[0].patterns[0], Pattern::Value(p, Some((b, _)), _) if p.len() == 1 && b == "p"));
    assert!(matches!(&arms[1].patterns[0], Pattern::Value(p, Some((b, _)), _) if p.len() == 2 && b == "d"));
    assert!(matches!(&arms[2].patterns[0], Pattern::Value(_, None, _)));
    assert!(matches!(&arms[3].patterns[0], Pattern::Else(_)));
    let ExprKind::Match(_, inner) = &arms[3].body.answer.kind else { panic!("a match in the arm") };
    assert!(matches!(&inner[0].patterns[0], Pattern::Lit(x) if matches!(x.kind, ExprKind::Neg(_))));
    assert!(matches!(&inner[0].patterns[1], Pattern::Lit(x) if matches!(x.kind, ExprKind::Num(_))));
    assert!(matches!(&inner[1].patterns[0], Pattern::Lit(x) if matches!(x.kind, ExprKind::Date(_))));
    assert!(matches!(&inner[2].patterns[0], Pattern::Lit(x) if matches!(x.kind, ExprKind::Str(_))));
    // `..x` first in the braces
    let m = module("module m v1\nfn f(p: Order.placed) -> Order\n  Order.placed { ..p, on: 2026-03-01 }\n");
    let ExprKind::Record(path, Some(base), fields) = &answer(&m).kind else { panic!("a record made from another") };
    assert_eq!(path.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>(), ["Order", "placed"]);
    assert!(matches!(&base.kind, ExprKind::Var(w) if w == "p"));
    assert_eq!(fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["on"]);
    // the type of a value, as a parameter's
    assert!(matches!(&m.fns[0].params[0].ty.kind, TypeKind::Named(p, _) if p.len() == 2));
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
    let ExprKind::Record(path, base, fields) = &body.kind else { panic!("a record") };
    assert_eq!(path[0].0, "Share");
    assert!(base.is_none());
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

/// The types of dicts, sets and the builtin records (their words are types only before `[`, so a
/// record may be called `set`), a dict literal and `[:]`, and a builtin written after a value: a call
/// of what is after the dot (DESIGN 21.4, 24.2).
#[test]
fn dicts_sets_and_builtins_written_after_a_value() {
    let m = module(
        "module m v1\nrecord R\n  a : dict[string, list[number]] count <=9\n  b : set[date]\n  c : entry[string, number]\n  d : indexed[R]\n  e : set\nrecord set\n  dict : number\n",
    );
    let kinds: Vec<&TypeKind> = m.records[0].fields.iter().map(|f| &f.ty.kind).collect();
    assert!(matches!(kinds[0], TypeKind::Dict(k, v) if matches!(k.kind, TypeKind::Str) && matches!(v.kind, TypeKind::List(_))));
    assert!(m.records[0].fields[0].bounds.count.is_some());
    assert!(matches!(kinds[1], TypeKind::Set(t) if matches!(t.kind, TypeKind::Date)));
    assert!(matches!(kinds[2], TypeKind::Entry(..)));
    assert!(matches!(kinds[3], TypeKind::Indexed(_)));
    assert!(matches!(kinds[4], TypeKind::Named(p, _) if p[0].0 == "set"));
    assert_eq!(m.records[1].name.text, "set");
    assert_eq!(m.records[1].fields[0].name.text, "dict");
    // a dict literal: each key and its value; `[:]` has none
    let m = module("module m v1\nfn f(k: string) -> dict[string, number]\n  [\"a\": 1, k: 2 + 3]\n");
    let ExprKind::Dict(pairs) = &answer(&m).kind else { panic!("a dict") };
    assert_eq!(pairs.len(), 2);
    assert!(matches!(&pairs[0].0.kind, ExprKind::Str(s) if s == "a"));
    assert!(matches!(&pairs[1].0.kind, ExprKind::Var(w) if w == "k") && matches!(pairs[1].1.kind, ExprKind::Bin(..)));
    let m = module("module m v1\nfn f() -> dict[string, number]\n  [:]\n");
    assert!(matches!(&answer(&m).kind, ExprKind::Dict(p) if p.is_empty()));
    // `xs.sort_by(x => x.due).take(3)`: a call of `take` on the call of `sort_by` on `xs`
    let m = module("module m v1\nfn f(xs: list[Line]) -> list[Line]\n  xs.sort_by(x => x.due).take(3)\n");
    let ExprKind::Call(callee, args) = &answer(&m).kind else { panic!("a call") };
    let ExprKind::Field(base, f, _) = &callee.kind else { panic!("after a value") };
    assert_eq!((f.as_str(), args.len()), ("take", 1));
    let ExprKind::Call(inner, args) = &base.kind else { panic!("a call") };
    assert!(matches!(&inner.kind, ExprKind::Field(x, f, _) if f == "sort_by" && matches!(&x.kind, ExprKind::Var(w) if w == "xs")));
    assert!(matches!(&args[0], Arg::Pos(x) if matches!(x.kind, ExprKind::Lambda(..))));
    // a key with no value after it: the `:` is looked for where the key ends
    assert_eq!(said("module m v1\nfn f() -> dict[string, number]\n  [\"a\": 1, 2]\n"), [("E001", 3, 13)]);
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
    // a listed enum with no value, a value under nothing, a field that does not line up, `..` twice
    // or late, `default` among the values
    assert_eq!(said("module m v1\nenum Order\nfn f() -> number\n  1\n"), [("E003", 2, 6)]);
    assert_eq!(said("module m v1\nenum Order\n  draft\n    items : number\n      more : number\n"), [("E004", 5, 7)]);
    assert_eq!(said("module m v1\nfn f(i: Item) -> Item\n  Item { a: 1, ..i }\n"), [("E001", 3, 16)]);
    assert_eq!(said("module m v1\nenum Order\n  draft\n  default draft\n"), [("E003", 4, 3)]);
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

/// Function types with the effect written on them, lambdas whose parameters have their types written,
/// lambdas anywhere an expression goes, and `private` before `fn`, `record` and `enum` (DESIGN 27.1,
/// 28.2): a word read only where it stands.
#[test]
fn function_types_lambdas_and_private() {
    let m = module(
        "module m v1\nfn total_by[T](xs: list[T], amount: (T) -> money[USDc], test: (T, number) -> bool total, f: () -> number fails range, overflow) -> money[USDc]\n  xs.sum(x => amount(x))\n",
    );
    let ps = &m.fns[0].params;
    let TypeKind::Fn(args, ret, eff) = &ps[1].ty.kind else { panic!("a function type") };
    assert_eq!(args.len(), 1);
    assert!(matches!(&ret.kind, TypeKind::Unit { dim, unit, .. } if dim == "money" && unit == "USDc"));
    assert!(eff.is_none());
    let TypeKind::Fn(args, _, eff) = &ps[2].ty.kind else { panic!("a function type") };
    assert_eq!(args.len(), 2);
    assert!(eff.as_ref().is_some_and(|e| e.total && e.fails.is_empty()));
    let TypeKind::Fn(args, _, eff) = &ps[3].ty.kind else { panic!("a function type") };
    assert!(args.is_empty());
    assert_eq!(eff.as_ref().unwrap().fails.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [FailKind::Range, FailKind::Overflow]);
    // a lambda a `let` names, its parameters' types written, and called
    let m = module("module m v1\nfn f(up: bool) -> number\n  let g = if up then (l: Line, n: number) => l.qty + n else (l: Line, n: number) => n\n  g(x, 1)\n");
    let l = &m.fns[0].body.lets[0];
    let ExprKind::If(_, t, _) = &l.value.kind else { panic!("an if") };
    let ExprKind::Lambda(ps, _) = &t.answer.kind else { panic!("a lambda") };
    assert_eq!(ps.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["l", "n"]);
    let written = &m.ltypes[&t.answer.id];
    assert!(written.iter().all(|w| w.is_some()));
    assert!(matches!(&answer(&m).kind, ExprKind::Call(callee, args) if matches!(&callee.kind, ExprKind::Var(w) if w == "g") && args.len() == 2));
    // `private`, and the word as a name where it is no keyword
    let m = module("module m v1\nprivate record Bounds\n  low : number\nprivate enum side = below | above\nprivate fn place(private: number) -> number\n  private\n");
    assert!(m.records[0].private && m.enums[0].private && m.fns[0].private);
    assert_eq!(m.fns[0].params[0].name.text, "private");
    let m = module("module m v1\nfn place(n: number) -> number\n  n\n");
    assert!(!m.fns[0].private);
    // `private` before what is neither a function nor a type
    assert_eq!(said("module m v1\nprivate use module p from \"p.kn\"\n").first().map(|x| x.0), Some("E003"));
}
