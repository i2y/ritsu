//! Line-oriented parser for `.rule` (§1.1, §1.2).
//!
//! The file structure is fixed, so this is a dispatch on the first word of each line
//! rather than a general grammar. Tables are runs of `|` lines.

use crate::ast::*;
use crate::diag::{Diag, Span};
use crate::lex::{Kind, Token, lex_line_soft};

/// The keywords that can start a line. Used to detect E009 (a declaration named like a
/// reserved word) and to check that the README's keyword table matches this list.
/// `rule` is header-only, so it is not included.
pub const KEYWORDS: &[&str] = crate::kw::LINE_HEAD;

pub struct Parsed {
    pub file: Option<RuleFile>,
    pub diags: Vec<Diag>,
}

struct P {
    lines: Vec<Vec<Token>>,
    i: usize,
    diags: Vec<Diag>,
    path: String,
    /// §11 principle 4: a location is file:line plus the table name.
    ctx: String,
}

pub fn parse(src: &str, path: &str) -> Parsed {
    let mut lines: Vec<Vec<Token>> = Vec::new();
    let mut diags: Vec<Diag> = Vec::new();
    for (n, text) in src.lines().enumerate() {
        match lex_line_soft(n + 1, text) {
            // A line that is nothing but a comment is not a blank line, and a blank line is
            // what ends a block. After lexing the two look alike — both come back with no
            // tokens — so a comment written between two inputs cut the block in half and the
            // declaration under it was reported as a word that cannot appear there. Dropping
            // it here is what makes a note in the middle of a block mean what it looks like.
            Ok((t, _)) if t.is_empty() && text.trim_start().starts_with('#') => {}
            Ok((t, soft)) => {
                diags.extend(soft.into_iter().map(|d| placed(d, path, n + 1)));
                lines.push(t);
            }
            Err(d) => {
                diags.push(placed(d, path, n + 1));
                lines.push(Vec::new());
            }
        }
    }
    let mut p = P { lines, i: 0, diags, path: path.to_string(), ctx: String::new() };
    let file = p.rule_file();
    Parsed { file, diags: p.diags }
}

/// A diagnostic from the lexer, which sees one line and no file, placed in the file the way
/// the parser's own are: `file:line`.
fn placed(d: Diag, path: &str, line: usize) -> Diag {
    if d.where_.is_empty() {
        d.at(format!("{path}:{line}"))
    } else {
        d
    }
}

/// Span covering a whole token run.
fn span_of(ts: &[Token]) -> Span {
    match (ts.first(), ts.last()) {
        (Some(a), Some(b)) => Span::new(a.span.line, a.span.col, b.span.col + b.span.len - a.span.col),
        _ => Span::new(1, 0, 1),
    }
}

/// The words whose line declares one thing. A block heading (`inputs`, `examples`, …) is not
/// among them: its heading alone is complete, and its rows speak for themselves.
const DECLARING: &[&str] = &[
    crate::kw::ENUM,
    crate::kw::GROUP,
    crate::kw::DERIVE,
    crate::kw::DEFINE,
    crate::kw::SHAPE,
    crate::kw::SOURCE,
    crate::kw::APPLY,
    crate::kw::COUNT,
    crate::kw::SUM,
    crate::kw::FOLD,
    crate::kw::CONSTRAINT,
    crate::kw::TABLE,
    crate::kw::CLAUSE,
    crate::kw::RESULT,
    crate::kw::MACHINE,
];

/// How many declarations the file holds so far. A line that leaves this and the diagnostics
/// as they were has been read into nothing.
fn declared(f: &RuleFile) -> usize {
    f.imports.len()
        + f.enum_imports.len()
        + f.sources.len()
        + f.applies.len()
        + f.enums.len()
        + f.groups.len()
        + f.inputs.len()
        + f.outputs.len()
        + f.items.len()
        + f.examples.len()
        + f.constraints.len()
        + f.shapes.len()
        + f.sequences.len()
        + f.scenarios.len()
        + usize::from(f.description.is_some())
        + usize::from(f.result.is_some())
        + usize::from(f.elements.is_some())
        + usize::from(f.fold.is_some())
        + usize::from(f.machine.is_some())
}

/// How a line that starts with `word` is written, for the note under E058.
fn line_shape(word: &str) -> String {
    match word {
        crate::kw::DESCRIPTION => tr!("`description \"<説明>\"`", "`description \"<one line>\"`"),
        crate::kw::ENUM => tr!("`enum <名前>(<別名>) = <値>(<別名>) | …`", "`enum <name>(<alias>) = <value>(<alias>) | …`"),
        crate::kw::GROUP => tr!("`group <名前>(<別名>) = <値>, …`", "`group <name>(<alias>) = <value>, …`"),
        crate::kw::DERIVE => tr!("`derive <名前>(<別名>) : <型> = <式>  range >=… <=…`", "`derive <name>(<alias>) : <type> = <expression>  range >=… <=…`"),
        crate::kw::DEFINE => tr!("`define <名前>(<別名>) : <型> = <式>`", "`define <name>(<alias>) : <type> = <expression>`"),
        crate::kw::RESULT => tr!("`result <出力> = <式>`", "`result <output> = <expression>`"),
        crate::kw::SHAPE => tr!("`shape <名前>(<別名>) = jsonschema \"<ファイル>\" \"<ポインタ>\"`", "`shape <name>(<alias>) = jsonschema \"<file>\" \"<pointer>\"`"),
        crate::kw::SOURCE => tr!("`source <名前> = law \"<法令番号>\" asof <日付>` か `source <名前> = file \"<ファイル>\" sha256:<ハッシュ>`", "`source <name> = law \"<law id>\" asof <date>` or `source <name> = file \"<file>\" sha256:<digest>`"),
        crate::kw::APPLY => tr!("`apply <名前>(<別名>) = \"<規則のファイル>\" sha256:<ハッシュ>` と、その下に入力ごとの読み替え", "`apply <name>(<alias>) = \"<rule file>\" sha256:<digest>`, with one binding per input under it"),
        crate::kw::COUNT => tr!("`count <名前>(<別名>) over <並び> where <列> = <値>  range >=… <=…`", "`count <name>(<alias>) over <sequence> where <column> = <value>  range >=… <=…`"),
        crate::kw::SUM => tr!("`sum <名前>(<別名>) over <並び> of <列>  range >=… <=…`", "`sum <name>(<alias>) over <sequence> of <column>  range >=… <=…`"),
        crate::kw::FOLD => tr!("`fold <判定の列> over <並び>`", "`fold <verdict column> over <sequence>`"),
        crate::kw::CONSTRAINT => tr!("`constraint <入力> <= <入力>`", "`constraint <input> <= <input>`"),
        crate::kw::TABLE => tr!("`table <名前>(<別名>)` と、その下に `policy` と表の行", "`table <name>(<alias>)`, with `policy` and the rows under it"),
        crate::kw::CLAUSE => tr!("`clause <名前>(<別名>) -> <出力>` と、その下に `when …` と `then …`", "`clause <name>(<alias>) -> <output>`, with `when …` and `then …` under it"),
        crate::kw::MACHINE => tr!("`machine <名前>(<別名>) over <表>` と、その下に `carry`・`initial`・`final`", "`machine <name>(<alias>) over <table>`, with `carry`, `initial` and `final` under it"),
        _ => format!("`{word} …`"),
    }
}

/// E058: a line that starts with a word of the vocabulary and cannot be read as what that
/// word declares. Such a line used to be dropped with nothing said.
fn unreadable(word: &str, line: &[Token], what: String, at: String) -> Diag {
    Diag::error("E058", tr!("`{word}` の行を読めません", "The `{word}` line cannot be read"))
        .at(at)
        .mark(span_of(line), what)
        .note(tr!("形は {} です。", "The shape is {}.", line_shape(word)))
        .note(tr!(
            "読めない行を飛ばして検査を続けると、書いた規則とは別の規則を検査することになります。",
            "Skipping a line that cannot be read would check a rule other than the one written."
        ))
}

impl P {
    fn cur(&self) -> Option<&Vec<Token>> {
        self.lines.get(self.i)
    }
    fn skip_blank(&mut self) {
        while self.cur().is_some_and(|l| l.is_empty()) {
            self.i += 1;
        }
    }
    fn err(&mut self, d: Diag) {
        self.diags.push(d);
    }

    /// `path:line 表 名前` (`path:line table name` in English) — §11 principle 4.
    fn at(&self, line: usize) -> String {
        if self.ctx.is_empty() {
            format!("{}:{}", self.path, line)
        } else {
            format!("{}:{} {}", self.path, line, self.ctx)
        }
    }

