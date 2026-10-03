//! Reading a `.ctx` (DESIGN 1.1, PLAN B.2): the heading, the sections in their order, and the
//! lines under them. E001 to E005 and E008 come from here; names across files, paths and the
//! things the names point at are `resolve.rs`'s.

use crate::ast::*;
use crate::diag::Diag;
use crate::i18n::Text;
use crate::kw;
use crate::lex::{self, Line, Node, Tok, Token};
use crate::naming::{self, Tool};

/// What kind of `.ctx` the text is, by the first word of its first line: `map`, `context`, or
/// neither. The scan of a directory uses it without reading the file through.
pub fn kind_of(src: &str) -> Option<&'static str> {
    let (lines, _) = lex::lex("", src);
    match lines.first().and_then(|l| l.tokens.first()).and_then(Token::word) {
        Some(kw::MAP) => Some(kw::MAP),
        Some(kw::CONTEXT) => Some(kw::CONTEXT),
        _ => None,
    }
}

/// Read a `.ctx`. `file` is the path the diagnostics name.
pub fn parse(file: &str, src: &str) -> (Option<File>, Vec<Diag>) {
    let (lines, mut diags) = lex::lex(file, src);
    if !diags.is_empty() {
        return (None, diags);
    }
    let (nodes, tdiags) = lex::tree(file, src, &lines);
    if !tdiags.is_empty() {
        diags.extend(tdiags);
        return (None, diags);
    }
    let mut p = P { file, src, lines: &lines, diags: Vec::new() };
    let f = p.file_of(&nodes);
    (f, p.diags)
}

struct P<'a> {
    file: &'a str,
    src: &'a str,
    lines: &'a [Line],
    diags: Vec<Diag>,
}

fn pos(l: &Line, t: &Token) -> Pos {
    Pos { line: l.no, col: t.col }
}

fn ascii_ident(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn is_package(s: &str) -> bool {
    !s.is_empty() && s.split('.').all(ascii_ident)
}

pub fn is_alias(s: &str) -> bool {
    ascii_ident(s)
}

/// The sections, in the order a file writes them (DESIGN 1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sec {
    Description,
    Use,
    Covers,
    Except,
    ProtoRoot,
    Code,
    Owner,
    Also,
    Owns,
    Published,
    Terms,
    Relation,
}

impl Sec {
    fn spelled(self) -> &'static str {
        match self {
            Sec::Description => "description",
            Sec::Use => "use context",
            Sec::Covers => "covers",
            Sec::Except => "except",
            Sec::ProtoRoot => "proto root",
            Sec::Code => "code",
            Sec::Owner => "owner",
            Sec::Also => "also",
            Sec::Owns => "owns",
            Sec::Published => "published language",
            Sec::Terms => "terms",
            Sec::Relation => "upstream",
        }
    }

    fn of_map(self) -> Option<usize> {
        [Sec::Description, Sec::Use, Sec::Covers, Sec::Except, Sec::ProtoRoot, Sec::Code].iter().position(|s| *s == self)
    }

    fn of_context(self) -> Option<usize> {
        [Sec::Description, Sec::Owner, Sec::Also, Sec::Owns, Sec::Published, Sec::Terms, Sec::Relation].iter().position(|s| *s == self)
    }

    fn repeats(self) -> bool {
        matches!(self, Sec::Use | Sec::Code | Sec::Published | Sec::Relation)
    }
}

fn map_order() -> Text {
    tr!(
        "map のファイルは、見出し、`description`、`use context`、`covers`、`except`、`proto root`、`code` の順に書きます。",
        "A map file goes: the heading, `description`, `use context`, `covers`, `except`, `proto root`, `code`."
    )
}

fn context_order() -> Text {
    tr!(
        "context のファイルは、見出し、`description`、`owner`、`also`、`owns`、`published language`、`terms`、関係（`upstream`、`downstream`、`shared kernel with`、`partnership with`、`separate ways from`）の順に書きます。",
        "A context file goes: the heading, `description`, `owner`, `also`, `owns`, `published language`, `terms`, then the relationships (`upstream`, `downstream`, `shared kernel with`, `partnership with`, `separate ways from`)."
    )
}

