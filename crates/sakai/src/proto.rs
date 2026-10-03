//! The `.proto` files of a map (DESIGN 4.2, PLAN B.5) are read by ritsu's one reader of them
//! (`ritsu-proto`, which began as this module): the syntax, the package, the imports with their
//! lines, the messages (nested ones by their dotted names), their fields, the enums and their
//! values with their numbers and lines, the services and their methods with the options written
//! on them (dandori's `(dandori.v1.workflow)` among them), and the names of types resolved by
//! protobuf's rule over the file and the files it imports. A proto2 `group` is refused (E106).
//!
//! What is sakai's is how a message or an enum is named (2 章) and which value of an enum says
//! that nothing is set (DESIGN 1.7).

pub use ritsu_proto::{Enum, EnumValue, Field, Import, Issue, Label, Message, Method, ProtoFile, Protos, ReadError, Resolved, SCALARS, Service, Symbol, Type, import_candidates, is_known, load, read};

/// How sakai names a message or an enum.
pub trait Naming {
    /// `proto "<file>" enum <name>` or `… message <name>`.
    fn naming(&self) -> crate::naming::Name;
}

impl Naming for Symbol {
    fn naming(&self) -> crate::naming::Name {
        crate::naming::Name::file(crate::naming::Tool::Proto, self.file.clone()).with(if self.is_enum { "enum" } else { "message" }, self.name.clone())
    }
}

/// The prefix buf asks the values of an enum to start with: the enum's name in upper snake case
/// and `_` (`PackingStatus` → `PACKING_STATUS_`, `HTTPMethod` → `HTTP_METHOD_`). A capital starts
/// a word when the letter before it is lower case, or when the letter after it is.
pub fn value_prefix(enum_name: &str) -> String {
    let short = enum_name.rsplit('.').next().unwrap_or(enum_name);
    let cs: Vec<char> = short.chars().collect();
    let mut out = String::new();
    for (i, c) in cs.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let before = cs[i - 1];
            let after = cs.get(i + 1).copied();
            if before.is_ascii_lowercase() || before.is_ascii_digit() || (before.is_ascii_uppercase() && after.is_some_and(|a| a.is_ascii_lowercase())) {
                out.push('_');
            }
        }
        out.push(c.to_ascii_uppercase());
    }
    out.push('_');
    out
}

/// Whether the value numbered 0 of an enum says no value is set (DESIGN 1.7): its name, with the
/// enum's prefix taken off, is `unspecified` (in any case), as with rulec's `import proto` and
/// dandori's types from a `.proto`. Any other value 0 (`HANDLING_STANDARD = 0`) is a value like
/// the rest.
pub fn is_unset(e: &Enum, v: &EnumValue) -> bool {
    if v.number != 0 {
        return false;
    }
    let rest = v.name.strip_prefix(&value_prefix(&e.name)).unwrap_or(&v.name);
    rest.eq_ignore_ascii_case("unspecified")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_value_that_says_nothing_is_set() {
        let e = |name: &str, vals: &[(&str, i64)]| Enum { name: name.into(), line: 1, values: vals.iter().map(|(n, x)| EnumValue { name: n.to_string(), number: *x, line: 1, options: vec![] }).collect(), options: vec![] };
        let os = e("OrderStatus", &[("ORDER_STATUS_UNSPECIFIED", 0)]);
        assert!(is_unset(&os, &os.values[0]));
        let st = e("Stock", &[("STOCK_UNSPECIFIED", 0)]);
        assert!(is_unset(&st, &st.values[0]));
        let dd = e("Stock", &[("unspecified", 0)]);
        assert!(is_unset(&dd, &dd.values[0]));
        let h = e("Handling", &[("HANDLING_STANDARD", 0)]);
        assert!(!is_unset(&h, &h.values[0]));
        assert_eq!(value_prefix("HTTPMethod"), "HTTP_METHOD_");
        assert_eq!(value_prefix("PackingStatus"), "PACKING_STATUS_");
    }
}