    fn rule_file(&mut self) -> Option<RuleFile> {
        self.skip_blank();
        let head = self.cur()?.clone();
        if head.first().and_then(|t| t.ident()) != Some(crate::kw::RULE) {
            self.err(
                Diag::error("E003", tr!("ファイルは `{}` の行で始まらなければなりません", "The file must start with a `{}` line", crate::kw::RULE))
                    .mark(span_of(&head), tr!("ここに `{} <名前>(<ascii>) v<版>` が要ります", "expected `{} <name>(<ascii>) v<version>` here", crate::kw::RULE)),
            );
            return None;
        }
        // A `rule` line with no name used to end the parse with nothing said, and the file
        // passed the check with no declarations in it (§15.156).
        let Some((name, mut k)) = self.name_at(&head, 1) else {
            self.err(
                Diag::error("E003", tr!("`{}` の行に規則の名前がありません", "The `{}` line has no name", crate::kw::RULE))
                    .mark(span_of(&head), tr!("ここに `{} <名前>(<ascii>) v<版>` が要ります", "expected `{} <name>(<ascii>) v<version>` here", crate::kw::RULE)),
            );
            return None;
        };
        let version = head
            .get(k)
            .and_then(|t| t.ident())
            .map(|s| s.trim_start_matches('v').to_string())
            .unwrap_or_default();
        k += 1;
        let _ = k;
        self.i += 1;

        let mut f = RuleFile {
            name,
            version,
            description: None,
            imports: Vec::new(),
            enum_imports: Vec::new(),
            sources: Vec::new(),
            applies: Vec::new(),
            enums: Vec::new(),
            groups: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            items: Vec::new(),
            result: None,
            examples: Vec::new(),
            constraints: Vec::new(),
            shapes: Vec::new(),
            elements: None,
            sequences: Vec::new(),
            fold: None,
            machine: None,
            scenarios: Vec::new(),
        };

        loop {
            self.skip_blank();
            let Some(line) = self.cur().cloned() else { break };
            let Some(word) = line.first().and_then(|t| t.ident()).map(|s| s.to_string()) else {
                self.err(
                    Diag::error("E004", tr!("行の先頭に語がありません", "The line does not start with a word"))
                        .mark(span_of(&line), ""),
                );
                self.i += 1;
                continue;
            };
            // Two guards hold around every line that starts with a word. The parser always
            // moves on: a handler that returned without consuming its line read that line
            // for ever, and a malformed `derive` did exactly that (§15.156). And a line that
            // declares nothing is never dropped in silence: if it added nothing to the file
            // and said nothing about why, E058 says it.
            let start = self.i;
            let before = (self.diags.len(), declared(&f));
            match word.as_str() {
                crate::kw::DESCRIPTION => {
                    match line.get(1).map(|t| t.kind.clone()) {
                        Some(Kind::Str(s)) if line.len() == 2 => f.description = Some(s),
                        _ => self.err(unreadable(&word, &line, tr!("`{}` の後には文字列を一つだけ書きます", "`{}` takes one string and nothing else", crate::kw::DESCRIPTION), self.at(span_of(&line).line))),
                    }
                    self.i += 1;
                }
                crate::kw::IMPORT => {
                    // Two kinds of import, told apart by the word after it: the built-in
                    // namespace, and an enum a `.proto` owns (§15.59).
                    let kind = match line.get(1).and_then(|t| t.ident()) {
                        Some(crate::kw::PROTO) => Some(EnumSource::Proto),
                        Some(crate::kw::JSONSCHEMA) => Some(EnumSource::JsonSchema),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        if let Some(p) = self.enum_import(&line, kind) {
                            f.enum_imports.push(p);
                        }
                    } else {
                        let path: String = line[1..]
                            .iter()
                            .filter_map(|t| t.ident())
                            .collect::<Vec<_>>()
                            .join("/");
                        f.imports.push((path, span_of(&line)));
                    }
                    self.i += 1;
                }
                crate::kw::ENUM => {
                    if let Some(e) = self.enum_decl(&line) {
                        f.enums.push(e);
                    }
                    self.i += 1;
                }
                crate::kw::GROUP => {
                    if let Some(g) = self.group_decl(&line) {
                        f.groups.push(g);
                    }
                    self.i += 1;
                }
                crate::kw::INPUTS => {
                    self.i += 1;
                    f.inputs.extend(self.var_block(crate::kw::INPUTS));
                }
                crate::kw::OUTPUTS => {
                    self.i += 1;
                    f.outputs.extend(self.out_block());
                }
                crate::kw::DERIVE => {
                    let (head, cite) = self.split_cite(&line);
                    if let Some(mut d) = self.derived(&head) {
                        d.cite = cite;
                        f.items.push(Item::Derived(d));
                    }
                }
                crate::kw::DEFINE => {
                    let (head, cite) = self.split_cite(&line);
                    if let Some(mut d) = self.define(&head) {
                        d.cite = cite;
                        f.items.push(Item::Define(d));
                    }
                    self.i += 1;
                }
                crate::kw::SHAPE => {
                    if let Some(d) = self.shape_decl(&line) {
                        f.shapes.push(d);
                    }
                    self.i += 1;
                }
                crate::kw::SOURCE => {
                    if let Some(d) = self.source(&line) {
                        f.sources.push(d);
                    }
                }
                crate::kw::APPLY => {
                    let (head, cite) = self.split_cite(&line);
                    let at = f.items.len();
                    if let Some(mut a) = self.apply(&head, at) {
                        a.cite = cite;
                        f.applies.push(a);
                    }
                }
                crate::kw::COUNT | crate::kw::SUM => {
                    let kind = if word == crate::kw::SUM { AggKind::Sum } else { AggKind::Count };
                    if let Some(d) = self.agg(&line, kind) {
                        f.items.push(Item::Agg(d));
                    }
                    self.i += 1;
                }
                crate::kw::ELEMENTS => {
                    let name = self.name_at(&line, 1).map(|(n, _)| n);
                    let span = span_of(&line);
                    self.i += 1;
                    let fields = self.var_block(crate::kw::ELEMENTS);
                    match name {
                        Some(name) if f.elements.is_none() => {
                            f.elements = Some(ElementsDecl { name, fields, span })
                        }
                        Some(_) => self.err(
                            Diag::error("E020", tr!("`elements` は一つしか書けません", "There can be only one `elements`"))
                                .at(self.at(span.line))
                                .mark(span, tr!("二本目です", "this is a second one"))
                                .note(tr!(
                                    "規則がたどる並びは一つです。二つ目の並びが要るなら、それは別の規則です。",
                                    "A rule walks one sequence. A second one means a second rule."
                                )),
                        ),
                        None => self.err(
                            Diag::error("E020", tr!("`elements` に名前がありません", "The `elements` line has no name"))
                                .at(self.at(span.line))
                                .mark(span, tr!("`elements 運賃行(fee_rows)` の形です", "the shape is `elements 運賃行(fee_rows)`")),
                        ),
                    }
                }
                crate::kw::FOLD => {
                    if let Some(d) = self.fold(&line) {
                        match f.fold {
                            None => f.fold = Some(d),
                            Some(_) => self.err(
                                Diag::error("E020", tr!("`fold` は一つしか書けません", "There can be only one `fold`"))
                                    .at(self.at(d.span.line))
                                    .mark(d.span.clone(), tr!("二本目です", "this is a second one"))
                                    .note(tr!(
                                        "並びを畳むのは一度だけです。二段に畳むと、要素の並びに対する振る舞いが小さなオートマトンでなくなります（§15.56）。",
                                        "A sequence is folded once. A second fold would take the walk out of the small automaton that makes it checkable (§15.56)."
                                    )),
                            ),
                        }
                    }
                }
                crate::kw::CONSTRAINT => {
                    if let Some(c) = self.constraint(&line) {
                        f.constraints.push(c);
                    }
                    self.i += 1;
                }
                crate::kw::TABLE => {
                    let (head, cite) = self.split_cite(&line);
                    if let Some(mut t) = self.table(&head) {
                        t.cite = cite;
                        f.items.push(Item::Table(t));
                    }
                }
                crate::kw::CLAUSE => {
                    let (head, cite) = self.split_cite(&line);
                    if let Some(mut t) = self.clause(&head) {
                        t.cite = cite;
                        f.items.push(Item::Table(t));
                    }
                }
                crate::kw::RESULT => {
                    if let Some(r) = self.result(&line) {
                        // E016: `result` affects the first output only, so a second one
                        // simply replaced the first — silently, until this said so.
                        match &f.result {
                            Some(first) => {
                                let line_no = first.span.line;
                                self.err(
                                    Diag::error("E016", tr!("`result` は一つしか書けません", "There can be only one `result`"))
                                        .at(self.at(r.span.line))
                                        .mark(r.span.clone(), tr!("{} 行目の `result` と二本目です", "this is a second one, after the `result` on line {}", line_no))
                                        .note(tr!(
                                            "`result` が組み立てるのは最初の出力だけなので、二本目は一本目を置き換えるだけになります。",
                                            "`result` assembles the first output and nothing else, so a second one only replaces the first."
                                        ))
                                        .note(tr!(
                                            "一本だけ残して、ほかの出力はその名前の `define` にしてください。",
                                            "Keep one, and take the other outputs from a `define` of the same name."
                                        )),
                                );
                            }
                            None => f.result = Some(r),
                        }
                    }
                    self.i += 1;
                }
                crate::kw::SEQUENCE => {
                    let name = self.name_at(&line, 1).map(|(n, _)| n);
                    let span = span_of(&line);
                    self.i += 1;
                    self.ctx = tr!("並び", "a named sequence");
                    // A block with no rows is the empty sequence, which is a case of its own
                    // (`empty ->`), so the header alone is a complete declaration.
                    let (cols, outs, rows) = self.grid(false, false).unwrap_or_default();
                    self.ctx.clear();
                    if let Some(o) = outs.first() {
                        self.err(
                            Diag::error("E026", tr!("`sequence` に出力の列は書けません", "A `sequence` has no output column"))
                                .at(self.at(o.span.line))
                                .mark(o.span.clone(), tr!("`->` があります", "there is a `->` here"))
                                .note(tr!(
                                    "これは値の並びであって表ではありません。フィールドは `elements` のフィールドだけです。",
                                    "This is a list of values, not a table: its columns are the fields of `elements` and nothing else."
                                )),
                        );
                    }
                    match name {
                        Some(name) => f.sequences.push(crate::ast::SeqDecl { name, cols, rows, span }),
                        None => self.err(
                            Diag::error("E026", tr!("`sequence` に名前がありません", "The `sequence` line has no name"))
                                .at(self.at(span.line))
                                .mark(span, tr!("`sequence 近い一件(near)` の形です", "the shape is `sequence near(near)`")),
                        ),
                    }
                }
                crate::kw::EXAMPLES => {
                    // Each section is kept with its own header; the span is the `examples`
                    // line, so a finding about the section points at the right one.
                    self.i += 1;
                    if let Some(mut t) = self.example_table() {
                        t.span = span_of(&line);
                        f.examples.push(t);
                    }
                }
                crate::kw::MACHINE => {
                    if let Some(d) = self.machine(&line) {
                        match f.machine {
                            None => f.machine = Some(d),
                            Some(_) => self.err(
                                Diag::error("E050", tr!("`machine` は一つしか書けません", "There can be only one `machine`"))
                                    .at(self.at(d.span.line))
                                    .mark(d.span.clone(), tr!("二つ目です", "this is a second one"))
                                    .note(tr!(
                                        "規則が一歩になるステートマシンは一つです。二つの状態を一緒に持ち越すなら、その組を一つの列挙にしてください。",
                                        "A rule is one step of one machine. To carry two states together, make the pair one enum."
                                    )),
                            ),
                        }
                    }
                }
                crate::kw::SCENARIO => {
                    let named = self.name_at(&line, 1).map(|(n, _)| n);
                    let span = span_of(&line);
                    self.i += 1;
                    self.ctx = match &named {
                        Some(n) => tr!("手順の例 {}", "scenario {}", n.text),
                        None => tr!("手順の例", "a scenario"),
                    };
                    let grid = self.grid(false, false);
                    self.ctx.clear();
                    match (named, grid) {
                        (Some(name), Some((inputs, outputs, rows))) => f.scenarios.push(ScenarioDecl {
                            name,
                            table: Table {
                                name: None,
                                policy: Policy::Unique,
                                inputs,
                                outputs,
                                rows,
                                span: span.clone(),
                                overrides: Vec::new(),
                                clause: false,
                                cite: None,
                                applied: None,
                            },
                            span,
                        }),
                        (None, _) => self.err(
                            Diag::error("E055", tr!("`scenario` に名前がありません", "The `scenario` line has no name"))
                                .at(self.at(span.line))
                                .mark(span, tr!("`scenario 取消のあとの入金(late_pay)` の形です", "the shape is `scenario late_pay(late_pay)`")),
                        ),
                        (Some(name), None) => self.err(
                            Diag::error("E055", tr!("手順の例 {} に行がありません", "Scenario {} has no rows", name.text))
                                .at(self.at(span.line))
                                .mark(span, tr!("見出しの直後に、呼び出し一回を一行で書きます", "write one call per row, right under the heading"))
                                .note(tr!(
                                    "見出しの行は、持ち越す入力のほかの入力、`->`、すべての出力です。一行目の呼び出しは `initial` の状態から始まります。",
                                    "The header names every input but the carried one, then `->`, then every output. The first row's call starts from the `initial` state."
                                )),
                        ),
                    }
                }
                other => {
                    self.err(
                        Diag::error("E005", tr!("`{other}` はこの位置に書けません", "`{other}` cannot appear at this position"))
                            .mark(line[0].span.clone(), "")
                            .note(tr!("書けるのは {} です", "The words allowed at this position are {}", crate::kw::line_heads())),
                    );
                    self.i += 1;
                }
            }
            if self.i == start {
                self.i += 1;
            }
            if DECLARING.contains(&word.as_str()) && (self.diags.len(), declared(&f)) == before {
                let at = self.at(span_of(&line).line);
                // A heading with lines under it (a table, a clause, a machine, an apply) is
                // lost when those lines are: one that did not lex leaves the heading alone.
                let what = if matches!(word.as_str(), crate::kw::TABLE | crate::kw::CLAUSE | crate::kw::MACHINE | crate::kw::APPLY) {
                    tr!("この見出しの下の行を読めず、何も宣言されていません", "nothing under this heading could be read, so nothing is declared")
                } else {
                    tr!("この行は何も宣言していません", "this line declares nothing")
                };
                self.err(unreadable(&word, &line, what, at));
            }
        }
        Some(f)
    }

    /// `名前(ascii)` starting at token `k`. Returns the name and the next index.
    fn name_at(&mut self, ts: &[Token], k: usize) -> Option<(Name, usize)> {
        let t = ts.get(k)?;
        let mut text = t.ident()?.to_string();
        let mut j = k + 1;
        // `呼び出し:出力`, the colon touching both words, names a definition an `apply`
        // brought in (§15.69). With a space on either side the colon starts a type instead.
        let mut end = t.span.col + t.span.len;
        while ts.get(j).is_some_and(|c| c.is(&Kind::Colon) && c.span.col == end)
            && ts.get(j + 1).and_then(|n| n.ident()).is_some()
            && ts[j + 1].span.col == ts[j].span.col + ts[j].span.len
        {
            text.push(':');
            text.push_str(ts[j + 1].ident().unwrap_or(""));
            end = ts[j + 1].span.col + ts[j + 1].span.len;
            j += 2;
        }
        let mut ascii = None;
        if ts.get(j).is_some_and(|t| t.is(&Kind::LParen)) {
            if let Some(a) = ts.get(j + 1).and_then(|t| t.ident()) {
                ascii = Some(a.to_string());
            }
            j += 3; // ( ident )
        }
        Some((Name { text, ascii, span: t.span.clone() }, j))
    }

    fn enum_decl(&mut self, line: &[Token]) -> Option<EnumDecl> {
        let (name, mut k) = self.decl_name(line, crate::kw::ENUM)?;
        if !line.get(k).is_some_and(|t| t.is(&Kind::Eq)) {
            self.err(Diag::error("E006", tr!("型の宣言に `=` がありません", "Missing `=` in the type declaration")).mark(span_of(line), ""));
            return None;
        }
        k += 1;
        let mut values = Vec::new();
        let mut default_marks = Vec::new();
        while k < line.len() {
            if line[k].is(&Kind::Pipe) {
                k += 1;
                continue;
            }
            let (v, nk) = self.name_at(line, k)?;
            values.push(v);
            k = nk;
            let marked = line.get(k).and_then(|t| t.ident()) == Some(crate::kw::DEFAULT);
            default_marks.push(marked);
            if marked {
                k += 1;
            }
        }
        Some(EnumDecl { name, values, default_marks, span: span_of(line) })
    }

    /// `import proto "<file>" <Enum> -> <enum>` and `import jsonschema "<file>" "<pointer>"
    /// -> <enum>` (§15.59, §15.60).
    ///
    /// A line that starts this way and then does not hold together is not read as the other
    /// kind of import: it says what the shape is, once, where the mistake is. What names the
    /// enum inside the file is a word in a `.proto` and a quoted pointer in a schema, so both
    /// spellings are accepted here and the reader for that kind decides what it means.
    fn enum_import(&mut self, line: &[Token], kind: EnumSource) -> Option<EnumImport> {
        let shape = || {
            let (w, sel) = match kind {
                EnumSource::Proto => (crate::kw::PROTO, tr!("<列挙>", "<Enum>")),
                EnumSource::JsonSchema => (crate::kw::JSONSCHEMA, tr!("\"<ポインタ>\"", "\"<pointer>\"")),
            };
            Diag::error(
                "E013",
                tr!("`import {w}` の行の形が違います", "The shape of the `import {w}` line is wrong"),
            )
            .mark(span_of(line), "")
            .note(tr!(
                "形は `import {w} \"<ファイル>\" {sel} -> <この規則の列挙>` です。",
                "The shape is `import {w} \"<file>\" {sel} -> <enum of this rule>`."
            ))
        };
        let Some(Kind::Str(file)) = line.get(2).map(|t| t.kind.clone()) else {
            self.err(shape());
            return None;
        };
        let source = match line.get(3).map(|t| t.kind.clone()) {
            Some(Kind::Str(s)) => s,
            _ => match line.get(3).and_then(|t| t.ident()).map(|s| s.to_string()) {
                Some(s) => s,
                None => {
                    self.err(shape());
                    return None;
                }
            },
        };
        if !line.get(4).is_some_and(|t| t.is(&Kind::Arrow)) {
            self.err(shape());
            return None;
        }
        let Some((target, _)) = self.name_at(line, 5) else {
            self.err(shape());
            return None;
        };
        Some(EnumImport { kind, file, source, target, span: span_of(line) })
    }

    fn group_decl(&mut self, line: &[Token]) -> Option<GroupDecl> {
        let (name, mut k) = self.decl_name(line, crate::kw::GROUP)?;
        if !line.get(k).is_some_and(|t| t.is(&Kind::Eq)) {
            self.err(Diag::error("E006", tr!("グループの宣言に `=` がありません", "Missing `=` in the group declaration")).mark(span_of(line), ""));
            return None;
        }
        k += 1;
        let mut members = Vec::new();
        while k < line.len() {
            if line[k].is(&Kind::Comma) {
                k += 1;
                continue;
            }
            let Some((v, nk)) = self.name_at(line, k) else {
                let at = self.at(line[k].span.line);
                self.err(unreadable(crate::kw::GROUP, &line[k..=k], tr!("グループの値として読めません", "this cannot be read as a value of the group"), at));
                return None;
            };
            members.push(v);
            k = nk;
        }
        Some(GroupDecl { name, members, span: span_of(line) })
    }

    /// An indented run of declarations under `inputs` / `outputs`.
    fn block_lines(&mut self) -> Vec<Vec<Token>> {
        let mut out = Vec::new();
        loop {
            let Some(line) = self.cur() else { break };
            if line.is_empty() {
                break;
            }
            let w = line.first().and_then(|t| t.ident()).unwrap_or("");
            if KEYWORDS.contains(&w) {
                // If the line has the shape of a declaration (`name(alias) :` or `name :`), it is
                // not the start of a section but a declaration named like a keyword. Dropping it
                // silently would only break at code generation, so say so here.
                let decl = line
                    .get(1)
                    .is_some_and(|t| t.is(&Kind::LParen) || t.is(&Kind::Colon));
                if decl {
                    let sp = line[0].span.clone();
                    let at = self.at(sp.line);
                    self.err(
                        Diag::error("E009", tr!("`{w}` はキーワードなので、名前にできません", "`{w}` is a keyword and cannot be used as a name"))
                            .at(at)
                            .mark(sp, "")
                            .note(tr!("行指向の構文なので、キーワードと同じ名前は宣言をセクションの始まりに見せてしまいます。", "The syntax is line-oriented, so a declaration named like a keyword looks like the start of a section."))
                            .note(tr!("別の名前を付けてください。", "Choose a different name.")),
                    );
                    self.i += 1;
                    continue;
                }
                break;
            }
            if line.first().is_some_and(|t| t.is(&Kind::Pipe)) {
                break;
            }
            out.push(line.clone());
            self.i += 1;
        }
        out
    }

