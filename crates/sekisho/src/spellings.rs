//! The names of a gate that the code of a target writes as an identifier it cannot take (W402 when
//! the file is checked, E401 when `gen` writes the target; DESIGN 24, 32).
//!
//! The module of each language declares the records of the types it reads, the enums, its own types
//! (`Store`, `Request`, …) and the input of each action, at its top level, and Go a field for each
//! attribute, the roles and the groups in each record. Each language spells a name its own way
//! (`order_state` is `OrderState`, `refund` has the input `RefundInput`), so two names of the gate
//! can be one identifier, one can be a name of the module's or one it reads from elsewhere (`Date`,
//! `Literal`), and one can be a reserved word of the language where the code writes it as a name:
//! Python writes an attribute as a field of its record and reads a type with a method of `Store`
//! (`class`, `return`), and the name of the file names Python's module, Go's package, and the
//! module an index of `ritsu gen`'s exports in TypeScript (`pass`, `select`, `interface`). A
//! generator renames none of them: a call is written with the names `check` read. Each form breaks
//! some of the three languages, and the others and Cedar stay as they are, so `check` warns, and
//! `gen` of a language it breaks stops. Types and enums of one name are E006, and a reserved word of
//! Cedar's, which every target writes, is E008, before any of this. Every other name of a gate is
//! written where a reserved word is a name too (a property, a method, a string) or with more around
//! it (`authorizeDelete`, `in_class`), and Go writes it with a capital.

use crate::cedar::Shape;
use crate::check::Outcome;
use crate::checks::Checked;
use crate::diag::Diag;
use crate::model::Gate;
use crate::names::Scope;
use crate::r#gen::{go, plan, python, typescript};
use ritsu_base::text::Text;

/// What a declared identifier is spelled from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum From {
    Type(usize),
    Enum(usize),
    /// the input of an action
    Input(usize),
    /// an attribute of a type, in its record
    Attr(usize, usize),
    /// a name the module declares for itself, or reads from elsewhere
    Own,
    /// a reserved word of the language, which cannot be a name where the code writes it
    Word,
    /// the name of the file (its alias), which names the module and the package of the code
    File,
}

/// One identifier the code of a language declares.
#[derive(Clone, Debug)]
pub struct Declared {
    pub id: String,
    pub from: From,
}

impl Declared {
    pub fn new(id: String, from: From) -> Declared {
        Declared { id, from }
    }
}

/// The three languages, as `--target` names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    TypeScript,
    Python,
    Go,
}

impl Language {
    const ALL: [Language; 3] = [Language::TypeScript, Language::Python, Language::Go];

    fn target(self) -> &'static str {
        match self {
            Language::TypeScript => "typescript",
            Language::Python => "python",
            Language::Go => "go",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Language::TypeScript => "TypeScript",
            Language::Python => "Python",
            Language::Go => "Go",
        }
    }

    /// What a reserved word written as a name does.
    fn kept(self) -> Text {
        match self {
            Language::TypeScript => tr!("モジュールを読み込めず、型の検査を通りません", "the module does not parse, and does not type-check"),
            Language::Python => tr!("モジュールを読み込めず（`SyntaxError`）、`mypy --strict` も通りません", "the module does not load (`SyntaxError`), nor pass `mypy --strict`"),
            Language::Go => tr!("パッケージをコンパイルできません（`syntax error`）", "the package does not compile (`syntax error`)"),
        }
    }

    /// What a second declaration of one name does.
    fn twice(self) -> Text {
        match self {
            Language::TypeScript => tr!(
                "同じ名前の型は宣言できないので型の検査を通らず（`Duplicate identifier`）、二つの interface なら一つに混ざります",
                "a name cannot be declared twice as a type, so the module does not type-check (`Duplicate identifier`), and two interfaces become one"
            ),
            Language::Python => tr!(
                "後の宣言が前の宣言を置き換えるので、前の宣言を読むコードが後の宣言を読み、`mypy --strict` を通りません（`already defined`）",
                "the later declaration takes the place of the earlier, so what reads the earlier reads the later, and the module does not pass `mypy --strict` (`already defined`)"
            ),
            Language::Go => tr!("パッケージをコンパイルできません（`redeclared in this block`）", "the package does not compile (`redeclared in this block`)"),
        }
    }
}

