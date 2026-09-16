//! The rule as one MCP tool (§15.44): a server for the agent that *calls* the rule,
//! generated beside the module it serves.
//!
//! `rulec mcp` is the commands as tools, for the agent that writes a rule. This is the other
//! side: the generated function as a tool, for an agent that has a judgement it must not
//! waver on. The server is generated code like the module — python3 or node alone, nothing
//! to install — and `rulec test` drives it over the same vectors the runner gets and holds
//! the record it returns to the reference evaluator byte for byte, so it sits inside the
//! word "proved" like everything else `gen` writes.
//!
//! The tool speaks the wire (§10.1): its arguments are the `in` object of `rulec schema`,
//! and its result is the line the module's record function writes — in, observed, trace —
//! so one tool call is one fixtures record, and `--record` appends every call to a file that
//! `replay` and `diff` read as it stands.

use super::{brand_of, pub_name, Gen};
use crate::json::{quote, strs, Obj};
use crate::types::Ty;

/// The shape of one fired row in the record, for the output schema.
const TRACE_SCHEMA: &str = "{\"type\":\"array\",\"items\":{\"type\":\"object\",\"properties\":{\"table\":{\"type\":\"string\"},\"row\":{\"type\":\"integer\"}},\"required\":[\"table\",\"row\"]}}";

impl<'a> Gen<'a> {
    /// The tool as MCP lists it: name, description, `inputSchema` (the wire `in`),
    /// `outputSchema` (a fixtures record).
    pub fn tool_json(&self) -> String {
        let output = Obj::new()
            .str("type", "object")
            .raw(
                "properties",
                Obj::new()
                    .raw("in", crate::verify::schema_in(self.f, self.c, false))
                    .raw("observed", crate::verify::schema_out(self.f, self.c, false))
                    .raw("trace", TRACE_SCHEMA)
                    .finish(),
            )
            .raw("required", strs(&["in", "observed", "trace"]))
            .finish();
        Obj::new()
            .str("name", pub_name(&self.f.name))
            .str("description", self.tool_description())
            .raw("inputSchema", crate::verify::schema_in(self.f, self.c, false))
            .raw("outputSchema", output)
            .finish()
    }

    /// What the calling agent reads before deciding whether to call, and how.
    fn tool_description(&self) -> String {
        let sep = if crate::i18n::ja() { "・" } else { ", " };
        let ins = self.f.inputs.iter().map(|i| i.name.text.as_str()).collect::<Vec<_>>().join(sep);
        let outs = self.f.outputs.iter().map(|o| o.name.text.as_str()).collect::<Vec<_>>().join(sep);
        let about = match &self.f.description {
            Some(d) => tr!("{d}。", "{d}. "),
            None => String::new(),
        };
        tr!(
            "規則 {} v{}（{}）: {about}{ins} から {outs} を決める。数は全部、説明にある単位の整数。率はその刻みの個数、日付は YYYY-MM-DD、列挙は挙げた名前のどれか。結果は一件の記録: in、observed（出力）、trace（決めた表と行）。",
            "Rule {} v{} ({}): {about}Decides {outs} from {ins}. Every number is an integer in the unit its description states; a rate is a count of its steps; a date is YYYY-MM-DD; an enum is one of the names listed. The result is one record: in, observed (the outputs), trace (the table rows that decided it).",
            self.f.name.text,
            self.f.version,
            pub_name(&self.f.name)
        )
    }

    /// The `instructions` of `initialize`: what a client shows its model once.
    fn tool_instructions(&self) -> String {
        tr!(
            "ツールは一つ、{}。結果は一件の記録で、isError が付いていれば呼び出し側の契約違反（範囲の外、列挙に無い値、整数でない数）で、本文がどの引数かを言う。",
            "One tool, {}. The result is one record; with isError the caller broke the contract (out of range, not a member of the enum, not an integer) and the text says which argument.",
            pub_name(&self.f.name)
        )
    }

