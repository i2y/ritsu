//! Java, the eleventh target (§15.78).
//!
//! The README has said "Java: planned" since the third backend went in (§15.13). This is
//! that promise, and the segment it reaches is the one the other ten do not: the premium
//! table, the fee schedule and the eligibility test that live in an insurer's or a bank's
//! JVM, next to the DMN engines this tool borrowed its vocabulary from.
//!
//! What it costs beyond a scripting backend is one runner: **Java has no JSON in its
//! standard library**, and the API that would give it one is an incubator proposed for a
//! release after the current LTS (JEP 540). But that bill was already paid once — the Rust
//! backend takes no crates, so its runner reads the wire with a hand-written reader, and
//! this is that reader in Java's spelling. Nothing else is needed: `javac` and `java` come
//! in every JDK, and there is no Maven, no Gradle and no dependency.
//!
//! The decisions:
//!
//! **`long` is int64.** E108 proves every intermediate fits there, and here the proof and
//! the machine word are the same thing — no `bigint`, no widening, no check at the door.
//! Overflow wraps rather than trapping, as it does in Go and Rust.
//!
//! **The floor is 17, not the newest LTS.** What a generated artifact has to decide is the
//! oldest release it runs on, not the newest that exists (§15.20 decided the same for Ruby
//! 3.x and 4.x). `--release 17` is passed to `javac` so that `rulec test` proves the floor
//! rather than assuming it, and the output uses nothing newer: enums, records, `List.of`.
//!
//! **No build tool.** `javac -d classes` then `java -cp classes` — the same promise the
//! Rust backend makes with `rustc` alone (§15.64), for the same reason.
//!
//! **Two things are pinned that would otherwise follow the machine.** `-encoding UTF-8`,
//! because a JDK before 18 reads source in the platform's charset and a Japanese identifier
//! would arrive as mojibake; and `Locale.ROOT` on every `String.format`, because `%04d` in a
//! locale with its own digits would not be the bytes the other ten targets write.

use super::{
    camel, cell_src, civil_needed, fired_doc, mode_fn, out_src, pascal, plain_doc,
    pub_name, raw_base, record_doc, round_cases, traced_doc, unparen, walked_doc, wire_of, Expr2, Gen, Lang,
    Phase, WalkStep, Wire,
};
use crate::ast::{Arm, Cell, CmpOp, Item, Lit, OutCell, Table};
use crate::num::{Rat, RoundMode};
use crate::types::Ty;
use std::collections::BTreeMap;

/// Every word Java reserves. All of them are lower case, so a PascalCase type name can
/// never collide; a method, a parameter or a field can.
const JAVA_KEYWORDS: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const",
    "continue", "default", "do", "double", "else", "enum", "extends", "final", "finally", "float",
    "for", "goto", "if", "implements", "import", "instanceof", "int", "interface", "long",
    "native", "new", "package", "private", "protected", "public", "return", "short", "static",
    "strictfp", "super", "switch", "synchronized", "this", "throw", "throws", "transient", "try",
    "void", "volatile", "while", "true", "false", "null", "var", "record", "yield", "sealed",
    "permits",
];

/// A name Java will accept. A keyword takes a trailing `_` — Java has no escape for one, as
/// Swift has backticks (§15.24) — and a name that starts with a digit takes a leading one.
fn java_id(s: &str) -> String {
    let mut n = s.to_string();
    if n.starts_with(|c: char| c.is_ascii_digit()) {
        n.insert(0, '_');
    }
    if JAVA_KEYWORDS.contains(&n.as_str()) {
        n.push('_');
    }
    n
}

/// The class a rule's module is, which is also the name of the file it has to live in.
pub fn java_class(alias: &str) -> String {
    java_id(&pascal(alias))
}

/// The same, for the places outside this file that have to spell a name the way the
/// generated Java spells it — `rulec api`, whose entry a caller reads instead of the code.
pub(super) fn java_name_pub(s: &str) -> String {
    java_name(s)
}

/// A method or a field: Java's own spelling of a name.
fn java_name(s: &str) -> String {
    java_id(&camel(s))
}