    /// The head every declaration line of `inputs`, `outputs` and `elements` starts with: a
    /// name and a type. A line without one of them used to be passed over in silence, and the
    /// rule then had one input fewer than its author wrote (§15.149).
    fn decl_head(&mut self, line: &[Token], section: &str) -> Option<(Name, TypeRef, usize)> {
        let shape = tr!("形は `<名前>(<別名>) : <型>` です。", "The shape is `<name>(<alias>) : <type>`.");
        let Some((name, k)) = self.name_at(line, 0) else {
            self.err(
                Diag::error("E004", tr!("行の先頭に語がありません", "The line does not start with a word"))
                    .at(self.at(span_of(line).line))
                    .mark(span_of(line), tr!("`{section}` の宣言として読めません", "this cannot be read as a declaration under `{section}`"))
                    .note(shape.clone()),
            );
            return None;
        };
        let Some((ty, k)) = self.type_ref(line, k) else {
            self.err(
                Diag::error("E057", tr!("宣言に型がありません", "The declaration has no type"))
                    .at(self.at(name.span.line))
                    .mark(name.span.clone(), tr!("{} の型が書かれていません", "{} is not given a type", name.text))
                    .note(shape)
                    .note(tr!(
                        "型は、範囲と丸めと単位の読み方と、生成する関数の引数を決めます。型の無い行は、宣言として数えられません。",
                        "The type decides how the range, the rounding and the unit are read, and what the generated function takes. A line with none is not a declaration."
                    )),
            );
            return None;
        };
        Some((name, ty, k))
    }

    fn var_block(&mut self, section: &str) -> Vec<VarDecl> {
        let mut v = Vec::new();
        for line in self.block_lines() {
            let Some((name, ty, k)) = self.decl_head(&line, section) else { continue };
            // `from …` runs to the end of the line, so everything before it is the tail the
            // range and the markers are read from (§15.125).
            let cut = line.iter().position(|t| t.ident() == Some(crate::kw::FROM)).unwrap_or(line.len());
            let head = &line[..cut];
            let from = (cut < line.len()).then(|| self.projection(&line[cut..])).flatten();
            let (range, contract_only) = self.tail_range(head, k);
            self.tail_junk(head, k);
            v.push(VarDecl { name, ty, range, contract_only, from, span: span_of(&line) });
        }
        v
    }

    /// `shape order(order) = jsonschema "order.json" "#/$defs/Order"` (§15.125).
    fn shape_decl(&mut self, line: &[Token]) -> Option<ShapeDecl> {
        let shape = || {
            Diag::error("E013", tr!("`shape` の行の形が違います", "The shape of the `shape` line is wrong"))
                .mark(span_of(line), "")
                .note(tr!(
                    "形は `shape <名前>(<別名>) = jsonschema \"<ファイル>\" \"<ポインタ>\"` か `shape <名前>(<別名>) = proto \"<ファイル>\" <メッセージ>` です。",
                    "The shape is `shape <name>(<alias>) = jsonschema \"<file>\" \"<pointer>\"` or `shape <name>(<alias>) = proto \"<file>\" <Message>`."
                ))
        };
        let Some((name, k)) = self.name_at(line, 1) else {
            self.err(shape());
            return None;
        };
        if !line.get(k).is_some_and(|t| t.is(&Kind::Eq)) {
            self.err(shape());
            return None;
        }
        let source = match line.get(k + 1).and_then(|t| t.ident()) {
            Some(crate::kw::PROTO) => EnumSource::Proto,
            Some(crate::kw::JSONSCHEMA) => EnumSource::JsonSchema,
            _ => {
                self.err(shape());
                return None;
            }
        };
        let Some(Kind::Str(file)) = line.get(k + 2).map(|t| t.kind.clone()) else {
            self.err(shape());
            return None;
        };
        let mut end = k + 4;
        let at = match line.get(k + 3).map(|t| t.kind.clone()) {
            Some(Kind::Str(s)) => s,
            _ => match line.get(k + 3).and_then(|t| t.ident()).map(|s| s.to_string()) {
                Some(mut s) => {
                    // A message named with its package, `shop.v1.Order`, is one name: the
                    // form the grammar shows, which read only as far as `shop` before.
                    while line.get(end).is_some_and(|t| t.is(&Kind::Dot)) {
                        let Some(w) = line.get(end + 1).and_then(|t| t.ident()) else { break };
                        s.push('.');
                        s.push_str(w);
                        end += 2;
                    }
                    s
                }
                None => {
                    self.err(shape());
                    return None;
                }
            },
        };
        self.tail_junk(line, end);
        Some(ShapeDecl { name, source, file, at, span: span_of(line) })
    }

    /// The tail of an input line, from `from` to the end (§15.125).
    fn projection(&mut self, ts: &[Token]) -> Option<Projection> {
        let shape = || {
            Diag::error("E013", tr!("`from` の形が違います", "The shape of `from` is wrong"))
                .mark(span_of(ts), "")
                .note(tr!(
                    "形は `from <shape の名前>.<フィールド>…`、`from any|all <shape の名前>.<並び> where <フィールド> = <値>`、`from count <shape の名前>.<並び> [where <フィールド> = <値>]` です。",
                    "The shapes are `from <shape>.<field>…`, `from any|all <shape>.<collection> where <field> = <value>`, and `from count <shape>.<collection> [where <field> = <value>]`."
                ))
        };
        let (word, mut k) = match ts.get(1).and_then(|t| t.ident()) {
            Some(w @ (crate::kw::ANY | crate::kw::ALL | crate::kw::COUNT)) => (Some(w.to_string()), 2),
            _ => (None, 1),
        };
        // The path: a name, then `.name` as far as it goes.
        let Some(root) = ts.get(k).and_then(|t| t.ident()).map(|w| Name {
            text: w.to_string(),
            ascii: None,
            span: ts[k].span.clone(),
        }) else {
            self.err(shape());
            return None;
        };
        k += 1;
        let mut path = Vec::new();
        while ts.get(k).is_some_and(|t| t.is(&Kind::Dot)) {
            let Some(n) = ts.get(k + 1).and_then(|t| t.ident()) else {
                self.err(shape());
                return None;
            };
            path.push(Name { text: n.to_string(), ascii: None, span: ts[k + 1].span.clone() });
            k += 2;
        }
        if path.is_empty() {
            self.err(shape());
            return None;
        }
        // `where <field> [=] <test>`
        let test = if ts.get(k).and_then(|t| t.ident()) == Some(crate::kw::WHERE) {
            let Some(field) = ts.get(k + 1).and_then(|t| t.ident()).map(|w| Name {
                text: w.to_string(),
                ascii: None,
                span: ts[k + 1].span.clone(),
            }) else {
                self.err(shape());
                return None;
            };
            let mut j = k + 2;
            if ts.get(j).is_some_and(|t| t.is(&Kind::Eq)) {
                j += 1;
            }
            // `where chilled` with nothing to compare it to used to reach `cell` with no tokens
            // at all, and index past the end of them (§15.156).
            if j >= ts.len() {
                self.err(shape().mark(ts[j - 1].span.clone(), tr!("`where` の後に比べる値がありません", "`where` names a field and nothing to compare it to")));
                return None;
            }
            let c = self.cell(&ts[j..])?;
            Some((field, c))
        } else {
            self.tail_junk(ts, k);
            None
        };
        let kind = match (word.as_deref(), test) {
            (None, None) => ProjKind::Field,
            (Some(crate::kw::ANY), Some((n, c))) => ProjKind::Any(n, c),
            (Some(crate::kw::ALL), Some((n, c))) => ProjKind::All(n, c),
            (Some(crate::kw::COUNT), t) => ProjKind::Count(t),
            _ => {
                self.err(shape());
                return None;
            }
        };
        Some(Projection { root, path, kind, span: span_of(ts) })
    }

    fn out_block(&mut self) -> Vec<OutDecl> {
        let mut v = Vec::new();
        for line in self.block_lines() {
            let Some((name, ty, k)) = self.decl_head(&line, crate::kw::OUTPUTS) else { continue };
            let rounding = self.tail_rounding(&line, k);
            self.tail_junk(&line, k);
            v.push(OutDecl { name, ty, rounding, span: span_of(&line) });
        }
        v
    }

    /// `: money[円, incl_tax]` / `: mass[g]` / `: rate[step 0.1%]` / `: 都道府県`
    fn type_ref(&mut self, ts: &[Token], mut k: usize) -> Option<(TypeRef, usize)> {
        if ts.get(k).is_some_and(|t| t.is(&Kind::Colon)) {
            k += 1;
        }
        let base = ts.get(k)?.ident()?.to_string();
        let start = ts[k].span.clone();
        k += 1;
        let mut args = Vec::new();
        if ts.get(k).is_some_and(|t| t.is(&Kind::LBracket)) {
            k += 1;
            while k < ts.len() && !ts[k].is(&Kind::RBracket) {
                match &ts[k].kind {
                    Kind::Comma => k += 1,
                    Kind::Ident(w) => {
                        let w = w.clone();
                        if let Some(Kind::Num(n)) = ts.get(k + 1).map(|t| t.kind.clone()) {
                            args.push(TypeArg::Scaled(w, n));
                            k += 2;
                        } else {
                            args.push(TypeArg::Word(w));
                            k += 1;
                        }
                    }
                    _ => k += 1,
                }
            }
            k += 1; // ]
        }
        let optional = ts.get(k).is_some_and(|t| t.is(&Kind::Question));
        if optional {
            k += 1;
        }
        // Extend the span to the `]`: §11's E104 underlines the whole type.
        let end = ts.get(k.saturating_sub(1)).map(|t| t.span.col + t.span.len).unwrap_or(start.col + start.len);
        let span = Span::new(start.line, start.col, end.saturating_sub(start.col).max(start.len));
        Some((TypeRef { base, args, optional, span }, k))
    }

    /// Everything a declaration's tail may hold: `range <bounds>`, `contract_only`, and
    /// `round <mode>(<grid>)`. The readers above walk the line looking for the word they
    /// want and step over everything else, so a token belonging to none of them used to
    /// vanish — a tax flag written after the range (`range >=0円 <=10000円 incl_tax`), or
    /// what is left of a bound whose unit did not lex as one. The range is the universe the
    /// completeness proof quantifies over and the entry guard of the generated code, so a
    /// bound lost this way was answered "complete" with one side missing. E047 stops at the
    /// first such token. A citation ends the tail, so `@…` is where the scan gives up.
    fn tail_junk(&mut self, ts: &[Token], mut k: usize) {
        while k < ts.len() {
            if ts[k].is(&Kind::At) {
                return;
            }
            match ts[k].ident() {
                Some(crate::kw::RANGE) => {
                    k += 1;
                    while k + 1 < ts.len()
                        && matches!(ts[k].kind, Kind::Le | Kind::Ge | Kind::Lt | Kind::Gt)
                        && lit_of(&ts[k + 1]).is_some()
                    {
                        k += 2;
                    }
                }
                Some(crate::kw::CONTRACT_ONLY) => k += 1,
                // `round up(10円)` is five tokens. A shorter one is E104's business, not
                // this check's, so the word alone is stepped over.
                Some(crate::kw::ROUND) => {
                    let full = ts.get(k + 2).is_some_and(|t| t.is(&Kind::LParen))
                        && matches!(ts.get(k + 3).map(|t| &t.kind), Some(Kind::Num(_)))
                        && ts.get(k + 4).is_some_and(|t| t.is(&Kind::RParen));
                    k += if full { 5 } else { 1 };
                }
                _ => {
                    let sp = ts[k].span.clone();
                    self.err(
                        Diag::error("E047", tr!("宣言の後ろに余分な語があります", "Extra token after the declaration"))
                            .at(self.at(sp.line))
                            .mark(sp, tr!("この語は宣言の一部として読まれません", "this is not read as part of the declaration"))
                            .note(tr!(
                                "宣言の行に置けるのは `{}`、`{}`、`{}` だけです。ほかの語はいままで黙って捨てられていました。",
                                "A declaration line holds `{}`, `{}` and `{}`, and nothing else. Anything else used to be dropped in silence.",
                                crate::kw::RANGE, crate::kw::ROUND, crate::kw::CONTRACT_ONLY
                            ))
                            .note(tr!(
                                "範囲は `{} >=<値> <=<値>` の形で、比較の記号と値の対だけが読まれます。単位の綴りが違っていると対として読めなくなり、余った分がここに出ます。",
                                "A range is `{} >=<value> <=<value>`: only pairs of a comparison and a value are read. A misspelled unit stops a pair from being read as one, and what is left over lands here.",
                                crate::kw::RANGE
                            )),
                    );
                    return;
                }
            }
        }
    }

    /// `range >=1g <=40kg` and the `contract_only` marker (§11 W111).
    fn tail_range(&mut self, ts: &[Token], mut k: usize) -> (Option<Range>, bool) {
        let mut range = None;
        let mut contract_only = false;
        while k < ts.len() {
            match ts[k].ident() {
                Some(crate::kw::RANGE) => {
                    let start = k;
                    k += 1;
                    let mut bounds = Vec::new();
                    while k + 1 < ts.len() {
                        let op = match ts[k].kind {
                            Kind::Le => CmpOp::Le,
                            Kind::Ge => CmpOp::Ge,
                            Kind::Lt => CmpOp::Lt,
                            Kind::Gt => CmpOp::Gt,
                            _ => break,
                        };
                        let Some(l) = lit_of(&ts[k + 1]) else { break };
                        bounds.push((op, l));
                        k += 2;
                    }
                    range = Some(Range { bounds, span: span_of(&ts[start..k.min(ts.len())]) });
                }
                Some(crate::kw::CONTRACT_ONLY) => {
                    contract_only = true;
                    k += 1;
                }
                _ => k += 1,
            }
        }
        (range, contract_only)
    }

    /// `round up(10円)`
    fn tail_rounding(&mut self, ts: &[Token], mut k: usize) -> Option<Rounding> {
        while k < ts.len() {
            if ts[k].ident() == Some(crate::kw::ROUND) {
                let mode = ts.get(k + 1)?.ident()?.to_string();
                let grid = match ts.get(k + 3)?.kind.clone() {
                    Kind::Num(n) => n,
                    _ => return None,
                };
                return Some(Rounding { mode, grid, span: span_of(&ts[k..]) });
            }
            k += 1;
        }
        None
    }

    fn derived(&mut self, line: &[Token]) -> Option<DerivedDecl> {
        // The line is consumed before anything can go wrong. This function used to move on
        // only when the line read, so a malformed `derive` was read again for ever (§15.156).
        self.i += 1;
        // §11's E112 example puts `range` on the next line; accept both. The next line belongs
        // to this one whether or not this one reads, so it is taken here too.
        let has_range = line.iter().any(|t| t.ident() == Some(crate::kw::RANGE));
        let cont = match self.cur() {
            Some(l) if !has_range && l.first().and_then(|t| t.ident()) == Some(crate::kw::RANGE) => Some(l.clone()),
            _ => None,
        };
        if cont.is_some() {
            self.i += 1;
        }
        let (name, k) = self.decl_name(line, crate::kw::DERIVE)?;
        let (ty, k) = self.decl_type(line, k, &name, crate::kw::DERIVE)?;
        let eq = self.decl_eq(line, k, crate::kw::DERIVE)?;
        // The expression runs to `range` on the same line, if present.
        let stop = line[eq + 1..]
            .iter()
            .position(|t| t.ident() == Some(crate::kw::RANGE))
            .map(|p| eq + 1 + p)
            .unwrap_or(line.len());
        let expr = self.expr_of(line, eq, &line[eq + 1..stop])?;
        let (mut range, _) = self.tail_range(line, k.max(eq));
        self.tail_junk(line, stop);
        if let Some(c) = cont {
            range = self.tail_range(&c, 0).0;
        }
        Some(DerivedDecl { cite: None, name, ty, expr, range, span: span_of(line) })
    }

