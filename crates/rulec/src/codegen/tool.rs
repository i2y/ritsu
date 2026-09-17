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
        let mut names: Vec<String> = self.f.inputs.iter().map(|i| format!("{:?}", i.name.text)).collect();
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
        // The sequence a walk reads is one more argument, and one element is converted the way
        // one call's inputs are (§15.56).
        if let Some(el) = &self.f.elements {
            let jp = format!("{:?}", el.name.text);
            names.push(jp.clone());
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let k = format!("{:?}", fd.name.text);
                    let t = self.ty_of(&fd.name.text);
                    match &t {
                        Ty::Enum(n) => format!("_enum({k}, m.{}, _el({k}, e)[{k}])", self.enum_names.get(n).cloned().unwrap_or_default()),
                        Ty::Bool => format!("_bool({k}, _el({k}, e)[{k}])"),
                        Ty::Date => format!("_date({k}, _el({k}, e)[{k}])"),
                        Ty::Str => format!("_str({k}, _el({k}, e)[{k}])"),
                        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("m.{}(_int({k}, _el({k}, e)[{k}]))", brand_of(&t)),
                        _ => format!("_int({k}, _el({k}, e)[{k}])"),
                    }
                })
                .collect();
            convs.push(format!(
                "[m.Element({}) for e in _seq({jp}, o[{jp}])]",
                if fields.len() == 1 { format!("{},", fields[0]) } else { fields.join(", ") }
            ));
            tys.push("list[m.Element]".to_string());
        }
        let doc = tr!(
            "規則 {} v{} を、MCP ツール一つとして出す。\n\n    python3 {alias}_mcp.py [--record <file.jsonl>]                 # stdio\n    python3 {alias}_mcp.py --http 8000 [--origin https://example.com]  # Streamable HTTP\n\nツールの引数は規則の入力のワイヤ形式（宣言した単位の整数、率は刻みの個数、日付は YYYY-MM-DD、列挙はその名前）。\n結果は生成コードの record 関数が書く一行そのもの: in、observed、trace。\n--record を付けると、その一行を毎回ファイルに追記する。エージェントが尋ねたことが、そのまま rulec replay と rulec diff の読む記録になる。\n\nstdio は手元のエージェント（Claude Code、IDE）が使う。--http は、HTTPS の口しか受け付けない\n連携先（チャットのコネクタ、ワークフロー製品、業務向けエージェント）のためのもので、待つのは\n既定で 127.0.0.1 だけである。TLS と認証は前に置くこと。このサーバは自分では持たない。",
            "Rule {} v{} as one MCP tool.\n\n    python3 {alias}_mcp.py [--record <file.jsonl>]                 # stdio\n    python3 {alias}_mcp.py --http 8000 [--origin https://example.com]  # Streamable HTTP\n\nThe tool's arguments are the wire form of the rule's inputs (an integer in the declared unit,\na rate as a count of its steps, YYYY-MM-DD for a date, an enum member by its name), and its\nresult is the line the module's record function writes: in, observed, trace. With --record\nthat line is also appended to a file, so what the agent asked is what rulec replay and rulec\ndiff read later.\n\nStdio is for an agent on the same machine (Claude Code, an IDE). --http is for the places\nthat only accept an HTTPS endpoint (a chat client's connectors, a workflow product, a\nbusiness agent); it listens on 127.0.0.1 alone unless told otherwise. Put TLS and\nauthentication in front of it: this server carries neither.",
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
            .replace("@D_HANDLE@", &tr!("メッセージ一つを受けて、返すメッセージ一つを返す（通知には返さない）。二つの経路はここを通る。", "One message in, one message out (none for a notification). Both transports come through here."))
            .replace("@D_ORIGIN@", &tr!("Origin を見る。既定で通すのは手元からの呼び出しだけで、ブラウザが開いているページに\n    このサーバを叩かせないための検査である。ほかを通すなら --origin で名指しする。", "Check the Origin. By default only a caller on this machine is allowed, so that a page\n    open in a browser cannot reach this server; name any other origin with --origin."))
            .replace("@D_HTTP@", &tr!("MCP の Streamable HTTP で待つ。POST 一つに答え一つ。", "Listen for MCP's Streamable HTTP: one POST, one answer."))
            .replace("@D_QUIET@", &tr!("アクセスログは出さない。運びの話であって、この道具の声ではない。", "No access log: that is the transport talking, not this tool."))
            .replace("@D_POST@", &tr!("メッセージを一つ読んで、答えを JSON で返す。", "Read one message and answer it in JSON."))
            .replace("@D_GET@", &tr!("こちらから送るものは無いので、開く流れも無い。", "Nothing is ever sent unasked, so there is no stream to open."))
            .replace("@D_DELETE@", &tr!("セッションを終える。", "End the session."))
            .replace("@D_SEQ@", &tr!("列は配列で渡す。", "The sequence is passed as an array."))
            .replace("@D_EL@", &tr!("要素はオブジェクトで渡す。", "An element is passed as an object."))
            .replace("@M_SEQ@", &tr!("f\"{{name}}: 配列で渡す。{{v!r}} は配列ではない\"", "f\"{{name}}: an array is expected, not {{v!r}}\""))
            .replace("@M_EL@", &tr!("f\"{{name}}: 要素はオブジェクトで渡す。{{v!r}} はオブジェクトではない\"", "f\"{{name}}: an element must be an object, not {{v!r}}\""))
            .replace("@D_MAIN@", &tr!("引数を読んで、stdio か HTTP のどちらかで待つ。", "Read the arguments and listen, on stdio or on HTTP."))
            .replace("@D_PAGE@", &tr!("隣にある承認者向けのページ。無ければ None で、そのときツールは記録だけを返す。", "The approver's page from beside this file, or None — and then the tool answers with the record alone."))
            .replace("@UI_DESC@", &py_str(&self.ui_description()))
    }

    /// What a host shows about the view before it renders it.
    fn ui_description(&self) -> String {
        tr!(
            "規則 {} v{} の表。呼び出した件が入った状態で開き、当てはまった行に色が付く。",
            "The table of rule {} v{}, opened on the case it was called with, and the rows that decided it lit up.",
            self.f.name.text,
            self.f.version
        )
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
        let mut names: Vec<String> = self.f.inputs.iter().map(|i| format!("{:?}", i.name.text)).collect();
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
        // The sequence a walk reads, one element at a time (§15.56).
        if let Some(el) = &self.f.elements {
            let jp = format!("{:?}", el.name.text);
            names.push(jp.clone());
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let k = format!("{:?}", fd.name.text);
                    let t = self.ty_of(&fd.name.text);
                    let src = format!("_el({k}, e)[{k}]");
                    let body = match &t {
                        Ty::Enum(n) => format!("m.parse{}(_str({k}, {src}))", self.enum_names.get(n).cloned().unwrap_or_default()),
                        Ty::Bool => format!("_bool({k}, {src})"),
                        Ty::Date => format!("_date({k}, {src})"),
                        Ty::Str => format!("_str({k}, {src})"),
                        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                            let b = brand_of(&t);
                            if !brands.contains(&b) {
                                brands.push(b.clone());
                            }
                            format!("_int({k}, {src}) as {b}")
                        }
                        _ => format!("_int({k}, {src})"),
                    };
                    format!("{}: {body}", pub_name(&fd.name))
                })
                .collect();
            convs.push(format!("_seq({jp}, o[{jp}]).map((e) => ({{ {} }}))", fields.join(", ")));
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
            "規則 {} v{} を、MCP ツール一つとして出す。\n *\n *     node {alias}_mcp.ts [--record <file.jsonl>]                 // stdio\n *     node {alias}_mcp.ts --http 8000 [--origin https://example.com]  // Streamable HTTP\n *\n * ツールの引数は規則の入力のワイヤ形式（宣言した単位の整数、率は刻みの個数、日付は YYYY-MM-DD、列挙はその名前）。\n * 結果は生成コードの record 関数が書く一行そのもの: in、observed、trace。\n * --record を付けると、その一行を毎回ファイルに追記する。エージェントが尋ねたことが、そのまま rulec replay と rulec diff の読む記録になる。\n *\n * stdio は手元のエージェント（Claude Code、IDE）が使う。--http は、HTTPS の口しか受け付けない\n * 連携先（チャットのコネクタ、ワークフロー製品、業務向けエージェント）のためのもので、待つのは\n * 既定で 127.0.0.1 だけである。TLS と認証は前に置くこと。このサーバは自分では持たない。",
            "Rule {} v{} as one MCP tool.\n *\n *     node {alias}_mcp.ts [--record <file.jsonl>]                 // stdio\n *     node {alias}_mcp.ts --http 8000 [--origin https://example.com]  // Streamable HTTP\n *\n * The tool's arguments are the wire form of the rule's inputs (an integer in the declared unit,\n * a rate as a count of its steps, YYYY-MM-DD for a date, an enum member by its name), and its\n * result is the line the module's record function writes: in, observed, trace. With --record\n * that line is also appended to a file, so what the agent asked is what rulec replay and rulec\n * diff read later.\n *\n * Stdio is for an agent on the same machine (Claude Code, an IDE). --http is for the places\n * that only accept an HTTPS endpoint (a chat client's connectors, a workflow product, a\n * business agent); it listens on 127.0.0.1 alone unless told otherwise. Put TLS and\n * authentication in front of it: this server carries neither.",
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
            ("@D_HANDLE@", tr!("メッセージ一つを受けて、返すメッセージ一つを返す（通知には返さない）。二つの経路はここを通る。", "One message in, one message out (none for a notification). Both transports come through here.")),
            ("@D_ORIGIN@", tr!("Origin を見る。既定で通すのは手元からの呼び出しだけで、ブラウザが開いているページに\n * このサーバを叩かせないための検査である。ほかを通すなら --origin で名指しする。", "Check the Origin. By default only a caller on this machine is allowed, so that a page\n * open in a browser cannot reach this server; name any other origin with --origin.")),
            ("@D_HTTP@", tr!("MCP の Streamable HTTP で待つ。POST 一つに答え一つ。", "Listen for MCP's Streamable HTTP: one POST, one answer.")),
            ("@D_GET@", tr!("こちらから送るものは無いので、開く流れも無い。", "Nothing is ever sent unasked, so there is no stream to open.")),
            ("@D_SEQ@", tr!("列は配列で渡す。", "The sequence is passed as an array.")),
            ("@D_EL@", tr!("要素はオブジェクトで渡す。", "An element is passed as an object.")),
            ("@M_SEQ@", tr!("`${{name}}: 配列で渡す。${{JSON.stringify(v)}} は配列ではない`", "`${{name}}: an array is expected, not ${{JSON.stringify(v)}}`")),
            ("@M_EL@", tr!("`${{name}}: 要素はオブジェクトで渡す。${{JSON.stringify(v)}} はオブジェクトではない`", "`${{name}}: an element must be an object, not ${{JSON.stringify(v)}}`")),
            ("@D_PORT@", tr!("0 を渡せば空いている番号が選ばれるので、どこで待っているかを一行出す。", "A port of 0 means any free one, so where it is listening is printed as one line.")),
            ("@D_PAGE@", tr!("隣にある承認者向けのページ。無ければ null で、そのときツールは記録だけを返す。", "The approver's page from beside this file, or null — and then the tool answers with the record alone.")),
            ("@UI_DESC@", quote(&self.ui_description())),
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
import http.server
import json
import os
import sys
import uuid
from typing import BinaryIO, TypeVar

