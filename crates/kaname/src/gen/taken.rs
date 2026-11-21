//! The names of a module's code in a target, as one table made from its declarations (DESIGN 10.3).
//!
//! A host reads some of them: the module's types (a record, an enum, the type of a value of a sum),
//! Go's constants of the values of an enum, and its functions, each as the target spells the name
//! the source gives it (`foo_bar` is `FooBar`, Go's `Way.ab` is `WayAb`). The check says E006 of two
//! that a target spells alike, and E007 of one the target reads as something else
//! (`names::Dc::spelled`): no generated name moves a name a host reads. The rest the generated code
//! alone reads: each function's inner function (`_claim`), the check of what a host gives it (Rust's
//! `check_claim`, Go's `CheckClaim`), the type of its kinds of failure (`ClaimFails`), the checks of
//! the fields of a record and of a sum (`_outside_Line`, Go's methods `KanameOutside` and
//! `KanameEnter`), and Go's function whose name a type or a constant of the module has (`ClaimFn`).
//! Each is the name it is spelled by, unless a name the table holds already has it in its space: then
//! it takes a number after it (`_claim_2`, `check_claim2`, `ClaimFn2`). The table is made from the
//! module's declarations alone, so that each module that calls into another names the functions the
//! other declares. The names of one file alone (its imports, the constant of the module's name, the
//! helpers it makes) are the generator's ([`super::plan::Gen::new`]), which takes them past these.

use super::Target;
use super::plan::{RUST_STD_VALUES, field_name, go_camel, hidden, member_name, type_name, variant_name};
use crate::check::Checked;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// A space of the names of a module's code: TypeScript and Rust keep the names of types apart from
/// the names of values, and Python and Go name both in one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Space {
    Value,
    Type,
}

/// What a name of a module's code names, as far as the space it takes goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A record, a sum, the type of a value of a sum, TypeScript's type of a function's kinds of
    /// failure.
    Type,
    /// An enum whose values have no fields: TypeScript's frozen object and its type.
    Enum,
    /// A function, a constant.
    Value,
    /// A module the code imports: TypeScript's namespace (a value and a type), Rust's module.
    Import,
}

/// The spaces a name of the kind takes in the target.
pub fn spaces(t: Target, k: Kind) -> &'static [Space] {
    match (t, k) {
        (Target::Python | Target::Go, _) => &[Space::Value],
        (Target::TypeScript, Kind::Enum | Kind::Import) => &[Space::Value, Space::Type],
        (_, Kind::Value) => &[Space::Value],
        (Target::TypeScript | Target::Rust, _) => &[Space::Type],
    }
}

/// The names taken in the spaces of a module's code.
#[derive(Clone, Debug, Default)]
pub struct Taken(BTreeSet<(Space, String)>);

impl Taken {
    /// Whether a name of the kind would be one the spaces it takes already hold.
    pub fn has(&self, t: Target, k: Kind, n: &str) -> bool {
        spaces(t, k).iter().any(|s| self.0.contains(&(*s, n.to_string())))
    }

    pub fn add(&mut self, t: Target, k: Kind, n: &str) {
        for s in spaces(t, k) {
            self.0.insert((*s, n.to_string()));
        }
    }

    /// Every name, of either space.
    pub fn all(&self) -> BTreeSet<String> {
        self.0.iter().map(|(_, n)| n.clone()).collect()
    }

    /// `base`, or the first of `more(2)`, `more(3)`, … that the spaces of the kind do not hold and
    /// `also` does not hold either, taken.
    pub fn fresh(&mut self, t: Target, k: Kind, base: &str, more: impl Fn(usize) -> String, also: impl Fn(&str) -> bool) -> String {
        let mut n = base.to_string();
        let mut i = 2;
        while self.has(t, k, &n) || also(&n) {
            n = more(i);
            i += 1;
        }
        self.add(t, k, &n);
        n
    }
}