    /// The name that follows the word starting a declaration line (E058 when there is none).
    fn decl_name(&mut self, line: &[Token], word: &str) -> Option<(Name, usize)> {
        let got = self.name_at(line, 1);
        if got.is_none() {
            let at = self.at(span_of(line).line);
            self.err(unreadable(word, line, tr!("`{word}` の後に名前がありません", "there is no name after `{word}`"), at));
        }
        got
    }

    /// The type of a `define` or a `derive` — E057, the code a line under `inputs` gets for
    /// the same slip.
    fn decl_type(&mut self, line: &[Token], k: usize, name: &Name, word: &str) -> Option<(TypeRef, usize)> {
        let got = self.type_ref(line, k);
        // A token after the name that is neither the `:` of a type nor the `=` is not a type
        // left out: it is something the name did not take (`derive 合算) : …`).
        if got.is_none() && line.get(k).is_some_and(|t| !t.is(&Kind::Colon) && !t.is(&Kind::Eq)) {
            let at = self.at(line[k].span.line);
            self.err(unreadable(word, &line[k..=k], tr!("名前の後のこの語は読まれません", "this is not read after the name"), at));
        } else if got.is_none() {
            self.err(
                Diag::error("E057", tr!("宣言に型がありません", "The declaration has no type"))
                    .at(self.at(name.span.line))
                    .mark(name.span.clone(), tr!("{} の型が書かれていません", "{} is not given a type", name.text))
                    .note(tr!("形は {} です。", "The shape is {}.", line_shape(word))),
            );
        }
        got
    }

    /// The `=` right after the name (and the type, where there is one). E006 when there is
    /// none; E058 when something stands before it, which used to be skipped in silence — a
    /// `range` written before the `=` was never read.
    fn decl_eq(&mut self, line: &[Token], k: usize, word: &str) -> Option<usize> {
        let at = self.at(span_of(line).line);
        let Some(eq) = line.iter().position(|t| t.is(&Kind::Eq)) else {
            self.err(
                Diag::error("E006", tr!("宣言に `=` がありません", "The declaration has no `=`"))
                    .at(at)
                    .mark(span_of(line), tr!("名前と中身を `=` で分けます", "`=` separates the name from the body"))
                    .note(tr!("形は {} です。", "The shape is {}.", line_shape(word))),
            );
            return None;
        };
        if eq > k {
            self.err(unreadable(word, &line[k..eq], tr!("`=` の前にあるこの語は読まれません", "this is not read: nothing stands between the declaration and its `=`"), at));
            return None;
        }
        Some(eq)
    }

    /// ```text
    /// count 一致数(hits) over 納入先 where 判定 = 一致  range >=0 <=100
    /// ```
    ///
    /// `= <値>` is what a bool column does not need: without it the test is `= true`
    /// (§15.58). The range is read by `tail_range` like any other, and required later —
    /// here a missing one is not a syntax error, so that the reason can be explained.
    fn agg(&mut self, line: &[Token], kind: AggKind) -> Option<AggDecl> {
        let span = span_of(line);
        let word = if kind == AggKind::Sum { crate::kw::SUM } else { crate::kw::COUNT };
        let shape = |p: &mut Self, what: String| -> Option<AggDecl> {
            p.err(
                Diag::error("E028", tr!("`{}` の書き方が正しくありません", "The `{}` is not written correctly", word))
                    .at(p.at(span.line))
                    .mark(span.clone(), what)
                    .note(if kind == AggKind::Sum {
                        tr!(
                            "形は `sum <名前>(<別名>) over <並びの名前> of <列>  range >=… <=…` です。",
                            "The shape is `sum <name>(<alias>) over <sequence> of <column>  range >=… <=…`."
                        )
                    } else {
                        tr!(
                            "形は `count <名前>(<別名>) over <並びの名前> where <列> = <値>` です。`= <値>` は、真偽の列なら書かなくて構いません。",
                            "The shape is `count <name>(<alias>) over <sequence> where <column> = <value>`. A bool column needs no `= <value>`."
                        )
                    }),
            );
            None
        };
        let Some((name, k)) = self.name_at(line, 1) else {
            return shape(self, tr!("まとめた結果の名前がありません", "the summary has no name"));
        };
        if line.get(k).and_then(|t| t.ident()) != Some(crate::kw::OVER) {
            return shape(self, tr!("`over` がありません", "`over` is missing"));
        }
        let Some(over) = line.get(k + 1).and_then(|t| t.ident()).map(str::to_string) else {
            return shape(self, tr!("たどる並びの名前がありません", "the sequence has no name"));
        };
        let joiner = if kind == AggKind::Sum { crate::kw::OF } else { crate::kw::WHERE };
        if line.get(k + 2).and_then(|t| t.ident()) != Some(joiner) {
            return shape(self, tr!("`{}` がありません", "`{}` is missing", joiner));
        }
        let Some((column, mut j)) = self.name_at(line, k + 3) else {
            return shape(self, tr!("読む列がありません", "no column is named"));
        };
        let mut value = None;
        if kind == AggKind::Count && line.get(j).is_some_and(|t| t.is(&Kind::Eq)) {
            let Some((v, after)) = self.name_at(line, j + 1) else {
                return shape(self, tr!("`=` の右に値がありません", "`=` has no value on its right"));
            };
            value = Some(v);
            j = after;
        }
        let (range, _) = self.tail_range(line, j);
        self.tail_junk(line, j);
        Some(AggDecl { kind, name, over, column, value, range, span })
    }

    fn define(&mut self, line: &[Token]) -> Option<DefineDecl> {
        let (name, k) = self.decl_name(line, crate::kw::DEFINE)?;
        let (ty, k) = self.decl_type(line, k, &name, crate::kw::DEFINE)?;
        let eq = self.decl_eq(line, k, crate::kw::DEFINE)?;
        let expr = self.expr_of(line, eq, &line[eq + 1..])?;
        Some(DefineDecl { cite: None, name, ty, expr, span: span_of(line) })
    }

    /// ```text
    /// fold 採用 over 運賃行
    ///   スキップ  -> next
    ///   打ち切り  -> stop
    ///   確定      -> take_unique 運賃
    ///   持ち越し  -> keep_max 運賃 by 閾値
    ///   empty     -> 0円
    ///   exhausted -> held
    /// ```
    ///
    /// One arm per value of the verdict's enum, and the two answers that are not about any
    /// element: `empty` for a sequence with nothing in it, `exhausted` for a walk that
    /// reached the end. Both are required, which is the point (§15.56).
    fn fold(&mut self, line: &[Token]) -> Option<FoldDecl> {
        let span = span_of(line);
        // The heading is consumed and so is the block under it, whatever else goes wrong: a
        // parser that returns without moving reads the same line for ever, and the arms would
        // otherwise be read as top-level declarations and reported twice.
        self.i += 1;
        let block = self.block_lines();
        let head = |p: &mut Self, what: String| -> Option<FoldDecl> {
            p.err(
                Diag::error("E021", tr!("`fold` の見出しの形が違います", "A `fold` heading is not shaped like this"))
                    .at(p.at(span.line))
                    .mark(span.clone(), what)
                    .note(tr!(
                        "形は `fold <判定の列> over <並びの名前>` です。左は表が出す列、右は `elements` で宣言した並びの名前です。",
                        "The shape is `fold <verdict column> over <sequence>`: a column some table produces, and the name declared by `elements`."
                    )),
            );
            None
        };
        let verdict = match line.get(1).and_then(|t| t.ident()) {
            Some(v) => v.to_string(),
            None => return head(self, tr!("畳む列がありません", "there is no column to fold")),
        };
        if line.get(2).and_then(|t| t.ident()) != Some(crate::kw::OVER) {
            return head(self, tr!("`over` がありません", "`over` is missing"));
        }
        let over = match line.get(3).and_then(|t| t.ident()) {
            Some(v) => v.to_string(),
            None => return head(self, tr!("たどる並びの名前がありません", "the sequence has no name")),
        };
        let mut arms = Vec::new();
        let (mut empty, mut exhausted) = (None, None);
        for l in block {
            let sp = span_of(&l);
            let Some((name, k)) = self.name_at(&l, 0) else { continue };
            // `->` is written as the arrow the cells use.
            let Some(a) = l.iter().position(|t| t.is(&Kind::Arrow)) else {
                self.err(
                    Diag::error("E021", tr!("`fold` の中に `->` の無い行があります", "A `fold` arm has no `->`"))
                        .at(self.at(sp.line))
                        .mark(sp, tr!("`{} -> next` のように書きます", "write it like `{} -> next`", name.text)),
                );
                continue;
            };
            let _ = k;
            let rest = &l[a + 1..];
            let word = rest.first().and_then(|t| t.ident()).unwrap_or("");
            match name.text.as_str() {
                crate::kw::EMPTY => empty = self.expr_after(&l[a], rest),
                crate::kw::EXHAUSTED => exhausted = self.expr_after(&l[a], rest),
                _ => {
                    let arm = match word {
                        crate::kw::NEXT => Some(Arm::Next),
                        crate::kw::STOP => {
                            if rest.get(1).and_then(|t| t.ident()) == Some(crate::kw::WITH) {
                                self.expr_after(&rest[1], &rest[2..]).map(|e| Arm::Stop(Some(e)))
                            } else {
                                Some(Arm::Stop(None))
                            }
                        }
                        crate::kw::TAKE_UNIQUE | crate::kw::TAKE_FIRST => self
                            .expr_after(&rest[0], &rest[1..])
                            .map(|e| Arm::Take { expr: e, unique: word == crate::kw::TAKE_UNIQUE }),
                        crate::kw::KEEP_MAX => {
                            let by = rest.iter().position(|t| t.ident() == Some(crate::kw::BY));
                            match by {
                                Some(b) => match (self.expr_after(&rest[0], &rest[1..b]), self.expr_after(&rest[b], &rest[b + 1..])) {
                                    (Some(expr), Some(key)) => Some(Arm::KeepMax { expr, key }),
                                    _ => None,
                                },
                                None => {
                                    self.err(
                                        Diag::error("E021", tr!("`keep_max` に `by` がありません", "A `keep_max` has no `by`"))
                                            .at(self.at(sp.line))
                                            .mark(sp.clone(), tr!("`keep_max <値> by <鍵>` の形です", "the shape is `keep_max <value> by <key>`"))
                                            .note(tr!(
                                                "どちらを残すかを決める鍵が要ります。鍵が無ければ「最後が勝つ」で、それは書いた人の意図とは限りません。",
                                                "The key is what decides which one is kept. Without it the last one wins, which is rarely what anyone meant."
                                            )),
                                    );
                                    None
                                }
                            }
                        }
                        other => {
                            self.err(
                                Diag::error("E021", tr!("`{other}` という行き先はありません", "There is no arm called `{other}`"))
                                    .at(self.at(sp.line))
                                    .mark(sp.clone(), tr!("行き先に書けるのは次のどれかです", "an arm is one of these"))
                                    .note(tr!(
                                        "`next`（次へ）、`stop`（終わり）、`stop with <値>`、`take_unique <値>`、`take_first <値>`、`keep_max <値> by <鍵>`。",
                                        "`next`, `stop`, `stop with <value>`, `take_unique <value>`, `take_first <value>`, `keep_max <value> by <key>`."
                                    )),
                            );
                            None
                        }
                    };
                    if let Some(arm) = arm {
                        arms.push((name, arm, sp));
                    }
                }
            }
        }
        Some(FoldDecl { verdict, over, arms, empty, exhausted, span })
    }