/// One thing a language says of a finding.
#[derive(Clone, Debug)]
pub struct Said {
    pub target: &'static str,
    pub note: Text,
}

/// Two names that are one identifier, or a name that is the code's own: the first [`From`] is of the
/// gate, and so is the second, or it is [`From::Own`].
#[derive(Clone, Debug)]
pub struct Found {
    pub first: From,
    pub second: From,
    /// the identifier, in the language that says it first
    pub id: String,
    pub said: Vec<Said>,
}

impl Found {
    /// The targets it breaks.
    pub fn targets(&self) -> Vec<&'static str> {
        self.said.iter().map(|s| s.target).collect()
    }
}

fn what(g: &Gate, f: From) -> Text {
    match f {
        From::Type(t) => {
            let a = &g.types[t].named.alias;
            tr!("型 `{a}`", "the type `{a}`")
        }
        From::Enum(e) => {
            let a = &g.enums[e].named.alias;
            tr!("列挙 `{a}`", "the enum `{a}`")
        }
        From::Input(i) => {
            let a = &g.actions[i].named.alias;
            tr!("action `{a}` の入力", "the input of the action `{a}`")
        }
        From::Attr(t, i) => {
            let (a, ty) = (&g.types[t].attrs[i].named.alias, &g.types[t].named.alias);
            tr!("型 `{ty}` の属性 `{a}`", "the attribute `{a}` of the type `{ty}`")
        }
        From::Own => tr!("生成するコード", "the generated code"),
        From::Word => tr!("言語の予約語", "a reserved word of the language"),
        From::File => {
            let a = &g.named.alias;
            tr!("ファイル `{a}`", "the file `{a}`")
        }
    }
}

fn line_of(g: &Gate, f: From) -> usize {
    match f {
        From::Type(t) => g.types[t].named.line,
        From::Enum(e) => g.enums[e].named.line,
        From::Input(i) => g.actions[i].named.line,
        From::Attr(t, i) => g.types[t].attrs[i].named.line,
        From::File => g.named.line,
        From::Own | From::Word => 0,
    }
}

/// What one language says of two gate's names that are one identifier.
fn two(lang: Language, g: &Gate, x: From, y: From, id: &str) -> Text {
    let (a, b, broke, l, t) = (what(g, x), what(g, y), lang.twice(), lang.title(), lang.target());
    let place = match x {
        From::Attr(ty, _) => {
            let ty = &g.types[ty].named.alias;
            tr!("型 `{ty}` のレコードの", "in the record of the type `{ty}`, ")
        }
        _ => tr!("", ""),
    };
    tr!(
        "{l}（`--target {t}`）: {}{} と {} を、どちらも `{id}` と書きます。{}。",
        "{l} (`--target {t}`): {}{} and {} are both written `{id}`: {}.",
        place.ja, a.ja, b.ja, broke.ja;
        place.en, a.en, b.en, broke.en
    )
}

/// What one language says of a gate's name that is one the code declares itself.
fn own(lang: Language, g: &Gate, x: From, id: &str) -> Text {
    let (a, broke, l, t) = (what(g, x), lang.twice(), lang.title(), lang.target());
    if matches!(x, From::Attr(..)) {
        return tr!(
            "{l}（`--target {t}`）: {} を `{id}` と書きますが、レコードにはコードが使う同じ名前のフィールドもあります（役割かグループを持つフィールド）。{}。",
            "{l} (`--target {t}`): {} is written `{id}`, and the record has a field of that name the code uses (for the roles, or a group): {}.",
            a.ja, broke.ja;
            a.en, broke.en
        );
    }
    tr!(
        "{l}（`--target {t}`）: {} を `{id}` と書きますが、コードは同じ名前を自分でも宣言します。{}。",
        "{l} (`--target {t}`): {} is written `{id}`, and the code declares that name itself: {}.",
        a.ja, broke.ja;
        a.en, broke.en
    )
}

