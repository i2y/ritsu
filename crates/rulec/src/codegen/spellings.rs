//! Two names the author wrote that the generated code of some language spells as one identifier,
//! and a name some language cannot take where its generated code puts it (§15.263).
//!
//! The generators write the identifier of a name — its alias, or the name itself where it has
//! none — each in its own way: as it is, in lowerCamelCase (Swift and Java), in PascalCase (Go's
//! fields), with an escape of their own (Java's and PHP's `_`), in upper case (an enum's values),
//! and the columns of the SQL query are read by SQLite with case set aside and by PostgreSQL as far
//! as 63 bytes. Two identifiers that differ as written can be one in such a spelling, and a
//! language can refuse an identifier for where it lands: Python's `NamedTuple` takes no field that
//! starts with `_`. (Ruby reads an identifier that starts with a capital as a constant; the Ruby
//! generator binds such a value under `_` and the name, §15.336.)
//!
//! §15.260 keeps every identifier rulec makes apart from all of these. The author's own are said
//! here, by W121, with the languages where they meet: the author chose them, so they are left as
//! written, as every alias is. Where every language that writes a kind of name writes it in one
//! spelling — an enum's type and its values in PascalCase, the functions of a prerequisites block in
//! snake_case, the element types of two sequences in PascalCase — two names that meet there are one
//! identifier everywhere, and `names::check` stops at them with E034 ([`enum_type`], [`member`],
//! [`block`], [`element_type`]).

use super::{camel, pascal, pg_head, pub_name, rb_const, snake, SQL_ROW};
use crate::ast::{Item, Name, RuleFile};
use crate::diag::{Diag, Span};
use crate::types::{Checked, Ty};

/// The PascalCase every language that names an enum's type gives it: `tier_level` and `tierLevel`
/// are both `TierLevel`.
pub fn enum_type(id: &str) -> String {
    pascal(id)
}

/// The PascalCase a value of an enum is written from: `Basic` in Rust and Go, `BASIC` where the
/// members are in upper case, `basic` in Swift. `basic` and `Basic` are both `Basic`.
pub fn member(id: &str) -> String {
    pascal(id)
}

/// The snake_case the functions of a prerequisites block are named after (`<it>_requires`,
/// `<it>_available`), which TypeScript, JavaScript, Swift and Java write in lowerCamelCase and Go
/// in PascalCase: `core_path` and `corePath` are both `core_path`.
pub fn block(id: &str) -> String {
    snake(id)
}

/// The type one element of a sequence is written as when the rule walks more than one
/// (`Gen::element_type`): `order_lines` and `orderLines` are both `OrderLinesElement`.
pub fn element_type(id: &str) -> String {
    format!("{}Element", pascal(id))
}

/// A field's JSON name in the `.proto`, as protobuf derives it (`ToJsonName`): each `_` dropped and
/// the character after it in upper case. buf and protoc refuse two fields of a message with one.
fn json_name(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut up = false;
    for c in id.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// The targets a W121 of this module names, in the order `rulec gen` writes them, with Connect —
/// the `.proto` every language's stubs are built from — last.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Target {
    Python,
    TypeScript,
    JavaScript,
    Rust,
    Ruby,
    Php,
    Go,
    Swift,
    Java,
    Sqlite,
    Postgres,
    Wasm,
    Connect,
}

impl Target {
    pub(super) fn name(self) -> String {
        match self {
            Target::Python => "Python".into(),
            Target::TypeScript => "TypeScript".into(),
            Target::JavaScript => "JavaScript".into(),
            Target::Rust => "Rust".into(),
            Target::Ruby => "Ruby".into(),
            Target::Php => "PHP".into(),
            Target::Go => "Go".into(),
            Target::Swift => "Swift".into(),
            Target::Java => "Java".into(),
            Target::Sqlite => tr!("SQL（SQLite）", "SQL (SQLite)"),
            Target::Postgres => tr!("SQL（PostgreSQL）", "SQL (PostgreSQL)"),
            Target::Wasm => "Wasm".into(),
            Target::Connect => "Connect".into(),
        }
    }

    /// The names `--target` and `--extra` give the code of the target, which `rulec gen` and
    /// `ritsu gen` do not write where a W121 names it (`Diag::targets`, §15.336): SQL's two dialects
    /// are one target, and Connect's service is the Python module it runs and, where `wire` says
    /// what the `.proto` declares, the `.proto` as well.
    pub(super) fn ids(self, wire: bool) -> &'static [&'static str] {
        match self {
            Target::Python => &["python"],
            Target::TypeScript => &["typescript"],
            Target::JavaScript => &["javascript"],
            Target::Rust => &["rust"],
            Target::Ruby => &["ruby"],
            Target::Php => &["php"],
            Target::Go => &["go"],
            Target::Swift => &["swift"],
            Target::Java => &["java"],
            Target::Sqlite | Target::Postgres => &["sql"],
            Target::Wasm => &["wasm"],
            Target::Connect if wire => &["proto", "connect"],
            Target::Connect => &["connect"],
        }
    }
}

/// [`Target::ids`] of each target, each once.
fn ids_of(ts: &[Target], wire: bool) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for t in ts {
        for id in t.ids(wire) {
            if !out.contains(id) {
                out.push(id);
            }
        }
    }
    out
}

/// The targets as the prose lists them: `A・B・C` in Japanese, `A, B and C` in English.
pub(super) fn listed(ts: &[Target]) -> String {
    let ns: Vec<String> = ts.iter().map(|t| t.name()).collect();
    if crate::i18n::ja() {
        return ns.join("・");
    }
    match ns.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => ns.join(""),
    }
}

/// A name the author declared, as the generated code writes it.
#[derive(Clone)]
struct Decl<'a> {
    name: &'a Name,
    /// The author's identifier: the alias, else the name.
    id: String,
    /// What the name is, for its mark: `入力`, `input`.
    what: String,
}

/// Two names some languages spell as one identifier, with a note for each spelling they meet in.
struct Meeting<'a> {
    first: Decl<'a>,
    second: Decl<'a>,
    notes: Vec<(Vec<Target>, String)>,
    /// Connect's note is about the `.proto`, which buf and protoc refuse: the `.proto` is not
    /// written either ([`Target::ids`]).
    wire: bool,
}

impl Meeting<'_> {
    fn targets(&self) -> Vec<Target> {
        let mut ts: Vec<Target> = self.notes.iter().flat_map(|(t, _)| t.iter().copied()).collect();
        ts.sort();
        ts.dedup();
        ts
    }
}

/// The meetings found so far, and where to add one more spelling to; the second is set while the
/// spellings met are the `.proto`'s.
#[derive(Default)]
struct Meetings<'a>(Vec<Meeting<'a>>, bool);

impl<'a> Meetings<'a> {
    /// Every two names of `decls` that `targets` spell alike, with `why` saying why there: each later
    /// name against the first with its spelling, as `names::check` says two names of one identifier.
    /// Two names of one identifier as written are E034's, and not said again here. `why` is given the
    /// two identifiers and their spelling.
    fn meet(&mut self, decls: &[Decl<'a>], targets: &[Target], spell: impl Fn(&str) -> String, why: impl Fn(&str, &str, &str) -> String) {
        self.meet_where(decls, targets, spell, why, |_, _| true);
    }

    /// [`Meetings::meet`], for the two names `keep` lets through.
    fn meet_where(&mut self, decls: &[Decl<'a>], targets: &[Target], spell: impl Fn(&str) -> String, why: impl Fn(&str, &str, &str) -> String, keep: impl Fn(&Decl, &Decl) -> bool) {
        let mut seen: Vec<(String, usize)> = Vec::new();
        for (i, d) in decls.iter().enumerate() {
            let s = spell(&d.id);
            match seen.iter().find(|(k, _)| *k == s) {
                Some((_, j)) if decls[*j].id != d.id => {
                    let first = &decls[*j];
                    if keep(first, d) {
                        self.add(first, d, targets, why(&first.id, &d.id, &s));
                    }
                }
                Some(_) => {}
                None => seen.push((s, i)),
            }
        }
    }

    /// One more spelling two names meet in: the targets not said for the two already, and why.
    fn add(&mut self, first: &Decl<'a>, second: &Decl<'a>, targets: &[Target], why: String) {
        let at = |d: &Decl| (d.name.span.line, d.name.span.col, d.name.text.clone());
        let i = match self.0.iter().position(|m| at(&m.first) == at(first) && at(&m.second) == at(second)) {
            Some(i) => i,
            None => {
                self.0.push(Meeting { first: first.clone(), second: second.clone(), notes: Vec::new(), wire: false });
                self.0.len() - 1
            }
        };
        let wire = self.1;
        let m = &mut self.0[i];
        let said = m.targets();
        let new: Vec<Target> = targets.iter().copied().filter(|t| !said.contains(t)).collect();
        if !new.is_empty() {
            m.wire |= wire && new.contains(&Target::Connect);
            let note = format!("{}: {why}", listed(&new));
            m.notes.push((new, note));
        }
    }
}

/// Where a name's mark points and what it says: the name, or its alias.
fn label(d: &Decl) -> String {
    if d.name.ascii.is_some() {
        tr!("{}の別名", "the {}'s alias", d.what)
    } else {
        tr!("{}の名前", "the {}'s name", d.what)
    }
}

/// The order the findings are said in: the second name's place, then the first's, the rule's own
/// file before a file merged into it.
fn place(s: &Span) -> (bool, String, usize, usize) {
    (s.file_path().is_some(), s.file_path().unwrap_or_default().to_string(), s.line, s.col)
}

/// The values one function of the generated code holds as parameters and variables, each once, in
/// the order declared: the inputs, the sequences, the outputs, and what the items decide. A name
/// whose identifier rulec makes (§15.210, §15.260) is rulec's to keep apart, and is not here.
fn values<'a>(f: &'a RuleFile, made: &super::Made) -> Vec<Decl<'a>> {
    let mut occurrences: Vec<(&'a Name, String)> = Vec::new();
    occurrences.extend(f.inputs.iter().map(|i| (&i.name, tr!("入力", "input"))));
    occurrences.extend(f.elements.iter().map(|el| (&el.name, tr!("並び", "sequence"))));
    occurrences.extend(f.outputs.iter().map(|o| (&o.name, tr!("出力", "output"))));
    for it in &f.items {
        match it {
            Item::Derived(d) => occurrences.push((&d.name, tr!("導出", "derived value"))),
            Item::Define(d) => occurrences.push((&d.name, tr!("定義", "definition"))),
            Item::Agg(d) => occurrences.push((&d.name, tr!("まとめ", "summary"))),
            Item::Fold(d) => occurrences.extend(d.answer.iter().map(|a| (&a.name, tr!("畳み込みの答え", "fold's answer")))),
            Item::Table(t) => occurrences.extend(t.outputs.iter().map(|o| (&o.name, tr!("表の出力", "table output")))),
        }
    }
    let mut out: Vec<Decl<'a>> = Vec::new();
    for (n, what) in &occurrences {
        if super::brought_in(&n.text) || made.values.contains_key(&n.text) || out.iter().any(|d| d.name.text == n.text) {
            continue;
        }
        // The alias of whichever line of the name writes one: a value has one (E034, §15.262).
        let id = occurrences.iter().filter(|(m, _)| m.text == n.text).find_map(|(m, _)| m.ascii.clone()).unwrap_or_else(|| n.text.clone());
        out.push(Decl { name: n, id, what: what.clone() });
    }
    out.sort_by_key(|d| place(&d.name.span));
    out
}

/// A name of a kind rulec does not rename (an enum, a value of one, a group, a shape, a block, a
/// table): its alias, else the name.
fn plain<'a>(n: &'a Name, what: String) -> Decl<'a> {
    Decl { name: n, id: pub_name(n), what }
}

/// The enums the `.proto` declares: those of an input, a field of an element or an output
/// (`connect::wire_enums`).
fn wire_enums(f: &RuleFile, c: &Checked) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let names = f.inputs.iter().map(|i| &i.name).chain(f.elements.iter().flat_map(|el| el.fields.iter().map(|fd| &fd.name))).chain(f.outputs.iter().map(|o| &o.name));
    for n in names {
        if let Some(Ty::Enum(e)) = c.ty_of(&n.text).map(|t| t.present().clone()) {
            if !out.contains(&e) {
                out.push(e);
            }
        }
    }
    out
}

/// The column of the SQL query that holds the number of the row a table matched, as
/// `sql::sql_row_col` names it: `<table>_row`, with `_` after it while it is the identifier or the
/// name of a value.
fn row_column(id: &str, f: &RuleFile, taken: &[String]) -> String {
    let mut n = format!("{id}{SQL_ROW}");
    while taken.contains(&n) || super::value_names(f).iter().any(|(m, _)| m.text == n) {
        n.push('_');
    }
    n
}