    /// `machine 注文(order) over 遷移` and the lines under it (§15.148).
    ///
    /// ```text
    /// machine 注文(order) over 遷移
    ///   carry   状態 -> 次の状態
    ///   initial 受付
    ///   final   配達済, 取消
    ///   never   出荷済 after 取消
    ///   once    返金額 >0円
    /// ```
    ///
    /// Only the shape is read here. Whether the names exist and fit is the type check's
    /// question, which has the declarations to answer it (E051–E053).
    fn machine(&mut self, line: &[Token]) -> Option<MachineDecl> {
        let span = span_of(line);
        // The heading and the block under it are consumed whatever else goes wrong, so the
        // lines under a broken heading are not read again as top-level declarations.
        self.i += 1;
        let block = self.block_lines();
        let shape_note = || {
            tr!(
                "形は `machine <名前>(<ascii>) over <表>` です。表は、持ち越す出力を決める表です。",
                "The shape is `machine <name>(<ascii>) over <table>`: the table is the one that decides the carried output."
            )
        };
        let Some((name, k)) = self.name_at(line, 1) else {
            self.err(
                Diag::error("E050", tr!("`machine` に名前がありません", "The `machine` line has no name"))
                    .at(self.at(span.line))
                    .mark(span.clone(), "")
                    .note(shape_note()),
            );
            return None;
        };
        let over = if line.get(k).and_then(|t| t.ident()) == Some(crate::kw::OVER) {
            line.get(k + 1).and_then(|t| t.ident()).map(|w| Name { text: w.to_string(), ascii: None, span: line[k + 1].span.clone() })
        } else {
            None
        };
        if over.is_none() || line.len() > k + 2 {
            self.err(
                Diag::error("E050", tr!("`machine` の見出しの形が違います", "A `machine` heading is not shaped like this"))
                    .at(self.at(span.line))
                    .mark(span.clone(), if over.is_none() { tr!("`over <表>` がありません", "`over <table>` is missing") } else { tr!("余分な語があります", "there are words left over") })
                    .note(shape_note()),
            );
        }
        let mut m = MachineDecl {
            name,
            over,
            carry: None,
            initial: None,
            finals: Vec::new(),
            final_span: None,
            nevers: Vec::new(),
            onces: Vec::new(),
            held: Vec::new(),
            held_span: None,
            span: span.clone(),
        };
        let words = |ts: &[Token]| -> Vec<Name> {
            ts.iter()
                .filter_map(|t| t.ident().map(|w| Name { text: w.to_string(), ascii: None, span: t.span.clone() }))
                .collect()
        };
        for l in block {
            let sp = span_of(&l);
            let word = l.first().and_then(|t| t.ident()).unwrap_or("").to_string();
            let bad = |p: &mut Self, shape: String| {
                p.err(
                    Diag::error("E050", tr!("`machine` の中の `{}` の行の形が違います", "A `{}` line under `machine` is not shaped like this", word))
                        .at(p.at(sp.line))
                        .mark(sp.clone(), "")
                        .note(tr!("形は `{}` です。", "The shape is `{}`.", shape)),
                );
            };
            let twice = |p: &mut Self| {
                p.err(
                    Diag::error("E050", tr!("`machine` の中に `{}` が二行あります", "There are two `{}` lines under `machine`", word))
                        .at(p.at(sp.line))
                        .mark(sp.clone(), tr!("二行目です", "this is the second"))
                        .note(tr!("この行は一つの `machine` に一行だけ書きます。", "A `machine` has one line of this kind.")),
                );
            };
            match word.as_str() {
                crate::kw::CARRY => {
                    let arrow = l.iter().position(|t| t.is(&Kind::Arrow));
                    match (arrow, l.get(1).and_then(|t| t.ident()), l.get(3).and_then(|t| t.ident())) {
                        (Some(2), Some(i), Some(o)) if l.len() == 4 => {
                            if m.carry.is_some() {
                                twice(self);
                            } else {
                                let n = |t: &Token, w: &str| Name { text: w.to_string(), ascii: None, span: t.span.clone() };
                                m.carry = Some((n(&l[1], i), n(&l[3], o), sp.clone()));
                            }
                        }
                        _ => bad(self, format!("{} <{}> -> <{}>", crate::kw::CARRY, tr!("入力", "input"), tr!("出力", "output"))),
                    }
                }
                crate::kw::INITIAL => match (l.len(), l.get(1).and_then(|t| t.ident())) {
                    (2, Some(v)) => {
                        if m.initial.is_some() {
                            twice(self);
                        } else {
                            m.initial = Some((Name { text: v.to_string(), ascii: None, span: l[1].span.clone() }, sp.clone()));
                        }
                    }
                    _ => bad(self, format!("{} <{}>", crate::kw::INITIAL, tr!("状態", "state"))),
                },
                crate::kw::FINAL => {
                    let vs = words(&l[1..]);
                    if vs.is_empty() || l[1..].iter().any(|t| t.ident().is_none() && !t.is(&Kind::Comma)) {
                        bad(self, format!("{} <{}>, …", crate::kw::FINAL, tr!("状態", "state")));
                    } else if m.final_span.is_some() {
                        twice(self);
                    } else {
                        m.finals = vs;
                        m.final_span = Some(sp.clone());
                    }
                }
                crate::kw::HELD => {
                    let vs = words(&l[1..]);
                    if vs.is_empty() || l[1..].iter().any(|t| t.ident().is_none() && !t.is(&Kind::Comma)) {
                        bad(self, format!("{} <{}>, …", crate::kw::HELD, tr!("入力", "input")));
                    } else if m.held_span.is_some() {
                        twice(self);
                    } else {
                        m.held = vs;
                        m.held_span = Some(sp.clone());
                    }
                }
                crate::kw::NEVER => {
                    let after = l.iter().position(|t| t.ident() == Some(crate::kw::AFTER));
                    let ok_list = |ts: &[Token]| !ts.is_empty() && ts.iter().all(|t| t.ident().is_some() || t.is(&Kind::Comma)) && ts.iter().any(|t| t.ident().is_some());
                    match after {
                        Some(a) if ok_list(&l[1..a]) && ok_list(&l[a + 1..]) => {
                            m.nevers.push(NeverDecl { states: words(&l[1..a]), after: words(&l[a + 1..]), span: sp.clone() })
                        }
                        _ => bad(
                            self,
                            format!(
                                "{} <{}>, … {} <{}>, …",
                                crate::kw::NEVER,
                                tr!("状態", "state"),
                                crate::kw::AFTER,
                                tr!("状態", "state")
                            ),
                        ),
                    }
                }
                crate::kw::ONCE => {
                    let out = l.get(1).and_then(|t| t.ident());
                    match out {
                        Some(o) if l.len() > 2 => {
                            let cell_ts = &l[2..];
                            if let Some(cell) = self.cell(cell_ts) {
                                m.onces.push(OnceDecl {
                                    output: Name { text: o.to_string(), ascii: None, span: l[1].span.clone() },
                                    cell,
                                    cell_span: span_of(cell_ts),
                                    span: sp.clone(),
                                });
                            }
                        }
                        _ => bad(self, format!("{} <{}> <{}>", crate::kw::ONCE, tr!("出力", "output"), tr!("セル", "cell"))),
                    }
                }
                other => {
                    self.err(
                        Diag::error("E050", tr!("`{other}` は `machine` の中に書けません", "`{other}` cannot appear under `machine`"))
                            .at(self.at(sp.line))
                            .mark(sp.clone(), "")
                            .note(tr!(
                                "`machine` の中に書けるのは `carry`・`held`・`initial`・`final`・`never`・`once` の行です。",
                                "The lines under `machine` are `carry`, `held`, `initial`, `final`, `never` and `once`."
                            )),
                    );
                }
            }
        }
        if m.carry.is_none() || m.initial.is_none() {
            let missing: Vec<&str> = [(m.carry.is_none(), crate::kw::CARRY), (m.initial.is_none(), crate::kw::INITIAL)]
                .iter()
                .filter(|(b, _)| *b)
                .map(|(_, w)| *w)
                .collect();
            self.err(
                Diag::error("E050", tr!("`machine` に `{}` の行がありません", "The `machine` has no `{}` line", missing.join("` / `")))
                    .at(self.at(span.line))
                    .mark(span.clone(), "")
                    .note(tr!(
                        "`carry` は次の呼び出しの入力になる出力を、`initial` は案件が始まる状態を言います。どちらも無いと、呼び出しの並びが決まりません。",
                        "`carry` names the output that is the next call's input, and `initial` the state a case starts in. Without both there is no sequence of calls to speak of."
                    )),
            );
        }
        Some(m)
    }

    /// `constraint <input> <= <input>` — one relation per line, and several lines all hold.
    ///
    /// Only the four comparisons the lexer already knows, and only between two names: what a
    /// constraint is for is saying which combinations exist, and `A = B` is the two lines
    /// `A <= B` and `A >= B` when it is ever wanted. One shape means one thing to read
    /// (§15.55).
    fn constraint(&mut self, line: &[Token]) -> Option<Constraint> {
        let span = span_of(line);
        let bad = |p: &mut Self, what: String| -> Option<Constraint> {
            p.err(
                Diag::error("E017", tr!("`constraint` の形が違います", "A `constraint` is not shaped like this"))
                    .at(p.at(span.line))
                    .mark(span.clone(), what)
                    .note(tr!(
                        "形は `constraint <入力> <= <入力>` です。比較は `<=` `<` `>=` `>` の四つで、両側とも入力の名前です。",
                        "The shape is `constraint <input> <= <input>`: one of `<=`, `<`, `>=`, `>`, with the name of an input on each side."
                    )),
            );
            None
        };
        let Some(p) = line.iter().position(|t| matches!(t.kind, Kind::Le | Kind::Ge | Kind::Lt | Kind::Gt)) else {
            return bad(self, tr!("比較がありません", "there is no comparison here"));
        };
        let op = match line[p].kind {
            Kind::Le => CmpOp::Le,
            Kind::Ge => CmpOp::Ge,
            Kind::Lt => CmpOp::Lt,
            _ => CmpOp::Gt,
        };
        let name_of = |ts: &[Token]| -> Option<String> {
            match ts {
                [t] => t.ident().map(|s| s.to_string()),
                _ => None,
            }
        };
        let (Some(left), Some(right)) = (name_of(&line[1..p]), name_of(&line[p + 1..])) else {
            return bad(self, tr!("両側とも入力の名前を一つずつ書きます", "one input's name is expected on each side"));
        };
        Some(Constraint { left, op, right, span })
    }

    fn result(&mut self, line: &[Token]) -> Option<ResultDecl> {
        let Some(name) = line.get(1).and_then(|t| t.ident()).map(str::to_string) else {
            let at = self.at(span_of(line).line);
            self.err(unreadable(crate::kw::RESULT, line, tr!("`result` の後に出力の名前がありません", "there is no output named after `result`"), at));
            return None;
        };
        let eq = self.decl_eq(line, 2, crate::kw::RESULT)?;
        let expr = self.expr_of(line, eq, &line[eq + 1..])?;
        Some(ResultDecl { name, expr, span: span_of(line) })
    }

    /// The citation at the end of a line — everything from `@` on (§15.68). Returns the
    /// tokens before it and the citation; a citation whose shape cannot be read is E037 and
    /// is dropped, the rest of the line still being read.
    fn split_cite(&mut self, line: &[Token]) -> (Vec<Token>, Option<Cite>) {
        let Some(at) = line.iter().position(|t| t.is(&Kind::At)) else {
            return (line.to_vec(), None);
        };
        let head = line[..at].to_vec();
        let rest = &line[at + 1..];
        let span = span_of(&line[at..]);
        let bad = |p: &mut Self| {
            p.err(
                Diag::error("E037", tr!("引用の形が読めません", "A citation is not shaped like this"))
                    .at(p.at(span.line))
                    .mark(span.clone(), "")
                    .note(tr!(
                        "形は `@<出典> <箇所>` で、箇所は `,` で区切って並べられます（`@法 第20条, 第21条`）。隣に置いたファイルは箇所無しで `@郵便` とも、表を指して `@郵便 表1` とも書けます。語にならない箇所は `\"` で囲みます（`@osha \"§1910.157\"`）。出典は `{}` で宣言した名前です。",
                        "The shape is `@<source> <fragment>`, several fragments separated by `,` (`@法 第20条, 第21条`); a file beside the rule may be cited whole, `@郵便`, or by one of its tables, `@郵便 表1`. A fragment that is not a word goes in quotes (`@osha \"§1910.157\"`). The source is a name a `{}` line declares.",
                        crate::kw::SOURCE
                    )),
            );
        };
        let Some(source) = rest.first().and_then(|t| t.ident()).map(|s| s.to_string()) else {
            bad(self);
            return (head, None);
        };
        // The places cited, if any: a `file` source is cited whole (`@郵便`) or by one of its
        // tables (`@郵便 表1`); a law needs its article, which the check says.
        let mut fragments: Vec<String> = Vec::new();
        let mut k = 1;
        while k < rest.len() {
            // A fragment is a word — `第91条`, `表1` — or, where the database writes one with
            // characters a name cannot hold, a quoted string: `@osha "§1910.157"`.
            let frag = rest
                .get(k)
                .and_then(|t| t.ident().map(|s| s.to_string()).or_else(|| match &t.kind {
                    Kind::Str(f) => Some(f.clone()),
                    _ => None,
                }));
            match frag {
                Some(f) => {
                    fragments.push(f);
                    k += 1;
                }
                None => {
                    bad(self);
                    return (head, None);
                }
            }
            match rest.get(k) {
                None => break,
                Some(t) if t.is(&Kind::Comma) && k + 1 < rest.len() => k += 1,
                Some(_) => {
                    bad(self);
                    return (head, None);
                }
            }
        }
        (head, Some(Cite { source, fragments, span }))
    }

    /// `source <name> = law "<law id>" asof <date>` with its pinned fragments on the lines
    /// below (`  第91条 sha256:…`), or `source <name> = file "<path>" [sha256:…]` (§15.68).
    fn source(&mut self, line: &[Token]) -> Option<SourceDecl> {
        let span = span_of(line);
        self.i += 1;
        // The pins are consumed first, whatever the heading's shape: a parser that returns
        // without moving on reads the same line for ever.
        let mut pins: Vec<Pin> = Vec::new();
        while let Some(l) = self.cur().cloned() {
            // The fragment is written on a pin line the way it is written in the citation:
            // a word, or a quoted string where the database writes one with characters a
            // name cannot hold (`"§1910.157"`).
            let frag = l.first().and_then(|t| {
                t.ident().map(|s| s.to_string()).or_else(|| match &t.kind {
                    Kind::Str(f) => Some(f.clone()),
                    _ => None,
                })
            });
            match (frag, l.get(1).map(|t| t.kind.clone())) {
                (Some(frag), Some(Kind::Hash(h))) if l.len() == 2 => {
                    pins.push(Pin { fragment: frag, hash: h, span: span_of(&l) });
                    self.i += 1;
                }
                _ => break,
            }
        }
        let shape = tr!(
            "形は `{s} <名前> = {l} \"<法令ID>\" {a} <日付>` か `{s} <名前> = {f} \"<ファイル>\" [sha256:<ハッシュ>]` です。その下には引用した箇所ごとに `  <箇所> sha256:<ハッシュ>` の行が並びます（法令なら `第91条`、文書なら `表1`。`rulec source pin` が書きます）。",
            "The shape is `{s} <name> = {l} \"<law id>\" {a} <date>` or `{s} <name> = {f} \"<file>\" [sha256:<digest>]`. Under either, one `  <fragment> sha256:<digest>` line per cited fragment — an article for a law, a table for a document (`rulec source pin` writes them).",
            s = crate::kw::SOURCE,
            l = crate::kw::LAW,
            a = crate::kw::ASOF,
            f = crate::kw::FILE
        );
        let bad = |p: &mut Self, what: String| {
            p.err(
                Diag::error("E037", tr!("`{}` の形が読めません", "A `{}` is not shaped like this", crate::kw::SOURCE))
                    .at(p.at(span.line))
                    .mark(span.clone(), what)
                    .note(shape.clone()),
            );
        };
        let Some((name, k)) = self.name_at(line, 1) else {
            bad(self, tr!("出典の名前がありません", "the source has no name"));
            return None;
        };
        if !line.get(k).is_some_and(|t| t.is(&Kind::Eq)) {
            bad(self, tr!("`=` が要ります", "`=` is required"));
            return None;
        }
        let kind = match line.get(k + 1).and_then(|t| t.ident()) {
            Some(crate::kw::LAW) => {
                // `law <database> "<id>"`. The database word is a plain word rather than a
                // reserved one — it can only stand where a quoted id is expected, so nothing
                // else in the language has to give the name up. Left out, it is e-Gov.
                let mut i = k + 2;
                let db = match line.get(i).and_then(|t| t.ident()).and_then(crate::ast::LawDb::parse) {
                    Some(d) => {
                        i += 1;
                        d
                    }
                    None => crate::ast::LawDb::Egov,
                };
                let Some(Kind::Str(id)) = line.get(i).map(|t| t.kind.clone()) else {
                    bad(self, tr!(
                        "法令 ID を `\"…\"` で書いてください（データベースを書くなら `{l} egov` か `{l} ecfr` を先に）",
                        "write the law id in quotes (a database goes first: `{l} egov` or `{l} ecfr`)",
                        l = crate::kw::LAW
                    ));
                    return None;
                };
                if line.get(i + 1).and_then(|t| t.ident()) != Some(crate::kw::ASOF) {
                    bad(self, tr!("`{} <日付>` が要ります", "`{} <date>` is required", crate::kw::ASOF));
                    return None;
                }
                let Some(Kind::Date(y, m, d)) = line.get(i + 2).map(|t| t.kind.clone()) else {
                    bad(self, tr!("日付は `YYYY-MM-DD` です", "the date is `YYYY-MM-DD`"));
                    return None;
                };
                if line.len() > i + 3 {
                    bad(self, tr!("日付の後に余分な語があります", "extra words after the date"));
                    return None;
                }
                SourceKind::Law { db, id, asof: format!("{y:04}-{m:02}-{d:02}") }
            }
            Some(crate::kw::FILE) => {
                let Some(Kind::Str(path)) = line.get(k + 2).map(|t| t.kind.clone()) else {
                    bad(self, tr!("ファイル名を `\"…\"` で書いてください", "write the file name in quotes"));
                    return None;
                };
                // `url "…"` before the digest: where the copy came from (§15.76). Optional,
                // because a document that arrived from a person has no address.
                let mut i = k + 3;
                let url = if line.get(i).and_then(|t| t.ident()) == Some(crate::kw::URL) {
                    let Some(Kind::Str(u)) = line.get(i + 1).map(|t| t.kind.clone()) else {
                        bad(self, tr!("URL を `\"…\"` で書いてください", "write the URL in quotes"));
                        return None;
                    };
                    i += 2;
                    Some(u)
                } else {
                    None
                };
                let hash = match line.get(i).map(|t| t.kind.clone()) {
                    Some(Kind::Hash(h)) => {
                        i += 1;
                        Some(h)
                    }
                    None => None,
                    Some(_) => {
                        bad(self, tr!(
                            "ファイル名の後に書けるのは `{} \"…\"` と `sha256:<ハッシュ>` だけです",
                            "only `{} \"…\"` and `sha256:<digest>` may follow the file name",
                            crate::kw::URL
                        ));
                        return None;
                    }
                };
                if line.len() > i {
                    bad(self, tr!("余分な語があります", "extra words"));
                    return None;
                }
                SourceKind::File { path, url, hash }
            }
            _ => {
                bad(self, tr!("`=` の後は `{}` か `{}` です", "after `=` comes `{}` or `{}`", crate::kw::LAW, crate::kw::FILE));
                return None;
            }
        };
        Some(SourceDecl { name, kind, pins, span, base: None })
    }

