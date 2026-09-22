//! Equivalence verification (§10).
//!
//! The legacy implementation is started as a child process and JSON Lines flow over
//! stdin/stdout. No FFI and no network: "a process and line-oriented JSON" is the smallest
//! surface shared across languages, and it is written the same way in Python and in Go.

use crate::ast::{Name, RuleFile};
use crate::eval::Val;
use crate::types::{Checked, Ty};
use crate::report::{Mismatch, Report, wire};
use crate::vectors::{self, Vector};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Naive extraction of a JSON value. Only as much as is needed, so as not to add a
/// dependency.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\":");
    let i = line.find(&pat)? + pat.len();
    let rest = line[i..].trim_start();
    Some(rest)
}

fn scalar(rest: &str) -> String {
    let rest = rest.trim_start();
    if let Some(s) = rest.strip_prefix('"') {
        let mut out = String::new();
        let mut esc = false;
        for c in s.chars() {
            if esc {
                out.push(c);
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                break;
            } else {
                out.push(c);
            }
        }
        out
    } else {
        rest.chars()
            .take_while(|c| !matches!(c, ',' | '}' | ']'))
            .collect::<String>()
            .trim()
            .to_string()
    }
}

/// Starts the adapter, streams the vectors through it and compares the answers.
pub fn run(f: &RuleFile, c: &Checked, adapter: &[String], vs: &[Vector]) -> Result<Report, String> {
    let (cmd, args) = adapter
        .split_first()
        .ok_or_else(|| tr!("アダプタのコマンドがありません", "No adapter command was given"))?;
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| tr!("アダプタを起動できません: {e}", "Cannot start the adapter: {e}"))?;
    let mut si = child
        .stdin
        .take()
        .ok_or_else(|| tr!("stdin を掴めません", "Cannot open the adapter's stdin"))?;
    let mut so = BufReader::new(
        child
            .stdout
            .take()
            .ok_or_else(|| tr!("stdout を掴めません", "Cannot open the adapter's stdout"))?,
    );

    // Handshake (§10.1). Names are sent as the canonical (Japanese) names from the `.rule`.
    let ins: Vec<String> = f.inputs.iter().map(|i| format!("\"{}\"", i.name.text)).collect();
    let outs: Vec<String> = f.outputs.iter().map(|o| format!("\"{}\"", o.name.text)).collect();
    writeln!(
        si,
        "{{\"rulec\":\"adapter/1\",\"rule\":\"{}\",\"in\":[{}],\"out\":[{}]}}",
        f.name.text,
        ins.join(","),
        outs.join(",")
    )
    .map_err(|e| e.to_string())?;
    si.flush().ok();

    let mut hello = String::new();
    so.read_line(&mut hello).map_err(|e| e.to_string())?;
    // Independent of whitespace. Our own template writes `"ok": true` via json.dumps, so a
    // naive string comparison would reject our own output.
    if field(&hello, "ok").map(scalar).as_deref() != Some("true") {
        return Err(tr!(
            "アダプタが握手を断りました: {}",
            "The adapter refused the handshake: {}",
            hello.trim()
        ));
    }
    let impl_id = field(&hello, "impl").map(scalar).unwrap_or_default();

    let mut rep =
        Report { impl_id, ..Report::new(f, if crate::i18n::ja() { "現行" } else { "legacy" }) };

    for (id, v) in vs.iter().enumerate() {
        let body = vectors::to_json(f, c, v);
        let inpart = field(&body, "in").map(|s| {
            let depth_end = s.find("},\"out\"").map(|i| i + 1).unwrap_or(s.len());
            s[..depth_end].to_string()
        });
        writeln!(si, "{{\"id\":{id},\"in\":{}}}", inpart.unwrap_or_default()).map_err(|e| e.to_string())?;
        si.flush().ok();

        let mut line = String::new();
        if so.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err(tr!("アダプタが {id} 件目で黙りました", "The adapter went silent at record {id}"));
        }
        rep.total += 1;
        let pairs: Vec<(String, Option<Val>, Option<String>)> = v
            .outputs
            .iter()
            .map(|(n, val)| (n.clone(), val.clone(), field(&line, n).map(scalar)))
            .collect();
        if let Some(e) = field(&line, "err") {
            rep.errored += 1;
            rep.mismatches.push(Mismatch {
                id,
                tag: String::new(),
                input: v.input.clone(),
                outs: pairs,
                err: Some(scalar(e)),
                fired: fired_of(v),
            });
            continue;
        }
        // With two or more outputs, a record matches only when all of them match (§8.5).
        if pairs.iter().all(|(n, a, b)| wire(c, n, a.as_ref()) == *b) {
            rep.agreed += 1;
        } else {
            rep.mismatches.push(Mismatch {
                id,
                tag: String::new(),
                input: v.input.clone(),
                outs: pairs,
                err: None,
                fired: fired_of(v),
            });
        }
    }
    drop(si);
    let _ = child.wait();
    Ok(rep)
}

