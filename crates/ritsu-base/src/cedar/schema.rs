//! Cedar's schemas, in the Cedar schema format (`.cedarschema`) and the JSON schema format, read
//! into one model: the JSON format's (Cedar's `json_schema::Fragment`), which both directions of
//! `cedar translate-schema` go through. The Cedar format is read with the checks Cedar 4.13.0
//! makes while it converts (`validator/cedar_schema/to_json_schema.rs`): names declared twice,
//! reserved names and keywords, `principal` and `resource` of each `appliesTo`. The JSON format
//! is read with the checks of its deserializer (`validator/json_schema.rs`): the fields each
//! object may and must have. Both writers order what they write as Cedar's maps do.

use super::ast::{Annotation, Name};
use super::lexer::{lex, Grammar, Lines, Tok, Token};
use super::Error;
use crate::json::Json;
use crate::text::Text;
use crate::tr;
use crate::yaml::{self, Node, Value};

/// A schema: its namespaces in the order they are written (the declarations outside any
/// namespace gathered into one, whose name is `None`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schema {
    pub namespaces: Vec<Namespace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Namespace {
    /// `None` for the declarations outside any namespace (the JSON format's `""`).
    pub name: Option<Name>,
    pub annotations: Vec<Annotation>,
    pub common_types: Vec<CommonType>,
    pub entity_types: Vec<EntityType>,
    pub actions: Vec<Action>,
    pub line: usize,
    pub col: usize,
}

/// `type Name = …;`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonType {
    pub name: String,
    pub ty: Type,
    pub annotations: Vec<Annotation>,
    pub line: usize,
    pub col: usize,
}

/// `entity Name …;`: one for each name of a declaration that names several.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityType {
    pub name: String,
    pub kind: EntityKind,
    pub annotations: Vec<Annotation>,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntityKind {
    /// `in [...]`, the attributes (a record; the JSON format allows another type, which the
    /// Cedar format cannot write), and `tags`.
    Standard { member_of: Vec<Name>, shape: Type, tags: Option<Type> },
    /// `enum ["a", "b"]`.
    Enum(Vec<String>),
}

/// `action "name" …;`: one for each name of a declaration that names several.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub name: String,
    /// `in [...]`; `None` when not written (an empty list is the JSON format's `[]`).
    pub member_of: Option<Vec<ActionRef>>,
    /// The Cedar format gives every action one, with empty lists when it writes none.
    pub applies_to: Option<AppliesTo>,
    pub annotations: Vec<Annotation>,
    pub line: usize,
    pub col: usize,
}

