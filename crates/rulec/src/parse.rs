//! Line-oriented parser for `.rule` (§1.1, §1.2).
//!
//! The file structure is fixed, so this is a dispatch on the first word of each line
//! rather than a general grammar. Tables are runs of `|` lines.

use crate::ast::*;
use crate::diag::{Diag, Span};
use crate::lex::{Kind, Num, Token, lex_line};

pub struct Parsed {
    pub file: Option<RuleFile>,
    pub diags: Vec<Diag>,
}

struct P {
    lines: Vec<Vec<Token>>,
    i: usize,
    diags: Vec<Diag>,
    path: String,
    /// §11 原則 4: 位置は ファイル:行 に加えて表名で示す。
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
    fn head(&self) -> Option<&str> {
        self.cur()?.first()?.ident()
    }
    fn err(&mut self, d: Diag) {
        self.diags.push(d);
    }

    /// `path:line 表 名前` — §11 原則 4。
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
        if head.first().and_then(|t| t.ident()) != Some("規則") {
            self.err(
                Diag::error("E003", "ファイルは `規則` の行で始まらなければなりません")
                    .mark(span_of(&head), "ここに `規則 <名前>(<ascii>) v<版>` が要ります"),
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
        };

        loop {
            self.skip_blank();
            let Some(line) = self.cur().cloned() else { break };
            let Some(word) = line.first().and_then(|t| t.ident()).map(|s| s.to_string()) else {
                self.err(
                    Diag::error("E004", "行の先頭に語がありません")
                        .mark(span_of(&line), ""),
                );
                self.i += 1;
                continue;
            };
            match word.as_str() {
                "説明" => {
                    if let Some(Kind::Str(s)) = line.get(1).map(|t| t.kind.clone()) {
                        f.description = Some(s);
                    }
                    self.i += 1;
                }
                "取込" => {
                    let path: String = line[1..]
                        .iter()
                        .filter_map(|t| t.ident())
                        .collect::<Vec<_>>()
                        .join("/");
                    f.imports.push((path, span_of(&line)));
                    self.i += 1;
                }
                "型" => {
                    if let Some(e) = self.enum_decl(&line) {
                        f.enums.push(e);
                    }
                    self.i += 1;
                }
                "群" => {
                    if let Some(g) = self.group_decl(&line) {
                        f.groups.push(g);
                    }
                    self.i += 1;
                }
                "入力" => {
                    self.i += 1;
                    f.inputs.extend(self.var_block());
                }
                "出力" => {
                    self.i += 1;
                    f.outputs.extend(self.out_block());
                }
                "導出" => {
                    if let Some(d) = self.derived(&line) {
                        f.items.push(Item::Derived(d));
                    }
                }
                "定義" => {
                    if let Some(d) = self.define(&line) {
                        f.items.push(Item::Define(d));
                    }
                    self.i += 1;
                }
                "表" => {
                    if let Some(t) = self.table(&line) {
                        f.items.push(Item::Table(t));
                    }
                }
                "結果" => {
                    if let Some(r) = self.result(&line) {
                        f.result = Some(r);
                    }
                    self.i += 1;
                }
                "例" => {
                    self.i += 1;
                    f.examples = self.example_table();
                }
                other => {
                    self.err(
                        Diag::error("E005", format!("`{other}` はこの位置で知らない語です"))
                            .mark(line[0].span.clone(), "")
                            .note("書けるのは 説明 / 取込 / 型 / 群 / 入力 / 出力 / 導出 / 定義 / 表 / 結果 / 例 です"),
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
            self.err(Diag::error("E006", "型の宣言に `=` がありません").mark(span_of(line), ""));
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
            let marked = line.get(k).and_then(|t| t.ident()) == Some("既定扱い");
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
            self.err(Diag::error("E006", "群の宣言に `=` がありません").mark(span_of(line), ""));
            return None;
        }
        k += 1;
        let mut members = Vec::new();
        while k < line.len() {
            if line[k].is(&Kind::Sep) {
                k += 1;
                continue;
            }
            let (v, nk) = self.name_at(line, k)?;
            members.push(v);
            k = nk;
        }
        Some(GroupDecl { name, members, span: span_of(line) })
    }

    /// An indented run of declarations under `入力` / `出力`.
    fn block_lines(&mut self) -> Vec<Vec<Token>> {
        let mut out = Vec::new();
        loop {
            let Some(line) = self.cur() else { break };
            if line.is_empty() {
                break;
            }
            let w = line.first().and_then(|t| t.ident()).unwrap_or("");
            if matches!(
                w,
                "説明" | "取込" | "型" | "群" | "入力" | "出力" | "導出" | "定義" | "表" | "結果" | "例" | "方式"
            ) {
                // 宣言の形（`名前(別名) :` か `名前 :`）をしているなら、それは
                // セクションの始まりではなく、キーワードと同じ名前の宣言である。
                // 黙って捨てると生成の段で初めて壊れるので、ここで言う。
                let decl = line
                    .get(1)
                    .is_some_and(|t| t.is(&Kind::LParen) || t.is(&Kind::Colon));
                if decl {
                    let sp = line[0].span.clone();
                    let at = self.at(sp.line);
                    self.err(
                        Diag::error("E009", format!("`{w}` はキーワードなので、名前にできません"))
                            .at(at)
                            .mark(sp, "")
                            .note("行指向の構文なので、キーワードと同じ名前は宣言をセクションの始まりに見せてしまいます。")
                            .note("別の名前を付けてください。"),
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

    /// `: 金額[円, 税込]` / `: 質量[g]` / `: 率[刻み 0.1%]` / `: 都道府県`
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
        // 位置は `]` まで伸ばす。§11 の E104 は型の全体に下線を引く。
        let end = ts.get(k.saturating_sub(1)).map(|t| t.span.col + t.span.len).unwrap_or(start.col + start.len);
        let span = Span::new(start.line, start.col, end.saturating_sub(start.col).max(start.len));
        Some((TypeRef { base, args, optional, span }, k))
    }

    /// `範囲 >=1g <=40kg` and the `契約のみ` marker (§11 W111).
    fn tail_range(&mut self, ts: &[Token], mut k: usize) -> (Option<Range>, bool) {
        let mut range = None;
        let mut contract_only = false;
        while k < ts.len() {
            match ts[k].ident() {
                Some("範囲") => {
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
                Some("契約のみ") => {
                    contract_only = true;
                    k += 1;
                }
                _ => k += 1,
            }
        }
        (range, contract_only)
    }

    /// `丸め 切り上げ(10円)`
    fn tail_rounding(&mut self, ts: &[Token], mut k: usize) -> Option<Rounding> {
        while k < ts.len() {
            if ts[k].ident() == Some("丸め") {
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
        // The expression runs to `範囲` on the same line, if present.
        let stop = line[eq + 1..]
            .iter()
            .position(|t| t.ident() == Some("範囲"))
            .map(|p| eq + 1 + p)
            .unwrap_or(line.len());
        let expr = self.expr(&line[eq + 1..stop])?;
        let (mut range, _) = self.tail_range(line, k.max(eq));
        self.i += 1;
        // §11's E112 example puts `範囲` on the next line; accept both.
        if range.is_none()
            && self.cur().is_some_and(|l| l.first().and_then(|t| t.ident()) == Some("範囲"))
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
        let mut policy = Policy::Unique; // §4: the default is 一意.
        if self.cur().is_some_and(|l| l.first().and_then(|t| t.ident()) == Some("方式")) {
            let l = self.cur().cloned().unwrap();
            match l.get(1).and_then(|t| t.ident()) {
                Some("一意") => policy = Policy::Unique,
                Some("上から") => policy = Policy::TopDown,
                other => self.err(
                    Diag::error("E007", format!("知らない方式 `{}` です", other.unwrap_or("")))
                        .mark(span_of(&l), "")
                        .note("書けるのは 一意 と 上から です（§4）"),
                ),
            }
            self.i += 1;
        }
        self.ctx = match &name {
            Some(n) => format!("表 {}", n.text),
            None => String::new(),
        };
        let (inputs, outputs, rows) = self.grid()?;
        self.ctx.clear();
        Some(Table { name, policy, inputs, outputs, rows, span })
    }

    fn example_table(&mut self) -> Option<Table> {
        self.ctx = "例".into();
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
        // `→` は入力と出力の境目を一度だけ示す。以降の列は `→` を書いても書かなくても
        // 出力である。二本目に `→` を書き忘れた列を黙って入力に化けさせないため
        // （複数出力の 例 でその素通りを踏んだ）。
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
                        .map(|n| format!("列 {n} が空です"))
                        .unwrap_or_else(|| "空です".into());
                    let at = self.at(rt[0].span.line);
                    self.err(
                        Diag::error("E008", "空のセルがあります")
                            .at(at)
                            .mark(cspan.clone(), col)
                            .note("任意の値に当てるなら `-` と書いてください（空欄は書き忘れと区別がつきません。§3）"),
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
                Diag::error("E010", "`..` は書けません")
                    .at(at)
                    .mark(t.span.clone(), "")
                    .note("`<=2000g` と `>2000g` のどちらの意味かが読めないためです（§3.1）。比較演算子で書いてください"),
            );
            return None;
        }
        if ts.len() == 1 && ts[0].is(&Kind::Minus) {
            return Some(Cell::DontCare);
        }
        if ts[0].ident() == Some("無し") {
            return Some(Cell::Nothing);
        }
        if ts[0].ident() == Some("以外") {
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
        if ts.iter().any(|t| t.is(&Kind::Sep)) {
            return Some(Cell::Set(lits(ts)));
        }
        lit_of(&ts[0]).map(Cell::Lit)
    }

    fn out_cell(&mut self, ts: &[Token]) -> OutCell {
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
    ts.iter().filter(|t| !t.is(&Kind::Sep)).filter_map(lit_of).collect()
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