    /// `python/<alias>_mcp.py`: the module as one MCP tool, over stdio, python3 alone.
    pub fn py_mcp(&self) -> String {
        let alias = pub_name(&self.f.name);
        let names: Vec<String> = self.f.inputs.iter().map(|i| format!("{:?}", i.name.text)).collect();
        let mut convs: Vec<String> = Vec::new();
        let mut tys: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let jp = format!("{:?}", i.name.text);
            let ty = self.ty_of(&i.name.text);
            let conv = |t: &Ty| -> String {
                match t {
                    Ty::Enum(n) => format!("_enum({jp}, m.{}, o[{jp}])", self.enum_names.get(n).cloned().unwrap_or_default()),
                    Ty::Bool => format!("_bool({jp}, o[{jp}])"),
                    Ty::Date => format!("_date({jp}, o[{jp}])"),
                    Ty::Str => format!("_str({jp}, o[{jp}])"),
                    Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("m.{}(_int({jp}, o[{jp}]))", brand_of(t)),
                    _ => format!("_int({jp}, o[{jp}])"),
                }
            };
            let qualified = |t: &Ty| -> String {
                let p = self.py_ty(t);
                if matches!(p.as_str(), "bool" | "int" | "str") { p } else { format!("m.{p}") }
            };
            match &ty {
                Ty::Opt(t) => {
                    convs.push(format!("None if o[{jp}] is None else {}", conv(t)));
                    tys.push(format!("{} | None", qualified(t)));
                }
                t => {
                    convs.push(conv(t));
                    tys.push(qualified(t));
                }
            }
        }
        let doc = tr!(
            "規則 {} v{} を、stdio の MCP ツール一つとして出す。\n\n    python3 {alias}_mcp.py [--record <file.jsonl>]\n\nツールの引数は規則の入力のワイヤ形式（宣言した単位の整数、率は刻みの個数、日付は YYYY-MM-DD、列挙はその名前）。\n結果は生成コードの record 関数が書く一行そのもの: in、observed、trace。\n--record を付けると、その一行を毎回ファイルに追記する。エージェントが尋ねたことが、そのまま rulec replay と rulec diff の読む記録になる。",
            "Rule {} v{} as one MCP tool, over stdio.\n\n    python3 {alias}_mcp.py [--record <file.jsonl>]\n\nThe tool's arguments are the wire form of the rule's inputs (an integer in the declared unit,\na rate as a count of its steps, YYYY-MM-DD for a date, an enum member by its name), and its\nresult is the line the module's record function writes: in, observed, trace. With --record\nthat line is also appended to a file, so what the agent asked is what rulec replay and rulec\ndiff read later.",
            self.f.name.text,
            self.f.version
        );
        PY_MCP
            .replace("@HEADER@", self.header("#").trim_end())
            .replace("@DOC@", &doc)
            .replace("@ALIAS@", &alias)
            .replace("@VERSION@", &self.f.version)
            .replace("@TOOL@", &self.tool_json())
            .replace("@INSTRUCTIONS@", &py_str(&self.tool_instructions()))
            // A one-element tuple needs its comma; a longer one must not have it, or the
            // formatter would spread the call over four lines.
            .replace("@NAMES@", &if names.len() == 1 { format!("{},", names[0]) } else { names.join(", ") })
            .replace("@TYPES@", &tys.join(", "))
            // One element sits on the line with its comma; two or more stand one per line,
            // and their trailing comma is what keeps the formatter from folding them back.
            .replace(
                "@RETURN@",
                &if convs.len() == 1 {
                    format!("    return ({},)", convs[0])
                } else {
                    format!("    return (\n{}    )", convs.iter().map(|c| format!("        {c},\n")).collect::<String>())
                },
            )
            .replace("@M_OBJECT@", &py_str(&tr!("引数はオブジェクト（名前と値の組）で渡す", "the arguments must be an object of name and value")))
            .replace("@M_EXTRA@", &tr!("f\"知らない引数: {{', '.join(extra)}}\"", "f\"unknown arguments: {{', '.join(extra)}}\""))
            .replace("@M_MISSING@", &tr!("f\"足りない引数: {{', '.join(missing)}}\"", "f\"missing arguments: {{', '.join(missing)}}\""))
            .replace("@M_INT@", &tr!("f\"{{name}}: 整数で渡す。{{v!r}} は整数ではない\"", "f\"{{name}}: an integer is expected, not {{v!r}}\""))
            .replace("@M_STR@", &tr!("f\"{{name}}: 文字列で渡す。{{v!r}} は文字列ではない\"", "f\"{{name}}: a string is expected, not {{v!r}}\""))
            .replace("@M_BOOL@", &tr!("f\"{{name}}: true か false で渡す。{{v!r}} はどちらでもない\"", "f\"{{name}}: true or false is expected, not {{v!r}}\""))
            .replace("@M_DATE@", &tr!("f\"{{name}}: 日付は YYYY-MM-DD で: {{v!r}}\"", "f\"{{name}}: not a YYYY-MM-DD date: {{v!r}}\""))
            .replace("@M_ENUM@", &tr!("f\"{{name}}: {{cls.__name__}} に無い: {{v!r}}\"", "f\"{{name}}: not in {{cls.__name__}}: {{v!r}}\""))
            .replace("@D_CALL@", &tr!("入力を一つの辞書で受け、規則を当てて、記録の一行を返す。", "Take the inputs as one dict, apply the rule, and return the record line."))
            .replace("@D_SERVE@", &tr!("stdin の JSON-RPC を一行ずつ読み、stdout に一行ずつ答える。", "Read JSON-RPC from stdin one line at a time and answer on stdout one line at a time."))
    }

    /// `typescript/<alias>_mcp.ts`: the same server, in erasable TypeScript, node alone.
    pub fn ts_mcp(&self) -> String {
        let (code, prose) = self.ts_mcp_parts();
        prose.iter().fold(code, |s, (k, v)| s.replace(k, v))
    }

    /// `javascript/<alias>_mcp.mjs`: the TypeScript server without its types (§15.36). The
    /// types are taken off before the prose goes in: a description is free to say "as a",
    /// and the stripper must not read that as a cast.
    pub fn js_mcp(&self) -> String {
        let alias = pub_name(&self.f.name);
        let (code, prose) = self.ts_mcp_parts();
        let js = super::strip_types(&code)
            .replace(".ts\";", ".mjs\";")
            .replace(&format!("node {alias}_mcp.ts"), &format!("node {alias}_mcp.mjs"));
        prose.iter().fold(js, |s, (k, v)| s.replace(k, v))
    }

    /// The server with its code filled in and its prose still as placeholders, and the
    /// prose to put in.
    fn ts_mcp_parts(&self) -> (String, Vec<(&'static str, String)>) {
        let alias = pub_name(&self.f.name);
        let names: Vec<String> = self.f.inputs.iter().map(|i| format!("{:?}", i.name.text)).collect();
        let mut convs: Vec<String> = Vec::new();
        let mut brands: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let jp = format!("{:?}", i.name.text);
            let ty = self.ty_of(&i.name.text);
            let mut conv = |t: &Ty| -> String {
                match t {
                    Ty::Enum(n) => format!("m.parse{}(_str({jp}, o[{jp}]))", self.enum_names.get(n).cloned().unwrap_or_default()),
                    Ty::Bool => format!("_bool({jp}, o[{jp}])"),
                    Ty::Date => format!("_date({jp}, o[{jp}])"),
                    Ty::Str => format!("_str({jp}, o[{jp}])"),
                    Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                        let b = brand_of(t);
                        if !brands.contains(&b) {
                            brands.push(b.clone());
                        }
                        format!("_int({jp}, o[{jp}]) as {b}")
                    }
                    _ => format!("_int({jp}, o[{jp}])"),
                }
            };
            match &ty {
                Ty::Opt(t) => convs.push(format!("o[{jp}] === null ? null : {}", conv(t))),
                t => convs.push(conv(t)),
            }
        }
        let code = TS_MCP
            .replace("@HEADER@", self.header("//").trim_end())
            .replace("@ALIAS@", &alias)
            .replace("@VERSION@", &self.f.version)
            .replace(
                "@TYPE_IMPORTS@",
                &if brands.is_empty() {
                    String::new()
                } else {
                    format!("import type {{ {} }} from \"./{alias}.ts\";\n", brands.join(", "))
                },
            )
            .replace("@NAMES@", &names.join(", "))
            .replace("@CONVS@", &convs.iter().map(|c| format!("    {c},\n")).collect::<String>());
        let doc = tr!(
            "規則 {} v{} を、stdio の MCP ツール一つとして出す。\n *\n *     node {alias}_mcp.ts [--record <file.jsonl>]\n *\n * ツールの引数は規則の入力のワイヤ形式（宣言した単位の整数、率は刻みの個数、日付は YYYY-MM-DD、列挙はその名前）。\n * 結果は生成コードの record 関数が書く一行そのもの: in、observed、trace。\n * --record を付けると、その一行を毎回ファイルに追記する。エージェントが尋ねたことが、そのまま rulec replay と rulec diff の読む記録になる。",
            "Rule {} v{} as one MCP tool, over stdio.\n *\n *     node {alias}_mcp.ts [--record <file.jsonl>]\n *\n * The tool's arguments are the wire form of the rule's inputs (an integer in the declared unit,\n * a rate as a count of its steps, YYYY-MM-DD for a date, an enum member by its name), and its\n * result is the line the module's record function writes: in, observed, trace. With --record\n * that line is also appended to a file, so what the agent asked is what rulec replay and rulec\n * diff read later.",
            self.f.name.text,
            self.f.version
        );
        let prose = vec![
            ("@DOC@", doc),
            ("@TOOL@", self.tool_json()),
            ("@INSTRUCTIONS@", quote(&self.tool_instructions())),
            ("@M_OBJECT@", quote(&tr!("引数はオブジェクト（名前と値の組）で渡す", "the arguments must be an object of name and value"))),
            ("@M_EXTRA@", tr!("`知らない引数: ${{extra.join(\", \")}}`", "`unknown arguments: ${{extra.join(\", \")}}`")),
            ("@M_MISSING@", tr!("`足りない引数: ${{missing.join(\", \")}}`", "`missing arguments: ${{missing.join(\", \")}}`")),
            ("@M_INT@", tr!("`${{name}}: 整数で渡す。${{JSON.stringify(v)}} は整数ではない`", "`${{name}}: an integer is expected, not ${{JSON.stringify(v)}}`")),
            ("@M_STR@", tr!("`${{name}}: 文字列で渡す。${{JSON.stringify(v)}} は文字列ではない`", "`${{name}}: a string is expected, not ${{JSON.stringify(v)}}`")),
            ("@M_BOOL@", tr!("`${{name}}: true か false で渡す。${{JSON.stringify(v)}} はどちらでもない`", "`${{name}}: true or false is expected, not ${{JSON.stringify(v)}}`")),
            ("@M_DATE@", tr!("`${{name}}: 日付は YYYY-MM-DD で渡す。${{JSON.stringify(v)}} は読めない`", "`${{name}}: a date as YYYY-MM-DD is expected, not ${{JSON.stringify(v)}}`")),
            ("@D_CALL@", tr!("入力を一つのオブジェクトで受け、規則を当てて、記録の一行を返す。", "Take the inputs as one object, apply the rule, and return the record line.")),
            ("@D_SERVE@", tr!("stdin の JSON-RPC を一行ずつ読み、stdout に一行ずつ答える。", "Read JSON-RPC from stdin one line at a time and answer on stdout one line at a time.")),
        ];
        (code, prose)
    }
}