    /// `apply <name> = "<path>" [sha256:…]`, then bindings `<input> = <value> [with a -> b, …]`,
    /// `except <target>, …` and `<output> -> <name>` lines, in any order (§15.69).
    fn apply(&mut self, head: &[Token], at: usize) -> Option<ApplyDecl> {
        let span = span_of(head);
        self.i += 1;
        let shape = tr!(
            "形は `{a} <名前> = \"<規則ファイル>\" sha256:<ハッシュ>` の下に、`<元の規則の入力> = <この規則の値>`（列挙なら `with <値> -> <値>, …` を続ける）、`{e} <準用しない定義>, …`、`<元の規則の出力> -> <名前>` を一行ずつ書きます。",
            "The shape is `{a} <name> = \"<rule file>\" sha256:<digest>`, then one `<callee input> = <value>` line each (for an enum, followed by `with <value> -> <value>, …`), `{e} <target>, …`, and `<callee output> -> <name>` lines.",
            a = crate::kw::APPLY,
            e = crate::kw::EXCEPT
        );
        let bad = |p: &mut Self, sp: Span, what: String| {
            p.err(
                Diag::error("E041", tr!("`{}` の形が読めません", "An `{}` is not shaped like this", crate::kw::APPLY))
                    .at(p.at(sp.line))
                    .mark(sp, what)
                    .note(shape.clone()),
            );
        };
        // The body first, so the block is consumed whatever the heading's shape.
        let mut bindings: Vec<Binding> = Vec::new();
        let mut excepts: Vec<(String, Span)> = Vec::new();
        let mut outputs: Vec<OutBinding> = Vec::new();
        let mut ok = true;
        while let Some(l) = self.cur().cloned() {
            if l.is_empty() || l.first().is_some_and(|t| t.is(&Kind::Pipe)) {
                break;
            }
            let Some(first) = l.first().and_then(|t| t.ident()).map(|s| s.to_string()) else { break };
            if crate::kw::LINE_HEAD.contains(&first.as_str()) && first != crate::kw::EXCEPT {
                break;
            }
            self.i += 1;
            if first == crate::kw::EXCEPT {
                let mut k = 1;
                while k < l.len() {
                    match l[k].ident() {
                        Some(t) => {
                            // `表:行` is one target; the segments are joined back with `:`.
                            let mut name = t.to_string();
                            let mut sp = l[k].span.clone();
                            k += 1;
                            while l.get(k).is_some_and(|t| t.is(&Kind::Colon)) {
                                match l.get(k + 1).and_then(|t| t.ident()) {
                                    Some(r) => {
                                        name.push(':');
                                        name.push_str(r);
                                        sp.len = l[k + 1].span.col + l[k + 1].span.len - sp.col;
                                        k += 2;
                                    }
                                    None => break,
                                }
                            }
                            excepts.push((name, sp));
                            if l.get(k).is_some_and(|t| t.is(&Kind::Comma)) {
                                k += 1;
                            }
                        }
                        None => {
                            bad(self, span_of(&l), tr!("`{}` の相手が読めません", "the target of `{}` cannot be read", crate::kw::EXCEPT));
                            ok = false;
                            break;
                        }
                    }
                }
                if excepts.is_empty() && ok {
                    bad(self, span_of(&l), tr!("`{}` の相手がありません", "`{}` names nothing", crate::kw::EXCEPT));
                    ok = false;
                }
                continue;
            }
            match l.get(1).map(|t| &t.kind) {
                Some(Kind::Eq) => {
                    let value = match l.get(2).map(|t| &t.kind) {
                        Some(Kind::Ident(n)) => BindValue::Name(n.clone()),
                        Some(_) => match lit_of(&l[2]) {
                            Some(v) => BindValue::Lit(v),
                            None => {
                                bad(self, span_of(&l), tr!("`=` の右は名前かリテラルです", "the right side of `=` is a name or a literal"));
                                ok = false;
                                continue;
                            }
                        },
                        None => {
                            bad(self, span_of(&l), tr!("`=` の右に値がありません", "nothing after `=`"));
                            ok = false;
                            continue;
                        }
                    };
                    let mut map: Vec<(String, String)> = Vec::new();
                    let mut k = 3;
                    if l.get(k).and_then(|t| t.ident()) == Some(crate::kw::WITH) {
                        k += 1;
                        loop {
                            let (Some(a), Some(arrow), Some(b)) = (l.get(k).and_then(|t| t.ident()), l.get(k + 1), l.get(k + 2).and_then(|t| t.ident())) else {
                                bad(self, span_of(&l), tr!("`{}` の後は `<値> -> <値>` の並びです", "after `{}` come `<value> -> <value>` pairs", crate::kw::WITH));
                                ok = false;
                                break;
                            };
                            if !arrow.is(&Kind::Arrow) {
                                bad(self, span_of(&l), tr!("`{}` の後は `<値> -> <値>` の並びです", "after `{}` come `<value> -> <value>` pairs", crate::kw::WITH));
                                ok = false;
                                break;
                            }
                            map.push((a.to_string(), b.to_string()));
                            k += 3;
                            match l.get(k) {
                                None => break,
                                Some(t) if t.is(&Kind::Comma) => k += 1,
                                Some(_) => {
                                    bad(self, span_of(&l), tr!("対応の区切りは `,` です", "pairs are separated by `,`"));
                                    ok = false;
                                    break;
                                }
                            }
                        }
                    } else if l.len() > 3 {
                        bad(self, span_of(&l), tr!("値の後に書けるのは `{} …` だけです", "only `{} …` may follow the value", crate::kw::WITH));
                        ok = false;
                        continue;
                    }
                    bindings.push(Binding { input: first, value, map, span: span_of(&l) });
                }
                Some(Kind::Arrow) => {
                    let Some((name, k)) = self.name_at(&l, 2) else {
                        bad(self, span_of(&l), tr!("`->` の右に名前がありません", "no name after `->`"));
                        ok = false;
                        continue;
                    };
                    if l.len() > k {
                        bad(self, span_of(&l), tr!("名前の後に余分な語があります", "extra words after the name"));
                        ok = false;
                        continue;
                    }
                    outputs.push(OutBinding { output: first, name, span: span_of(&l) });
                }
                _ => {
                    bad(self, span_of(&l), tr!("`<入力> = <値>` か `<出力> -> <名前>` の形です", "a line is `<input> = <value>` or `<output> -> <name>`"));
                    ok = false;
                }
            }
        }
        let Some((name, k)) = self.name_at(head, 1) else {
            bad(self, span.clone(), tr!("準用の名前がありません", "the apply has no name"));
            return None;
        };
        if !head.get(k).is_some_and(|t| t.is(&Kind::Eq)) {
            bad(self, span.clone(), tr!("`=` が要ります", "`=` is required"));
            return None;
        }
        let Some(Kind::Str(path)) = head.get(k + 1).map(|t| t.kind.clone()) else {
            bad(self, span.clone(), tr!("規則ファイルを `\"…\"` で書いてください", "write the rule file in quotes"));
            return None;
        };
        let hash = match head.get(k + 2).map(|t| t.kind.clone()) {
            Some(Kind::Hash(h)) => Some(h),
            None => None,
            Some(_) => {
                bad(self, span.clone(), tr!("ファイル名の後に書けるのは `sha256:<ハッシュ>` だけです", "only `sha256:<digest>` may follow the file name"));
                return None;
            }
        };
        if head.len() > k + 3 {
            bad(self, span.clone(), tr!("余分な語があります", "extra words"));
            return None;
        }
        if !ok {
            return None;
        }
        Some(ApplyDecl {
            name,
            path,
            hash,
            bindings,
            excepts,
            outputs,
            cite: None,
            span,
            at,
            count: 0,
            defines: Vec::new(),
            callee_src: String::new(),
            callee_path: String::new(),
            callee: None,
        })
    }

    fn table(&mut self, head: &[Token]) -> Option<Table> {
        let named = self.name_at(head, 1);
        let name = named.as_ref().map(|(n, _)| n.clone());
        let span = span_of(head);
        self.i += 1;
        let mut policy = Policy::Unique; // §4: the default is unique.
        let mut overrides: Vec<OverrideRef> = Vec::new();
        // `policy` and `overrides` may follow the `table` line, in either order, once each.
        for _ in 0..2 {
            let Some(l) = self.cur().cloned() else { break };
            match l.first().and_then(|t| t.ident()) {
                Some(crate::kw::POLICY) => {
                    match l.get(1).and_then(|t| t.ident()) {
                        Some(crate::kw::UNIQUE) => policy = Policy::Unique,
                        Some(crate::kw::FIRST) => policy = Policy::TopDown,
                        other => self.err(
                            Diag::error("E007", tr!("`{}` という方式はありません", "There is no policy `{}`", other.unwrap_or("")))
                                .fix(crate::diag::FixKind::ChangePolicy, format!("{} {}", crate::kw::POLICY, crate::kw::UNIQUE))
                                .mark(span_of(&l), "")
                                .note(tr!("書けるのは {} です（§4）", "The policy must be {} (§4)", crate::kw::policies())),
                        ),
                    }
                    self.i += 1;
                }
                Some(crate::kw::OVERRIDES) => {
                    overrides = self.override_refs(&l);
                    self.i += 1;
                }
                _ => break,
            }
        }
        self.ctx = match &name {
            Some(n) => tr!("表 {}", "table {}", n.text),
            None => String::new(),
        };
        let (inputs, outputs, rows) = self.grid(true, false)?;
        self.ctx.clear();
        // A table is named, and nothing else stands on its line. One with no name was read as
        // an anonymous table, the word after `table` dropped, and the code generated for it
        // lost the scale of its output — a rate of 0.6% came back a thousand times too large
        // (§15.156). The rows are read first, so that they are not reported line by line.
        let at = self.at(span.line);
        match named {
            None => {
                self.err(unreadable(crate::kw::TABLE, head, tr!("`table` の後に名前がありません", "there is no name after `table`"), at));
                return None;
            }
            Some((_, k)) if k < head.len() => {
                self.err(unreadable(crate::kw::TABLE, &head[k..], tr!("名前の後のこの語は読まれません", "this is not read after the name"), at));
                return None;
            }
            _ => {}
        }
        Some(Table { name, policy, inputs, outputs, rows, span, overrides, clause: false, cite: None, applied: None })
    }

    /// A clause (§15.67): `clause <name>(<alias>) -> <output>[ : <type>]`, then a
    /// `when` line, a `then` line and an optional `overrides` line, in any order. It becomes a
    /// one-row table whose columns are the ones `when` names.
    fn clause(&mut self, head: &[Token]) -> Option<Table> {
        let span = span_of(head);
        self.i += 1;
        // The body lines are consumed whatever goes wrong with the heading: a parser that
        // returns without moving on reads the same line for ever.
        let mut when: Option<Vec<Token>> = None;
        let mut then: Option<Vec<Token>> = None;
        let mut overrides: Vec<OverrideRef> = Vec::new();
        let mut twice: Vec<Vec<Token>> = Vec::new();
        loop {
            let Some(l) = self.cur().cloned() else { break };
            match l.first().and_then(|t| t.ident()) {
                Some(crate::kw::WHEN) => {
                    if when.is_some() {
                        twice.push(l);
                    } else {
                        when = Some(l);
                    }
                    self.i += 1;
                }
                Some(crate::kw::THEN) => {
                    if then.is_some() {
                        twice.push(l);
                    } else {
                        then = Some(l);
                    }
                    self.i += 1;
                }
                Some(crate::kw::OVERRIDES) => {
                    overrides = self.override_refs(&l);
                    self.i += 1;
                }
                _ => break,
            }
        }
        let shape = tr!(
            "形は `{c} <名前>(<別名>) -> <出力>` の下に `{w} <列> <セル> and …`（条件が無ければ `{w} {a}`）と `{t} <値>` を一行ずつ、任意で `{o} <相手>` です。",
            "The shape is `{c} <name>(<alias>) -> <output>`, then one `{w} <column> <cell> and …` line (`{w} {a}` when there is no condition), one `{t} <value>` line, and optionally `{o} <target>`.",
            c = crate::kw::CLAUSE,
            w = crate::kw::WHEN,
            a = crate::kw::ALWAYS,
            t = crate::kw::THEN,
            o = crate::kw::OVERRIDES
        );
        let bad = |p: &mut Self, sp: Span, what: String| {
            p.err(
                Diag::error("E046", tr!("`{}` の形が読めません", "A `{}` is not shaped like this", crate::kw::CLAUSE))
                    .at(p.at(sp.line))
                    .mark(sp, what)
                    .note(shape.clone()),
            );
        };
        let Some((name, k)) = self.name_at(head, 1) else {
            bad(self, span.clone(), tr!("節の名前がありません", "the clause has no name"));
            return None;
        };
        if !head.get(k).is_some_and(|t| t.is(&Kind::Arrow)) {
            bad(self, span.clone(), tr!("`->` と出力の名前が要ります", "`->` and the output's name are required"));
            return None;
        }
        let Some((oname, k2)) = self.name_at(head, k + 1) else {
            bad(self, span.clone(), tr!("`->` の後に出力の名前がありません", "no output name after `->`"));
            return None;
        };
        let ty = self.type_ref(head, k2).map(|(t, _)| t);
        let out = OutCol { name: oname, ty, span: span_of(&head[k + 1..]) };
        for l in twice {
            bad(self, span_of(&l), tr!("同じ行が二度あります", "this line appears twice"));
        }
        let Some(wl) = when else {
            bad(self, span.clone(), tr!("`{}` の行がありません", "there is no `{}` line", crate::kw::WHEN));
            return None;
        };
        let Some(tl) = then else {
            bad(self, span.clone(), tr!("`{}` の行がありません", "there is no `{}` line", crate::kw::THEN));
            return None;
        };
        // `when always`, or `<column> <cell>` joined by `and`.
        let mut inputs: Vec<(String, Span)> = Vec::new();
        let mut cells: Vec<Cell> = Vec::new();
        let mut cell_spans: Vec<Span> = Vec::new();
        let body = &wl[1..];
        let always = body.len() == 1 && body[0].ident() == Some(crate::kw::ALWAYS);
        if body.is_empty() {
            bad(self, span_of(&wl), tr!("条件がありません。無いなら `{} {}` と書きます", "there is no condition; write `{} {}` when there is none", crate::kw::WHEN, crate::kw::ALWAYS));
            return None;
        }
        if !always {
            let mut parts: Vec<Vec<Token>> = vec![Vec::new()];
            for t in body {
                if t.ident() == Some("and") {
                    parts.push(Vec::new());
                } else {
                    parts.last_mut().unwrap().push(t.clone());
                }
            }
            let mut ok = true;
            for part in parts {
                let Some(col) = part.first().and_then(|t| t.ident()).map(|s| s.to_string()) else {
                    bad(self, span_of(&wl), tr!("`and` の区切りの中に列の名前がありません", "a part between `and` has no column name"));
                    ok = false;
                    continue;
                };
                if part.len() < 2 {
                    bad(self, part[0].span.clone(), tr!("列 {col} の条件がありません", "column {col} has no condition"));
                    ok = false;
                    continue;
                }
                if inputs.iter().any(|(c, _)| *c == col) {
                    bad(self, part[0].span.clone(), tr!("列 {col} が二度あります", "column {col} appears twice"));
                    ok = false;
                    continue;
                }
                let Some(cell) = self.cell(&part[1..]) else {
                    ok = false;
                    continue;
                };
                inputs.push((col, part[0].span.clone()));
                cells.push(cell);
                cell_spans.push(span_of(&part[1..]));
            }
            if !ok {
                return None;
            }
        }
        if tl.len() < 2 {
            bad(self, span_of(&tl), tr!("`{}` の後に値がありません", "there is no value after `{}`", crate::kw::THEN));
            return None;
        }
        let value = self.out_cell(&tl[1..]);
        let row = Row {
            cells,
            cell_spans,
            outs: vec![value],
            out_spans: vec![span_of(&tl[1..])],
            span: span_of(&wl),
            index: 1,
            label: None,
            origin: None,
            cite: None,
        };
        Some(Table {
            name: Some(name),
            policy: Policy::Unique,
            inputs,
            outputs: vec![out],
            rows: vec![row],
            span,
            overrides,
            clause: true,
            cite: None,
            applied: None,
        })
    }

