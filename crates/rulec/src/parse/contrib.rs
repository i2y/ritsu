//! The lines of the merging of files (§15.227–§15.229): the `open` value of a rule and the terms
//! under it, and the lines only a `contribution` file holds — `to`, `after`, `extend`, `into` and
//! the `prerequisites` lines it adds. Every word is read by its place in the line, so none is
//! reserved (§15.157): `open` and `combine` where an `open` line starts and goes on, `to`, `after`,
//! `into` and `extend` where a line of a contribution file starts.
//!
//! ```text
//! open stackable : money[USD]  combine sum  range >=0USD <=100USD
//!   member_off
//!   channel_off
//!
//! contribution regional_campaign v1
//! to "../../order_discount.rule" signature:fe7307da2694d75a
//! extend channel with kiosk "Kiosk"
//! into stackable
//!   campaign_off
//! ```

use super::{P, span_of};
use crate::ast::{Combine, ContribHead, ExtendDecl, IntoDecl, Name, OpenDecl, RequiresLine, Target, Term};
use crate::diag::{Diag, Span};
use crate::lex::{Kind, Token};

/// How an `open` line is written, for the notes of E068.
fn open_shape() -> String {
    tr!(
        "形は `open <名前>(<別名>) : <型> combine sum|product|max|min [empty <値>] range >=… <=…` の行と、その下に一行に一つずつの項の名前です。",
        "The shape is `open <name>(<alias>) : <type> combine sum|product|max|min [empty <value>] range >=… <=…`, with the names of its terms under it, one to a line."
    )
}

/// How the lines of a contribution are written, for the notes of E067.
pub(super) fn contrib_shape() -> String {
    tr!(
        "寄与のファイルは `contribution <名前>(<別名>) v<版>` で始め、`to \"<元のファイル>\" signature:<16 桁>` で足す先を言い、`after \"<寄与のファイル>\" signature:<16 桁>`、`inputs`、`extend <列挙> with <値>`、表・節・定義・導出、`into <開いた値>` と項、`prerequisites <塊>` と `<値> requires …`、`examples` を書きます。",
        "A contribution file starts with `contribution <name>(<alias>) v<version>`, says where it adds with `to \"<file>\" signature:<16 hex>`, and holds `after \"<contribution>\" signature:<16 hex>`, `inputs`, `extend <enum> with <value>`, tables, clauses, definitions and derives, `into <open value>` with its terms, `prerequisites <block>` with `<value> requires …` lines, and `examples`."
    )
}

impl P {
    /// The names under an `open` or an `into` line, one to a line, up to a blank line or a line
    /// that starts another section. A name may join an apply's or a contribution's prefix with `:`.
    fn term_lines(&mut self) -> Vec<(Vec<Token>, Option<Name>)> {
        let mut out = Vec::new();
        while let Some(line) = self.cur().cloned() {
            if line.is_empty() || line.first().is_some_and(|t| t.is(&Kind::Pipe)) {
                break;
            }
            let w = line.first().and_then(|t| t.ident()).unwrap_or("");
            if (super::KEYWORDS.contains(&w) && !super::named_like_head(&line)) || crate::kw::CONTRIBUTION_HEAD.contains(&w) {
                break;
            }
            self.i += 1;
            let got = self.name_at(&line, 0).filter(|(_, k)| *k == line.len()).map(|(n, _)| n);
            out.push((line, got));
        }
        out
    }

