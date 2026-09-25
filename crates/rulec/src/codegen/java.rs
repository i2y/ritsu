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
    camel, cell_src, civil_needed, fired_doc, mode_fn, out_src, pascal, plain_doc, pub_name,
    raw_base, record_doc, round_cases, traced_doc, unparen, wire_of, Expr2, Gen, Lang, Phase, Wire,
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
fn java_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
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
    let t = s
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
                v = s.text();
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

    /** 平たいオブジェクトの並び（§15.56）。 */
    private static List<Map<String, String>> rows(String v) {
        List<Map<String, String>> out = new ArrayList<>();
        int i = 0;
        while (i < v.length()) {
            if (v.charAt(i) == '{') {
                Pairs p = pairsFrom(v, i);
                out.add(p.map());
                i = p.next();
            } else {
                i++;
            }
        }
        return out;
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

    private static long n(Map<String, String> d, String k) {
        try {
            return Long.parseLong(get(d, k));
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    private static String s(Map<String, String> d, String k) {
        return get(d, k);
    }

    private static boolean b(Map<String, String> d, String k) {
        return get(d, k).equals("true");
    }

    /** 1970-01-01 からの通算日。道具が使うのと同じ暦の計算。 */
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
            Ty::Enum(n) => java_class(&self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into())),
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
            _ => format!("long  // {ty}"),
        }
    }

    /// An enum value as the constant the class declares for it.
    fn java_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{}.{}", java_class(ty), java_id(&alias.to_uppercase())),
            None => java_str(v),
        }
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
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.java_value(w),
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
                        out.extend(ms.iter().map(|m| self.java_value(m)));
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
                format!("{}.contains({var})", self.java_group(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => eq(l),
            Cell::Set(ls) => format!("{}.contains({var})", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!{}.contains({var})", self.java_group(w))
            }
            Cell::Not(ls) => format!("!{}.contains({var})", members(ls)),
            Cell::Cmp(cs) => cs
                .iter()
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
        })
    }

    /// The module: one class, one file, no package and no build file.
    pub fn java(&self) -> String {
        let cls = self.java_class();
        let mut o = self.header("//");
        o.push_str("\nimport java.util.ArrayList;\nimport java.util.List;\n\n");
        o.push_str(&format!(
            "/** {} */\npublic final class {cls} {{\n    private {cls}() {{\n    }}\n\n",
            tr!("規則 {} v{}", "Rule {} v{}", self.f.name.text, self.f.version)
        ));

        // Enums. Each constant carries the source name, which is also the wire value
        // (§10.2), so the runner converts in one call and keeps no table of its own.
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            let ty = java_class(ascii);
            o.push_str(&format!("    /** {jp} */\n    public enum {ty} {{\n"));
            let cs: Vec<String> = vals
                .iter()
                .map(|v| {
                    let name = java_id(
                        &self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone()),
                    );
                    format!("        {name}({})", java_str(v))
                })
                .collect();
            o.push_str(&format!("{};\n\n", cs.join(",\n")));
            o.push_str(&format!(
                "        private final String wire;\n\n        \
                 {ty}(String wire) {{\n            this.wire = wire;\n        }}\n\n        \
                 /** {vd} */\n        public String value() {{\n            return wire;\n        }}\n\n        \
                 /** {fd} */\n        public static {ty} from(String v) {{\n            \
                 for ({ty} x : values()) {{\n                if (x.wire.equals(v)) {{\n                    \
                 return x;\n                }}\n            }}\n            \
                 throw new RuleInputError({msg} + v);\n        }}\n    }}\n\n",
                vd = tr!("記録とワイヤに出る綴り。", "The spelling this member has on the wire and in a record."),
                fd = tr!("ワイヤの綴りから引く。無ければ入口で断る。", "The member with this spelling on the wire; refused at the door when there is none."),
                msg = java_str(&tr!(
                    "{jp} が列挙 {ascii} の値ではありません: ",
                    "{jp} is not a value of enum {ascii}: "
                )),
            ));
        }

        // The machine this function is one step of (§15.148).
        if let Some((en, init, fins)) = self.machine_consts() {
            let ety = java_class(&self.enum_names.get(&en).cloned().unwrap_or_default());
            o.push_str(&format!(
                "    /** {} */\n    public static final {ety} INITIAL = {};\n\n    \
                 /** {} */\n    public static final List<{ety}> FINAL = List.of({});\n\n    \
                 /** {} */\n    public static boolean isFinal({ety} state) {{\n        return FINAL.contains(state);\n    }}\n\n",
                tr!("案件が始まる状態（§15.148）。", "The state a case starts in (§15.148)."),
                self.java_value(&init),
                tr!("案件が終わる状態。", "The states a case ends in."),
                fins.iter().map(|v| self.java_value(v)).collect::<Vec<_>>().join(", "),
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
            tr!("何を断ったかを言う文。", "The sentence that says what was refused."),
            tr!("断られた値。理由が値についてでなければ null。", "The value refused, or null when the refusal is not about one.")
        ));
        o.push_str(&format!(
            "    /** {} */\n    public static final class RuleContradictionError extends RuntimeException {{\n        \
             private static final long serialVersionUID = 1L;\n\n        \
             /** {} */\n        public final String what;\n\n        \
             public RuleContradictionError(String what) {{\n            super(what);\n            this.what = what;\n        }}\n    }}\n\n",
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller."),
            tr!("何が矛盾したかを言う文。", "The sentence that says what contradicted.")
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

        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.java_value(&m.text)).collect();
            let ety = match g.members.first().and_then(|m| self.value_names.get(&m.text)) {
                Some((ty, _)) => java_class(ty),
                None => "String".into(),
            };
            o.push_str(&format!(
                "    /** {} */\n    private static final List<{ety}> {} = List.of({});\n\n",
                g.name.text,
                self.java_group(&g.name.text),
                ms.join(", ")
            ));
        }

        o.push_str(&round_java("    "));
        o.push('\n');
        o.push_str(&self.java_fn());
        o.push('\n');
        o.push_str(&self.java_record());
        o.push_str("}\n");
        o
    }

    /// The rule's items as Java, at the base indentation (§15.56).
    fn java_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for it in &self.f.items {
            if !self.in_phase(it, phase) {
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
                Item::Agg(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.java_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// The walk, in Java (§15.56).
    fn java_walk(&self, fold: &crate::ast::FoldDecl, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a fold has elements");
        let seq = java_name(&pub_name(&el.name));
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), java_name(&pub_name(&fd.name)))).collect();
        let (answer, stopped, taken, kept, best) = self.fold_locals();
        let (answer, stopped) = (java_name(&answer), java_name(&stopped));
        let (taken, kept, best) = (java_name(&taken), java_name(&kept), java_name(&best));
        let e = java_name(&self.temp("elem"));
        let held = java_name(&self.temp("held"));
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}()"),
                None if n == crate::kw::HELD => held.clone(),
                None => java_name(&self.ident(n)),
            }
        };
        let out_name = self.f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
        let out_ty = self.java_ty(&self.ty_of(&out_name));
        let acc = self.java_boxed(&self.ty_of(&out_name));
        let text = |x: &Option<crate::ast::Expr>| -> String {
            x.as_ref().map(|x| java_expr(unparen(&self.expr(x, &local).text))).unwrap_or_else(|| "0L".to_string())
        };

        let mut o = String::new();
        o.push_str(&format!(
            "        {out_ty} {answer} = {};  // {}\n",
            text(&fold.empty),
            tr!("要素ゼロ件の答え", "the answer for no elements")
        ));
        o.push_str(&format!("        boolean {stopped} = false;\n"));
        o.push_str(&format!(
            "        {acc} {taken} = null;\n        {acc} {kept} = null;\n        Long {best} = null;\n"
        ));
        o.push_str(&format!("        for (Element {e} : {seq}) {{\n"));

        let mut body = self.java_element_guards(&local);
        body.push_str(&self.java_items(&local, trace, Phase::All));
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let val = self.java_value(&name.text);
            body.push_str(&format!("        if ({v} == {val}) {{  // {}\n", name.text));
            match arm {
                Arm::Next => body.push_str("            // next\n"),
                Arm::Stop(None) => body.push_str("            break;\n"),
                Arm::Stop(Some(x)) => {
                    body.push_str(&format!("            {answer} = {};\n", java_expr(unparen(&self.expr(x, &local).text))));
                    body.push_str(&format!("            {stopped} = true;\n            break;\n"));
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        body.push_str(&format!("            if ({taken} != null) {{\n"));
                        body.push_str(&format!(
                            "                throw new RuleContradictionError({});\n            }}\n",
                            java_str(&tr!(
                                "畳み込み {}: take_unique に二件当たりました",
                                "fold {}: two elements matched a take_unique",
                                fold.verdict
                            ))
                        ));
                        body.push_str(&format!("            {taken} = {};\n", java_expr(unparen(&self.expr(expr, &local).text))));
                    } else {
                        body.push_str(&format!("            if ({taken} == null) {{\n"));
                        body.push_str(&format!(
                            "                {taken} = {};\n            }}\n",
                            java_expr(unparen(&self.expr(expr, &local).text))
                        ));
                    }
                }
                Arm::KeepMax { expr, key } => {
                    let k = java_name(&self.temp("key"));
                    body.push_str(&format!("            long {k} = {};\n", java_expr(unparen(&self.expr(key, &local).text))));
                    body.push_str(&format!("            if ({best} == null || {k} > {best}) {{\n"));
                    body.push_str(&format!("                {best} = {k};\n"));
                    body.push_str(&format!(
                        "                {kept} = {};\n            }}\n",
                        java_expr(unparen(&self.expr(expr, &local).text))
                    ));
                }
            }
            body.push_str("        }\n");
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("        }\n");

        o.push_str(&format!("        if (!{seq}.isEmpty() && !{stopped}) {{\n"));
        o.push_str(&format!(
            "            {out_ty} {held} = {taken} != null ? {taken} : ({kept} != null ? {kept} : {});\n",
            text(&fold.empty)
        ));
        o.push_str(&format!("            {answer} = {};\n        }}\n", text(&fold.exhausted)));
        o.push_str(&format!("        {out_ty} {} = {answer};\n", java_name(&self.ident(&out_name))));
        o
    }

    /// The counting walk in Java (§15.58).
    fn java_count_walk(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a count has elements");
        let seq = java_name(&pub_name(&el.name));
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), java_name(&pub_name(&fd.name)))).collect();
        let e = java_name(&self.temp("elem"));
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}()"),
                None => java_name(&self.ident(n)),
            }
        };
        let mut o = String::new();
        if let Some(cap) = self.count_cap() {
            o.push_str(&format!("        if ({seq}.size() > {cap}) {{\n"));
            o.push_str(&format!(
                "            throw new RuleInputError({}, (long) {seq}.size());\n        }}\n",
                java_str(&tr!(
                    "{} の要素が多すぎます（上限 {cap}）",
                    "{} has too many elements (at most {cap})",
                    el.name.text
                ))
            ));
        }
        for d in self.counts() {
            o.push_str(&format!(
                "        long {} = 0;  // {}\n",
                java_name(&self.ident(&d.name.text)),
                self.agg_word(d)
            ));
        }
        o.push_str(&format!("        for (Element {e} : {seq}) {{\n"));
        let mut body = self.java_element_guards(&local);
        body.push_str(&self.java_items(&local, trace, Phase::Walk));
        for d in self.counts() {
            let v = local(&d.column.text);
            if d.kind == crate::ast::AggKind::Sum {
                body.push_str(&format!("        {} += {v};  // {}\n", java_name(&self.ident(&d.name.text)), d.name.text));
                continue;
            }
            let test = match self.count_member(d) {
                Some(w) => format!("{v} == {}", self.java_value(&w.text)),
                None if self.count_negated(d) => format!("!{v}"),
                None => v,
            };
            body.push_str(&format!(
                "        if ({test}) {{  // {}\n            {} += 1;\n        }}\n",
                d.name.text,
                java_name(&self.ident(&d.name.text))
            ));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("        }\n");
        for (d, cap) in self.sum_caps() {
            let n = java_name(&self.ident(&d.name.text));
            o.push_str(&format!(
                "        if ({n} > {cap}L) {{\n            throw new RuleInputError({}, {n});\n        }}\n",
                java_str(&tr!("{} が範囲の外です", "{} is out of range", d.name.text)),
            ));
        }
        o.push_str(&self.java_items(outer, trace, Phase::Main));
        o
    }

    /// The range guard of one element's fields, inside the walk. The kind of the value is
    /// the declared type's business here, so only the range is asked about.
    fn java_element_guards(&self, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields() {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "        if ({v} < {}L || {v} > {}L) {{\n            throw new RuleInputError({}, {v});\n        }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        java_str(&tr!("{} が範囲の外です", "{} is out of range", i.name.text)),
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
        if let Some(el) = &self.f.elements {
            let fs: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    format!("{} {}", self.java_ty(&self.ty_of(&fd.name.text)), java_name(&pub_name(&fd.name)))
                })
                .collect();
            o.push_str(&format!(
                "    /** {} */\n    public record Element({}) {{\n    }}\n\n",
                tr!("{} の一件", "one of {}", el.name.text),
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
        if let Some(el) = &self.f.elements {
            params.push(format!("List<Element> {}", java_name(&pub_name(&el.name))));
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
        if let Some(el) = &self.f.elements {
            o.push_str(&format!("     *   {} : List&lt;Element&gt;\n", pub_name(&el.name)));
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

        o.push_str(&format!("    /** {} */\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!("    public static Traced {traced}({}) {{\n", params.join(", ")));

        // Entry guards (§8.5). The declared types already refuse a value of the wrong kind,
        // so what is left to ask is the range (§15.43).
        let local = |n: &str| -> String { java_name(&self.ident(n)) };
        for i in &self.f.inputs {
            let v = java_name(&pub_name(&i.name));
            let ty = self.ty_of(&i.name.text);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "        if ({v} < {}L || {v} > {}L) {{\n            throw new RuleInputError({}, {v});\n        }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        java_str(&tr!("{} が範囲の外です", "{} is out of range", i.name.text)),
                    ));
                }
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Java, "        ", |m| {
            format!("            throw new RuleInputError({});\n", java_str(m))
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("        List<Fired> {} = new ArrayList<>();\n", java_name(&trace)));

        match &self.f.fold {
            Some(fold) => o.push_str(&self.java_walk(fold, &trace)),
            None if self.counts().is_empty() => o.push_str(&self.java_items(&local, &trace, Phase::All)),
            None => o.push_str(&self.java_count_walk(&local, &trace)),
        }

        // Every output: its source, brought to the wire scale, then its rounding once.
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let ty = self.ty_of(out_name);
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = java_name(&self.temp(&raw_base(oi)));
                        o.push_str(&format!(
                            "        long {raw} = {};  // {}\n",
                            java_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("{}({raw}, {grid_i}L) / {}L", java_round_fn(m), res.scale / os)
                    } else {
                        format!("{}({}, {grid_i}L)", java_round_fn(m), java_expr(&res.text))
                    }
                }
                None => java_expr(&res.text),
            });
        }
        if outs.len() == 1 {
            o.push_str(&format!("        return new Traced({}, {});\n", finals[0], java_name(&trace)));
        } else {
            o.push_str(&format!(
                "        return new Traced(new Output({}), {});\n",
                finals.join(", "),
                java_name(&trace)
            ));
        }
        o.push_str("    }\n");
        o
    }

    fn java_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("        // {}\n", self.table_head(t));
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
            o.push_str(&format!("        {kw} ({cond}) {{  // {}\n", self.row_head(t, ri, &cells.join(" | "))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        format!("{}L", self.int_lit(n, &ty, self.scale(&oc.name.text)))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.java_value(w),
                        Lit::Date(y, m, d) => format!("{}L", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => super::str_lit(x),
                        _ => "0L".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.java_value(w)
                        } else {
                            java_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
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
        if let Some(el) = &self.f.elements {
            params.push(format!("List<Element> {}", java_name(&pub_name(&el.name))));
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
            format!("            \"\\\"{jp}\\\":\" + {}", Self::java_wire(e, w))
        };
        // The sequence goes in first (§15.56).
        let mut parts: Vec<String> = Vec::new();
        if let Some(el) = &self.f.elements {
            let e = java_name(&self.temp("elem"));
            let v_els = java_name(&self.temp("els"));
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "\"\\\"{}\\\":\" + {}",
                        fd.name.text,
                        Self::java_wire(&format!("{e}.{}()", java_name(&pub_name(&fd.name))), &wire_of(&ty))
                    )
                })
                .collect();
            o.push_str(&format!(
                "        List<String> {v_els} = new ArrayList<>();\n        for (Element {e} : {}) {{\n            \
                 {v_els}.add(\"{{\" + {} + \"}}\");\n        }}\n",
                java_name(&pub_name(&el.name)),
                inner.join(" + \",\" + ")
            ));
            parts.push(format!(
                "            \"\\\"{}\\\":[\" + String.join(\",\", {v_els}) + \"]\"",
                el.name.text
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
    pub fn java_runner(&self) -> String {
        let cls = self.java_class();
        let fname = self.java_fname();
        let d = java_name(&self.temp("d"));
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let jp = java_str(&i.name.text);
            args.push(match &self.ty_of(&i.name.text) {
                Ty::Enum(n) => format!(
                    "{cls}.{}.from(s({d}, {jp}))",
                    java_class(&self.enum_names.get(n).cloned().unwrap_or_default())
                ),
                Ty::Str => format!("s({d}, {jp})"),
                Ty::Bool => format!("b({d}, {jp})"),
                Ty::Date => format!("ord(s({d}, {jp}))"),
                // `null` on the wire is a null reference; the module's parameter is the
                // enum itself, which Java lets be null. It used to fall to `n(...)`, which
                // hands a long to an enum and does not compile (DESIGN §15.89).
                Ty::Opt(inner) => {
                    let one = match inner.as_ref() {
                        Ty::Enum(nm) => format!(
                            "{cls}.{}.from(s({d}, {jp}))",
                            java_class(&self.enum_names.get(nm).cloned().unwrap_or_default())
                        ),
                        Ty::Str => format!("s({d}, {jp})"),
                        Ty::Bool => format!("b({d}, {jp})"),
                        Ty::Date => format!("ord(s({d}, {jp}))"),
                        _ => format!("n({d}, {jp})"),
                    };
                    format!("(\"null\".equals(s({d}, {jp})) ? null : {one})")
                }
                _ => format!("n({d}, {jp})"),
            });
        }
        // The sequence a walk reads, one element at a time (§15.56).
        let mut seq = String::new();
        if let Some(el) = &self.f.elements {
            let e = java_name(&self.temp("e"));
            let v = java_name(&self.temp("els"));
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let k = java_str(&fd.name.text);
                    match &self.ty_of(&fd.name.text) {
                        Ty::Enum(n) => format!(
                            "{cls}.{}.from(s({e}, {k}))",
                            java_class(&self.enum_names.get(n).cloned().unwrap_or_default())
                        ),
                        Ty::Str => format!("s({e}, {k})"),
                        Ty::Bool => format!("b({e}, {k})"),
                        Ty::Date => format!("ord(s({e}, {k}))"),
                        // `null` on the wire is a null reference; the module's parameter is the
                        // enum itself, which Java lets be null. It used to fall to `n(...)`, which
                        // hands a long to an enum and does not compile (DESIGN §15.89).
                        Ty::Opt(inner) => {
                            let one = match inner.as_ref() {
                                Ty::Enum(nm) => format!(
                                    "{cls}.{}.from(s({e}, {k}))",
                                    java_class(&self.enum_names.get(nm).cloned().unwrap_or_default())
                                ),
                                Ty::Str => format!("s({e}, {k})"),
                                Ty::Bool => format!("b({e}, {k})"),
                                Ty::Date => format!("ord(s({e}, {k}))"),
                                _ => format!("n({e}, {k})"),
                            };
                            format!("(\"null\".equals(s({e}, {k})) ? null : {one})")
                        }
                        _ => format!("n({e}, {k})"),
                    }
                })
                .collect();
            seq = format!(
                "                List<{cls}.Element> {v} = new ArrayList<>();\n                \
                 for (Map<String, String> {e} : rows(get({d}, {}))) {{\n                    \
                 {v}.add(new {cls}.Element({}));\n                }}\n",
                java_str(&el.name.text),
                fields.join(", ")
            );
            args.push(v);
        }
        let (r, line) = (java_name(&self.temp("r")), java_name(&self.temp("line")));
        // A machine's traces (§15.148): the carried input is the state this language answered
        // to the call before, handed over as the value it is.
        let (top, state, stepping) = (java_name(&self.temp("top")), java_name(&self.temp("state")), java_name(&self.temp("stepping")));
        let (mut head, mut meta, mut tail) = (String::new(), String::new(), String::new());
        if let (Some((en, _, _)), Some((cin, _)), Some(out)) = (self.machine_consts(), self.carried(), self.carried_out_alias()) {
            let ety = format!("{cls}.{}", java_class(&self.enum_names.get(&en).cloned().unwrap_or_default()));
            if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == cin) {
                args[k] = format!("({stepping} ? {state} : {})", args[k]);
            }
            head = format!("        {ety} {state} = {cls}.INITIAL;\n");
            meta = format!(
                "                Map<String, String> {top} = pairsFrom({line}, 0).map();\n                \
                 if ({top}.containsKey(\"machine\")) {{\n                    \
                     List<String> fin = new ArrayList<>();\n                    \
                     for ({ety} s : {ety}.values()) {{\n                        if ({cls}.isFinal(s)) {{\n                            fin.add(\"\\\"\" + s.value() + \"\\\"\");\n                        }}\n                    }}\n                    \
                     out.println(\"{{\\\"initial\\\":\\\"\" + {cls}.INITIAL.value() + \"\\\",\\\"final\\\":[\" + String.join(\",\", fin) + \"]}}\");\n                    \
                     continue;\n                }}\n                \
                 boolean {stepping} = {top}.containsKey(\"step\");\n                \
                 if (\"start\".equals({top}.get(\"step\"))) {{\n                    {state} = {ety}.from({top}.get(\"state\"));\n                }}\n"
            );
            tail = format!(
                "                if ({stepping}) {{\n                    {state} = {};\n                }}\n",
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
             {head}        \
             while (({line} = in.readLine()) != null) {{\n            \
             {line} = {line}.trim();\n            \
             if (!{line}.isEmpty()) {{\n\
             {meta}                \
             Map<String, String> {d} = fields({line});\n\
             {seq}                \
             {cls}.Traced {r} = {cls}.{fname}Traced({args});\n                \
             out.println({cls}.{fname}Record({args}, {r}.value(), {r}.trace(), \"\"));\n\
             {tail}            \
             }}\n        }}\n    }}\n\n\
             {helpers}}}\n",
            env!("CARGO_PKG_VERSION"),
            args = args.join(", "),
            helpers = JAVA_JSON_HELPERS,
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
pub fn round_tests_java() -> String {
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
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        ),
        round_java("    "),
    );
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("            new Case(\"{}\", {x}L, {g}L, {want}L),\n", mode_fn(m)));
    }
    o.push_str("        };\n        int bad = 0;\n        for (Case c : cases) {\n");
    o.push_str("            long got = call(c.mode(), c.x(), c.g());\n            if (got == c.want()) {\n                continue;\n            }\n");
    o.push_str("            System.out.println(\"NG \" + c.mode() + \"(\" + c.x() + \", \" + c.g() + \") = \" + got + \", want \" + c.want());\n            bad++;\n        }\n");
    o.push_str("        if (bad > 0) {\n            System.exit(1);\n        }\n");
    o.push_str(&format!(
        "        System.out.println({});\n    }}\n}}\n",
        tr!("\"ok \" + cases.length + \" 件\"", "\"ok \" + cases.length + \" cases\"")
    ));
    o
}