/// A Python string literal for prose: double-quoted, with the two characters that need it
/// escaped.
fn py_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

const PY_MCP: &str = r#"@HEADER@
"""@DOC@
"""

from __future__ import annotations

import datetime
import enum
import json
import sys
from typing import BinaryIO, TypeVar

import @ALIAS@ as m

TOOL: dict[str, object] = json.loads(r'''@TOOL@''')

E = TypeVar("E", bound=enum.Enum)


def _object(d: object, names: tuple[str, ...]) -> dict[str, object]:
    if not isinstance(d, dict):
        raise m.RuleInputError(@M_OBJECT@)
    extra = [k for k in d if k not in names]
    if extra:
        raise m.RuleInputError(@M_EXTRA@)
    missing = [n for n in names if n not in d]
    if missing:
        raise m.RuleInputError(@M_MISSING@)
    return d


def _int(name: str, v: object) -> int:
    if not isinstance(v, int) or isinstance(v, bool):
        raise m.RuleInputError(@M_INT@)
    return v


def _str(name: str, v: object) -> str:
    if not isinstance(v, str):
        raise m.RuleInputError(@M_STR@)
    return v


def _bool(name: str, v: object) -> bool:
    if not isinstance(v, bool):
        raise m.RuleInputError(@M_BOOL@)
    return v