/// Every W121 of the spellings: two names the author wrote that the generated code of some
/// language writes as one identifier, and a name some language cannot take where its code puts it.
pub fn warnings(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let made = super::made_identifiers(f);
    let at = |line: usize| format!("{path}:{line}");
    let mut ms = Meetings::default();
    let vs = values(f, &made);
    // SQL and NumPy write no rule that walks a sequence (§15.56).
    let sql = f.elements.is_empty();

    // The values: the parameters and the variables of one function.
    ms.meet(&vs, &[Target::Swift, Target::Java], camel, |_, _, s| {
        tr!(
            "値の名前を lowerCamelCase で書くので、どちらも `{s}` です。一つの関数に同じ名前の引数や変数が二つになり、組めないか、片方がもう片方を隠します。",
            "a value is written in lowerCamelCase, so both are `{s}`: one function declares one name twice, which does not compile, or one hides the other."
        )
    });
    ms.meet(&vs, &[Target::Java], super::java::java_name_pub, |_, _, s| {
        tr!(
            "Java の予約語と同じ名前には `_` を付けて書くので、どちらも `{s}` です。一つの関数に同じ名前の引数や変数が二つになり、組めません。",
            "a name that is a keyword of Java is written with `_` after it, so both are `{s}`: one function declares one name twice, which does not compile."
        )
    });
    ms.meet(&vs, &[Target::Php], super::php::php_var_pub, |_, _, s| {
        tr!(
            "PHP が使う変数（`$this` とスーパーグローバル）と同じ名前には `_` を付けて書くので、どちらも `{s}` です。一つの関数で同じ変数になり、片方がもう片方を上書きします。",
            "a name PHP keeps for a variable of its own (`$this` and the superglobals) is written with `_` after it, so both are `{s}`: they are one variable, and one overwrites the other."
        )
    });
    if sql {
        ms.meet(&vs, &[Target::Sqlite], |s| s.to_ascii_lowercase(), |a, b, _| {
            tr!(
                "値は問い合わせの列になり、SQLite は列の名前の大文字と小文字を区別しません。`{a}` と `{b}` は一つの列として読まれ、問い合わせは片方の値で答えます。",
                "a value is a column of the query, and SQLite reads the name of a column with case set aside, so `{a}` and `{b}` are one column, and the query answers with one of the two values."
            )
        });
        ms.meet(&vs, &[Target::Postgres], |s| pg_head(s).to_string(), |_, _, s| {
            tr!(
                "値は問い合わせと関数の列になり、PostgreSQL は名前の先頭の 63 バイトしか残さないので、どちらも `{s}` です。PostgreSQL では、問い合わせも関数も、どちらの列か決められずに止まります。",
                "a value is a column of the query and of the function, and PostgreSQL keeps the first 63 bytes of a name, so both are `{s}`: on PostgreSQL the query and the function stop at a column they cannot tell apart."
            )
        });
    }

    // The inputs and the sequences: the fields of Go's `Input` and of the `.proto`'s request.
    let passed: Vec<Decl> = vs.iter().filter(|d| f.inputs.iter().any(|i| i.name.text == d.name.text) || f.elements.iter().any(|el| el.name.text == d.name.text)).cloned().collect();
    ms.meet(&passed, &[Target::Go], pascal, |_, _, s| {
        tr!(
            "入力と並びは `Input` 構造体のフィールドになり、名前を PascalCase で書くので、どちらも `{s}` です。一つの構造体に同じ名前のフィールドは置けないので、コンパイルできません。",
            "the inputs and the sequences are the fields of the `Input` struct, in PascalCase, so both are `{s}`, and a struct cannot hold two fields of one name: it does not compile."
        )
    });
    ms.1 = true;
    ms.meet(&passed, &[Target::Connect], json_name, |_, _, s| {
        tr!(
            "入力と並びは `.proto` の `DecideRequest` のフィールドになります。フィールドの JSON の名前は、`_` を除いてその次の字を大文字にしたもので、どちらも `{s}` です。buf と protoc は、JSON の名前が同じ二つのフィールドをエラーにします。",
            "the inputs and the sequences are the fields of `DecideRequest` in the `.proto`, and a field's JSON name drops each `_` and capitalizes the letter after it, so both are `{s}`: buf and protoc refuse two fields of one JSON name."
        )
    });
    ms.1 = false;

    // The outputs: the fields of Go's `Output` and of the `.proto`'s answer. PHP's `Output` holds
    // them under the names of their variables, which the values above were held to.
    let returned: Vec<Decl> = vs.iter().filter(|d| f.outputs.iter().any(|o| o.name.text == d.name.text)).cloned().collect();
    ms.meet(&returned, &[Target::Go], pascal, |_, _, s| {
        tr!(
            "出力は `Output` 構造体のフィールドになり、名前を PascalCase で書くので、どちらも `{s}` です。一つの構造体に同じ名前のフィールドは置けないので、コンパイルできません。",
            "the outputs are the fields of the `Output` struct, in PascalCase, so both are `{s}`, and a struct cannot hold two fields of one name: it does not compile."
        )
    });
    ms.1 = true;
    ms.meet(&returned, &[Target::Connect], json_name, |_, _, s| {
        tr!(
            "出力は `.proto` の `DecideResponse` のフィールドになります。フィールドの JSON の名前は、`_` を除いてその次の字を大文字にしたもので、どちらも `{s}` です。buf と protoc は、JSON の名前が同じ二つのフィールドをエラーにします。",
            "the outputs are the fields of `DecideResponse` in the `.proto`, and a field's JSON name drops each `_` and capitalizes the letter after it, so both are `{s}`: buf and protoc refuse two fields of one JSON name."
        )
    });
    ms.1 = false;

    // The fields of one element: a struct, a record, a message and a class of their own.
    for el in &f.elements {
        let ty = if f.elements.len() <= 1 { "Element".to_string() } else { element_type(&pub_name(&el.name)) };
        let fields: Vec<Decl> = el
            .fields
            .iter()
            .filter(|fd| !super::brought_in(&fd.name.text) && !made.values.contains_key(&fd.name.text))
            .map(|fd| Decl { name: &fd.name, id: pub_name(&fd.name), what: tr!("要素のフィールド", "element field") })
            .collect();
        ms.meet(&fields, &[Target::Swift, Target::Java], camel, |_, _, s| {
            tr!(
                "要素のフィールドは `{ty}` のフィールドになり、名前を lowerCamelCase で書くので、どちらも `{s}` です。一つの型に同じ名前のフィールドは置けないので、組めません。",
                "the fields of an element are the fields of `{ty}`, in lowerCamelCase, so both are `{s}`, and a type cannot hold two fields of one name: it does not compile."
            )
        });
        ms.meet(&fields, &[Target::Java], super::java::java_name_pub, |_, _, s| {
            tr!(
                "要素のフィールドは `{ty}` のフィールドになり、Java の予約語と同じ名前には `_` を付けて書くので、どちらも `{s}` です。一つの型に同じ名前のフィールドは置けないので、組めません。",
                "the fields of an element are the fields of `{ty}`, and a name that is a keyword of Java is written with `_` after it, so both are `{s}`, and a type cannot hold two fields of one name: it does not compile."
            )
        });
        ms.meet(&fields, &[Target::Go], pascal, |_, _, s| {
            tr!(
                "要素のフィールドは `{ty}` 構造体のフィールドになり、名前を PascalCase で書くので、どちらも `{s}` です。一つの構造体に同じ名前のフィールドは置けないので、コンパイルできません。",
                "the fields of an element are the fields of the `{ty}` struct, in PascalCase, so both are `{s}`, and a struct cannot hold two fields of one name: it does not compile."
            )
        });
        ms.meet(&fields, &[Target::Php], super::php::php_prop, |_, _, s| {
            tr!(
                "要素のフィールドは `{ty}` のプロパティになり、PHP が使う変数（`$this` とスーパーグローバル）と同じ名前には `_` を付けて書くので、どちらも `{s}` です。一つのクラスに同じ名前のプロパティは置けないので、読み込めません。",
                "the fields of an element are the properties of `{ty}`, and a name PHP keeps for a variable of its own (`$this` and the superglobals) is written with `_` after it, so both are `{s}`, and a class cannot hold two properties of one name: the file does not load."
            )
        });
        ms.1 = true;
        ms.meet(&fields, &[Target::Connect], json_name, |_, _, s| {
            tr!(
                "要素のフィールドは `.proto` の `{ty}` のフィールドになります。フィールドの JSON の名前は、`_` を除いてその次の字を大文字にしたもので、どちらも `{s}` です。buf と protoc は、JSON の名前が同じ二つのフィールドをエラーにします。",
                "the fields of an element are the fields of `{ty}` in the `.proto`, and a field's JSON name drops each `_` and capitalizes the letter after it, so both are `{s}`: buf and protoc refuse two fields of one JSON name."
            )
        });
        ms.1 = false;
    }

    // The enums the rule declares itself: their types, and the values of each. One read from an
    // `enums` file is that file's, and is said where that file is checked.
    let own: Vec<&crate::ast::EnumDecl> = f.enums.iter().filter(|e| e.from.is_none() && !super::brought_in(&e.name.text)).collect();
    let types: Vec<Decl> = own.iter().map(|e| plain(&e.name, tr!("列挙", "enum"))).collect();
    // Two of one PascalCase are one type in every language, and `names::check` stopped at them.
    ms.meet(&types, &[Target::Php], |s| enum_type(s).to_ascii_lowercase(), |a, b, _| {
        let (a, b) = (enum_type(a), enum_type(b));
        tr!(
            "列挙の型は `{a}` と `{b}` ですが、PHP はクラスや列挙の名前の大文字と小文字を区別しないので、同じ列挙を二度宣言することになり、読み込めません。",
            "the enums' types are `{a}` and `{b}`, and PHP reads the names of classes and enums with case set aside, so one enum is declared twice: the file does not load."
        )
    });
    let class = super::java::java_class(&pub_name(&f.name));
    ms.meet(&types, &[Target::Java], |s| enum_type(s).to_ascii_lowercase(), |a, b, _| {
        let (a, b) = (enum_type(a), enum_type(b));
        tr!(
            "列挙の型は規則のクラスの中に置かれ、javac は二つを `{class}${a}.class` と `{class}${b}.class` に書きます。大文字と小文字を区別しないファイルシステム（macOS と Windows の既定）では二つのファイルが一つになり、片方の型が読み込めません。",
            "an enum's type is nested in the rule's class, and javac writes the two to `{class}${a}.class` and `{class}${b}.class`: on a file system that sets case aside (the default of macOS and Windows) the two files are one, and one of the types does not load."
        )
    });
    // The `.proto` names the values of an enum after its type in UPPER_SNAKE_CASE, and protobuf
    // puts the values of every enum of the package beside one another (the scoping of C++): two
    // wire enums whose types are one in UPPER_SNAKE_CASE share their zero value
    // `<TYPE>_UNSPECIFIED`, and every value of one name. Two such types are one with case set aside,
    // which the meetings above have said for PHP and Java.
    let wire = wire_enums(f, c);
    let wire_types: Vec<Decl> = own.iter().filter(|e| wire.contains(&e.name.text)).map(|e| plain(&e.name, tr!("列挙", "enum"))).collect();
    ms.1 = true;
    ms.meet(&wire_types, &[Target::Connect], |s| crate::proto::upper_snake(&enum_type(s)), |_, _, s| {
        tr!(
            "`.proto` は列挙の値を、型の名前を UPPER_SNAKE_CASE にしたものを頭に付けて書き、パッケージのどの列挙の値も一つの場所に並べます。二つの頭はどちらも `{s}` なので、値が設定されていないことを表す `{s}_UNSPECIFIED` が二度になり、buf と protoc がエラーにします。",
            "the `.proto` names the values of an enum after its type in UPPER_SNAKE_CASE, and puts the values of every enum of the package in one place: both are `{s}`, so the zero value `{s}_UNSPECIFIED`, which a field that is not set holds, is declared twice, which buf and protoc refuse."
        )
    });
    ms.1 = false;
    for e in &own {
        let vals: Vec<Decl> = e.values.iter().map(|v| plain(v, tr!("列挙の値", "enum value"))).collect();
        let upper = |s: &str| member(s).to_uppercase();
        ms.meet(&vals, &[Target::Python, Target::TypeScript, Target::JavaScript, Target::Java], upper, |_, _, s| {
            tr!(
                "列挙の値の名前を大文字で書くので、どちらも `{s}` です。一つの列挙に同じ名前の値が二つになり、読み込めないか、片方がもう片方を隠します。",
                "an enum's values are written in upper case, so both are `{s}`: one enum declares one name twice, which does not load, or one hides the other."
            )
        });
        // The service of Connect runs the Python module. An enum on the wire is said below, by
        // the `.proto` that declares it as well.
        if !wire.contains(&e.name.text) {
            ms.meet(&vals, &[Target::Connect], upper, |_, _, s| {
                tr!(
                    "サービスは Python のモジュールを動かします。そのモジュールは列挙の値の名前を大文字で書くので、どちらも `{s}` になり、読み込めません。",
                    "the service runs the Python module, which writes an enum's values in upper case, so both are `{s}` there: the module does not load."
                )
            });
        }
        ms.meet(&vals, &[Target::Ruby], |s| rb_const(&upper(s)), |_, _, s| {
            tr!(
                "列挙の値の名前を定数として大文字で書くので、どちらも `{s}` です。一つのモジュールに同じ名前の定数が二つになり、片方がもう片方を上書きします。",
                "an enum's values are constants in upper case, so both are `{s}`: one module assigns one constant twice, and one overwrites the other."
            )
        });
        ms.meet(&vals, &[Target::Php], |s| super::php::php_name(&upper(s)), |_, _, s| {
            tr!(
                "列挙の値の名前を大文字で書くので、どちらも `{s}` です。一つの列挙に同じ名前の値は置けないので、読み込めません。",
                "an enum's values are written in upper case, so both are `{s}`, and an enum cannot hold two cases of one name: the file does not load."
            )
        });
        ms.meet(&vals, &[Target::Swift], |s| camel(&member(s)), |_, _, s| {
            tr!(
                "列挙の値の名前を lowerCamelCase で書くので、どちらも `{s}` です。一つの列挙に同じ名前の値は置けないので、組めません。",
                "an enum's values are written in lowerCamelCase, so both are `{s}`, and an enum cannot hold two cases of one name: it does not compile."
            )
        });
        if wire.contains(&e.name.text) {
            let prefix = crate::proto::upper_snake(&enum_type(&pub_name(&e.name)));
            ms.1 = true;
            ms.meet(&vals, &[Target::Connect], |s| format!("{prefix}_{}", upper(s)), |_, _, s| {
                tr!(
                    "`.proto` の列挙の値の名前は、列挙の名前と値の名前を大文字にしてつないだもので、どちらも `{s}` です。buf と protoc は、同じ名前の値をエラーにします。",
                    "a value of an enum in the `.proto` is named by the enum's name and the value's in upper case, so both are `{s}`, and buf and protoc refuse two values of one name."
                )
            });
            ms.1 = false;
        }
    }

    // Go's package of the rule, where the types of the enums and their members stand side by side:
    // a member is the type's name and the value's, joined (`TierGold`), so it can meet a member of
    // another enum (`a`'s `b_c` and `a_b`'s `c` are both `ABC`) or another enum's type.
    // What each is: the type, the function that reads a value of it (`ParseTier`), or a member,
    // with the enum it is of. An enum the rule reads from an `enums` file is handed on there under
    // the file's names (`type PlanGold = sides.PlanGold`, its constants and its `Parse`), but for a
    // type the package names by the file's package where it would meet the rule's own (§15.239), and
    // is said with the file it is read from (§15.336).
    #[derive(Clone, Copy, PartialEq)]
    enum Go {
        Type,
        Parse,
        Member,
    }
    // The enums read from a file come first, at the line that reads them, so that the name a meeting
    // says second, the one to give an alias, is the rule's own.
    let mut go: Vec<(String, Decl, Go, usize, Option<&str>)> = Vec::new();
    let rule_names = super::go_rule_names(f);
    for (k, e) in f.enums.iter().enumerate().filter(|(_, e)| !super::brought_in(&e.name.text)) {
        let Some(o) = &e.from else { continue };
        let k = own.len() + k;
        let t = enum_type(&pub_name(&o.name));
        let file = Some(o.file.as_str());
        if !rule_names.contains(&t) {
            go.push((t.clone(), plain(&e.name, tr!("列挙", "enum")), Go::Type, k, file));
        }
        go.push((format!("Parse{t}"), plain(&e.name, tr!("列挙", "enum")), Go::Parse, k, file));
        for v in &e.values {
            go.push((format!("{t}{}", member(&pub_name(v))), plain(v, tr!("列挙の値", "enum value")), Go::Member, k, file));
        }
    }
    for (k, e) in own.iter().enumerate() {
        let t = enum_type(&pub_name(&e.name));
        go.push((t.clone(), plain(&e.name, tr!("列挙", "enum")), Go::Type, k, None));
        go.push((format!("Parse{t}"), plain(&e.name, tr!("列挙", "enum")), Go::Parse, k, None));
        for v in &e.values {
            go.push((format!("{t}{}", member(&pub_name(v))), plain(v, tr!("列挙の値", "enum value")), Go::Member, k, None));
        }
    }
    let mut first_of: Vec<(String, usize)> = Vec::new();
    for (i, (s, d, kind, k, file)) in go.iter().enumerate() {
        match first_of.iter().find(|(x, _)| x == s) {
            // Two types, and two values of one enum, are E034's (`names::check`).
            Some((_, j)) if !(go[*j].2 == Go::Type && *kind == Go::Type) && !(go[*j].2 == Go::Member && *kind == Go::Member && go[*j].3 == *k) => {
                let first = go[*j].1.clone();
                let what = |x: &Decl, g: Go, file: Option<&str>| match (g, file) {
                    (Go::Type, None) => tr!("列挙 `{}` の型", "the type of the enum `{}`", x.name.text),
                    (Go::Parse, None) => tr!("列挙 `{}` を読む関数", "the function that reads the enum `{}`", x.name.text),
                    (Go::Member, None) => tr!("列挙の値 `{}`", "the enum value `{}`", x.name.text),
                    (Go::Type, Some(p)) => tr!("`{p}` から読む列挙 `{}` の型（規則のパッケージが同じ名前で渡す）", "the type of the enum `{}` of `{p}` (the rule's package hands it on under its name)", x.name.text),
                    (Go::Parse, Some(p)) => tr!("`{p}` から読む列挙 `{}` を読む関数（規則のパッケージが同じ名前で渡す）", "the function that reads the enum `{}` of `{p}` (the rule's package hands it on under its name)", x.name.text),
                    (Go::Member, Some(p)) => tr!("`{p}` から読む列挙の値 `{}`（規則のパッケージが同じ名前で渡す）", "the enum value `{}` of `{p}` (the rule's package hands it on under its name)", x.name.text),
                };
                let (a, b) = (what(&first, go[*j].2, go[*j].4), what(d, *kind, *file));
                ms.add(
                    &first,
                    d,
                    &[Target::Go],
                    tr!(
                        "列挙の値は、列挙の型の名前と値の名前を PascalCase でつないだ定数で、型と、値を読む関数（`Parse<型>`）と同じく、規則のパッケージに並びます。{a} と{b} は、どちらも `{s}` になり、一つのパッケージに同じ名前は二つ置けないので、コンパイルできません。",
                        "a value of an enum is a constant named by the enum's type and the value's name in PascalCase, joined, and stands in the rule's package beside the types and the functions that read a value (`Parse<Type>`): {a} and {b} are both `{s}`, and a package holds one thing of a name, so it does not compile."
                    ),
                );
            }
            Some(_) => {}
            None => first_of.push((s.clone(), i)),
        }
    }

    // The groups the rule declares itself: a function or a set of their own.
    let groups: Vec<Decl> = f.groups.iter().filter(|g| g.from.is_none()).map(|g| plain(&g.name, tr!("グループ", "group"))).collect();
    ms.meet(&groups, &[Target::Go], |s| format!("is{}", pascal(s)), |_, _, s| {
        tr!(
            "グループは、値がそのグループにあるかを確かめる関数になり、その名前は `is` のあとにグループの名前を PascalCase で書くので、どちらも `{s}` です。同じ名前の関数を二度宣言することになり、コンパイルできません。",
            "a group is a function that asks whether a value is in it, named `is` followed by the group's name in PascalCase, so both are `{s}`: one function is declared twice, which does not compile."
        )
    });
    ms.meet(&groups, &[Target::Swift], |s| format!("_{}", camel(s)), |_, _, s| {
        tr!(
            "グループは値の集合になり、その名前は `_` のあとにグループの名前を lowerCamelCase で書くので、どちらも `{s}` です。同じ名前の定数を二度宣言することになり、組めません。",
            "a group is a set of values, named `_` followed by the group's name in lowerCamelCase, so both are `{s}`: one constant is declared twice, which does not compile."
        )
    });
    ms.meet(&groups, &[Target::Php], |s| format!("GROUP_{}", super::php::php_name(s)), |_, _, s| {
        tr!(
            "グループは定数 `GROUP_<名前>` になり、PHP の予約語と同じ名前には `_` を付けて書くので、どちらも `{s}` です。同じ名前の定数を二度宣言することになり、読み込めません。",
            "a group is a constant `GROUP_<name>`, and a name that is a keyword of PHP is written with `_` after it, so both are `{s}`: one constant is declared twice, and the file does not load."
        )
    });

    // The tables and the clauses: the SQL query's column of the number of the row that matched,
    // which stands among the columns of the values.
    let mut cols: Vec<Decl> = Vec::new();
    if sql {
        let ids: Vec<String> = vs.iter().map(|d| d.id.clone()).collect();
        for it in &f.items {
            if let Item::Table(t) = it {
                if let Some(n) = t.name.as_ref().filter(|n| !super::brought_in(&n.text)) {
                    let what = if t.clause { tr!("節", "clause") } else { tr!("表", "table") };
                    cols.push(Decl { name: n, id: row_column(&pub_name(n), f, &ids), what });
                }
            }
        }
        // In the order declared, so that the first of two is the one written above; two values
        // were said above, and only what meets a table's column is said here.
        let mut with_values: Vec<Decl> = vs.iter().cloned().chain(cols.iter().cloned()).collect();
        with_values.sort_by_key(|d| place(&d.name.span));
        let column = |d: &Decl| cols.iter().any(|c| c.name.span.same(&d.name.span));
        let keep = |a: &Decl, b: &Decl| column(a) || column(b);
        ms.meet_where(
            &with_values,
            &[Target::Sqlite],
            |s| s.to_ascii_lowercase(),
            |a, b, _| {
                tr!(
                    "問い合わせは、値と、表ごとに当たった行の番号（`<識別子>_row`）を列に持ちます。SQLite は列の名前の大文字と小文字を区別しないので、列 `{a}` と `{b}` は一つの列として読まれ、問い合わせは片方の列で答えます。",
                    "the query holds the values and, for each table, the number of the row that matched (a column `<identifier>_row`) as columns, and SQLite reads the name of a column with case set aside, so the columns `{a}` and `{b}` are one, and the query answers with one of the two."
                )
            },
            keep,
        );
        ms.meet_where(
            &with_values,
            &[Target::Postgres],
            |s| pg_head(s).to_string(),
            |_, _, s| {
                tr!(
                    "問い合わせと関数は、値と、表ごとに当たった行の番号（`<識別子>_row`）を列に持ちます。PostgreSQL は名前の先頭の 63 バイトしか残さないので、どちらも `{s}` です。PostgreSQL では、問い合わせも関数も、どちらの列か決められずに止まります。",
                    "the query and the function hold the values and, for each table, the number of the row that matched (a column `<identifier>_row`) as columns, and PostgreSQL keeps the first 63 bytes of a name, so both are `{s}`: on PostgreSQL the query and the function stop at a column they cannot tell apart."
                )
            },
            keep,
        );
    }

    // The prerequisites blocks: two functions each.
    let blocks: Vec<Decl> = f.prerequisites.iter().map(|p| plain(&p.name, tr!("前提のつながり", "prerequisites"))).collect();
    ms.meet(&blocks, &[Target::TypeScript, Target::JavaScript, Target::Swift, Target::Java], |s| camel(&format!("{}_requires", block(s))), |_, _, s| {
        tr!(
            "前提のつながりの関数（`<名前>Requires` と `<名前>Available`）の名前を lowerCamelCase で書くので、どちらも `{s}` です。同じ名前の関数を二度宣言することになり、組めません。",
            "the functions of a prerequisites block (`<name>Requires` and `<name>Available`) are written in lowerCamelCase, so both are `{s}`: one function is declared twice, which does not compile."
        )
    });
    ms.meet(&blocks, &[Target::Go], |s| pascal(&format!("{}_requires", block(s))), |_, _, s| {
        tr!(
            "前提のつながりの関数（`<Name>Requires` と `<Name>Available`）の名前を PascalCase で書くので、どちらも `{s}` です。同じ名前の関数を二度宣言することになり、コンパイルできません。",
            "the functions of a prerequisites block (`<Name>Requires` and `<Name>Available`) are written in PascalCase, so both are `{s}`: one function is declared twice, which does not compile."
        )
    });

    // The function that reads the inputs out of the caller's objects (§15.125): the shapes it reads
    // from, and the inputs and the sequences the caller still passes as they are.
    let shapes = used_shapes(f);
    if !shapes.is_empty() {
        let mut args: Vec<Decl> = shapes.iter().map(|s| plain(s, tr!("`shape`", "shape"))).collect();
        args.extend(vs.iter().filter(|d| f.inputs.iter().any(|i| i.name.text == d.name.text && i.from.is_none()) || f.elements.iter().any(|el| el.name.text == d.name.text && el.from.is_none())).cloned());
        args.sort_by_key(|d| place(&d.name.span));
        ms.meet(&args, &[Target::Php], super::php::php_var_pub, |_, _, s| {
            tr!(
                "呼ぶ側のオブジェクトから入力を読む関数（`<規則>_from`）の引数で、PHP が使う変数（`$this` とスーパーグローバル）と同じ名前には `_` を付けて書くので、どちらも `{s}` です。同じ名前の引数が二つになり、読み込めません。",
                "in the parameters of the function that reads the inputs out of the caller's objects (`<rule>_from`), a name PHP keeps for a variable of its own (`$this` and the superglobals) is written with `_` after it, so both are `{s}`: the function takes one parameter twice, and the file does not load."
            )
        });
    }

    let mut out: Vec<Diag> = Vec::new();
    let mut found = ms.0;
    found.sort_by_key(|m| (place(&m.second.name.span), place(&m.first.name.span)));
    for m in found {
        let (a, b) = (&m.first, &m.second);
        let ts = listed(&m.targets());
        // `Go・Swift の`, and `SQL（SQLite）の` with no space after the closing parenthesis.
        let gap = if ts.ends_with('）') { "" } else { " " };
        let mut d = Diag::warning(
            "W121",
            tr!("`{}` と `{}` は、{ts}{gap}の生成コードで同じ識別子になります", "`{}` and `{}` are one identifier in the code generated for {ts}", a.name.text, b.name.text),
        )
        .at(at(b.name.span.line))
        .mark(a.name.span.clone(), label(a))
        .mark(b.name.span.clone(), label(b))
        .targets(&ids_of(&m.targets(), m.wire));
        let mut notes = m.notes.clone();
        notes.sort_by_key(|(t, _)| t.first().copied());
        for (_, note) in notes {
            d = d.note(note);
        }
        let fix = match b.name.ascii {
            Some(_) => tr!(
                "使わない生成先なら、このままで構いません。使うなら、どちらかの別名を変えてください。",
                "For a target you do not generate, leave it. For one you do, change the alias of one of the two."
            ),
            None => tr!(
                "使わない生成先なら、このままで構いません。使うなら、どちらかに別名を付けてください（`{}(<別名>)`）。",
                "For a target you do not generate, leave it. For one you do, give one of the two an alias (`{}(<alias>)`).",
                b.name.text
            ),
        };
        out.push(d.note(fix));
    }

    // A name a language cannot take where its code puts it. (A value or a shape whose identifier
    // starts with a capital letter, which Ruby reads as a constant, is bound under `_` and its
    // identifier by the Ruby generator where its functions hold it, §15.336: no field and no key
    // of it changes, and W121 does not say it.)
    let mut alone: Vec<(Decl, Diag)> = Vec::new();
    // A name of `_` alone is said with the other languages that cannot take it (`own_warnings`).
    let underscore = |d: &Decl| d.id.starts_with('_') && d.id != "_";
    let mut python: Vec<(Decl, String)> = Vec::new();
    if f.outputs.len() >= 2 {
        for d in vs.iter().filter(|d| f.outputs.iter().any(|o| o.name.text == d.name.text)).filter(|d| underscore(d)) {
            python.push((
                d.clone(),
                tr!(
                    "Python: 出力が二つ以上ある規則の出力は `NamedTuple` の `Output` のフィールドになり、`NamedTuple` のフィールドの名前は `_` で始められません。生成した Python のファイルは読み込めません。",
                    "Python: the outputs of a rule with two or more are the fields of `Output`, a `NamedTuple`, and a `NamedTuple` takes no field whose name starts with `_`, so the generated Python file does not load."
                ),
            ));
        }
    }
    for el in &f.elements {
        let ty = if f.elements.len() <= 1 { "Element".to_string() } else { element_type(&pub_name(&el.name)) };
        for fd in el.fields.iter().filter(|fd| !made.values.contains_key(&fd.name.text) && !super::brought_in(&fd.name.text)) {
            let d = Decl { name: &fd.name, id: pub_name(&fd.name), what: tr!("要素のフィールド", "element field") };
            if underscore(&d) {
                python.push((
                    d,
                    tr!(
                        "Python: 要素のフィールドは `NamedTuple` の `{ty}` のフィールドになり、`NamedTuple` のフィールドの名前は `_` で始められません。生成した Python のファイルは読み込めません。",
                        "Python: the fields of an element are the fields of `{ty}`, a `NamedTuple`, and a `NamedTuple` takes no field whose name starts with `_`, so the generated Python file does not load."
                    ),
                ));
            }
        }
    }
    for (d, why) in python {
        let ts = listed(&[Target::Python, Target::Connect]);
        let diag = Diag::warning("W121", tr!("`{}` は `_` で始まるので、{ts} の生成コードでは使えません", "`{}` starts with `_`, which the code generated for {ts} cannot take", d.id))
            .at(at(d.name.span.line))
            .mark(d.name.span.clone(), label(&d))
            .targets(&ids_of(&[Target::Python, Target::Connect], false))
            .note(why)
            .note(tr!("Connect: サービスは Python のモジュールを動かすので、読み込めません。", "Connect: the service runs the Python module, which does not load."))
            .note(match d.name.ascii {
                Some(_) => tr!("使わない生成先なら、このままで構いません。使うなら、別名を `_` で始まらないものに変えてください。", "For a target you do not generate, leave it. For one you do, change the alias to one that does not start with `_`."),
                None => tr!(
                    "使わない生成先なら、このままで構いません。使うなら、`_` で始まらない別名を付けてください（`{}(<別名>)`）。",
                    "For a target you do not generate, leave it. For one you do, give it an alias that does not start with `_` (`{}(<alias>)`).",
                    d.name.text
                ),
            });
        alone.push((d, diag));
    }
    // The columns the query and the function return, which PostgreSQL cuts at 63 bytes: the
    // outputs, and the number of the row each table matched.
    if sql {
        let returned = vs.iter().filter(|d| f.outputs.iter().any(|o| o.name.text == d.name.text)).cloned().chain(cols.iter().cloned());
        for d in returned.filter(|d| d.id.len() > 63) {
            let cut = pg_head(&d.id).to_string();
            let id = &d.id;
            let (title, why) = if cols.iter().any(|c| c.name.span.same(&d.name.span)) {
                (
                    tr!(
                        "表 `{}` の行の番号の列 `{id}` は 63 バイトより長いので、PostgreSQL では名前が切られます",
                        "The column `{id}` of the row number of table `{}` is longer than 63 bytes, and PostgreSQL cuts its name",
                        d.name.text
                    ),
                    tr!(
                        "SQL（PostgreSQL）: 問い合わせと関数は、表の当たった行の番号を列 `{id}` で返しますが、PostgreSQL は名前の先頭の 63 バイトしか残さないので、その列の名前は `{cut}` になります。生成したファイルの先頭のコメントや `rulec api` に書いた名前では読めません。",
                        "SQL (PostgreSQL): the query and the function return the number of the row the table matched in the column `{id}`, and PostgreSQL keeps the first 63 bytes of a name, so the column is named `{cut}`, and is not found under the name the generated file's header and `rulec api` give."
                    ),
                )
            } else {
                (
                    tr!("`{id}` は 63 バイトより長いので、PostgreSQL では列の名前が切られます", "`{id}` is longer than 63 bytes, and PostgreSQL cuts the name of its column"),
                    tr!(
                        "SQL（PostgreSQL）: PostgreSQL は名前の先頭の 63 バイトしか残さないので、問い合わせと関数が返す列 `{id}` の名前は `{cut}` になります。生成したファイルの先頭のコメントや `rulec api` に書いた名前では読めません。",
                        "SQL (PostgreSQL): PostgreSQL keeps the first 63 bytes of a name, so the column `{id}` the query and the function return is named `{cut}`, and is not found under the name the generated file's header and `rulec api` give."
                    ),
                )
            };
            let diag = Diag::warning("W121", title)
                .at(at(d.name.span.line))
                .mark(d.name.span.clone(), label(&d))
                .targets(&ids_of(&[Target::Postgres], false))
                .note(why)
                .note(match d.name.ascii {
                    Some(_) => tr!("使わない生成先なら、このままで構いません。使うなら、列の名前が 63 バイト以下になるよう、別名を縮めてください。", "For a target you do not generate, leave it. For one you do, shorten the alias so that the column's name is 63 bytes or fewer."),
                    None => tr!(
                        "使わない生成先なら、このままで構いません。使うなら、列の名前が 63 バイト以下になる別名を付けてください（`{}(<別名>)`）。",
                        "For a target you do not generate, leave it. For one you do, give it an alias that makes the column's name 63 bytes or fewer (`{}(<alias>)`).",
                        d.name.text
                    ),
                });
            alone.push((d, diag));
        }
    }
    // A value of an enum is written in each language's own case (`spellings::member_keyword`), which
    // can be a keyword in two of them: Rust writes a variant in PascalCase, where `self` is `Self`,
    // the keyword for the type itself, which names no variant and which no raw identifier takes (in
    // Rust and in Wasm, whose module is the Rust one); Ruby writes a constant in upper case, where
    // `begin` and `end` are `BEGIN` and `END`, keywords that no constant can be. The module does not
    // build there. A caller names the variant or the constant (`Payer::Self`, `K::BEGIN`), so it is
    // not written apart. The value is said by its alias as by its name: the alias is the identifier
    // each language spells.
    for e in f.enums.iter().filter(|e| e.from.is_none() && !super::brought_in(&e.name.text)) {
        for v in &e.values {
            let d = plain(v, tr!("列挙の値", "enum value"));
            let mut met: Vec<(String, Vec<Target>)> = Vec::new();
            for b in crate::backend::ALL {
                let (Some(t), Some(word)) = (target_of(b.id), member_keyword(b.id, &d.id)) else { continue };
                match met.iter_mut().find(|(w, _)| *w == word) {
                    Some((_, ts)) => ts.push(t),
                    None => met.push((word, vec![t])),
                }
            }
            if met.is_empty() {
                continue;
            }
            let all: Vec<Target> = met.iter().flat_map(|(_, ts)| ts.iter().copied()).collect();
            let ts = listed(&all);
            let gap = if ts.ends_with('）') { "" } else { " " };
            let mut diag = Diag::warning("W121", tr!("`{}` は、{ts}{gap}の生成コードでは使えません", "`{}` is a name the code generated for {ts} cannot take", d.id))
                .at(at(d.name.span.line))
                .mark(d.name.span.clone(), label(&d))
                .targets(&ids_of(&all, false));
            for (word, wts) in &met {
                let why = if wts.contains(&Target::Ruby) {
                    tr!(
                        "列挙の値は大文字の名前の定数になり、`{0}` は `{1}` になります。`{1}` は予約語で、定数の名前にならないので、生成したモジュールは組めません。",
                        "a value of an enum is a constant named in upper case, so `{0}` is `{1}`, a keyword, which no constant can be named: the generated module does not build.",
                        d.id,
                        word
                    )
                } else {
                    tr!(
                        "列挙の値は PascalCase の名前の列挙子になり、`{0}` は `Self` になります。`Self` は型そのものを指す予約語で、列挙子の名前にならず、`r#Self` とも書けないので、生成したモジュールは組めません。",
                        "a value of an enum is a variant named in PascalCase, so `{0}` is `Self`, the keyword for the type itself, which names no variant, nor can it be written `r#Self`: the generated module does not build.",
                        d.id
                    )
                };
                diag = diag.note(format!("{}: {why}", listed(wts)));
            }
            let diag = diag
                .note(match d.name.ascii {
                    Some(_) => tr!("使わない生成先なら、このままで構いません。使うなら、別名を変えてください。", "For a target you do not generate, leave it. For one you do, change the alias."),
                    None => tr!(
                        "使わない生成先なら、このままで構いません。使うなら、別名を付けてください（`{}(<別名>)`）。",
                        "For a target you do not generate, leave it. For one you do, give it an alias (`{}(<alias>)`).",
                        d.name.text
                    ),
                });
            alone.push((d, diag));
        }
    }
    alone.sort_by_key(|(d, _)| place(&d.name.span));
    out.extend(alone.into_iter().map(|(_, diag)| diag));
    out
}