/// §10.1: the adapter template. That it fits in 20 to 30 lines is the crux of this approach.
pub fn template(lang: &str, f: &RuleFile) -> String {
    let ins: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let out = f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    match lang {
        // The legacy implementation is not always a process to start. When it is a Connect
        // service, what goes in front of it is a client, and the template says so (§15.112).
        "connect-python" => crate::codegen::connect_template(f),
        "go" => tr!(
            "// rulec のアダプタのテンプレート（規則 {}）。\n\
             // 標準入出力で JSON Lines をやりとりするだけ。いま動いている実装をこの中から呼ぶ。\n\
             package main\n\n\
             import (\n\t\"bufio\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"os\"\n)\n\n\
             func main() {{\n\t\
             sc := bufio.NewScanner(os.Stdin)\n\t\
             sc.Buffer(make([]byte, 1<<20), 1<<20)\n\t\
             sc.Scan() // 握手\n\t\
             fmt.Println(`{{\"ok\":true,\"impl\":\"legacy@REPLACE_ME\"}}`)\n\t\
             for sc.Scan() {{\n\t\t\
             var req struct {{\n\t\t\tID  int            `json:\"id\"`\n\t\t\tIn  map[string]any `json:\"in\"`\n\t\t}}\n\t\t\
             if err := json.Unmarshal(sc.Bytes(), &req); err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\n\t\t\
             // ここでいまの実装を呼ぶ。入力は {}。\n\t\t\
             var got any = 0 // TODO: legacy.Compute(req.In)\n\n\t\t\
             b, _ := json.Marshal(map[string]any{{\"id\": req.ID, \"out\": map[string]any{{{:?}: got}}}})\n\t\t\
             fmt.Println(string(b))\n\t}}\n}}\n",
            "// rulec adapter template (rule {}).\n\
             // It only exchanges JSON Lines over stdin/stdout. Call the legacy implementation from here.\n\
             package main\n\n\
             import (\n\t\"bufio\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"os\"\n)\n\n\
             func main() {{\n\t\
             sc := bufio.NewScanner(os.Stdin)\n\t\
             sc.Buffer(make([]byte, 1<<20), 1<<20)\n\t\
             sc.Scan() // handshake\n\t\
             fmt.Println(`{{\"ok\":true,\"impl\":\"legacy@REPLACE_ME\"}}`)\n\t\
             for sc.Scan() {{\n\t\t\
             var req struct {{\n\t\t\tID  int            `json:\"id\"`\n\t\t\tIn  map[string]any `json:\"in\"`\n\t\t}}\n\t\t\
             if err := json.Unmarshal(sc.Bytes(), &req); err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\n\t\t\
             // Call the legacy implementation here. Inputs: {}.\n\t\t\
             var got any = 0 // TODO: legacy.Compute(req.In)\n\n\t\t\
             b, _ := json.Marshal(map[string]any{{\"id\": req.ID, \"out\": map[string]any{{{:?}: got}}}})\n\t\t\
             fmt.Println(string(b))\n\t}}\n}}\n",
            f.name.text,
            ins.join(", "),
            out
        ),
        _ => tr!(
            "# rulec のアダプタのテンプレート（規則 {}）。\n\
             # 標準入出力で JSON Lines をやりとりするだけ。いま動いている実装をこの中から呼ぶ。\n\
             import json, sys\n\n\
             sys.stdin.readline()  # 握手\n\
             print(json.dumps({{\"ok\": True, \"impl\": \"legacy@REPLACE_ME\"}}), flush=True)\n\n\
             for line in sys.stdin:\n    \
             line = line.strip()\n    \
             if not line:\n        \
             continue\n    \
             req = json.loads(line)\n    \
             d = req[\"in\"]  # 入力は {}\n\n    \
             # ここでいまの実装を呼ぶ。\n    \
             got = 0  # TODO: legacy.compute(d)\n\n    \
             print(json.dumps({{\"id\": req[\"id\"], \"out\": {{{:?}: got}}}}, ensure_ascii=False), flush=True)\n",
            "# rulec adapter template (rule {}).\n\
             # It only exchanges JSON Lines over stdin/stdout. Call the legacy implementation from here.\n\
             import json, sys\n\n\
             sys.stdin.readline()  # handshake\n\
             print(json.dumps({{\"ok\": True, \"impl\": \"legacy@REPLACE_ME\"}}), flush=True)\n\n\
             for line in sys.stdin:\n    \
             line = line.strip()\n    \
             if not line:\n        \
             continue\n    \
             req = json.loads(line)\n    \
             d = req[\"in\"]  # inputs: {}\n\n    \
             # Call the legacy implementation here.\n    \
             got = 0  # TODO: legacy.compute(d)\n\n    \
             print(json.dumps({{\"id\": req[\"id\"], \"out\": {{{:?}: got}}}}, ensure_ascii=False), flush=True)\n",
            f.name.text,
            ins.join(", "),
            out
        ),
    }
}

