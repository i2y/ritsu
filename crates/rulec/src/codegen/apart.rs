//! The names of their own that the code of Java, PHP and SQL writes otherwise where a name of the
//! rule's would meet one, and the names the service of Connect imports under another (§15.336).
//! The other languages' are worked out in `clash.rs` (`Gen::apart`).
//!
//! What moves is always the generated code's: a name of the rule's that a caller sees — its
//! function, its class, its columns — keeps the name the rule gives it.

use super::spellings::Taken;
use super::{pascal, pub_name, Gen};
use std::collections::BTreeMap;

/// The names of its own the code of the backend `id` writes otherwise, as it writes them, with what
/// it writes instead, for the backends this module speaks for: none for the others.
pub(super) fn owns(g: &Gen, id: &str, _taken: &[Taken]) -> BTreeMap<String, String> {
    match id {
        "java" => java(g),
        "php" => php(g),
        "sql" => sql(g),
        _ => BTreeMap::new(),
    }
}

/// `name` with `_` after it, and another while `taken` says it is taken.
fn fresh(name: &str, taken: impl Fn(&str) -> bool) -> String {
    let mut n = format!("{name}_");
    while taken(&n) {
        n.push('_');
    }
    n
}

/// Java. The runner's class keeps the records it reads the wire with (`Str`, `Pairs`) and the type
/// its reader of an enum's value takes (`E`), and the rule's class the record the walk of a pick by
/// weight gives back (`Walked`), which no caller names: where the rule's class has the name of one
/// of them, which hides it there or which it may not hold, that one takes `_` after it. The class's
/// public types (`Fired`, `Traced`, `Output`, …) are names a caller writes, and W121 says those.
fn java(g: &Gen) -> BTreeMap<String, String> {
    let class = g.java_class();
    let mut own: Vec<&str> = vec!["Str", "Pairs", "E"];
    if g.weighted_fold().is_some() {
        own.push("Walked");
    }
    let mut out = BTreeMap::new();
    if own.contains(&class.as_str()) {
        out.insert(class.clone(), fresh(&class, |n| n == class || own.contains(&n)));
    }
    out
}

/// PHP. The module's namespace holds the rule's functions beside the functions of its own
/// (`_json_str`, `_round_up`, the readers of a projection), and the module calls the builtins it
/// uses by their bare names, which a function of the namespace of one of those names answers in
/// their place (PHP looks in the namespace first). Where one of the rule's functions has the name
/// of one of the module's, the module's takes `_` after it; where it has the name of a builtin the
/// module calls, the module calls the builtin by its full name (`\count`). PHP reads a function's
/// name with case set aside.
fn php(g: &Gen) -> BTreeMap<String, String> {
    let family = g.php_family();
    let rules = |n: &str| family.iter().any(|f| f.eq_ignore_ascii_case(n));
    let (declared, called) = g.php_names();
    let mut out = BTreeMap::new();
    for n in declared.iter().filter(|n| rules(n)) {
        out.insert(n.clone(), fresh(n, |x| rules(x) || declared.iter().chain(&called).any(|d| d.eq_ignore_ascii_case(x))));
    }
    for b in called.iter().filter(|b| rules(b)) {
        out.insert(b.clone(), format!("\\{b}"));
    }
    out
}

/// SQL. The query keeps columns of its own beside the rule's: the row's `_id`, the door's
/// `_input_error` and a contradiction's `_contradiction`, which a caller reads. Where an input or
/// an output — a column a caller reads too — is one with one of them, as SQLite reads a name (with
/// case set aside) or PostgreSQL (cut at 63 bytes), that one takes `_` after it. The columns of its
/// own no caller reads, and a value of the rule's that a caller does not read, are held apart in
/// the query itself (`Gen::sql_cols`).
fn sql(g: &Gen) -> BTreeMap<String, String> {
    if g.f.walks() {
        return BTreeMap::new();
    }
    let seen: Vec<String> = g.f.inputs.iter().map(|i| &i.name).chain(g.f.outputs.iter().map(|o| &o.name)).map(|n| g.written(&n.text)).collect();
    let own = ["_id", "_input_error", "_contradiction"];
    let mut out = BTreeMap::new();
    let mut moved: Vec<String> = Vec::new();
    for n in own {
        if !seen.iter().any(|s| super::sql::one_column(s, n)) {
            continue;
        }
        let to = fresh(n, |x| seen.iter().chain(&moved).any(|s| super::sql::one_column(s, x)) || own.contains(&x));
        moved.push(to.clone());
        out.insert(n.to_string(), to);
    }
    out
}

/// The names the service of Connect imports, each under the name it is read by there, where the
/// rule's service class (`<Rule>` and `<Rule>Sync`), which the module declares after them, has its
/// name: `_` after it (§15.336). The service is a module apart from the rule's Python module, so
/// these are not among the names `owns` gives the generator of Python. A builtin the service reads
/// that the class has the name of is read through `builtins` (`builtins.KeyError`): the class
/// would take its place in the module, and the `except` that catches it would not catch.
pub(super) fn connect_imports(g: &Gen) -> BTreeMap<String, String> {
    let service = pascal(&pub_name(&g.f.name));
    let classes = [service.clone(), format!("{service}Sync")];
    let mut out = BTreeMap::new();
    for n in CONNECT_TOP {
        if classes.iter().any(|c| c == n) {
            out.insert(n.to_string(), fresh(n, |x| classes.iter().any(|c| c == x) || CONNECT_TOP.contains(&x)));
        }
    }
    for n in CONNECT_BUILTINS {
        if classes.iter().any(|c| c == n) {
            out.insert(n.to_string(), format!("builtins.{n}"));
        }
    }
    out
}

/// The builtins the service of Connect reads by a name a class can have: what its readers of an
/// enum's value (`KeyError`) and of a date (`ValueError`) catch, and what its command line raises
/// (`SystemExit`).
pub(super) const CONNECT_BUILTINS: &[&str] = &["KeyError", "SystemExit", "ValueError"];

/// What the service of Connect imports or declares at its top besides the rule's classes, which its
/// code reads.
pub(super) const CONNECT_TOP: &[&str] = &[
    "Any",
    "CODECS",
    "Code",
    "ConnectError",
    "E",
    "IO",
    "Iterable",
    "Mapping",
    "RequestContext",
    "SOURCE_SHA256",
    "StartResponse",
    "TYPE_CHECKING",
    "TypeVar",
    "V",
    "WSGIApplication",
    "WSGIEnvironment",
    "WSGIServer",
];