/// Where a target writes the names of one list of [`Declared`]: what a reserved word among them is
/// written as.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// at the top level of the module
    Top,
    /// as a field of the record of a type
    Field,
    /// as the method of `Store` that reads a type
    Method,
    /// as the name of the module: Python's file, and the module an index of `ritsu gen`'s exports in
    /// TypeScript
    Module,
    /// as the name of Go's package
    Package,
}

/// What one language says of a gate's name that it writes as one of its reserved words.
fn kept(lang: Language, g: &Gate, x: From, id: &str, place: Place) -> Text {
    let (a, l, t) = (what(g, x), lang.title(), lang.target());
    let (as_ja, as_en) = match place {
        Place::Top => ("", ""),
        Place::Field => ("レコードのフィールドとして", " as a field of its record"),
        Place::Method => ("読む `Store` のメソッドとして", " as the method of `Store` that reads it"),
        Place::Module => ("モジュールの名前として", " as the name of its module"),
        Place::Package => ("パッケージの名前として", " as the name of its package"),
    };
    let (word, broke) = match (place, lang) {
        // the module itself parses; what reads it does not (DESIGN 32)
        (Place::Module, Language::TypeScript) => (
            tr!("TypeScript の strict モードの予約語", "a reserved word of TypeScript's strict mode"),
            tr!(
                "`ritsu gen` がパッケージに書く索引の `export * as {id} from …` が、型の検査を通りません",
                "the index `ritsu gen` writes in a package, `export * as {id} from …`, does not type-check"
            ),
        ),
        (Place::Module, _) => (
            tr!("{l} の予約語", "a reserved word of {l}"),
            tr!(
                "`import` でモジュール（`authz/{id}.py`）を読めず、`ritsu gen` がパッケージに書く `__init__.py` の `from . import {id} as {id}` も読み込めません（`SyntaxError`）",
                "no `import` can read the module (`authz/{id}.py`), and the `__init__.py` `ritsu gen` writes in a package, `from . import {id} as {id}`, does not load (`SyntaxError`)"
            ),
        ),
        _ => (tr!("{l} の予約語", "a reserved word of {l}"), lang.kept()),
    };
    tr!(
        "{l}（`--target {t}`）: {} を{as_ja} `{id}` と書きますが、`{id}` は {}です。{}。",
        "{l} (`--target {t}`): {} is written `{id}`{as_en}, and `{id}` is {}: {}.",
        a.ja, word.ja, broke.ja;
        a.en, word.en, broke.en
    )
}

/// What one language says of a gate's name that is one the code reads from elsewhere.
fn global(lang: Language, g: &Gate, x: From, id: &str) -> Text {
    let a = what(g, x);
    match lang {
        Language::TypeScript => tr!(
            "TypeScript（`--target typescript`）: {} を `{id}` と書きますが、モジュールはグローバルの `{id}` か、読み込む同じ名前のものを使います。宣言がそれを隠すので、型の検査を通りません。",
            "TypeScript (`--target typescript`): {} is written `{id}`, and the module uses the global `{id}`, or what it imports of that name: the declaration hides it, and the module does not type-check.",
            a.ja;
            a.en
        ),
        _ => tr!(
            "Python（`--target python`）: {} を `{id}` と書きますが、`{id}` はモジュールが読み込む名前です。宣言がそれを隠すので、`mypy --strict` を通らず、呼び出しが止まります。",
            "Python (`--target python`): {} is written `{id}`, and `{id}` is what the module imports: the declaration hides it, and the module does not pass `mypy --strict` and a call stops.",
            a.ja;
            a.en
        ),
    }
}

