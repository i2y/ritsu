//! Equivalence verification (§10).
//!
//! The legacy implementation is started as a child process and JSON Lines flow over
//! stdin/stdout. No FFI and no network: "a process and line-oriented JSON" is the smallest
//! surface shared across languages, and it is written the same way in Python and in Go.

use crate::ast::{Name, RuleFile};
use crate::json::Json;
use crate::types::{Checked, Ty};
use crate::report::{Mismatch, Out, Report};
use crate::vectors::{self, Vector};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// One line the adapter wrote, read as the JSON it is.
///
/// It used to be searched as text for `"名前":`. A key written `"\u540d\u524d"` — which is
/// what Python's `json.dumps` and PHP's `json_encode` write unless told otherwise — never
/// matched, every output read as missing, and an implementation that answered every case
/// right came out at 0%. A value fared no better: `"\u5165\u91d1"` was read as `u5165u91d1`
/// (§15.151).
fn parse_line(line: &str, what: &str) -> Result<Json, String> {
    // A byte order mark before the first line is no part of it: a runtime that writes one at the
    // head of its standard output answered the handshake with a line that was "not JSON".
    crate::json::parse(line.trim().trim_start_matches('\u{feff}').trim()).map_err(|e| {
        tr!(
            "アダプタの{what}を JSON として読めません: {e}\n  {}",
            "The adapter's {what} is not JSON: {e}\n  {}",
            line.trim()
        )
    })
}

/// An output's value as the adapter sent it, as the JSON it came as: a number as its digits, a
/// string in its quotes — so `"11"` where a number goes is told apart from 11, in the witness
/// and in `--format json` alike — and `null` as `null`.
fn observed(j: &Json) -> String {
    crate::json::unparse(j)
}

