//! How the projection function reads the caller's object (§15.125, §15.223, §15.234, §15.244).
//!
//! Each value the function reads out of the object is held to the kind the rule takes it as before
//! it is used, the way the rule's own door holds an argument (§15.203, §15.205): a whole number
//! (one written with a point that is whole too, and the decimal string protojson writes a 64-bit
//! integer as when the contract is a `.proto`), JSON's `true` or `false`, a string, a
//! `YYYY-MM-DD` string of a day the calendar has, a value of the rule's enum. A field a `where`
//! tests is held to what the contract carries there. A value that is not is refused with the
//! module's `RuleInputError`, and the sentence names its path as the rule writes it —
//! `order.lines[].amount_usd is not an integer` — with the value beside it where the language's
//! error carries one; a place on the way that is not an object, a value that is not there, and a
//! collection that is not an array are refused the same way. Before, the value was read the way
//! the language reads it: `int(1.5)` is 1, `"abc".to_i` is 0, `BigInt(true)` is `1n`, and the rule
//! answered for a value it does not take.
//!
//! The five read in one order, so that one object stops at the same value in each: the inputs in
//! the order the rule declares them, then the sequences; along a collection, element by element,
//! the value the `where` tests and then, when the element passes, the value that is added up or
//! compared; along a sequence, element by element, its fields in the order declared. `any` and
//! `all` test every element rather than stopping at the first that settles them, so that a value
//! of the wrong kind after it is not let through in one language and refused in another.

use super::{Gen, Proj, SeqProj, Syn, brand_of, cell_whole, elem_cond_with, pb_zero, pub_name, raw_lit};
use crate::ast::{Cell, Lit, ProjKind};
use crate::projection::{At, Zero};
use crate::types::Ty;

/// What a value read out of the object is held to (§15.234).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Kind {
    /// A whole number, or a number with a point that is whole.
    Int,
    /// The same, or the decimal string protojson writes a 64-bit integer as.
    IntPb,
    Bool,
    Str,
    /// A `YYYY-MM-DD` string of a day the calendar has, read as its day number.
    Date,
    /// Any number, a fraction too: a field of a `number` a `where` compares.
    Num,
    /// One of the values of the rule's enum of this name.
    Enum(String),
}

/// The kind a value read into a value of `ty` is held to. `pb`: it is read from a `.proto`
/// contract's JSON form.
pub(super) fn kind_of(ty: &Ty, pb: bool) -> Option<Kind> {
    Some(match ty.present() {
        Ty::Number | Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
            if pb {
                Kind::IntPb
            } else {
                Kind::Int
            }
        }
        Ty::Bool => Kind::Bool,
        Ty::Str => Kind::Str,
        Ty::Date => Kind::Date,
        Ty::Enum(n) => Kind::Enum(n.clone()),
        _ => return None,
    })
}

/// The literals of a `where` cell.
fn cell_lits(cell: &Cell) -> Vec<&Lit> {
    match cell {
        Cell::Lit(l) => vec![l],
        Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
        Cell::Cmp(ops) => ops.iter().map(|(_, l)| l).collect(),
        _ => Vec::new(),
    }
}

/// The kind the field a `where` tests is held to: what the contract carries there — from a
/// `.proto`, what protojson writes it as — and a date where the cell names one, since a date is
/// compared as the day it is.
pub(super) fn test_kind(cell: &Cell, at: Option<&At>, zero: Option<&Zero>) -> Kind {
    let lits = cell_lits(cell);
    if lits.iter().any(|l| matches!(l, Lit::Date(..))) {
        return Kind::Date;
    }
    let word = |l: &&Lit| matches!(l, Lit::Word(w) if w == crate::kw::TRUE || w == crate::kw::FALSE);
    match (zero, at) {
        (Some(Zero::Int), _) => Kind::IntPb,
        (Some(Zero::Frac), _) => Kind::Num,
        (Some(Zero::Bool), _) => Kind::Bool,
        (Some(_), _) => Kind::Str,
        (None, Some(At::Int)) => Kind::Int,
        (None, Some(At::Frac)) => Kind::Num,
        (None, Some(At::Bool)) => Kind::Bool,
        (None, Some(At::Str)) => Kind::Str,
        _ => match lits.first() {
            Some(Lit::Num(_)) => Kind::Num,
            Some(l) if word(l) => Kind::Bool,
            _ => Kind::Str,
        },
    }
}

/// One sentence of the door with the place of the path left open: what comes before the path and
/// what after it, in the language the code is generated in.
fn sentence(f: impl Fn(&str) -> String) -> (String, String) {
    const MARK: &str = "\u{1}";
    let s = f(MARK);
    match s.split_once(MARK) {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (s, String::new()),
    }
}

/// The sentences a refusal of the projection function says, each with the place of the path open.
pub(super) struct Says {
    pub(super) object: (String, String),
    pub(super) missing: (String, String),
    pub(super) integer: (String, String),
    pub(super) boolean: (String, String),
    pub(super) string: (String, String),
    pub(super) date: (String, String),
    pub(super) number: (String, String),
    pub(super) sequence: (String, String),
}

pub(super) fn says() -> Says {
    Says {
        object: sentence(crate::door::not_an_object),
        missing: sentence(crate::door::missing),
        integer: sentence(crate::door::not_integer),
        boolean: sentence(crate::door::not_bool),
        string: sentence(crate::door::not_string),
        date: sentence(crate::door::not_date),
        number: sentence(crate::door::not_number),
        sequence: sentence(crate::door::not_sequence),
    }
}

/// `"before" + at + "after"` in a language whose strings join with `join` and are quoted by `q`,
/// leaving out a part that is empty.
pub(super) fn joined((a, b): &(String, String), at: &str, join: &str, q: &dyn Fn(&str) -> String) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !a.is_empty() {
        parts.push(q(a));
    }
    parts.push(at.to_string());
    if !b.is_empty() {
        parts.push(q(b));
    }
    parts.join(&format!(" {join} "))
}

fn dq(s: &str) -> String {
    format!("{s:?}")
}

/// Whether `body` calls `name` (a whole name, not a part of a longer one).
pub(super) fn calls(body: &str, name: &str) -> bool {
    let part = |c: char| c.is_alphanumeric() || c == '_';
    body.match_indices(&format!("{name}(")).any(|(i, _)| !body[..i].chars().next_back().is_some_and(part))
}

/// The path of the object a `where` and an `of` read a field of: `order.lines[]`.
pub(super) fn elem_at(p: &Proj) -> String {
    format!("{}[]", p.shown)
}

/// The path of the object the last step of `p`'s path is read from, and the steps before it are
/// read from: `order` for `order.shipping.zone`, `order.lines[]` for a field of an element.
pub(super) fn base_of(p: &Proj) -> String {
    let tail = format!(".{}", p.path.join("."));
    p.shown.strip_suffix(&tail).unwrap_or(&p.shown).to_string()
}

/// The path of the object a sequence's collection is read from: `order` for `order.lines`.
pub(super) fn seq_base(q: &SeqProj) -> String {
    let tail = format!(".{}", q.path.join("."));
    q.shown.strip_suffix(&tail).unwrap_or(&q.shown).to_string()
}