    /// `open <name>(<alias>) : <type> combine <how> [empty <value>] range …` and the terms under it
    /// (§15.227). What is missing from the line (the way of combining, the `empty` of `max` and
    /// `min`, the range) is the check's to say (E068), so that the reason can be given with the
    /// type known; what cannot be read at all is said here.
    pub(super) fn open_decl(&mut self, line: &[Token], at: usize) -> Option<OpenDecl> {
        let span = span_of(line);
        self.i += 1;
        let lines = self.term_lines();
        let bad = |p: &mut Self, sp: Span, what: String| {
            p.err(
                Diag::error("E068", tr!("`open` の行を読めません", "The `open` line cannot be read"))
                    .at(p.at(sp.line))
                    .mark(sp, what)
                    .note(open_shape()),
            );
        };
        let mut terms = Vec::new();
        let mut ok = true;
        for (l, got) in lines {
            match got {
                Some(n) => terms.push(Term { name: n, from: None }),
                None => {
                    bad(self, span_of(&l), tr!("項は一行に一つの名前です", "a term is one name to a line"));
                    ok = false;
                }
            }
        }
        let Some((name, k)) = self.name_at(line, 1) else {
            bad(self, span.clone(), tr!("`open` の後に名前がありません", "there is no name after `open`"));
            return None;
        };
        let Some((ty, mut k)) = self.type_ref(line, k).filter(|_| line.get(k).is_some_and(|t| t.is(&Kind::Colon))) else {
            bad(self, name.span.clone(), tr!("{} の型が書かれていません", "{} is not given a type", name.text));
            return None;
        };
        let mut combine = None;
        let mut empty = None;
        let mut range = None;
        while k < line.len() {
            if line[k].is(&Kind::At) {
                break;
            }
            match line[k].ident() {
                Some(crate::kw::COMBINE) => {
                    let how = line.get(k + 1).and_then(|t| t.ident()).and_then(Combine::parse);
                    match how {
                        Some(c) => combine = Some((c, span_of(&line[k..k + 2]))),
                        None => {
                            let sp = line.get(k + 1).map(|t| t.span.clone()).unwrap_or_else(|| line[k].span.clone());
                            self.err(
                                Diag::error("E068", tr!("合わせ方が読めません", "The way of combining cannot be read"))
                                    .at(self.at(sp.line))
                                    .mark(sp, tr!("`sum`・`product`・`max`・`min` のどれかです", "it is one of `sum`, `product`, `max` and `min`"))
                                    .note(open_shape()),
                            );
                            return None;
                        }
                    }
                    k += 2;
                }
                Some(crate::kw::EMPTY) => {
                    match line.get(k + 1).and_then(super::lit_of) {
                        Some(l) => empty = Some((l, line[k + 1].span.clone())),
                        None => {
                            bad(self, line[k].span.clone(), tr!("`empty` の後に値がありません", "there is no value after `empty`"));
                            return None;
                        }
                    }
                    k += 2;
                }
                Some(crate::kw::RANGE) => {
                    let (r, _) = self.tail_range(line, k);
                    range = r;
                    // `tail_range` reads the pairs it can; what is left after them is not read.
                    let mut j = k + 1;
                    while j + 1 < line.len() && matches!(line[j].kind, Kind::Le | Kind::Ge | Kind::Lt | Kind::Gt) && super::lit_of(&line[j + 1]).is_some() {
                        j += 2;
                    }
                    k = j;
                }
                _ => {
                    bad(self, line[k].span.clone(), tr!("この語は `open` の行の一部として読まれません", "this is not read as part of the `open` line"));
                    return None;
                }
            }
        }
        if !ok {
            return None;
        }
        Some(OpenDecl { name, ty, combine, empty, range, terms, at, span })
    }

    /// `to "<file>" signature:<digest>` or `after "<file>" signature:<digest>` (§15.227, §15.230).
    /// The digest may be left out: E040 then says what to pin, as for an `apply`.
    pub(super) fn target_line(&mut self, line: &[Token], word: &str) -> Option<Target> {
        self.i += 1;
        let span = span_of(line);
        let bad = |p: &mut Self, what: String| {
            p.err(
                Diag::error("E067", tr!("`{word}` の行の形が違います", "A `{word}` line is not shaped like this"))
                    .at(p.at(span.line))
                    .mark(span.clone(), what)
                    .note(contrib_shape()),
            );
        };
        let Some(Kind::Str(file)) = line.get(1).map(|t| t.kind.clone()) else {
            bad(self, tr!("ファイルを `\"…\"` で書いてください", "write the file in quotes"));
            return None;
        };
        let signature = match line.get(2).map(|t| t.kind.clone()) {
            None => None,
            Some(Kind::Signature(h)) if line.len() == 3 => Some(h),
            Some(_) => {
                bad(self, tr!("ファイルの後に書けるのは `signature:<16 桁>` だけです", "only `signature:<16 hex>` may follow the file"));
                return None;
            }
        };
        Some(Target { file, signature, span })
    }

    /// `extend <enum> with <value>(<alias>) ["<label>"] [after <value>] [default]` (§15.228).
    pub(super) fn extend_line(&mut self, line: &[Token]) -> Option<ExtendDecl> {
        self.i += 1;
        let span = span_of(line);
        let shape = tr!(
            "形は `extend <列挙> with <値>(<別名>) [\"<表示名>\"] [after <値>] [default]` です。",
            "The shape is `extend <enum> with <value>(<alias>) [\"<label>\"] [after <value>] [default]`."
        );
        let bad = |p: &mut Self, sp: Span, what: String| {
            p.err(Diag::error("E067", tr!("`extend` の行の形が違います", "An `extend` line is not shaped like this")).at(p.at(sp.line)).mark(sp, what).note(shape.clone()));
        };
        let Some((enum_name, k)) = self.name_at(line, 1) else {
            bad(self, span.clone(), tr!("`extend` の後に列挙の名前がありません", "there is no enum after `extend`"));
            return None;
        };
        if line.get(k).and_then(|t| t.ident()) != Some(crate::kw::WITH) {
            bad(self, span.clone(), tr!("列挙の名前の後に `with <値>` が要ります", "`with <value>` follows the enum"));
            return None;
        }
        let Some((value, mut k)) = self.name_at(line, k + 1) else {
            bad(self, span.clone(), tr!("`with` の後に値がありません", "there is no value after `with`"));
            return None;
        };
        let mut label = None;
        if let Some(Kind::Str(s)) = line.get(k).map(|t| t.kind.clone()) {
            label = Some((s, line[k].span.clone()));
            k += 1;
        }
        let mut after = None;
        let mut default = false;
        while k < line.len() {
            match line[k].ident() {
                Some(crate::kw::AFTER) if after.is_none() => match line.get(k + 1).and_then(|t| t.ident()) {
                    Some(w) => {
                        after = Some(Name { text: w.to_string(), ascii: None, span: line[k + 1].span.clone() });
                        k += 2;
                    }
                    None => {
                        bad(self, line[k].span.clone(), tr!("`after` の後に値がありません", "there is no value after `after`"));
                        return None;
                    }
                },
                Some(crate::kw::DEFAULT) if !default => {
                    default = true;
                    k += 1;
                }
                _ => {
                    bad(self, line[k].span.clone(), tr!("この語は読まれません", "this is not read"));
                    return None;
                }
            }
        }
        Some(ExtendDecl { enum_name, value, label, after, default, span })
    }