import @ALIAS@ as m

TOOL: dict[str, object] = json.loads(r'''@TOOL@''')

# MCP Apps (SEP-1865): the tool's view is the page an approver reads, written beside this
# server by the same `rulec gen`. A host that renders one gets the table, the case it was
# called with, and the rows that decided it; a host that does not gets the record alone.
UI_URI = "ui://@ALIAS@/table"
UI_MIME = "text/html;profile=mcp-app"
UI_EXT = "io.modelcontextprotocol/ui"
UI_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "@ALIAS@_page.html")
# Whether the client said it can render one. Set at initialize; the tool is offered without
# a view until it does (§SEP-1865: the UI is an enhancement, never a requirement).
_ui = False


def page() -> str | None:
    """@D_PAGE@"""
    try:
        with open(UI_FILE, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None

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


def _seq(name: str, v: object) -> list[object]:
    """@D_SEQ@"""
    if not isinstance(v, list):
        raise m.RuleInputError(@M_SEQ@)
    return v


def _el(name: str, v: object) -> dict[str, object]:
    """@D_EL@"""
    if not isinstance(v, dict):
        raise m.RuleInputError(@M_EL@)
    return v


def _args(d: object) -> tuple[@TYPES@]:
    o = _object(d, (@NAMES@))
@RETURN@


def call(d: object, tag: str = "") -> str:
    """@D_CALL@"""
    a = _args(d)
    out, trace = m.@ALIAS@_traced(*a)
    return m.@ALIAS@_record(*a, out, trace, tag)


def _ok(id_: object, result: object) -> dict[str, object]:
    return {"jsonrpc": "2.0", "id": id_, "result": result}


def _err(id_: object, code: int, message: str) -> dict[str, object]:
    return {"jsonrpc": "2.0", "id": id_, "error": {"code": code, "message": message}}


def handle(req: object, record: BinaryIO | None = None) -> dict[str, object] | None:
    """@D_HANDLE@"""
    if not isinstance(req, dict) or "id" not in req:
        return None
    id_ = req["id"]
    method = req.get("method")
    params = req.get("params")
    if not isinstance(params, dict):
        params = {}
    if method == "initialize":
        global _ui
        caps = params.get("capabilities")
        ext = caps.get("extensions") if isinstance(caps, dict) else None
        _ui = isinstance(ext, dict) and UI_EXT in ext and page() is not None
        return _ok(
            id_,
            {
                "protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": TOOL["name"], "version": "@VERSION@"},
                "instructions": @INSTRUCTIONS@,
            },
        )
    if method == "ping":
        return _ok(id_, {})
    if method == "tools/list":
        tool = dict(TOOL)
        if _ui:
            tool["_meta"] = {"ui": {"resourceUri": UI_URI}}
        return _ok(id_, {"tools": [tool]})
    if method == "resources/list":
        listed = [] if page() is None else [{"uri": UI_URI, "name": TOOL["name"], "description": @UI_DESC@, "mimeType": UI_MIME}]
        return _ok(id_, {"resources": listed})
    if method == "resources/read":
        html = page()
        if params.get("uri") != UI_URI or html is None:
            return _err(id_, -32602, f"unknown resource: {params.get('uri')!r}")
        return _ok(id_, {"contents": [{"uri": UI_URI, "mimeType": UI_MIME, "text": html}]})
    if method == "tools/call":
        if params.get("name") != TOOL["name"]:
            return _err(id_, -32602, f"unknown tool: {params.get('name')!r}")
        try:
            rec = call(params.get("arguments") or {})
        except m.RuleInputError as e:
            return _ok(id_, {"content": [{"type": "text", "text": f"RuleInputError: {e}"}], "isError": True})
        except m.RuleContradictionError as e:
            return _ok(id_, {"content": [{"type": "text", "text": f"RuleContradictionError: {e}"}], "isError": True})
        if record is not None:
            record.write((rec + "\n").encode("utf-8"))
            record.flush()
        return _ok(id_, {"content": [{"type": "text", "text": rec}], "structuredContent": json.loads(rec)})
    return _err(id_, -32601, f"unknown method: {method}")


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
            msg: dict[str, object] | None = _err(None, -32700, "parse error")
        else:
            msg = handle(req, record)
        if msg is None:
            continue
        out.write((json.dumps(msg, ensure_ascii=False) + "\n").encode("utf-8"))
        out.flush()