/// What to say of an adapter that closed its output where an answer was due: that it ended,
/// with its exit status when it has one by now, and where to look. The line that was read is
/// empty then, and calling it "not JSON" sent the reader to the wrong place: the adapter raised
/// on its first line — a module it could not import, a path that is not there — and what it
/// said is on standard error, above.
fn ended(child: &mut crate::child::Owned, at: &str) -> String {
    // It closed its output a moment ago; its status follows shortly when it is exiting.
    let mut status = None;
    for _ in 0..20 {
        if let Ok(Some(st)) = child.try_wait() {
            status = Some(st);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let how = match status {
        Some(st) => match st.code() {
            Some(code) => tr!("（終了コード {code}）", " (exit status {code})"),
            None => tr!("（シグナルで終了）", " (ended by a signal)"),
        },
        None => String::new(),
    };
    tr!(
        "アダプタが{at}終わりました{how}。アダプタが標準エラーに書いたものがあれば、この上に出ています",
        "The adapter ended {at}{how}. Whatever it wrote to standard error is above"
    )
}

/// Starts the adapter, streams the vectors through it and compares the answers.
pub fn run(f: &RuleFile, c: &Checked, adapter: &[String], vs: &[Vector]) -> Result<Report, String> {
    let (cmd, args) = adapter
        .split_first()
        .ok_or_else(|| tr!("アダプタのコマンドがありません", "No adapter command was given"))?;
    // An adapter that broke the protocol is stopped when this returns, rather than left to run
    // on after rulec (§15.163).
    let mut child = crate::child::Owned::spawn(Command::new(cmd).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()))
        .map_err(|e| if ritsu_base::wasi::unsupported(&e) { crate::i18n::text(&ritsu_base::wasi::cannot_start(cmd)) } else { tr!("アダプタを起動できません: {e}", "Cannot start the adapter: {e}") })?;
    let mut si = child
        .stdin
        .take()
        .ok_or_else(|| tr!("stdin を取得できません", "Cannot open the adapter's stdin"))?;
    let mut so = BufReader::new(
        child
            .stdout
            .take()
            .ok_or_else(|| tr!("stdout を取得できません", "Cannot open the adapter's stdout"))?,
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
    .and_then(|_| si.flush())
    .ok();

    let mut line = String::new();
    let before = tr!("ハンドシェイクに答える前に", "before answering the handshake");
    // A handshake that could not be written is an adapter that is gone already, and so is one
    // whose output ends here.
    if so.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
        return Err(ended(&mut child, &before));
    }
    let hello = parse_line(&line, &tr!("ハンドシェイクの応答", "answer to the handshake"))?;
    if hello.get("ok") != Some(&Json::Bool(true)) {
        return Err(tr!(
            "アダプタがハンドシェイクを受け付けませんでした: {}",
            "The adapter refused the handshake: {}",
            line.trim()
        ));
    }
    let impl_id = hello.get("impl").map(|j| j.as_str().map(str::to_string).unwrap_or_else(|| crate::json::show(j))).unwrap_or_default();

    let mut rep =
        Report { impl_id, records: false, ..Report::new(f, if crate::i18n::ja() { "現行" } else { "legacy" }) };

    for (id, v) in vs.iter().enumerate() {
        // A record that cannot be written went to an adapter that has ended; the read below finds
        // its output closed and says so, where the write's own error was "Broken pipe".
        let _ = writeln!(si, "{{\"id\":{id},\"in\":{}}}", vectors::in_object(f, c, v)).and_then(|_| si.flush());

        let mut line = String::new();
        if so.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err(ended(&mut child, &tr!("{id} 件目に答えずに", "without answering record {id}")));
        }
        let ans = parse_line(&line, &tr!("{id} 件目の応答", "answer to record {id}"))?;
        // An answer to another record would compare one case's values with another's.
        if let Some(got) = ans.get("id") {
            if got.as_int() != Some(id as i128) {
                // `"id":"0"` is no record's number, and reading it as one said "answered record 0
                // with the answer to record 0".
                if got.as_int().is_none() {
                    return Err(tr!(
                        "アダプタの {id} 件目の応答の id が整数ではありません: {}",
                        "The `id` of the adapter's answer to record {id} is not an integer: {}",
                        line.trim()
                    ));
                }
                let got = crate::json::show(got);
                return Err(tr!(
                    "アダプタが {id} 件目に {got} 件目の応答を返しました",
                    "The adapter answered record {id} with the answer to record {got}"
                ));
            }
        }
        let (out, err) = (ans.get("out"), ans.get("err"));
        if out.is_none() && err.is_none() {
            return Err(tr!(
                "アダプタの {id} 件目の応答に out も err もありません: {}",
                "The adapter's answer to record {id} has neither out nor err: {}",
                line.trim()
            ));
        }
        // `out` is the object of the outputs by name. `"out":11` names none of them, and read as
        // an answer it made every record a mismatch with nothing to show for it.
        if err.is_none() && !matches!(out, Some(Json::Obj(_))) {
            return Err(tr!(
                "アダプタの {id} 件目の応答の out がオブジェクトではありません。出力の名前ごとに値を入れます: {}",
                "The `out` of the adapter's answer to record {id} is not an object; it holds a value for each output by name: {}",
                line.trim()
            ));
        }
        rep.total += 1;
        // §15.199: each answer is read as the value it is — a decimal included, where the
        // implementation computed finer than the step — and compared exactly; the text it came
        // as is kept for the JSON, and is what a person sees of an answer that is no value of
        // the rule's type.
        let pairs: Vec<Out> = v
            .outputs
            .iter()
            .map(|(n, val)| {
                let j = out.and_then(|o| o.get(n));
                let theirs = j.and_then(|j| c.ty_of(n).and_then(|ty| crate::fixtures::observed_as(j, &ty, c, n, None).ok()));
                Out::against(c, n, val.clone(), theirs, j.map(observed))
            })
            .collect();
        if let Some(e) = err {
            rep.errored += 1;
            rep.mismatches.push(Mismatch {
                line: id + 1,
                tag: String::new(),
                input: v.input.clone(),
                outs: pairs,
                err: Some(e.as_str().map(str::to_string).unwrap_or_else(|| crate::json::show(e))),
                fired: fired_of(v),
                shown: Vec::new(),
                refusal: None,
            });
            continue;
        }
        // An answer the rule cannot produce is counted beside the headline (§15.199).
        rep.count_beyond(
            pairs.iter().filter(|o| o.theirs.as_ref().is_some_and(|t| crate::fixtures::beyond_reach(c, &o.name, t).is_some())).map(|o| o.name.as_str()),
        );
        // With two or more outputs, a record matches only when all of them match (§8.5).
        if pairs.iter().all(|o| o.same) {
            rep.agreed += 1;
        } else {
            rep.mismatches.push(Mismatch {
                line: id + 1,
                tag: String::new(),
                input: v.input.clone(),
                outs: pairs,
                err: None,
                fired: fired_of(v),
                shown: Vec::new(),
                refusal: None,
            });
        }
    }
    drop(si);
    let _ = child.wait();
    Ok(rep)
}