    /// `into <open value>` and the terms under it (§15.227).
    pub(super) fn into_decl(&mut self, line: &[Token]) -> Option<IntoDecl> {
        let span = span_of(line);
        self.i += 1;
        let lines = self.term_lines();
        let bad = |p: &mut Self, sp: Span, what: String| {
            p.err(Diag::error("E067", tr!("`into` の形が違います", "An `into` is not shaped like this")).at(p.at(sp.line)).mark(sp, what).note(contrib_shape()));
        };
        let mut terms = Vec::new();
        let mut ok = true;
        for (l, got) in lines {
            match got {
                Some(n) => terms.push(n),
                None => {
                    bad(self, span_of(&l), tr!("項は一行に一つの名前です", "a term is one name to a line"));
                    ok = false;
                }
            }
        }
        let target = match self.name_at(line, 1) {
            Some((n, k)) if k == line.len() => n,
            _ => {
                bad(self, span.clone(), tr!("`into` の後に、開いた値の名前を一つ書いてください", "write the name of one open value after `into`"));
                return None;
            }
        };
        if terms.is_empty() {
            bad(self, span.clone(), tr!("`into` の下に項がありません", "there is no term under `into`"));
            return None;
        }
        ok.then_some(IntoDecl { target, terms, span })
    }

    /// `prerequisites <block>` in a contribution, and the `<value> requires …` lines under it: the
    /// lines this file adds to an open block of the file it adds to (§15.229). `start` and `ordered
    /// by` belong to the block's own file.
    pub(super) fn contrib_prereq(&mut self, line: &[Token]) -> Option<(Name, Vec<RequiresLine>)> {
        let span = span_of(line);
        self.i += 1;
        let mut block: Vec<Vec<Token>> = Vec::new();
        while let Some(l) = self.cur() {
            if l.is_empty() || l.first().is_some_and(|t| t.is(&Kind::Pipe)) {
                break;
            }
            let w = l.first().and_then(|t| t.ident()).unwrap_or("");
            let own = l.get(1).and_then(|t| t.ident()) == Some(crate::kw::REQUIRES);
            if !own && (super::KEYWORDS.contains(&w) || crate::kw::CONTRIBUTION_HEAD.contains(&w)) {
                break;
            }
            block.push(l.clone());
            self.i += 1;
        }
        let bad = |p: &mut Self, sp: Span, what: String| {
            p.err(
                Diag::error("E067", tr!("寄与の `prerequisites` の形が違います", "A contribution's `prerequisites` is not shaped like this"))
                    .at(p.at(sp.line))
                    .mark(sp, what)
                    .note(tr!(
                        "寄与は、足す先の `open` の塊に、自分が足した値の `<値> requires …` の行を足します。形は `prerequisites <塊の名前>` の行と、その下の `<値> requires …` の行です。`start` と `ordered by` は塊のファイルのものです。",
                        "A contribution adds `<value> requires …` lines for the values it adds to an `open` block of the file it adds to. The shape is `prerequisites <block>`, with `<value> requires …` lines under it; `start` and `ordered by` belong to the block's own file."
                    )),
            );
        };
        let name = match self.name_at(line, 1) {
            Some((n, k)) if k == line.len() => n,
            _ => {
                bad(self, span.clone(), tr!("`prerequisites` の後に、塊の名前だけを書いてください", "write the name of the block, and nothing else, after `prerequisites`"));
                return None;
            }
        };
        let mut lines = Vec::new();
        let mut ok = true;
        for l in &block {
            if l.get(1).and_then(|t| t.ident()) == Some(crate::kw::REQUIRES) {
                match self.requires_line(l) {
                    Some(r) => lines.push(r),
                    None => ok = false,
                }
            } else {
                bad(self, span_of(l), tr!("書けるのは `<値> requires …` の行だけです", "only `<value> requires …` lines are written here"));
                ok = false;
            }
        }
        ok.then_some((name, lines))
    }
}

/// An empty head, for a contribution file whose lines fill it in.
pub(super) fn head() -> ContribHead {
    ContribHead { to: None, afters: Vec::new(), extends: Vec::new(), intos: Vec::new(), prereq_lines: Vec::new() }
}