/// What the names of one place come to: each pair of one identifier, the second from the gate or
/// the code's own; and each name that `taken` holds.
fn hits(decls: &[Declared], taken: impl Fn(&str) -> bool) -> (Vec<(From, From, String)>, Vec<(From, String)>) {
    let mut out = Vec::new();
    for (i, x) in decls.iter().enumerate() {
        for y in &decls[i + 1..] {
            if x.id == y.id && !(x.from == From::Own && y.from == From::Own) {
                let (a, b) = if x.from == From::Own { (y.from, x.from) } else { (x.from, y.from) };
                out.push((a, b, x.id.clone()));
            }
        }
    }
    let globals = decls.iter().filter(|d| d.from != From::Own && taken(&d.id)).map(|d| (d.from, d.id.clone())).collect();
    (out, globals)
}

fn add(found: &mut Vec<Found>, first: From, second: From, id: &str, lang: Language, note: Text) {
    let said = Said { target: lang.target(), note };
    match found.iter_mut().find(|f| f.first == first && f.second == second) {
        Some(f) => f.said.push(said),
        None => found.push(Found { first, second, id: id.to_string(), said: vec![said] }),
    }
}

/// What every language says of a gate, each finding once, by its place in the file.
pub fn find(g: &Gate, shape: &Shape) -> Vec<Found> {
    let mut found = Vec::new();
    let mut read = |lang: Language, decls: &[Declared], taken: &dyn Fn(&str) -> bool, place: Place, kept_word: &dyn Fn(&str) -> bool| {
        let (pairs, globals) = hits(decls, taken);
        for (a, b, id) in pairs {
            let note = if b == From::Own { own(lang, g, a, &id) } else { two(lang, g, a, b, &id) };
            add(&mut found, a, b, &id, lang, note);
        }
        for (a, id) in globals {
            add(&mut found, a, From::Own, &id, lang, global(lang, g, a, &id));
        }
        for d in decls.iter().filter(|d| d.from != From::Own && kept_word(&d.id)) {
            add(&mut found, d.from, From::Word, &d.id, lang, kept(lang, g, d.from, &d.id, place));
        }
    };
    read(Language::TypeScript, &typescript::declared(g, shape), &typescript::global, Place::Top, &typescript::reserved);
    read(Language::Python, &python::declared(g, shape), &python::imported, Place::Top, &python::keyword);
    read(Language::Go, &go::declared(g, shape), &|_| false, Place::Top, &go::keyword);
    let read_types: Vec<usize> = plan::types_used(g).into_iter().filter(|&t| plan::reads(g, shape, t)).collect();
    for &t in &read_types {
        read(Language::Go, &go::declared_fields(g, shape, t), &|_| false, Place::Field, &go::keyword);
        read(Language::Python, &python::declared_fields(g, t), &|_| false, Place::Field, &python::keyword);
    }
    read(Language::Python, &python::store_methods(g, &read_types), &|_| false, Place::Method, &python::keyword);
    // the name of the file, as each language names its module or its package (DESIGN 32)
    let file = |id: String| [Declared::new(id, From::File)];
    read(Language::TypeScript, &file(g.named.alias.clone()), &|_| false, Place::Module, &typescript::index_word);
    read(Language::Python, &file(g.named.alias.clone()), &|_| false, Place::Module, &python::keyword);
    read(Language::Go, &file(ritsu_emit::ident::go_package(&g.named.alias)), &|_| false, Place::Package, &go::keyword);
    found.sort_by_key(|f| (line_of(g, f.first).max(line_of(g, f.second)), f.first, f.second));
    found
}