    /// The targets of an `overrides` line: `A, B, 表:行`, each a table declared above or one
    /// labelled row of it. The shape is checked here; whether the target exists is E035, in
    /// the type checker.
    fn override_refs(&mut self, line: &[Token]) -> Vec<OverrideRef> {
        let mut out = Vec::new();
        let mut k = 1;
        let mut bad = false;
        while k < line.len() {
            let Some(table) = line[k].ident().map(|s| s.to_string()) else {
                bad = true;
                break;
            };
            let mut span = line[k].span.clone();
            k += 1;
            // `表:行`, or `呼び出し:表:行` for a row of an applied rule (§15.69): the segments
            // before the last name the table, the last names the row — unless the whole
            // spelling names a table, which the checker decides.
            let mut segs: Vec<String> = vec![table];
            while line.get(k).is_some_and(|t| t.is(&Kind::Colon)) {
                match line.get(k + 1).and_then(|t| t.ident()) {
                    Some(r) => {
                        segs.push(r.to_string());
                        span.len = line[k + 1].span.col + line[k + 1].span.len - span.col;
                        k += 2;
                    }
                    None => {
                        bad = true;
                        break;
                    }
                }
            }
            if bad {
                break;
            }
            let (table, row) = if segs.len() == 1 {
                (segs.remove(0), None)
            } else {
                let row = segs.pop();
                (segs.join(":"), row)
            };
            out.push(OverrideRef { table, row, span });
            match line.get(k) {
                None => break,
                Some(t) if t.is(&Kind::Comma) => k += 1,
                Some(_) => {
                    bad = true;
                    break;
                }
            }
        }
        if bad || out.is_empty() {
            self.err(
                Diag::error("E035", tr!("`{}` の指す先が読めません", "The target of `{}` cannot be read", crate::kw::OVERRIDES))
                    .at(self.at(line[0].span.line))
                    .mark(span_of(line), "")
                    .note(tr!(
                        "形は `{} <表>` か `{} <表>:<行ラベル>` で、複数は `,` で区切ります。",
                        "The form is `{} <table>` or `{} <table>:<row label>`, several separated by `,`.",
                        crate::kw::OVERRIDES,
                        crate::kw::OVERRIDES
                    )),
            );
        }
        out
    }

    fn example_table(&mut self) -> Option<Table> {
        self.ctx = tr!("例", "examples");
        let (inputs, outputs, rows) = self.grid(false, true)?;
        self.ctx.clear();
        Some(Table {
            name: None,
            policy: Policy::Unique,
            inputs,
            outputs,
            rows,
            span: Span::new(1, 0, 1),
            overrides: Vec::new(),
            clause: false,
            cite: None,
            applied: None,
        })
    }

    /// The `|` block: one header row, then rule rows. With `labels`, a row may start with one
    /// word before its first bar, which names the row (§2.1 of the statute draft); the rows of
    /// `examples` and of a `sequence` have no use for a name, so there a leading word ends the
    /// block as any statement does.
    /// The rows under a heading. `examples` is set for the examples: their rows are held to the
    /// header after typing (`eval::check_examples`), where E111 and E025 can first name a
    /// column the header is missing — a row one cell longer than such a header is its
    /// symptom, not its cause.
    fn grid(&mut self, labels: bool, examples: bool) -> Option<(Vec<(String, Span)>, Vec<OutCol>, Vec<Row>)> {
        let mut raw_rows: Vec<Vec<Token>> = Vec::new();
        let is_row = |l: &[Token]| -> bool {
            match l.first() {
                Some(t) if t.is(&Kind::Pipe) => true,
                Some(t) if labels && t.ident().is_some() => l.get(1).is_some_and(|t| t.is(&Kind::Pipe)),
                _ => false,
            }
        };
        while self.cur().is_some_and(|l| is_row(l)) {
            raw_rows.push(self.cur().cloned().unwrap());
            self.i += 1;
        }
        if raw_rows.is_empty() {
            return None;
        }
        let header = split_cells(&raw_rows[0]);
        // A header that cannot be read is said once, and the rows are then not held to it: a
        // row counted against a broken header would be reported for the header's fault.
        let mut header_ok = self.closed(&raw_rows[0]);
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        // `→` marks the boundary between inputs and outputs once. Every column after it is an
        // output whether or not it carries a `→`, so a column whose second `→` was left out does
        // not silently turn into an input (the examples of a multi-output rule hit exactly that).
        let mut in_outputs = false;
        for (cell, _) in &header {
            let arrow = cell.first().is_some_and(|t| t.is(&Kind::Arrow));
            in_outputs |= arrow;
            if in_outputs {
                let (nm, k) = match self.name_at(cell, usize::from(arrow)) {
                    Some(v) => v,
                    // A blank heading in the examples leaves an output without its column,
                    // which E111 names.
                    None if examples && cell.is_empty() => continue,
                    None => {
                        let what = tr!("出力の列に名前がありません", "this output column has no name");
                        self.bad_cell(cell, &raw_rows[0], what);
                        header_ok = false;
                        continue;
                    }
                };
                let ty = self.type_ref(cell, k).map(|(t, _)| t);
                outputs.push(OutCol { name: nm, ty, span: span_of(cell) });
            } else {
                // One name per input column. A heading that is not a name used to be skipped,
                // and every cell under it moved one column to the left.
                match cell.first().and_then(|t| t.ident()) {
                    Some(w) if cell.len() == 1 => inputs.push((w.to_string(), span_of(cell))),
                    Some(_) => match self.name_at(cell, 0) {
                        Some((nm, k)) if k == cell.len() => inputs.push((nm.text, span_of(cell))),
                        _ => {
                            let what = tr!("列の名前の後ろに、読めない語があります", "something follows the column's name");
                            self.bad_cell(cell, &raw_rows[0], what);
                            header_ok = false;
                        }
                    },
                    None => {
                        let what = tr!("列の名前がありません", "this column has no name");
                        self.bad_cell(cell, &raw_rows[0], what);
                        header_ok = false;
                    }
                }
            }
        }
        let ncol = inputs.len();
        let mut rows = Vec::new();
        for (n, rt) in raw_rows[1..].iter().enumerate() {
            let (rt, cite) = self.split_cite(rt);
            let rt = &rt;
            let cells_t = split_cells(rt);
            // One cell per column, and a bar at the end. A row one cell short, or with its last
            // bar left off (and its last cell with it), passed: the reference evaluator and the
            // generated code then read the row two different ways (§15.156).
            if self.closed(rt) && header_ok && !examples && cells_t.len() != header.len() {
                let at = self.at(rt[0].span.line);
                self.err(
                    Diag::error("E064", tr!("行が見出しと合いません", "The row does not fit the header"))
                        .at(at)
                        .mark(span_of(rt), tr!("この行のセルは {} 個で、見出しの列は {} 個です", "this row has {} cells, and the header {} columns", cells_t.len(), header.len()))
                        .note(tr!(
                            "一つの列に一つのセルを書きます。どの値でもよい列には `-` を書きます。",
                            "Write one cell per column; a column that takes any value gets `-`."
                        )),
                );
            }
            let mut cells = Vec::new();
            let mut cell_spans = Vec::new();
            let mut outs = Vec::new();
            let mut out_spans = Vec::new();
            for (ci, (ct, cspan)) in cells_t.iter().enumerate() {
                if ct.is_empty() {
                    let col = self
                        .colname(&inputs, &outputs, ci)
                        .map(|n| tr!("列 {n} が空です", "column {n} is empty"))
                        .unwrap_or_else(|| tr!("空です", "empty"));
                    let at = self.at(rt[0].span.line);
                    self.err(
                        Diag::error("E008", tr!("空のセルがあります", "Empty cell"))
                            .at(at)
                            .mark(cspan.clone(), col)
                            .note(tr!("任意の値に当てるなら `-` と書いてください（空欄は書き忘れと区別がつきません。§3）", "Write `-` to match any value (an empty cell cannot be told apart from an omission; §3)")),
                    );
                    // The place is kept, so that the cells after it stay under their own
                    // headings; the error has already stopped the rule.
                    if ci < ncol {
                        cells.push(Cell::DontCare);
                        cell_spans.push(cspan.clone());
                    } else {
                        outs.push(OutCell::Name(String::new()));
                        out_spans.push(cspan.clone());
                    }
                    continue;
                }
                if ci < ncol {
                    // A cell that cannot be read keeps its place too: dropping it moved every
                    // cell after it one column to the left.
                    let c = self.cell(ct).unwrap_or(Cell::DontCare);
                    cells.push(c);
                    cell_spans.push(cspan.clone());
                } else {
                    outs.push(self.out_cell(ct));
                    out_spans.push(cspan.clone());
                }
            }
            // The word before the first bar is the row's label. `split_cells` starts at the
            // first bar, so the label never reaches the cells.
            let label = match rt.first() {
                Some(t) if !t.is(&Kind::Pipe) => t.ident().map(|w| Name { text: w.to_string(), ascii: None, span: t.span.clone() }),
                _ => None,
            };
            rows.push(Row { cells, cell_spans, outs, out_spans, span: span_of(rt), index: n + 1, label, origin: None, cite });
        }
        Some((inputs, outputs, rows))
    }

    fn colname(&self, inputs: &[(String, Span)], outputs: &[OutCol], ci: usize) -> Option<String> {
        if ci < inputs.len() {
            Some(inputs[ci].0.clone())
        } else {
            outputs.get(ci - inputs.len()).map(|o| o.name.text.clone())
        }
    }

    fn cell(&mut self, ts: &[Token]) -> Option<Cell> {
        // Every caller hands over at least one token; an empty cell is E008's, reported where
        // the row is split.
        if ts.is_empty() {
            return None;
        }
        if let Some(t) = ts.iter().find(|t| t.is(&Kind::DotDot)) {
            let at = self.at(t.span.line);
            self.err(
                Diag::error("E010", tr!("`..` は書けません", "`..` is not allowed"))
                    .at(at)
                    .mark(t.span.clone(), "")
                    .note(tr!("`<=2000g` と `>2000g` のどちらの意味かが読めないためです（§3.1）。比較演算子で書いてください", "It is unclear whether `<=2000g` or `>2000g` is meant (§3.1). Use a comparison operator instead")),
            );
            return None;
        }
        if ts.len() == 1 && ts[0].is(&Kind::Minus) {
            return Some(Cell::DontCare);
        }
        if ts[0].ident() == Some(crate::kw::NONE) {
            if ts.len() > 1 {
                return self.cell_junk(&ts[1..]);
            }
            return Some(Cell::Nothing);
        }
        if ts[0].ident() == Some(crate::kw::STARTS_WITH) {
            let rest = if ts.get(1).is_some_and(|t| t.is(&Kind::Colon)) { &ts[2..] } else { &ts[1..] };
            let mut v = Vec::new();
            for t in rest {
                match &t.kind {
                    Kind::Str(s) => v.push(s.clone()),
                    Kind::Comma => {}
                    _ => {
                        let at = self.at(t.span.line);
                        self.err(
                            Diag::error("E010", tr!("`starts_with` の右は文字列です", "`starts_with` takes a string"))
                                .at(at)
                                .mark(t.span.clone(), tr!("文字列ではありません", "not a string"))
                                .note(tr!(
                                    "`starts_with \"ABC\"` の形で書いてください。前置きを二つ以上書くならコンマで区切ります。",
                                    "Write it as `starts_with \"ABC\"`. Separate two or more prefixes with a comma."
                                )),
                        );
                        return None;
                    }
                }
            }
            return Some(Cell::Prefix(v));
        }
        if ts[0].ident() == Some(crate::kw::NOT) {
            let rest = if ts.get(1).is_some_and(|t| t.is(&Kind::Colon)) { &ts[2..] } else { &ts[1..] };
            if let Some(bad) = rest.iter().find(|t| !t.is(&Kind::Comma) && lit_of(t).is_none()) {
                return self.cell_junk(std::slice::from_ref(bad));
            }
            return Some(Cell::Not(lits(rest)));
        }
        if matches!(ts[0].kind, Kind::Le | Kind::Ge | Kind::Lt | Kind::Gt) {
            let mut v = Vec::new();
            let mut k = 0;
            while k + 1 < ts.len() + 1 && k < ts.len() {
                let op = match ts[k].kind {
                    Kind::Le => CmpOp::Le,
                    Kind::Ge => CmpOp::Ge,
                    Kind::Lt => CmpOp::Lt,
                    Kind::Gt => CmpOp::Gt,
                    _ => break,
                };
                let Some(l) = ts.get(k + 1).and_then(lit_of) else { break };
                v.push((op, l));
                k += 2;
            }
            // `<=0円 + false` read as `<=0円`, the rest dropped (§15.156).
            if k < ts.len() {
                return self.cell_junk(&ts[k..]);
            }
            return Some(Cell::Cmp(v));
        }
        if ts.iter().any(|t| t.is(&Kind::Comma)) {
            if let Some(bad) = ts.iter().find(|t| !t.is(&Kind::Comma) && lit_of(t).is_none()) {
                return self.cell_junk(std::slice::from_ref(bad));
            }
            return Some(Cell::Set(lits(ts)));
        }
        // One value: `false 0円` read as `false`, and `+` as nothing at all.
        match lit_of(&ts[0]) {
            Some(l) if ts.len() == 1 => Some(Cell::Lit(l)),
            Some(_) => self.cell_junk(&ts[1..]),
            None => self.cell_junk(&ts[..1]),
        }
    }