/// Every W121 of a name of the rule's that its generated code writes as one it gives itself, and
/// of a name some language cannot take in a way §15.263 does not say (§15.265): said once the check
/// has typed every item and built every definition set, which what the generated code reads is
/// worked out from.
pub fn own_warnings(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    // An `enums` file's module declares its types and nothing else, which no name of the file's
    // meets, but the module's own name; a contribution is checked as part of the rule it adds to.
    if f.kind == crate::ast::FileKind::Enums {
        return enums_file_warnings(f, path);
    }
    if f.kind != crate::ast::FileKind::Rule {
        return Vec::new();
    }
    let made = super::made_identifiers(f);
    let at = |line: usize| format!("{path}:{line}");
    let vs = values(f, &made);
    let sql = f.elements.is_empty();
    let own: Vec<&crate::ast::EnumDecl> = f.enums.iter().filter(|e| e.from.is_none() && !super::brought_in(&e.name.text)).collect();
    let wire = wire_enums(f, c);
    let mut alone: Vec<(Decl, Diag)> = Vec::new();
    // A name some languages cannot take, said with the languages and why (§15.265); `wire` where
    // what Connect cannot take is the `.proto`'s.
    let cannot = |d: &Decl, ts: &[Target], notes: Vec<(Vec<Target>, String)>, wire: bool| -> Diag {
        let listed_ts = listed(ts);
        let gap = if listed_ts.ends_with('）') { "" } else { " " };
        let mut diag = Diag::warning("W121", tr!("`{}` は、{listed_ts}{gap}の生成コードでは使えません", "`{}` is a name the code generated for {listed_ts} cannot take", d.id))
            .at(at(d.name.span.line))
            .mark(d.name.span.clone(), label(d))
            .targets(&ids_of(ts, wire));
        for (nts, why) in notes {
            diag = diag.note(format!("{}: {why}", listed(&nts)));
        }
        diag.note(match d.name.ascii {
            Some(_) => tr!("使わない生成先なら、このままで構いません。使うなら、別名を変えてください。", "For a target you do not generate, leave it. For one you do, change the alias."),
            None => tr!(
                "使わない生成先なら、このままで構いません。使うなら、別名を付けてください（`{}(<別名>)`）。",
                "For a target you do not generate, leave it. For one you do, give it an alias (`{}(<alias>)`).",
                d.name.text
            ),
        })
    };
    // `_` is no variable's name in Go, Java, Rust and Swift: the blank identifier, a keyword, the
    // pattern that binds nothing; the generators bind a value of the name under `__` where the
    // functions hold it, and Python's plain function keeps its own answer apart from a parameter
    // `_` (§15.336). What a caller names stays: a field of the answer of a rule with two or more
    // outputs (Python's `NamedTuple`, which the service of Connect loads, the struct of Rust, Go
    // and Swift, Java's record), Go's field of `in` and Swift's label of the argument.
    for d in vs.iter().filter(|d| d.id == "_") {
        let passed = f.inputs.iter().any(|i| i.name.text == d.name.text) || f.elements.iter().any(|el| el.name.text == d.name.text);
        let returned = f.outputs.iter().any(|o| o.name.text == d.name.text);
        let fielded = returned && f.outputs.len() >= 2;
        let mut notes: Vec<(Vec<Target>, String)> = Vec::new();
        if fielded {
            notes.push((
                vec![Target::Python],
                tr!(
                    "出力が二つ以上ある規則の出力は `NamedTuple` の `Output` のフィールドになり、`NamedTuple` のフィールドの名前は `_` で始められません。生成した Python のファイルは読み込めません。",
                    "the outputs of a rule with two or more are the fields of `Output`, a `NamedTuple`, and a `NamedTuple` takes no field whose name starts with `_`, so the generated Python file does not load."
                ),
            ));
            notes.push((
                vec![Target::Rust, Target::Swift, Target::Wasm],
                tr!(
                    "出力が二つ以上ある規則の出力は構造体 `Output` のフィールドになり、`_` は値を捨てるパターンなので、フィールドの名前になりません。",
                    "the outputs of a rule with two or more are the fields of the `Output` struct, and `_` is the pattern that binds nothing: it names no field."
                ),
            ));
            notes.push((
                vec![Target::Go],
                tr!(
                    "出力が二つ以上ある規則の出力は構造体 `Output` のフィールドになり、`_` は空白の識別子なので、その名前のフィールドには値を書くことも読むこともできません。",
                    "the outputs of a rule with two or more are the fields of the `Output` struct, and `_` is the blank identifier: a field of that name can be neither written nor read."
                ),
            ));
            notes.push((
                vec![Target::Java],
                tr!(
                    "出力が二つ以上ある規則の出力はレコード `Output` の成分になり、`_` は予約語です。",
                    "the outputs of a rule with two or more are the components of the `Output` record, and `_` is a keyword."
                ),
            ));
        }
        if passed {
            notes.push((
                vec![Target::Go],
                tr!(
                    "入力と並びは `Input` 構造体のフィールドになり、`_` は空白の識別子なので、その名前のフィールドには値を書くことも読むこともできません。",
                    "the inputs and the sequences are the fields of the `Input` struct, and `_` is the blank identifier: a field of that name can be neither written nor read."
                ),
            ));
            notes.push((
                vec![Target::Swift],
                tr!(
                    "入力と並びは、規則の関数を呼ぶときに引数に付けるラベルになり、`_` はラベルを付けない引数を表すので、その名前では渡せず、関数もその引数を名前で読めません。",
                    "the inputs and the sequences are the labels a caller passes the arguments of the rule's function under, and `_` is the label of an argument that has none: it cannot be passed by that name, and the function cannot read it by its name."
                ),
            ));
        }
        if fielded {
            notes.push((vec![Target::Connect], tr!("サービスは Python のモジュールを動かすので、読み込めません。", "the service runs the Python module, which does not load.")));
        }
        if notes.is_empty() {
            continue;
        }
        let mut ts: Vec<Target> = notes.iter().flat_map(|(t, _)| t.iter().copied()).collect();
        ts.sort();
        notes.sort_by_key(|(t, _)| t.iter().min().copied());
        alone.push((d.clone(), cannot(d, &ts, notes, false)));
    }
    // A field of one element named `_` is a field of the element's struct, record and message
    // (Python's `NamedTuple` takes no field that starts with `_`, which §15.263 says).
    for el in &f.elements {
        for fd in el.fields.iter().filter(|fd| pub_name(&fd.name) == "_" && !made.values.contains_key(&fd.name.text) && !super::brought_in(&fd.name.text)) {
            let d = Decl { name: &fd.name, id: "_".into(), what: tr!("要素のフィールド", "element field") };
            let ty = if f.elements.len() <= 1 { "Element".to_string() } else { element_type(&pub_name(&el.name)) };
            let notes: Vec<(Vec<Target>, String)> = vec![
                (
                    vec![Target::Python],
                    tr!(
                        "要素のフィールドは `NamedTuple` の `{ty}` のフィールドになり、`NamedTuple` のフィールドの名前は `_` で始められません。生成した Python のファイルは読み込めません。",
                        "the fields of an element are the fields of `{ty}`, a `NamedTuple`, and a `NamedTuple` takes no field whose name starts with `_`, so the generated Python file does not load."
                    ),
                ),
                (vec![Target::Rust, Target::Swift, Target::Wasm], tr!("`_` は値を捨てるパターンで、フィールドの名前になりません。", "`_` is the pattern that binds nothing, and names no field.")),
                (vec![Target::Go], tr!("`_` は空白の識別子で、その名前のフィールドには値を書くことも読むこともできません。", "`_` is the blank identifier: a field of that name can be neither written nor read.")),
                (vec![Target::Java], tr!("`_` は予約語です。", "`_` is a keyword.")),
                (vec![Target::Connect], tr!("サービスは Python のモジュールを動かすので、読み込めません。", "the service runs the Python module, which does not load.")),
            ];
            let ts = [Target::Python, Target::Rust, Target::Go, Target::Swift, Target::Java, Target::Wasm, Target::Connect];
            alone.push((d.clone(), cannot(&d, &ts, notes, false)));
        }
    }
    // An enum's type and its values are written in PascalCase, of which nothing is left of a name
    // that is all `_`: no language that writes them can.
    let empty = |id: &str| pascal(id).is_empty();
    for e in &own {
        let d = plain(&e.name, tr!("列挙", "enum"));
        let mut targets: Vec<Target> = vec![Target::Python, Target::TypeScript, Target::JavaScript, Target::Rust, Target::Php, Target::Go, Target::Swift, Target::Java, Target::Wasm];
        if wire.contains(&e.name.text) {
            targets.push(Target::Connect);
        }
        let why = tr!("型と値の名前は PascalCase にして書くので、`_` だけの名前からは何も残らず、名前の無い型や値になります。", "the types and the values of an enum are written in PascalCase, of which nothing is left of a name of `_` alone: the type or the value has no name.");
        let on_wire = wire.contains(&e.name.text);
        if empty(&d.id) {
            alone.push((d.clone(), cannot(&d, &targets, vec![(targets.clone(), why.clone())], on_wire)));
        }
        for v in &e.values {
            let dv = plain(v, tr!("列挙の値", "enum value"));
            if empty(&dv.id) {
                alone.push((dv.clone(), cannot(&dv, &targets, vec![(targets.clone(), why.clone())], on_wire)));
            }
        }
    }
    // SQLite gives a column no name of `true` or `false`, whatever their case: the query's
    // `AS "True"` names it `column2`, and `"True"` then reads a string (§15.264). An output is a
    // column the query hands back, which a caller reads by its name; the query writes the column of
    // any other value apart (§15.336).
    if sql {
        let output = |d: &&Decl| f.outputs.iter().any(|o| o.name.text == d.name.text);
        for d in vs.iter().filter(output).filter(|d| d.id.eq_ignore_ascii_case("true") || d.id.eq_ignore_ascii_case("false")) {
            let why = tr!(
                "SQLite は、問い合わせの式に付けた名前 `AS \"{0}\"` を列の名前にしません（`true` と `false` は、大文字と小文字を問わず）。`\"{0}\"` と書いて読むと文字列になり、問い合わせは違う値で答えます。PostgreSQL は列として読みます。",
                "SQLite gives a column no name of `true` or `false`, whatever their case: the query's `AS \"{0}\"` leaves the column unnamed, and `\"{0}\"` then reads a string, so the query answers otherwise. PostgreSQL reads the column.",
                d.id
            );
            alone.push((d.clone(), cannot(d, &[Target::Sqlite], vec![(vec![Target::Sqlite], why)], false)));
        }
    }
    // The `.proto` takes ASCII letters, digits and `_` for its names, and an enum on the wire is
    // declared there with its values, by their identifiers.
    for e in own.iter().filter(|e| wire.contains(&e.name.text)) {
        let why = tr!("`.proto` の名前は ASCII の英字と数字と `_` だけで、この名前の列挙の型や値は書けません。buf と protoc がエラーにします。", "the names of the `.proto` take ASCII letters, digits and `_` alone, and an enum's type or value of this name cannot be written there, which buf and protoc refuse.");
        let d = plain(&e.name, tr!("列挙", "enum"));
        if !d.id.is_ascii() {
            alone.push((d.clone(), cannot(&d, &[Target::Connect], vec![(vec![Target::Connect], why.clone())], true)));
        }
        for v in &e.values {
            let dv = plain(v, tr!("列挙の値", "enum value"));
            if !dv.id.is_ascii() {
                alone.push((dv.clone(), cannot(&dv, &[Target::Connect], vec![(vec![Target::Connect], why.clone())], true)));
            }
        }
    }
    // Go's file of the rule is `<rule>.go`, which the go command leaves out of the package where it
    // starts with `_` (`go help packages`): the package then has none of the rule's functions.
    let rule = plain(&f.name, tr!("規則", "rule"));
    if !super::brought_in(&f.name.text) && rule.id.starts_with('_') {
        let file = format!("{}.go", rule.id);
        let why = tr!(
            "規則のファイルは `{file}` になりますが、go コマンドは `_` で始まる名前のファイルをパッケージに入れないので（`go help packages`）、規則の関数の無いパッケージになり、それを呼ぶコードは組めません。",
            "the rule's file is `{file}`, and the go command leaves a file whose name starts with `_` out of the package (`go help packages`): the package has none of the rule's functions, and the code that calls them does not build."
        );
        alone.push((rule.clone(), cannot(&rule, &[Target::Go], vec![(vec![Target::Go], why)], false)));
    }
    // (A name rulec writes with `_` after it because a language keeps it as a keyword, §15.210,
    // keeps its capital, `True_`, which Ruby reads as a constant: the Ruby generator binds it under
    // `_` before it where the functions hold it, §15.336, and W121 does not say it.)
    alone.sort_by_key(|(d, _)| place(&d.name.span));
    let mut out: Vec<Diag> = alone.into_iter().map(|(_, diag)| diag).collect();
    // What the generated code reads is worked out from the code rulec writes for the rule (the
    // SQL query's bound columns), which a rule the check stops at has none of.
    if !crate::has_error(&c.diags) {
        out.extend(takes_own(f, c, path, &vs, &own, &wire));
        out.extend(sw_files_twice(f, c, path));
    }
    out
}