fn headline(g: &Gate, f: &Found, place: Text) -> Text {
    let (a, id) = (what(g, f.first), &f.id);
    if f.second == From::Word {
        tr!(
            "{} は、{}で `{id}` と書かれ、その言語の予約語です",
            "{} is written `{id}` in {}, a reserved word of the language",
            a.ja, place.ja;
            a.en, place.en
        )
    } else if f.second == From::Own {
        tr!(
            "{} は、{}で `{id}` と書かれ、コードが自分で使う名前と同じです",
            "{} is written `{id}` in {}, a name the code uses itself",
            a.ja, place.ja;
            a.en, place.en
        )
    } else {
        let b = what(g, f.second);
        tr!(
            "{} と {} は、{}で同じ `{id}` になります",
            "{} and {} are one name, `{id}`, in {}",
            a.ja, b.ja, place.ja;
            a.en, b.en, place.en
        )
    }
}

fn fix(f: &Found, at_build: bool) -> Text {
    let rename = if matches!(f.second, From::Own | From::Word) {
        tr!("その別名を変えてください", "change its alias")
    } else {
        tr!("どちらかの別名を変えてください", "change the alias of one of the two")
    };
    let then = if f.first == From::File {
        tr!(
            "ファイルの別名は、生成するファイルとモジュールとパッケージの名前でもあるので、それを読むコードも合わせて直してください。",
            "The alias of the file also names the files, the module and the package the code is written to, so change what reads them with it."
        )
    } else {
        tr!(
            "別名は Cedar の名前でもあるので、すでに使っているゲートなら、変えたあと、使っている版と `sekisho diff` で比べてください。",
            "An alias is also the name in Cedar, so if the gate is in use, compare the changed file with the version in use with `sekisho diff`."
        )
    };
    if at_build {
        tr!("{}。{}", "Please {}. {}", rename.ja, then.ja; rename.en, then.en)
    } else {
        tr!(
            "使わない出力先なら、このままで構いません。使うなら、{}。{}",
            "For a target you do not build, leave it as it is. For one you do, {}. {}",
            rename.ja, then.ja;
            rename.en, then.en
        )
    }
}

fn line_for(g: &Gate, f: &Found) -> usize {
    line_of(g, f.first).max(line_of(g, f.second))
}

/// W402: what every target's code would break on, as `check` says it.
pub fn warnings(scope: &Scope, checked: &Checked) -> Vec<Diag> {
    let g = &checked.gate;
    let shape = crate::cedar::shape(g, scope, checked);
    find(g, &shape)
        .iter()
        .map(|f| {
            let langs: Vec<&str> = Language::ALL.into_iter().filter(|l| f.targets().contains(&l.target())).map(|l| l.title()).collect();
            let ja = langs.join("、");
            let en = match langs.as_slice() {
                [init @ .., last] if !init.is_empty() => format!("{} and {last}", init.join(", ")),
                _ => langs.join(""),
            };
            let place = tr!("{ja} のコード", "the code of {en}");
            let mut d = Diag::warning("W402", &g.file, line_for(g, f), 1, headline(g, f, place)).source(&g.src);
            for s in &f.said {
                d = d.note(s.note.clone());
            }
            d.note(fix(f, false))
        })
        .collect()
}

/// E401: what the code of `target` would break on, for a file that passes its check; nothing for a
/// target that writes no such code (`cedar`).
pub fn stops(o: &Outcome, target: &str) -> Vec<Diag> {
    let (Some(scope), Some(checked)) = (o.scope.as_ref(), o.walked.as_ref()) else { return Vec::new() };
    let g = &checked.gate;
    let shape = crate::cedar::shape(g, scope, checked);
    let place = tr!("`--target {target}` のコード", "the code of `--target {target}`");
    find(g, &shape)
        .iter()
        .filter(|f| f.targets().contains(&target))
        .map(|f| {
            let mut d = Diag::error("E401", &g.file, line_for(g, f), 1, headline(g, f, place.clone())).source(&g.src);
            for s in f.said.iter().filter(|s| s.target == target) {
                d = d.note(s.note.clone());
            }
            d.note(fix(f, true))
        })
        .collect()
}