/// One property of the wire (§10.1): the canonical (Japanese) name, an integer in the
/// canonical unit, and a description that says which unit and, for a rate, what one step
/// is. That sentence is what a caller who never reads the rule — an agent handed the tool
/// (§15.44) — decides the value from, so it names the trap: 18.3% at a step of 0.1% is 183.
fn prop(name: &Name, ty: &Ty, c: &Checked, alias: bool) -> String {
    let key = if alias { name.ascii.clone().unwrap_or_else(|| name.text.clone()) } else { name.text.clone() };
    let name = name.text.as_str();
    let body = match ty {
        Ty::Enum(en) => {
            let vs: Vec<String> = c
                .enums
                .get(en)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|v| format!("{v:?}"))
                .collect();
            format!("{{\"type\":\"string\",\"enum\":[{}]}}", vs.join(","))
        }
        Ty::Bool => "{\"type\":\"boolean\"}".into(),
        Ty::Date => tr!(
            "{{\"type\":\"string\",\"format\":\"date\",\"description\":\"日付。YYYY-MM-DD\"}}",
            "{{\"type\":\"string\",\"format\":\"date\",\"description\":\"a date, YYYY-MM-DD\"}}"
        ),
        Ty::Str => "{\"type\":\"string\"}".into(),
        Ty::Opt(t) => format!("{{\"oneOf\":[{},{{\"type\":\"null\"}}]}}", prop_inner(t, c)),
        _ => {
            // The schema describes the wire, so the bounds are converted the same way a
            // value is: a rate's 100% is 100 steps, not 1 (§10.2).
            let sc = c.wire_scale(name);
            let unit = wire_unit(name, ty, c);
            let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
            let mut s = format!("{{\"type\":\"integer\",\"description\":\"{unit}\"");
            if let Some(l) = lo {
                s.push_str(&format!(",\"minimum\":{}", crate::types::wire_int(l, sc)));
            }
            if let Some(h) = hi {
                s.push_str(&format!(",\"maximum\":{}", crate::types::wire_int(h, sc)));
            }
            s.push('}');
            s
        }
    };
    // Keyed by alias, the rule's own name still travels, as the property's title.
    let body = if alias && key != name {
        format!("{{\"title\":{},{}", crate::json::quote(name), &body[1..])
    } else {
        body
    };
    format!("{}:{body}", crate::json::quote(&key))
}

/// What one number on the wire is, in words: which unit the integer is in, and for a rate
/// what one step of it counts.
///
/// The JSON Schema puts it in `description` and the `.proto` puts it in the comment beside
/// the field (§15.112). It is the same sentence either way, because it is the same fact:
/// a caller who never reads the rule decides the value from it, so the trap has to be named
/// where the value is written — 18.3% at a step of 0.1% is 183.
pub fn wire_unit(name: &str, ty: &Ty, c: &Checked) -> String {
    let sc = c.wire_scale(name);
    match ty {
        Ty::Money { cur, tax } => {
            let t = match tax.as_deref() {
                Some("incl_tax") => tr!("（税込）", " (tax included)"),
                Some("excl_tax") => tr!("（税抜）", " (tax excluded)"),
                _ => String::new(),
            };
            tr!("整数。単位は {cur}{t}", "an integer, in {cur}{t}")
        }
        Ty::Qty { unit, .. } => tr!("整数。単位は {unit}", "an integer, in {unit}"),
        Ty::Rate => {
            let step = if 100 % sc == 0 { format!("{}%", 100 / sc) } else { format!("{}%", 100.0 / sc as f64) };
            tr!(
                "整数。率を {step} 刻みの個数で書く（100% なら {sc}）",
                "an integer: the rate as a count of {step} steps (100% is {sc})"
            )
        }
        Ty::Number => tr!("整数（個数や日数のような、単位の無い数）", "an integer (a plain count of things or days)"),
        _ => String::new(),
    }
}