/// Every name two of the Swift files `gen` writes for this rule declare — its module, its runner
/// and the module of every `enums` file it reads build into one module, where the second of two
/// declarations of one name is `invalid redeclaration` (§15.345): said with the files that
/// declare it, as the names two targets spell as one are, and with `swift` the target it stops.
fn sw_files_twice(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let g = super::Gen::for_names(f, c);
    let owned = g.for_target("swift");
    let g = owned.as_ref().unwrap_or(&g);
    let rule_file = format!("{}.swift", pub_name(&f.name));
    let runner_file = format!("{}_runner.swift", pub_name(&f.name));
    let mut out = Vec::new();
    for (name, files) in g.sw_redeclared() {
        // The declaration of this rule the name comes from, which the finding marks: an enum it
        // declares (an `apply`'s own among them), an enum it reads, else its own name.
        let own = files.iter().find(|(file, _)| *file == rule_file).and_then(|(_, jp)| jp.as_ref()).and_then(|jp| f.enums.iter().find(|e| e.name.text == *jp));
        let (span, label) = match own {
            Some(e) if super::brought_in(&e.name.text) => (e.name.span.clone(), tr!("`apply` が持ち込む列挙", "an enum an `apply` brings in")),
            Some(e) if e.from.is_some() => (e.name.span.clone(), tr!("`import enum` で読む列挙", "an enum read with `import enum`")),
            Some(e) => (e.name.span.clone(), tr!("この列挙", "this enum")),
            None => {
                // an enum read with `import enum`: the one that is the name in its file, else any
                // line that reads one of the files the module of which is among the two
                let module = |o: &crate::ast::EnumOwner| format!("{}.swift", pub_name(&o.module));
                let imp = f
                    .enums
                    .iter()
                    .find(|e| e.from.as_ref().is_some_and(|o| files.iter().any(|(file, jp)| *file == module(o) && jp.as_deref() == Some(o.name.text.as_str()))))
                    .or_else(|| f.enums.iter().find(|e| e.from.as_ref().is_some_and(|o| files.iter().any(|(file, _)| *file == module(o)))));
                match imp {
                    Some(e) => (e.name.span.clone(), tr!("`import enum` で読む列挙", "an enum read with `import enum`")),
                    None => (f.name.span.clone(), tr!("規則の名前", "the rule's name")),
                }
            }
        };
        let mut d = Diag::warning("W121", tr!("`{name}` が、Swift の生成コードで二度宣言されます", "`{name}` is declared twice in the code generated for Swift", name = name))
            .at(format!("{path}:{}", span.line))
            .mark(span, label)
            .targets(&ids_of(&[Target::Swift], false));
        for (file, jp) in &files {
            let what = match jp {
                Some(j) if *file == rule_file => match f.enums.iter().find(|e| e.name.text == *j) {
                    Some(e) if super::brought_in(&e.name.text) => tr!("`apply` が持ち込む列挙 `{j}` の型", "the type of the enum `{j}` an `apply` brings in", j = j),
                    Some(e) if e.from.is_some() => tr!("`import enum` で読む列挙 `{j}` の型", "the type of the enum `{j}` read with `import enum`", j = j),
                    _ => tr!("列挙 `{j}` の型", "the type of the enum `{j}`", j = j),
                },
                Some(j) => tr!("`enums` ファイルの列挙 `{j}` の型", "the type of the enum `{j}` of the `enums` file", j = j),
                None if *file == runner_file => tr!("runner の `{name}`", "the runner's `{name}`", name = name),
                None => tr!("`{name}`", "`{name}`", name = name),
            };
            d = d.note(tr!("`{file}` は{what}として `{name}` を宣言します", "`{file}` declares `{name}` as {what}", file = file, what = what, name = name));
        }
        d = d
            .note(tr!(
                "Swift は、規則のモジュールと runner と、規則が読む `enums` ファイルのモジュールを一つのモジュールに組むので、同じ名前を二度宣言する二つのファイルは組めません。",
                "The generated Swift builds the rule's module, its runner and the modules of the `enums` files it reads into one module, where two files declaring one name do not build."
            ))
            .note(tr!(
                "どちらかの宣言が作る名前を変えてください（規則か列挙に別名を付ける、準用の名前を変える、`enums` ファイルの列挙の名前か別名を変える、`import enum … -> <別の名前>` で読む名前を変える）。生成したコードが呼ぶ側に見せる名前は約束なので、生成器は名前を付け替えません。",
                "Rename what one of the two declarations makes (an alias on the rule or the enum, another name for the `apply`, the enum's name or alias in the `enums` file, or the name it is read as with `import enum … -> <another name>`): the name the generated code shows its callers is a promise, so the generator does not spell it otherwise."
            ));
        out.push(d);
    }
    out
}

/// What a note of [`Takes`] says a name of the rule's meets: the target's own name, and the kind of
/// place it stands in.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Meets {
    /// A name the rule's functions read, which the value hides there; only a type checker reads it
    /// where `checker` is set.
    Read { name: String, what: super::own::What, written: String, checker: bool },
    /// A name the module declares at its top, which the enum's type is declared as again.
    Declared { name: String, what: super::own::What, written: String, caseless: bool },
    /// The field of the answer the rows that matched travel in.
    TraceField,
    /// The zero value of every enum of the `.proto`.
    ZeroValue { name: String },
    /// A message or (`service`) the service of the `.proto`.
    Message { name: String, service: bool },
    /// A name the module or the runner declares, which the rule's function, or (`class`) its class,
    /// is declared as again (`written`).
    RuleDeclared { name: String, what: super::own::What, written: String, class: bool },
    /// A name the go command keeps for itself, which the rule's package takes.
    GoPackage { name: String },
    /// `main.swift`, the file Swift reads as the program's top-level code.
    SwiftMain,
    /// A helper the module declares, which the group's constant is declared as again.
    GroupDeclared { name: String, what: super::own::What },
    /// A word of the language: a keyword, a name it has built in, a module of its library.
    Word { word: String, kind: Word },
    /// A field of a `NamedTuple` named like the type a later field of it is annotated with.
    Annotation { class: String, ty: String },
    /// A builtin the module's helpers read by its own name, which the rule's function is named as.
    ModuleReads { name: String },
    /// A name the module declares or imports at its top, which an enum the rule reads from the
    /// `enums` file `file` is declared or imported as again, under the file's type (`written`).
    ReadEnum { name: String, what: super::own::What, written: String, file: String },
}

/// What a word of a language is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Word {
    Keyword,
    /// A name the language has built in, met by an enum's type (`ty`) or by the rule's function.
    Builtin {
        ty: bool,
    },
    Module,
}

/// The target a row of the registry is, for the ones this module names.
fn target_of(id: &str) -> Option<Target> {
    Some(match id {
        "python" => Target::Python,
        "typescript" => Target::TypeScript,
        "javascript" => Target::JavaScript,
        "rust" => Target::Rust,
        "ruby" => Target::Ruby,
        "php" => Target::Php,
        "go" => Target::Go,
        "swift" => Target::Swift,
        "java" => Target::Java,
        "wasm" => Target::Wasm,
        _ => return None,
    })
}