def _date(name: str, v: object) -> int:
    s = _str(name, v)
    try:
        y, mo, d = (int(x) for x in s.split("-"))
        return (datetime.date(y, mo, d) - datetime.date(1970, 1, 1)).days
    except ValueError:
        raise m.RuleInputError(@M_DATE@) from None


def _enum(name: str, cls: type[E], v: object) -> E:
    try:
        return cls(_str(name, v))
    except ValueError:
        raise m.RuleInputError(@M_ENUM@) from None


def _args(d: object) -> tuple[@TYPES@]:
    o = _object(d, (@NAMES@))
@RETURN@


def call(d: object, tag: str = "") -> str:
    """@D_CALL@"""
    a = _args(d)
    out, trace = m.@ALIAS@_traced(*a)
    return m.@ALIAS@_record(*a, out, trace, tag)


def _reply(out: BinaryIO, id_: object, result: object = None, error: object = None) -> None:
    msg: dict[str, object] = {"jsonrpc": "2.0", "id": id_}
    if error is not None:
        msg["error"] = error
    else:
        msg["result"] = result
    out.write((json.dumps(msg, ensure_ascii=False) + "\n").encode("utf-8"))
    out.flush()


def serve(record: BinaryIO | None = None) -> None:
    """@D_SERVE@"""
    out = sys.stdout.buffer
    for raw in sys.stdin.buffer:
        line = raw.decode("utf-8").strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except ValueError:
            _reply(out, None, error={"code": -32700, "message": "parse error"})
            continue
        if not isinstance(req, dict) or "id" not in req:
            continue
        id_ = req["id"]
        method = req.get("method")
        params = req.get("params") or {}
        if method == "initialize":
            _reply(
                out,
                id_,
                {
                    "protocolVersion": params.get("protocolVersion", "2025-06-18"),
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": TOOL["name"], "version": "@VERSION@"},
                    "instructions": @INSTRUCTIONS@,
                },
            )
        elif method == "ping":
            _reply(out, id_, {})
        elif method == "tools/list":
            _reply(out, id_, {"tools": [TOOL]})
        elif method == "tools/call":
            if params.get("name") != TOOL["name"]:
                _reply(out, id_, error={"code": -32602, "message": f"unknown tool: {params.get('name')!r}"})
                continue
            try:
                rec = call(params.get("arguments") or {})
            except m.RuleInputError as e:
                _reply(out, id_, {"content": [{"type": "text", "text": f"RuleInputError: {e}"}], "isError": True})
                continue
            except m.RuleContradictionError as e:
                _reply(out, id_, {"content": [{"type": "text", "text": f"RuleContradictionError: {e}"}], "isError": True})
                continue
            if record is not None:
                record.write((rec + "\n").encode("utf-8"))
                record.flush()
            _reply(out, id_, {"content": [{"type": "text", "text": rec}], "structuredContent": json.loads(rec)})
        else:
            _reply(out, id_, error={"code": -32601, "message": f"unknown method: {method}"})