/// The `in` object of the wire: every input, all required, nothing else allowed. This is
/// also the `inputSchema` of the rule as an MCP tool (§15.44). `alias` keys the properties
/// by their ASCII aliases instead of the rule's names (§15.45).
pub fn schema_in(f: &RuleFile, c: &Checked, alias: bool) -> String {
    let mut ins: Vec<String> = f
        .inputs
        .iter()
        .map(|i| prop(&i.name, &c.ty_of(&i.name.text).unwrap_or(Ty::Unknown), c, alias))
        .collect();
    let key = |n: &Name| if alias { n.ascii.clone().unwrap_or_else(|| n.text.clone()) } else { n.text.clone() };
    let mut req: Vec<String> = f.inputs.iter().map(|i| crate::json::quote(&key(&i.name))).collect();
    // The sequence a walk reads: an array of objects, each of them the element's own fields
    // in the same wire (§15.56). It is required like any other input — a caller that sends
    // no sequence has not sent an empty one.
    if let Some(el) = &f.elements {
        let fields: Vec<String> = el
            .fields
            .iter()
            .map(|fd| prop(&fd.name, &c.ty_of(&fd.name.text).unwrap_or(Ty::Unknown), c, alias))
            .collect();
        let freq: Vec<String> = el.fields.iter().map(|fd| crate::json::quote(&key(&fd.name))).collect();
        ins.push(format!(
            "{}:{{\"type\":\"array\",\"items\":{{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}],\"additionalProperties\":false}}}}",
            crate::json::quote(&key(&el.name)),
            fields.join(","),
            freq.join(",")
        ));
        req.push(crate::json::quote(&key(&el.name)));
    }
    format!(
        "{{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}],\"additionalProperties\":false}}",
        ins.join(","),
        req.join(",")
    )
}

/// The `out` object of the wire: every output, with its range and unit.
pub fn schema_out(f: &RuleFile, c: &Checked, alias: bool) -> String {
    let outs: Vec<String> = f
        .outputs
        .iter()
        .map(|o| prop(&o.name, &c.ty_of(&o.name.text).unwrap_or(Ty::Unknown), c, alias))
        .collect();
    format!("{{\"type\":\"object\",\"properties\":{{{}}}}}", outs.join(","))
}

/// A precondition the entry guard holds a caller to that the **shape** of the input cannot
/// carry.
///
/// `rulec schema` says what the wire looks like: the fields, their types, and each one's own
/// range. A caller that validates against it has done everything JSON Schema can express —
/// and is still not done. A relation between two inputs (§15.55), a bound on the total of a
/// sequence (§15.100) and a cap on how many elements one may have (§15.58) are all refused
/// at the door by the generated code, and not one of the three is a shape.
///
/// Where the caller is a step of a workflow that went and fetched the sequence, learning
/// this by being refused is one boundary too late: the value is already recorded, and the
/// refusal happens again on every replay (§15.116).
#[derive(Debug, Clone)]
pub enum Pre {
    /// `constraint <left> <op> <right>`.
    Rel { left: String, op: &'static str, right: String },
    /// The total of one column over a sequence, bounded above.
    Sum { name: String, over: String, of: String, max: i128 },
    /// How many elements a sequence may have.
    Length { sequence: String, max: i128 },
}

/// Every precondition of the rule, in the order a reader meets them: the declared relations
/// first, then what the walk is held to.
pub fn preconditions(f: &RuleFile, c: &Checked) -> Vec<Pre> {
    use crate::ast::{AggKind, Item};
    let mut v: Vec<Pre> = f
        .constraints
        .iter()
        .map(|k| Pre::Rel { left: k.left.clone(), op: k.op.word(), right: k.right.clone() })
        .collect();
    let aggs: Vec<&crate::ast::AggDecl> =
        f.items.iter().filter_map(|it| if let Item::Agg(d) = it { Some(d) } else { None }).collect();
    for d in aggs.iter().filter(|d| d.kind == AggKind::Sum) {
        let Some(hi) = c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi) else { continue };
        let sc = c.scales.get(&d.name.text).copied().unwrap_or(1);
        v.push(Pre::Sum {
            name: d.name.text.clone(),
            over: d.over.clone(),
            of: d.column.text.clone(),
            max: crate::types::wire_int(hi, sc),
        });
    }
    // A count can be as large as the sequence, so the smallest upper bound any of them
    // declares is what the length is held to (§15.58). A `sum` says nothing about length.
    let cap = aggs
        .iter()
        .filter(|d| d.kind == AggKind::Count)
        .filter_map(|d| c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi))
        .map(|hi| crate::types::wire_int(hi, 1))
        .min();
    if let (Some(cap), Some(el)) = (cap, f.elements.as_ref()) {
        v.push(Pre::Length { sequence: el.name.text.clone(), max: cap });
    }
    v
}