/// Whether `w` is one of PHP's keywords, or `true`, `false` or `null`, in any case: a word no function
/// and no class of PHP's can be named, which PHP keeps among its globals since a value of PHP's is a
/// variable (`$list`). The rest of its globals are builtin functions (`count`), which the generated
/// code calls from the global namespace where a function of the rule's has the name (§15.336).
pub(crate) fn php_keyword(w: &str) -> bool {
    ritsu_emit::copies::rulec::PHP_GLOBAL.iter().any(|k| k.eq_ignore_ascii_case(w))
}

/// The row of the registry a target is, for the ones [`target_of`] names.
fn backend_of(t: Target) -> Option<&'static str> {
    Some(match t {
        Target::Python => "python",
        Target::TypeScript => "typescript",
        Target::JavaScript => "javascript",
        Target::Rust => "rust",
        Target::Ruby => "ruby",
        Target::Php => "php",
        Target::Go => "go",
        Target::Swift => "swift",
        Target::Java => "java",
        Target::Wasm => "wasm",
        _ => return None,
    })
}

/// The rule's function as the backend `id` writes it, from the rule's identifier: in PascalCase in
/// Go, in lowerCamelCase in Swift, as the class it is in Java, and as it is in the others. None for
/// a backend that writes no function of the rule.
pub(crate) fn rule_function(id: &str, rule: &str) -> Option<String> {
    Some(match target_of(id)? {
        Target::Go => pascal(rule),
        Target::Swift => camel(rule),
        Target::Java => super::java::java_class(rule),
        _ => rule.to_string(),
    })
}

/// Ruby's keywords that a constant can be spelled as: no module and no constant takes them
/// (`module BEGIN` and `BEGIN = "begin"` do not parse).
const RB_CONSTANT_KEYWORDS: [&str; 2] = ["BEGIN", "END"];

/// Whether `w` is a keyword of the language of the backend `id` as it stands: PHP's in any case, the
/// others' in their own case, with Rust's `Self`, the one keyword of Rust's that is capitalized.
fn keyword_as_written(id: &str, w: &str) -> bool {
    match id {
        "php" => php_keyword(w),
        "rust" | "wasm" if w == "Self" => true,
        _ => crate::backend::ALL.iter().find(|b| b.id == id).is_some_and(|b| b.reserved.iter().any(|k| k == w)),
    }
}

/// The keyword of its language the backend `id` writes the rule as, from the rule's identifier
/// `rule`, if it writes one (§15.103, §15.265): the rule's function, and the module or the package
/// the rule is, each as the language spells it. Go's function is in PascalCase, which no keyword
/// is, but its package is the identifier in lower case without its `_`, which can be one
/// (`fall_through` is `fallthrough`). Ruby defines the function on the rule's module
/// (`def self.for`) and calls it there, which a keyword can be, and its module is a constant. Java
/// writes the method with `_` after a keyword, and its class is in PascalCase.
pub(crate) fn rule_keyword(id: &str, rule: &str) -> Option<String> {
    match target_of(id)? {
        Target::Ruby => Some(super::ruby_module(rule)).filter(|m| RB_CONSTANT_KEYWORDS.contains(&m.as_str())),
        Target::Go => Some(crate::backend::go_package(rule)).filter(|p| keyword_as_written(id, p)),
        Target::Java => None,
        _ => rule_function(id, rule).filter(|f| keyword_as_written(id, f)),
    }
}

/// The keyword of its language the backend `id` writes an enum's type as, from the enum's
/// identifier `name`, if it writes one: the type is the identifier in PascalCase in every language
/// (`self` is `Self` in Rust and Swift, `none` is `None` in Python), Ruby's module of the enum is a
/// constant, and Swift keeps three more capitalized words that no type can be. PHP's keywords are
/// read in any case.
pub(crate) fn enum_type_keyword(id: &str, name: &str) -> Option<String> {
    let ty = enum_type(name);
    let keyword = match target_of(id)? {
        Target::Ruby => RB_CONSTANT_KEYWORDS.contains(&rb_const(&ty).as_str()),
        Target::Swift => ["Any", "Self", "Type", "Protocol"].contains(&ty.as_str()) || keyword_as_written(id, &ty),
        _ => keyword_as_written(id, &ty),
    };
    keyword.then_some(ty)
}

/// The keyword of its language the backend `id` writes a value of an enum as, from the value's
/// identifier `name`, if it writes one: Rust's variant is the identifier in PascalCase (`self` is
/// `Self`), and Ruby's constant is that in upper case (`begin` is `BEGIN`). Swift writes a keyword
/// in backticks and PHP with `_` after it; Python's, Java's and TypeScript's members are in upper
/// case and Go's start with the type's name, which no keyword is.
pub(crate) fn member_keyword(id: &str, name: &str) -> Option<String> {
    let m = member(name);
    match target_of(id)? {
        Target::Rust | Target::Wasm => Some(m).filter(|m| m == "Self"),
        Target::Ruby => Some(rb_const(&m.to_uppercase())).filter(|c| RB_CONSTANT_KEYWORDS.contains(&c.as_str())),
        _ => None,
    }
}

/// What a name is that a backend writes with `_` after it, a keyword of its language there
/// ([`respelled`]).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Respelt {
    /// PHP's function of the rule.
    Function,
    /// PHP's namespace of the rule, or of an `enums` file.
    Namespace,
    /// PHP's class of an enum.
    Class,
    /// PHP's case of a value, with its class (`Kind_::FINAL_`).
    Case,
    /// Java's method of the rule, in the rule's class.
    Method,
}

/// The name a [`respelled`] identifier is.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Of<'a> {
    /// The rule's name.
    Rule,
    /// The name of an `enums` file.
    EnumsFile,
    /// An enum's name.
    Enum,
    /// A value of the enum of this identifier.
    Value(&'a str),
}

/// What the backend `id` writes with `_` after the identifier `ident` of `of`, where the name as
/// the backend writes it is a keyword of its language, and its code builds under it (§15.344).
/// PHP's function and namespace of the rule (`list_`, `List_`), namespace of an `enums` file,
/// class of an enum (`Array_`) and case of a value (`Kind::FINAL_`): `php::php_name` moves each off
/// PHP's keywords, without regard to case, as the generator does. Java's method of the rule (`for_`,
/// in the class `For`). Nothing for the other backends, and where no name is a keyword.
pub(crate) fn respelled(id: &str, ident: &str, of: Of) -> Vec<(Respelt, String)> {
    use super::php::php_name;
    let moved = |w: &str| Some(php_name(w)).filter(|p| p != w);
    match (target_of(id), of) {
        (Some(Target::Php), Of::Rule) => {
            let func = moved(ident).map(|w| (Respelt::Function, w));
            let ns = moved(&pascal(ident)).map(|w| (Respelt::Namespace, w));
            func.into_iter().chain(ns).collect()
        }
        (Some(Target::Php), Of::EnumsFile) => moved(&pascal(ident)).map(|w| (Respelt::Namespace, w)).into_iter().collect(),
        (Some(Target::Php), Of::Enum) => moved(&enum_type(ident)).map(|w| (Respelt::Class, w)).into_iter().collect(),
        (Some(Target::Php), Of::Value(en)) => {
            moved(&ident.to_uppercase()).map(|w| (Respelt::Case, format!("{}::{w}", php_name(&enum_type(en))))).into_iter().collect()
        }
        (Some(Target::Java), Of::Rule) => {
            let m = super::java::java_name_pub(ident);
            if m != camel(ident) { vec![(Respelt::Method, m)] } else { Vec::new() }
        }
        _ => Vec::new(),
    }
}

/// The W121 of the name `ident` of `of`, by its alias where it has one (`aliased`), where some
/// language writes it with `_` after it, a keyword of its own there (§15.344): what each such
/// language writes, marked at `span`. None where no language does. The code builds under those
/// names, so the finding keeps no target from being written.
pub(crate) fn respelled_warning(span: &Span, ident: &str, of: Of, aliased: bool, path: &str) -> Option<Diag> {
    let said: Vec<(&'static str, Vec<(Respelt, String)>)> =
        crate::backend::ALL.iter().map(|b| (b.name, respelled(b.id, ident, of))).filter(|(_, r)| !r.is_empty()).collect();
    if said.is_empty() {
        return None;
    }
    let names: Vec<&str> = said.iter().map(|(n, _)| *n).collect();
    let langs = if crate::i18n::ja() {
        names.join("・")
    } else {
        match names.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => names.join(""),
        }
    };
    let title = if aliased {
        tr!(
            "別名 `{ident}` は {langs} で書くと予約語になるので、その生成コードでは後ろに `_` を付けて書きます",
            "The alias `{ident}` is a keyword as written in {langs}, so the generated code there writes it with `_` after it"
        )
    } else {
        tr!(
            "`{ident}` は {langs} で書くと予約語になるので、その生成コードでは後ろに `_` を付けて書きます",
            "`{ident}` is a keyword as written in {langs}, so the generated code there writes it with `_` after it"
        )
    };
    let label = match (of, aliased) {
        (Of::Rule, false) => tr!("規則の名前", "the rule's name"),
        (Of::Rule, true) => tr!("規則の別名", "the rule's alias"),
        (Of::EnumsFile, false) => tr!("enums ファイルの名前", "the enums file's name"),
        (Of::EnumsFile, true) => tr!("enums ファイルの別名", "the enums file's alias"),
        (Of::Enum, false) => tr!("列挙の名前", "the enum's name"),
        (Of::Enum, true) => tr!("列挙の別名", "the enum's alias"),
        (Of::Value(_), false) => tr!("列挙の値", "the enum's value"),
        (Of::Value(_), true) => tr!("値の別名", "the value's alias"),
    };
    let mut d = Diag::warning("W121", title).at(format!("{path}:{}", span.line)).mark(span.clone(), label);
    for (lang, r) in &said {
        let get = |k: Respelt| r.iter().find(|(x, _)| *x == k).map(|(_, w)| w.clone());
        let note = match (get(Respelt::Method), get(Respelt::Function), get(Respelt::Namespace), get(Respelt::Class), get(Respelt::Case)) {
            (Some(m), ..) => {
                let class = super::java::java_class(ident);
                tr!("{lang}: 規則のクラス `{class}` のメソッドを `{m}` と書きます。呼ぶ側もその名前で呼びます。", "{lang}: the method of the rule's class `{class}` is `{m}`, the name a caller calls it by.")
            }
            (None, Some(func), Some(ns), ..) => tr!(
                "{lang}: 規則の関数を `{func}`、名前空間を `{ns}` と書きます。呼ぶ側もその名前で読みます。",
                "{lang}: the rule's function is `{func}` and its namespace `{ns}`, the names a caller reads it by."
            ),
            (None, Some(func), None, ..) => tr!("{lang}: 規則の関数を `{func}` と書きます。呼ぶ側もその名前で呼びます。", "{lang}: the rule's function is `{func}`, the name a caller calls it by."),
            (None, None, Some(ns), ..) if of == Of::EnumsFile => tr!(
                "{lang}: enums ファイルの名前空間を `{ns}` と書きます。それを読む規則のコードも、呼ぶ側も、その名前で読みます。",
                "{lang}: the enums file's namespace is `{ns}`, the name a caller reads it by, as do the rules that read from the file."
            ),
            (None, None, Some(ns), ..) => tr!("{lang}: 規則の名前空間を `{ns}` と書きます。呼ぶ側もその名前で読みます。", "{lang}: the rule's namespace is `{ns}`, the name a caller reads it by."),
            (None, None, None, Some(class), _) => tr!("{lang}: 列挙のクラスを `{class}` と書きます。呼ぶ側もその名前で読みます。", "{lang}: the enum's class is `{class}`, the name a caller reads it by."),
            (None, None, None, None, Some(case)) => tr!("{lang}: 列挙の値を `{case}` と書きます。呼ぶ側もその名前で読みます。", "{lang}: the enum's value is `{case}`, the name a caller reads it by."),
            (None, None, None, None, None) => continue,
        };
        d = d.note(note);
    }
    let fix = if aliased {
        tr!(
            "生成コードはこのままで組めます。どの言語でも `_` の付かない名前にするなら、別名を変えてください。",
            "The code builds as it is. For a name with no `_` in any language, change the alias."
        )
    } else {
        tr!(
            "生成コードはこのままで組めます。どの言語でも `_` の付かない名前にするなら、別名を付けてください（`{ident}(<別名>)`）。",
            "The code builds as it is. For a name with no `_` in any language, give it an alias (`{ident}(<alias>)`)."
        )
    };
    Some(d.note(fix))
}

/// Every W121 of a name that some language writes with `_` after it, a keyword of its own there
/// (§15.344), but a rule's alias and an enum's, which §15.103 takes and says with the same words:
/// the rule's name with no alias, the name of an `enums` file and a value, by the alias where they
/// have one (no other stage holds them to a keyword of PHP's), and an enum's name with no alias.
pub fn respelled_warnings(f: &RuleFile, path: &str) -> Vec<Diag> {
    use crate::ast::FileKind;
    let mut out: Vec<Diag> = Vec::new();
    let own = |n: &Name| pub_name(n).is_ascii() && !super::brought_in(&n.text);
    match f.kind {
        FileKind::Rule if f.name.ascii.is_none() && own(&f.name) => out.extend(respelled_warning(&f.name.span, &pub_name(&f.name), Of::Rule, false, path)),
        FileKind::Enums if own(&f.name) => out.extend(respelled_warning(&f.name.span, &pub_name(&f.name), Of::EnumsFile, f.name.ascii.is_some(), path)),
        _ => {}
    }
    if matches!(f.kind, FileKind::Rule | FileKind::Enums) {
        for e in f.enums.iter().filter(|e| e.from.is_none() && own(&e.name)) {
            let en = pub_name(&e.name);
            if e.name.ascii.is_none() {
                out.extend(respelled_warning(&e.name.span, &en, Of::Enum, false, path));
            }
            for v in e.values.iter().filter(|v| pub_name(v).is_ascii()) {
                out.extend(respelled_warning(&v.span, &pub_name(v), Of::Value(&en), v.ascii.is_some(), path));
            }
        }
    }
    out
}

impl Meets {
    /// Whether the code of `t` meets this only where `rulec gen` writes the rule's module beside
    /// the language's own, by its name alone (`Diag::beside`): a Python module named like one of
    /// the library, which `import` finds in its place, and the service that imports it; a Go
    /// package named like a package of the library, or kept by the go command, but for `main`,
    /// which no path can import; Swift's `main.swift`. A package of rules names the module by the
    /// package's path. Ruby's module is a constant at the top whatever the files beside it, which
    /// a class of the language's meets anywhere.
    fn beside(&self, t: Target) -> bool {
        match self {
            Meets::Word { kind: Word::Module, .. } => t != Target::Ruby,
            Meets::GoPackage { name } => name != "main",
            Meets::SwiftMain => true,
            _ => false,
        }
    }