def _origin_ok(origin: str | None, allowed: tuple[str, ...]) -> bool:
    """@D_ORIGIN@"""
    if origin is None or origin in allowed:
        return True
    return origin.startswith(("http://localhost", "http://127.0.0.1", "http://[::1]"))


def serve_http(
    host: str, port: int, record: BinaryIO | None = None, allowed: tuple[str, ...] = ()
) -> None:
    """@D_HTTP@"""
    sessions: set[str] = set()

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"
        server_version = "@ALIAS@/@VERSION@"

        def log_message(self, format: str, *args: object) -> None:
            """@D_QUIET@"""

        def _send(self, code: int, body: bytes = b"", extra: dict[str, str] | None = None) -> None:
            self.send_response(code)
            if body:
                self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            for k, v in (extra or {}).items():
                self.send_header(k, v)
            self.end_headers()
            if body:
                self.wfile.write(body)

        def _json(self, code: int, msg: object, extra: dict[str, str] | None = None) -> None:
            self._send(code, json.dumps(msg, ensure_ascii=False).encode("utf-8"), extra)

        def do_POST(self) -> None:
            """@D_POST@"""
            if not _origin_ok(self.headers.get("Origin"), allowed):
                self._json(403, _err(None, -32000, "origin not allowed"))
                return
            sid = self.headers.get("Mcp-Session-Id")
            if sid is not None and sid not in sessions:
                self._json(404, _err(None, -32001, "no such session"))
                return
            try:
                n = int(self.headers.get("Content-Length") or 0)
                req = json.loads(self.rfile.read(n).decode("utf-8"))
            except ValueError:
                self._json(400, _err(None, -32700, "parse error"))
                return
            if isinstance(req, list):
                self._json(400, _err(None, -32600, "a batch is not accepted"))
                return
            msg = handle(req, record)
            if msg is None:
                self._send(202)
                return
            extra: dict[str, str] = {}
            if isinstance(req, dict) and req.get("method") == "initialize":
                new = uuid.uuid4().hex
                sessions.add(new)
                extra["Mcp-Session-Id"] = new
            self._json(200, msg, extra)

        def do_GET(self) -> None:
            """@D_GET@"""
            self._send(405)

        def do_DELETE(self) -> None:
            """@D_DELETE@"""
            sid = self.headers.get("Mcp-Session-Id")
            if sid is not None:
                sessions.discard(sid)
            self._send(204)

    srv = http.server.ThreadingHTTPServer((host, port), Handler)
    # The port is printed because 0 means "any free one", and because a caller that started
    # this process needs to know where to send the first request.
    print(f"http://{host}:{srv.socket.getsockname()[1]}/mcp", flush=True)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