/// `base` with the number `i` after it, as the target writes a name the generated code makes: `_2`
/// after it, and in Go the number alone (Go writes no `_` in a name of several words).
pub fn numbered(t: Target, base: &str, i: usize) -> String {
    if t == Target::Go { format!("{base}{i}") } else { format!("{base}_{i}") }
}

/// A function's name as the target spells it, before Go moves it aside from a type: its alias
/// (TypeScript, Python), in Pascal case (Go, `total_weight` is `TotalWeight`), Rust's with `_` after
/// a value of the prelude ([`RUST_STD_VALUES`]) or a word no raw identifier writes.
pub fn fn_spelled(t: Target, alias: &str) -> String {
    match t {
        Target::Go => type_name(t, alias),
        Target::Rust if RUST_STD_VALUES.contains(&alias) => format!("{alias}_"),
        Target::Rust => ritsu_emit::ident::rust_name(alias),
        _ => alias.to_string(),
    }
}

/// The names of a module's code in a target that the module's declarations give.
#[derive(Clone, Debug, Default)]
pub struct Table {
    /// Each function's name, as a host calls it: Go's with `Fn` after it where a type or a constant
    /// of the module has its name (`fn claim` and `record Claim`).
    pub fns: Vec<String>,
    /// Each function's inner function, which the module's functions call past the check of what a
    /// host gives (DESIGN 4.4): `_claim`, Go's `_weightOf`.
    pub inner: Vec<String>,
    /// Each function's check of what a host gives it: Rust's `check_claim`, Go's `CheckClaim` (none
    /// in TypeScript and Python, and for a function no host calls).
    pub checks: Vec<String>,
    /// Each function's type of its kinds of failure (TypeScript, Python, DESIGN 4.5): `ClaimFails`.
    pub fails: Vec<String>,
    /// The checks of the fields of each record and each sum, by its name in the source: the one of
    /// what a parameter reaches and the one of what a host gives (DESIGN 28.1). TypeScript's and
    /// Python's functions `_outside_Line` and `_enter_Line`, Go's methods `KanameOutside` and
    /// `KanameEnter` (with a number after them where a field of the type has the name), Rust's
    /// methods `kaname_outside` and `kaname_enter` (Rust tells a method from a field).
    pub checks_of: BTreeMap<String, (String, String)>,
    /// Every name of the module's code above, in its spaces.
    pub taken: Taken,
}

/// The table of the module `c` in the target `t`, made once.
pub fn of(t: Target, c: &Checked) -> Rc<Table> {
    let i = Target::ALL.iter().position(|x| *x == t).unwrap_or(0);
    c.tables[i].get_or_init(|| Rc::new(make(t, c))).clone()
}