    fn said(&self, t: Target, kind: &str, n: &str) -> String {
        match self {
            Meets::Read { name, what: super::own::What::Local, written, checker } => {
                let spelled = if written == n {
                    tr!("{kind}の `{n}` もそこでは同じ名前で", "the {kind} `{n}` is that name there as well")
                } else {
                    tr!("{kind}の `{n}` もそこでは `{written}` と書かれ", "the {kind} `{n}` is written `{written}` there")
                };
                let ends = broken(*checker, t);
                tr!(
                    "規則の関数は、自分の局所変数 `{name}` を持ちます。{spelled}、二つが一つの変数になるので、{ends}",
                    "the rule's functions keep a local variable of their own, `{name}`, and {spelled}: the two are one variable, and {ends}"
                )
            }
            Meets::Read { name, what, written, checker } => {
                let spelled = if written == n {
                    tr!("{kind}の `{n}` もそこでは同じ名前で", "the {kind} `{n}` is that name there as well")
                } else {
                    tr!("{kind}の `{n}` もそこでは `{written}` と書かれ", "the {kind} `{n}` is written `{written}` there")
                };
                let ends = broken(*checker, t);
                tr!(
                    "規則の関数は、{} `{name}` を読みます。{spelled}、それを隠すので、{ends}",
                    "the rule's functions read `{name}`, {}, and {spelled}, which hides it: {ends}",
                    what.said()
                )
            }
            Meets::Declared { name, what: super::own::What::Builtin, written, .. } | Meets::ReadEnum { name, what: super::own::What::Builtin, written, .. } if t == Target::Connect => tr!(
                "`.proto` から書く Python の stub は、要求のフィールドを列挙の型の名前 `{written}` で注釈します。型の検査はそれを組み込みの `{name}` と読むので、サービスも、それを読む呼ぶ側のコードも、型の検査（mypy）が通りません。",
                "the Python stub written from the `.proto` annotates the request's field with the enum's type, `{written}`, which a type checker reads as the builtin `{name}`: neither the service nor a caller's code that reads it passes the type check (mypy)."
            ),
            Meets::Declared { name, what: super::own::What::Import, written, .. } => tr!(
                "生成コードは `{name}` を読み込みます。列挙の型も `{written}` なので、それに取って代わり、組めないか、答えを違えます。",
                "the generated code imports `{name}`, and the enum's type is `{written}` as well, which takes its place: the code does not build, or answers otherwise."
            ),
            Meets::Declared { name, what, written, caseless } => {
                if *caseless && written != name {
                    let why = match t {
                        Target::Php => tr!("PHP はクラスや列挙の名前の大文字と小文字を区別しないので", "PHP reads the names of classes and enums with case set aside, so"),
                        _ => tr!("javac は入れ子の型をクラスの名前のファイルに書き、大文字と小文字を区別しないファイルシステム（macOS と Windows の既定）では二つのファイルが一つになるので", "javac writes a nested type to a file named after it, and on a file system that sets case aside (the default of macOS and Windows) two such files are one, so"),
                    };
                    tr!(
                        "生成コードは `{name}` を{}として宣言し、列挙の型は `{written}` です。{why}、同じ名前を二度宣言することになり、組めません。",
                        "the generated code declares `{name}`, {}, and the enum's type is `{written}`: {why} one name is declared twice, which does not build.",
                        what.said()
                    )
                } else {
                    tr!(
                        "生成コードは `{name}` を{}として宣言します。列挙の型も `{name}` なので、同じ名前を二度宣言することになり、組めません。",
                        "the generated code declares `{name}`, {}, and the enum's type is `{name}` as well: one name is declared twice, which does not build.",
                        what.said()
                    )
                }
            }
            Meets::TraceField => tr!(
                "答えのメッセージ `DecideResponse` は、当てはまった行を自分のフィールド `trace` で返します。この出力のフィールドも `trace` なので、buf と protoc がエラーにします。",
                "the answer's message `DecideResponse` returns the rows that matched in a field of its own, `trace`, and this output's field is `trace` as well, which buf and protoc refuse."
            ),
            Meets::ZeroValue { name } => tr!(
                "`.proto` の列挙は、値が設定されていないフィールドが持つ値 `{name}` を自分で持ちます。この値も `{name}` になるので、buf と protoc がエラーにします。",
                "an enum of the `.proto` has a value of its own, `{name}`, which a field that is not set holds, and this value is `{name}` as well, which buf and protoc refuse."
            ),
            Meets::Message { name, service: false } => tr!(
                "`.proto` は `{name}` をメッセージとして宣言します。列挙の型も `{name}` なので、buf と protoc がエラーにします。",
                "the `.proto` declares `{name}`, a message, and the enum's type is `{name}` as well, which buf and protoc refuse."
            ),
            Meets::Message { name, service: true } => tr!(
                "`.proto` は `{name}` を規則のサービスとして宣言します。列挙の型も `{name}` なので、buf と protoc がエラーにします。",
                "the `.proto` declares `{name}`, the rule's service, and the enum's type is `{name}` as well, which buf and protoc refuse."
            ),
            Meets::RuleDeclared { name, what, written, class } => {
                let rule = if *class { tr!("規則のクラス", "the rule's class") } else { tr!("規則の関数", "the rule's function") };
                if *what == super::own::What::Import {
                    tr!(
                        "生成コードは `{name}` を読み込みます。{rule}も `{written}` なので、それに取って代わり、組めないか、答えを違えます。",
                        "the generated code imports `{name}`, and {rule} is `{written}` as well, which takes its place: the code does not build, or answers otherwise."
                    )
                } else {
                    tr!(
                        "生成コードは `{name}` を{}として宣言します。{rule}も `{written}` なので、同じ名前を二度宣言することになり、組めません。",
                        "the generated code declares `{name}`, {}, and {rule} is `{written}` as well: one name is declared twice, which does not build.",
                        what.said()
                    )
                }
            }
            Meets::GoPackage { name } => tr!(
                "規則のパッケージは `{name}` になりますが、go コマンドはこの名前を自分のために取ってあるので（`go help packages`）、走らせる側がそのパッケージを読み込めません。",
                "the rule's package is `{name}`, a name the go command keeps for itself (`go help packages`), so the runner cannot import it."
            ),
            Meets::SwiftMain => tr!(
                "規則のファイルは `main.swift` になり、swiftc はそのファイルをプログラムの最上位のコードとして読むので、走らせる側の `@main` と並べられず、組めません。",
                "the rule's file is `main.swift`, which swiftc reads as the program's top-level code, so the runner's `@main` cannot stand beside it: it does not build."
            ),
            Meets::Word { word, kind: Word::Keyword } => tr!(
                "`{word}` はこの言語の予約語なので、そう書く{kind}の名前を書けず、組めません。",
                "`{word}` is a keyword of the language, which the {kind} written so cannot be named: it does not build."
            ),
            // Java writes apart every class an enum's type hides, and those its files import where
            // a class of the package is named so: what is left is a class of `java.lang`'s.
            Meets::Word { word, kind: Word::Builtin { .. } } if t == Target::Java => tr!(
                "{kind}のクラス `{word}` は、パッケージの中で `java.lang` の `{word}` を隠します。同じディレクトリのほかのコードと丸めの試験もそれを単純名で読むので、組めないか、答えを違えます。",
                "the {kind}'s class `{word}` hides `java.lang`'s `{word}` in the whole package, which the other code of the directory and the rounding test read by its simple name as well: the code does not build, or answers otherwise."
            ),
            Meets::Word { word, kind: Word::Builtin { .. } } => tr!(
                "`{word}` はこの言語が持つ名前で、生成コードも使います。そう書く{kind}の名前がそれを隠すので、組めないか、答えを違えます。",
                "the language has `{word}` built in, which the generated code uses as well, and the {kind} written so hides it: the code does not build, or answers otherwise."
            ),
            Meets::Word { word, kind: Word::Module } => tr!(
                "標準のライブラリに `{word}` というモジュールがあり、規則のために書いたモジュールがその代わりに読まれるか、二つを読み分けられなくなります。",
                "the standard library has a module `{word}`, and the module written for the rule is read in its place, or the two cannot be told apart."
            ),
            Meets::Annotation { class, ty } => tr!(
                "`{class}` は `NamedTuple` で、あとのフィールドの型を `{ty}` と書きます。この{kind}もそこでは `{ty}` なので、mypy はその型をこのフィールドとして読み、型の検査が通りません。",
                "`{class}` is a `NamedTuple` a later field of which is of the type `{ty}`, and the {kind} is `{ty}` there as well, so mypy reads that type as this field: the code does not pass the type check."
            ),
            Meets::ModuleReads { name } => tr!(
                "モジュールの補助関数は、組み込みの `{name}` をその名前で読みます。規則の関数もそこでは `{name}` なので、補助関数はそちらを呼び、答えを違えます。",
                "the module's helpers read the builtin `{name}` by its name, and the rule's function is `{name}` there as well, which the helpers call instead: the code answers otherwise."
            ),
            Meets::GroupDeclared { name, what } => tr!(
                "グループの定数は `{name}` ですが、生成コードは同じ名前の{}を宣言します。同じ名前を二度宣言することになり、組めません。",
                "the group's constant is `{name}`, and the generated code declares {} of that name: one name is declared twice, which does not build.",
                what.said()
            ),
            Meets::ReadEnum { name, what: super::own::What::Import, written, file } => tr!(
                "生成コードは `{name}` を読み込みます。`{file}` から読む列挙も、そのファイルの型の名前 `{written}` で読み込むので、それに取って代わり、組めないか、答えを違えます。",
                "the generated code imports `{name}`, and imports the enum read from `{file}` under the file's type, `{written}`, as well, which takes its place: the code does not build, or answers otherwise."
            ),
            Meets::ReadEnum { name, what, written, file } => tr!(
                "生成コードは `{name}` を{}として宣言します。`{file}` から読む列挙も、そのファイルの型の名前 `{written}` でもう一度宣言するか読み込むので、同じ名前が二つになり、組めないか、答えを違えます。",
                "the generated code declares `{name}`, {}, and declares or imports the enum read from `{file}` under the file's type, `{written}`, as well: one name stands for two things, and the code does not build, or answers otherwise.",
                what.said()
            ),
        }
    }

    /// Whether what the note says is the `.proto`'s, which buf and protoc refuse ([`Target::ids`]).
    fn wire(&self) -> bool {
        matches!(self, Meets::TraceField | Meets::ZeroValue { .. } | Meets::Message { .. })
    }
}

/// A kind of meeting the generated code of a target can leave behind by writing its own name
/// another way, or by reaching what it reads another way (§15.336): what [`written_apart`] asks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Apart {
    /// A value hides a name the rule's functions read: the value is bound under another name there.
    Read,
    /// A field of a `NamedTuple` is named like the type a later field is annotated with.
    Annotation,
    /// An enum's type, or the rule's function or class, is named like what the module imports.
    Import,
    /// An enum's type is named like a builtin the module raises.
    Builtin,
    /// The rule's function, or class, is named like a helper of the module, or like a type of the
    /// runner's own.
    Helper,
    /// The rule's function is named like a builtin the module's helpers call.
    ModuleReads,
    /// A group's constant is named like a helper of the module.
    Group,
    /// The rule's name, or an enum's, is a name the language has built in: written apart word by
    /// word, where the generator of the language reaches the word past the name
    /// (`clash::reaches_past`).
    Word,
}

/// The kind of meeting `m` is in `t` where the generated code can leave it behind (§15.336): a name
/// of the generated code's own — a helper, what it imports, a local, a parameter a caller passes by
/// its place — or a builtin it reaches another way. None where a caller names both names: a type, a
/// public function, a field, a value of an enum, a key of the JSON, a module, a file.
fn apart_kind(t: Target, m: &Meets) -> Option<Apart> {
    use super::own::What;
    match m {
        Meets::Read { .. } => Some(Apart::Read),
        Meets::Annotation { .. } => Some(Apart::Annotation),
        Meets::Declared { what: What::Import, .. } | Meets::ReadEnum { what: What::Import, .. } => Some(Apart::Import),
        // The `.proto` names the enum's type as the rule does, and the stub the protobuf plugin
        // writes for Python annotates the request's field with it, which a type checker reads as
        // the builtin: a name of the wire's, which a caller's code reads as well.
        Meets::Declared { what: What::Builtin, .. } | Meets::ReadEnum { what: What::Builtin, .. } if t == Target::Connect => None,
        Meets::Declared { what: What::Builtin, .. } | Meets::ReadEnum { what: What::Builtin, .. } => Some(Apart::Builtin),
        Meets::RuleDeclared { what: What::Import, .. } => Some(Apart::Import),
        Meets::RuleDeclared { what: What::Helper, .. } => Some(Apart::Helper),
        // The runner's own records, the type a method of its takes, and the record the walk of a
        // pick by weight gives back, which no caller names.
        Meets::RuleDeclared { name, class: true, .. } if t == Target::Java && ["Str", "Pairs", "E", "Walked"].contains(&name.as_str()) => Some(Apart::Helper),
        Meets::ModuleReads { .. } => Some(Apart::ModuleReads),
        Meets::GroupDeclared { .. } => Some(Apart::Group),
        Meets::Word { kind: Word::Builtin { .. }, .. } => Some(Apart::Word),
        _ => None,
    }
}

/// Every W121 of an enum whose type the generated code of some language writes with a number after
/// it, because a name made of it meets one of the rule's own there (§15.326, §15.337): the type it
/// takes in each such language and the two names that met, as §15.260 says a name it numbers. The
/// code builds, so the finding keeps no target from being written.
pub fn renamed_warnings(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    if f.kind != crate::ast::FileKind::Rule {
        return Vec::new();
    }
    let g = super::Gen::for_names(f, c);
    // The languages whose module puts an enum's names beside the rule's functions, each with the
    // targets that write its code, in the order `rulec gen` writes them.
    let mut langs: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for b in crate::backend::ALL {
        let Some(l) = super::clash::speech(b.id) else { continue };
        match langs.iter_mut().find(|(x, _)| *x == l) {
            Some((_, bs)) => bs.push(b.name),
            None => langs.push((l, vec![b.name])),
        }
    }
    let renamed: Vec<(&str, std::collections::BTreeMap<String, String>)> = langs.iter().map(|(l, _)| (*l, g.renamed_types(l))).collect();
    let rule = &f.name.text;
    let mut out: Vec<Diag> = Vec::new();
    for e in f.enums.iter().filter(|e| !super::brought_in(&e.name.text)) {
        let jp = &e.name.text;
        let t = g.enum_type(jp);
        // (the targets of one language, the language, the type it takes there)
        let mut said: Vec<(Vec<&'static str>, &str, String)> = Vec::new();
        for ((l, names), (_, to)) in langs.iter().zip(&renamed) {
            if let Some(new) = to.get(&t) {
                said.push((names.clone(), l, new.clone()));
            }
        }
        if said.is_empty() {
            continue;
        }
        let names: Vec<&str> = said.iter().flat_map(|(ns, _, _)| ns.iter().copied()).collect();
        let listed_ts = if crate::i18n::ja() {
            names.join("・")
        } else {
            match names.split_last() {
                Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
                _ => names.join(""),
            }
        };
        let gap = if listed_ts.ends_with('）') { "" } else { " " };
        let one = said.iter().all(|(_, _, n)| *n == said[0].2);
        let title = if one {
            let new = &said[0].2;
            tr!("列挙 `{jp}` の型は、{listed_ts}{gap}の生成コードでは `{new}` と書きます", "The type of enum `{jp}` is written `{new}` in the code generated for {listed_ts}")
        } else {
            tr!("列挙 `{jp}` の型は、{listed_ts}{gap}の生成コードでは後ろに番号を付けて書きます", "The type of enum `{jp}` takes a number after it in the code generated for {listed_ts}")
        };
        let mut d = Diag::warning("W121", title).at(format!("{path}:{}", e.name.span.line)).mark(e.name.span.clone(), tr!("型 `{t}`", "the type `{t}`"));
        for (ns, l, new) in &said {
            let ns = if crate::i18n::ja() {
                ns.join("・")
            } else {
                match ns.split_last() {
                    Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
                    _ => ns.join(""),
                }
            };
            let fns = g.rule_fn_names(l);
            let met = g.enum_decl_names(l, jp, &t).into_iter().find(|n| fns.contains(n)).unwrap_or_else(|| t.clone());
            let made: Vec<String> = g.enum_decl_names(l, jp, new).into_iter().filter(|n| n != new).map(|n| format!("`{n}`")).collect();
            let made = made.join(if crate::i18n::ja() { "、" } else { ", " });
            let note = if *l == "java" {
                tr!(
                    "{ns}: 列挙は規則のクラス `{met}` の中に置かれ、クラスはそれ自身と同じ名前の型を持てないので、そこでは列挙の型を `{new}` と書きます。",
                    "{ns}: the enum is nested in the rule's class `{met}`, which holds no type of its own name, so the enum's type is `{new}` there."
                )
            } else {
                let place = if *l == "go" { tr!("パッケージ", "package") } else { tr!("モジュール", "module") };
                let follow = if made.is_empty() {
                    String::new()
                } else {
                    tr!("型から作る名前もそれに従います（{made}）。", " The names made of it follow ({made}).")
                };
                tr!(
                    "{ns}: 規則の{place}には、規則の `{met}` があります。列挙から作る名前にも `{met}` があり、一つの{place}に同じ名前は二つ置けないので、そこでは列挙の型を `{new}` と書きます。{follow}",
                    "{ns}: the rule's {place} declares `{met}` for the rule, and one of the names made of the enum is `{met}` as well; a {place} holds one thing of a name, so the enum's type is `{new}` there.{follow}"
                )
            };
            d = d.note(note);
        }
        d = d.note(tr!(
            "規則の関数の名前は、どの言語でも変わりません。ここに挙げていない言語は、型を `{t}` と書きます。言語ごとの名前は `rulec api` が挙げます。",
            "The rule's functions keep their names in every language, and a language not named here writes the type as `{t}`; `rulec api` gives the names of each language."
        ));
        let fix = match &e.from {
            Some(o) => {
                let file = &o.file;
                tr!(
                    "どの言語でも型を `{t}` と書くなら、規則に別名を付けるか（`rule {rule}(<別名>)`）、`{file}` でその列挙に別名を付けてください。",
                    "To have every language write the type as `{t}`, give the rule an alias (`rule {rule}(<alias>)`), or the enum one in `{file}`."
                )
            }
            None => tr!(
                "どの言語でも型を `{t}` と書くなら、規則か列挙に別名を付けてください（`rule {rule}(<別名>)`、`enum {jp}(<別名>) = …`）。",
                "To have every language write the type as `{t}`, give the rule or the enum an alias (`rule {rule}(<alias>)`, `enum {jp}(<alias>) = …`)."
            ),
        };
        out.push(d.note(fix));
    }
    out
}

/// Whether the generated code of `t` writes the rule's name apart from what `m` says it meets there,
/// so that W121 does not say it (§15.336).
pub(super) fn written_apart(t: Target, m: &Meets) -> bool {
    match (apart_kind(t, m), m) {
        (Some(Apart::Word), Meets::Word { word, kind: Word::Builtin { ty } }) => backend_of(t).is_some_and(|id| super::clash::reaches_past(id, word, *ty)),
        (Some(_), _) => true,
        (None, _) => false,
    }
}

/// Whether the generated code of the backend `id` reaches the builtin `word` another way where the
/// rule's function (`ty` false) or an enum's type (`ty` true) of that name, at the top of its
/// module, hides it, so that W121 does not say an alias that does (§15.336). `word` is the name as
/// the backend writes it.
pub(crate) fn builtin_written_apart(id: &str, word: &str, ty: bool) -> bool {
    target_of(id).is_some_and(|t| written_apart(t, &Meets::Word { word: word.to_string(), kind: Word::Builtin { ty } }))
}

/// How the code breaks where a name of the rule's hides one the generated code reads: it does not
/// build or answers otherwise, or, where only a type checker reads the name, the check fails.
fn broken(checker: bool, t: Target) -> String {
    let tool = if matches!(t, Target::TypeScript) { "tsc" } else { "mypy" };
    if checker {
        tr!("型の検査（{tool}）が通りません。", "the type check ({tool}) does not pass.")
    } else {
        tr!("組めないか、答えを違えます。", "the code does not build, or answers otherwise.")
    }
}

/// The names of the rule's that a target spells as one its generated code gives itself, each with
/// the targets and what it meets there.
#[derive(Default)]
struct Takes<'a>(Vec<(Decl<'a>, Vec<(Meets, Vec<Target>)>)>, Vec<Taken>);