USAGE = "usage: python3 @ALIAS@_mcp.py [--record <file.jsonl>] [--http [<host>:]<port>] [--origin <origin>]\n"


def main(argv: list[str]) -> int:
    """@D_MAIN@"""
    record_path: str | None = None
    listen: str | None = None
    allowed: list[str] = []
    rest = list(argv)
    while rest:
        flag = rest.pop(0)
        if flag == "--record" and rest:
            record_path = rest.pop(0)
        elif flag == "--http" and rest:
            listen = rest.pop(0)
        elif flag == "--origin" and rest:
            allowed.append(rest.pop(0))
        else:
            sys.stderr.write(USAGE)
            return 2
    host, _, port = listen.rpartition(":") if listen is not None else ("", "", "")
    if listen is not None and not port.isdigit():
        sys.stderr.write(USAGE)
        return 2
    f = open(record_path, "ab") if record_path is not None else None
    try:
        if listen is None:
            serve(f)
        else:
            serve_http(host or "127.0.0.1", int(port), f, tuple(allowed))
    finally:
        if f is not None:
            f.close()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
"#;

const TS_MCP: &str = r#"@HEADER@
/** @DOC@
 */
import { randomUUID } from "node:crypto";
import { appendFileSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import { createInterface } from "node:readline";
import * as m from "./@ALIAS@.ts";
@TYPE_IMPORTS@
const TOOL = @TOOL@;

const INSTRUCTIONS = @INSTRUCTIONS@;

// MCP Apps (SEP-1865): the tool's view is the page an approver reads, written beside this
// server by the same `rulec gen`. A host that renders one gets the table, the case it was
// called with, and the rows that decided it; a host that does not gets the record alone.
const UI_URI = "ui://@ALIAS@/table";
const UI_MIME = "text/html;profile=mcp-app";
const UI_EXT = "io.modelcontextprotocol/ui";
const UI_DESC = @UI_DESC@;
// Whether the client said it can render one. Set at initialize; the tool is offered without
// a view until it does — the UI is an enhancement, never a requirement.
let ui = false;

/** @D_PAGE@ */
function page(): string | null {
  try {
    return readFileSync(new URL("@ALIAS@_page.html", import.meta.url), "utf8");
  } catch {
    return null;
  }
}

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

/** @D_SEQ@ */
function _seq(name: string, v: unknown): unknown[] {
  if (!Array.isArray(v)) {
    throw new m.RuleInputError(@M_SEQ@);
  }
  return v;
}

/** @D_EL@ */
function _el(name: string, v: unknown): Record<string, unknown> {
  if (typeof v !== "object" || v === null || Array.isArray(v)) {
    throw new m.RuleInputError(@M_EL@);
  }
  return v as Record<string, unknown>;
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

function _ok(id: unknown, result: unknown): Record<string, unknown> {
  return { jsonrpc: "2.0", id, result };
}

function _err(id: unknown, code: number, message: string): Record<string, unknown> {
  return { jsonrpc: "2.0", id, error: { code, message } };
}

/** @D_HANDLE@ */
export function handle(req: unknown): Record<string, unknown> | null {
  if (typeof req !== "object" || req === null || !("id" in req)) {
    return null;
  }
  const r = req as Record<string, unknown>;
  const id = r.id;
  const method = r.method;
  const params = (r.params ?? {}) as Record<string, unknown>;
  if (method === "initialize") {
    const caps = (params.capabilities ?? {}) as Record<string, unknown>;
    const ext = (caps.extensions ?? {}) as Record<string, unknown>;
    ui = UI_EXT in ext && page() !== null;
    return _ok(id, {
      protocolVersion: params.protocolVersion ?? "2025-06-18",
      capabilities: { tools: {}, resources: {} },
      serverInfo: { name: TOOL.name, version: "@VERSION@" },
      instructions: INSTRUCTIONS,
    });
  }
  if (method === "ping") {
    return _ok(id, {});
  }
  if (method === "tools/list") {
    const tool = ui ? { ...TOOL, _meta: { ui: { resourceUri: UI_URI } } } : TOOL;
    return _ok(id, { tools: [tool] });
  }
  if (method === "resources/list") {
    const listed =
      page() === null ? [] : [{ uri: UI_URI, name: TOOL.name, description: UI_DESC, mimeType: UI_MIME }];
    return _ok(id, { resources: listed });
  }
  if (method === "resources/read") {
    const html = page();
    if (params.uri !== UI_URI || html === null) {
      return _err(id, -32602, `unknown resource: ${String(params.uri)}`);
    }
    return _ok(id, { contents: [{ uri: UI_URI, mimeType: UI_MIME, text: html }] });
  }
  if (method === "tools/call") {
    if (params.name !== TOOL.name) {
      return _err(id, -32602, `unknown tool: ${String(params.name)}`);
    }
    let rec: string;
    try {
      rec = call(params.arguments ?? {});
    } catch (e) {
      if (e instanceof m.RuleInputError || e instanceof m.RuleContradictionError) {
        return _ok(id, { content: [{ type: "text", text: `${e.name}: ${e.message}` }], isError: true });
      }
      throw e;
    }
    if (record !== null) {
      appendFileSync(record, rec + "\n");
    }
    return _ok(id, { content: [{ type: "text", text: rec }], structuredContent: JSON.parse(rec) });
  }
  return _err(id, -32601, `unknown method: ${String(method)}`);
}

/** @D_SERVE@ */
function onLine(raw: string): void {
  const line = raw.trim();
  if (line === "") {
    return;
  }
  let req: unknown;
  let msg: Record<string, unknown> | null;
  try {
    req = JSON.parse(line);
  } catch {
    msg = _err(null, -32700, "parse error");
    process.stdout.write(JSON.stringify(msg) + "\n");
    return;
  }
  msg = handle(req);
  if (msg !== null) {
    process.stdout.write(JSON.stringify(msg) + "\n");
  }
}

/** @D_ORIGIN@ */
function originOk(origin: string | null, allowed: readonly string[]): boolean {
  if (origin === null || allowed.includes(origin)) {
    return true;
  }
  return ["http://localhost", "http://127.0.0.1", "http://[::1]"].some((p) => origin.startsWith(p));
}

/** @D_HTTP@ */
function serveHttp(host: string, port: number, allowed: readonly string[]): void {
  const sessions: Set<string> = new Set();
  const srv = createServer((req, res) => {
    function send(code: number, body: string, extra: Record<string, string>): void {
      const head: Record<string, string | number> = { "Content-Length": Buffer.byteLength(body) };
      if (body !== "") {
        head["Content-Type"] = "application/json";
      }
      res.writeHead(code, { ...head, ...extra });
      res.end(body);
    }
    const origin = req.headers.origin;
    const sid = req.headers["mcp-session-id"];
    if (req.method === "DELETE") {
      if (typeof sid === "string") {
        sessions.delete(sid);
      }
      send(204, "", {});
      return;
    }
    if (req.method !== "POST") {
      // @D_GET@
      send(405, "", {});
      return;
    }
    if (!originOk(typeof origin === "string" ? origin : null, allowed)) {
      send(403, JSON.stringify(_err(null, -32000, "origin not allowed")), {});
      return;
    }
    if (typeof sid === "string" && !sessions.has(sid)) {
      send(404, JSON.stringify(_err(null, -32001, "no such session")), {});
      return;
    }
    let body = "";
    req.on("data", (chunk) => {
      body += String(chunk);
    });
    req.on("end", () => {
      let parsed: unknown;
      try {
        parsed = JSON.parse(body);
      } catch {
        send(400, JSON.stringify(_err(null, -32700, "parse error")), {});
        return;
      }
      if (Array.isArray(parsed)) {
        send(400, JSON.stringify(_err(null, -32600, "a batch is not accepted")), {});
        return;
      }
      const msg = handle(parsed);
      if (msg === null) {
        send(202, "", {});
        return;
      }
      const extra: Record<string, string> = {};
      if ((parsed as Record<string, unknown>).method === "initialize") {
        const fresh = randomUUID().replace(/-/g, "");
        sessions.add(fresh);
        extra["Mcp-Session-Id"] = fresh;
      }
      send(200, JSON.stringify(msg), extra);
    });
  });
  srv.listen(port, host, () => {
    // @D_PORT@
    const a = srv.address();
    const bound = typeof a === "object" && a !== null ? a.port : port;
    process.stdout.write(`http://${host}:${bound}/mcp\n`);
  });
}

const USAGE =
  "usage: node @ALIAS@_mcp.ts [--record <file.jsonl>] [--http [<host>:]<port>] [--origin <origin>]\n";

const argv = process.argv.slice(2);
let listen: string | null = null;
const allowed: string[] = [];
while (argv.length > 0) {
  const flag = argv.shift();
  const value = argv.shift();
  if (value === undefined) {
    process.stderr.write(USAGE);
    process.exit(2);
  } else if (flag === "--record") {
    record = value;
  } else if (flag === "--http") {
    listen = value;
  } else if (flag === "--origin") {
    allowed.push(value);
  } else {
    process.stderr.write(USAGE);
    process.exit(2);
  }
}
if (listen === null) {
  createInterface({ input: process.stdin, crlfDelay: Infinity }).on("line", onLine);
} else {
  const cut = listen.lastIndexOf(":");
  const host = cut < 0 ? "127.0.0.1" : listen.slice(0, cut) || "127.0.0.1";
  const port = Number(cut < 0 ? listen : listen.slice(cut + 1));
  if (!Number.isInteger(port)) {
    process.stderr.write(USAGE);
    process.exit(2);
  }
  serveHttp(host, port, allowed);
}
"#;