/// The table of the module `c` in the target `t`.
pub fn make(t: Target, c: &Checked) -> Table {
    let d = &c.decls;
    let mut tk = Taken::default();
    // the types, and Go's constants of the values of an enum
    for r in &d.records {
        tk.add(t, Kind::Type, &hidden(type_name(t, r.name.ascii()), r.private));
    }
    for e in &d.enums {
        if e.is_sum() {
            tk.add(t, Kind::Type, &hidden(type_name(t, e.name.ascii()), e.private));
            for v in &e.values {
                tk.add(t, Kind::Type, &hidden(variant_name(t, e.name.ascii(), v.ascii()), e.private));
            }
        } else {
            tk.add(t, Kind::Enum, &hidden(type_name(t, e.name.ascii()), e.private));
            if t == Target::Go {
                for v in &e.values {
                    tk.add(t, Kind::Value, &hidden(member_name(t, e.name.ascii(), v.ascii()), e.private));
                }
            }
        }
    }
    let n = d.fns.len();
    // the functions a host calls by name; in Go, every function not `private` (a generic function is
    // one function there, DESIGN 26.7)
    let named = |f: usize| if t == Target::Go { !d.fns[f].private } else { !d.fns[f].inner() };
    let spelled: Vec<String> = d.fns.iter().map(|i| fn_spelled(t, i.name.ascii())).collect();
    let mut fns = spelled.clone();
    if t == Target::Go {
        // the names as spelled first; then, with `Fn` after it, each a type or a constant has
        let moved: Vec<usize> = (0..n).filter(|&f| named(f) && tk.has(t, Kind::Value, &spelled[f])).collect();
        for f in (0..n).filter(|f| named(*f) && !moved.contains(f)) {
            tk.add(t, Kind::Value, &spelled[f]);
        }
        for &f in &moved {
            let b = format!("{}Fn", spelled[f]);
            fns[f] = tk.fresh(t, Kind::Value, &b, |i| format!("{b}{i}"), |_| false);
        }
    } else {
        for f in (0..n).filter(|f| named(*f)) {
            tk.add(t, Kind::Value, &spelled[f]);
        }
    }
    // the checks of the fields of the records and the sums
    let mut checks_of = BTreeMap::new();
    let typed = d.records.iter().map(|r| (&r.name, r.private, r.fields.iter().collect::<Vec<_>>())).chain(d.enums.iter().filter(|e| e.is_sum()).map(|e| (&e.name, e.private, e.fields.iter().flatten().collect::<Vec<_>>())));
    for (name, private, fields) in typed {
        let ty = hidden(type_name(t, name.ascii()), private);
        let pair = match t {
            Target::TypeScript | Target::Python => {
                let (o, e) = (format!("_outside_{ty}"), format!("_enter_{ty}"));
                (tk.fresh(t, Kind::Value, &o, |i| numbered(t, &o, i), |_| false), tk.fresh(t, Kind::Value, &e, |i| numbered(t, &e, i), |_| false))
            }
            // a method of Go's type, whose fields' names are its selectors too
            Target::Go => {
                let fs: BTreeSet<String> = fields.iter().map(|f| field_name(t, f.name.ascii())).collect();
                let pick = |b: &str| {
                    let mut m = b.to_string();
                    let mut i = 2;
                    while fs.contains(&m) {
                        m = format!("{b}{i}");
                        i += 1;
                    }
                    m
                };
                (pick("KanameOutside"), pick("KanameEnter"))
            }
            Target::Rust => ("kaname_outside".to_string(), "kaname_enter".to_string()),
        };
        checks_of.insert(name.text.clone(), pair);
    }
    // the inner functions; Python's function a host calls names the inner one in a body where the
    // parameters, which a host gives by their names, hide the module's names
    let mut inner = vec![String::new(); n];
    for (f, i) in d.fns.iter().enumerate() {
        let base = match t {
            Target::Go => format!("_{}", go_camel(i.name.ascii())),
            _ => format!("_{}", i.name.ascii()),
        };
        let given: BTreeSet<String> = if t == Target::Python && !i.inner() { i.params.iter().map(|p| p.name.ascii().to_string()).collect() } else { BTreeSet::new() };
        inner[f] = tk.fresh(t, Kind::Value, &base, |k| numbered(t, &base, k), |w| given.contains(w));
    }
    // the checks of what a host gives; Rust's apart from the name of every function as spelled
    // (a generic function's too, whose code is its inner function)
    let mut checks = vec![String::new(); n];
    for f in (0..n).filter(|&f| !d.fns[f].inner()) {
        let base = match t {
            Target::Rust => format!("check_{}", d.fns[f].name.ascii()),
            Target::Go => format!("Check{}", fns[f]),
            _ => continue,
        };
        checks[f] = tk.fresh(t, Kind::Value, &base, |k| format!("{base}{k}"), |w| t == Target::Rust && spelled.iter().any(|s| s == w));
    }
    // the types of the kinds of failure
    let mut fails = vec![String::new(); n];
    if matches!(t, Target::TypeScript | Target::Python) {
        for f in (0..n).filter(|&f| !d.fns[f].inner()) {
            let base = format!("{}Fails", ritsu_emit::ident::pascal(d.fns[f].name.ascii()));
            fails[f] = tk.fresh(t, Kind::Type, &base, |k| numbered(t, &base, k), |_| false);
        }
    }
    Table { fns, inner, checks, fails, checks_of, taken: tk }
}