/// One name of the rule's that the code of a target writes as one of its own there, and what it
/// meets: every one, where W121 says only the first of each target (§15.265, §15.336).
#[derive(Clone, Debug)]
pub(super) struct Taken {
    /// The name as the rule declares it.
    pub(super) name: String,
    pub(super) target: Target,
    pub(super) meets: Meets,
}

impl<'a> Takes<'a> {
    fn add(&mut self, d: &Decl<'a>, t: Target, m: Meets) {
        if !self.1.iter().any(|x| x.name == d.name.text && x.target == t && x.meets == m) {
            self.1.push(Taken { name: d.name.text.clone(), target: t, meets: m.clone() });
        }
        // What the generator of the target writes apart is not said (§15.336).
        if written_apart(t, &m) {
            return;
        }
        let at = |x: &Decl| (x.name.span.line, x.name.span.col, x.name.text.clone());
        let i = match self.0.iter().position(|(x, _)| at(x) == at(d)) {
            Some(i) => i,
            None => {
                self.0.push((d.clone(), Vec::new()));
                self.0.len() - 1
            }
        };
        let notes = &mut self.0[i].1;
        // One target is said once for a name, with the first thing it meets there.
        if notes.iter().any(|(_, ts)| ts.contains(&t)) {
            return;
        }
        match notes.iter_mut().find(|(x, _)| *x == m) {
            Some((_, ts)) => ts.push(t),
            None => notes.push((m, vec![t])),
        }
    }
}

/// The targets whose generated code writes each kind of name, in the order the notes list them.
const VALUE_TARGETS: [Target; 11] = [Target::Python, Target::TypeScript, Target::JavaScript, Target::Rust, Target::Go, Target::Swift, Target::Java, Target::Sqlite, Target::Postgres, Target::Wasm, Target::Connect];
const TYPE_TARGETS: [Target; 11] = [Target::Python, Target::TypeScript, Target::JavaScript, Target::Rust, Target::Ruby, Target::Php, Target::Go, Target::Swift, Target::Java, Target::Wasm, Target::Connect];

/// A value as `t` writes it where the rule's functions hold it, or none where they do not: Go reads
/// the inputs and the sequences as fields of `in`, PHP writes every variable with `$`, which no
/// name of the module's has, and SQL writes no rule that walks a sequence.
fn value_in(t: Target, d: &Decl, f: &RuleFile) -> Option<String> {
    let passed = f.inputs.iter().any(|i| i.name.text == d.name.text) || f.elements.iter().any(|el| el.name.text == d.name.text);
    match t {
        Target::Python | Target::Connect | Target::TypeScript | Target::JavaScript | Target::Rust | Target::Wasm => Some(d.id.clone()),
        Target::Go => (!passed).then(|| d.id.clone()),
        Target::Swift => Some(camel(&d.id)),
        Target::Java => Some(super::java::java_name_pub(&d.id)),
        Target::Sqlite | Target::Postgres => f.elements.is_empty().then(|| d.id.clone()),
        Target::Ruby | Target::Php => None,
    }
}

/// Every W121 of a name of the rule's that its generated code writes as one it gives itself
/// (§15.265): a value that hides what the rule's functions read, an enum whose type the module
/// declares already, an output named like the field of the answer the rows that matched travel
/// in, a value of an enum named like the zero value of the `.proto`'s enums, and a wire enum named
/// like a message or the service of the `.proto`.
fn takes_own(f: &RuleFile, c: &Checked, path: &str, vs: &[Decl], own_enums: &[&crate::ast::EnumDecl], wire: &[String]) -> Vec<Diag> {
    let takes = takes(f, c, vs, own_enums, wire);
    said_takes(takes, path)
}

/// Every name of the rule's that the code of some target writes as one of its own there, with each
/// thing it meets in each target: what the generators read to write the two apart (§15.336). None
/// for a file the check stops at, or that is not a rule.
pub(super) fn taken(f: &RuleFile, c: &Checked) -> Vec<Taken> {
    if f.kind != crate::ast::FileKind::Rule || crate::has_error(&c.diags) {
        return Vec::new();
    }
    let made = super::made_identifiers(f);
    let vs = values(f, &made);
    let own: Vec<&crate::ast::EnumDecl> = f.enums.iter().filter(|e| e.from.is_none() && !super::brought_in(&e.name.text)).collect();
    let wire = wire_enums(f, c);
    takes(f, c, &vs, &own, &wire).1
}

/// The W121 of the name of an `enums` file (§15.265): each language writes the file as a module, a
/// class or a package of that name (its alias, else the name; `Gen::enums_module_files`), which the
/// module of each rule that reads from the file imports or reads by the name, and which is held to
/// the language's words as the rule's name is. What the language writes as a file and reads by its
/// path (TypeScript, JavaScript) meets nothing, nor does PHP's namespace, which takes `_` after a
/// keyword and builds (`respelled_warnings` says so, §15.344); Wasm, SQL and NumPy write the types
/// into the rule's own code.
fn enums_file_warnings(f: &RuleFile, path: &str) -> Vec<Diag> {
    let id = pub_name(&f.name);
    if super::brought_in(&f.name.text) || !id.is_ascii() {
        return Vec::new();
    }
    let d = plain(&f.name, tr!("enums ファイル", "enums file"));
    // (the targets, why) and the targets whose meeting is only where `rulec gen` writes the module
    // beside the language's own (`Diag::beside`): a package of ritsu's names it by its path.
    let mut notes: Vec<(Vec<Target>, String)> = Vec::new();
    let mut beside: Vec<Target> = Vec::new();
    let copies = |w: &[&str], x: &str| w.contains(&x);
    // Python: the rule's module reads the types with `from <id> import …`, and the service of
    // Connect runs that module.
    if keyword_as_written("python", &id) {
        notes.push((vec![Target::Python, Target::Connect], tr!(
            "`{id}` はこの言語の予約語で、規則のモジュールは enums ファイルのモジュールをその名前で読み込む（`from {id} import …`）ので、読み込めません。",
            "`{id}` is a keyword of the language, and the rule's module imports the enums file's module by that name (`from {id} import …`): it does not load."
        )));
    } else if copies(ritsu_emit::copies::rulec::PY_STDLIB, &id) {
        notes.push((vec![Target::Python, Target::Connect], tr!(
            "標準のライブラリに `{id}` というモジュールがあり、規則のモジュールは enums ファイルのモジュールを同じ名前で読み込む（`from {id} import …`）ので、二つを読み分けられません。ライブラリのモジュールが先に読み込まれていれば、規則のモジュールは読み込めません。",
            "the standard library has a module `{id}`, and the rule's module imports the enums file's module by the same name (`from {id} import …`), so the two cannot be told apart: where the library's is loaded first, the rule's module does not load."
        )));
        beside.extend([Target::Python, Target::Connect]);
    }
    // Rust: the runner declares the module at the root of the crate, `mod <id>;`.
    if keyword_as_written("rust", &id) {
        notes.push((vec![Target::Rust], tr!(
            "`{id}` はこの言語の予約語で、enums ファイルのモジュールはその名前で宣言される（`mod {id};`）ので、組めません。",
            "`{id}` is a keyword of the language, and the enums file's module is declared by that name (`mod {id};`): it does not build."
        )));
    } else if id == "std" {
        notes.push((vec![Target::Rust], tr!(
            "enums ファイルのモジュールはクレートの根に `std` として宣言され、生成コードがその名前で読む標準のライブラリのクレートに取って代わるので、組めません。",
            "the enums file's module is declared at the root of the crate as `std`, which takes the place of the standard library's crate the generated code reads by that name: it does not build."
        )));
    }
    // Ruby: the module is a constant at the top, `module <Id>`.
    let rb = super::ruby_module(&id);
    if RB_CONSTANT_KEYWORDS.contains(&rb.as_str()) {
        notes.push((vec![Target::Ruby], tr!(
            "`{rb}` はこの言語の予約語で、enums ファイルのモジュールはその名前の定数（`module {rb}`）なので、読み込めません。",
            "`{rb}` is a keyword of the language, and the enums file's module is a constant of that name (`module {rb}`): it does not load."
        )));
    } else if copies(ritsu_emit::copies::rulec::RB_CORE, &rb) {
        notes.push((vec![Target::Ruby], tr!(
            "この言語は `{rb}` を持ち、enums ファイルのモジュールは同じ名前の定数（`module {rb}`）なので、そのクラスかモジュールを開き直します。クラスならモジュールとして開けずに読み込めず、モジュールなら列挙の型を言語のモジュールに入れます。",
            "the language has `{rb}` built in, and the enums file's module is a constant of the same name (`module {rb}`), which opens it again: a class is no module, and the generated module does not load; a module takes the enums' types into the language's own."
        )));
    }
    // Go: the package, `package <pkg>`, which the rule's package imports by the path `<pkg>` and
    // reads by its name.
    let pkg = crate::backend::go_package(&id);
    if keyword_as_written("go", &pkg) {
        notes.push((vec![Target::Go], tr!(
            "`{pkg}` はこの言語の予約語で、enums ファイルのパッケージはその名前（`package {pkg}`）なので、組めません。",
            "`{pkg}` is a keyword of the language, and the enums file's package is named so (`package {pkg}`): it does not build."
        )));
    } else if copies(ritsu_emit::copies::rulec::GO_GLOBAL, &pkg) {
        notes.push((vec![Target::Go], tr!(
            "この言語は `{pkg}` を持ち、規則のパッケージは enums ファイルのパッケージをその名前で読み込むので、規則のファイルの中ではそれを隠し、組めません。",
            "the language has `{pkg}` built in, and the rule's package imports the enums file's package by that name, which hides it in the rule's file: the code does not build."
        )));
    } else if copies(ritsu_emit::copies::rulec::GO_STD, &pkg) {
        notes.push((vec![Target::Go], tr!(
            "標準のライブラリに `{pkg}` というパッケージがあり、規則のパッケージは enums ファイルのパッケージをパス `{pkg}` で読み込むので、go コマンドが二つを読み分けられず、組めません。",
            "the standard library has a package `{pkg}`, and the rule's package imports the enums file's package by the path `{pkg}`, which the go command cannot tell from the library's: it does not build."
        )));
        beside.push(Target::Go);
    } else if ["main", "cmd", "tool", "work", "all", "std"].contains(&pkg.as_str()) {
        notes.push((vec![Target::Go], tr!(
            "enums ファイルのパッケージは `{pkg}` になりますが、go コマンドはこの名前を自分のために取ってあるので（`go help packages`）、規則のパッケージがそれを読み込めません。",
            "the enums file's package is `{pkg}`, a name the go command keeps for itself (`go help packages`), so the rule's package cannot import it."
        )));
        if pkg != "main" {
            beside.push(Target::Go);
        }
    }
    // Swift: the file, `<id>.swift`, compiled with the rule's and the runner's.
    if id == "main" {
        notes.push((vec![Target::Swift], tr!(
            "enums ファイルの Swift のファイルは `main.swift` になり、swiftc はそのファイルをプログラムの最上位のコードとして読むので、走らせる側の `@main` と並べられず、組めません。",
            "the enums file's Swift file is `main.swift`, which swiftc reads as the program's top-level code, so the runner's `@main` cannot stand beside it: it does not build."
        )));
    }
    // Java: the class, `final class <Id>`, in the package of the rules that read from it. A class
    // the files import is written apart by its full name; one of `java.lang` is hidden in the whole
    // package (`java::reaches_past`).
    let class = super::java::java_class(&id);
    if copies(ritsu_emit::copies::rulec::JAVA_GLOBAL, &class) && !super::java::reaches_past(&class, false) {
        notes.push((vec![Target::Java], tr!(
            "enums ファイルのクラス `{class}` は、パッケージの中で `java.lang` の `{class}` を隠します。規則のコードと丸めの試験もそれを単純名で読むので、組めないか、答えを違えます。",
            "the enums file's class `{class}` hides `java.lang`'s `{class}` in the whole package, which the rules' code and the rounding test read by its simple name: the code does not build, or answers otherwise."
        )));
    }
    if notes.is_empty() {
        return Vec::new();
    }
    let mut ts: Vec<Target> = notes.iter().flat_map(|(ts, _)| ts.iter().copied()).collect();
    ts.sort();
    ts.dedup();
    beside.sort();
    beside.dedup();
    let listed_ts = listed(&ts);
    let gap = if listed_ts.ends_with('）') { "" } else { " " };
    let mut diag = Diag::warning(
        "W121",
        tr!("`{}` は、{listed_ts}{gap}の生成コードが自分で使う名前と同じになります", "`{}` takes a name the code generated for {listed_ts} uses itself", d.name.text),
    )
    .at(format!("{path}:{}", d.name.span.line))
    .mark(d.name.span.clone(), label(&d))
    .targets(&ids_of(&ts, false))
    .beside(&ids_of(&beside, false));
    for (nts, why) in notes {
        diag = diag.note(format!("{}: {why}", listed(&nts)));
    }
    vec![diag.note(match d.name.ascii {
        Some(_) => tr!("使わない生成先なら、このままで構いません。使うなら、別名を変えてください。", "For a target you do not generate, leave it. For one you do, change the alias."),
        None => tr!(
            "使わない生成先なら、このままで構いません。使うなら、別名を付けてください（`{}(<別名>)`）。",
            "For a target you do not generate, leave it. For one you do, give it an alias (`{}(<alias>)`).",
            d.name.text
        ),
    })]
}

