//! Line-oriented parser for `.rule` (§1.1, §1.2).
//!
//! The file structure is fixed, so this is a dispatch on the first word of each line
//! rather than a general grammar. Tables are runs of `|` lines.

use crate::ast::*;
use crate::diag::{Diag, Span};
use crate::lex::{Kind, Token, lex_line};

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
        match lex_line(n + 1, text) {
            Ok(t) => lines.push(t),
            Err(d) => {
                diags.push(d);
                lines.push(Vec::new());
            }
        }
    }
    let mut p = P { lines, i: 0, diags, path: path.to_string(), ctx: String::new() };
    let file = p.rule_file();
    Parsed { file, diags: p.diags }
}

/// Span covering a whole token run.
fn span_of(ts: &[Token]) -> Span {
    match (ts.first(), ts.last()) {
        (Some(a), Some(b)) => Span::new(a.span.line, a.span.col, b.span.col + b.span.len - a.span.col),
        _ => Span::new(1, 0, 1),
    }
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
        let (name, mut k) = self.name_at(&head, 1)?;
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
            enums: Vec::new(),
            groups: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            items: Vec::new(),
            result: None,
            examples: None,
            constraints: Vec::new(),
            elements: None,
            sequences: Vec::new(),
            fold: None,
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
            match word.as_str() {
                crate::kw::DESCRIPTION => {
                    if let Some(Kind::Str(s)) = line.get(1).map(|t| t.kind.clone()) {
                        f.description = Some(s);
                    }
                    self.i += 1;
                }
                crate::kw::IMPORT => {
                    let path: String = line[1..]
                        .iter()
                        .filter_map(|t| t.ident())
                        .collect::<Vec<_>>()
                        .join("/");
                    f.imports.push((path, span_of(&line)));
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
                    f.inputs.extend(self.var_block());
                }
                crate::kw::OUTPUTS => {
                    self.i += 1;
                    f.outputs.extend(self.out_block());
                }
                crate::kw::DERIVE => {
                    if let Some(d) = self.derived(&line) {
                        f.items.push(Item::Derived(d));
                    }
                }
                crate::kw::DEFINE => {
                    if let Some(d) = self.define(&line) {
                        f.items.push(Item::Define(d));
                    }
                    self.i += 1;
                }
                crate::kw::ELEMENTS => {
                    let name = self.name_at(&line, 1).map(|(n, _)| n);
                    let span = span_of(&line);
                    self.i += 1;
                    let fields = self.var_block();
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
                    if let Some(t) = self.table(&line) {
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
                    let (cols, outs, rows) = self.grid().unwrap_or_default();
                    self.ctx.clear();
                    if let Some(o) = outs.first() {
                        self.err(
                            Diag::error("E026", tr!("`sequence` に出力の列は書けません", "A `sequence` has no output column"))
                                .at(self.at(o.span.line))
                                .mark(o.span.clone(), tr!("`->` があります", "there is a `->` here"))
                                .note(tr!(
                                    "これは値の並びであって表ではありません。欄は `elements` の欄だけです。",
                                    "This is a list of values, not a table: its columns are the fields of `elements` and nothing else."
                                )),
                        );
                    }
                    if let Some(name) = name {
                        f.sequences.push(crate::ast::SeqDecl { name, cols, rows, span });
                    }
                }
                crate::kw::EXAMPLES => {
                    self.i += 1;
                    f.examples = self.example_table();
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
        }
        Some(f)
    }

    /// `名前(ascii)` starting at token `k`. Returns the name and the next index.
    fn name_at(&mut self, ts: &[Token], k: usize) -> Option<(Name, usize)> {
        let t = ts.get(k)?;
        let text = t.ident()?.to_string();
        let mut j = k + 1;
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
        let (name, mut k) = self.name_at(line, 1)?;
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

    fn group_decl(&mut self, line: &[Token]) -> Option<GroupDecl> {
        let (name, mut k) = self.name_at(line, 1)?;
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
            let (v, nk) = self.name_at(line, k)?;
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

    fn var_block(&mut self) -> Vec<VarDecl> {
        let mut v = Vec::new();
        for line in self.block_lines() {
            let Some((name, k)) = self.name_at(&line, 0) else { continue };
            let Some((ty, k)) = self.type_ref(&line, k) else { continue };
            let (range, contract_only) = self.tail_range(&line, k);
            v.push(VarDecl { name, ty, range, contract_only, span: span_of(&line) });
        }
        v
    }

    fn out_block(&mut self) -> Vec<OutDecl> {
        let mut v = Vec::new();
        for line in self.block_lines() {
            let Some((name, k)) = self.name_at(&line, 0) else { continue };
            let Some((ty, k)) = self.type_ref(&line, k) else { continue };
            let rounding = self.tail_rounding(&line, k);
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
        let (name, k) = self.name_at(line, 1)?;
        let (ty, k) = self.type_ref(line, k)?;
        let eq = line.iter().position(|t| t.is(&Kind::Eq))?;
        // The expression runs to `range` on the same line, if present.
        let stop = line[eq + 1..]
            .iter()
            .position(|t| t.ident() == Some(crate::kw::RANGE))
            .map(|p| eq + 1 + p)
            .unwrap_or(line.len());
        let expr = self.expr(&line[eq + 1..stop])?;
        let (mut range, _) = self.tail_range(line, k.max(eq));
        self.i += 1;
        // §11's E112 example puts `range` on the next line; accept both.
        if range.is_none()
            && self.cur().is_some_and(|l| l.first().and_then(|t| t.ident()) == Some(crate::kw::RANGE))
        {
            let cont = self.cur().cloned().unwrap();
            let (r, _) = self.tail_range(&cont, 0);
            range = r;
            self.i += 1;
        }
        Some(DerivedDecl { name, ty, expr, range, span: span_of(line) })
    }

    fn define(&mut self, line: &[Token]) -> Option<DefineDecl> {
        let (name, k) = self.name_at(line, 1)?;
        let (ty, _k) = self.type_ref(line, k)?;
        let eq = line.iter().position(|t| t.is(&Kind::Eq))?;
        let expr = self.expr(&line[eq + 1..])?;
        Some(DefineDecl { name, ty, expr, span: span_of(line) })
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
                crate::kw::EMPTY => empty = self.expr(rest),
                crate::kw::EXHAUSTED => exhausted = self.expr(rest),
                _ => {
                    let arm = match word {
                        crate::kw::NEXT => Some(Arm::Next),
                        crate::kw::STOP => {
                            if rest.get(1).and_then(|t| t.ident()) == Some(crate::kw::WITH) {
                                self.expr(&rest[2..]).map(|e| Arm::Stop(Some(e)))
                            } else {
                                Some(Arm::Stop(None))
                            }
                        }
                        crate::kw::TAKE_UNIQUE | crate::kw::TAKE_FIRST => self
                            .expr(&rest[1..])
                            .map(|e| Arm::Take { expr: e, unique: word == crate::kw::TAKE_UNIQUE }),
                        crate::kw::KEEP_MAX => {
                            let by = rest.iter().position(|t| t.ident() == Some(crate::kw::BY));
                            match by {
                                Some(b) => match (self.expr(&rest[1..b]), self.expr(&rest[b + 1..])) {
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
        let name = line.get(1)?.ident()?.to_string();
        let eq = line.iter().position(|t| t.is(&Kind::Eq))?;
        let expr = self.expr(&line[eq + 1..])?;
        Some(ResultDecl { name, expr, span: span_of(line) })
    }

    fn table(&mut self, head: &[Token]) -> Option<Table> {
        let name = self.name_at(head, 1).map(|(n, _)| n);
        let span = span_of(head);
        self.i += 1;
        let mut policy = Policy::Unique; // §4: the default is unique.
        if self.cur().is_some_and(|l| l.first().and_then(|t| t.ident()) == Some(crate::kw::POLICY)) {
            let l = self.cur().cloned().unwrap();
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
        self.ctx = match &name {
            Some(n) => tr!("表 {}", "table {}", n.text),
            None => String::new(),
        };
        let (inputs, outputs, rows) = self.grid()?;
        self.ctx.clear();
        Some(Table { name, policy, inputs, outputs, rows, span })
    }

    fn example_table(&mut self) -> Option<Table> {
        self.ctx = tr!("例", "examples");
        let (inputs, outputs, rows) = self.grid()?;
        self.ctx.clear();
        Some(Table {
            name: None,
            policy: Policy::Unique,
            inputs,
            outputs,
            rows,
            span: Span::new(1, 0, 1),
        })
    }

    /// The `|` block: one header row, then rule rows.
    fn grid(&mut self) -> Option<(Vec<(String, Span)>, Vec<OutCol>, Vec<Row>)> {
        let mut raw_rows: Vec<Vec<Token>> = Vec::new();
        while self
            .cur()
            .is_some_and(|l| l.first().is_some_and(|t| t.is(&Kind::Pipe)))
        {
            raw_rows.push(self.cur().cloned().unwrap());
            self.i += 1;
        }
        if raw_rows.is_empty() {
            return None;
        }
        let header = split_cells(&raw_rows[0]);
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
                    None => continue,
                };
                let ty = self.type_ref(cell, k).map(|(t, _)| t);
                outputs.push(OutCol { name: nm, ty, span: span_of(cell) });
            } else if let Some(w) = cell.first().and_then(|t| t.ident()) {
                inputs.push((w.to_string(), span_of(cell)));
            }
        }
        let ncol = inputs.len();
        let mut rows = Vec::new();
        for (n, rt) in raw_rows[1..].iter().enumerate() {
            let cells_t = split_cells(rt);
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
                    continue;
                }
                if ci < ncol {
                    match self.cell(ct) {
                        Some(c) => {
                            cells.push(c);
                            cell_spans.push(cspan.clone());
                        }
                        None => continue,
                    }
                } else {
                    outs.push(self.out_cell(ct));
                    out_spans.push(cspan.clone());
                }
            }
            rows.push(Row { cells, cell_spans, outs, out_spans, span: span_of(rt), index: n + 1 });
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
            return Some(Cell::Nothing);
        }
        if ts[0].ident() == Some(crate::kw::NOT) {
            let rest = if ts.get(1).is_some_and(|t| t.is(&Kind::Colon)) { &ts[2..] } else { &ts[1..] };
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
            return Some(Cell::Cmp(v));
        }
        if ts.iter().any(|t| t.is(&Kind::Comma)) {
            return Some(Cell::Set(lits(ts)));
        }
        lit_of(&ts[0]).map(Cell::Lit)
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
            None => OutCell::Name(String::new()),
        }
    }

    /// Precedence: comparison < additive < multiplicative.
    fn expr(&mut self, ts: &[Token]) -> Option<Expr> {
        if ts.is_empty() {
            return None;
        }
        self.expr_cmp(ts)
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
            let l = self.expr_add(&ts[..p])?;
            let r = self.expr_add(&ts[p + 1..])?;
            return Some(Expr::Bin(Box::new(l), op, Box::new(r), span_of(ts)));
        }
        self.expr_add(ts)
    }

    fn expr_add(&mut self, ts: &[Token]) -> Option<Expr> {
        if let Some(p) = top_pos_last(ts, |k| matches!(k, Kind::Plus | Kind::Minus)) {
            let op = if ts[p].is(&Kind::Plus) { BinOp::Add } else { BinOp::Sub };
            let l = self.expr_add(&ts[..p])?;
            let r = self.expr_mul(&ts[p + 1..])?;
            return Some(Expr::Bin(Box::new(l), op, Box::new(r), span_of(ts)));
        }
        self.expr_mul(ts)
    }

    fn expr_mul(&mut self, ts: &[Token]) -> Option<Expr> {
        if let Some(p) = top_pos_last(ts, |k| matches!(k, Kind::Star | Kind::Slash)) {
            let op = if ts[p].is(&Kind::Star) { BinOp::Mul } else { BinOp::Div };
            let l = self.expr_mul(&ts[..p])?;
            let r = self.expr_atom(&ts[p + 1..])?;
            return Some(Expr::Bin(Box::new(l), op, Box::new(r), span_of(ts)));
        }
        self.expr_atom(ts)
    }

    fn expr_atom(&mut self, ts: &[Token]) -> Option<Expr> {
        if ts.is_empty() {
            return None;
        }
        if ts[0].is(&Kind::LParen) {
            let close = matching(ts, 0)?;
            return self.expr(&ts[1..close]);
        }
        if let Some(name) = ts[0].ident() {
            if ts.get(1).is_some_and(|t| t.is(&Kind::LParen)) {
                let close = matching(ts, 1)?;
                let mut args = Vec::new();
                for part in split_top(&ts[2..close], Kind::Comma) {
                    if let Some(e) = self.expr(&part) {
                        args.push(e);
                    }
                }
                return Some(Expr::Call(name.to_string(), args, span_of(ts)));
            }
            return Some(Expr::Name(name.to_string(), ts[0].span.clone()));
        }
        lit_of(&ts[0]).map(|l| Expr::Lit(l, ts[0].span.clone()))
    }
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