if __name__ == "__main__":
    argv = sys.argv[1:]
    if argv[:1] == ["--record"] and len(argv) == 2:
        with open(argv[1], "ab") as f:
            serve(f)
    elif argv:
        sys.stderr.write("usage: python3 @ALIAS@_mcp.py [--record <file.jsonl>]\n")
        sys.exit(2)
    else:
        serve()
"#;

const TS_MCP: &str = r#"@HEADER@
/** @DOC@
 */
import { appendFileSync } from "node:fs";
import { createInterface } from "node:readline";
import * as m from "./@ALIAS@.ts";
@TYPE_IMPORTS@
const TOOL = @TOOL@;

const INSTRUCTIONS = @INSTRUCTIONS@;

function _object(d: unknown, names: readonly string[]): Record<string, unknown> {
  if (typeof d !== "object" || d === null || Array.isArray(d)) {
    throw new m.RuleInputError(@M_OBJECT@);
  }
  const o = d as Record<string, unknown>;
  const extra = Object.keys(o).filter((k) => !names.includes(k));
  if (extra.length > 0) {
    throw new m.RuleInputError(@M_EXTRA@);
  }
  const missing = names.filter((n) => !(n in o));
  if (missing.length > 0) {
    throw new m.RuleInputError(@M_MISSING@);
  }
  return o;
}

