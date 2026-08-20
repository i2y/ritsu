//! 等価検証（§10）。
//!
//! 旧実装を子プロセスとして起動し、JSON Lines を stdin/stdout で流す。
//! FFI もネットワークも使わない。言語をまたぐ最小の共通面が「プロセスと行指向 JSON」で、
//! Python でも Go でも同じ書き方になる。

use crate::ast::RuleFile;
use crate::eval::Val;
use crate::types::{Checked, Ty};
use crate::report::{Mismatch, Report, wire};
use crate::vectors::{self, Vector};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// 素朴な JSON の値取り出し。依存を増やさないために必要な分だけ。
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

/// アダプタを起動し、ベクタを流して突き合わせる。
pub fn run(f: &RuleFile, c: &Checked, adapter: &[String], vs: &[Vector]) -> Result<Report, String> {
    let (cmd, args) = adapter.split_first().ok_or("アダプタのコマンドがありません")?;
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("アダプタを起動できません: {e}"))?;
    let mut si = child.stdin.take().ok_or("stdin を掴めません")?;
    let mut so = BufReader::new(child.stdout.take().ok_or("stdout を掴めません")?);

    // 握手（§10.1）。名前は `.rule` の正準名（和名）で送る。
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
    // 空白の有無に依存しない。自分の雛形が json.dumps で `"ok": true` と書くので、
    // 素朴な文字列一致では自分の出力を弾いてしまう。
    if field(&hello, "ok").map(scalar).as_deref() != Some("true") {
        return Err(format!("アダプタが握手を断りました: {}", hello.trim()));
    }
    let impl_id = field(&hello, "impl").map(scalar).unwrap_or_default();

    let mut rep =
        Report { impl_id, ..Report::new(f, "旧") };

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
            return Err(format!("アダプタが {id} 件目で黙りました"));
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
                trace: v.trace.clone(),
            });
            continue;
        }
        // 出力が二つ以上あれば、全部一致して初めて一致（§8.5）。
        if pairs.iter().all(|(_, a, b)| wire(a.as_ref()) == *b) {
            rep.agreed += 1;
        } else {
            rep.mismatches.push(Mismatch {
                id,
                tag: String::new(),
                input: v.input.clone(),
                outs: pairs,
                err: None,
                trace: v.trace.clone(),
            });
        }
    }
    drop(si);
    let _ = child.wait();
    Ok(rep)
}

/// §10.1: アダプタの雛形。20〜30 行で書けることが、この方式の肝。
pub fn template(lang: &str, f: &RuleFile) -> String {
    let ins: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let out = f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    match lang {
        "go" => format!(
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
            f.name.text,
            ins.join(", "),
            out
        ),
        _ => format!(
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
            f.name.text,
            ins.join(", "),
            out
        ),
    }
}

/// §10.1: ワイヤの形を JSON Schema で吐く。名前は正準名（和名）、値は正準単位の整数。
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
                    Ty::Rate => "率（刻み単位の整数）".into(),
                    _ => String::new(),
                };
                let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
                let mut s = format!("{{\"type\":\"integer\",\"description\":\"単位: {unit}\"");
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
    format!(
        "{{\n  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n  \
         \"title\": \"規則 {} v{}\",\n  \"type\": \"object\",\n  \
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