impl Gen<'_> {
    /// The enum's type, as the module of `lang` names it.
    pub(super) fn read_enum_ty(&self, n: &str) -> String {
        self.enum_names.get(n).cloned().unwrap_or_default()
    }

    /// The literal a `where` cell compares with, for a field held to `kind`, in one language:
    /// a date as its day number, a number as written (`n` after it for a TypeScript bigint), a word
    /// and a string quoted.
    pub(super) fn read_lit(&self, l: &Lit, kind: &Kind, lang: &str) -> String {
        match l {
            Lit::Date(y, m, d) => {
                let n = crate::types::date_ord(*y, *m, *d).num;
                if lang == "ts" { format!("{n}n") } else { n.to_string() }
            }
            Lit::Num(n) if lang == "ts" && matches!(kind, Kind::Int | Kind::IntPb) => format!("{}n", n.raw),
            Lit::Str(s) | Lit::Word(s) if lang == "php" && s != crate::kw::TRUE && s != crate::kw::FALSE => super::php::php_str(s),
            l => match lang {
                "py" => raw_lit(l, "True", "False"),
                _ => raw_lit(l, "true", "false"),
            },
        }
    }

    // --- Python -------------------------------------------------------------------------------

    /// A value read out of the object `o` at the key `k`, `at` naming `o`, held to `kind` (Python).
    fn py_obj_at(&self, kind: &Kind, o: &str, k: &str, at: &str) -> String {
        let (kq, aq) = (dq(k), dq(at));
        match kind {
            Kind::Int => format!("_int_at({o}, {kq}, {aq})"),
            Kind::IntPb => format!("_int_pb(_at({o}, {kq}, {aq}), {})", dq(&format!("{at}.{k}"))),
            Kind::Bool => format!("_bool_at({o}, {kq}, {aq})"),
            Kind::Str => format!("_str_at({o}, {kq}, {aq})"),
            Kind::Date => format!("_date_at({o}, {kq}, {aq})"),
            Kind::Num => format!("_num_at({o}, {kq}, {aq})"),
            Kind::Enum(n) => format!(
                "{}(_at({o}, {kq}, {aq}), {})",
                self.py_enum_reader(n),
                dq(&crate::door::not_in_enum(&format!("{at}.{k}"), n))
            ),
        }
    }

    /// A value already read, `at` naming it, held to `kind` (Python).
    fn py_obj_of(&self, kind: &Kind, v: &str, at: &str) -> String {
        let aq = dq(at);
        match kind {
            Kind::Int => format!("_int_of({v}, {aq})"),
            Kind::IntPb => format!("_int_pb({v}, {aq})"),
            Kind::Bool => format!("_bool_of({v}, {aq})"),
            Kind::Str => format!("_str_of({v}, {aq})"),
            Kind::Date => format!("_date_of({v}, {aq})"),
            Kind::Num => format!("_num_of({v}, {aq})"),
            Kind::Enum(n) => format!("{}({v}, {})", self.py_enum_reader(n), dq(&crate::door::not_in_enum(at, n))),
        }
    }

    /// The function that reads a value of an enum, for each type of enum the rule has, in the
    /// order it has them (§15.244): `_shipping_zone_of` for `ShippingZone`, the type in snake case
    /// between `_` and `_of`, as `_int_of` and `_bool_of` read the other kinds. One function per
    /// type, each answering its own type: a function that took the type as an argument would need
    /// a type variable, a statement at the top of the module that is computed when the module
    /// runs, and a compiler of typed Python that builds the module ahead of time does not take
    /// one. A name the module already gives a builtin, a helper or a group, a parameter of the
    /// projection function, or the reader of a type before it, gets one more `_`.
    fn py_enum_readers(&self) -> Vec<(String, String)> {
        let mut taken: Vec<String> = super::PY_BUILTINS.iter().map(|(alias, _)| alias.to_string()).collect();
        taken.extend(super::PY_HELPERS.iter().map(|h| h.to_string()));
        taken.extend(self.f.groups.iter().map(|g| self.py_group(&g.name.text)));
        taken.extend(self.plain_from_params());
        let mut readers: Vec<(String, String)> = Vec::new();
        for en in &self.c.enum_order {
            let ty = self.enum_type(en);
            if readers.iter().any(|(t, _)| *t == ty) {
                continue;
            }
            let mut name = format!("_{}_of", super::snake(&ty));
            while taken.contains(&name) {
                name.push('_');
            }
            taken.push(name.clone());
            readers.push((ty, name));
        }
        readers
    }

    /// The function that reads a value of the enum `en` (§15.244).
    fn py_enum_reader(&self, en: &str) -> String {
        let ty = self.enum_type(en);
        self.py_enum_readers().into_iter().find(|(t, _)| *t == ty).map(|(_, r)| r).unwrap_or_else(|| format!("_{}_of", super::snake(&ty)))
    }

    /// A checked whole number as the rule's brand of its type (Python).
    fn py_obj_branded(&self, ty: &Ty, e: String) -> String {
        match ty.present() {
            t @ (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) => format!("{}({e})", brand_of(t)),
            _ => e,
        }
    }

    /// The steps of a path from `root` before its last one, each read as an object: the expression
    /// for the object the last step is read from, and that object's path. Python and Ruby.
    pub(super) fn walk_to_last(&self, p: &Proj, root: &str, at_fn: &dyn Fn(&str, &str, &str) -> String) -> (String, String) {
        let mut o = root.to_string();
        let mut at = base_of(p);
        for s in &p.path[..p.path.len().saturating_sub(1)] {
            o = at_fn(&o, s, &at);
            at = format!("{at}.{s}");
        }
        (o, at)
    }

    /// The test of an element a `where` makes, in Python.
    fn py_obj_test(&self, p: &Proj, t: (&crate::ast::Name, &Cell)) -> String {
        let (field, cell) = t;
        let syn = Syn { is: "==", isnt: "!=", and: "and", or: "or", tru: "True", not: Some("not ") };
        let (kind, v) = match &p.proto {
            Some(pp) => {
                let (j, n, z) = pp.elem.clone().unwrap_or((field.text.clone(), field.text.clone(), Zero::Str));
                let kind = test_kind(cell, None, Some(&z));
                let zero = pb_zero(&z, "False", &dq);
                (kind.clone(), self.py_obj_of(&kind, &format!("_pget(_e, {}, {}, {zero}, {})", dq(&j), dq(&n), dq(&elem_at(p))), &format!("{}.{}", elem_at(p), field.text)))
            }
            None => {
                let kind = test_kind(cell, p.test_kind.as_ref(), None);
                (kind.clone(), self.py_obj_at(&kind, "_e", &field.text, &elem_at(p)))
            }
        };
        elem_cond_with(cell, &v, &syn, &|l| self.read_lit(l, &kind, "py"))
    }

    /// The value of an element `sum`, `max` and `min` read, in Python.
    fn py_obj_of_field(&self, p: &Proj, of: &crate::ast::Name) -> String {
        let at = format!("{}.{}", elem_at(p), of.text);
        match (&p.proto, &p.proto.as_ref().and_then(|pp| pp.of.clone())) {
            (Some(_), Some((j, n, z))) => {
                let kind = if matches!(p.ty, Ty::Date) { Kind::Date } else { Kind::IntPb };
                let zero = pb_zero(z, "False", &dq);
                self.py_obj_of(&kind, &format!("_pget(_e, {}, {}, {zero}, {})", dq(j), dq(n), dq(&elem_at(p))), &at)
            }
            _ => {
                let kind = if matches!(p.ty, Ty::Date) { Kind::Date } else { Kind::Int };
                self.py_obj_at(&kind, "_e", &of.text, &elem_at(p))
            }
        }
    }

    /// The collection a walk reads, as a list, in Python.
    fn py_obj_collection(&self, p: &Proj, root: &str) -> String {
        match &p.proto {
            Some(pp) => {
                let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("({}, {})", dq(j), dq(n))).collect();
                let tuple = if steps.len() == 1 { format!("({},)", steps[0]) } else { format!("({})", steps.join(", ")) };
                format!("_seq_of(_proto({root}, {tuple}, [], [], {}), {})", dq(&base_of(p)), dq(&p.shown))
            }
            None => {
                let (o, at) = self.walk_to_last(p, root, &|o, k, at| format!("_at({o}, {}, {})", dq(k), dq(at)));
                format!("_seq_at({o}, {}, {})", dq(p.path.last().map(String::as_str).unwrap_or("")), dq(&at))
            }
        }
    }

    /// One projected value in Python: an input's, or a field's of an element read from `_e`.
    pub(super) fn py_obj_read(&self, p: &Proj) -> String {
        // The shape's identifier, as the function's parameter is named (`Gen::proj_params`): not
        // looked up among the values', which a value of the shape's name would answer (§15.263).
        let root = p.root.clone();
        let pb = p.proto.is_some();
        match p.kind {
            ProjKind::Field => {
                let Some(kind) = kind_of(&p.ty, pb) else { return "None".into() };
                let opt = matches!(p.ty, Ty::Opt(_));
                let read = match &p.proto {
                    Some(pp) => {
                        let lit = |z: &Zero| pb_zero(z, "False", &dq);
                        let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("({}, {})", dq(j), dq(n))).collect();
                        let tuple = if steps.len() == 1 { format!("({},)", steps[0]) } else { format!("({})", steps.join(", ")) };
                        let (absent, unset) = self.proto_defaults(p, pp, &lit, "None");
                        let walk = format!("_proto({root}, {tuple}, {absent}, {unset}, {})", dq(&base_of(p)));
                        if opt { format!("(None if {walk} is None else {})", self.py_obj_branded(&p.ty, self.py_obj_of(&kind, &walk, &p.shown))) } else { self.py_obj_branded(&p.ty, self.py_obj_of(&kind, &walk, &p.shown)) }
                    }
                    None if opt => {
                        let keys: Vec<String> = p.path.iter().map(|s| dq(s)).collect();
                        let tuple = if keys.len() == 1 { format!("({},)", keys[0]) } else { format!("({})", keys.join(", ")) };
                        let walk = format!("_opt({root}, {tuple}, {})", dq(&base_of(p)));
                        format!("(None if {walk} is None else {})", self.py_obj_branded(&p.ty, self.py_obj_of(&kind, &walk, &p.shown)))
                    }
                    None => {
                        let (o, at) = self.walk_to_last(p, &root, &|o, k, at| format!("_at({o}, {}, {})", dq(k), dq(at)));
                        self.py_obj_branded(&p.ty, self.py_obj_at(&kind, &o, p.path.last().map(String::as_str).unwrap_or(""), &at))
                    }
                };
                read
            }
            ProjKind::Any(..) | ProjKind::All(..) | ProjKind::Count(_) | ProjKind::Sum(..) | ProjKind::Max(..) | ProjKind::Min(..) => {
                let seq = self.py_obj_collection(p, &root);
                let test = p.kind.test().map(|t| self.py_obj_test(p, t));
                match p.kind {
                    ProjKind::Any(..) => format!("_any([{} for _e in {seq}])", test.unwrap_or_else(|| "True".into())),
                    ProjKind::All(..) => format!("_all([{} for _e in {seq}])", test.unwrap_or_else(|| "True".into())),
                    ProjKind::Count(None) => format!("_len({seq})"),
                    ProjKind::Count(Some(_)) => format!("_sum(1 for _e in {seq} if {})", test.unwrap_or_else(|| "True".into())),
                    _ => {
                        let Some(of) = p.kind.of().cloned() else { return "0".into() };
                        let each = self.py_obj_of_field(p, &of);
                        let tail = match &test {
                            Some(t) => format!(" for _e in {seq} if {t}"),
                            None => format!(" for _e in {seq}"),
                        };
                        match p.kind {
                            ProjKind::Sum(..) => self.py_obj_branded(&p.ty, format!("_sum({each}{tail})")),
                            _ => self.py_obj_branded(&p.ty, format!("{}([{each}{tail}], {})", Self::extreme_helper(p.kind), self.proj_empty(p))),
                        }
                    }
                }
            }
        }
    }

    /// One sequence built out of the caller's object in Python (§15.223): an element of the
    /// rule's own type for every element of the collection, its fields read in the order declared.
    fn py_obj_sequence(&self, q: &SeqProj) -> String {
        let root = self.ident(&q.root);
        let seq = match &q.proto {
            Some(pp) => {
                let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("({}, {})", dq(j), dq(n))).collect();
                let tuple = if steps.len() == 1 { format!("({},)", steps[0]) } else { format!("({})", steps.join(", ")) };
                format!("_seq_of(_proto({root}, {tuple}, [], [], {}), {})", dq(&seq_base(q)), dq(&q.shown))
            }
            None => {
                let mut o = root;
                let mut at: String = q.shown.split('.').next().unwrap_or_default().to_string();
                for s in &q.path[..q.path.len().saturating_sub(1)] {
                    o = format!("_at({o}, {}, {})", dq(s), dq(&at));
                    at = format!("{at}.{s}");
                }
                format!("_seq_at({o}, {}, {})", dq(q.path.last().map(String::as_str).unwrap_or("")), dq(&at))
            }
        };
        let fields: Vec<String> = q.fields.iter().map(|p| format!("{}={}", self.elem_field_alias(q.s, &p.jp), self.py_obj_read(p))).collect();
        format!("[{}({}) for _e in {seq}]", self.element_type(q.s), fields.join(", "))
    }

    /// The helpers the Python projection function calls, the ones `body` uses and what they call.
    fn py_from_helpers(&self, body: &str) -> String {
        let s = says();
        let j = |p: &(String, String), at: &str| joined(p, at, "+", &dq);
        let mut want: Vec<&str> = Vec::new();
        let names = [
            "_int_at", "_bool_at", "_str_at", "_date_at", "_num_at", "_seq_at", "_int_pb", "_int_of", "_bool_of", "_str_of", "_date_of", "_num_of",
            "_seq_of", "_opt", "_pget", "_at",
        ];
        for n in names {
            if calls(body, n) {
                want.push(n);
            }
        }
        // What the fused readers and the protojson integer call.
        let add = |n: &'static str, want: &mut Vec<&str>| {
            if !want.contains(&n) {
                want.push(n);
            }
        };
        for (fused, of) in [("_int_at", "_int_of"), ("_bool_at", "_bool_of"), ("_str_at", "_str_of"), ("_date_at", "_date_of"), ("_num_at", "_num_of"), ("_seq_at", "_seq_of"), ("_int_pb", "_int_of")] {
            if want.contains(&fused) {
                add(of, &mut want);
                if fused.ends_with("_at") {
                    add("_at", &mut want);
                }
            }
        }
        let mut o = String::new();
        let at_key = "at + \".\" + k";
        if want.contains(&"_at") {
            o.push_str(&format!(
                "def _at(o: Any, k: str, at: str) -> Any:\n    d = cast(\"dict[str, Any]\", o)\n    if not isinstance(o, dict):\n        raise RuleInputError({}, _repr(o))\n    v: Any = d.get(k)\n    if v is None:\n        raise RuleInputError({})\n    return v\n\n\n",
                j(&s.object, "at"),
                j(&s.missing, at_key)
            ));
        }
        if want.contains(&"_opt") {
            o.push_str(&format!(
                "def _opt(o: Any, keys: tuple[str, ...], at: str) -> Any:\n    v: Any = o\n    for k in keys:\n        if v is None:\n            return None\n        d = cast(\"dict[str, Any]\", v)\n        if not isinstance(v, dict):\n            raise RuleInputError({}, _repr(v))\n        at = {at_key}\n        v = d.get(k)\n    return v\n\n\n",
                j(&s.object, "at")
            ));
        }
        if want.contains(&"_pget") {
            o.push_str(&format!(
                "def _pget(e: Any, j: str, n: str, absent: Any, at: str) -> Any:\n    d = cast(\"dict[str, Any]\", e)\n    if not isinstance(e, dict):\n        raise RuleInputError({}, _repr(e))\n    v: Any = d.get(j)\n    if v is None:\n        v = d.get(n)\n    return absent if v is None else v\n\n\n",
                j(&s.object, "at")
            ));
        }
        if want.contains(&"_seq_of") {
            o.push_str(&format!(
                "def _seq_of(v: Any, at: str) -> list[Any]:\n    seq = cast(\"list[Any]\", v)\n    if not isinstance(v, list):\n        raise RuleInputError({}, _repr(v))\n    return seq\n\n\n",
                j(&s.sequence, "at")
            ));
        }
        if want.contains(&"_int_of") {
            o.push_str(&format!(
                "def _int_of(v: Any, at: str) -> int:\n    if isinstance(v, float) and v.is_integer():\n        return int(v)\n    if not isinstance(v, int) or isinstance(v, bool):\n        raise RuleInputError({}, _repr(v))\n    return v\n\n\n",
                j(&s.integer, "at")
            ));
        }
        if want.contains(&"_int_pb") {
            o.push_str(&format!(
                "def _int_pb(v: Any, at: str) -> int:\n    if isinstance(v, str):\n        d = v[1:] if v.startswith(\"-\") else v\n        if d == \"\" or not all(c in \"0123456789\" for c in d):\n            raise RuleInputError({}, _repr(v))\n        return int(v)\n    return _int_of(v, at)\n\n\n",
                j(&s.integer, "at")
            ));
        }
        if want.contains(&"_bool_of") {
            o.push_str(&format!(
                "def _bool_of(v: Any, at: str) -> bool:\n    if not isinstance(v, bool):\n        raise RuleInputError({}, _repr(v))\n    return v\n\n\n",
                j(&s.boolean, "at")
            ));
        }
        if want.contains(&"_str_of") {
            o.push_str(&format!(
                "def _str_of(v: Any, at: str) -> str:\n    if not isinstance(v, str):\n        raise RuleInputError({}, _repr(v))\n    return v\n\n\n",
                j(&s.string, "at")
            ));
        }
        if want.contains(&"_num_of") {
            o.push_str(&format!(
                "def _num_of(v: Any, at: str) -> float:\n    if isinstance(v, bool) or not isinstance(v, (int, float)):\n        raise RuleInputError({}, _repr(v))\n    return v\n\n\n",
                j(&s.number, "at")
            ));
        }
        if want.contains(&"_date_of") {
            o.push_str(&format!(
                "def _date_of(v: Any, at: str) -> int:\n    if not isinstance(v, str) or len(v) != 10 or v[4] != \"-\" or v[7] != \"-\":\n        raise RuleInputError({said}, _repr(v))\n    if not all(c in \"0123456789\" for c in v[:4] + v[5:7] + v[8:]):\n        raise RuleInputError({said}, _repr(v))\n    y, m, d = int(v[:4]), int(v[5:7]), int(v[8:])\n    leap = y % 4 == 0 and (y % 100 != 0 or y % 400 == 0)\n    last = [31, 29 if leap else 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]\n    if not (1 <= m <= 12 and 1 <= d <= last[m - 1]):\n        raise RuleInputError({said}, _repr(v))\n    return _days(v)\n\n\n",
                said = j(&s.date, "at")
            ));
        }
        // A value of an enum: one reader per type the function reads, answering that type
        // (§15.244).
        for (ty, reader) in self.py_enum_readers() {
            if calls(body, &reader) {
                o.push_str(&format!(
                    "def {reader}(v: Any, what: str) -> {ty}:\n    if isinstance(v, str):\n        try:\n            return {ty}(v)\n        except ValueError:\n            pass\n    raise RuleInputError(what, _repr(v))\n\n\n"
                ));
            }
        }
        for (fused, of) in [("_int_at", "_int_of"), ("_bool_at", "_bool_of"), ("_str_at", "_str_of"), ("_date_at", "_date_of"), ("_num_at", "_num_of"), ("_seq_at", "_seq_of")] {
            if want.contains(&fused) {
                let ret = match of {
                    "_int_of" | "_date_of" => "int",
                    "_bool_of" => "bool",
                    "_str_of" => "str",
                    "_num_of" => "float",
                    _ => "list[Any]",
                };
                o.push_str(&format!("def {fused}(o: Any, k: str, at: str) -> {ret}:\n    return {of}(_at(o, k, at), {at_key})\n\n\n"));
            }
        }
        o
    }

    /// The Python projection function itself, with no helper.
    fn py_from_body(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut o = format!("def {alias}_from({}) -> {}:\n    \"\"\"{}\"\"\"\n    return {alias}(\n", self.py_from_params().join(", "), self.py_ret(), self.proj_doc());
        let projs = self.projections();
        for i in &self.f.inputs {
            let e = match projs.iter().find(|p| p.jp == i.name.text) {
                Some(p) => self.py_obj_read(p),
                None => pub_name(&i.name),
            };
            o.push_str(&format!("        {e},\n"));
        }
        let seqs = self.seq_projections("_e");
        for (s, el) in self.f.elements.iter().enumerate() {
            let e = match seqs.iter().find(|q| q.s == s) {
                Some(q) => self.py_obj_sequence(q),
                None => pub_name(&el.name),
            };
            o.push_str(&format!("        {e},\n"));
        }
        o.push_str("    )\n");
        o
    }

    /// The projection function in Python, with the helpers it calls above it (§15.125, §15.234).
    pub(super) fn py_from(&self) -> String {
        if !self.projects() {
            return String::new();
        }
        let body = self.py_from_body();
        let mut o = String::new();
        if calls(&body, "_proto") {
            // A place on the way that is not an object is refused, as `_at` refuses it; one that
            // is not there, or is `null`, is protojson's default (§15.234).
            o.push_str(&format!(
                "def _proto(o: Any, path: tuple[tuple[str, str], ...], absent: Any, unset: Any, at: str) -> Any:\n    \
                 for i, (j, p) in enumerate(path):\n        \
                 d = cast(\"dict[str, Any]\", o)\n        \
                 if not isinstance(o, dict):\n            \
                 raise RuleInputError({}, _repr(o))\n        \
                 v: Any = d.get(j, d.get(p))\n        \
                 if v is None:\n            \
                 return absent if i == len(path) - 1 else unset\n        \
                 o = v\n        \
                 at = at + \".\" + p\n    \
                 return o\n\n\n",
                joined(&says().object, "at", "+", &dq)
            ));
        }
        let helpers = self.py_from_helpers(&body);
        if calls(&helpers, "_days") || calls(&body, "_days") {
            o.push_str(
                "def _days(s: str) -> int:\n    \
                 y, m, d = (int(x) for x in s.split(\"-\"))\n    \
                 y -= 1 if m <= 2 else 0\n    \
                 era = (y if y >= 0 else y - 399) // 400\n    \
                 yoe = y - era * 400\n    \
                 doy = (153 * (m + (-3 if m > 2 else 9)) + 2) // 5 + d - 1\n    \
                 return era * 146097 + yoe * 365 + yoe // 4 - yoe // 100 + doy - 719468\n\n\n",
            );
        }
        o.push_str(&helpers);
        // The largest and the smallest value of a field over a collection, or the value for none
        // (§15.223). `max` and `min` here are Python's: these helpers hold no name of the rule's.
        if calls(&body, "_largest") {
            o.push_str("def _largest(vs: list[int], empty: int) -> int:\n    return max(vs) if len(vs) > 0 else empty\n\n\n");
        }
        if calls(&body, "_smallest") {
            o.push_str("def _smallest(vs: list[int], empty: int) -> int:\n    return min(vs) if len(vs) > 0 else empty\n\n\n");
        }
        o.push_str(&body);
        o
    }

    // --- TypeScript and JavaScript ----------------------------------------------------------

    /// A value read out of the object `o` at the key `k`, `at` naming `o`, held to `kind`.
    fn ts_obj_at(&self, kind: &Kind, o: &str, k: &str, at: &str) -> String {
        let (kq, aq) = (dq(k), dq(at));
        match kind {
            Kind::Int => format!("_intAt({o}, {kq}, {aq})"),
            Kind::IntPb => format!("_intPb(_at({o}, {kq}, {aq}), {})", dq(&format!("{at}.{k}"))),
            Kind::Bool => format!("_boolAt({o}, {kq}, {aq})"),
            Kind::Str => format!("_strAt({o}, {kq}, {aq})"),
            Kind::Date => format!("_dateAt({o}, {kq}, {aq})"),
            Kind::Num => format!("_numAt({o}, {kq}, {aq})"),
            Kind::Enum(n) => {
                let ty = self.read_enum_ty(n);
                format!("(_enumOf(Object.values({ty}), _at({o}, {kq}, {aq}), {}) as {ty})", dq(&crate::door::not_in_enum(&format!("{at}.{k}"), n)))
            }
        }
    }

    /// A value already read, `at` naming it, held to `kind` (TypeScript).
    fn ts_obj_of(&self, kind: &Kind, v: &str, at: &str) -> String {
        let aq = dq(at);
        match kind {
            Kind::Int => format!("_intOf({v}, {aq})"),
            Kind::IntPb => format!("_intPb({v}, {aq})"),
            Kind::Bool => format!("_boolOf({v}, {aq})"),
            Kind::Str => format!("_strOf({v}, {aq})"),
            Kind::Date => format!("_dateOf({v}, {aq})"),
            Kind::Num => format!("_numOf({v}, {aq})"),
            Kind::Enum(n) => {
                let ty = self.read_enum_ty(n);
                format!("(_enumOf(Object.values({ty}), {v}, {}) as {ty})", dq(&crate::door::not_in_enum(at, n)))
            }
        }
    }

    /// A checked whole number as the rule's brand of its type (TypeScript).
    fn ts_obj_branded(&self, ty: &Ty, e: String) -> String {
        match ty.present() {
            t @ (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) => format!("({e} as {})", brand_of(t)),
            _ => e,
        }
    }

    /// The test of an element a `where` makes, in TypeScript.
    fn ts_obj_test(&self, p: &Proj, t: (&crate::ast::Name, &Cell)) -> String {
        let (field, cell) = t;
        let syn = Syn { is: "===", isnt: "!==", and: "&&", or: "||", tru: "true", not: None };
        let (kind, v) = match &p.proto {
            Some(pp) => {
                let (j, n, z) = pp.elem.clone().unwrap_or((field.text.clone(), field.text.clone(), Zero::Str));
                let kind = test_kind(cell, None, Some(&z));
                let zero = pb_zero(&z, "false", &dq);
                (kind.clone(), self.ts_obj_of(&kind, &format!("_pget(_e, {}, {}, {zero}, {})", dq(&j), dq(&n), dq(&elem_at(p))), &format!("{}.{}", elem_at(p), field.text)))
            }
            None => {
                let kind = test_kind(cell, p.test_kind.as_ref(), None);
                (kind.clone(), self.ts_obj_at(&kind, "_e", &field.text, &elem_at(p)))
            }
        };
        // A whole number is a bigint, compared with a bigint literal; one compared with a number
        // that has a fraction is compared as a number.
        if matches!(kind, Kind::Int | Kind::IntPb) && !cell_whole(cell) {
            return elem_cond_with(cell, &format!("Number({v})"), &syn, &|l| raw_lit(l, "true", "false"));
        }
        elem_cond_with(cell, &v, &syn, &|l| self.read_lit(l, &kind, "ts"))
    }

    /// The value of an element `sum`, `max` and `min` read, in TypeScript.
    fn ts_obj_of_field(&self, p: &Proj, of: &crate::ast::Name) -> String {
        let at = format!("{}.{}", elem_at(p), of.text);
        match p.proto.as_ref().and_then(|pp| pp.of.clone()) {
            Some((j, n, z)) => {
                let kind = if matches!(p.ty, Ty::Date) { Kind::Date } else { Kind::IntPb };
                let zero = pb_zero(&z, "false", &dq);
                self.ts_obj_of(&kind, &format!("_pget(_e, {}, {}, {zero}, {})", dq(&j), dq(&n), dq(&elem_at(p))), &at)
            }
            None => {
                let kind = if matches!(p.ty, Ty::Date) { Kind::Date } else { Kind::Int };
                self.ts_obj_at(&kind, "_e", &of.text, &elem_at(p))
            }
        }
    }

    /// The collection a walk reads, as an array, in TypeScript.
    fn ts_obj_collection(&self, p: &Proj, root: &str) -> String {
        match &p.proto {
            Some(pp) => {
                let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", dq(j), dq(n))).collect();
                format!("_seqOf(_proto({root}, [{}], [], [], {}), {})", steps.join(", "), dq(&base_of(p)), dq(&p.shown))
            }
            None => {
                let (o, at) = self.walk_to_last(p, root, &|o, k, at| format!("_at({o}, {}, {})", dq(k), dq(at)));
                format!("_seqAt({o}, {}, {})", dq(p.path.last().map(String::as_str).unwrap_or("")), dq(&at))
            }
        }
    }

    /// One projected value in TypeScript: an input's, or a field's of an element read from `_e`.
    pub(super) fn ts_obj_read(&self, p: &Proj) -> String {
        let root = p.root.clone();
        let pb = p.proto.is_some();
        match p.kind {
            ProjKind::Field => {
                let Some(kind) = kind_of(&p.ty, pb) else { return "null".into() };
                let opt = matches!(p.ty, Ty::Opt(_));
                match &p.proto {
                    Some(pp) => {
                        let lit = |z: &Zero| pb_zero(z, "false", &dq);
                        let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", dq(j), dq(n))).collect();
                        let (absent, unset) = self.proto_defaults(p, pp, &lit, "null");
                        let walk = format!("_proto({root}, [{}], {absent}, {unset}, {})", steps.join(", "), dq(&base_of(p)));
                        let read = self.ts_obj_branded(&p.ty, self.ts_obj_of(&kind, &walk, &p.shown));
                        if opt { format!("({walk} == null ? null : {read})") } else { read }
                    }
                    None if opt => {
                        let keys: Vec<String> = p.path.iter().map(|s| dq(s)).collect();
                        let walk = format!("_opt({root}, [{}], {})", keys.join(", "), dq(&base_of(p)));
                        format!("({walk} == null ? null : {})", self.ts_obj_branded(&p.ty, self.ts_obj_of(&kind, &walk, &p.shown)))
                    }
                    None => {
                        let (o, at) = self.walk_to_last(p, &root, &|o, k, at| format!("_at({o}, {}, {})", dq(k), dq(at)));
                        self.ts_obj_branded(&p.ty, self.ts_obj_at(&kind, &o, p.path.last().map(String::as_str).unwrap_or(""), &at))
                    }
                }
            }
            _ => {
                let seq = self.ts_obj_collection(p, &root);
                let test = p.kind.test().map(|t| self.ts_obj_test(p, t));
                let t = || test.clone().unwrap_or_else(|| "true".into());
                match p.kind {
                    // Every element is tested, so that a value of the wrong kind after the one that
                    // settles it is refused here as in every other language.
                    // (`reduce` over `unknown[]` answers `unknown` to the type checker.)
                    ProjKind::Any(..) => format!("({seq}.reduce((_a, _e) => ({}) || _a, false) as boolean)", t()),
                    ProjKind::All(..) => format!("({seq}.reduce((_a, _e) => ({}) && _a, true) as boolean)", t()),
                    ProjKind::Count(None) => format!("BigInt({seq}.length)"),
                    ProjKind::Count(Some(_)) => format!("BigInt({seq}.filter((_e) => {}).length)", t()),
                    _ => {
                        let Some(of) = p.kind.of().cloned() else { return "0".into() };
                        let each = self.ts_obj_of_field(p, &of);
                        let vals = match &test {
                            Some(c) => format!("{seq}.flatMap((_e) => ({c} ? [{each}] : []))"),
                            None => format!("{seq}.map((_e) => {each})"),
                        };
                        match p.kind {
                            ProjKind::Sum(..) => self.ts_obj_branded(&p.ty, format!("{vals}.reduce((_s, _v) => _s + _v, 0n)")),
                            _ => self.ts_obj_branded(&p.ty, format!("{}({vals}, {}n)", Self::extreme_helper(p.kind), self.proj_empty(p))),
                        }
                    }
                }
            }
        }
    }

    /// One sequence built out of the caller's object in TypeScript (§15.223).
    fn ts_obj_sequence(&self, q: &SeqProj) -> String {
        let root = self.ident(&q.root);
        let seq = match &q.proto {
            Some(pp) => {
                let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", dq(j), dq(n))).collect();
                format!("_seqOf(_proto({root}, [{}], [], [], {}), {})", steps.join(", "), dq(&seq_base(q)), dq(&q.shown))
            }
            None => {
                let mut o = root;
                let mut at: String = q.shown.split('.').next().unwrap_or_default().to_string();
                for s in &q.path[..q.path.len().saturating_sub(1)] {
                    o = format!("_at({o}, {}, {})", dq(s), dq(&at));
                    at = format!("{at}.{s}");
                }
                format!("_seqAt({o}, {}, {})", dq(q.path.last().map(String::as_str).unwrap_or("")), dq(&at))
            }
        };
        let fields: Vec<String> = q.fields.iter().map(|p| format!("{}: {}", self.elem_field_alias(q.s, &p.jp), self.ts_obj_read(p))).collect();
        format!("{seq}.map((_e) => ({{ {} }}))", fields.join(", "))
    }

    /// The helpers the TypeScript projection function calls: the ones `body` uses and what they
    /// call. Each is a function of its own on one line of its head, which the JavaScript stripping
    /// reads (§15.36).
    fn ts_from_helpers(&self, body: &str) -> String {
        let s = says();
        let j = |p: &(String, String), at: &str| joined(p, at, "+", &dq);
        let names = [
            "_intAt", "_boolAt", "_strAt", "_dateAt", "_numAt", "_seqAt", "_intPb", "_intOf", "_boolOf", "_strOf", "_dateOf", "_numOf", "_seqOf", "_enumOf",
            "_opt", "_pget", "_at",
        ];
        let mut want: Vec<&str> = names.iter().copied().filter(|n| calls(body, n)).collect();
        let add = |n: &'static str, want: &mut Vec<&str>| {
            if !want.contains(&n) {
                want.push(n);
            }
        };
        for (fused, of) in [("_intAt", "_intOf"), ("_boolAt", "_boolOf"), ("_strAt", "_strOf"), ("_dateAt", "_dateOf"), ("_numAt", "_numOf"), ("_seqAt", "_seqOf"), ("_intPb", "_intOf")] {
            if want.contains(&fused) {
                add(of, &mut want);
                if fused.ends_with("At") {
                    add("_at", &mut want);
                }
            }
        }
        let at_key = "at + \".\" + k";
        let mut o = String::new();
        if want.contains(&"_at") {
            o.push_str(&format!(
                "function _at(o: unknown, k: string, at: string): unknown {{\n  if (typeof o !== \"object\" || o === null || Array.isArray(o)) {{\n    throw new RuleInputError({}, o);\n  }}\n  const v = (o as Record<string, unknown>)[k];\n  if (v === undefined || v === null) {{\n    throw new RuleInputError({});\n  }}\n  return v;\n}}\n\n",
                j(&s.object, "at"),
                j(&s.missing, at_key)
            ));
        }
        if want.contains(&"_opt") {
            o.push_str(&format!(
                "function _opt(o: unknown, keys: string[], at: string): unknown {{\n  let v: unknown = o;\n  let here: string = at;\n  for (const k of keys) {{\n    if (v === undefined || v === null) {{\n      return null;\n    }}\n    if (typeof v !== \"object\" || Array.isArray(v)) {{\n      throw new RuleInputError({}, v);\n    }}\n    here = here + \".\" + k;\n    v = (v as Record<string, unknown>)[k];\n  }}\n  return v ?? null;\n}}\n\n",
                j(&s.object, "here")
            ));
        }
        if want.contains(&"_pget") {
            o.push_str(&format!(
                "function _pget(e: unknown, j: string, n: string, absent: unknown, at: string): unknown {{\n  if (typeof e !== \"object\" || e === null || Array.isArray(e)) {{\n    throw new RuleInputError({}, e);\n  }}\n  const o = e as Record<string, unknown>;\n  return o[j] ?? o[n] ?? absent;\n}}\n\n",
                j(&s.object, "at")
            ));
        }
        if want.contains(&"_seqOf") {
            o.push_str(&format!(
                "function _seqOf(v: unknown, at: string): unknown[] {{\n  if (!Array.isArray(v)) {{\n    throw new RuleInputError({}, v);\n  }}\n  return v;\n}}\n\n",
                j(&s.sequence, "at")
            ));
        }
        if want.contains(&"_intOf") {
            o.push_str(&format!(
                "function _intOf(v: unknown, at: string): bigint {{\n  if (typeof v === \"bigint\") {{\n    return v;\n  }}\n  if (typeof v !== \"number\" || !Number.isInteger(v)) {{\n    throw new RuleInputError({}, v);\n  }}\n  return BigInt(v);\n}}\n\n",
                j(&s.integer, "at")
            ));
        }
        if want.contains(&"_intPb") {
            o.push_str(&format!(
                "function _intPb(v: unknown, at: string): bigint {{\n  if (typeof v === \"string\") {{\n    if (!/^-?[0-9]+$/.test(v)) {{\n      throw new RuleInputError({}, v);\n    }}\n    return BigInt(v);\n  }}\n  return _intOf(v, at);\n}}\n\n",
                j(&s.integer, "at")
            ));
        }
        if want.contains(&"_boolOf") {
            o.push_str(&format!(
                "function _boolOf(v: unknown, at: string): boolean {{\n  if (typeof v !== \"boolean\") {{\n    throw new RuleInputError({}, v);\n  }}\n  return v;\n}}\n\n",
                j(&s.boolean, "at")
            ));
        }
        if want.contains(&"_strOf") {
            o.push_str(&format!(
                "function _strOf(v: unknown, at: string): string {{\n  if (typeof v !== \"string\") {{\n    throw new RuleInputError({}, v);\n  }}\n  return v;\n}}\n\n",
                j(&s.string, "at")
            ));
        }
        if want.contains(&"_numOf") {
            o.push_str(&format!(
                "function _numOf(v: unknown, at: string): number {{\n  if (typeof v === \"bigint\") {{\n    return Number(v);\n  }}\n  if (typeof v !== \"number\") {{\n    throw new RuleInputError({}, v);\n  }}\n  return v;\n}}\n\n",
                j(&s.number, "at")
            ));
        }
        if want.contains(&"_dateOf") {
            o.push_str(&format!(
                "function _dateOf(v: unknown, at: string): bigint {{\n  if (typeof v !== \"string\" || !/^[0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}$/.test(v)) {{\n    throw new RuleInputError({said}, v);\n  }}\n  const [y, m, d] = v.split(\"-\").map(Number);\n  const leap = y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);\n  const last = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];\n  if (m < 1 || m > 12 || d < 1 || d > last[m - 1]) {{\n    throw new RuleInputError({said}, v);\n  }}\n  return _days(v);\n}}\n\n",
                said = j(&s.date, "at")
            ));
        }
        if want.contains(&"_enumOf") {
            o.push_str(
                "function _enumOf(values: readonly unknown[], v: unknown, what: string): unknown {\n  if (!values.includes(v)) {\n    throw new RuleInputError(what, v);\n  }\n  return v;\n}\n\n",
            );
        }
        for (fused, of, ret) in [("_intAt", "_intOf", "bigint"), ("_boolAt", "_boolOf", "boolean"), ("_strAt", "_strOf", "string"), ("_dateAt", "_dateOf", "bigint"), ("_numAt", "_numOf", "number"), ("_seqAt", "_seqOf", "unknown[]")] {
            if want.contains(&fused) {
                o.push_str(&format!("function {fused}(o: unknown, k: string, at: string): {ret} {{\n  return {of}(_at(o, k, at), {at_key});\n}}\n\n"));
            }
        }
        o
    }

    /// The projection function in TypeScript, with the helpers it calls above it (§15.125, §15.234).
    pub(super) fn ts_from(&self) -> String {
        if !self.projects() {
            return String::new();
        }
        let alias = pub_name(&self.f.name);
        let fname = self.ident(&alias);
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.ts_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let mut body = format!("/** {} */\nexport function {fname}_from({}): {ret} {{\n  return {fname}(\n", self.proj_doc(), self.ts_from_params().join(", "));
        let projs = self.projections();
        for i in &self.f.inputs {
            let e = match projs.iter().find(|p| p.jp == i.name.text) {
                Some(p) => self.ts_obj_read(p),
                None => pub_name(&i.name),
            };
            body.push_str(&format!("    {e},\n"));
        }
        let seqs = self.seq_projections("_e");
        for (s, el) in self.f.elements.iter().enumerate() {
            let e = match seqs.iter().find(|q| q.s == s) {
                Some(q) => self.ts_obj_sequence(q),
                None => pub_name(&el.name),
            };
            body.push_str(&format!("    {e},\n"));
        }
        body.push_str("  );\n}\n");
        let mut o = String::new();
        // The caller's object is read, never named: a type for it would be a domain object
        // model, and this tool does not make one (§15-6).
        o.push_str("type _Obj = { [k: string]: any };\n\n");
        if calls(&body, "_proto") {
            // A place on the way that is not an object is refused, as `_at` refuses it; one that
            // is not there, or is `null`, is protojson's default (§15.234).
            o.push_str(&format!(
                "function _proto(o: unknown, path: [string, string][], absent: unknown, unset: unknown, at: string): any {{\n  \
                 let v: any = o;\n  \
                 let here: string = at;\n  \
                 for (let i = 0; i < path.length; i++) {{\n    \
                 if (typeof v !== \"object\" || v === null || Array.isArray(v)) {{\n      \
                 throw new RuleInputError({}, v);\n    \
                 }}\n    \
                 const [j, p] = path[i];\n    \
                 const next = v[j] ?? v[p];\n    \
                 if (next == null) return i === path.length - 1 ? absent : unset;\n    \
                 v = next;\n    \
                 here = here + \".\" + p;\n  \
                 }}\n  \
                 return v;\n}}\n\n",
                joined(&says().object, "here", "+", &dq)
            ));
        }
        let helpers = self.ts_from_helpers(&body);
        if calls(&helpers, "_days") {
            o.push_str(
                "function _days(s: string): bigint {\n  \
                 const [y0, m, d] = s.split(\"-\").map(Number);\n  \
                 const y = m <= 2 ? y0 - 1 : y0;\n  \
                 const era = Math.floor((y >= 0 ? y : y - 399) / 400);\n  \
                 const yoe = y - era * 400;\n  \
                 const doy = Math.floor((153 * (m + (m > 2 ? -3 : 9)) + 2) / 5) + d - 1;\n  \
                 const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;\n  \
                 return BigInt(era * 146097 + doe - 719468);\n}\n\n",
            );
        }
        o.push_str(&helpers);
        // The largest and the smallest value of a field over a collection, or the value for none
        // (§15.223).
        for (name, op) in [("_largest", ">"), ("_smallest", "<")] {
            if calls(&body, name) {
                o.push_str(&format!(
                    "function {name}(vs: bigint[], empty: bigint): bigint {{\n  \
                     let m = empty;\n  \
                     let seen = false;\n  \
                     for (const v of vs) {{\n    \
                     if (!seen || v {op} m) {{\n      \
                     m = v;\n      \
                     seen = true;\n    \
                     }}\n  \
                     }}\n  \
                     return m;\n}}\n\n"
                ));
            }
        }
        o.push_str(&body);
        o
    }

    // --- Ruby -------------------------------------------------------------------------------

    /// A value read out of the object `o` at the key `k`, `at` naming `o`, held to `kind` (Ruby).
    fn rb_obj_at(&self, kind: &Kind, o: &str, k: &str, at: &str) -> String {
        let (kq, aq) = (dq(k), dq(at));
        match kind {
            Kind::Int => format!("_int_at({o}, {kq}, {aq})"),
            Kind::IntPb => format!("_int_pb(_at({o}, {kq}, {aq}), {})", dq(&format!("{at}.{k}"))),
            Kind::Bool => format!("_bool_at({o}, {kq}, {aq})"),
            Kind::Str => format!("_str_at({o}, {kq}, {aq})"),
            Kind::Date => format!("_date_at({o}, {kq}, {aq})"),
            Kind::Num => format!("_num_at({o}, {kq}, {aq})"),
            Kind::Enum(n) => format!(
                "_enum_of({}::ALL, _at({o}, {kq}, {aq}), {})",
                super::rb_const(&self.read_enum_ty(n)),
                dq(&crate::door::not_in_enum(&format!("{at}.{k}"), n))
            ),
        }
    }

    /// A value already read, `at` naming it, held to `kind` (Ruby).
    fn rb_obj_of(&self, kind: &Kind, v: &str, at: &str) -> String {
        let aq = dq(at);
        match kind {
            Kind::Int => format!("_int_of({v}, {aq})"),
            Kind::IntPb => format!("_int_pb({v}, {aq})"),
            Kind::Bool => format!("_bool_of({v}, {aq})"),
            Kind::Str => format!("_str_of({v}, {aq})"),
            Kind::Date => format!("_date_of({v}, {aq})"),
            Kind::Num => format!("_num_of({v}, {aq})"),
            Kind::Enum(n) => format!("_enum_of({}::ALL, {v}, {})", super::rb_const(&self.read_enum_ty(n)), dq(&crate::door::not_in_enum(at, n))),
        }
    }

    /// The test of an element a `where` makes, in Ruby.
    fn rb_obj_test(&self, p: &Proj, t: (&crate::ast::Name, &Cell)) -> String {
        let (field, cell) = t;
        let syn = Syn { is: "==", isnt: "!=", and: "&&", or: "||", tru: "true", not: None };
        let (kind, v) = match &p.proto {
            Some(pp) => {
                let (j, n, z) = pp.elem.clone().unwrap_or((field.text.clone(), field.text.clone(), Zero::Str));
                let kind = test_kind(cell, None, Some(&z));
                let zero = pb_zero(&z, "false", &dq);
                (kind.clone(), self.rb_obj_of(&kind, &format!("_pget(e, {}, {}, {zero}, {})", dq(&j), dq(&n), dq(&elem_at(p))), &format!("{}.{}", elem_at(p), field.text)))
            }
            None => {
                let kind = test_kind(cell, p.test_kind.as_ref(), None);
                (kind.clone(), self.rb_obj_at(&kind, "e", &field.text, &elem_at(p)))
            }
        };
        elem_cond_with(cell, &v, &syn, &|l| self.read_lit(l, &kind, "rb"))
    }

    /// The value of an element `sum`, `max` and `min` read, in Ruby.
    fn rb_obj_of_field(&self, p: &Proj, of: &crate::ast::Name) -> String {
        let at = format!("{}.{}", elem_at(p), of.text);
        match p.proto.as_ref().and_then(|pp| pp.of.clone()) {
            Some((j, n, z)) => {
                let kind = if matches!(p.ty, Ty::Date) { Kind::Date } else { Kind::IntPb };
                let zero = pb_zero(&z, "false", &dq);
                self.rb_obj_of(&kind, &format!("_pget(e, {}, {}, {zero}, {})", dq(&j), dq(&n), dq(&elem_at(p))), &at)
            }
            None => {
                let kind = if matches!(p.ty, Ty::Date) { Kind::Date } else { Kind::Int };
                self.rb_obj_at(&kind, "e", &of.text, &elem_at(p))
            }
        }
    }

    /// The collection a walk reads, as an array, in Ruby.
    fn rb_obj_collection(&self, p: &Proj, root: &str) -> String {
        match &p.proto {
            Some(pp) => {
                let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", dq(j), dq(n))).collect();
                format!("_seq_of(_proto({root}, [{}], Array.new, Array.new, {}), {})", steps.join(", "), dq(&base_of(p)), dq(&p.shown))
            }
            None => {
                let (o, at) = self.walk_to_last(p, root, &|o, k, at| format!("_at({o}, {}, {})", dq(k), dq(at)));
                format!("_seq_at({o}, {}, {})", dq(p.path.last().map(String::as_str).unwrap_or("")), dq(&at))
            }
        }
    }

    /// One projected value in Ruby: an input's, or a field's of an element read from `e`.
    pub(super) fn rb_obj_read(&self, p: &Proj) -> String {
        let root = p.root.clone();
        let pb = p.proto.is_some();
        match p.kind {
            ProjKind::Field => {
                let Some(kind) = kind_of(&p.ty, pb) else { return "nil".into() };
                let opt = matches!(p.ty, Ty::Opt(_));
                match &p.proto {
                    Some(pp) => {
                        let lit = |z: &Zero| match z {
                            Zero::List => "Array.new".to_string(),
                            z => pb_zero(z, "false", &dq),
                        };
                        let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", dq(j), dq(n))).collect();
                        let (absent, unset) = self.proto_defaults(p, pp, &lit, "nil");
                        let walk = format!("_proto({root}, [{}], {absent}, {unset}, {})", steps.join(", "), dq(&base_of(p)));
                        let read = self.rb_obj_of(&kind, &walk, &p.shown);
                        if opt { format!("({walk}.nil? ? nil : {read})") } else { read }
                    }
                    None if opt => {
                        let keys: Vec<String> = p.path.iter().map(|s| dq(s)).collect();
                        let walk = format!("_opt({root}, [{}], {})", keys.join(", "), dq(&base_of(p)));
                        format!("({walk}.nil? ? nil : {})", self.rb_obj_of(&kind, &walk, &p.shown))
                    }
                    None => {
                        let (o, at) = self.walk_to_last(p, &root, &|o, k, at| format!("_at({o}, {}, {})", dq(k), dq(at)));
                        self.rb_obj_at(&kind, &o, p.path.last().map(String::as_str).unwrap_or(""), &at)
                    }
                }
            }
            _ => {
                let seq = self.rb_obj_collection(p, &root);
                let test = p.kind.test().map(|t| self.rb_obj_test(p, t));
                let t = || test.clone().unwrap_or_else(|| "true".into());
                match p.kind {
                    // Every element is tested, as in every other language.
                    ProjKind::Any(..) => format!("{seq}.reduce(false) {{ |a, e| ({}) || a }}", t()),
                    ProjKind::All(..) => format!("{seq}.reduce(true) {{ |a, e| ({}) && a }}", t()),
                    ProjKind::Count(None) => format!("{seq}.length"),
                    ProjKind::Count(Some(_)) => format!("{seq}.count {{ |e| {} }}", t()),
                    _ => {
                        let Some(of) = p.kind.of().cloned() else { return "0".into() };
                        let each = self.rb_obj_of_field(p, &of);
                        let vals = match &test {
                            Some(c) => format!("{seq}.flat_map {{ |e| {c} ? [{each}] : [] }}"),
                            None => format!("{seq}.map {{ |e| {each} }}"),
                        };
                        match p.kind {
                            ProjKind::Sum(..) => format!("{vals}.sum"),
                            ProjKind::Max(..) => format!("({vals}.max || {})", self.proj_empty(p)),
                            _ => format!("({vals}.min || {})", self.proj_empty(p)),
                        }
                    }
                }
            }
        }
    }

    /// One sequence built out of the caller's object in Ruby (§15.223).
    fn rb_obj_sequence(&self, q: &SeqProj) -> String {
        let root = self.ident(&q.root);
        let seq = match &q.proto {
            Some(pp) => {
                let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", dq(j), dq(n))).collect();
                format!("_seq_of(_proto({root}, [{}], Array.new, Array.new, {}), {})", steps.join(", "), dq(&seq_base(q)), dq(&q.shown))
            }
            None => {
                let mut o = root;
                let mut at: String = q.shown.split('.').next().unwrap_or_default().to_string();
                for s in &q.path[..q.path.len().saturating_sub(1)] {
                    o = format!("_at({o}, {}, {})", dq(s), dq(&at));
                    at = format!("{at}.{s}");
                }
                format!("_seq_at({o}, {}, {})", dq(q.path.last().map(String::as_str).unwrap_or("")), dq(&at))
            }
        };
        let fields: Vec<String> = q.fields.iter().map(|p| self.rb_obj_read(p)).collect();
        format!("{seq}.map {{ |e| {}.new({}) }}", self.element_type(q.s), fields.join(", "))
    }

    /// The names of the Ruby helpers `body` calls and the ones they call, in the order written.
    fn rb_from_wanted(body: &str) -> Vec<&'static str> {
        let names = [
            "_int_at", "_bool_at", "_str_at", "_date_at", "_num_at", "_seq_at", "_int_pb", "_int_of", "_bool_of", "_str_of", "_date_of", "_num_of",
            "_seq_of", "_enum_of", "_opt", "_pget", "_at",
        ];
        let mut want: Vec<&'static str> = names.iter().copied().filter(|n| calls(body, n)).collect();
        for (fused, of) in [("_int_at", "_int_of"), ("_bool_at", "_bool_of"), ("_str_at", "_str_of"), ("_date_at", "_date_of"), ("_num_at", "_num_of"), ("_seq_at", "_seq_of"), ("_int_pb", "_int_of")] {
            if want.contains(&fused) {
                if !want.contains(&of) {
                    want.push(of);
                }
                if fused.ends_with("_at") && !want.contains(&"_at") {
                    want.push("_at");
                }
            }
        }
        want
    }

    /// The helpers the Ruby projection method calls, as methods of the module (§15.234).
    fn rb_from_helpers(&self, body: &str) -> String {
        let s = says();
        let j = |p: &(String, String), at: &str| joined(p, at, "+", &dq);
        let want = Self::rb_from_wanted(body);
        let at_key = "at + \".\" + k";
        let mut o = String::new();
        if want.contains(&"_at") {
            o.push_str(&format!(
                "  def self._at(o, k, at)\n    raise RuleInputError.new({}, o) unless o.is_a?(Hash)\n    v = o[k]\n    raise RuleInputError.new({}) if v.nil?\n    v\n  end\n\n",
                j(&s.object, "at"),
                j(&s.missing, at_key)
            ));
        }
        if want.contains(&"_opt") {
            o.push_str(&format!(
                "  def self._opt(o, keys, at)\n    v = o\n    here = at\n    keys.each do |k|\n      return nil if v.nil?\n      raise RuleInputError.new({}, v) unless v.is_a?(Hash)\n      here = here + \".\" + k\n      v = v[k]\n    end\n    v\n  end\n\n",
                j(&s.object, "here")
            ));
        }
        if want.contains(&"_pget") {
            o.push_str(&format!(
                "  def self._pget(e, j, n, absent, at)\n    raise RuleInputError.new({}, e) unless e.is_a?(Hash)\n    v = e[j]\n    v = e[n] if v.nil?\n    v.nil? ? absent : v\n  end\n\n",
                j(&s.object, "at")
            ));
        }
        if want.contains(&"_seq_of") {
            o.push_str(&format!("  def self._seq_of(v, at)\n    raise RuleInputError.new({}, v) unless v.is_a?(Array)\n    v\n  end\n\n", j(&s.sequence, "at")));
        }
        if want.contains(&"_int_of") {
            o.push_str(&format!(
                "  def self._int_of(v, at)\n    return v.to_i if v.is_a?(Float) && v.finite? && v == v.floor\n    raise RuleInputError.new({}, v) unless v.is_a?(Integer)\n    v\n  end\n\n",
                j(&s.integer, "at")
            ));
        }
        if want.contains(&"_int_pb") {
            o.push_str(&format!(
                "  def self._int_pb(v, at)\n    if v.is_a?(String)\n      raise RuleInputError.new({}, v) unless v.match?(/\\A-?[0-9]+\\z/)\n      return v.to_i\n    end\n    _int_of(v, at)\n  end\n\n",
                j(&s.integer, "at")
            ));
        }
        if want.contains(&"_bool_of") {
            o.push_str(&format!("  def self._bool_of(v, at)\n    raise RuleInputError.new({}, v) unless v == true || v == false\n    v\n  end\n\n", j(&s.boolean, "at")));
        }
        if want.contains(&"_str_of") {
            o.push_str(&format!("  def self._str_of(v, at)\n    raise RuleInputError.new({}, v) unless v.is_a?(String)\n    v\n  end\n\n", j(&s.string, "at")));
        }
        if want.contains(&"_num_of") {
            o.push_str(&format!("  def self._num_of(v, at)\n    raise RuleInputError.new({}, v) unless v.is_a?(Integer) || v.is_a?(Float)\n    v\n  end\n\n", j(&s.number, "at")));
        }
        if want.contains(&"_date_of") {
            o.push_str(&format!(
                "  def self._date_of(v, at)\n    raise RuleInputError.new({said}, v) unless v.is_a?(String) && v.match?(/\\A[0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}\\z/)\n    y, m, d = v.split(\"-\").map(&:to_i)\n    leap = (y % 4).zero? && (!(y % 100).zero? || (y % 400).zero?)\n    last = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]\n    raise RuleInputError.new({said}, v) unless m.between?(1, 12) && d.between?(1, last[m - 1])\n    _days(v)\n  end\n\n",
                said = j(&s.date, "at")
            ));
        }
        if want.contains(&"_enum_of") {
            o.push_str("  def self._enum_of(values, v, what)\n    raise RuleInputError.new(what, v) unless values.include?(v)\n    v\n  end\n\n");
        }
        for (fused, of) in [("_int_at", "_int_of"), ("_bool_at", "_bool_of"), ("_str_at", "_str_of"), ("_date_at", "_date_of"), ("_num_at", "_num_of"), ("_seq_at", "_seq_of")] {
            if want.contains(&fused) {
                o.push_str(&format!("  def self.{fused}(o, k, at)\n    {of}(_at(o, k, at), {at_key})\n  end\n\n"));
            }
        }
        o
    }

    /// The signatures of the Ruby projection method's helpers, for the module's `.rbs`.
    pub(super) fn rbs_from_helpers(&self) -> String {
        let body = self.rb_from_body();
        let mut o = String::new();
        if calls(&body, "_proto") {
            o.push_str("  private def self._proto: (untyped, Array[[String, String]], untyped, untyped, String) -> untyped\n");
        }
        let helpers = self.rb_from_helpers(&body);
        if calls(&helpers, "_days") {
            o.push_str("  private def self._days: (String) -> Integer\n");
        }
        for n in Self::rb_from_wanted(&body) {
            let sig = match n {
                "_at" => "(untyped, String, String) -> untyped",
                "_opt" => "(untyped, Array[String], String) -> untyped",
                "_pget" => "(untyped, String, String, untyped, String) -> untyped",
                "_seq_of" => "(untyped, String) -> Array[untyped]",
                "_seq_at" => "(untyped, String, String) -> Array[untyped]",
                "_int_of" | "_int_pb" | "_date_of" => "(untyped, String) -> Integer",
                "_int_at" | "_date_at" => "(untyped, String, String) -> Integer",
                "_bool_of" => "(untyped, String) -> bool",
                "_bool_at" => "(untyped, String, String) -> bool",
                "_str_of" => "(untyped, String) -> String",
                "_str_at" => "(untyped, String, String) -> String",
                "_num_of" => "(untyped, String) -> (Integer | Float)",
                "_num_at" => "(untyped, String, String) -> (Integer | Float)",
                "_enum_of" => "(Array[untyped], untyped, String) -> untyped",
                _ => continue,
            };
            o.push_str(&format!("  private def self.{n}: {sig}\n"));
        }
        o
    }

    /// The Ruby projection method itself, with no helper.
    fn rb_from_body(&self) -> String {
        let alias = pub_name(&self.f.name);
        let fname = self.ident(&alias);
        let mut o = format!("  # {}\n  def self.{fname}_from({})\n    {fname}(\n", self.proj_doc(), self.plain_from_params().join(", "));
        let projs = self.projections();
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            args.push(match projs.iter().find(|p| p.jp == i.name.text) {
                Some(p) => self.rb_obj_read(p),
                None => pub_name(&i.name),
            });
        }
        let seqs = self.seq_projections("e");
        for (s, el) in self.f.elements.iter().enumerate() {
            args.push(match seqs.iter().find(|q| q.s == s) {
                Some(q) => self.rb_obj_sequence(q),
                None => pub_name(&el.name),
            });
        }
        for a in &args {
            o.push_str(&format!("      {a},\n"));
        }
        o.push_str("    )\n  end\n");
        o
    }

    /// The projection method in Ruby, with the helpers it calls above it (§15.125, §15.234).
    pub(super) fn rb_from(&self) -> String {
        if !self.projects() {
            return String::new();
        }
        let body = self.rb_from_body();
        let mut o = String::new();
        if calls(&body, "_proto") {
            // A place on the way that is not an object is refused, as `_at` refuses it; one that
            // is not there, or is `null`, is protojson's default (§15.234).
            o.push_str(&format!(
                "  def self._proto(o, path, absent, unset, at)\n    \
                 path.each_with_index do |(j, p), i|\n      \
                 raise RuleInputError.new({}, o) unless o.is_a?(Hash)\n      \
                 v = o.key?(j) ? o[j] : o[p]\n      \
                 return (i == path.length - 1 ? absent : unset) if v.nil?\n      \
                 o = v\n      \
                 at = at + \".\" + p\n    \
                 end\n    \
                 o\n  \
                 end\n\n",
                joined(&says().object, "at", "+", &dq)
            ));
        }
        let helpers = self.rb_from_helpers(&body);
        if calls(&helpers, "_days") {
            o.push_str(
                "  def self._days(s)\n    \
                 y, m, d = s.split(\"-\").map(&:to_i)\n    \
                 y -= 1 if m <= 2\n    \
                 era = (y >= 0 ? y : y - 399) / 400\n    \
                 yoe = y - era * 400\n    \
                 doy = (153 * (m + (m > 2 ? -3 : 9)) + 2) / 5 + d - 1\n    \
                 era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468\n  \
                 end\n\n",
            );
        }
        o.push_str(&helpers);
        o.push_str(&body);
        o
    }
}