/// The preconditions of §15.116, as data. Structured and not prose: a caller matches on the
/// kind of the entry, and the sentence the generated code refuses with moves with `--lang`
/// while these do not (§12.1).
pub fn preconditions_json(f: &RuleFile, c: &Checked) -> String {
    crate::json::arr(
        &preconditions(f, c)
            .iter()
            .map(|p| match p {
                Pre::Rel { left, op, right } => crate::json::Obj::new()
                    .str("kind", "constraint")
                    .str("left", left)
                    .str("op", op)
                    .str("right", right)
                    .finish(),
                Pre::Sum { name, over, of, max } => crate::json::Obj::new()
                    .str("kind", "sum")
                    .str("name", name)
                    .str("over", over)
                    .str("of", of)
                    .int("max", *max)
                    .finish(),
                Pre::Length { sequence, max } => crate::json::Obj::new()
                    .str("kind", "length")
                    .str("sequence", sequence)
                    .int("max", *max)
                    .finish(),
            })
            .collect::<Vec<_>>(),
    )
}

/// The kinds of precondition this rule has, named, or nothing when it has none. Only the
/// kinds that are actually there: a note that lists a kind the rule does not have teaches
/// the reader to skim the next one.
fn pre_kinds(f: &RuleFile, c: &Checked) -> Option<String> {
    let ps = preconditions(f, c);
    let mut words: Vec<String> = Vec::new();
    if ps.iter().any(|p| matches!(p, Pre::Rel { .. })) {
        words.push(tr!("入力どうしの関係", "a relation between two inputs"));
    }
    if ps.iter().any(|p| matches!(p, Pre::Sum { .. })) {
        words.push(tr!("並びの合計の上限", "a bound on a sequence's total"));
    }
    if ps.iter().any(|p| matches!(p, Pre::Length { .. })) {
        words.push(tr!("並びの長さの上限", "a cap on a sequence's length"));
    }
    if words.is_empty() {
        return None;
    }
    Some(words.join(if crate::i18n::ja() { "と" } else { ", " }))
}

/// §10.1: emits the wire format as a JSON Schema. Names are the canonical (Japanese) names,
/// or the ASCII aliases with `alias` (§15.45); values are integers in the canonical unit.
pub fn schema(f: &RuleFile, c: &Checked, alias: bool) -> String {
    tr!(
        "{{\n  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n  \
         \"title\": \"規則 {} v{}\",\n  \"type\": \"object\",\n  \
         \"properties\": {{\n    \"in\": {},\n    \"out\": {}\n  }}{}\n}}\n",
        "{{\n  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n  \
         \"title\": \"Rule {} v{}\",\n  \"type\": \"object\",\n  \
         \"properties\": {{\n    \"in\": {},\n    \"out\": {}\n  }}{}\n}}\n",
        f.name.text,
        f.version,
        schema_in(f, c, alias),
        schema_out(f, c, alias),
        // **The shape is necessary and not sufficient.** A caller that validates against it
        // and stops has done everything this document can ask for and can still be refused
        // at the door. `$comment` is the one place a schema may say so without pretending
        // to be a keyword a validator will act on (§15.116).
        match pre_kinds(f, c) {
            None => String::new(),
            Some(kinds) => tr!(
                ",\n  \"$comment\": \"入力の形だけでは足りません。ここに書けないものは、入口で断られます——{kinds}。`rulec api` の `preconditions` に並びます。\"",
                ",\n  \"$comment\": \"The shape is not the whole contract. What it cannot say is refused at the door instead: {kinds}. `rulec api` lists these under `preconditions`.\""
            ),
        }
    )
}

fn prop_inner(ty: &Ty, c: &Checked) -> String {
    match ty {
        Ty::Enum(en) => {
            let vs: Vec<String> = c
                .enums
                .get(en)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|v| format!("{v:?}"))
                .collect();
            format!("{{\"type\":\"string\",\"enum\":[{}]}}", vs.join(","))
        }
        Ty::Bool => "{\"type\":\"boolean\"}".into(),
        _ => "{\"type\":\"integer\"}".into(),
    }
}

/// A vector's fired rows, in the shape the clustering uses.
fn fired_of(v: &crate::vectors::Vector) -> Vec<crate::report::Fired> {
    v.fired
        .iter()
        .map(|(t, r)| crate::report::Fired::One { table: t.clone(), row: *r })
        .collect()
}