/// §10.1: the adapter template. That it fits in 20 to 30 lines is the crux of this approach.
pub fn template(lang: &str, f: &RuleFile) -> String {
    // What `in` holds: the inputs, and each sequence the rule walks with its elements' fields
    // (`lines[] (quantity, price)`). A rule that only walks used to be told its inputs were none.
    let ins: Vec<String> = f
        .inputs
        .iter()
        .map(|i| i.name.text.clone())
        .chain(f.elements.iter().map(|el| {
            format!("{}[] ({})", el.name.text, el.fields.iter().map(|fd| fd.name.text.clone()).collect::<Vec<_>>().join(", "))
        }))
        .collect();
    // `out` holds every output by name. The template used to write the first one alone, and a
    // rule with two came out of a filled-in template at 0%, each answer missing the second.
    let outs: Vec<String> = f.outputs.iter().map(|o| o.name.text.clone()).collect();
    let out = outs.iter().map(|n| format!("{n:?}: got")).collect::<Vec<_>>().join(", ");
    let many = if outs.len() > 1 {
        tr!("出力は {} の全部が要る。", " Every output is required: {}.", outs.join(if crate::i18n::ja() { "、" } else { ", " }))
    } else {
        String::new()
    };
    // An optional output answers that there is no value as `null` (§15.249), and the template says
    // how its language writes that.
    let optional: Vec<String> = f.outputs.iter().filter(|o| o.ty.optional).map(|o| o.name.text.clone()).collect();
    let absent = |none: &str| -> String {
        if optional.is_empty() {
            return many.clone();
        }
        let names = optional.join(if crate::i18n::ja() { "、" } else { ", " });
        tr!("{many}{names} に値が無いときは {none} を返す。", "{many} When {names} has no value, send {none}.")
    };
    let (py_none, go_none) = (absent("None"), absent("nil"));
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
             sc.Scan() // ハンドシェイク\n\t\
             fmt.Println(`{{\"ok\":true,\"impl\":\"legacy@REPLACE_ME\"}}`)\n\t\
             for sc.Scan() {{\n\t\t\
             var req struct {{\n\t\t\tID  int            `json:\"id\"`\n\t\t\tIn  map[string]any `json:\"in\"`\n\t\t}}\n\t\t\
             if err := json.Unmarshal(sc.Bytes(), &req); err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\n\t\t\
             // ここでいまの実装を呼ぶ。入力は {}。{go_none}\n\t\t\
             var got any = 0 // TODO: legacy.Compute(req.In)\n\n\t\t\
             b, _ := json.Marshal(map[string]any{{\"id\": req.ID, \"out\": map[string]any{{{}}}}})\n\t\t\
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
             // Call the legacy implementation here. Inputs: {}.{go_none}\n\t\t\
             var got any = 0 // TODO: legacy.Compute(req.In)\n\n\t\t\
             b, _ := json.Marshal(map[string]any{{\"id\": req.ID, \"out\": map[string]any{{{}}}}})\n\t\t\
             fmt.Println(string(b))\n\t}}\n}}\n",
            f.name.text,
            ins.join(", "),
            out
        ),
        _ => tr!(
            "# rulec のアダプタのテンプレート（規則 {}）。\n\
             # 標準入出力で JSON Lines をやりとりするだけ。いま動いている実装をこの中から呼ぶ。\n\
             import json, sys\n\n\
             sys.stdin.readline()  # ハンドシェイク\n\
             print(json.dumps({{\"ok\": True, \"impl\": \"legacy@REPLACE_ME\"}}), flush=True)\n\n\
             for line in sys.stdin:\n    \
             line = line.strip()\n    \
             if not line:\n        \
             continue\n    \
             req = json.loads(line)\n    \
             d = req[\"in\"]  # 入力は {}\n\n    \
             # ここでいまの実装を呼ぶ。{py_none}\n    \
             got = 0  # TODO: legacy.compute(d)\n\n    \
             print(json.dumps({{\"id\": req[\"id\"], \"out\": {{{}}}}}, ensure_ascii=False), flush=True)\n",
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
             # Call the legacy implementation here.{py_none}\n    \
             got = 0  # TODO: legacy.compute(d)\n\n    \
             print(json.dumps({{\"id\": req[\"id\"], \"out\": {{{}}}}}, ensure_ascii=False), flush=True)\n",
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
    let body = prop_body(name, ty, c);
    // Keyed by alias, the rule's own name still travels, as the property's title.
    let body = if alias && key != name {
        format!("{{\"title\":{},{}", crate::json::quote(name), &body[1..])
    } else {
        body
    };
    format!("{}:{body}", crate::json::quote(&key))
}

/// The schema of one value of a property: of the value an optional wraps too, which is the same
/// value with `null` beside it. An optional date was an integer here and an optional number had
/// no range, while the entry guard holds both to what the plain type is held to (§15.201). An
/// input of an ordered enum that declares a range lists the values the range lets in, as a number
/// has its `minimum` and `maximum`: the entry guard refuses the others (§15.210, §15.251).
fn prop_body(name: &str, ty: &Ty, c: &Checked) -> String {
    match ty {
        Ty::Enum(en) => {
            let vs: Vec<String> = c
                .enum_ranges
                .get(name)
                .or_else(|| c.enums.get(en))
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
        Ty::Opt(t) => format!("{{\"oneOf\":[{},{{\"type\":\"null\"}}]}}", prop_body(name, t, c)),
        _ => {
            // The schema describes the wire, so the bounds are converted the same way a
            // value is: a rate's 100% is 100 steps, not 1 (§10.2). An output's bounds are the
            // range its answers lie in, the one the port gives the other languages (§15.271): a
            // client that checks an answer against `outputSchema` checks it against that.
            let unit = wire_unit(name, ty, c);
            let (lo, hi) = c.wire_range(name).unwrap_or((None, None));
            let mut s = format!("{{\"type\":\"integer\",\"description\":\"{unit}\"");
            if let Some(l) = lo {
                s.push_str(&format!(",\"minimum\":{l}"));
            }
            if let Some(h) = hi {
                s.push_str(&format!(",\"maximum\":{h}"));
            }
            s.push('}');
            s
        }
    }
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
    // The sequences a walk reads: each an array of objects, each of them the element's own
    // fields in the same wire (§15.56, §15.219). It is required like any other input — a caller
    // that sends no sequence has not sent an empty one.
    for el in &f.elements {
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

/// The `out` object of the wire: every output, with its range and unit. The range is the one the
/// rule's answers lie in (§15.271), an output a `result` decides included.
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
/// sequence (§15.100), a cap on how many elements one may have (§15.58) and a field no two
/// elements may share (§15.290) are all refused at the door by the generated code, and not one
/// of them is a shape: `uniqueItems` compares whole elements, not one field of them.
///
/// Where the caller is a step of a workflow that went and fetched the sequence, learning
/// this by being refused is one boundary too late: the value is already recorded, and the
/// refusal happens again on every replay (§15.116).
#[derive(Debug, Clone)]
pub enum Pre {
    /// `constraint <left> <op> <right>`, each side the inputs it adds: one name, or several for a
    /// side written as a sum (§15.216).
    Rel { left: Vec<String>, op: &'static str, right: Vec<String> },
    /// The total of one column over a sequence, bounded above.
    Sum { name: String, over: String, of: String, max: i128 },
    /// How many elements a sequence may have.
    Length { sequence: String, max: i128 },
    /// No two elements of a sequence with the same value of a field marked `unique` (§15.290).
    Unique { sequence: String, field: String },
    /// A date input that takes only the days of a koyomi date (§15.174): the file and the date
    /// as the rule names them, and the days, as ordinals.
    Days { input: String, file: String, date: String, days: Vec<i64> },
    /// A `never` line the door reads (§15.282): the line, and what it tests on the wire.
    Never { said: String, conds: Vec<ritsu_ports::NeverCond> },
}

/// What one condition of a `never` line tests, on the wire: what a caller of the door can hold a
/// value to before it calls.
fn never_test(k: &crate::ast::NeverCond, c: &Checked) -> ritsu_ports::NeverTest {
    use crate::ast::{Cell, Lit};
    use ritsu_ports::{NeverTest, Value};
    let name = &k.name.text;
    let Some(ty) = c.ty_of(name) else { return NeverTest::Other };
    let ty = ty.present().clone();
    let sc = c.wire_scale(name);
    let on_wire = |n: &crate::lex::Num| crate::types::lit_value_in_pub(n, &ty).map(|v| crate::types::wire_int(v, sc));
    let value = |l: &Lit| -> Option<Value> {
        Some(match l {
            Lit::Word(w) if w == crate::kw::TRUE => Value::Bool(true),
            Lit::Word(w) if w == crate::kw::FALSE => Value::Bool(false),
            Lit::Word(w) => Value::Enum(w.clone()),
            Lit::Str(s) => Value::Str(s.clone()),
            Lit::Num(n) => Value::Int(on_wire(n)?),
            Lit::Date(y, m, d) => Value::Date(format!("{y:04}-{m:02}-{d:02}")),
        })
    };
    let among = |ls: &[Lit], yes: bool| -> NeverTest {
        // A word can be a group of values (§6.3): the values, as the door takes them.
        let mut values = Vec::new();
        for l in ls {
            match (l, c.groups.get(&crate::ast::lit_text(l))) {
                (Lit::Word(_), Some((_, members))) => values.extend(members.iter().cloned().map(Value::Enum)),
                _ => match value(l) {
                    Some(v) => values.push(v),
                    None => return NeverTest::Other,
                },
            }
        }
        NeverTest::Among { values, yes }
    };
    match &k.cell {
        Cell::Lit(l) => among(std::slice::from_ref(l), true),
        Cell::Set(ls) => among(ls, true),
        Cell::Not(ls) => among(ls, false),
        Cell::Ranked { values, .. } => NeverTest::Among { values: values.iter().cloned().map(Value::Enum).collect(), yes: true },
        Cell::Cmp(ops) => {
            let mut out = Vec::new();
            for (op, l) in ops {
                let Lit::Num(n) = l else { return NeverTest::Other };
                let Some(v) = on_wire(n) else { return NeverTest::Other };
                out.push((op.word().to_string(), v));
            }
            NeverTest::Compare(out)
        }
        _ => NeverTest::Other,
    }
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
    // A count can be as large as the sequence, so the smallest upper bound any of them over
    // one sequence declares is what its length is held to (§15.58). A `sum` says nothing about
    // length. One entry per sequence, in the order the rule declares them (§15.219).
    for el in &f.elements {
        let cap = aggs
            .iter()
            .filter(|d| d.kind == AggKind::Count && d.over == el.name.text)
            .filter_map(|d| c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi))
            .map(|hi| crate::types::wire_int(hi, 1))
            .min();
        if let Some(cap) = cap {
            v.push(Pre::Length { sequence: el.name.text.clone(), max: cap });
        }
        // Each field marked `unique`, in the order the sequence declares them (§15.290).
        for fd in el.fields.iter().filter(|fd| fd.unique.is_some()) {
            v.push(Pre::Unique { sequence: el.name.text.clone(), field: fd.name.text.clone() });
        }
    }
    // The days of a koyomi date, in the order the inputs are declared (§15.174).
    for i in &f.inputs {
        if let Some(d) = c.day_sets.get(&i.name.text)
            && let Some(fr) = &d.from
        {
            v.push(Pre::Days { input: i.name.text.clone(), file: fr.file.clone(), date: fr.date.clone(), days: d.days.clone() });
        }
    }
    // The `never` lines the door reads (§15.282).
    for n in f.door_nevers() {
        let conds = n.conds.iter().map(|k| ritsu_ports::NeverCond { name: k.name.text.clone(), test: never_test(k, c) }).collect();
        v.push(Pre::Never { said: n.said(), conds });
    }
    v
}

/// One condition of a `never` line, as data.
fn never_cond_json(k: &ritsu_ports::NeverCond) -> String {
    use ritsu_ports::{NeverTest, Value};
    let o = crate::json::Obj::new().str("name", &k.name);
    let value = |v: &Value| match v {
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Str(s) | Value::Date(s) | Value::Enum(s) => crate::json::quote(s),
        _ => "null".to_string(),
    };
    match &k.test {
        NeverTest::Among { values, yes } => o.raw("among", crate::json::arr(&values.iter().map(value).collect::<Vec<_>>())).bool("yes", *yes).finish(),
        NeverTest::Compare(ops) => o.raw("compare", crate::json::arr(&ops.iter().map(|(op, v)| crate::json::Obj::new().str("op", op).int("value", *v).finish()).collect::<Vec<_>>())).finish(),
        NeverTest::Other => o.bool("door_only", true).finish(),
    }
}

/// The preconditions of §15.116, as data. Structured and not prose: a caller matches on the
/// kind of the entry, and the sentence the generated code refuses with moves with `--lang`
/// while these do not (§12.1).
pub fn preconditions_json(f: &RuleFile, c: &Checked) -> String {
    crate::json::arr(
        &preconditions(f, c)
            .iter()
            .map(|p| match p {
                // A side of one input is its name, as before a side could add inputs up; a side that
                // does is the list of them (§15.216).
                Pre::Rel { left, op, right } => {
                    let side = |ns: &[String]| match ns {
                        [one] => crate::json::quote(one),
                        many => crate::json::strs(many),
                    };
                    crate::json::Obj::new().str("kind", "constraint").raw("left", side(left)).str("op", op).raw("right", side(right)).finish()
                }
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
                Pre::Unique { sequence, field } => crate::json::Obj::new()
                    .str("kind", "unique")
                    .str("sequence", sequence)
                    .str("field", field)
                    .finish(),
                Pre::Never { said, conds } => crate::json::Obj::new()
                    .str("kind", "never")
                    .str("said", said)
                    .raw("conds", crate::json::arr(&conds.iter().map(never_cond_json).collect::<Vec<_>>()))
                    .finish(),
                Pre::Days { input, file, date, days } => crate::json::Obj::new()
                    .str("kind", "days")
                    .str("input", input)
                    .raw("from", crate::json::Obj::new().str("tool", "koyomi").str("file", file).str("date", date).finish())
                    .raw(
                        "days",
                        crate::json::strs(
                            &days
                                .iter()
                                .map(|d| {
                                    let (y, m, dd) = crate::types::ord_to_date(crate::num::Rat::int(*d as i128));
                                    format!("{y:04}-{m:02}-{dd:02}")
                                })
                                .collect::<Vec<_>>(),
                        ),
                    )
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
    if ps.iter().any(|p| matches!(p, Pre::Rel { left, right, .. } if left.len() > 1 || right.len() > 1)) {
        words.push(tr!("入力どうしの関係", "a relation between inputs"));
    } else if ps.iter().any(|p| matches!(p, Pre::Rel { .. })) {
        words.push(tr!("入力どうしの関係", "a relation between two inputs"));
    }
    if ps.iter().any(|p| matches!(p, Pre::Sum { .. })) {
        words.push(tr!("並びの合計の上限", "a bound on a sequence's total"));
    }
    if ps.iter().any(|p| matches!(p, Pre::Length { .. })) {
        words.push(tr!("並びの長さの上限", "a cap on a sequence's length"));
    }
    if ps.iter().any(|p| matches!(p, Pre::Unique { .. })) {
        words.push(tr!("並びの要素どうしで同じ値をとらないフィールド", "a field no two elements of a sequence share"));
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
                ",\n  \"$comment\": \"入力の形だけでは足りません。ここに書けない条件（{kinds}）を満たさない入力は、生成コードが入口で受け付けません。その条件は `rulec api` の `preconditions` に並びます。\"",
                ",\n  \"$comment\": \"The shape is not the whole contract. What it cannot say is refused at the door instead: {kinds}. `rulec api` lists these under `preconditions`.\""
            ),
        }
    )
}

/// A vector's fired rows, in the shape the clustering uses.
fn fired_of(v: &crate::vectors::Vector) -> Vec<crate::report::Fired> {
    v.fired
        .iter()
        .map(|(t, r)| crate::report::Fired::One { table: t.clone(), row: *r })
        .collect()
}