impl<'a> P<'a> {
    fn line(&self, n: &Node) -> &'a Line {
        &self.lines[n.line]
    }

    fn push(&mut self, code: &'static str, l: &Line, col: usize, msg: Text) -> &mut Diag {
        self.diags.push(Diag::at(code, self.file, l.no, col, msg).source(self.src));
        self.diags.last_mut().unwrap()
    }

    fn note(&mut self, t: Text) {
        if let Some(d) = self.diags.last_mut() {
            d.notes.push(t);
        }
    }

    /// Nothing after token `i`.
    fn end(&mut self, l: &Line, i: usize) -> bool {
        match l.tokens.get(i) {
            None => true,
            Some(t) => {
                let rest = l.rest_from(t.col).trim().to_string();
                self.push("E002", l, t.col, tr!("この行の `{rest}` は書けません", "`{rest}` does not belong on this line"));
                false
            }
        }
    }

    /// No line under `n`.
    fn leaf(&mut self, n: &Node, what: &str) {
        for c in &n.children {
            let l = self.line(c);
            let w = l.rest_from(l.tokens[0].col).trim().to_string();
            self.push("E002", l, l.tokens[0].col, tr!("ここに `{w}` は書けません", "`{w}` does not belong here"));
            self.note(tr!("`{what}` の行の下には、何も書きません。", "Nothing goes under a `{what}` line."));
        }
    }

    /// A name of a map, a context, a term, or a downstream value (DESIGN 1.2): letters, digits
    /// and `_`, not starting with a digit, and not a keyword.
    fn name(&mut self, l: &Line, col: usize, name: &str) -> bool {
        if kw::is_reserved(name) {
            self.push("E002", l, col, tr!("`{name}` は予約語なので、名前にできません", "`{name}` is a keyword and cannot be a name"));
            return false;
        }
        let bad = name.chars().enumerate().find(|(i, c)| !(c.is_alphanumeric() || *c == '_') || (*i == 0 && c.is_ascii_digit()));
        if name.is_empty() || bad.is_some() {
            let c = bad.map(|(_, c)| c.to_string()).unwrap_or_default();
            self.push("E001", l, col, tr!("名前「{name}」に使えない文字 `{c}` があります", "The name `{name}` has `{c}`, which a name cannot have"));
            self.note(tr!("名前は文字、数字、`_` で書き、数字では始めません。", "A name is letters, digits and `_`, and does not start with a digit."));
            return false;
        }
        true
    }

    fn file_of(&mut self, nodes: &[Node]) -> Option<File> {
        let first = &nodes[0];
        let l = self.line(first);
        match l.tokens[0].word() {
            Some(kw::MAP) => self.map_file(nodes).map(File::Map),
            Some(kw::CONTEXT) => self.context_file(nodes).map(File::Context),
            _ => {
                self.push("E003", l, l.tokens[0].col, tr!("ファイルが `map` か `context` の行で始まっていません", "The file does not start with a `map` or a `context` line"));
                self.note(tr!(
                    "コンテキストマップなら `map 通販(shop) v1`、一つのコンテキストなら `context 在庫(inventory) v1` のように書き始めます。",
                    "Start a context map like `map 通販(shop) v1`, and one context like `context 在庫(inventory) v1`."
                ));
                None
            }
        }
    }

    fn heading(&mut self, n: &Node) -> Option<Heading> {
        let l = self.line(n);
        let kw0 = l.tokens[0].word().unwrap_or_default().to_string();
        let Some(t1) = l.tokens.get(1) else {
            self.push("E002", l, l.tokens[0].end, tr!("見出しに名前がありません", "The heading has no name"));
            self.note(tr!("`{kw0} 名前(<別名>) v1` と書きます。", "Write `{kw0} name(<alias>) v1`."));
            return None;
        };
        let Some(w) = t1.word() else {
            self.push("E002", l, t1.col, tr!("見出しの名前は引用符なしで書きます", "The name of the heading is written without quotes"));
            return None;
        };
        let (name, alias) = match w.find('(') {
            None => (w.to_string(), None),
            Some(i) => {
                if !w.ends_with(')') {
                    self.push("E008", l, t1.col + w[..i].chars().count(), tr!("別名の丸括弧が閉じていません", "The parentheses of the alias are not closed"));
                    return None;
                }
                (w[..i].to_string(), Some(w[i + 1..w.len() - 1].to_string()))
            }
        };
        if !self.name(l, t1.col, &name) {
            return None;
        }
        let mut next = 2;
        let alias_col = t1.col + name.chars().count() + 1;
        match &alias {
            None => {
                if let Some(t2) = l.tokens.get(2).filter(|t| t.word().is_some_and(|w| w.starts_with('('))) {
                    self.push("E008", l, t2.col, tr!("別名は、名前のすぐあとに空白を入れずに書きます", "The alias goes right after the name, with no space"));
                    self.note(tr!("`{name}(<別名>)` のように書きます。", "Write `{name}(<alias>)`."));
                    return None;
                }
                let what = if kw0 == kw::MAP { tr!("地図", "the map") } else { tr!("コンテキスト", "the context") };
                self.push("E008", l, t1.col, tr!("{}「{name}」に ASCII の別名がありません", "{} {name} has no ASCII alias", what.ja; what.en));
                self.note(tr!(
                    "`{name}(<別名>)` のように、名前のすぐあとに丸括弧で書きます。別名は `[A-Za-z_][A-Za-z0-9_]*` の形で、Context Mapper の CML の名前と、import の検査の設定の名前になります。",
                    "Write it in parentheses right after the name: `{name}(<alias>)`. An alias is of the form `[A-Za-z_][A-Za-z0-9_]*`, and names it in Context Mapper's CML and in the settings of the import linters."
                ));
                return None;
            }
            Some(a) if !is_alias(a) => {
                self.push("E008", l, alias_col, tr!("別名 `{a}` の形が違います", "The alias `{a}` is not of the right form"));
                self.note(tr!("別名の形は `[A-Za-z_][A-Za-z0-9_]*` です。", "An alias is of the form `[A-Za-z_][A-Za-z0-9_]*`."));
                return None;
            }
            Some(_) => {}
        }
        let version = match l.tokens.get(next).and_then(Token::word) {
            Some(v) if v.len() > 1 && v.starts_with('v') && v[1..].chars().all(|c| c.is_ascii_digit()) => {
                next += 1;
                v[1..].to_string()
            }
            _ => {
                let col = l.tokens.get(next).map(|t| t.col).unwrap_or(t1.end);
                self.push("E002", l, col, tr!("見出しの終わりに、`v1` のようにバージョンを書きます", "The heading ends with its version, like `v1`"));
                return None;
            }
        };
        if !self.end(l, next) {
            return None;
        }
        self.leaf(n, &kw0);
        Some(Heading { name, alias, version, pos: pos(l, &l.tokens[0]), name_pos: pos(l, t1) })
    }

    /// What section a line starts, and the tokens its keyword takes.
    fn section(&self, l: &Line) -> Option<(Sec, usize)> {
        let w = |i: usize| l.tokens.get(i).and_then(Token::word);
        Some(match w(0)? {
            kw::DESCRIPTION => (Sec::Description, 1),
            kw::USE if w(1) == Some(kw::CONTEXT) => (Sec::Use, 2),
            kw::COVERS => (Sec::Covers, 1),
            kw::EXCEPT => (Sec::Except, 1),
            "proto" if w(1) == Some(kw::ROOT) => (Sec::ProtoRoot, 2),
            kw::CODE => (Sec::Code, 1),
            kw::OWNER => (Sec::Owner, 1),
            kw::ALSO => (Sec::Also, 1),
            kw::OWNS => (Sec::Owns, 1),
            kw::PUBLISHED if w(1) == Some(kw::LANGUAGE) => (Sec::Published, 2),
            kw::TERMS => (Sec::Terms, 1),
            kw::UPSTREAM | kw::DOWNSTREAM | kw::SHARED | kw::PARTNERSHIP | kw::SEPARATE => (Sec::Relation, 0),
            _ => return None,
        })
    }

    /// Hold a section to its file's order (E004). Returns whether to read it.
    fn in_order(&mut self, l: &Line, sec: Sec, is_map: bool, last: &mut Option<usize>, seen: &mut Vec<Sec>) -> bool {
        let idx = if is_map { sec.of_map() } else { sec.of_context() };
        let s = sec.spelled();
        let Some(idx) = idx else {
            if is_map {
                self.push("E004", l, l.tokens[0].col, tr!("map のファイルに `{s}` は書けません", "`{s}` does not belong in a map file"));
                self.note(tr!("`{s}` は context のファイルに書く節です。", "`{s}` is a section of a context file."));
            } else {
                self.push("E004", l, l.tokens[0].col, tr!("context のファイルに `{s}` は書けません", "`{s}` does not belong in a context file"));
                self.note(tr!("`{s}` は map のファイルに書く節です。", "`{s}` is a section of a map file."));
            }
            return false;
        };
        if seen.contains(&sec) && !sec.repeats() {
            self.push("E004", l, l.tokens[0].col, tr!("`{s}` が二度あります", "`{s}` comes twice"));
            self.note(tr!("一つのファイルに一度だけ書く節です。", "The section comes once in a file."));
            return false;
        }
        if let Some(prev) = *last
            && idx < prev
        {
            self.push("E004", l, l.tokens[0].col, tr!("`{s}` の位置が違います", "`{s}` is out of its place"));
            self.note(if is_map { map_order() } else { context_order() });
            return false;
        }
        *last = Some(idx);
        if !seen.contains(&sec) {
            seen.push(sec);
        }
        true
    }

    fn unknown_section(&mut self, l: &Line, is_map: bool) {
        let t = &l.tokens[0];
        let w = l.rest_from(t.col).split_whitespace().next().unwrap_or("").to_string();
        if t.is(kw::MAP) || t.is(kw::CONTEXT) {
            self.push("E004", l, t.col, tr!("見出しが二度あります", "There is a second heading"));
            self.note(tr!(
                "一つのファイルには、地図か一つのコンテキストのどちらかだけを書きます。",
                "A file holds one map or one context, nothing more."
            ));
            return;
        }
        self.push("E002", l, t.col, tr!("ここに `{w}` は書けません", "`{w}` does not belong here"));
        self.note(if is_map { map_order() } else { context_order() });
    }

    fn map_file(&mut self, nodes: &[Node]) -> Option<MapFile> {
        let heading = self.heading(&nodes[0])?;
        let mut m = MapFile { heading, description: None, uses: vec![], covers: vec![], except: vec![], proto_roots: vec![], code: vec![] };
        let (mut last, mut seen) = (None, Vec::new());
        let before = self.diags.len();
        for n in &nodes[1..] {
            let l = self.line(n);
            let Some((sec, k)) = self.section(l) else {
                self.unknown_section(l, true);
                continue;
            };
            if !self.in_order(l, sec, true, &mut last, &mut seen) {
                continue;
            }
            match sec {
                Sec::Description => {
                    m.description = self.one_string(l, k);
                    self.leaf(n, "description");
                }
                Sec::Use => {
                    if let Some(s) = self.one_string(l, k) {
                        m.uses.push(s);
                    }
                    self.leaf(n, "use context");
                }
                Sec::Covers => {
                    m.covers = self.strings(l, k).unwrap_or_default();
                    self.leaf(n, "covers");
                }
                Sec::Except => {
                    m.except = self.strings(l, k).unwrap_or_default();
                    self.leaf(n, "except");
                }
                Sec::ProtoRoot => {
                    m.proto_roots = self.strings(l, k).unwrap_or_default();
                    self.leaf(n, "proto root");
                }
                Sec::Code => {
                    if let Some(c) = self.code(n, &m.code) {
                        m.code.push(c);
                    }
                }
                _ => unreachable!("of_map lets only map sections through"),
            }
        }
        let h = nodes[0].line;
        let hl = &self.lines[h];
        if self.diags.len() == before {
            if m.uses.is_empty() {
                self.push("E004", hl, 1, tr!("`use context` がありません", "There is no `use context`"));
                self.note(tr!("地図は、コンテキストのファイルを `use context \"<パス>\"` で一つ以上読みます。", "A map reads one or more context files with `use context \"<path>\"`."));
            }
            if !seen.contains(&Sec::Covers) {
                self.push("E004", hl, 1, tr!("`covers` がありません", "There is no `covers`"));
                self.note(tr!("地図が覆う範囲を `covers \".\"` のように書きます。", "Write what the map covers, like `covers \".\"`."));
            }
        }
        Some(m)
    }

    fn one_string(&mut self, l: &Line, i: usize) -> Option<Str> {
        match l.tokens.get(i) {
            Some(Token { tok: Tok::Str(s), col, .. }) => {
                let s = Str { value: s.clone(), pos: Pos { line: l.no, col: *col } };
                self.end(l, i + 1).then_some(s)
            }
            other => {
                let col = other.map(|t| t.col).unwrap_or_else(|| l.tokens.last().map(|t| t.end).unwrap_or(1));
                self.push("E002", l, col, tr!("ここには `\"…\"` を一つ書きます", "One `\"…\"` goes here"));
                None
            }
        }
    }

    /// `"…" (, "…")*` from token `i` to the end of the line.
    fn strings(&mut self, l: &Line, i: usize) -> Option<Vec<Str>> {
        let mut out = Vec::new();
        let mut j = i;
        loop {
            match l.tokens.get(j) {
                Some(Token { tok: Tok::Str(s), col, .. }) => out.push(Str { value: s.clone(), pos: Pos { line: l.no, col: *col } }),
                other => {
                    let col = other.map(|t| t.col).unwrap_or_else(|| l.tokens.last().map(|t| t.end).unwrap_or(1));
                    self.push("E002", l, col, tr!("ここにはパスを `\"…\"` で書きます", "A path in `\"…\"` goes here"));
                    return None;
                }
            }
            match l.tokens.get(j + 1) {
                None => return Some(out),
                Some(Token { tok: Tok::Comma, .. }) => j += 2,
                Some(_) => {
                    self.end(l, j + 1);
                    return None;
                }
            }
        }
    }

    /// Files and directories (`dir "…", "…"`, `rulec "…"`) from token `i` to the end of the line.
    /// A tool's word holds for the strings after it until another tool's word.
    fn items(&mut self, l: &Line, i: usize) -> Option<Vec<Item>> {
        let mut out = Vec::new();
        let mut j = i;
        let mut tool: Option<Option<Tool>> = None;
        loop {
            let t = l.tokens.get(j);
            match t.map(|t| &t.tok) {
                Some(Tok::Word(w)) => {
                    let w = w.clone();
                    let col = t.unwrap().col;
                    if w == kw::DIR {
                        tool = Some(None);
                    } else {
                        match Tool::from_word(&w) {
                            Some(Tool::Yurai | Tool::Sakai) => {
                                self.push("E002", l, col, tr!("ここに `{w}` のファイルは書けません", "A `{w}` file does not go here"));
                                self.note(tr!(
                                    "書けるのは成果物（rulec、dandori、koyomi、chobo、geas、proto、file）とディレクトリ（dir）です。",
                                    "Artifacts (rulec, dandori, koyomi, chobo, geas, proto, file) and directories (dir) go here."
                                ));
                                return None;
                            }
                            Some(tt) => tool = Some(Some(tt)),
                            None => {
                                self.push("E002", l, col, tr!("ここに `{w}` は書けません", "`{w}` does not belong here"));
                                self.note(tr!(
                                    "`dir \"<パス>\"` か、`rulec \"<パス>\"` のようにツールの語とパスを書きます。",
                                    "Write `dir \"<path>\"`, or a tool and a path, like `rulec \"<path>\"`."
                                ));
                                return None;
                            }
                        }
                    }
                    j += 1;
                    continue;
                }
                Some(Tok::Str(s)) => {
                    let Some(tl) = tool else {
                        self.push("E002", l, t.unwrap().col, tr!("パスの前に `dir` かツールの語を書きます", "A path comes after `dir` or a tool"));
                        return None;
                    };
                    out.push(Item { tool: tl, path: Str { value: s.clone(), pos: Pos { line: l.no, col: t.unwrap().col } } });
                }
                _ => {
                    let col = t.map(|t| t.col).unwrap_or_else(|| l.tokens.last().map(|t| t.end).unwrap_or(1));
                    self.push("E002", l, col, tr!("ここにはパスを `\"…\"` で書きます", "A path in `\"…\"` goes here"));
                    return None;
                }
            }
            match l.tokens.get(j + 1) {
                None => return Some(out),
                Some(Token { tok: Tok::Comma, .. }) => j += 2,
                Some(t) => {
                    let col = t.col;
                    self.push("E002", l, col, tr!("ここに書けるのはファイルとディレクトリだけで、要素は書けません", "Only files and directories go here, not the things in them"));
                    self.note(tr!(
                        "コンテキストに属するのはファイルの単位です。項は `,` で区切ります。",
                        "A context owns whole files. Separate the entries with `,`."
                    ));
                    return None;
                }
            }
        }
    }

    fn code(&mut self, n: &Node, seen: &[Code]) -> Option<Code> {
        let l = self.line(n);
        let lang = match l.tokens.get(1).and_then(Token::word) {
            Some(w) if kw::LANGUAGES.contains(&w) => w.to_string(),
            other => {
                let col = l.tokens.get(1).map(|t| t.col).unwrap_or(l.tokens[0].end);
                let w = other.unwrap_or("").to_string();
                self.push("E002", l, col, tr!("`code` のあとの言語 `{w}` は知りません", "`{w}` after `code` is not a language sakai knows"));
                self.note(tr!("書けるのは python、typescript、java、go です。", "The languages are python, typescript, java and go."));
                return None;
            }
        };
        if seen.iter().any(|c| c.language == lang) {
            self.push("E004", l, l.tokens[0].col, tr!("`code {lang}` が二度あります", "`code {lang}` comes twice"));
            self.note(tr!("言語ごとに、置き場所は一つです。", "Each language has one place."));
            return None;
        }
        let path = self.one_string(l, 2)?;
        let mut test = None;
        for c in &n.children {
            let cl = self.line(c);
            if cl.tokens[0].is(kw::TEST) && lang == "java" && test.is_none() {
                test = self.one_string(cl, 1);
                self.leaf(c, "test");
            } else {
                let w = cl.rest_from(cl.tokens[0].col).trim().to_string();
                self.push("E002", cl, cl.tokens[0].col, tr!("ここに `{w}` は書けません", "`{w}` does not belong here"));
                self.note(tr!(
                    "`code` の下に書けるのは、java の `test \"<パス>\"`（ArchUnit のテストを書く場所）を一つだけです。",
                    "Under `code` goes only, for java, one `test \"<path>\"` (where the ArchUnit test is written)."
                ));
            }
        }
        Some(Code { language: lang, path, test, pos: pos(l, &l.tokens[0]) })
    }

    fn context_file(&mut self, nodes: &[Node]) -> Option<ContextFile> {
        let heading = self.heading(&nodes[0])?;
        let mut c = ContextFile { heading, description: None, owner: None, also: vec![], owns: vec![], published: vec![], terms: vec![], relations: vec![] };
        let (mut last, mut seen) = (None, Vec::new());
        let before = self.diags.len();
        for n in &nodes[1..] {
            let l = self.line(n);
            let Some((sec, k)) = self.section(l) else {
                self.unknown_section(l, false);
                continue;
            };
            if !self.in_order(l, sec, false, &mut last, &mut seen) {
                continue;
            }
            match sec {
                Sec::Description => {
                    c.description = self.one_string(l, k);
                    self.leaf(n, "description");
                }
                Sec::Owner => {
                    c.owner = self.one_string(l, k);
                    self.leaf(n, "owner");
                }
                Sec::Also => {
                    c.also = self.strings(l, k).unwrap_or_default();
                    self.leaf(n, "also");
                }
                Sec::Owns => {
                    self.end(l, 1);
                    if n.children.is_empty() {
                        self.push("E004", l, l.tokens[0].col, tr!("`owns` の下に何もありません", "There is nothing under `owns`"));
                        self.note(tr!("コンテキストに属するディレクトリとファイルを、`dir \"<パス>\"` のように字下げして並べます。", "List the directories and files the context owns, indented, like `dir \"<path>\"`."));
                    }
                    for ch in &n.children {
                        let cl = self.line(ch);
                        if let Some(items) = self.items(cl, 0) {
                            c.owns.extend(items);
                        }
                        self.leaf(ch, "owns");
                    }
                }
                Sec::Published => {
                    if let Some(p) = self.published(n) {
                        c.published.push(p);
                    }
                }
                Sec::Terms => {
                    self.end(l, 1);
                    for ch in &n.children {
                        if let Some(t) = self.term(ch) {
                            c.terms.push(t);
                        }
                    }
                }
                Sec::Relation => {
                    if let Some(r) = self.relation(n) {
                        c.relations.push(r);
                    }
                }
                _ => unreachable!("of_context lets only context sections through"),
            }
        }
        if self.diags.len() == before && !seen.contains(&Sec::Owns) {
            let hl = &self.lines[nodes[0].line];
            self.push("E004", hl, 1, tr!("`owns` がありません", "There is no `owns`"));
            self.note(tr!(
                "コンテキストは、属するディレクトリとファイルを `owns` に並べます。成果物は、どれもちょうど一つのコンテキストに属します。",
                "A context lists the directories and files it owns under `owns`; every artifact belongs to exactly one context."
            ));
        }
        Some(c)
    }

    fn published(&mut self, n: &Node) -> Option<Published> {
        let l = self.line(n);
        let Some(t) = l.tokens.get(2) else {
            self.push("E002", l, l.tokens[1].end, tr!("`published language` のあとに package を書きます", "`published language` is followed by its package"));
            return None;
        };
        let pkg = t.word().unwrap_or("").to_string();
        if !is_package(&pkg) {
            self.push("E002", l, t.col, tr!("ここには proto の package を書きます", "A proto package goes here"));
            self.note(tr!(
                "package は `warehouse.v1` のように、ASCII の名前を `.` でつないだものです。公表された言語は proto の package を単位にします。",
                "A package is ASCII names joined with `.`, like `warehouse.v1`; a published language is one proto package."
            ));
            return None;
        }
        self.end(l, 3);
        let mut p = Published { package: pkg, pos: pos(l, &l.tokens[0]), package_pos: pos(l, t), protos: vec![], rulec: None, services: vec![], generated: vec![] };
        let mut said_services = false;
        let before = self.diags.len();
        for ch in &n.children {
            let cl = self.line(ch);
            let t0 = &cl.tokens[0];
            if t0.is("proto") {
                if p.rulec.is_some() {
                    self.mixed(cl);
                } else if let Some(ss) = self.strings(cl, 1) {
                    p.protos.extend(ss);
                }
            } else if t0.is("rulec") {
                if !p.protos.is_empty() || p.rulec.is_some() {
                    self.mixed(cl);
                } else {
                    p.rulec = self.one_string(cl, 1);
                }
            } else if t0.is(kw::OPEN) && cl.tokens.get(1).is_some_and(|t| t.is(kw::HOST)) && cl.tokens.get(2).is_some_and(|t| t.is(kw::SERVICE)) {
                if said_services {
                    self.push("E004", cl, t0.col, tr!("`open host service` が二度あります", "`open host service` comes twice"));
                    self.note(tr!("公開ホストサービスは一行に `,` で並べます。", "List the open host services on one line, separated by `,`."));
                } else {
                    said_services = true;
                    p.services = self.idents(cl, 3)?;
                }
            } else if t0.is(kw::GENERATED) && cl.tokens.get(1).is_some_and(|t| t.is(kw::DIR)) {
                if let Some(ss) = self.strings(cl, 2) {
                    p.generated.extend(ss);
                }
            } else {
                let w = cl.rest_from(t0.col).trim().to_string();
                self.push("E002", cl, t0.col, tr!("ここに `{w}` は書けません", "`{w}` does not belong here"));
                self.note(tr!(
                    "公表された言語の下には、`proto \"<パス>\"` か `rulec \"<パス>\"`、`open host service <サービス>`、`generated dir \"<パス>\"` を書きます。",
                    "Under a published language go `proto \"<path>\"` or `rulec \"<path>\"`, `open host service <service>`, and `generated dir \"<path>\"`."
                ));
            }
            self.leaf(ch, cl.tokens[0].word().unwrap_or(""));
        }
        if p.protos.is_empty() && p.rulec.is_none() && self.diags.len() == before {
            let pk = p.package.clone();
            self.push("E004", l, l.tokens[0].col, tr!("公表された言語 {pk} に、proto も rulec の規則もありません", "The published language {pk} has no proto and no rule"));
            self.note(tr!(
                "下に `proto \"<パス>\"`（その package を宣言する proto のファイル）か、`rulec \"<パス>\"`（Connect のサービスを公表する規則）を書きます。",
                "Write under it `proto \"<path>\"` (the proto files that declare the package) or `rulec \"<path>\"` (a rule whose Connect service it publishes)."
            ));
        }
        Some(p)
    }

    fn mixed(&mut self, l: &Line) {
        self.push("E004", l, l.tokens[0].col, tr!("一つの公表された言語に、proto と rulec を混ぜたり、規則を二つ書いたりはできません", "One published language cannot mix proto and rulec, or hold two rules"));
        self.note(tr!(
            "rulec の規則は、規則ごとに一つの Connect のサービスを公表するので、規則ごとに `published language rulec.<別名>.v<版>` を分けます。",
            "A rule publishes one Connect service of its own, so each rule gets its own `published language rulec.<alias>.v<n>`."
        ));
    }

    /// `A, B, C`: the names of services or packages, ASCII identifiers (or packages, `dotted`).
    fn idents_with(&mut self, l: &Line, i: usize, dotted: bool) -> Option<Vec<(String, Pos)>> {
        let mut out = Vec::new();
        let mut j = i;
        loop {
            match l.tokens.get(j) {
                Some(t @ Token { tok: Tok::Word(w), .. }) if (dotted && is_package(w)) || (!dotted && ascii_ident(w)) => out.push((w.clone(), pos(l, t))),
                other => {
                    let col = other.map(|t| t.col).unwrap_or_else(|| l.tokens.last().map(|t| t.end).unwrap_or(1));
                    if dotted {
                        self.push("E002", l, col, tr!("ここには proto の package を書きます", "A proto package goes here"));
                        self.note(tr!("package は `warehouse.v1` のように、ASCII の名前を `.` でつないだものです。", "A package is ASCII names joined with `.`, like `warehouse.v1`."));
                    } else {
                        self.push("E002", l, col, tr!("ここには proto のサービスの名前を書きます", "The name of a proto service goes here"));
                    }
                    return None;
                }
            }
            match l.tokens.get(j + 1) {
                None => return Some(out),
                Some(Token { tok: Tok::Comma, .. }) => j += 2,
                Some(_) => {
                    self.end(l, j + 1);
                    return None;
                }
            }
        }
    }

    fn idents(&mut self, l: &Line, i: usize) -> Option<Vec<(String, Pos)>> {
        self.idents_with(l, i, false)
    }

    fn term(&mut self, n: &Node) -> Option<Term> {
        let l = self.line(n);
        let t0 = &l.tokens[0];
        let Some(name) = t0.word().map(String::from) else {
            self.push("E002", l, t0.col, tr!("語の名前は引用符なしで書きます", "A term's name is written without quotes"));
            return None;
        };
        if !self.name(l, t0.col, &name) {
            return None;
        }
        let mut t = Term { name: name.clone(), pos: pos(l, t0), definition: None, as_term: None, means: vec![], also: vec![] };
        match l.tokens.get(1) {
            Some(Token { tok: Tok::Str(s), col, .. }) => {
                t.definition = Some(Str { value: s.clone(), pos: Pos { line: l.no, col: *col } });
                if l.tokens.get(2).is_some_and(|x| x.is(kw::AS)) {
                    self.both(l, &name);
                    return None;
                }
                if !self.end(l, 2) {
                    return None;
                }
            }
            Some(x) if x.is(kw::AS) => {
                let Some(tw) = l.tokens.get(2) else {
                    self.push("E002", l, x.end, tr!("`as` のあとに `<コンテキスト>.<語>` を書きます", "`as` is followed by `<context>.<term>`"));
                    return None;
                };
                let w = tw.word().unwrap_or("").to_string();
                let Some((cx, tm)) = w.split_once('.') else {
                    self.push("E002", l, tw.col, tr!("`as` のあとに `<コンテキスト>.<語>` を書きます", "`as` is followed by `<context>.<term>`"));
                    self.note(tr!("`注文 as 受注.注文` のように、相手のコンテキストの名前と語の名前を `.` でつなぎます。", "Join the other context's name and its term with `.`, like `注文 as 受注.注文`."));
                    return None;
                };
                if !self.name(l, tw.col, cx) || !self.name(l, tw.col + cx.chars().count() + 1, tm) {
                    return None;
                }
                if l.tokens.get(3).is_some_and(|x| matches!(x.tok, Tok::Str(_))) {
                    self.both(l, &name);
                    return None;
                }
                if !self.end(l, 3) {
                    return None;
                }
                t.as_term = Some(AsTerm { context: cx.to_string(), term: tm.to_string(), pos: pos(l, tw) });
            }
            Some(x) => {
                let col = x.col;
                self.push("E002", l, col, tr!("語の名前のあとには、定義の文か `as` を書きます", "A term's name is followed by its definition or by `as`"));
                return None;
            }
            None => {
                self.push("E004", l, t0.col, tr!("語「{name}」に、定義の文も `as` もありません", "The term {name} has neither a definition nor `as`"));
                self.note(tr!(
                    "語は、`\"…\"` の定義の文か、`as <コンテキスト>.<語>`（相手の語を同じ意味のまま取り入れる）のどちらか一つを持ちます。",
                    "A term has either its definition in `\"…\"` or `as <context>.<term>` (the other context's term, with the same meaning), one of the two."
                ));
                return None;
            }
        }
        let mut said_also = false;
        for ch in &n.children {
            let cl = self.line(ch);
            let c0 = &cl.tokens[0];
            if c0.is(kw::MEANS) {
                if let Some(e) = self.element(cl, 1) {
                    t.means.push(e);
                }
            } else if c0.is(kw::ALSO) {
                if said_also {
                    self.push("E004", cl, c0.col, tr!("`also` が二度あります", "`also` comes twice"));
                    self.note(tr!("別の呼び方は一行に `,` で並べます。", "List the other names on one line, separated by `,`."));
                } else {
                    said_also = true;
                    t.also = self.strings(cl, 1).unwrap_or_default();
                }
            } else {
                let w = cl.rest_from(c0.col).trim().to_string();
                self.push("E002", cl, c0.col, tr!("ここに `{w}` は書けません", "`{w}` does not belong here"));
                self.note(tr!("語の下には `means <要素>` と `also \"<名前>\"` を書きます。", "Under a term go `means <element>` and `also \"<name>\"`."));
            }
            self.leaf(ch, c0.word().unwrap_or(""));
        }
        Some(t)
    }

    fn both(&mut self, l: &Line, name: &str) {
        self.push("E004", l, l.tokens[0].col, tr!("語「{name}」が、定義の文と `as` の両方を持っています", "The term {name} has both a definition and `as`"));
        self.note(tr!(
            "`as` で取り入れた語は、相手の定義をそのまま使うので、定義の文を書きません。",
            "A term taken with `as` keeps the other context's definition, so it has none of its own."
        ));
    }

    /// A thing in an artifact, from token `i` to the end of the line (DESIGN 2.1, 2.7).
    fn element(&mut self, l: &Line, i: usize) -> Option<Element> {
        let Some(t) = l.tokens.get(i) else {
            let col = l.tokens.last().map(|t| t.end).unwrap_or(1);
            self.push("E002", l, col, tr!("ここに要素を書きます", "An element goes here"));
            self.note(element_forms());
            return None;
        };
        let w = t.word().unwrap_or("");
        if Tool::from_word(w).is_some() {
            let raw = l.rest_from(t.col);
            return match naming::read(&raw) {
                Ok(written) => Some(Element::Long { written, pos: pos(l, t) }),
                Err((off, msg)) => {
                    self.push("E011", l, t.col + off, msg);
                    None
                }
            };
        }
        if matches!(w, "message" | "enum" | "service") {
            let Some(nt) = l.tokens.get(i + 1).filter(|x| x.word().is_some()) else {
                self.push("E002", l, t.end, tr!("`{w}` のあとに名前がありません", "`{w}` has no name after it"));
                return None;
            };
            let name = nt.word().unwrap().to_string();
            let mut child = None;
            if let Some(ct) = l.tokens.get(i + 2) {
                let ck = ct.word().unwrap_or("").to_string();
                if Tool::Proto.child(w) != Some(ck.as_str()) {
                    let msg = match Tool::Proto.parent(&ck) {
                        Some(p) => tr!("`{ck}` は `{p}` のすぐあとにだけ書けます", "`{ck}` comes only right after `{p}`"),
                        None => tr!("`{ck}` は `{w}` の下に書けません", "`{ck}` cannot come under `{w}`"),
                    };
                    self.push("E011", l, ct.col, msg);
                    return None;
                }
                let Some(cn) = l.tokens.get(i + 3).and_then(Token::word) else {
                    self.push("E002", l, ct.end, tr!("`{ck}` のあとに名前がありません", "`{ck}` has no name after it"));
                    return None;
                };
                child = Some((ck, cn.to_string()));
                if !self.end(l, i + 4) {
                    return None;
                }
            }
            return Some(Element::Short { kind: w.to_string(), name, child, pos: pos(l, t) });
        }
        let shown = l.rest_from(t.col).trim().to_string();
        self.push("E002", l, t.col, tr!("`{shown}` は要素の名指しではありません", "`{shown}` does not name an element"));
        self.note(element_forms());
        None
    }

    fn partner(&mut self, l: &Line, i: usize) -> Option<(String, Pos)> {
        let Some(t) = l.tokens.get(i) else {
            let col = l.tokens.last().map(|t| t.end).unwrap_or(1);
            self.push("E002", l, col, tr!("相手のコンテキストの名前がありません", "The other context's name is missing"));
            return None;
        };
        let w = t.word().unwrap_or("").to_string();
        if !self.name(l, t.col, &w) {
            return None;
        }
        Some((w, pos(l, t)))
    }

    fn words_are(&mut self, l: &Line, ws: &[&str], form: &str) -> bool {
        for (i, w) in ws.iter().enumerate() {
            if !l.tokens.get(i).is_some_and(|t| t.is(w)) {
                let col = l.tokens.get(i).map(|t| t.col).unwrap_or_else(|| l.tokens.last().map(|t| t.end).unwrap_or(1));
                self.push("E002", l, col, tr!("`{form}` と書きます", "Write `{form}`"));
                return false;
            }
        }
        true
    }

    fn relation(&mut self, n: &Node) -> Option<Relation> {
        let l = self.line(n);
        let t0 = &l.tokens[0];
        let at = pos(l, t0);
        match t0.word().unwrap_or("") {
            kw::UPSTREAM => {
                let (partner, ppos) = self.partner(l, 1)?;
                let mut roles = Vec::new();
                let mut j = 2;
                loop {
                    let Some(t) = l.tokens.get(j) else {
                        if roles.is_empty() {
                            self.push("E002", l, l.tokens[1].end, tr!("`upstream {partner}` に役割がありません", "`upstream {partner}` has no role"));
                            self.note(roles_note());
                            return None;
                        }
                        break;
                    };
                    let role = match t.word().unwrap_or("") {
                        kw::CONFORMIST => Role::Conformist,
                        kw::CUSTOMER => Role::Customer,
                        kw::ANTICORRUPTION if l.tokens.get(j + 1).is_some_and(|x| x.is(kw::LAYER)) => {
                            j += 1;
                            Role::Acl
                        }
                        other => {
                            let other = other.to_string();
                            self.push("E002", l, t.col, tr!("`{other}` は上流との関係の役割ではありません", "`{other}` is not a role toward an upstream"));
                            self.note(roles_note());
                            return None;
                        }
                    };
                    roles.push((role, pos(l, t)));
                    match l.tokens.get(j + 1) {
                        None => break,
                        Some(Token { tok: Tok::Comma, .. }) => j += 2,
                        Some(_) => {
                            self.end(l, j + 1);
                            return None;
                        }
                    }
                }
                let mut u = Upstream { roles, through: vec![], layer: vec![], enums: vec![], terms: vec![] };
                let mut said_through = false;
                for ch in &n.children {
                    let cl = self.line(ch);
                    let c0 = &cl.tokens[0];
                    match c0.word().unwrap_or("") {
                        kw::THROUGH => {
                            if said_through {
                                self.push("E004", cl, c0.col, tr!("`through` が二度あります", "`through` comes twice"));
                                self.note(tr!("通る package は一行に `,` で並べます。", "List the packages on one line, separated by `,`."));
                            } else {
                                said_through = true;
                                if let Some(ps) = self.idents_with(cl, 1, true) {
                                    u.through = ps;
                                }
                            }
                            self.leaf(ch, "through");
                        }
                        kw::LAYER => {
                            if let Some(items) = self.items(cl, 1) {
                                u.layer.extend(items);
                            }
                            self.leaf(ch, "layer");
                        }
                        kw::ENUM => {
                            if let Some(e) = self.enum_map(ch) {
                                u.enums.push(e);
                            }
                        }
                        kw::TERM => {
                            if let Some(t) = self.term_map(cl) {
                                u.terms.push(t);
                            }
                            self.leaf(ch, "term");
                        }
                        _ => {
                            let w = cl.rest_from(c0.col).trim().to_string();
                            self.push("E002", cl, c0.col, tr!("ここに `{w}` は書けません", "`{w}` does not belong here"));
                            self.note(tr!(
                                "`upstream` の下には、`through`、`layer`、`enum <上流の列挙> -> <先>`、`term <上流の語> -> <下流の語>` を書きます。",
                                "Under `upstream` go `through`, `layer`, `enum <upstream enum> -> <target>` and `term <upstream term> -> <term>`."
                            ));
                        }
                    }
                }
                if !said_through {
                    self.push("E004", l, t0.col, tr!("`upstream {partner}` の下に `through` がありません", "`upstream {partner}` has no `through` under it"));
                    self.note(tr!(
                        "上流と下流の関係には、下流が通ってよい上流の公表された言語の package を `through warehouse.v1` のように書きます。",
                        "An upstream relationship says which of the upstream's published packages the downstream goes through, like `through warehouse.v1`."
                    ));
                }
                Some(Relation { kind: RelKind::Upstream(u), partner, partner_pos: ppos, pos: at })
            }
            kw::DOWNSTREAM => {
                let (partner, ppos) = self.partner(l, 1)?;
                match l.tokens.get(2) {
                    Some(t) if t.is(kw::SUPPLIER) => {
                        self.end(l, 3);
                    }
                    other => {
                        let col = other.map(|t| t.col).unwrap_or(l.tokens[1].end);
                        self.push("E002", l, col, tr!("`downstream {partner}` のあとには supplier だけを書けます", "`downstream {partner}` takes only supplier after it"));
                        self.note(tr!(
                            "上流と下流の関係は、下流のファイルに `upstream <上流> <役割>` で書きます。上流の側が書くのは、顧客を引き受けるときの `downstream <顧客> supplier` だけです。",
                            "An upstream relationship is written in the downstream's file, as `upstream <upstream> <role>`. The upstream writes only `downstream <customer> supplier`, to take a customer on."
                        ));
                        return None;
                    }
                }
                self.leaf(n, "downstream");
                Some(Relation { kind: RelKind::Downstream, partner, partner_pos: ppos, pos: at })
            }
            kw::SHARED => {
                if !self.words_are(l, &[kw::SHARED, kw::KERNEL, kw::WITH], "shared kernel with <コンテキスト>") {
                    return None;
                }
                let (partner, ppos) = self.partner(l, 3)?;
                self.end(l, 4);
                let mut items = Vec::new();
                if n.children.is_empty() {
                    self.push("E004", l, t0.col, tr!("共有カーネルに並べるものがありません", "The shared kernel lists nothing"));
                    self.note(tr!(
                        "二つのコンテキストが一緒に持つ成果物とディレクトリを、下に字下げして並べます。",
                        "List, indented under it, the artifacts and directories the two contexts hold together."
                    ));
                }
                for ch in &n.children {
                    let cl = self.line(ch);
                    if let Some(is) = self.items(cl, 0) {
                        items.extend(is);
                    }
                    self.leaf(ch, "shared kernel with");
                }
                Some(Relation { kind: RelKind::SharedKernel(items), partner, partner_pos: ppos, pos: at })
            }
            kw::PARTNERSHIP => {
                if !self.words_are(l, &[kw::PARTNERSHIP, kw::WITH], "partnership with <コンテキスト>") {
                    return None;
                }
                let (partner, ppos) = self.partner(l, 2)?;
                self.end(l, 3);
                self.leaf(n, "partnership with");
                Some(Relation { kind: RelKind::Partnership, partner, partner_pos: ppos, pos: at })
            }
            kw::SEPARATE => {
                if !self.words_are(l, &[kw::SEPARATE, kw::WAYS, kw::FROM], "separate ways from <コンテキスト>") {
                    return None;
                }
                let (partner, ppos) = self.partner(l, 3)?;
                self.end(l, 4);
                self.leaf(n, "separate ways from");
                Some(Relation { kind: RelKind::SeparateWays, partner, partner_pos: ppos, pos: at })
            }
            _ => unreachable!("section() lets only the relationship words through"),
        }
    }

    fn enum_map(&mut self, n: &Node) -> Option<EnumMap> {
        let l = self.line(n);
        let form = tr!("`enum <上流の列挙> -> <先>` と書きます", "Write `enum <upstream enum> -> <target>`");
        let (Some(et), Some(Token { tok: Tok::Arrow, .. }), Some(tt)) = (l.tokens.get(1), l.tokens.get(2), l.tokens.get(3)) else {
            let col = l.tokens.get(1).map(|t| t.col).unwrap_or(l.tokens[0].end);
            self.push("E002", l, col, form);
            return None;
        };
        let Some(from) = et.word().map(String::from) else {
            self.push("E002", l, et.col, form);
            return None;
        };
        let target = match &tt.tok {
            Tok::Word(w) if Tool::from_word(w).is_some() || w == kw::ENUM => Target::Element(self.element(l, 3)?),
            Tok::Word(w) | Tok::Str(w) => {
                let w = w.clone();
                if matches!(tt.tok, Tok::Word(_)) && !self.name(l, tt.col, &w) {
                    return None;
                }
                if !self.end(l, 4) {
                    return None;
                }
                Target::Name(w, pos(l, tt))
            }
            _ => {
                self.push("E002", l, tt.col, form);
                return None;
            }
        };
        let mut values = Vec::new();
        for ch in &n.children {
            let cl = self.line(ch);
            let vform = tr!(
                "値の行は `<上流の値> -> <下流の値>` か `<上流の値> -> refuse \"<理由>\"` と書きます",
                "A value line is `<upstream value> -> <value>` or `<upstream value> -> refuse \"<why>\"`"
            );
            let (Some(vt), Some(Token { tok: Tok::Arrow, .. }), Some(wt)) = (cl.tokens.first(), cl.tokens.get(1), cl.tokens.get(2)) else {
                self.push("E002", cl, cl.tokens[0].col, vform);
                continue;
            };
            let Some(v) = vt.word().map(String::from) else {
                self.push("E002", cl, vt.col, vform);
                continue;
            };
            let to = if wt.is(kw::REFUSE) {
                let reason = match cl.tokens.get(3) {
                    Some(Token { tok: Tok::Str(s), col, .. }) => Some(Str { value: s.clone(), pos: Pos { line: cl.no, col: *col } }),
                    _ => None,
                };
                let next = if reason.is_some() { 4 } else { 3 };
                if !self.end(cl, next) {
                    continue;
                }
                ValueTo::Refuse(reason, pos(cl, wt))
            } else {
                match &wt.tok {
                    Tok::Word(w) | Tok::Str(w) => {
                        let w = w.clone();
                        if !self.end(cl, 3) {
                            continue;
                        }
                        ValueTo::Value(w, pos(cl, wt))
                    }
                    _ => {
                        self.push("E002", cl, wt.col, vform);
                        continue;
                    }
                }
            };
            values.push(ValueMap { from: v, from_pos: pos(cl, vt), to });
            self.leaf(ch, "enum");
        }
        Some(EnumMap { from, from_pos: pos(l, et), target, values, pos: pos(l, &l.tokens[0]) })
    }

    fn term_map(&mut self, l: &Line) -> Option<TermMap> {
        let form = tr!("`term <上流の語> -> <下流の語>` と書きます", "Write `term <upstream term> -> <term>`");
        let (Some(a), Some(Token { tok: Tok::Arrow, .. }), Some(b)) = (l.tokens.get(1), l.tokens.get(2), l.tokens.get(3)) else {
            let col = l.tokens.get(1).map(|t| t.col).unwrap_or(l.tokens[0].end);
            self.push("E002", l, col, form);
            return None;
        };
        let (Some(aw), Some(bw)) = (a.word().map(String::from), b.word().map(String::from)) else {
            self.push("E002", l, a.col, form);
            return None;
        };
        if !self.name(l, a.col, &aw) || !self.name(l, b.col, &bw) || !self.end(l, 4) {
            return None;
        }
        Some(TermMap { from: aw, from_pos: pos(l, a), to: bw, to_pos: pos(l, b), pos: pos(l, &l.tokens[0]) })
    }
}

fn roles_note() -> Text {
    tr!(
        "役割は conformist（順応者）、anticorruption layer（腐敗防止層）、customer（顧客）のどれかです。customer には anticorruption layer を `,` で添えられます。",
        "The role is conformist, anticorruption layer or customer; customer may take anticorruption layer beside it, after a `,`."
    )
}

fn element_forms() -> Text {
    tr!(
        "要素は `proto \"<パス>\" message Order` のように名指すか、自分の公表された言語の proto の要素なら `message Order`、`enum Stock value STOCK_SHORT` のように短く書きます。",
        "Name the element like `proto \"<path>\" message Order`, or, for an element of the context's own published language, shortly: `message Order`, `enum Stock value STOCK_SHORT`."
    )
}