/// A string literal.
pub(super) fn java_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Text a rule holds, made safe in a Java comment (ritsu's DESIGN 9.2): on one line, since a `\r`
/// the lexer leaves in a string would end a `//` comment, and with its backslashes doubled, since
/// javac reads a `\uXXXX` in a comment before it reads the comment (JLS 3.3), and a `\u000a` in a
/// name would end the comment and make the rest Java.
fn jc(s: &str) -> String {
    ritsu_emit::header::for_unicode_comment(&ritsu_emit::header::one_line(s))
}

/// `"<name>"` and what follows it in a record (`:`, `:[`), as a Java string literal: the name
/// written as JSON first, so a `\` in it reads back as itself, then as Java, so its backslashes
/// are doubled and javac reads no `\uXXXX` in it.
fn json_key(name: &str, after: &str) -> String {
    java_str(&format!("{}{after}", crate::json::quote(name)))
}

/// Append `L` to every integer literal, and only to those.
///
/// A literal wider than int is a compile error in Java without the suffix, and a tariff in
/// 銭 or a rate held in hundred-thousandths passes 2^31 easily. The rule is the one
/// `bigint_literals` uses for TypeScript: a run of digits that touches a letter, `_` or `.`
/// on either side belongs to a name (`SizeClass.S60`, `項目1`) and is left alone.
fn long_literals(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let part = |c: char| c.is_alphanumeric() || c == '_' || c == '.';
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !part(b[i - 1])) {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let touches_name = i < b.len() && part(b[i]);
            out.extend(&b[start..i]);
            if !touches_name {
                out.push('L');
            }
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// The shared expression text as Java. `/` on two `long`s truncates toward zero, which is
/// what `//` means everywhere the generator emits it (§15.13), and `Math.min` / `Math.max`
/// are already in the language.
fn java_expr(s: &str) -> String {
    let t = super::arith::java_expr(s)
        .replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "roundDown(")
        .replace("_round_up(", "roundUp(")
        .replace("_round_half_down(", "roundHalfDown(")
        .replace("_round_half(", "roundHalf(")
        .replace("_round_bankers(", "roundBankers(")
        .replace("_min(", "Math.min(")
        .replace("_max(", "Math.max(");
    long_literals(&t)
}

/// The helper for one rounding mode, as the generated class spells it.
fn java_round_fn(m: RoundMode) -> String {
    format!("round{}", pascal(mode_fn(m)))
}

/// The five modes of §7.3, as members of whichever class holds them. The magnitude is
/// rounded and the sign put back, so a mode means here what it means in the other ten.
fn round_java(indent: &str) -> String {
    let body = format!(
        r#"
/** {down} */
private static long roundDown(long x, long g) {{
    long v = Math.abs(x) / g * g;
    return x < 0 ? -v : v;
}}

/** {up} */
private static long roundUp(long x, long g) {{
    long q = Math.abs(x) / g;
    long r = Math.abs(x) % g;
    long v = r == 0 ? q * g : (q + 1) * g;
    return x < 0 ? -v : v;
}}

/** {half} */
private static long roundHalf(long x, long g) {{
    long q = Math.abs(x) / g;
    long r = Math.abs(x) % g;
    long v = 2 * r >= g ? (q + 1) * g : q * g;
    return x < 0 ? -v : v;
}}

/** {half_down} */
private static long roundHalfDown(long x, long g) {{
    long q = Math.abs(x) / g;
    long r = Math.abs(x) % g;
    long v = 2 * r > g ? (q + 1) * g : q * g;
    return x < 0 ? -v : v;
}}

/** {bankers} */
private static long roundBankers(long x, long g) {{
    long q = Math.abs(x) / g;
    long r = Math.abs(x) % g;
    if (2 * r > g || (2 * r == g && q % 2 == 1)) {{
        q += 1;
    }}
    long v = q * g;
    return x < 0 ? -v : v;
}}
"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen -> -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen -> -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    );
    Gen::indent_block(body.trim_start_matches('\n'), indent)
}

/// The JSON reading the generated Java runner does, in the shape the Rust runner's reader
/// has (§15.13): one flat object as key → the value's source text, and a reader per wire
/// type. Java's `charAt` walks UTF-16 code units where Rust walks code points; every
/// delimiter compared here is ASCII, which no half of a surrogate pair can be, so the two
/// read the same bytes the same way.
const JAVA_JSON_HELPERS: &str = r##"    /** 読んだ文字列と、閉じ引用符の次の位置。 */
    private record Str(String text, int next) {
    }

    /** 読んだ組と、閉じ波括弧の次の位置。 */
    private record Pairs(Map<String, String> map, int next) {
    }

    /** ひとつの平たい JSON オブジェクトを、値は原文のまま組にして返す。 */
    private static Map<String, String> fields(String line) {
        int i = line.indexOf("\"in\"");
        return pairsFrom(line, i < 0 ? 0 : i).map();
    }

    private static Pairs pairsFrom(String b, int start) {
        Map<String, String> out = new LinkedHashMap<>();
        int i = start;
        while (i < b.length() && b.charAt(i) != '{') {
            i++;
        }
        i++;
        while (i < b.length() && b.charAt(i) != '}') {
            while (i < b.length() && b.charAt(i) != '"' && b.charAt(i) != '}') {
                i++;
            }
            if (i >= b.length() || b.charAt(i) == '}') {
                break;
            }
            Str k = stringAt(b, i);
            i = k.next();
            while (i < b.length() && b.charAt(i) != ':') {
                i++;
            }
            i++;
            while (i < b.length() && b.charAt(i) == ' ') {
                i++;
            }
            String v;
            char c = b.charAt(i);
            if (c == '"') {
                Str s = stringAt(b, i);
                v = "\"" + s.text();
                i = s.next();
            } else if (c == '[' || c == '{') {
                Str s = balanced(b, i);
                v = s.text();
                i = s.next();
            } else {
                int from = i;
                while (i < b.length() && b.charAt(i) != ',' && b.charAt(i) != '}') {
                    i++;
                }
                v = b.substring(from, i).trim();
            }
            out.put(k.text(), v);
        }
        return new Pairs(out, i + 1);
    }

    /** 括弧で囲まれた値ひとつの原文。引用符の中は数えない。 */
    private static Str balanced(String b, int i) {
        char open = b.charAt(i) == '[' ? '[' : '{';
        char close = open == '[' ? ']' : '}';
        int depth = 0;
        int j = i;
        while (j < b.length()) {
            if (b.charAt(j) == '"') {
                j = stringAt(b, j).next();
                continue;
            }
            if (b.charAt(j) == open) {
                depth++;
            }
            if (b.charAt(j) == close) {
                depth--;
                if (depth == 0) {
                    j++;
                    break;
                }
            }
            j++;
        }
        return new Str(b.substring(i, j), j);
    }

    private static Str stringAt(String b, int i) {
        StringBuilder s = new StringBuilder();
        int j = i + 1;
        while (j < b.length() && b.charAt(j) != '"') {
            if (b.charAt(j) == '\\' && j + 1 < b.length()) {
                j++;
                char c = b.charAt(j);
                s.append(c == 'n' ? '\n' : c == 't' ? '\t' : c);
            } else {
                s.append(b.charAt(j));
            }
            j++;
        }
        return new Str(s.toString(), j + 1);
    }

    private static String get(Map<String, String> d, String k) {
        return d.getOrDefault(k, "");
    }

    private static String s(Map<String, String> d, String k) {
        String v = get(d, k);
        return v.startsWith("\"") ? v.substring(1) : v;
    }

    /** 1970-01-01 からの通算日。rulec が使うのと同じ暦の計算。 */
    private static long ord(String v) {
        String[] p = v.split("-");
        long y = Long.parseLong(p[0]);
        long m = Long.parseLong(p[1]);
        long d = Long.parseLong(p[2]);
        long y2 = m <= 2 ? y - 1 : y;
        long era = (y2 >= 0 ? y2 : y2 - 399) / 400;
        long yoe = y2 - era * 400;
        long mp = (m + 9) % 12;
        long doy = (153 * mp + 2) / 5 + d - 1;
        long doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        return era * 146097 + doe - 719468;
    }
"##;

impl<'a> Gen<'a> {
    /// The class the module is, and the name of the file it lives in.
    pub fn java_class(&self) -> String {
        java_class(&pub_name(&self.f.name))
    }

    /// A name as the generated Java spells it.
    pub(super) fn java_name_of(&self, s: &str) -> String {
        java_name(s)
    }

    /// The public method's name.
    pub(super) fn java_fname(&self) -> String {
        java_name(&pub_name(&self.f.name))
    }

    /// The type Java declares for a value. Enums, strings, booleans and `long` are all
    /// expressible; the unit is not — Java has no zero-cost wrapper, so it is documented,
    /// as it is in Ruby and PHP.
    pub(super) fn java_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.java_enum(n),
            Ty::Bool => "boolean".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => self.java_boxed(t),
            _ => "long".into(),
        }
    }

    /// The same type where `null` has to fit: a value that can be absent is a reference.
    fn java_boxed(&self, ty: &Ty) -> String {
        match ty {
            Ty::Bool => "Boolean".into(),
            Ty::Enum(_) | Ty::Str | Ty::Opt(_) => self.java_ty(ty),
            _ => "Long".into(),
        }
    }

    /// How a value's type is written for a reader: the declared type, and the unit beside it
    /// where there is one.
    fn java_doc_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(_) | Ty::Bool | Ty::Str | Ty::Opt(_) => self.java_ty(ty),
            _ => format!("long  // {}", jc(&ty.to_string())),
        }
    }

    /// An enum value as the constant the class declares for it.
    pub(super) fn java_value(&self, v: &str, ty: &Ty) -> String {
        match (self.value_name(v, ty), ty.present()) {
            // A value of an `enums` file's enum is that file's class's (§15.212).
            (Some((_, alias)), Ty::Enum(en)) if self.c.owners.contains_key(en) => format!("{}.{}", self.java_enum(en), java_id(&alias.to_uppercase())),
            (Some((ty, alias)), _) => format!("{}.{}", java_class(ty), java_id(&alias.to_uppercase())),
            (None, _) => java_str(v),
        }
    }

    /// An enum's type as the rule's class names it: its own nested enum, or the one nested in
    /// the class of the `enums` file it is read from, which sits beside it (§15.212).
    pub(super) fn java_enum(&self, en: &str) -> String {
        let t = java_class(&self.enum_names.get(en).cloned().unwrap_or_else(|| "String".into()));
        match self.c.owners.get(en) {
            Some(o) => format!("{}.{t}", java_class(&pub_name(&o.module))),
            None => t,
        }
    }

    /// The same type as code outside the rule's class names it.
    pub(super) fn java_enum_outside(&self, en: &str) -> String {
        match self.c.owners.get(en) {
            Some(_) => self.java_enum(en),
            None => format!("{}.{}", self.java_class(), self.java_enum(en)),
        }
    }

    /// One enum as Java: a nested enum whose constants carry their wire spelling, in the order
    /// declared, with the display names when it has them (§15.211). Java's enums compare in
    /// the order their constants are declared, which is the order of an ordered enum
    /// (§15.210).
    pub(super) fn java_enum_decl(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let ty = java_class(ascii);
        let mut o = format!("    /** {} */\n    public enum {ty} {{\n", jc(jp));
        let labels = self.c.labels.get(jp);
        let cs: Vec<String> = vals
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let name = java_id(&self.value_names.get(&(jp.to_string(), v.clone())).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone()));
                match labels {
                    Some(ls) => format!("        {name}({}, {})", java_str(v), java_str(&ls.get(i).cloned().flatten().unwrap_or_else(|| v.clone()))),
                    None => format!("        {name}({})", java_str(v)),
                }
            })
            .collect();
        o.push_str(&format!("{};\n\n", cs.join(",\n")));
        let (field, ctor, init) = if labels.is_some() {
            (
                "        private final String wire;\n        private final String shown;\n\n".to_string(),
                format!("        {ty}(String wire, String shown) {{\n            this.wire = wire;\n            this.shown = shown;\n        }}\n\n"),
                format!(
                    "        /** {} */\n        public String label() {{\n            return shown;\n        }}\n\n",
                    tr!("表示名。表示名の無い値は値の名前。", "The display name: a value without one is shown by its name.")
                ),
            )
        } else {
            (
                "        private final String wire;\n\n".to_string(),
                format!("        {ty}(String wire) {{\n            this.wire = wire;\n        }}\n\n"),
                String::new(),
            )
        };
        o.push_str(&field);
        o.push_str(&ctor);
        o.push_str(&init);
        // A file of enums has no door of its own, and so no `RuleInputError` (§15.212).
        let error = if self.f.kind == crate::ast::FileKind::Enums { "IllegalArgumentException" } else { "RuleInputError" };
        o.push_str(&format!(
            "        /** {vd} */\n        public String value() {{\n            return wire;\n        }}\n\n        \
             /** {fd} */\n        public static {ty} from(String v) {{\n            \
             for ({ty} x : values()) {{\n                if (x.wire.equals(v)) {{\n                    \
             return x;\n                }}\n            }}\n            \
             throw new {error}({msg} + v);\n        }}\n    }}\n\n",
            vd = tr!("記録と、受け渡しする値に使う綴り。", "The spelling this member has on the wire and in a record."),
            fd = tr!("受け渡しする値の綴りから引く。無ければ入口でエラーにする。", "The member with this spelling on the wire; refused at the door when there is none."),
            msg = java_str(&tr!(
                "{jp} が列挙 {ascii} の値ではありません: ",
                "{jp} is not a value of enum {ascii}: "
            )),
        ));
        o
    }

    /// The helpers the functions call for the expressions over an ordered enum (§15.210).
    fn java_enum_helpers(&self) -> String {
        let mut o = String::new();
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            let (t, cls) = (self.enum_type(en), self.java_enum(en));
            if rank {
                o.push_str(&format!("    private static long _rank_{t}({cls} x) {{\n        return x.ordinal();\n    }}\n\n"));
            }
            if max {
                o.push_str(&format!("    private static {cls} _max_{t}({cls} a, {cls} b) {{\n        return a.compareTo(b) >= 0 ? a : b;\n    }}\n\n"));
            }
            if min {
                o.push_str(&format!("    private static {cls} _min_{t}({cls} a, {cls} b) {{\n        return a.compareTo(b) <= 0 ? a : b;\n    }}\n\n"));
            }
        }
        o
    }

    /// The constant that holds a group's members.
    fn java_group(&self, name: &str) -> String {
        format!("GROUP_{}", java_id(&self.ident(name)))
    }

    /// Render a cell as a Java condition. A don't-care yields None (no condition).
    pub(super) fn java_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let text = matches!(inner, Ty::Str);
        // An optional value is a `Long`, a `Boolean` or an enum reference, `null` when it is
        // absent. Comparing a `Long` or reading a `Boolean` unboxes it and throws, and so does
        // `List.of(…).contains(null)`: every test but `none` asks first that there is a value,
        // and `not:` holds of the absent value (§15.201).
        let optional = matches!(ty, Ty::Opt(_));
        let present = |c: String| -> String { if optional { format!("{var} != null && {c}") } else { c } };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.java_value(w, ty),
                Lit::Num(n) => format!("{}L", self.int_lit(n, inner, col_scale)),
                Lit::Date(y, m, d) => format!("{}L", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => java_str(s),
            }
        };
        // A string is compared with `equals`, and the literal goes first so that an absent
        // value is false rather than an exception. Everything else — an enum constant, a
        // `long`, a boolean — is `==`, which for those three is a value comparison.
        let eq = |l: &Lit| -> String {
            if text {
                format!("{}.equals({var})", lit(l))
            } else {
                format!("{var} == {}", lit(l))
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.java_value(m, ty)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("List.of({})", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} == null"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.startsWith({})", java_str(p)))
                .collect::<Vec<_>>()
                .join(" || "),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                present(format!("{}.contains({var})", self.java_group(w)))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE && optional => format!("Boolean.TRUE.equals({var})"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE && optional => format!("Boolean.FALSE.equals({var})"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            // `==` on a string literal's `equals` and on an enum reference is false of `null`.
            Cell::Lit(l) if text || matches!(inner, Ty::Enum(_)) => eq(l),
            Cell::Lit(l) => present(eq(l)),
            Cell::Set(ls) => present(format!("{}.contains({var})", members(ls))),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                let c = format!("!{}.contains({var})", self.java_group(w));
                if optional { format!("({var} == null || {c})") } else { c }
            }
            Cell::Not(ls) => {
                let c = format!("!{}.contains({var})", members(ls));
                if optional { format!("({var} == null || {c})") } else { c }
            }
            Cell::Cmp(cs) => present(
                cs.iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        format!("{var} {op} {}", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(" && "),
            ),
            // A comparison on an ordered enum: Java's enums compare in the order their constants
            // are declared (§15.210).
            Cell::Ranked { ops, .. } => present(
                ops.iter()
                    .map(|(o, v)| format!("{var}.compareTo({}) {} 0", self.java_value(v, ty), o.word()))
                    .collect::<Vec<_>>()
                    .join(" && "),
            ),
        })
    }

    /// The module: one class, one file, no package and no build file.
    pub fn java(&self) -> String {
        let cls = self.java_class();
        // Java reads a `\uXXXX` in a comment before it reads the comment (JLS 3.3), so a `\u000a`
        // in a cited path or URL would end the head's comment; the backslashes are doubled to make
        // any such escape inert (ritsu's DESIGN 9.2).
        let mut o = ritsu_emit::header::for_unicode_comment(&self.header("//"));
        o.push_str("\nimport java.util.ArrayList;\nimport java.util.List;\n\n");
        o.push_str(&format!(
            "/** {} */\npublic final class {cls} {{\n    private {cls}() {{\n    }}\n\n",
            jc(&tr!("規則 {} v{}", "Rule {} v{}", self.f.name.text, self.f.version))
        ));

        // Enums. Each constant carries the source name, which is also the wire value
        // (§10.2), so the runner converts in one call and keeps no table of its own. An enum
        // of an `enums` file is nested in that file's class, beside this one (§15.212).
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.java_enum_decl(&jp, &ascii));
        }
        o.push_str(&self.java_enum_helpers());
        // Which values need which before them (§15.222).
        o.push_str(&self.java_prereqs());

        // The machine this function is one step of (§15.148).
        if let Some((en, init, fins)) = self.machine_consts() {
            let ety = self.java_enum(&en);
            // The values carried along with the state, and where a case starts (§15.217).
            let mut more = String::new();
            for mc in self.more_carried() {
                let v = match mc.kind() {
                    super::MoreKind::Enum => self.java_value(&mc.word(), &mc.ty),
                    super::MoreKind::Bool => mc.bool_word().to_string(),
                    super::MoreKind::Number | super::MoreKind::Branded => format!("{}L", mc.int),
                };
                more.push_str(&format!(
                    "    /** {} */\n    public static final {} {} = {v};\n\n",
                    tr!("持ち越す {} の始まりの値。", "Where the carried {} starts.", mc.input),
                    self.java_ty(&mc.ty),
                    mc.upper()
                ));
            }
            o.push_str(&format!(
                "    /** {} */\n    public static final {ety} INITIAL = {};\n\n    \
                 /** {} */\n    public static final List<{ety}> FINAL = List.of({});\n\n{more}    \
                 /** {} */\n    public static boolean isFinal({ety} state) {{\n        return FINAL.contains(state);\n    }}\n\n",
                tr!("案件が始まる状態。", "The state a case starts in."),
                self.java_value(&init, &Ty::Enum(en.clone())),
                tr!("案件が終わる状態。", "The states a case ends in."),
                fins.iter().map(|v| self.java_value(v, &Ty::Enum(en.clone()))).collect::<Vec<_>>().join(", "),
                tr!("この状態で案件が終わっているか。", "Whether a case in this state has ended.")
            ));
        }

        o.push_str(&format!(
            "    /** {} */\n    public static final class RuleInputError extends IllegalArgumentException {{\n        \
             private static final long serialVersionUID = 1L;\n\n        \
             /** {} */\n        public final String what;\n        /** {} */\n        public final Long value;\n\n        \
             public RuleInputError(String what) {{\n            this(what, null);\n        }}\n\n        \
             public RuleInputError(String what, Long value) {{\n            \
             super(value == null ? what : what + \": \" + value);\n            \
             this.what = what;\n            this.value = value;\n        }}\n    }}\n\n",
            tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller."),
            tr!("何を受け付けなかったかを説明するメッセージ。", "The sentence that says what was refused."),
            tr!("受け付けなかった値。理由が値についてでなければ null。", "The value refused, or null when the refusal is not about one.")
        ));
        o.push_str(&format!(
            "    /** {} */\n    public static final class RuleContradictionError extends RuntimeException {{\n        \
             private static final long serialVersionUID = 1L;\n\n        \
             /** {} */\n        public final String what;\n\n        \
             public RuleContradictionError(String what) {{\n            super(what);\n            this.what = what;\n        }}\n    }}\n\n",
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller."),
            tr!("何が矛盾したかを説明するメッセージ。", "The sentence that says what contradicted.")
        ));
        o.push_str(&format!("    /** {} */\n    public record Fired(String table, int row, String label) {{\n    }}\n\n", fired_doc()));

        // What the traced twin gives back. Java has no tuple, and a pair with the concrete
        // type in it reads better at the call site than a generic one would.
        let ret = if self.f.outputs.len() == 1 {
            self.java_ty(&self.ty_of(&self.f.outputs[0].name.text))
        } else {
            "Output".into()
        };
        o.push_str(&format!(
            "    /** {} */\n    public record Traced({ret} value, List<Fired> trace) {{\n    }}\n\n",
            tr!("出力と、当てはまった行。", "What came out, and the rows that matched.")
        ));
        // What the walk of a rule that picks by weight gives back besides (§15.225).
        if self.weighted_fold().is_some() {
            o.push_str(&format!(
                "    /** {} */\n    record Walked({ret} value, List<Fired> trace, List<Long> cuts) {{\n    }}\n\n",
                tr!(
                    "出力と、当てはまった行と、重みで選ぶ腕が重みを足すたびの合計。",
                    "What came out, the rows that matched, and the running total of the weights each time the arm that picks by weight adds one."
                )
            ));
        }

        for g in &self.f.groups {
            let gty = self.group_ty(&g.name.text);
            let ms: Vec<String> = g.members.iter().map(|m| self.java_value(&m.text, &gty)).collect();
            let ety = match (&gty, g.members.first().and_then(|m| self.value_name(&m.text, &gty))) {
                (Ty::Enum(en), Some(_)) => self.java_enum(en),
                (_, Some((ty, _))) => java_class(ty),
                (_, None) => "String".into(),
            };
            o.push_str(&format!(
                "    /** {} */\n    private static final List<{ety}> {} = List.of({});\n\n",
                jc(&g.name.text),
                self.java_group(&g.name.text),
                ms.join(", ")
            ));
        }

        o.push_str(&round_java("    "));
        o.push_str(&Gen::indent_block(&super::arith::java_helpers(&super::arith::Uses::of(self.f)), "    "));
        o.push('\n');
        o.push_str(&self.java_fn());
        o.push('\n');
        o.push_str(&self.java_record());
        // The method that counts the answers (§15.225).
        if self.f.has_random() {
            o.push('\n');
            o.push_str(self.java_chances().trim_end());
            o.push('\n');
        }
        o.push_str("}\n");
        o
    }

    /// The rule's items as Java, at the base indentation (§15.56).
    fn java_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for (i, it) in self.f.items.iter().enumerate() {
            if !self.in_phase(i, it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!(
                        "        {} {} = {};  // {}\n",
                        self.java_ty(&self.ty_of(&d.name.text)),
                        java_name(&self.ident(&d.name.text)),
                        java_expr(unparen(&e.text)),
                        tr!("導出", "derived value"),
                    ));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!(
                        "        {} {} = {};  // {}\n",
                        self.java_ty(&self.ty_of(&d.name.text)),
                        java_name(&self.ident(&d.name.text)),
                        java_expr(unparen(&e.text)),
                        tr!("定義", "definition"),
                    ));
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.java_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// Every walk of a rule that walks sequences, in Java (§15.56, §15.58, §15.219). The shape is
    /// the one `py_walks` writes.
    fn java_walks(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            if let Some(cap) = self.count_cap_of(s) {
                let seq = java_name(&pub_name(&el.name));
                o.push_str(&format!("        if ({seq}.size() > {cap}) {{\n"));
                o.push_str(&format!(
                    "            throw new RuleInputError({}, (long) {seq}.size());\n        }}\n",
                    java_str(&crate::door::too_many(&el.name.text, cap))
                ));
            }
        }
        for step in self.walk_steps() {
            match step {
                // What a walk does not depend on is worked out once, before it (§15.208).
                WalkStep::Once(ix) => o.push_str(&self.java_items(outer, trace, Phase::Only(&ix))),
                WalkStep::Walk { seq, body, sums, .. } => o.push_str(&self.java_walk(seq, &body, &sums, outer, trace)),
            }
        }
        o
    }

    /// One walk over sequence `s`, in Java (§15.219).
    fn java_walk(&self, s: usize, body_items: &[usize], sums: &[usize], outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = &self.f.elements[s];
        let seq = java_name(&pub_name(&el.name));
        let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), java_name(&pub_name(&fd.name)))).collect();
        let e = java_name(&self.elem_var(s));
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}()"),
                None => outer(n),
            }
        };
        let mut o = String::new();
        let mut body = self.java_element_guards(s, &local);
        body.push_str(&self.java_items(&local, trace, Phase::Only(body_items)));
        let mut after = String::new();
        for &i in sums {
            match &self.f.items[i] {
                crate::ast::Item::Agg(d) if d.kind.extreme() => {
                    let (init, step) = self.java_extreme(d, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                }
                crate::ast::Item::Agg(d) => {
                    o.push_str(&format!("        long {} = 0;  // {}\n", java_name(&self.ident(&d.name.text)), self.agg_word(d)));
                    let v = local(&d.column.text);
                    if d.kind == crate::ast::AggKind::Sum {
                        body.push_str(&format!("        {} += {v};  // {}\n", java_name(&self.ident(&d.name.text)), jc(&d.name.text)));
                    } else {
                        let test = self.java_test(&v, &d.column.text, &d.value);
                        body.push_str(&format!(
                            "        if ({test}) {{  // {}\n            {} += 1;\n        }}\n",
                            jc(&d.name.text),
                            java_name(&self.ident(&d.name.text))
                        ));
                    }
                    if let Some((_, cap)) = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text) {
                        let n = java_name(&self.ident(&d.name.text));
                        after.push_str(&format!(
                            "        if ({n} > {cap}L) {{\n            throw new RuleInputError({}, {n});\n        }}\n",
                            java_str(&crate::door::out_of_range(&d.name.text)),
                        ));
                    }
                }
                crate::ast::Item::Fold(fold) => {
                    let (init, step, fin) = self.java_fold(i, fold, &seq, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                    after.push_str(&fin);
                }
                _ => {}
            }
        }
        o.push_str(&format!("        for ({} {e} : {seq}) {{\n", self.element_type(s)));
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("        }\n");
        o.push_str(&after);
        o
    }

    /// A test on a column of one element, in Java: `count`'s, and the `where` of `max` and `min`.
    fn java_test(&self, v: &str, col: &str, value: &Option<crate::ast::Name>) -> String {
        let ty = self.ty_of(col);
        match (&ty, value) {
            (Ty::Bool, Some(w)) if w.text == crate::kw::FALSE => format!("!{v}"),
            (Ty::Bool, _) => v.to_string(),
            (_, Some(w)) => format!("{v} == {}", self.java_value(&w.text, &ty)),
            (_, None) => v.to_string(),
        }
    }

    /// A `max` or a `min`, in Java (§15.219).
    fn java_extreme(&self, d: &crate::ast::AggDecl, local: &dyn Fn(&str) -> String) -> (String, String) {
        let n = java_name(&self.ident(&d.name.text));
        let set = java_name(&self.temp(&format!("{}Set", self.ident(&d.name.text))));
        let v = local(&d.column.text);
        let ty = self.ty_of(&d.column.text);
        let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
        let (jt, start, better) = match self.extreme_enum(d) {
            Some(en) => {
                let t = self.enum_type(&en);
                let w = self.extreme_empty_word(d).unwrap_or_default();
                (self.java_ty(&ty), self.java_value(&w, &ty), format!("_rank_{t}({v}) {op} _rank_{t}({n})"))
            }
            None => ("long".to_string(), format!("{}L", self.extreme_empty_int(d).unwrap_or_else(|| "0".into())), format!("{v} {op} {n}")),
        };
        let init = format!("        {jt} {n} = {start};  // {}\n        boolean {set} = false;\n", self.agg_word(d));
        let mut step = String::new();
        let pad = match &d.filter {
            Some((w, want)) => {
                step.push_str(&format!("        if ({}) {{  // {}\n", self.java_test(&local(&w.text), &w.text, want), jc(&d.name.text)));
                "    "
            }
            None => "",
        };
        step.push_str(&format!("{pad}        if (!{set} || {better}) {{\n{pad}            {n} = {v};\n{pad}            {set} = true;\n{pad}        }}\n"));
        if !pad.is_empty() {
            step.push_str("        }\n");
        }
        (init, step)
    }

    /// A fold, in Java (§15.56).
    fn java_fold(&self, i: usize, fold: &crate::ast::FoldDecl, seq: &str, local: &dyn Fn(&str) -> String) -> (String, String, String) {
        let names = self.fold_names(i);
        let (answer, stopped, done) = (java_name(&names.answer), java_name(&names.stopped), java_name(&names.done));
        let (taken, kept, best) = (java_name(&names.taken), java_name(&names.kept), java_name(&names.best));
        let (held, key) = (java_name(&names.held), java_name(&names.key));
        let alone = self.fold_alone(i);
        let with_held = |n: &str| -> String { if n == crate::kw::HELD { held.clone() } else { local(n) } };
        let (out_name, answer_ty) = self.fold_target(fold);
        let out_ty = self.java_ty(&answer_ty);
        let acc = self.java_boxed(&answer_ty);
        // An answer is read in the type the fold answers with: a value of its enum is that
        // enum's member (§15.208).
        let val = |x: &crate::ast::Expr, l: &dyn Fn(&str) -> String| -> String {
            match x {
                crate::ast::Expr::Lit(Lit::Word(w), _) if w != crate::kw::TRUE && w != crate::kw::FALSE => self.java_value(w, &answer_ty),
                _ => java_expr(unparen(&self.expr(x, l).text)),
            }
        };
        let text = |x: &Option<crate::ast::Expr>| -> String { x.as_ref().map(|x| val(x, &with_held)).unwrap_or_else(|| "0L".to_string()) };

        let mut init = String::new();
        init.push_str(&format!(
            "        {out_ty} {answer} = {};  // {}\n",
            text(&fold.empty),
            tr!("要素ゼロ件の答え", "the answer for no elements")
        ));
        init.push_str(&format!("        boolean {stopped} = false;\n"));
        if !alone {
            init.push_str(&format!("        boolean {done} = false;\n"));
        }
        init.push_str(&format!(
            "        {acc} {taken} = null;\n        {acc} {kept} = null;\n        Long {best} = null;\n"
        ));
        let weighted = fold.weighted();
        let draw = fold.draw.as_ref().map(|d| local(&d.text)).unwrap_or_default();
        let (total, weight, cuts) = (java_name(&names.total), java_name(&names.weight), java_name(&names.cuts));
        if weighted {
            init.push_str(&format!(
                "        long {total} = 0L;  // {}\n        List<Long> {cuts} = new ArrayList<>();\n",
                tr!("重みの合計", "the running total of the weights")
            ));
        }

        let mut arms = String::new();
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let is = self.java_value(&name.text, &self.ty_of(&fold.verdict));
            arms.push_str(&format!("        if ({v} == {is}) {{  // {}\n", jc(&name.text)));
            match arm {
                Arm::Next => arms.push_str("            // next\n"),
                Arm::Stop(None) if alone => arms.push_str("            break;\n"),
                Arm::Stop(None) => arms.push_str(&format!("            {done} = true;\n")),
                Arm::Stop(Some(x)) => {
                    arms.push_str(&format!("            {answer} = {};\n", val(x, local)));
                    if alone {
                        arms.push_str(&format!("            {stopped} = true;\n            break;\n"));
                    } else {
                        arms.push_str(&format!("            {stopped} = true;\n            {done} = true;\n"));
                    }
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        arms.push_str(&format!("            if ({taken} != null) {{\n"));
                        arms.push_str(&format!(
                            "                throw new RuleContradictionError({});\n            }}\n",
                            java_str(&tr!(
                                "畳み込み {}: take_unique に二件当たりました",
                                "fold {}: two elements matched a take_unique",
                                fold.verdict
                            ))
                        ));
                        arms.push_str(&format!("            {taken} = {};\n", val(expr, local)));
                    } else {
                        arms.push_str(&format!("            if ({taken} == null) {{\n"));
                        arms.push_str(&format!("                {taken} = {};\n            }}\n", val(expr, local)));
                    }
                }
                Arm::KeepMax { expr, key: k } => {
                    arms.push_str(&format!("            long {key} = {};\n", java_expr(unparen(&self.expr(k, local).text))));
                    arms.push_str(&format!("            if ({best} == null || {key} > {best}) {{\n"));
                    arms.push_str(&format!("                {best} = {key};\n"));
                    arms.push_str(&format!("                {kept} = {};\n            }}\n", val(expr, local)));
                }
                // The weight goes on the running total, and the element is taken where the drawn
                // value is below the total for the first time (§15.226).
                Arm::TakeWeighted { expr, weight: wx } => {
                    arms.push_str(&format!("            long {weight} = {};\n", java_expr(unparen(&self.expr(wx, local).text))));
                    arms.push_str(&format!("            {total} += {weight};\n            {cuts}.add({total});\n"));
                    arms.push_str(&format!("            if ({taken} == null && {draw} < {total}) {{\n"));
                    arms.push_str(&format!("                {taken} = {};\n            }}\n", val(expr, local)));
                }
            }
            arms.push_str("        }\n");
        }
        let step = if alone { arms } else { format!("        if (!{done}) {{  // {}\n{}        }}\n", jc(&fold.verdict), Self::indent_block(&arms, "    ")) };

        // The drawn value of a pick by weight is below the total of the weights, unless they add up
        // to 0 (§15.226).
        let mut fin = String::new();
        if weighted {
            let what = fold.draw.as_ref().map(|d| crate::door::draw_past(&d.text)).unwrap_or_default();
            fin.push_str(&format!(
                "        if ({total} > 0 && {draw} >= {total}) {{\n            throw new RuleInputError({}, {draw});\n        }}\n",
                java_str(&what)
            ));
        }
        fin.push_str(&format!("        if (!{seq}.isEmpty() && !{stopped}) {{\n"));
        fin.push_str(&format!(
            "            {out_ty} {held} = {taken} != null ? {taken} : ({kept} != null ? {kept} : {});\n",
            text(&fold.empty)
        ));
        fin.push_str(&format!("            {answer} = {};\n        }}\n", text(&fold.exhausted)));
        fin.push_str(&format!("        {out_ty} {} = {answer};\n", java_name(&self.ident(&out_name))));
        (init, step, fin)
    }

    /// The range guard of one element's fields, inside the walk. The kind of the value is
    /// the declared type's business here, so only the range is asked about.
    fn java_element_guards(&self, s: usize, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields(s) {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = super::Guarded::new(&mut o, optional.then(|| (format!("        if ({v} != null) {{\n"), "        }\n".to_string())), "    ");
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "        if ({v} < {}L || {v} > {}L) {{\n            throw new RuleInputError({}, {v});\n        }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        java_str(&crate::door::out_of_range(&i.name.text)),
                    ));
                }
            }
        }
        o
    }

    fn java_fn(&self) -> String {
        let fname = self.java_fname();
        let outs = &self.f.outputs;
        let mut o = String::new();

        // One element is a row of inputs, so it gets the same shape as `Output`.
        for (s, el) in self.f.elements.iter().enumerate() {
            let fs: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    format!("{} {}", self.java_ty(&self.ty_of(&fd.name.text)), java_name(&pub_name(&fd.name)))
                })
                .collect();
            o.push_str(&format!(
                "    /** {} */\n    public record {}({}) {{\n    }}\n\n",
                jc(&tr!("{} の一件", "one of {}", el.name.text)),
                self.element_type(s),
                fs.join(", ")
            ));
        }

        // Multiple outputs come back as one record; a single output is the value itself
        // (§8.5).
        if outs.len() > 1 {
            let fs: Vec<String> = outs
                .iter()
                .map(|od| format!("{} {}", self.java_ty(&self.ty_of(&od.name.text)), java_name(&pub_name(&od.name))))
                .collect();
            o.push_str(&format!("    public record Output({}) {{\n    }}\n\n", fs.join(", ")));
        }

        let traced = format!("{fname}Traced");
        let ret = if outs.len() == 1 {
            self.java_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{} {}", self.java_ty(&self.ty_of(&i.name.text)), java_name(&pub_name(&i.name))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("List<{}> {}", self.element_type(s), java_name(&pub_name(&el.name))));
        }
        let args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| java_name(&pub_name(&i.name)))
            .chain(self.f.elements.iter().map(|el| java_name(&pub_name(&el.name))))
            .collect();

        o.push_str(&format!("    /**\n     * {}\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        for i in &self.f.inputs {
            o.push_str(&format!(
                "     *   {} : {}\n",
                pub_name(&i.name),
                self.java_doc_ty(&self.ty_of(&i.name.text)).replace("long  // ", "")
            ));
        }
        for (s, el) in self.f.elements.iter().enumerate() {
            o.push_str(&format!("     *   {} : List&lt;{}&gt;\n", pub_name(&el.name), self.element_type(s)));
        }
        for od in outs {
            o.push_str(&format!(
                "     * -&gt; {} : {}\n",
                pub_name(&od.name),
                self.java_doc_ty(&self.ty_of(&od.name.text)).replace("long  // ", "")
            ));
        }
        o.push_str("     */\n");
        o.push_str(&format!(
            "    public static {ret} {fname}({}) {{\n        return {traced}({}).value();\n    }}\n\n",
            params.join(", "),
            args.join(", ")
        ));

        o.push_str(&format!("    /** {} */\n", jc(&traced_doc(&self.f.name.text, &self.f.version))));
        o.push_str(&format!("    public static Traced {traced}({}) {{\n", params.join(", ")));
        // A rule that picks by weight runs its walk in a method of its own, which gives back the
        // running totals of the weights too (§15.225).
        let walked = self.weighted_fold().map(|(i, _)| (self.walked_fn("java"), java_name(&self.fold_names(i).cuts)));
        if let Some((w, _)) = &walked {
            o.push_str(&format!("        Walked w = {w}({});\n        return new Traced(w.value(), w.trace());\n    }}\n\n", args.join(", ")));
            o.push_str(&format!("    /** {} */\n    static Walked {w}({}) {{\n", jc(&walked_doc()), params.join(", ")));
        }

        // Entry guards (§8.5). The declared types already refuse a value of the wrong kind,
        // so what is left to ask is the range (§15.43).
        let local = |n: &str| -> String { java_name(&self.ident(n)) };
        for i in &self.f.inputs {
            let v = java_name(&pub_name(&i.name));
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = super::Guarded::new(&mut o, optional.then(|| (format!("        if ({v} != null) {{\n"), "        }\n".to_string())), "    ");
            // The values the range of an ordered enum lets in, first to last (§15.210).
            if let Some((lo, hi)) = self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?))) {
                o.push_str(&format!(
                    "        if ({v}.compareTo({}) < 0 || {v}.compareTo({}) > 0) {{\n            throw new RuleInputError({});\n        }}\n",
                    self.java_value(lo, &ty),
                    self.java_value(hi, &ty),
                    java_str(&crate::door::out_of_range(&i.name.text)),
                ));
            }
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "        if ({v} < {}L || {v} > {}L) {{\n            throw new RuleInputError({}, {v});\n        }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        java_str(&crate::door::out_of_range(&i.name.text)),
                    ));
                }
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Java, "        ", |m| {
            format!("            throw new RuleInputError({});\n", java_str(m))
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("        List<Fired> {} = new ArrayList<>();\n", java_name(&trace)));

        if self.f.walks() {
            o.push_str(&self.java_walks(&local, &trace));
        } else {
            o.push_str(&self.java_items(&local, &trace, Phase::All));
        }

        // Every output: its source, brought to the wire scale, then its rounding once. An
        // optional output takes those steps only when its column has a value, and is `null` when
        // it has none (§15.249): a `Long` that is `null` is not unboxed.
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let opt = self.opt_out(oi);
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os && !opt {
                        let raw = java_name(&self.temp(&raw_base(oi)));
                        o.push_str(&format!(
                            "        long {raw} = {};  // {}\n",
                            java_expr(unparen(&res.text)),
                            jc(&tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty))
                        ));
                        format!("{}({raw}, {grid_i}L) / {}L", java_round_fn(m), res.scale / os)
                    } else if res.scale != os {
                        format!("{}({}, {grid_i}L) / {}L", java_round_fn(m), java_expr(unparen(&res.text)), res.scale / os)
                    } else {
                        format!("{}({}, {grid_i}L)", java_round_fn(m), java_expr(&res.text))
                    }
                }
                None => java_expr(&res.text),
            };
            let v = local(out_name);
            finals.push(if opt && body != v { format!("{v} == null ? null : {body}") } else { body });
        }
        let (made, cuts) = match &walked {
            Some((_, c)) => ("Walked", format!(", {c}")),
            None => ("Traced", String::new()),
        };
        if outs.len() == 1 {
            o.push_str(&format!("        return new {made}({}, {}{cuts});\n", finals[0], java_name(&trace)));
        } else {
            o.push_str(&format!(
                "        return new {made}(new Output({}), {}{cuts});\n",
                finals.join(", "),
                java_name(&trace)
            ));
        }
        o.push_str("    }\n");
        o
    }

    fn java_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("        // {}\n", jc(&self.table_head(t)));
        // Java wants the variable to exist before the branches assign it. The last arm
        // throws, so the compiler can see that every path assigns it exactly once.
        for oc in &t.outputs {
            o.push_str(&format!(
                "        {} {};\n",
                self.java_ty(&self.ty_of(&oc.name.text)),
                java_name(&self.ident(&oc.name.text))
            ));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.java_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let kw = if ri == 0 { "if" } else { "} else if" };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            o.push_str(&format!("        {kw} ({cond}) {{  // {}\n", jc(&self.row_head(t, ri, &cells.join(" | ")))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        format!("{}L", self.int_lit(n, &ty, self.scale(&oc.name.text)))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.java_value(w, &self.ty_of(&oc.name.text)),
                        Lit::Date(y, m, d) => format!("{}L", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => super::str_lit(x),
                        _ => "0L".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.is_value(w) {
                            self.java_value(w, &self.ty_of(&oc.name.text))
                        } else {
                            let v = local(w);
                            match java_expr(&self.rescaled(w, &oc.name.text, v.clone())) {
                                // A value that may be absent is scaled only when it is there.
                                r if r != v && matches!(self.ty_of(w), Ty::Opt(_)) => format!("{v} == null ? null : {r}"),
                                r => r,
                            }
                        }
                    }
                    Some(OutCell::Nothing) => "null".into(),
                    None => "0L".into(),
                };
                o.push_str(&format!("            {} = {v};\n", java_name(&self.ident(&oc.name.text))));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            o.push_str(&format!(
                "            {}.add(new Fired({}, {rn}, {}));\n",
                java_name(trace),
                java_str(&tn),
                java_str(&lbl)
            ));
        }
        o.push_str(&format!(
            "        }} else {{\n            throw new RuleContradictionError({});\n        }}\n",
            java_str(&tr!(
                "到達不能: 完全性は rulec が静的に検査済み",
                "unreachable: completeness was statically checked by rulec"
            ))
        ));
        o.push_str(&self.guards(t, local, Lang::Java, "        ", |name, i, j| {
            format!(
                "            throw new RuleContradictionError({});\n",
                java_str(&tr!(
                    "表 {name}: {i} と {j} が同時に当てはまりました",
                    "table {name}: {i} and {j} matched at the same time"
                ))
            )
        }));
        o
    }

    fn java_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("jsonStr({e}.value())"),
            Wire::Bool => format!("({e} ? \"true\" : \"false\")"),
            Wire::Int | Wire::Brand => format!("String.valueOf({e})"),
            Wire::Date => format!("jsonStr(civil({e}))"),
            Wire::Str => format!("jsonStr({e})"),
            Wire::Opt(w) => format!("({e} == null ? \"null\" : {})", Self::java_wire(e, w)),
        }
    }

    /// One call as a record (§15.35): the same canonical JSON every other target writes.
    pub fn java_record(&self) -> String {
        let fname = self.java_fname();
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (v_out, v_trace, v_tag) = (java_name(&out), java_name(&trace), java_name(&tag));
        let (ins, obs) = self.record_fields(
            |a, _| java_name(a),
            |_, a, _| if single { v_out.clone() } else { format!("{v_out}.{}()", java_name(a)) },
        );
        let ret = if single {
            self.java_ty(&self.ty_of(&self.f.outputs[0].name.text))
        } else {
            "Output".into()
        };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{} {}", self.java_ty(&self.ty_of(&i.name.text)), java_name(&pub_name(&i.name))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("List<{}> {}", self.element_type(s), java_name(&pub_name(&el.name))));
        }

        let mut o = String::new();
        o.push_str(
            "    private static String jsonStr(String s) {\n        \
             StringBuilder o = new StringBuilder(\"\\\"\");\n        \
             for (int i = 0; i < s.length(); i++) {\n            \
             char c = s.charAt(i);\n            \
             if (c == '\"' || c == '\\\\') {\n                o.append('\\\\').append(c);\n            \
             } else if (c < 32) {\n                \
             o.append(String.format(java.util.Locale.ROOT, \"\\\\u%04x\", (int) c));\n            \
             } else {\n                o.append(c);\n            }\n        }\n        \
             return o.append('\"').toString();\n    }\n\n",
        );
        if civil_needed(self.f, self.c) {
            o.push_str(
                "    private static String civil(long days) {\n        \
                 long z = days + 719468;\n        \
                 long era = (z >= 0 ? z : z - 146096) / 146097;\n        \
                 long doe = z - era * 146097;\n        \
                 long yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;\n        \
                 long doy = doe - (365 * yoe + yoe / 4 - yoe / 100);\n        \
                 long mp = (5 * doy + 2) / 153;\n        \
                 long d = doy - (153 * mp + 2) / 5 + 1;\n        \
                 long m = mp < 10 ? mp + 3 : mp - 9;\n        \
                 long y = yoe + era * 400 + (m <= 2 ? 1 : 0);\n        \
                 return String.format(java.util.Locale.ROOT, \"%04d-%02d-%02d\", y, m, d);\n    }\n\n",
            );
        }
        o.push_str(&format!("    /** {} */\n", record_doc()));
        o.push_str(&format!(
            "    public static String {fname}Record({}, {ret} {v_out}, List<Fired> {v_trace}, String {v_tag}) {{\n",
            params.join(", ")
        ));
        let (v_ins, v_obs, v_rows, v_head) =
            (java_name(&self.temp("ins")), java_name(&self.temp("obs")), java_name(&self.temp("rows")), java_name(&self.temp("head")));
        let field = |(jp, e, w): &(String, String, Wire)| {
            format!("            {} + {}", json_key(jp, ":"), Self::java_wire(e, w))
        };
        // The sequence goes in first (§15.56).
        let mut parts: Vec<String> = Vec::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let e = java_name(&self.temp("elem"));
            let v_els = java_name(&if s == 0 { self.temp("els") } else { self.temp(&format!("els{}", s + 1)) });
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "{} + {}",
                        json_key(&fd.name.text, ":"),
                        Self::java_wire(&format!("{e}.{}()", java_name(&pub_name(&fd.name))), &wire_of(&ty))
                    )
                })
                .collect();
            o.push_str(&format!(
                "        List<String> {v_els} = new ArrayList<>();\n        for ({} {e} : {}) {{\n            \
                 {v_els}.add(\"{{\" + {} + \"}}\");\n        }}\n",
                self.element_type(s),
                java_name(&pub_name(&el.name)),
                inner.join(" + \",\" + ")
            ));
            parts.push(format!(
                "            {} + String.join(\",\", {v_els}) + \"]\"",
                json_key(&el.name.text, ":[")
            ));
        }
        parts.extend(ins.iter().map(field));
        o.push_str(&format!("        String {v_ins} = String.join(\",\"{});\n", join_args(&parts)));
        o.push_str(&format!(
            "        String {v_obs} = String.join(\",\"{});\n",
            join_args(&obs.iter().map(field).collect::<Vec<_>>())
        ));
        let f = java_name(&self.temp("f"));
        o.push_str(&format!(
            "        List<String> {v_rows} = new ArrayList<>();\n        for (Fired {f} : {v_trace}) {{\n            \
             {v_rows}.add(\"{{\\\"table\\\":\" + jsonStr({f}.table()) + \",\\\"row\\\":\" + {f}.row()\n                \
             + ({f}.label().isEmpty() ? \"\" : \",\\\"label\\\":\" + jsonStr({f}.label())) + \"}}\");\n        }}\n"
        ));
        o.push_str(&format!(
            "        String {v_head} = {v_tag}.isEmpty() ? \"{{\" : \"{{\\\"tag\\\":\" + jsonStr({v_tag}) + \",\";\n"
        ));
        o.push_str(&format!(
            "        return {v_head} + \"\\\"in\\\":{{\" + {v_ins} + \"}},\\\"observed\\\":{{\" + {v_obs}\n            \
             + \"}},\\\"trace\\\":[\" + String.join(\",\", {v_rows}) + \"]}}\";\n    }}\n"
        ));
        o
    }

    /// A Java runner that reads JSONL on stdin and prints one record per line. Both streams
    /// are wrapped in UTF-8 explicitly: `System.out` follows the console's encoding
    /// otherwise, and a 円 would come back as bytes nothing else in the suite writes.
    /// What the Java runner answers with `--labels` (§15.211): for each line, an enum's name and
    /// a value's, the display name the class gives.
    fn java_labels_mode(&self, line: &str) -> String {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return String::new();
        }
        let arms: String = enums
            .iter()
            .map(|(en, _, _)| format!("                    case {} -> out.println({}.from(v).label());\n", java_str(en), self.java_enum_outside(en)))
            .collect();
        format!(
            "        if (args.length > 0 && args[0].equals(\"--labels\")) {{\n            while (({line} = in.readLine()) != null) {{\n                int tab = {line}.indexOf('\\t');\n                if (tab < 0) {{\n                    continue;\n                }}\n                String v = {line}.substring(tab + 1);\n                switch ({line}.substring(0, tab)) {{\n{arms}                    default -> out.println(\"?\");\n                }}\n            }}\n            return;\n        }}\n"
        )
    }

    pub fn java_runner(&self) -> String {
        let cls = self.java_class();
        let fname = self.java_fname();
        let d = java_name(&self.temp("d"));
        // One input read off the wire the way every door reads it (§15.204): there at all, of
        // its kind, and only then the module's type. Every reader throws the module's own
        // `RuleInputError`, with the sentence every language says.
        let read = |ty: &Ty, src: &str, name: &str| -> String {
            let optional = matches!(ty, Ty::Opt(_));
            let take = format!("take({src}, {}, {}, {optional})", java_str(name), java_str(&crate::door::missing(name)));
            let one = |v: &str| -> String {
                match ty.present() {
                    Ty::Enum(n) => format!(
                        "member({v}, {}, {}::from)",
                        java_str(&crate::door::not_in_enum(name, n)),
                        self.java_enum_outside(n)
                    ),
                    Ty::Bool => format!("b({v}, {})", java_str(&crate::door::not_bool(name))),
                    Ty::Date => format!("date({v}, {})", java_str(&crate::door::not_date(name))),
                    Ty::Str => format!("text({v}, {})", java_str(&crate::door::not_string(name))),
                    _ => format!("integer({v}, {})", java_str(&crate::door::not_integer(name))),
                }
            };
            if optional {
                // `null` on the wire is a null reference (DESIGN §15.89).
                format!("maybe({take}, x -> {})", one("x"))
            } else {
                one(&take)
            }
        };
        let mut binds = String::new();
        let mut args: Vec<String> = Vec::new();
        for (k, i) in self.f.inputs.iter().enumerate() {
            let a = java_name(&self.temp(&format!("a{k}")));
            binds.push_str(&format!("                    var {a} = {};\n", read(&self.ty_of(&i.name.text), &d, &i.name.text)));
            args.push(a);
        }
        // The sequence a walk reads, one element at a time (§15.56).
        for (s, el) in self.f.elements.iter().enumerate() {
            let e = java_name(&self.temp("e"));
            let v = java_name(&if s == 0 { self.temp("els") } else { self.temp(&format!("els{}", s + 1)) });
            let seq = &el.name.text;
            let ty = self.element_type(s);
            let fields: Vec<String> = el.fields.iter().map(|fd| read(&self.ty_of(&fd.name.text), &e, &fd.name.text)).collect();
            binds.push_str(&format!(
                "                    List<{cls}.{ty}> {v} = new ArrayList<>();\n                    \
                 for (Map<String, String> {e} : list(take({d}, {}, {}, false), {}, {})) {{\n                        \
                 {v}.add(new {cls}.{ty}({}));\n                    }}\n",
                java_str(seq),
                java_str(&crate::door::missing(seq)),
                java_str(&crate::door::not_sequence(seq)),
                java_str(&crate::door::not_object(seq)),
                fields.join(", ")
            ));
            args.push(v);
        }
        // `--chances` reads every input of a line plainly, as the vectors are read (§15.225).
        let chances_mode = {
            let plain = binds.replace("\n                    ", "\n                ");
            let plain = plain.strip_prefix("                    ").map(|p| format!("                {p}")).unwrap_or(plain);
            let pass: Vec<String> = self
                .f
                .inputs
                .iter()
                .zip(args.iter())
                .filter(|(i, _)| i.random.is_none())
                .map(|(_, a)| a.clone())
                .chain(args[self.f.inputs.len()..].iter().cloned())
                .collect();
            (plain, pass.join(", "))
        };
        let (r, line) = (java_name(&self.temp("r")), java_name(&self.temp("line")));
        // A machine's traces (§15.148): the carried input is the state this language answered
        // to the call before, handed over as the value it is.
        let (top, state, stepping) = (java_name(&self.temp("top")), java_name(&self.temp("state")), java_name(&self.temp("stepping")));
        let (mut head, mut meta, mut tail) = (String::new(), String::new(), String::new());
        if let (Some((en, _, _)), Some((cin, _)), Some(out)) = (self.machine_consts(), self.carried(), self.carried_out_alias()) {
            let ety = self.java_enum_outside(&en);
            if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == cin) {
                let a = &args[k];
                binds = binds.replace(&format!("var {a} = "), &format!("var {a} = {stepping} ? {state} : "));
            }
            // The values carried along with the state (§15.217): a local each, from the
            // module's constant, read where each trace starts and handed on from the answer.
            let (mut more_head, mut more_meta, mut more_start, mut more_tail) = (String::new(), Vec::<String>::new(), String::new(), String::new());
            for mc in self.more_carried() {
                let local = java_name(&self.temp(&format!("carried_{}", mc.in_alias)));
                if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == mc.input) {
                    let a = &args[k];
                    binds = binds.replace(&format!("var {a} = "), &format!("var {a} = {stepping} ? {local} : "));
                }
                let ty = match mc.kind() {
                    super::MoreKind::Enum => format!("{cls}.{}", self.java_ty(&mc.ty)),
                    _ => self.java_ty(&mc.ty),
                };
                more_head.push_str(&format!("        {ty} {local} = {cls}.{};\n", mc.upper()));
                let key = mc.key().replace('\\', "\\\\").replace('"', "\\\"");
                more_meta.push(match mc.kind() {
                    super::MoreKind::Enum => format!("\"{key}:\\\"\" + {cls}.{}.value() + \"\\\"\"", mc.upper()),
                    _ => format!("\"{key}:\" + {cls}.{}", mc.upper()),
                });
                more_start.push_str(&format!("                    {local} = {};\n", read(&mc.ty, "cv", &mc.input)));
                more_tail.push_str(&format!("                        {local} = {r}.value().{}();\n", java_name(&mc.out_alias)));
            }
            // `]` and the closing brace, with what is carried between them when anything is.
            let carried_meta = if more_meta.is_empty() { "]}\"".to_string() } else { format!("]\" + \",\\\"carried\\\":{{\" + {} + \"}}}}\"", more_meta.join(" + \",\" + ")) };
            if !more_start.is_empty() {
                more_start = format!("                    Map<String, String> cv = pairsFrom({top}.get(\"carried\"), 0).map();\n{more_start}");
            }
            head = format!("        {ety} {state} = {cls}.INITIAL;\n{more_head}");
            meta = format!(
                "                Map<String, String> {top} = pairsFrom({line}, 0).map();\n                \
                 if ({top}.containsKey(\"machine\")) {{\n                    \
                     List<String> fin = new ArrayList<>();\n                    \
                     for ({ety} s : {ety}.values()) {{\n                        if ({cls}.isFinal(s)) {{\n                            fin.add(\"\\\"\" + s.value() + \"\\\"\");\n                        }}\n                    }}\n                    \
                     out.println(\"{{\\\"initial\\\":\\\"\" + {cls}.INITIAL.value() + \"\\\",\\\"final\\\":[\" + String.join(\",\", fin) + \"{carried_meta});\n                    \
                     continue;\n                }}\n                \
                 boolean {stepping} = {top}.containsKey(\"step\");\n                \
                 if (\"start\".equals(s({top}, \"step\"))) {{\n                    {state} = {ety}.from(s({top}, \"state\"));\n{more_start}                }}\n"
            );
            tail = format!(
                "                    if ({stepping}) {{\n                        {state} = {};\n{more_tail}                    }}\n",
                self.next_state_of(&format!("{r}.value()"), &format!(".{}()", java_name(&out)))
            );
        }
        format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n\n\
             import java.io.BufferedReader;\n\
             import java.io.IOException;\n\
             import java.io.InputStreamReader;\n\
             import java.io.PrintStream;\n\
             import java.nio.charset.StandardCharsets;\n\
             import java.util.ArrayList;\n\
             import java.util.LinkedHashMap;\n\
             import java.util.List;\n\
             import java.util.Map;\n\n\
             public final class {cls}Runner {{\n    \
             private {cls}Runner() {{\n    }}\n\n    \
             public static void main(String[] args) throws IOException {{\n        \
             BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));\n        \
             PrintStream out = new PrintStream(System.out, true, StandardCharsets.UTF_8);\n        \
             String {line};\n\
             {labels}\
             {head}        \
             while (({line} = in.readLine()) != null) {{\n            \
             {line} = {line}.trim();\n            \
             if (!{line}.isEmpty()) {{\n\
             {meta}                \
             Map<String, String> {d} = fields({line});\n                \
             // One call, or the refusal its door gives, in the place of the record (§15.204).\n                \
             try {{\n\
             {binds}                    \
             {cls}.Traced {r} = {cls}.{fname}Traced({args});\n                    \
             out.println({cls}.{fname}Record({args}, {r}.value(), {r}.trace(), \"\"));\n\
             {tail}                \
             }} catch ({cls}.RuleInputError e) {{\n                    \
             out.println(refusal(\"input\", e.what));\n                \
             }} catch ({cls}.RuleContradictionError e) {{\n                    \
             out.println(refusal(\"contradiction\", e.what));\n                \
             }}\n            \
             }}\n        }}\n    }}\n\n\
             {helpers}{wire}}}\n",
            ritsu_emit::header::VERSION,
            args = args.join(", "),
            labels = self.java_labels_mode(&line) + &self.java_prereq_mode(&line) + &self.java_chances_mode(&line, &d, &chances_mode.0, &chances_mode.1, &args.join(", ")),
            helpers = JAVA_JSON_HELPERS,
            wire = JAVA_WIRE.replace("@CLS@", &cls).replace(
                "@D_WIRE@",
                &tr!(
                    "線の上の入力を、どの入口とも同じに読む。無いもの、型の違うものは、ほかの言語と同じ文で拒む。",
                    "An input read off the wire the way every door reads it: one not there, or of another kind, is refused with the sentence the other languages say."
                )
            ),
        )
    }

    /// The type as the inventory states it for Java. There are no brands here, so a number
    /// is `long` and the unit travels in the entry's own `unit` field.
    pub(super) fn java_api_ty(&self, ty: &Ty) -> String {
        self.java_ty(ty)
    }
}

/// The rest of a `String.join(",", …)` call: each part on its own line, and nothing at all
/// when there are none — an empty element would put a comma in the record that the other ten
/// targets do not write.
fn join_args(parts: &[String]) -> String {
    if parts.is_empty() {
        String::new()
    } else {
        format!(",\n{}", parts.join(",\n"))
    }
}

/// The five modes of §7.3 in Java, checked against the same reference cases the other ten
/// are checked against. `rulec test` runs it beside the generated module.
pub fn round_tests_java(u: &super::Uses) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n\
         // {}\n\n\
         public final class _RoundTest {{\n    private _RoundTest() {{\n    }}\n\n\
         {}\n    \
         private record Case(String mode, long x, long g, long want) {{\n    }}\n\n    \
         private static long call(String mode, long x, long g) {{\n        \
         switch (mode) {{\n            \
         case \"down\":\n                return roundDown(x, g);\n            \
         case \"up\":\n                return roundUp(x, g);\n            \
         case \"half\":\n                return roundHalf(x, g);\n            \
         case \"half_down\":\n                return roundHalfDown(x, g);\n            \
         default:\n                return roundBankers(x, g);\n        }}\n    }}\n\n    \
         public static void main(String[] args) {{\n        Case[] cases = {{\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        ),
        round_java("    ") + &Gen::indent_block(&super::arith::java_helpers(u), "    "),
    );
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("            new Case(\"{}\", {x}L, {g}L, {want}L),\n", mode_fn(m)));
    }
    o.push_str("        };\n        int bad = 0;\n        for (Case c : cases) {\n");
    o.push_str("            long got = call(c.mode(), c.x(), c.g());\n            if (got == c.want()) {\n                continue;\n            }\n");
    o.push_str("            System.out.println(\"NG \" + c.mode() + \"(\" + c.x() + \", \" + c.g() + \") = \" + got + \", want \" + c.want());\n            bad++;\n        }\n");
    let (arith, n) = super::arith::java_tests(u);
    o.push_str(&arith);
    o.push_str("        if (bad > 0) {\n            System.exit(1);\n        }\n");
    let count = if n == 0 { "cases.length".to_string() } else { format!("(cases.length + {n})") };
    o.push_str(&format!(
        "        System.out.println({});\n    }}\n}}\n",
        tr!("\"ok \" + {count} + \" 件\"", "\"ok \" + {count} + \" cases\"")
    ));
    o
}

/// How the generated Java runner reads the wire (§15.204): the readers of the Python runner's,
/// throwing the module's own `RuleInputError`. A string keeps its opening quote in the reader
/// above, so `"12"` is not a number and the text `true` is not a truth value.
const JAVA_WIRE: &str = r##"
    /** @D_WIRE@ */
    private static String take(Map<String, String> d, String k, String missing, boolean optional) {
        String v = d.get(k);
        if (v == null || (v.equals("null") && !optional)) {
            throw new @CLS@.RuleInputError(missing);
        }
        return v;
    }

    private static <T> T maybe(String v, java.util.function.Function<String, T> read) {
        return v.equals("null") ? null : read.apply(v);
    }

    // A JSON number whose value is a whole number, as that integer: `1000`, and `1000.0`, `1e3` or
    // `-0.0` too (§15.205). A string keeps its quote, so `"12"` is not one, and neither is `1.5`
    // or a value past 64 bits.
    private static long integer(String v, String what) {
        String digits = v.startsWith("-") ? v.substring(1) : v;
        if (digits.isEmpty() || digits.charAt(0) < '0' || digits.charAt(0) > '9') {
            throw new @CLS@.RuleInputError(what);
        }
        try {
            return new java.math.BigDecimal(v).longValueExact();
        } catch (NumberFormatException | ArithmeticException e) {
            // An exponent past an int: zero is still zero, and anything else does not fit.
            if (v.matches("-?0+(\\.0+)?[eE][-+]?[0-9]+")) {
                return 0;
            }
            throw new @CLS@.RuleInputError(what);
        }
    }

    private static boolean b(String v, String what) {
        if (v.equals("true")) {
            return true;
        }
        if (v.equals("false")) {
            return false;
        }
        throw new @CLS@.RuleInputError(what);
    }

    private static String text(String v, String what) {
        if (!v.startsWith("\"")) {
            throw new @CLS@.RuleInputError(what);
        }
        return v.substring(1);
    }

    private static long date(String v, String what) {
        String s = text(v, what);
        if (!s.matches("[0-9]{4}-[0-9]{2}-[0-9]{2}")) {
            throw new @CLS@.RuleInputError(what);
        }
        try {
            java.time.LocalDate.parse(s);
        } catch (java.time.format.DateTimeParseException e) {
            throw new @CLS@.RuleInputError(what);
        }
        return ord(s);
    }

    private static <E> E member(String v, String what, java.util.function.Function<String, E> from) {
        String s = text(v, what);
        try {
            return from.apply(s);
        } catch (IllegalArgumentException e) {
            throw new @CLS@.RuleInputError(what);
        }
    }

    private static List<Map<String, String>> list(String v, String notSequence, String notObject) {
        if (!v.startsWith("[")) {
            throw new @CLS@.RuleInputError(notSequence);
        }
        List<Map<String, String>> out = new ArrayList<>();
        int i = 1;
        while (true) {
            while (i < v.length() && (Character.isWhitespace(v.charAt(i)) || v.charAt(i) == ',')) {
                i++;
            }
            if (i >= v.length() || v.charAt(i) == ']') {
                return out;
            }
            if (v.charAt(i) != '{') {
                throw new @CLS@.RuleInputError(notObject);
            }
            Pairs p = pairsFrom(v, i);
            out.add(p.map());
            i = p.next();
        }
    }

    /** A refusal, in the place of the record: which of the two the door raised, and its sentence. */
    private static String refusal(String kind, String what) {
        StringBuilder o = new StringBuilder("{\"refused\":\"").append(kind).append("\",\"error\":\"");
        for (char c : what.toCharArray()) {
            if (c == '"' || c == '\\') {
                o.append('\\').append(c);
            } else if (c < 0x20) {
                o.append(String.format("\\u%04x", (int) c));
            } else {
                o.append(c);
            }
        }
        return o.append("\"}").toString();
    }
"##;

// ── The function that counts the answers (§15.225) ──────────────────────────────

impl<'a> Gen<'a> {
    /// The `Way` and `Chances` records and the method, in Java, inside the rule's class.
    pub(super) fn java_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let (doc_fn, doc_ty) = self.chances_docs();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.java_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let fname = self.java_fname();
        let name = self.chances_name("java");
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .filter(|i| i.random.is_none())
            .map(|i| format!("{} {}", self.java_ty(&self.ty_of(&i.name.text)), java_name(&pub_name(&i.name))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("List<{}> {}", self.element_type(s), java_name(&pub_name(&el.name))));
        }
        let call = |draw: Option<(&str, &str)>| -> String {
            let mut args: Vec<String> = self
                .f
                .inputs
                .iter()
                .map(|i| match draw {
                    Some((d, v)) if i.name.text == d => v.to_string(),
                    _ => java_name(&pub_name(&i.name)),
                })
                .collect();
            for el in &self.f.elements {
                args.push(java_name(&pub_name(&el.name)));
            }
            args.join(", ")
        };
        let (total, ways, out, k, last, e) = (
            java_name(&self.temp("total")),
            java_name(&self.temp("ways")),
            java_name(&self.temp("out")),
            java_name(&self.temp("k")),
            java_name(&self.temp("last")),
            java_name(&self.temp("e")),
        );
        let add = |ind: &str, n: &str| -> String {
            format!(
                "{ind}int {k} = 0;\n{ind}while ({k} < {ways}.size() && !java.util.Objects.equals({ways}.get({k}).out(), {out})) {{\n{ind}    {k} += 1;\n{ind}}}\n{ind}if ({k} == {ways}.size()) {{\n{ind}    {ways}.add(new Way({out}, {n}));\n{ind}}} else {{\n{ind}    {ways}.set({k}, new Way({out}, {ways}.get({k}).count() + {n}));\n{ind}}}\n"
            )
        };
        let mut o = format!(
            "    /** {} */\n    public record Way({ret} out, long count) {{\n    }}\n\n    /** {} */\n    public record Chances(long total, List<Way> ways) {{\n    }}\n\n",
            jc(&tr!("答えの一つと、それを出す引き方の数。", "One answer, and how many draws give it.")),
            jc(&doc_ty)
        );
        o.push_str(&format!("    /** {} */\n    public static Chances {name}({}) {{\n", jc(&doc_fn), params.join(", ")));
        o.push_str(&format!("        long {total} = 0L;\n        List<Way> {ways} = new ArrayList<>();\n"));
        match self.weighted_draw_pub() {
            Some((draw, lo, hi)) => {
                let (cuts, low, cut, n) = (java_name(&self.temp("cuts")), java_name(&self.temp("low")), java_name(&self.temp("cut")), java_name(&self.temp("n")));
                o.push_str(&format!("        List<Long> {cuts} = {}({}).cuts();\n", self.walked_fn("java"), call(Some((&draw, &format!("{lo}L"))))));
                o.push_str(&format!(
                    "        if ({cuts}.isEmpty() || {cuts}.get({cuts}.size() - 1) == 0L) {{\n            return new Chances({all}L, List.of(new Way({fname}({}), {all}L)));\n        }}\n",
                    call(Some((&draw, &format!("{lo}L")))),
                    all = hi - lo + 1
                ));
                o.push_str(&format!("        long {low} = {lo}L;\n        for (long {cut} : {cuts}) {{\n            if ({cut} > {low} && {low} <= {hi}L) {{\n"));
                o.push_str(&format!("                long {n} = Math.min({cut}, {}L) - {low};\n", hi + 1));
                o.push_str(&format!("                {ret} {out} = {fname}({});\n", call(Some((&draw, &low)))));
                o.push_str(&add("                ", &n));
                o.push_str(&format!("                {total} += {n};\n            }}\n            if ({cut} > {low}) {{\n                {low} = {cut};\n            }}\n        }}\n        return new Chances({total}, {ways});\n    }}\n\n"));
            }
            None => {
                o.push_str(&format!("        RuleInputError {last} = null;\n"));
                let mut ind = "        ".to_string();
                for (j, d) in self.drawn().iter().enumerate() {
                    let v = java_name(&d.alias);
                    match &d.from {
                        super::chances::From::Range(lo, hi) => {
                            let ctr = java_name(&self.temp(&format!("_i{j}")));
                            o.push_str(&format!("{ind}for (long {ctr} = {lo}L; {ctr} <= {hi}L; {ctr}++) {{\n{ind}    long {v} = {ctr};\n"));
                        }
                        super::chances::From::Values(vs) => {
                            let t = self.java_ty(&d.ty);
                            let members: Vec<String> = vs.iter().map(|x| self.java_value(x, &d.ty)).collect();
                            o.push_str(&format!("{ind}for ({t} {v} : new {t}[] {{{}}}) {{\n", members.join(", ")));
                        }
                        super::chances::From::Bool => o.push_str(&format!("{ind}for (boolean {v} : new boolean[] {{false, true}}) {{\n")),
                    }
                    ind.push_str("    ");
                }
                o.push_str(&format!(
                    "{ind}{ret} {out};\n{ind}try {{\n{ind}    {out} = {fname}({});\n{ind}}} catch (RuleInputError {e}) {{\n{ind}    {last} = {e};\n{ind}    continue;\n{ind}}}\n{ind}{total} += 1;\n",
                    call(None)
                ));
                o.push_str(&add(&ind, "1L"));
                for _ in self.drawn() {
                    ind.truncate(ind.len() - 4);
                    o.push_str(&format!("{ind}}}\n"));
                }
                o.push_str(&format!("        if ({total} == 0L && {last} != null) {{\n            throw {last};\n        }}\n        return new Chances({total}, {ways});\n    }}\n\n"));
            }
        }
        o
    }

    /// What the Java runner answers with `--chances`: `binds` reads every input of a line into a
    /// local, `pass` is what the function is called with, `args` what the record function is.
    pub(super) fn java_chances_mode(&self, line: &str, d: &str, binds: &str, pass: &str, args: &str) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let cls = self.java_class();
        let fname = self.java_fname();
        let name = self.chances_name("java");
        r#"        if (args.length > 0 && args[0].equals("--chances")) {
            while ((@LINE@ = in.readLine()) != null) {
                @LINE@ = @LINE@.trim();
                if (@LINE@.isEmpty()) {
                    continue;
                }
                Map<String, String> @D@ = fields(@LINE@);
@BINDS@                @CLS@.Chances c = @CLS@.@NAME@(@PASS@);
                out.println("{\"total\":" + c.total() + "}");
                for (@CLS@.Way w : c.ways()) {
                    String rec = @CLS@.@FNAME@Record(@ARGS@, w.out(), List.of(), "");
                    out.println(w.count() + "\t" + rec.substring(rec.lastIndexOf(",\"observed\":") + 12, rec.lastIndexOf(",\"trace\":")));
                }
            }
            return;
        }
"#
        .replace("@LINE@", line)
        .replace("@D@", d)
        .replace("@BINDS@", binds)
        .replace("@CLS@", &cls)
        .replace("@NAME@", &name)
        .replace("@PASS@", pass)
        .replace("@FNAME@", &fname)
        .replace("@ARGS@", args)
    }
}