    /// E063 for what a cell holds and cannot be read.
    fn cell_junk(&mut self, junk: &[Token]) -> Option<Cell> {
        let at = self.at(junk[0].span.line);
        self.err(
            Diag::error("E063", tr!("セルを読めません", "The cell cannot be read"))
                .at(at)
                .mark(span_of(junk), tr!("ここはセルとして読めません", "this is not read as part of the cell"))
                .note(tr!(
                    "セルに書けるのは、値一つ、`-`、`none`、比較（`<=2000g`、`>=1 <10`）、コンマで区切った値の集合、`not:` と集合、`starts_with` と文字列です（§3）。",
                    "A cell holds one value, `-`, `none`, a comparison (`<=2000g`, `>=1 <10`), a set of values separated by commas, `not:` with a set, or `starts_with` with a string (§3)."
                )),
        );
        None
    }

    /// E063 for a header cell, at the header's line.
    fn bad_cell(&mut self, cell: &[Token], header: &[Token], what: String) {
        let at = self.at(header.first().map(|t| t.span.line).unwrap_or(1));
        let span = if cell.is_empty() { span_of(header) } else { span_of(cell) };
        self.err(Diag::error("E063", tr!("セルを読めません", "The cell cannot be read")).at(at).mark(span, what));
    }

    /// Whether the row ends with its bar. E064 when words follow the last one: they were a
    /// cell whose closing bar was left off, and they were dropped with it.
    fn closed(&mut self, row: &[Token]) -> bool {
        let Some(last_bar) = row.iter().rposition(|t| t.is(&Kind::Pipe)) else { return true };
        if last_bar + 1 == row.len() {
            return true;
        }
        let at = self.at(row[0].span.line);
        self.err(
            Diag::error("E064", tr!("行が見出しと合いません", "The row does not fit the header"))
                .at(at)
                .mark(span_of(&row[last_bar + 1..]), tr!("最後の `|` の後ろにあります。行を `|` で閉じてください", "this follows the last `|`; close the row with a `|`")),
        );
        false
    }

    fn out_cell(&mut self, ts: &[Token]) -> OutCell {
        // §3.2 allows one value or one name here and no arithmetic. Reading only the first
        // token and dropping the rest turned `商品合計 × 割引率` into `商品合計`, which
        // generated code that quietly left the multiplication out.
        if ts.len() > 1 {
            let at = self.at(ts[0].span.line);
            let first = &ts[0].span;
            let last = &ts[ts.len() - 1].span;
            let span = Span::new(first.line, first.col, (last.col + last.len).saturating_sub(first.col));
            self.err(
                Diag::error("E014", tr!("出力のセルに式は書けません", "An output cell cannot hold an expression"))
                    .at(at)
                    .mark(span, tr!("ここは語が二つ以上あります", "two or more words here"))
                    .note(tr!(
                        "出力のセルに書けるのは、値一つか名前一つだけです（§3.2）。かけ算や足し算は書けません。",
                        "An output cell holds one value or one name (§3.2). Arithmetic cannot be written there."
                    ))
                    .note(tr!(
                        "計算には名前を付けて、`define` の行に出してください。表には名前だけが残ります。",
                        "Give the calculation a name on a `define` line, and leave only that name in the table."
                    )),
            );
        }
        match lit_of(&ts[0]) {
            Some(Lit::Word(w)) => {
                // A bare word is an enum value or, per §3.2, the name of an input / derived / define.
                OutCell::Name(w)
            }
            Some(l) => OutCell::Lit(l),
            None => {
                self.cell_junk(&ts[..1]);
                OutCell::Name(String::new())
            }
        }
    }

    /// The expression after `anchor` — the `=` of a declaration, the `->` of an arm, the
    /// `with` of a stop. E059 when nothing follows it.
    fn expr_after(&mut self, anchor: &Token, ts: &[Token]) -> Option<Expr> {
        if ts.is_empty() {
            let what = match &anchor.kind {
                Kind::Ident(w) => tr!("`{w}` の右に式がありません", "there is no expression after `{w}`"),
                k => tr!("`{}` の右に式がありません", "there is no expression after `{}`", op_text(k)),
            };
            self.err(bad_expr(std::slice::from_ref(anchor), what, self.at(anchor.span.line)));
            return None;
        }
        self.expr(ts)
    }

    /// `expr_after` for a declaration line, whose anchor is its `=`.
    fn expr_of(&mut self, line: &[Token], eq: usize, ts: &[Token]) -> Option<Expr> {
        self.expr_after(&line[eq], ts)
    }

    /// Precedence: comparison < additive < multiplicative.
    ///
    /// Every token is read or E059 says which one was not. This used to return nothing, or
    /// return what it had read so far, whenever it met something it did not expect: `金額 税`
    /// (a `+` forgotten) was read as `金額`, `x +` and `(x + 1` made the whole declaration
    /// vanish, and a malformed `derive` hung the parser (§15.156). An empty slice is the
    /// caller's to report, since only the caller knows what it follows.
    fn expr(&mut self, ts: &[Token]) -> Option<Expr> {
        if ts.is_empty() {
            return None;
        }
        self.expr_cmp(ts)
    }

    /// E059 for an operator with nothing on one side.
    fn missing_side(&mut self, op: &Token, left: bool) -> Option<Expr> {
        let o = op_text(&op.kind);
        let what = if left {
            tr!("`{o}` の左に値がありません", "there is nothing on the left of `{o}`")
        } else {
            tr!("`{o}` の右に値がありません", "there is nothing on the right of `{o}`")
        };
        self.err(bad_expr(std::slice::from_ref(op), what, self.at(op.span.line)));
        None
    }

    fn expr_cmp(&mut self, ts: &[Token]) -> Option<Expr> {
        if let Some(p) = top_pos(ts, |k| matches!(k, Kind::Le | Kind::Ge | Kind::Lt | Kind::Gt | Kind::Eq)) {
            let op = match ts[p].kind {
                Kind::Le => BinOp::Le,
                Kind::Ge => BinOp::Ge,
                Kind::Lt => BinOp::Lt,
                Kind::Gt => BinOp::Gt,
                _ => BinOp::Eq,
            };
            if p == 0 {
                return self.missing_side(&ts[p], true);
            }
            if p + 1 == ts.len() {
                return self.missing_side(&ts[p], false);
            }
            let l = self.expr_add(&ts[..p])?;
            let r = self.expr_add(&ts[p + 1..])?;
            return Some(Expr::Bin(Box::new(l), op, Box::new(r), span_of(ts)));
        }
        self.expr_add(ts)
    }

    fn expr_add(&mut self, ts: &[Token]) -> Option<Expr> {
        // `top_pos_last` never answers the first token, so the left side is never empty.
        if let Some(p) = top_pos_last(ts, |k| matches!(k, Kind::Plus | Kind::Minus)) {
            let op = if ts[p].is(&Kind::Plus) { BinOp::Add } else { BinOp::Sub };
            if p + 1 == ts.len() {
                return self.missing_side(&ts[p], false);
            }
            let l = self.expr_add(&ts[..p])?;
            let r = self.expr_mul(&ts[p + 1..])?;
            return Some(Expr::Bin(Box::new(l), op, Box::new(r), span_of(ts)));
        }
        self.expr_mul(ts)
    }

    fn expr_mul(&mut self, ts: &[Token]) -> Option<Expr> {
        if let Some(p) = top_pos_last(ts, |k| matches!(k, Kind::Star | Kind::Slash)) {
            let op = if ts[p].is(&Kind::Star) { BinOp::Mul } else { BinOp::Div };
            if p + 1 == ts.len() {
                return self.missing_side(&ts[p], false);
            }
            let l = self.expr_mul(&ts[..p])?;
            let r = self.expr_atom(&ts[p + 1..])?;
            return Some(Expr::Bin(Box::new(l), op, Box::new(r), span_of(ts)));
        }
        self.expr_atom(ts)
    }

    /// E059 for what is left after a complete value: `金額 税`, `(x + 1) y`, `x -1`.
    fn trailing(&mut self, extra: &[Token]) -> Option<Expr> {
        let mut d = bad_expr(
            extra,
            tr!("ここから先を式として読めません", "this is not read as part of the expression"),
            self.at(extra[0].span.line),
        )
        .note(tr!(
            "値が二つ、演算子を挟まずに並んでいます。`+` か `×` が抜けていないか見てください。",
            "Two values stand side by side with no operator between them. Look for a missing `+` or `×`."
        ));
        if let Kind::Num(n) = &extra[0].kind
            && n.neg
        {
            let abs = n.raw.trim_start_matches(['-', '−']);
            d = d.note(tr!(
                "`{}` は、記号と数字がくっついているので負の数として読まれています。引くのなら `- {abs}` のように間を空けてください。",
                "`{}` is read as a negative number, the sign touching the digits. To subtract, leave a space: `- {abs}`.",
                n.raw
            ));
        }
        self.err(d);
        None
    }

    fn expr_atom(&mut self, ts: &[Token]) -> Option<Expr> {
        // An empty slice only arrives here from `()`, which the branch below reports.
        let first = ts.first()?;
        if first.is(&Kind::LParen) {
            let Some(close) = matching(ts, 0) else {
                self.err(bad_expr(std::slice::from_ref(first), tr!("この `(` が閉じていません", "this `(` is never closed"), self.at(first.span.line)));
                return None;
            };
            if close == 1 {
                self.err(bad_expr(&ts[..=1], tr!("括弧の中が空です", "there is nothing between the parentheses"), self.at(first.span.line)));
                return None;
            }
            if close + 1 < ts.len() {
                return self.trailing(&ts[close + 1..]);
            }
            return self.expr(&ts[1..close]);
        }
        if let Some(name) = first.ident() {
            if ts.get(1).is_some_and(|t| t.is(&Kind::LParen)) {
                let Some(close) = matching(ts, 1) else {
                    self.err(bad_expr(&ts[1..=1], tr!("この `(` が閉じていません", "this `(` is never closed"), self.at(ts[1].span.line)));
                    return None;
                };
                if close + 1 < ts.len() {
                    return self.trailing(&ts[close + 1..]);
                }
                let mut args = Vec::new();
                for part in split_top(&ts[2..close], Kind::Comma) {
                    if part.is_empty() {
                        self.err(bad_expr(&ts[1..=close], tr!("引数が一つ抜けています", "an argument is missing here"), self.at(ts[1].span.line)));
                        return None;
                    }
                    args.push(self.expr(&part)?);
                }
                return Some(Expr::Call(name.to_string(), args, span_of(ts)));
            }
            if ts.len() > 1 {
                return self.trailing(&ts[1..]);
            }
            return Some(Expr::Name(name.to_string(), first.span.clone()));
        }
        match lit_of(first) {
            Some(_) if ts.len() > 1 => self.trailing(&ts[1..]),
            Some(l) => Some(Expr::Lit(l, first.span.clone())),
            None => {
                let what = match op_text(&first.kind) {
                    "" => tr!("これは式の中に書けません", "this cannot appear in an expression"),
                    o => tr!("`{o}` は、ここには書けません", "`{o}` cannot stand here"),
                };
                self.err(bad_expr(std::slice::from_ref(first), what, self.at(first.span.line)));
                None
            }
        }
    }
}

/// How an operator or a mark is written, for a diagnostic that names it.
fn op_text(k: &Kind) -> &'static str {
    match k {
        Kind::Plus => "+",
        Kind::Minus => "-",
        Kind::Star => "×",
        Kind::Slash => "÷",
        Kind::Le => "<=",
        Kind::Ge => ">=",
        Kind::Lt => "<",
        Kind::Gt => ">",
        Kind::Eq => "=",
        Kind::Arrow => "->",
        Kind::Comma => ",",
        Kind::LParen => "(",
        Kind::RParen => ")",
        Kind::LBracket => "[",
        Kind::RBracket => "]",
        Kind::Question => "?",
        Kind::Pipe => "|",
        Kind::Colon => ":",
        Kind::At => "@",
        Kind::Dot => ".",
        Kind::DotDot => "..",
        _ => "",
    }
}

/// E059: an expression that cannot be read to its end. Reading what could be read and
/// dropping the rest would check an expression other than the one written.
fn bad_expr(at_tokens: &[Token], what: String, at: String) -> Diag {
    Diag::error("E059", tr!("式を読めません", "The expression cannot be read"))
        .at(at)
        .mark(span_of(at_tokens), what)
        .note(tr!(
            "式に書けるのは、名前、数と金額、`+` `-` `×` `÷`、比較、括弧、`min(a, b)` のような呼び出しです。",
            "An expression is made of names, numbers and amounts, `+` `-` `×` `÷`, comparisons, parentheses, and calls such as `min(a, b)`."
        ))
}

fn lit_of(t: &Token) -> Option<Lit> {
    match &t.kind {
        Kind::Num(n) => Some(Lit::Num(n.clone())),
        Kind::Ident(w) => Some(Lit::Word(w.clone())),
        Kind::Date(y, m, d) => Some(Lit::Date(*y, *m, *d)),
        Kind::Str(s) => Some(Lit::Str(s.clone())),
        _ => None,
    }
}

fn lits(ts: &[Token]) -> Vec<Lit> {
    ts.iter().filter(|t| !t.is(&Kind::Comma)).filter_map(lit_of).collect()
}

/// Cells of a `|`-delimited row, without the bars. An empty cell keeps the gap
/// between its bars as its span, so E008 can point at the cell and not the row.
fn split_cells(ts: &[Token]) -> Vec<(Vec<Token>, Span)> {
    let mut out = Vec::new();
    let mut cur: Vec<Token> = Vec::new();
    let mut open: Option<&Token> = None;
    for t in ts {
        if t.is(&Kind::Pipe) {
            if let Some(o) = open {
                let span = if cur.is_empty() {
                    let from = o.span.col + o.span.len;
                    Span::new(o.span.line, from, (t.span.col - from).max(1))
                } else {
                    span_of(&cur)
                };
                out.push((std::mem::take(&mut cur), span));
            }
            open = Some(t);
            continue;
        }
        if open.is_some() {
            cur.push(t.clone());
        }
    }
    out
}

fn depth_scan(ts: &[Token]) -> Vec<i32> {
    let mut d = 0;
    ts.iter()
        .map(|t| {
            if t.is(&Kind::LParen) {
                d += 1;
                d
            } else if t.is(&Kind::RParen) {
                let cur = d;
                d -= 1;
                cur
            } else {
                d
            }
        })
        .collect()
}

fn top_pos(ts: &[Token], f: impl Fn(&Kind) -> bool) -> Option<usize> {
    let d = depth_scan(ts);
    (0..ts.len()).find(|&i| d[i] == 0 && f(&ts[i].kind))
}

fn top_pos_last(ts: &[Token], f: impl Fn(&Kind) -> bool) -> Option<usize> {
    let d = depth_scan(ts);
    (0..ts.len()).rev().find(|&i| d[i] == 0 && f(&ts[i].kind) && i != 0)
}

fn matching(ts: &[Token], open: usize) -> Option<usize> {
    let mut d = 0;
    for i in open..ts.len() {
        if ts[i].is(&Kind::LParen) {
            d += 1;
        } else if ts[i].is(&Kind::RParen) {
            d -= 1;
            if d == 0 {
                return Some(i);
            }
        }
    }
    None
}

fn split_top(ts: &[Token], sep: Kind) -> Vec<Vec<Token>> {
    let d = depth_scan(ts);
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for (i, t) in ts.iter().enumerate() {
        if d[i] == 0 && t.kind == sep {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(t.clone());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