function _int(name: string, v: unknown): bigint {
  if (typeof v === "number" && Number.isInteger(v)) {
    return BigInt(v);
  }
  throw new m.RuleInputError(@M_INT@);
}

function _str(name: string, v: unknown): string {
  if (typeof v !== "string") {
    throw new m.RuleInputError(@M_STR@);
  }
  return v;
}

function _bool(name: string, v: unknown): boolean {
  if (typeof v !== "boolean") {
    throw new m.RuleInputError(@M_BOOL@);
  }
  return v;
}

function _date(name: string, v: unknown): bigint {
  const parts = _str(name, v).split("-").map(Number);
  if (parts.length !== 3 || parts.some((x) => !Number.isInteger(x))) {
    throw new m.RuleInputError(@M_DATE@);
  }
  const [y, mo, d] = parts;
  return BigInt(Math.round(Date.UTC(y, mo - 1, d) / 86400000));
}

function _args(d: unknown) {
  const o = _object(d, [@NAMES@]);
  return [
@CONVS@  ] as const;
}

/** @D_CALL@ */
export function call(d: unknown, tag = ""): string {
  const a = _args(d);
  const [out, trace] = m.@ALIAS@_traced(...a);
  return m.@ALIAS@_record(...a, out, trace, tag);
}

let record: string | null = null;

function _reply(id: unknown, result: unknown, error: unknown): void {
  const msg: Record<string, unknown> = { jsonrpc: "2.0", id };
  if (error !== undefined) {
    msg.error = error;
  } else {
    msg.result = result;
  }
  process.stdout.write(JSON.stringify(msg) + "\n");
}

/** @D_SERVE@ */
function onLine(raw: string): void {
  const line = raw.trim();
  if (line === "") {
    return;
  }
  let req: unknown;
  try {
    req = JSON.parse(line);
  } catch {
    _reply(null, undefined, { code: -32700, message: "parse error" });
    return;
  }
  if (typeof req !== "object" || req === null || !("id" in req)) {
    return;
  }
  const r = req as Record<string, unknown>;
  const id = r.id;
  const method = r.method;
  const params = (r.params ?? {}) as Record<string, unknown>;
  if (method === "initialize") {
    _reply(
      id,
      {
        protocolVersion: params.protocolVersion ?? "2025-06-18",
        capabilities: { tools: {} },
        serverInfo: { name: TOOL.name, version: "@VERSION@" },
        instructions: INSTRUCTIONS,
      },
      undefined,
    );
  } else if (method === "ping") {
    _reply(id, {}, undefined);
  } else if (method === "tools/list") {
    _reply(id, { tools: [TOOL] }, undefined);
  } else if (method === "tools/call") {
    if (params.name !== TOOL.name) {
      _reply(id, undefined, { code: -32602, message: `unknown tool: ${String(params.name)}` });
      return;
    }
    let rec: string;
    try {
      rec = call(params.arguments ?? {});
    } catch (e) {
      if (e instanceof m.RuleInputError || e instanceof m.RuleContradictionError) {
        _reply(id, { content: [{ type: "text", text: `${e.name}: ${e.message}` }], isError: true }, undefined);
        return;
      }
      throw e;
    }
    if (record !== null) {
      appendFileSync(record, rec + "\n");
    }
    _reply(id, { content: [{ type: "text", text: rec }], structuredContent: JSON.parse(rec) }, undefined);
  } else {
    _reply(id, undefined, { code: -32601, message: `unknown method: ${String(method)}` });
  }
}

const argv = process.argv.slice(2);
if (argv.length === 2 && argv[0] === "--record") {
  record = argv[1];
} else if (argv.length > 0) {
  process.stderr.write("usage: node @ALIAS@_mcp.ts [--record <file.jsonl>]\n");
  process.exit(2);
}
createInterface({ input: process.stdin, crlfDelay: Infinity }).on("line", onLine);
"#;