/// A parent action: `"id"` (of type `Action`) or `Type::"id"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionRef {
    pub ty: Option<Name>,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliesTo {
    pub principal_types: Vec<Name>,
    pub resource_types: Vec<Name>,
    /// A record (empty when not written) or a common type.
    pub context: Type,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    /// The JSON format's `{"type": "Long"}`.
    Long,
    String,
    /// `{"type": "Boolean"}`.
    Bool,
    Set(Box<Type>),
    Record(RecordType),
    /// `{"type": "Entity", "name": …}`.
    Entity(Name),
    /// A name the Cedar format writes (`Long`, `User`, `Address`): the JSON format's
    /// `{"type": "EntityOrCommon", "name": …}`, resolved when the schema is used.
    EntityOrCommon(Name),
    /// `{"type": "Extension", "name": …}`.
    Extension(String),
    /// `{"type": "<common type>"}`; the Cedar format makes one of a context written as a name.
    CommonRef(Name),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordType {
    pub attrs: Vec<Attr>,
    /// The JSON format's `additionalAttributes` (false unless written).
    pub additional_attributes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attr {
    pub name: String,
    pub ty: Type,
    pub required: bool,
    pub annotations: Vec<Annotation>,
    pub line: usize,
    pub col: usize,
}

impl Type {
    pub fn empty_record() -> Type {
        Type::Record(RecordType { attrs: vec![], additional_attributes: false })
    }

    pub fn is_empty_record(&self) -> bool {
        matches!(self, Type::Record(r) if r.attrs.is_empty() && !r.additional_attributes)
    }
}

const RESERVED: [&str; 9] = ["true", "false", "if", "then", "else", "in", "is", "like", "has"];
const COMMON_TYPE_KEYWORDS: [&str; 8] = ["Bool", "Boolean", "Entity", "Extension", "Long", "Record", "Set", "String"];

fn is_ident(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if c == '_' || c.is_ascii_alphabetic()) && cs.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

// ── The Cedar schema format ─────────────────────────────────────────────

type R<T> = Result<T, Error>;

struct P<'a> {
    t: &'a [Token],
    i: usize,
    lines: Lines<'a>,
    src: &'a str,
    depth: usize,
}

/// The declarations of one namespace as written: annotations, where, what.
type Decls = Vec<(Vec<Annotation>, usize, Decl)>;

/// A declaration as written, before it is split into one per name.
enum Decl {
    Entity { names: Vec<(String, usize)>, kind: EntityKind },
    Action { names: Vec<(String, usize)>, member_of: Option<Vec<ActionRef>>, applies_to: AppliesTo },
    Type { name: String, at: usize, ty: Type },
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<&'a Tok> {
        self.t.get(self.i).map(|t| &t.tok)
    }

    fn is_p(&self, p: &str) -> bool {
        matches!(self.peek(), Some(Tok::P(q)) if *q == p)
    }

    fn is_p_at(&self, k: usize, p: &str) -> bool {
        matches!(self.t.get(self.i + k).map(|t| &t.tok), Some(Tok::P(q)) if *q == p)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s == w)
    }

    fn at(&self) -> usize {
        self.t.get(self.i).map(|t| t.start).unwrap_or(self.src.len())
    }

    fn err_at(&self, i: usize, m: Text) -> Error {
        self.lines.err(i, m)
    }

    fn err_here(&self, what: Text) -> Error {
        match self.t.get(self.i) {
            Some(t) => {
                let shown = &self.src[t.start..t.end];
                self.err_at(t.start, tr!("ここには{}が要ります（`{shown}` があります）", "unexpected `{shown}`: {} goes here", what.ja; what.en))
            }
            None => self.err_at(self.src.len(), tr!("ファイルが途中で終わっています（ここには{}が要ります）", "the file ends too early: {} goes here", what.ja; what.en)),
        }
    }

    fn expect_p(&mut self, p: &str) -> R<()> {
        if self.is_p(p) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err_here(Text::new(format!(" `{p}` "), format!("`{p}`"))))
        }
    }

    fn any_ident(&mut self) -> R<(String, usize)> {
        match self.peek() {
            Some(Tok::Ident(w)) => {
                let at = self.at();
                self.i += 1;
                Ok((w.clone(), at))
            }
            _ => Err(self.err_here(tr!("識別子", "an identifier"))),
        }
    }

    /// An identifier that is not a reserved word (`Ident`).
    fn ident(&mut self) -> R<(String, usize)> {
        let (w, at) = self.any_ident()?;
        if RESERVED.contains(&w.as_str()) {
            return Err(self.err_at(at, tr!("`{w}` は予約語なので、名前に使えません", "`{w}` is a reserved identifier")));
        }
        Ok((w, at))
    }

    fn string(&mut self) -> R<(String, usize)> {
        match self.peek() {
            Some(Tok::Str(s)) => {
                let at = self.at();
                self.i += 1;
                let v = super::ast::unescape_text(s).map_err(|m| self.err_at(at, m))?;
                Ok((v, at))
            }
            _ => Err(self.err_here(tr!("文字列", "a string"))),
        }
    }

    fn path(&mut self) -> R<(Name, usize)> {
        let (first, at) = self.ident()?;
        let mut parts = vec![first];
        while self.is_p("::") && matches!(self.t.get(self.i + 1).map(|t| &t.tok), Some(Tok::Ident(_))) {
            self.i += 1;
            parts.push(self.ident()?.0);
        }
        let id = parts.pop().unwrap();
        Ok((Name { path: parts, id }, at))
    }

    fn annotations(&mut self) -> R<Vec<Annotation>> {
        let mut out: Vec<Annotation> = Vec::new();
        while self.is_p("@") {
            let at = self.at();
            self.i += 1;
            let (key, _) = self.any_ident()?;
            let value = if self.is_p("(") {
                self.i += 1;
                let (v, _) = self.string()?;
                self.expect_p(")")?;
                v
            } else {
                // the Cedar format's `@key` is the annotation `""`
                String::new()
            };
            if out.iter().any(|a| a.key == key) {
                return Err(self.err_at(at, tr!("注釈 `@{key}` が二度あります", "the annotation `@{key}` is written twice")));
            }
            let (line, col) = self.lines.at(at);
            out.push(Annotation { key, value: Some(value), line, col });
        }
        Ok(out)
    }

    fn ty(&mut self) -> R<Type> {
        self.depth += 1;
        if self.depth > 200 {
            return Err(self.err_at(self.at(), tr!("入れ子が深すぎます（200 段まで）", "nested deeper than 200 levels")));
        }
        let t = if self.is_word("Set") && self.is_p_at(1, "<") {
            self.i += 2;
            let inner = self.ty()?;
            self.expect_p(">")?;
            Type::Set(Box::new(inner))
        } else if self.is_p("{") {
            self.i += 1;
            let attrs = self.attr_decls()?;
            self.expect_p("}")?;
            Type::Record(RecordType { attrs, additional_attributes: false })
        } else {
            Type::EntityOrCommon(self.path()?.0)
        };
        self.depth -= 1;
        Ok(t)
    }

    /// `AttrDecls?` up to the closing brace: a later attribute of the same name replaces an
    /// earlier one, as Cedar's map keeps the last.
    fn attr_decls(&mut self) -> R<Vec<Attr>> {
        let mut out: Vec<Attr> = Vec::new();
        while !self.is_p("}") {
            let annotations = self.annotations()?;
            let at = self.at();
            let name = match self.peek() {
                Some(Tok::Str(_)) => self.string()?.0,
                _ => self.ident()?.0,
            };
            let required = if self.is_p("?") {
                self.i += 1;
                false
            } else {
                true
            };
            self.expect_p(":")?;
            let ty = self.ty()?;
            let (line, col) = self.lines.at(at);
            out.retain(|a| a.name != name);
            out.push(Attr { name, ty, required, annotations, line, col });
            if self.is_p(",") {
                self.i += 1;
            } else {
                break;
            }
        }
        Ok(out)
    }

    /// `Path` or `[Path, …]` (the list may be empty; no comma after the last).
    fn ent_types(&mut self) -> R<Vec<Name>> {
        if self.is_p("[") {
            self.i += 1;
            let mut out = Vec::new();
            if !self.is_p("]") {
                loop {
                    out.push(self.path()?.0);
                    if self.is_p(",") {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
            }
            self.expect_p("]")?;
            Ok(out)
        } else {
            Ok(vec![self.path()?.0])
        }
    }

    /// `Name` (an identifier or a string).
    fn name(&mut self) -> R<(String, usize)> {
        match self.peek() {
            Some(Tok::Str(_)) => self.string(),
            _ => self.ident(),
        }
    }

    fn qual_name(&mut self) -> R<ActionRef> {
        if let Some(Tok::Ident(_)) = self.peek() {
            let save = self.i;
            let (path, _) = self.path()?;
            if self.is_p("::") && matches!(self.t.get(self.i + 1).map(|t| &t.tok), Some(Tok::Str(_))) {
                self.i += 1;
                let (id, _) = self.string()?;
                return Ok(ActionRef { ty: Some(path), id });
            }
            self.i = save;
        }
        let (id, _) = self.name()?;
        Ok(ActionRef { ty: None, id })
    }

    fn decl(&mut self) -> R<Decl> {
        if self.is_word("entity") {
            self.i += 1;
            let mut names = vec![self.ident()?];
            while self.is_p(",") {
                self.i += 1;
                names.push(self.ident()?);
            }
            if self.is_word("enum") {
                self.i += 1;
                self.expect_p("[")?;
                let mut choices = vec![self.string()?.0];
                while self.is_p(",") {
                    self.i += 1;
                    choices.push(self.string()?.0);
                }
                self.expect_p("]")?;
                self.expect_p(";")?;
                return Ok(Decl::Entity { names, kind: EntityKind::Enum(choices) });
            }
            let member_of = if self.is_word("in") {
                self.i += 1;
                self.ent_types()?
            } else {
                vec![]
            };
            let mut shape = Type::empty_record();
            if self.is_p("=") || self.is_p("{") {
                if self.is_p("=") {
                    self.i += 1;
                }
                self.expect_p("{")?;
                let attrs = self.attr_decls()?;
                self.expect_p("}")?;
                shape = Type::Record(RecordType { attrs, additional_attributes: false });
            }
            let tags = if self.is_word("tags") {
                self.i += 1;
                Some(self.ty()?)
            } else {
                None
            };
            self.expect_p(";")?;
            return Ok(Decl::Entity { names, kind: EntityKind::Standard { member_of, shape, tags } });
        }
        if self.is_word("action") {
            let action_at = self.at();
            self.i += 1;
            let mut names = vec![self.name()?];
            while self.is_p(",") {
                self.i += 1;
                names.push(self.name()?);
            }
            let member_of = if self.is_word("in") {
                self.i += 1;
                if self.is_p("[") {
                    self.i += 1;
                    let mut v = vec![self.qual_name()?];
                    while self.is_p(",") {
                        self.i += 1;
                        v.push(self.qual_name()?);
                    }
                    self.expect_p("]")?;
                    Some(v)
                } else {
                    Some(vec![self.qual_name()?])
                }
            } else {
                None
            };
            let first = names[0].0.clone();
            let applies_to = if self.is_word("appliesTo") {
                self.i += 1;
                self.expect_p("{")?;
                let a = self.app_decls(&first, action_at)?;
                self.expect_p("}")?;
                a
            } else {
                AppliesTo { principal_types: vec![], resource_types: vec![], context: Type::empty_record() }
            };
            if self.is_word("attributes") {
                self.i += 1;
                self.expect_p("{")?;
                self.expect_p("}")?;
            }
            self.expect_p(";")?;
            return Ok(Decl::Action { names, member_of, applies_to });
        }
        if self.is_word("type") {
            self.i += 1;
            let (name, at) = self.ident()?;
            self.expect_p("=")?;
            let ty = self.ty()?;
            self.expect_p(";")?;
            return Ok(Decl::Type { name, at, ty });
        }
        Err(self.err_here(tr!(" `entity`、`action`、`type` のどれか", "`entity`, `action` or `type`")))
    }

    fn app_decls(&mut self, action: &str, action_at: usize) -> R<AppliesTo> {
        let mut principal: Option<Vec<Name>> = None;
        let mut resource: Option<Vec<Name>> = None;
        let mut context: Option<Type> = None;
        let mut first = true;
        loop {
            if !first && self.is_p("}") {
                break;
            }
            first = false;
            let at = self.at();
            if self.is_word("principal") || self.is_word("resource") {
                let is_principal = self.is_word("principal");
                self.i += 1;
                self.expect_p(":")?;
                let tys = self.ent_types()?;
                let which = if is_principal { "principal" } else { "resource" };
                let slot = if is_principal { &mut principal } else { &mut resource };
                if slot.is_some() {
                    return Err(self.err_at(at, tr!("action `{action}` に `{which}` が二度あります", "`{which}` is declared twice in action `{action}`")));
                }
                if tys.is_empty() {
                    return Err(self.err_at(at, tr!("action `{action}` の `{which}` が `[]` です。型を一つ以上書いてください", "`{which}` of action `{action}` is `[]`; it takes one type or more")));
                }
                *slot = Some(tys);
            } else if self.is_word("context") {
                self.i += 1;
                self.expect_p(":")?;
                let ty = if self.is_p("{") {
                    self.i += 1;
                    let attrs = self.attr_decls()?;
                    self.expect_p("}")?;
                    Type::Record(RecordType { attrs, additional_attributes: false })
                } else {
                    Type::CommonRef(self.path()?.0)
                };
                if context.is_some() {
                    return Err(self.err_at(at, tr!("action `{action}` に `context` が二度あります", "`context` is declared twice in action `{action}`")));
                }
                context = Some(ty);
            } else {
                return Err(self.err_here(tr!(" `principal`、`resource`、`context` のどれか", "`principal`, `resource` or `context`")));
            }
            if self.is_p(",") {
                self.i += 1;
            } else {
                break;
            }
        }
        let missing = |w: &str| self.err_at(action_at, tr!("action `{action}` に `{w}` がありません", "action `{action}` has no `{w}`"));
        let resource_types = resource.ok_or_else(|| missing("resource"))?;
        let principal_types = principal.ok_or_else(|| missing("principal"))?;
        Ok(AppliesTo { principal_types, resource_types, context: context.unwrap_or_else(Type::empty_record) })
    }
}

fn uses_cedar(n: &Name) -> bool {
    n.path.iter().chain(std::iter::once(&n.id)).any(|p| p == "__cedar")
}

/// The declarations of one namespace, split one per name, with the checks of the conversion.
fn fill(ns: &mut Namespace, lines: &Lines, decls: Decls) -> R<()> {
    for (annotations, _at, d) in decls {
        match d {
            Decl::Entity { names, kind } => {
                for (name, nat) in names {
                    if name == "__cedar" {
                        return Err(lines.err(nat, tr!("予約された名前空間 `__cedar` は使えません", "the `__cedar` namespace is reserved")));
                    }
                    if ns.entity_types.iter().any(|e| e.name == name) {
                        return Err(lines.err(nat, tr!("`{name}` が二度宣言されています", "`{name}` is declared twice")));
                    }
                    let (line, col) = lines.at(nat);
                    ns.entity_types.push(EntityType { name, kind: kind.clone(), annotations: annotations.clone(), line, col });
                }
            }
            Decl::Action { names, member_of, applies_to } => {
                for (name, nat) in names {
                    if ns.actions.iter().any(|a| a.name == name) {
                        return Err(lines.err(nat, tr!("`{name}` が二度宣言されています", "`{name}` is declared twice")));
                    }
                    let (line, col) = lines.at(nat);
                    ns.actions.push(Action { name, member_of: member_of.clone(), applies_to: Some(applies_to.clone()), annotations: annotations.clone(), line, col });
                }
            }
            Decl::Type { name, at: nat, ty } => {
                if name == "__cedar" {
                    return Err(lines.err(nat, tr!("予約された名前空間 `__cedar` は使えません", "the `__cedar` namespace is reserved")));
                }
                if COMMON_TYPE_KEYWORDS.contains(&name.as_str()) {
                    return Err(lines.err(nat, tr!("`{name}` はスキーマの予約語なので、共通の型の名前に使えません", "`{name}` is a reserved schema keyword")));
                }
                if ns.common_types.iter().any(|c| c.name == name) {
                    return Err(lines.err(nat, tr!("`{name}` が二度宣言されています", "`{name}` is declared twice")));
                }
                let (line, col) = lines.at(nat);
                ns.common_types.push(CommonType { name, ty, annotations, line, col });
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_cedar(src: &str) -> R<Schema> {
    let tokens = lex(src, Grammar::Schema)?;
    let mut p = P { t: &tokens, i: 0, lines: Lines::new(src), src, depth: 0 };
    let mut named: Vec<(Namespace, Decls)> = Vec::new();
    let mut loose: Decls = Vec::new();
    // where the unnamed namespace stands among the named ones: before the k-th
    let mut loose_at: Option<(usize, usize)> = None;
    while p.i < p.t.len() {
        let annotations = p.annotations()?;
        let at = p.at();
        if p.is_word("namespace") {
            p.i += 1;
            let (name, nat) = p.path()?;
            if uses_cedar(&name) {
                return Err(p.err_at(nat, tr!("予約された名前空間 `__cedar` は使えません", "the `__cedar` namespace is reserved")));
            }
            if named.iter().any(|(n, _)| n.name.as_ref() == Some(&name)) {
                let s = name.to_string();
                return Err(p.err_at(nat, tr!("名前空間 `{s}` が二度あります", "the namespace `{s}` is declared twice")));
            }
            p.expect_p("{")?;
            let mut decls = Vec::new();
            while !p.is_p("}") {
                let a = p.annotations()?;
                let dat = p.at();
                decls.push((a, dat, p.decl()?));
            }
            p.expect_p("}")?;
            let (line, col) = p.lines.at(nat);
            named.push((Namespace { name: Some(name), annotations, common_types: vec![], entity_types: vec![], actions: vec![], line, col }, decls));
        } else {
            if loose_at.is_none() {
                loose_at = Some((named.len(), at));
            }
            let d = p.decl()?;
            loose.push((annotations, at, d));
        }
    }
    let mut namespaces = Vec::new();
    for (mut ns, decls) in named {
        fill(&mut ns, &p.lines, decls)?;
        namespaces.push(ns);
    }
    if let Some((k, at)) = loose_at {
        let (line, col) = p.lines.at(at);
        let mut ns = Namespace { name: None, annotations: vec![], common_types: vec![], entity_types: vec![], actions: vec![], line, col };
        fill(&mut ns, &p.lines, loose)?;
        namespaces.insert(k, ns);
    }
    Ok(Schema { namespaces })
}

// ── The JSON schema format ──────────────────────────────────────────────

fn jerr(n: &Node, m: Text) -> Error {
    Error { line: n.line, col: n.col, message: m }
}

/// What a value of the JSON format is, in both languages. The Japanese takes a space on the
/// side where it meets the sentence with a name in backquotes or an English word.
fn w(ja: &str, en: &str) -> Text {
    let edge = |c: Option<char>| c.is_some_and(|c| c == '`' || c.is_ascii_alphanumeric());
    let lead = if edge(ja.chars().next()) { " " } else { "" };
    let trail = if edge(ja.chars().last()) { " " } else { "" };
    Text::new(format!("{lead}{ja}{trail}"), en)
}

/// A message whose Japanese starts with [`w`]'s space: without it.
fn trimmed(t: Text) -> Text {
    Text { ja: t.ja.trim_start().to_string(), en: t.en }
}

fn obj<'n>(n: &'n Node, what: &Text) -> R<&'n [(yaml::Key, Node)]> {
    n.as_map().ok_or_else(|| jerr(n, trimmed(tr!("{}はオブジェクトで書いてください", "{} is an object", what.ja; what.en))))
}

fn only(n: &Node, what: &Text, allowed: &[&str]) -> R<()> {
    for (k, _) in obj(n, what)? {
        if !allowed.contains(&k.name.as_str()) {
            let (key, ja_list, en_list) = (&k.name, allowed.join("`、`"), allowed.join("`, `"));
            return Err(Error { line: k.line, col: k.col, message: trimmed(tr!("{}にフィールド `{key}` は書けません（書けるのは `{ja_list}` です）", "{} has no field `{key}` (the fields are `{en_list}`)", what.ja; what.en)) });
        }
    }
    Ok(())
}

fn string(n: &Node, what: &Text) -> R<String> {
    n.as_str().map(str::to_string).ok_or_else(|| jerr(n, trimmed(tr!("{}は文字列で書いてください", "{} is a string", what.ja; what.en))))
}

fn arr<'n>(n: &'n Node, what: &Text) -> R<&'n [Node]> {
    n.as_seq().ok_or_else(|| jerr(n, trimmed(tr!("{}は配列で書いてください", "{} is an array", what.ja; what.en))))
}

/// A name in its normalized form (`A::B`): identifiers that are not reserved words.
fn norm_name(n: &Node, what: &Text, allow_cedar: bool) -> R<Name> {
    let s = string(n, what)?;
    let parts: Vec<&str> = s.split("::").collect();
    if parts.iter().any(|p| !is_ident(p) || RESERVED.contains(p)) || (!allow_cedar && parts.contains(&"__cedar")) {
        return Err(jerr(n, tr!("`{s}` は{}の名前として読めません", "`{s}` is not read as the name of {}", what.ja; what.en)));
    }
    Ok(Name::parse(&s))
}

fn annotations_json(n: Option<&Node>) -> R<Vec<Annotation>> {
    let Some(n) = n else { return Ok(vec![]) };
    let mut out = Vec::new();
    for (k, v) in obj(n, &w("注釈", "the annotations"))? {
        if !is_ident(&k.name) {
            let key = &k.name;
            return Err(Error { line: k.line, col: k.col, message: tr!("注釈の名前 `{key}` は識別子ではありません", "the annotation name `{key}` is not an identifier") });
        }
        let value = match &v.value {
            Value::Null => None,
            Value::Str(s) => Some(s.clone()),
            _ => return Err(jerr(v, tr!("注釈の値は文字列か null で書いてください", "the value of an annotation is a string or null"))),
        };
        out.push(Annotation { key: k.name.clone(), value, line: k.line, col: k.col });
    }
    Ok(out)
}

fn type_json(n: &Node, extra: &[&str]) -> R<Type> {
    let fields = obj(n, &w("型", "a type"))?;
    let mut allowed = vec!["type", "element", "attributes", "additionalAttributes", "name"];
    allowed.extend_from_slice(extra);
    only(n, &w("型", "a type"), &allowed)?;
    let Some(t) = n.get("type") else { return Err(jerr(n, tr!("型に `type` がありません", "a type has no `type`"))) };
    let t = string(t, &w("`type`", "`type`"))?;
    let has = |k: &str| fields.iter().any(|(x, _)| x.name == k);
    let forbid = |ks: &[&str]| -> R<()> {
        for k in ks {
            if has(k) {
                return Err(jerr(n, tr!("型 `{t}` にフィールド `{k}` は書けません", "a type `{t}` has no field `{k}`")));
            }
        }
        Ok(())
    };
    let need = |k: &str| -> R<&Node> { n.get(k).ok_or_else(|| jerr(n, tr!("型 `{t}` に `{k}` がありません", "a type `{t}` has no `{k}`"))) };
    Ok(match t.as_str() {
        "String" | "Long" | "Boolean" => {
            forbid(&["element", "attributes", "additionalAttributes", "name"])?;
            match t.as_str() {
                "String" => Type::String,
                "Long" => Type::Long,
                _ => Type::Bool,
            }
        }
        "Set" => {
            forbid(&["attributes", "additionalAttributes", "name"])?;
            Type::Set(Box::new(type_json(need("element")?, &[])?))
        }
        "Record" => {
            forbid(&["element", "name"])?;
            let attrs_node = need("attributes")?;
            let mut attrs = Vec::new();
            for (k, v) in obj(attrs_node, &w("`attributes`", "`attributes`"))? {
                obj(v, &w("属性", "an attribute"))?;
                let ty = type_json(v, &["annotations", "required"])?;
                let required = match v.get("required") {
                    None => true,
                    Some(Node { value: Value::Bool(b), .. }) => *b,
                    Some(o) => return Err(jerr(o, tr!("`required` は真偽値で書いてください", "`required` is a boolean"))),
                };
                attrs.push(Attr { name: k.name.clone(), ty, required, annotations: annotations_json(v.get("annotations"))?, line: k.line, col: k.col });
            }
            let additional_attributes = match n.get("additionalAttributes") {
                None => false,
                Some(Node { value: Value::Bool(b), .. }) => *b,
                Some(o) => return Err(jerr(o, tr!("`additionalAttributes` は真偽値で書いてください", "`additionalAttributes` is a boolean"))),
            };
            Type::Record(RecordType { attrs, additional_attributes })
        }
        "Entity" | "EntityOrCommon" => {
            forbid(&["element", "attributes", "additionalAttributes"])?;
            let name = norm_name(need("name")?, &w("型", "a type"), true)?;
            if t == "Entity" { Type::Entity(name) } else { Type::EntityOrCommon(name) }
        }
        "Extension" => {
            forbid(&["element", "attributes", "additionalAttributes"])?;
            let name_node = need("name")?;
            let name = string(name_node, &w("`name`", "`name`"))?;
            if !is_ident(&name) || RESERVED.contains(&name.as_str()) || name == "__cedar" {
                return Err(jerr(name_node, tr!("`{name}` は拡張の型の名前として読めません", "`{name}` is not read as the name of an extension type")));
            }
            Type::Extension(name)
        }
        _ => {
            forbid(&["element", "attributes", "additionalAttributes", "name"])?;
            Type::CommonRef(norm_name(n.get("type").unwrap(), &w("共通の型", "a common type"), true)?)
        }
    })
}

fn names_json(n: &Node, what: &Text) -> R<Vec<Name>> {
    arr(n, what)?.iter().map(|x| norm_name(x, &w("エンティティタイプ", "an entity type"), true)).collect()
}

pub(crate) fn parse_json(src: &str) -> R<Schema> {
    let root = yaml::read_json(src).map_err(|e| Error { line: e.line, col: e.col, message: e.message })?;
    let mut namespaces = Vec::new();
    for (k, def) in obj(&root, &w("スキーマ", "the schema"))? {
        let name = if k.name.is_empty() {
            None
        } else {
            let key_node = Node { value: Value::Str(k.name.clone()), line: k.line, col: k.col };
            Some(norm_name(&key_node, &w("名前空間", "a namespace"), false)?)
        };
        only(def, &w("名前空間", "a namespace"), &["commonTypes", "entityTypes", "actions", "annotations"])?;
        let annotations = annotations_json(def.get("annotations"))?;
        if name.is_none() && !annotations.is_empty() {
            return Err(jerr(def, tr!("名前の無い名前空間には注釈を付けられません", "annotations are not allowed on the empty namespace")));
        }
        let mut ns = Namespace { name, annotations, common_types: vec![], entity_types: vec![], actions: vec![], line: k.line, col: k.col };
        if let Some(cts) = def.get("commonTypes") {
            for (ck, cv) in obj(cts, &w("`commonTypes`", "`commonTypes`"))? {
                if !is_ident(&ck.name) || RESERVED.contains(&ck.name.as_str()) || ck.name == "__cedar" || COMMON_TYPE_KEYWORDS.contains(&ck.name.as_str()) {
                    let n = &ck.name;
                    return Err(Error { line: ck.line, col: ck.col, message: tr!("`{n}` は共通の型の名前に使えません", "`{n}` cannot name a common type") });
                }
                let ty = type_json(cv, &["annotations"])?;
                ns.common_types.push(CommonType { name: ck.name.clone(), ty, annotations: annotations_json(cv.get("annotations"))?, line: ck.line, col: ck.col });
            }
        }
        let ets = def.get("entityTypes").ok_or_else(|| jerr(def, tr!("名前空間に `entityTypes` がありません", "a namespace has no `entityTypes`")))?;
        for (ek, ev) in obj(ets, &w("`entityTypes`", "`entityTypes`"))? {
            if !is_ident(&ek.name) || RESERVED.contains(&ek.name.as_str()) || ek.name == "__cedar" {
                let n = &ek.name;
                return Err(Error { line: ek.line, col: ek.col, message: tr!("`{n}` はエンティティタイプの名前に使えません", "`{n}` cannot name an entity type") });
            }
            only(ev, &w("エンティティタイプ", "an entity type"), &["memberOfTypes", "shape", "tags", "enum", "annotations"])?;
            let annotations = annotations_json(ev.get("annotations"))?;
            let kind = match ev.get("enum") {
                Some(en) => {
                    for k in ["memberOfTypes", "shape", "tags"] {
                        if ev.get(k).is_some() {
                            return Err(jerr(ev, tr!("`enum` のエンティティタイプに `{k}` は書けません", "an entity type with `enum` has no `{k}`")));
                        }
                    }
                    let choices: Vec<String> = arr(en, &w("`enum`", "`enum`"))?.iter().map(|x| string(x, &w("`enum` の値", "a value of `enum`"))).collect::<R<_>>()?;
                    if choices.is_empty() {
                        return Err(jerr(en, tr!("`enum` が空です", "`enum` is empty")));
                    }
                    EntityKind::Enum(choices)
                }
                None => EntityKind::Standard {
                    member_of: match ev.get("memberOfTypes") {
                        Some(m) => names_json(m, &w("`memberOfTypes`", "`memberOfTypes`"))?,
                        None => vec![],
                    },
                    shape: match ev.get("shape") {
                        Some(s) => type_json(s, &[])?,
                        None => Type::empty_record(),
                    },
                    tags: match ev.get("tags") {
                        Some(t) => Some(type_json(t, &[])?),
                        None => None,
                    },
                },
            };
            ns.entity_types.push(EntityType { name: ek.name.clone(), kind, annotations, line: ek.line, col: ek.col });
        }
        let acts = def.get("actions").ok_or_else(|| jerr(def, tr!("名前空間に `actions` がありません", "a namespace has no `actions`")))?;
        for (ak, av) in obj(acts, &w("`actions`", "`actions`"))? {
            only(av, &w("action", "an action"), &["attributes", "appliesTo", "memberOf", "annotations"])?;
            if let Some(a) = av.get("attributes") {
                return Err(jerr(a, tr!("action の `attributes` は読みません（Cedar が受け付けない機能です）", "the `attributes` of an action are not read (Cedar does not support them)")));
            }
            let applies_to = match av.get("appliesTo") {
                Some(ap) => {
                    only(ap, &w("`appliesTo`", "`appliesTo`"), &["resourceTypes", "principalTypes", "context"])?;
                    let need = |k: &str| ap.get(k).ok_or_else(|| jerr(ap, tr!("`appliesTo` に `{k}` がありません", "`appliesTo` has no `{k}`")));
                    Some(AppliesTo {
                        resource_types: names_json(need("resourceTypes")?, &w("`resourceTypes`", "`resourceTypes`"))?,
                        principal_types: names_json(need("principalTypes")?, &w("`principalTypes`", "`principalTypes`"))?,
                        context: match ap.get("context") {
                            Some(c) => type_json(c, &[])?,
                            None => Type::empty_record(),
                        },
                    })
                }
                None => None,
            };
            let member_of = match av.get("memberOf") {
                Some(m) => {
                    let mut v = Vec::new();
                    for x in arr(m, &w("`memberOf`", "`memberOf`"))? {
                        only(x, &w("親の action", "a parent action"), &["id", "type"])?;
                        let id = string(x.get("id").ok_or_else(|| jerr(x, tr!("親の action に `id` がありません", "a parent action has no `id`")))?, &w("`id`", "`id`"))?;
                        let ty = match x.get("type") {
                            Some(t) => Some(norm_name(t, &w("action の型", "an action type"), true)?),
                            None => None,
                        };
                        v.push(ActionRef { ty, id });
                    }
                    Some(v)
                }
                None => None,
            };
            ns.actions.push(Action { name: ak.name.clone(), member_of, applies_to, annotations: annotations_json(av.get("annotations"))?, line: ak.line, col: ak.col });
        }
        namespaces.push(ns);
    }
    Ok(Schema { namespaces })
}

// ── Writing ─────────────────────────────────────────────────────────────

fn name_key(n: &Option<Name>) -> (bool, String, Vec<String>) {
    match n {
        None => (false, String::new(), vec![]),
        Some(n) => (true, n.id.clone(), n.path.clone()),
    }
}

/// The namespaces in the order of Cedar's map: the unnamed one first, then by the last part of
/// the name, then by the parts before it.
fn sorted_namespaces(s: &Schema) -> Vec<&Namespace> {
    let mut v: Vec<&Namespace> = s.namespaces.iter().collect();
    v.sort_by(|a, b| {
        let (ka, kb) = (name_key(&a.name), name_key(&b.name));
        ka.0.cmp(&kb.0).then_with(|| ka.1.as_bytes().cmp(kb.1.as_bytes())).then_with(|| {
            let pa: Vec<&[u8]> = ka.2.iter().map(|x| x.as_bytes()).collect();
            let pb: Vec<&[u8]> = kb.2.iter().map(|x| x.as_bytes()).collect();
            pa.cmp(&pb)
        })
    });
    v
}

fn by_name<T>(items: &[T], name: impl Fn(&T) -> &str) -> Vec<&T> {
    let mut v: Vec<&T> = items.iter().collect();
    v.sort_by(|a, b| name(a).as_bytes().cmp(name(b).as_bytes()));
    v
}

fn annotations_to_json(a: &[Annotation]) -> Json {
    Json::Obj(by_name(a, |x| &x.key).into_iter().map(|x| (x.key.clone(), x.value.clone().map(Json::Str).unwrap_or(Json::Null))).collect())
}

fn push_annotations(out: &mut Vec<(String, Json)>, a: &[Annotation]) {
    if !a.is_empty() {
        out.push(("annotations".to_string(), annotations_to_json(a)));
    }
}

fn type_fields(t: &Type) -> Vec<(String, Json)> {
    let s = |x: &str| Json::Str(x.to_string());
    match t {
        Type::Long => vec![("type".into(), s("Long"))],
        Type::String => vec![("type".into(), s("String"))],
        Type::Bool => vec![("type".into(), s("Boolean"))],
        Type::Set(e) => vec![("type".into(), s("Set")), ("element".into(), type_to_json(e))],
        Type::Record(r) => {
            let attrs = by_name(&r.attrs, |a| &a.name)
                .into_iter()
                .map(|a| {
                    let mut f = type_fields(&a.ty);
                    push_annotations(&mut f, &a.annotations);
                    if !a.required {
                        f.push(("required".into(), Json::Bool(false)));
                    }
                    (a.name.clone(), Json::Obj(f))
                })
                .collect();
            let mut f = vec![("type".into(), s("Record")), ("attributes".into(), Json::Obj(attrs))];
            if r.additional_attributes {
                f.push(("additionalAttributes".into(), Json::Bool(true)));
            }
            f
        }
        Type::Entity(n) => vec![("type".into(), s("Entity")), ("name".into(), Json::Str(n.to_string()))],
        Type::EntityOrCommon(n) => vec![("type".into(), s("EntityOrCommon")), ("name".into(), Json::Str(n.to_string()))],
        Type::Extension(n) => vec![("type".into(), s("Extension")), ("name".into(), s(n))],
        Type::CommonRef(n) => vec![("type".into(), Json::Str(n.to_string()))],
    }
}

fn type_to_json(t: &Type) -> Json {
    Json::Obj(type_fields(t))
}

fn names_to_json(ns: &[Name]) -> Json {
    Json::Arr(ns.iter().map(|n| Json::Str(n.to_string())).collect())
}

pub(crate) fn to_json(s: &Schema) -> Json {
    let mut out = Vec::new();
    for ns in sorted_namespaces(s) {
        let mut d = Vec::new();
        if !ns.common_types.is_empty() {
            let cts = by_name(&ns.common_types, |c| &c.name)
                .into_iter()
                .map(|c| {
                    let mut f = type_fields(&c.ty);
                    push_annotations(&mut f, &c.annotations);
                    (c.name.clone(), Json::Obj(f))
                })
                .collect();
            d.push(("commonTypes".to_string(), Json::Obj(cts)));
        }
        let ets = by_name(&ns.entity_types, |e| &e.name)
            .into_iter()
            .map(|e| {
                let mut f = Vec::new();
                match &e.kind {
                    EntityKind::Standard { member_of, shape, tags } => {
                        if !member_of.is_empty() {
                            f.push(("memberOfTypes".to_string(), names_to_json(member_of)));
                        }
                        if !shape.is_empty_record() {
                            f.push(("shape".to_string(), type_to_json(shape)));
                        }
                        if let Some(t) = tags {
                            f.push(("tags".to_string(), type_to_json(t)));
                        }
                    }
                    EntityKind::Enum(choices) => f.push(("enum".to_string(), Json::Arr(choices.iter().cloned().map(Json::Str).collect()))),
                }
                push_annotations(&mut f, &e.annotations);
                (e.name.clone(), Json::Obj(f))
            })
            .collect();
        d.push(("entityTypes".to_string(), Json::Obj(ets)));
        let acts = by_name(&ns.actions, |a| &a.name)
            .into_iter()
            .map(|a| {
                let mut f = Vec::new();
                if let Some(ap) = &a.applies_to {
                    let mut g = vec![("resourceTypes".to_string(), names_to_json(&ap.resource_types)), ("principalTypes".to_string(), names_to_json(&ap.principal_types))];
                    if !ap.context.is_empty_record() {
                        g.push(("context".to_string(), type_to_json(&ap.context)));
                    }
                    f.push(("appliesTo".to_string(), Json::Obj(g)));
                }
                if let Some(m) = &a.member_of {
                    let refs = m
                        .iter()
                        .map(|r| {
                            let mut o = vec![("id".to_string(), Json::Str(r.id.clone()))];
                            if let Some(t) = &r.ty {
                                o.push(("type".to_string(), Json::Str(t.to_string())));
                            }
                            Json::Obj(o)
                        })
                        .collect();
                    f.push(("memberOf".to_string(), Json::Arr(refs)));
                }
                push_annotations(&mut f, &a.annotations);
                (a.name.clone(), Json::Obj(f))
            })
            .collect();
        d.push(("actions".to_string(), Json::Obj(acts)));
        push_annotations(&mut d, &ns.annotations);
        let key = ns.name.as_ref().map(|n| n.to_string()).unwrap_or_default();
        out.push((key, Json::Obj(d)));
    }
    Json::Obj(out)
}

/// A string as Rust's `str::escape_debug` writes it, which the Cedar schema writer uses: `\t`,
/// `\r`, `\n`, `\\`, `"`, `'` and `\0` escaped, characters that do not print as `\u{…}`, and a
/// combining mark at the very start too.
pub(crate) fn escape_debug(s: &str) -> String {
    let mut o = String::new();
    for (i, c) in s.chars().enumerate() {
        let u = c as u32;
        match c {
            '\0' => o.push_str("\\0"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            '\n' => o.push_str("\\n"),
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\'' => o.push_str("\\'"),
            _ if c.is_control()
                || matches!(u, 0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x180E | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB | 0xE000..=0xF8FF | 0xFFFE | 0xFFFF)
                || (i == 0 && matches!(u, 0x300..=0x36F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x3099 | 0x309A)) =>
            {
                o.push_str(&format!("\\u{{{u:x}}}"))
            }
            c => o.push(c),
        }
    }
    o
}

fn annotations_text(a: &[Annotation], indent: &str, o: &mut String) {
    for x in by_name(a, |x| &x.key) {
        match &x.value {
            Some(v) => o.push_str(&format!("{indent}@{}(\"{}\")\n", x.key, escape_debug(v))),
            None => o.push_str(&format!("{indent}@{}\n", x.key)),
        }
    }
}

fn is_normalized_ident(s: &str) -> bool {
    is_ident(s) && !RESERVED.contains(&s) && s != "__cedar"
}

fn type_text(t: &Type, base: &str) -> String {
    match t {
        Type::Bool => "__cedar::Bool".to_string(),
        Type::Long => "__cedar::Long".to_string(),
        Type::String => "__cedar::String".to_string(),
        Type::Entity(n) | Type::EntityOrCommon(n) | Type::CommonRef(n) => n.to_string(),
        Type::Extension(n) => format!("__cedar::{n}"),
        Type::Set(e) => format!("Set<{}>", type_text(e, base)),
        Type::Record(r) => {
            let member = format!("{base}  ");
            let mut o = String::from("{");
            let attrs = by_name(&r.attrs, |a| &a.name);
            for (i, a) in attrs.iter().enumerate() {
                if i == 0 {
                    o.push('\n');
                }
                annotations_text(&a.annotations, &member, &mut o);
                let name = if is_normalized_ident(&a.name) { a.name.clone() } else { format!("\"{}\"", escape_debug(&a.name)) };
                o.push_str(&format!("{member}{name}{}: {}{}\n", if a.required { "" } else { "?" }, type_text(&a.ty, &member), if i + 1 < attrs.len() { "," } else { "" }));
            }
            o.push_str(base);
            o.push('}');
            o
        }
    }
}

fn list(ns: &[String]) -> String {
    format!("[{}]", ns.join(", "))
}

fn namespace_text(ns: &Namespace, base: &str, o: &mut String) {
    let total = ns.common_types.len() + ns.entity_types.len() + ns.actions.len();
    let mut k = 0;
    let mut sep = |o: &mut String| {
        if k + 1 < total {
            o.push('\n');
        }
        k += 1;
    };
    for c in by_name(&ns.common_types, |c| &c.name) {
        annotations_text(&c.annotations, base, o);
        o.push_str(&format!("{base}type {} = {};\n", c.name, type_text(&c.ty, base)));
        sep(o);
    }
    for e in by_name(&ns.entity_types, |e| &e.name) {
        annotations_text(&e.annotations, base, o);
        let mut body = String::new();
        match &e.kind {
            EntityKind::Standard { member_of, shape, tags } => {
                if !member_of.is_empty() {
                    body.push_str(&format!(" in {}", list(&member_of.iter().map(|n| n.to_string()).collect::<Vec<_>>())));
                }
                if !shape.is_empty_record() {
                    body.push_str(&format!(" = {}", type_text(shape, base)));
                }
                if let Some(t) = tags {
                    body.push_str(&format!(" tags {}", type_text(t, "")));
                }
            }
            EntityKind::Enum(choices) => {
                body.push_str(&format!(" enum [{}]", choices.iter().map(|c| format!("\"{}\"", escape_debug(c))).collect::<Vec<_>>().join(", ")));
            }
        }
        o.push_str(&format!("{base}entity {}{body};\n", e.name));
        sep(o);
    }
    for a in by_name(&ns.actions, |a| &a.name) {
        annotations_text(&a.annotations, base, o);
        let mut body = String::new();
        if let Some(m) = a.member_of.as_ref().filter(|m| !m.is_empty()) {
            let refs: Vec<String> = m.iter().map(|r| format!("{}::\"{}\"", r.ty.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "Action".to_string()), escape_debug(&r.id))).collect();
            body.push_str(&format!(" in {}", list(&refs)));
        }
        if let Some(ap) = &a.applies_to
            && !ap.principal_types.is_empty()
            && !ap.resource_types.is_empty()
        {
            let member = format!("{base}  ");
            let names = |v: &[Name]| list(&v.iter().map(|n| n.to_string()).collect::<Vec<_>>());
            body.push_str(&format!(" appliesTo {{\n{member}principal: {},\n{member}resource: {}", names(&ap.principal_types), names(&ap.resource_types)));
            if ap.context.is_empty_record() {
                body.push_str(&format!(",\n{member}context: {{}}"));
            } else {
                body.push_str(&format!(",\n{member}context: {}", type_text(&ap.context, &member)));
            }
            body.push_str(&format!("\n{base}}}"));
        }
        o.push_str(&format!("{base}action \"{}\"{body};\n", escape_debug(&a.name)));
        sep(o);
    }
}

pub(crate) fn write(s: &Schema) -> R<String> {
    // what the Cedar format cannot say: an entity type and a common type of one name in a
    // named namespace, and an entity's shape that is not a record
    for ns in &s.namespaces {
        if let Some(name) = &ns.name {
            for e in &ns.entity_types {
                if ns.common_types.iter().any(|c| c.name == e.name) {
                    let n = format!("{name}::{}", e.name);
                    return Err(Error { line: e.line, col: e.col, message: tr!("`{n}` が、エンティティタイプと共通の型の両方の名前になっています。人が読む形（`.cedarschema`）では書けません", "`{n}` names both an entity type and a common type, which the Cedar schema format cannot write") });
                }
            }
        }
        for e in &ns.entity_types {
            if let EntityKind::Standard { shape, .. } = &e.kind
                && !matches!(shape, Type::Record(_))
            {
                let n = &e.name;
                return Err(Error { line: e.line, col: e.col, message: tr!("エンティティタイプ `{n}` の shape がレコードではありません。人が読む形（`.cedarschema`）では書けません", "the shape of entity type `{n}` is not a record, which the Cedar schema format cannot write") });
            }
        }
    }
    let nss = sorted_namespaces(s);
    let mut o = String::new();
    for (i, ns) in nss.iter().enumerate() {
        match &ns.name {
            None => namespace_text(ns, "", &mut o),
            Some(name) => {
                annotations_text(&ns.annotations, "", &mut o);
                o.push_str(&format!("namespace {name} {{\n"));
                namespace_text(ns, "  ", &mut o);
                o.push_str("}\n");
            }
        }
        if i + 1 < nss.len() {
            o.push('\n');
        }
    }
    Ok(o)
}
