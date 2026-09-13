//! Equivalence verification (§10).
//!
//! The legacy implementation is started as a child process and JSON Lines flow over
//! stdin/stdout. No FFI and no network: "a process and line-oriented JSON" is the smallest
//! surface shared across languages, and it is written the same way in Python and in Go.

use crate::ast::RuleFile;
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
pub fn run(f: &RuleFile, _c: &Checked, adapter: &[String], vs: &[Vector]) -> Result<Report, String> {
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
        Report { impl_id, ..Report::new(f, if crate::i18n::ja() { "旧" } else { "legacy" }) };

    for (id, v) in vs.iter().enumerate() {
        let body = vectors::to_json(f, v);
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
        if pairs.iter().all(|(_, a, b)| wire(a.as_ref()) == *b) {
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
        "go" => tr!(
            "// rulec のアダプタ雛形（規則 {}）。\n\
             // 標準入出力で JSON Lines をやりとりするだけ。旧実装をこの中から呼ぶ。\n\
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
             // ここで旧実装を呼ぶ。入力は {} 。\n\t\t\
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
            "# rulec のアダプタ雛形（規則 {}）。\n\
             # 標準入出力で JSON Lines をやりとりするだけ。旧実装をこの中から呼ぶ。\n\
             import json, sys\n\n\
             sys.stdin.readline()  # 握手\n\
             print(json.dumps({{\"ok\": True, \"impl\": \"legacy@REPLACE_ME\"}}), flush=True)\n\n\
             for line in sys.stdin:\n    \
             line = line.strip()\n    \
             if not line:\n        \
             continue\n    \
             req = json.loads(line)\n    \
             d = req[\"in\"]  # 入力は {} \n\n    \
             # ここで旧実装を呼ぶ。\n    \
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

/// §10.1: emits the wire format as a JSON Schema. Names are the canonical (Japanese) names;
/// values are integers in the canonical unit.
pub fn schema(f: &RuleFile, c: &Checked) -> String {
    let prop = |name: &str, ty: &Ty| -> String {
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
            Ty::Date => "{\"type\":\"string\",\"format\":\"date\"}".into(),
            Ty::Str => "{\"type\":\"string\"}".into(),
            Ty::Opt(t) => format!("{{\"oneOf\":[{},{{\"type\":\"null\"}}]}}", {
                let inner = prop_inner(t, c);
                inner
            }),
            _ => {
                let unit = match ty {
                    Ty::Money { cur, tax } => format!("{cur}, {}", tax.clone().unwrap_or_default()),
                    Ty::Qty { unit, .. } => unit.clone(),
                    Ty::Rate => tr!("率（刻み単位の整数）", "rate (integer in units of the step)"),
                    _ => String::new(),
                };
                let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
                let mut s = tr!(
                    "{{\"type\":\"integer\",\"description\":\"単位: {unit}\"",
                    "{{\"type\":\"integer\",\"description\":\"Unit: {unit}\""
                );
                if let Some(l) = lo {
                    s.push_str(&format!(",\"minimum\":{}", l.num / l.den));
                }
                if let Some(h) = hi {
                    s.push_str(&format!(",\"maximum\":{}", h.num / h.den));
                }
                s.push('}');
                s
            }
        };
        format!("\"{name}\":{body}")
    };
    let ins: Vec<String> = f
        .inputs
        .iter()
        .map(|i| prop(&i.name.text, &c.ty_of(&i.name.text).unwrap_or(Ty::Unknown)))
        .collect();
    let outs: Vec<String> = f
        .outputs
        .iter()
        .map(|o| prop(&o.name.text, &c.ty_of(&o.name.text).unwrap_or(Ty::Unknown)))
        .collect();
    let req: Vec<String> = f.inputs.iter().map(|i| format!("{:?}", i.name.text)).collect();
    tr!(
        "{{\n  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n  \
         \"title\": \"規則 {} v{}\",\n  \"type\": \"object\",\n  \
         \"properties\": {{\n    \"in\": {{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}],\"additionalProperties\":false}},\n    \
         \"out\": {{\"type\":\"object\",\"properties\":{{{}}}}}\n  }}\n}}\n",
        "{{\n  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n  \
         \"title\": \"Rule {} v{}\",\n  \"type\": \"object\",\n  \
         \"properties\": {{\n    \"in\": {{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}],\"additionalProperties\":false}},\n    \
         \"out\": {{\"type\":\"object\",\"properties\":{{{}}}}}\n  }}\n}}\n",
        f.name.text,
        f.version,
        ins.join(","),
        req.join(","),
        outs.join(",")
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