/// The names of [`takes_own`], with what each meets in each target.
fn takes<'a>(f: &'a RuleFile, c: &Checked, vs: &[Decl<'a>], own_enums: &[&'a crate::ast::EnumDecl], wire: &[String]) -> Takes<'a> {
    let has = super::own::Has::of(f, c);
    let mut takes = Takes::default();
    let passed = |d: &Decl| f.inputs.iter().any(|i| i.name.text == d.name.text) || f.elements.iter().any(|el| el.name.text == d.name.text);

    // The values: what the rule's functions read. A value is a parameter (an input, a sequence) or
    // a local declared by the item that decides it; in Go and Rust a local hides only what is read
    // after it.
    let declared_at = |d: &Decl| -> Option<(usize, Option<usize>)> {
        if passed(d) {
            return None;
        }
        let k = f.items.iter().position(|it| match it {
            Item::Derived(x) => x.name.text == d.name.text,
            Item::Define(x) => x.name.text == d.name.text,
            Item::Agg(x) => x.name.text == d.name.text,
            Item::Fold(x) => x.answer.as_ref().is_some_and(|a| a.name.text == d.name.text),
            Item::Table(x) => x.outputs.iter().any(|o| o.name.text == d.name.text),
        })?;
        has.declared.get(k).copied()
    };
    for t in VALUE_TARGETS {
        let reads = super::own::reads(t, f, &has);
        for d in vs {
            let Some(written) = value_in(t, d, f) else { continue };
            let caseless = t == Target::Sqlite;
            let at = declared_at(d);
            let hides = |r: &super::own::Own| -> bool {
                if r.inputs_only && !passed(d) {
                    return false;
                }
                // What only a float or a type checker reaches: the service is given whole numbers by
                // the wire, and JavaScript has no type checker.
                match r.reach {
                    super::own::Reach::Float if t == Target::Connect => return false,
                    super::own::Reach::Typecheck if matches!(t, Target::Connect | Target::JavaScript) => return false,
                    _ => {}
                }
                // A local the generated code keeps for itself takes another name where a name of the
                // rule's is the same as written (`Gen::temp`).
                if r.apart && (d.id == r.name || d.name.text == r.name) {
                    return false;
                }
                let same = if caseless { r.name.eq_ignore_ascii_case(&written) } else { r.name == written };
                if !same {
                    return false;
                }
                match (t, at, r.at) {
                    // A local hides what is read after it, in its own block or in one inside it.
                    (Target::Go | Target::Rust | Target::Wasm, Some((p, lp)), super::own::At::Pos { pos, lp: rlp }) => pos > p && (lp.is_none() || lp == rlp),
                    (Target::Go | Target::Rust | Target::Wasm, Some((_, lp)), super::own::At::End) => lp.is_none(),
                    (Target::Go, None, _) => false,
                    _ => true,
                }
            };
            if let Some(r) = reads.iter().find(|r| hides(r)) {
                takes.add(d, t, Meets::Read { name: r.name.clone(), what: r.what, written, checker: r.reach == super::own::Reach::Typecheck });
            }
        }
    }

    // Go declares the outputs of one table one after another, each with its type: an output named
    // like the type of one after it hides that type from its declaration.
    for (k, tr) in &has.tables_read {
        let Item::Table(t) = &f.items[*k] else { continue };
        for (i, oc) in t.outputs.iter().enumerate() {
            let Some(d) = vs.iter().find(|d| d.name.text == oc.name.text) else { continue };
            if let Some(ty) = tr.go_types.iter().skip(i + 1).find(|ty| **ty == d.id) {
                let what = if matches!(ty.as_str(), "int64" | "bool" | "string") { super::own::What::Builtin } else { super::own::What::Type };
                takes.add(d, Target::Go, Meets::Read { name: ty.clone(), what, written: d.id.clone(), checker: false });
            }
        }
    }

    // The types of the enums the rule declares itself, beside the module's own types.
    for e in own_enums {
        let d = plain(&e.name, tr!("列挙", "enum"));
        let ty = enum_type(&pub_name(&e.name));
        for t in TYPE_TARGETS {
            let caseless = matches!(t, Target::Php | Target::Java);
            // The service calls the rule's function and nothing else of its module: the type of
            // the answers' chances is read only where they are counted.
            let tops: Vec<super::own::Own> = super::own::tops(t, &has).into_iter().filter(|o| !(t == Target::Connect && o.name == "Chances")).collect();
            let found = tops.iter().find(|o| if caseless { o.name.eq_ignore_ascii_case(&ty) } else { o.name == ty });
            if let Some(o) = found {
                // Said as one with the targets that read the names as they are, where the two are
                // the same as written.
                let caseless = caseless && o.name != ty;
                takes.add(&d, t, Meets::Declared { name: o.name.clone(), what: o.what, written: ty.clone(), caseless });
            }
        }
        if wire.contains(&e.name.text) {
            if let Some(m) = super::own::proto_messages(f, &has).into_iter().find(|m| *m == ty) {
                let service = m.ends_with("Service") && m == format!("{}Service", pascal(&pub_name(&f.name)));
                takes.add(&d, Target::Connect, Meets::Message { name: m, service });
            }
            // Every enum of the `.proto` has its zero value, `<TYPE>_UNSPECIFIED`.
            let prefix = crate::proto::upper_snake(&ty);
            let zero = format!("{prefix}_UNSPECIFIED");
            for v in &e.values {
                let dv = plain(v, tr!("列挙の値", "enum value"));
                if format!("{prefix}_{}", member(&pub_name(v)).to_uppercase()) == zero {
                    takes.add(&dv, Target::Connect, Meets::ZeroValue { name: zero.clone() });
                }
            }
        }
    }

    // The types of the enums the rule reads from `enums` files, under the file's type (§15.212),
    // beside the module's own types (§15.336). Go's package and Java's class name such an enum by the
    // file's package and class (§15.239), and meet nothing here; the other languages import it, or
    // declare it again, under its type, and the `.proto` declares one on the wire beside its
    // messages.
    for e in f.enums.iter().filter(|e| !super::brought_in(&e.name.text)) {
        let Some(o) = &e.from else { continue };
        let d = plain(&e.name, tr!("列挙", "enum"));
        let ty = enum_type(&pub_name(&o.name));
        for t in TYPE_TARGETS.into_iter().filter(|t| !matches!(t, Target::Go | Target::Java)) {
            let caseless = t == Target::Php;
            let tops: Vec<super::own::Own> = super::own::tops(t, &has).into_iter().filter(|o| !(t == Target::Connect && o.name == "Chances")).collect();
            if let Some(top) = tops.iter().find(|x| if caseless { x.name.eq_ignore_ascii_case(&ty) } else { x.name == ty }) {
                takes.add(&d, t, Meets::ReadEnum { name: top.name.clone(), what: top.what, written: ty.clone(), file: o.file.clone() });
            }
        }
        if wire.contains(&e.name.text) {
            if let Some(m) = super::own::proto_messages(f, &has).into_iter().find(|m| *m == ty) {
                let service = m.ends_with("Service") && m == format!("{}Service", pascal(&pub_name(&f.name)));
                takes.add(&d, Target::Connect, Meets::Message { name: m, service });
            }
            let prefix = crate::proto::upper_snake(&ty);
            let zero = format!("{prefix}_UNSPECIFIED");
            for v in &e.values {
                let dv = plain(v, tr!("列挙の値", "enum value"));
                if format!("{prefix}_{}", member(&pub_name(v)).to_uppercase()) == zero {
                    takes.add(&dv, Target::Connect, Meets::ZeroValue { name: zero.clone() });
                }
            }
        }
    }

    // The outputs, beside the field of the answer the rows that matched travel in.
    for d in vs.iter().filter(|d| f.outputs.iter().any(|o| o.name.text == d.name.text)) {
        if d.id == "trace" {
            takes.add(d, Target::Connect, Meets::TraceField);
        }
    }

    // Python's `NamedTuple`s: the outputs of a rule of two or more, and the fields of each element.
    // A field's name is a name of the class's body from where it is declared, so mypy reads the
    // annotation of a later field by it.
    let g = super::Gen::for_names(f, c);
    let made = super::made_identifiers(f);
    let annotation = |n: &Name| g.py_ty(&c.ty_of(&n.text).unwrap_or(Ty::Unknown));
    let mut classes: Vec<(String, Vec<(Decl, String)>)> = Vec::new();
    if f.outputs.len() >= 2 {
        let fields: Vec<(Decl, String)> = f.outputs.iter().filter_map(|o| vs.iter().find(|d| d.name.text == o.name.text).map(|d| (d.clone(), annotation(&o.name)))).collect();
        classes.push(("Output".into(), fields));
    }
    for (k, el) in f.elements.iter().enumerate() {
        let fields: Vec<(Decl, String)> = el
            .fields
            .iter()
            .filter(|fd| !super::brought_in(&fd.name.text) && !made.values.contains_key(&fd.name.text))
            .map(|fd| (Decl { name: &fd.name, id: pub_name(&fd.name), what: tr!("要素のフィールド", "element field") }, annotation(&fd.name)))
            .collect();
        classes.push((has.element_types.get(k).cloned().unwrap_or_else(|| "Element".into()), fields));
    }
    for (class, fields) in &classes {
        for (i, (d, _)) in fields.iter().enumerate() {
            let later = fields[i + 1..].iter().any(|(_, ann)| ann.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')).any(|w| w == d.id));
            if later {
                takes.add(d, Target::Python, Meets::Annotation { class: class.clone(), ty: d.id.clone() });
            }
        }
    }

    // The rule's function, class and package, beside what the module and the runner declare.
    let rule = plain(&f.name, tr!("規則", "rule"));
    if !super::brought_in(&f.name.text) {
        let id = rule.id.clone();
        let go_fn = pascal(&id);
        let go_family = [go_fn.clone(), format!("{go_fn}Traced"), format!("{go_fn}Record"), super::chances::chances_fn_name(&id, "go")];
        for o in super::own::tops(Target::Go, &has) {
            if let Some(w) = go_family.iter().find(|w| **w == o.name) {
                takes.add(&rule, Target::Go, Meets::RuleDeclared { name: o.name.clone(), what: o.what, written: w.clone(), class: false });
            }
        }
        let pkg = crate::backend::go_package(&id);
        if ["main", "cmd", "tool", "work", "all", "std"].contains(&pkg.as_str()) {
            takes.add(&rule, Target::Go, Meets::GoPackage { name: pkg });
        }
        let class = super::java::java_class(&id);
        let mut java: Vec<(String, super::own::What)> = super::own::tops(Target::Java, &has).into_iter().map(|o| (o.name, o.what)).collect();
        // The runner's own records, and the type a method of its takes, where it names the rule's
        // class.
        java.extend(["Str", "Pairs", "E"].map(|n| (n.to_string(), super::own::What::Type)));
        if let Some((n, w)) = java.iter().find(|(n, _)| *n == class) {
            takes.add(&rule, Target::Java, Meets::RuleDeclared { name: n.clone(), what: *w, written: class.clone(), class: true });
        }
        for t in [Target::Rust, Target::Wasm] {
            if let Some(n) = super::own::rust_fns(&has).into_iter().find(|n| *n == id) {
                takes.add(&rule, t, Meets::RuleDeclared { name: n, what: super::own::What::Helper, written: id.clone(), class: false });
            }
        }
        if id == "main" {
            takes.add(&rule, Target::Swift, Meets::SwiftMain);
        }
        // Python's module: the rule's functions beside what the module imports and declares, and
        // the builtins its helpers read. A type of the module's is read where a call is answered,
        // and so is a builtin, in the module and in the service of Connect that runs it; what it
        // imports for its annotations is read by a type checker alone.
        let projects = super::Gen::for_names(f, c).projects();
        let py_family = [id.clone(), format!("{id}_traced"), format!("{id}_record"), format!("{id}_from"), super::chances::chances_fn_name(&id, "python")];
        let mut py: Vec<(String, super::own::What, bool)> = super::own::tops(Target::Python, &has)
            .into_iter()
            .filter(|o| !["NamedTuple", "NewType", "ValueError", "AssertionError"].contains(&o.name.as_str()))
            .map(|o| (o.name, o.what, true))
            .collect();
        py.extend(["annotations", "NamedTuple", "NewType", "enum", "Any"].map(|n| (n.to_string(), super::own::What::Import, false)));
        if projects {
            py.push(("cast".into(), super::own::What::Import, true));
        }
        if let Some((n, w, runs, written)) = py.iter().find_map(|(n, w, runs)| py_family.iter().find(|x| *x == n).map(|x| (n.clone(), *w, *runs, x.clone()))) {
            takes.add(&rule, Target::Python, Meets::RuleDeclared { name: n.clone(), what: w, written: written.clone(), class: false });
            if runs {
                takes.add(&rule, Target::Connect, Meets::RuleDeclared { name: n, what: w, written, class: false });
            }
        }
        if let Some(n) = super::own::py_reads(projects).into_iter().find(|n| py_family.iter().any(|x| x == n)) {
            for t in [Target::Python, Target::Connect] {
                takes.add(&rule, t, Meets::ModuleReads { name: n.to_string() });
            }
        }
        // PHP's namespace of the rule: the rule's functions beside the builtins its helpers call.
        let php_family = [id.clone(), format!("{id}_traced"), format!("{id}_record"), format!("{id}_from"), super::chances::chances_fn_name(&id, "php")];
        if let Some(n) = super::own::php_reads(&has, projects).into_iter().find(|n| php_family.iter().any(|x| x.eq_ignore_ascii_case(n))) {
            takes.add(&rule, Target::Php, Meets::ModuleReads { name: n.to_string() });
        }
        // The service's module declares the rule's service as a class named after the rule, and one
        // more with `Sync` after it, beside the names it imports.
        let service = pascal(&id);
        let mut imports: Vec<&str> = vec!["Code", "ConnectError", "Any", "TypeVar", "Iterable", "Mapping", "WSGIServer", "StartResponse", "WSGIApplication", "WSGIEnvironment", "RequestContext"];
        if !wire.is_empty() {
            imports.push("E");
        }
        for written in [service.clone(), format!("{service}Sync")] {
            if let Some(n) = imports.iter().find(|n| **n == written) {
                takes.add(&rule, Target::Connect, Meets::RuleDeclared { name: n.to_string(), what: super::own::What::Import, written, class: true });
                break;
            }
        }
    }

    // The rule's name and an enum's where they have no alias are the identifiers themselves, held to
    // the words of each language as §15.103 and §15.149 hold an alias, in the spelling each language
    // writes them in: an alias is said there.
    if f.name.ascii.is_none() && !super::brought_in(&f.name.text) && f.name.text.is_ascii() {
        let id = rule.id.clone();
        for b in crate::backend::ALL {
            let Some(t) = target_of(b.id) else { continue };
            let caseless = t == Target::Php;
            let is = |w: &str, x: &str| if caseless { w.eq_ignore_ascii_case(x) } else { w == x };
            // The function, and the class, the module or the package the rule is.
            let Some(func) = rule_function(b.id, &id) else { continue };
            let module: Option<String> = match t {
                Target::Python => Some(id.clone()),
                Target::Ruby => Some(super::ruby_module(&id)),
                Target::Go => Some(crate::backend::go_package(&id)),
                _ => None,
            };
            // The service of Connect imports the Python module by its name, which a keyword cannot
            // be; a builtin its helpers read is said above.
            let also: &[Target] = if t == Target::Python { &[Target::Python, Target::Connect] } else { std::slice::from_ref(&t) };
            if let Some(word) = rule_keyword(b.id, &id) {
                // PHP writes its function and its namespace with `_` after a keyword, and its code
                // builds: `respelled_warnings` says so, and keeps no target from being written
                // (§15.344).
                if respelled(b.id, &id, Of::Rule).is_empty() {
                    for &t in also {
                        takes.add(&rule, t, Meets::Word { word: word.clone(), kind: Word::Keyword });
                    }
                }
            } else if b.globals.iter().any(|w| is(w, &func)) {
                takes.add(&rule, t, Meets::Word { word: func.clone(), kind: Word::Builtin { ty: false } });
            } else if let Some(m) = module.filter(|m| b.modules.iter().any(|w| w.eq_ignore_ascii_case(m))) {
                takes.add(&rule, t, Meets::Word { word: m.clone(), kind: Word::Module });
                if t == Target::Python {
                    takes.add(&rule, Target::Connect, Meets::Word { word: m, kind: Word::Module });
                }
            }
        }
    }
    for e in own_enums.iter().filter(|e| e.name.ascii.is_none() && e.name.text.is_ascii() && !super::brought_in(&e.name.text)) {
        let d = plain(&e.name, tr!("列挙", "enum"));
        let ty = enum_type(&d.id);
        for b in crate::backend::ALL {
            let Some(t) = target_of(b.id) else { continue };
            if matches!(t, Target::Sqlite | Target::Postgres) {
                continue;
            }
            // PHP keeps its classes apart from its functions, so a type named like a builtin
            // function (`Count`) meets nothing there; its keywords name no class.
            let php = t == Target::Php;
            // Ruby nests the enum's module in the rule's, where it hides a class of the language's only
            // from the code there that reads that class.
            if let Some(word) = enum_type_keyword(b.id, &d.id) {
                // PHP writes its class with `_` after a keyword (§15.344): `respelled_warnings`.
                if respelled(b.id, &d.id, Of::Enum).is_empty() {
                    takes.add(&d, t, Meets::Word { word, kind: Word::Keyword });
                }
            } else if !php && (b.globals.iter().any(|w| *w == ty) || (t == Target::Ruby && super::own::rb_reads().contains(&ty.as_str()))) {
                takes.add(&d, t, Meets::Word { word: ty.clone(), kind: Word::Builtin { ty: true } });
            }
        }
    }

    // The groups, whose constants TypeScript and JavaScript write as `_<group>` beside the module's
    // helpers.
    for gr in f.groups.iter().filter(|g| g.from.is_none() && !super::brought_in(&g.name.text)) {
        let d = plain(&gr.name, tr!("グループ", "group"));
        let constant = format!("_{}", d.id);
        for t in [Target::TypeScript, Target::JavaScript] {
            if let Some(n) = super::own::ts_helpers(&has).into_iter().find(|n| *n == constant) {
                takes.add(&d, t, Meets::GroupDeclared { name: n, what: super::own::What::Helper });
            }
        }
    }

    takes
}

/// The W121 of each name of [`Takes`], with the first thing it meets in each target.
fn said_takes(takes: Takes, path: &str) -> Vec<Diag> {
    let mut out: Vec<Diag> = Vec::new();
    let at = |line: usize| format!("{path}:{line}");
    // What the `.proto` declares as well, which W121 says only the first thing of: buf and protoc
    // refuse the `.proto` then, which is not written either.
    let all = takes.1;
    let mut found = takes.0;
    found.sort_by_key(|(d, _)| place(&d.name.span));
    for (d, notes) in found {
        let mut ts: Vec<Target> = notes.iter().flat_map(|(_, ts)| ts.iter().copied()).collect();
        ts.sort();
        ts.dedup();
        let listed_ts = listed(&ts);
        let gap = if listed_ts.ends_with('）') { "" } else { " " };
        let wire = all.iter().any(|x| x.name == d.name.text && x.target == Target::Connect && x.meets.wire());
        // A target the name meets only beside the language's modules, for every reason it meets it.
        let beside: Vec<Target> = ts.iter().copied().filter(|t| notes.iter().filter(|(_, nts)| nts.contains(t)).all(|(m, _)| m.beside(*t))).collect();
        let mut diag = Diag::warning(
            "W121",
            tr!("`{}` は、{listed_ts}{gap}の生成コードが自分で使う名前と同じになります", "`{}` takes a name the code generated for {listed_ts} uses itself", d.name.text),
        )
        .at(at(d.name.span.line))
        .mark(d.name.span.clone(), label(&d))
        .targets(&ids_of(&ts, wire))
        .beside(&ids_of(&beside, false));
        let mut notes = notes;
        notes.sort_by_key(|(_, ts)| ts.iter().min().copied());
        // An enum read from an `enums` file takes its type there: the file's line is what changes it.
        let read = notes.iter().find_map(|(m, _)| match m {
            Meets::ReadEnum { file, written, .. } => Some((file.clone(), written.clone())),
            _ => None,
        });
        for (m, mut nts) in notes {
            nts.sort();
            diag = diag.note(format!("{}: {}", listed(&nts), m.said(nts[0], &d.what, &d.name.text)));
        }
        if let Some((file, written)) = read {
            out.push(diag.note(tr!(
                "使わない生成先なら、このままで構いません。使うなら、`{file}` のその列挙に別名を付けてください（列挙の型 `{written}` は、そのファイルの列挙の名前から作ります。`->` で付ける名前は、この規則の中だけの名前です）。",
                "For a target you do not generate, leave it. For one you do, give that enum an alias in `{file}`: its type, `{written}`, is made from its name there, and a name `->` gives it is the rule's own alone."
            )));
            continue;
        }
        let fix = match d.name.ascii {
            Some(_) => tr!("使わない生成先なら、このままで構いません。使うなら、別名を変えてください。", "For a target you do not generate, leave it. For one you do, change the alias."),
            None => tr!(
                "使わない生成先なら、このままで構いません。使うなら、別名を付けてください（`{}(<別名>)`）。",
                "For a target you do not generate, leave it. For one you do, give it an alias (`{}(<alias>)`).",
                d.name.text
            ),
        };
        out.push(diag.note(fix));
    }
    out
}

/// The shapes the function that reads the caller's objects takes (`Gen::proj_params`): those an
/// input or a sequence is read from.
fn used_shapes(f: &RuleFile) -> Vec<&Name> {
    let roots = |p: &Option<crate::ast::Projection>, s: &crate::ast::ShapeDecl| p.as_ref().is_some_and(|p| p.root.text == s.name.text);
    f.shapes.iter().filter(|s| f.inputs.iter().any(|i| roots(&i.from, s)) || f.elements.iter().any(|e| roots(&e.from, s))).map(|s| &s.name).collect()
}

/// The shapes the function that reads the caller's objects takes, for `names::check`: a shape and
/// an input or a sequence the caller passes as it is are parameters of one function.
pub(crate) fn shapes_read(f: &RuleFile) -> Vec<&Name> {
    used_shapes(f)
}
