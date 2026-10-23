//! Two names the author wrote that the generated code of some language spells as one identifier,
//! and a name some language cannot take where its generated code puts it (§15.263).
//!
//! The generators write the identifier of a name — its alias, or the name itself where it has
//! none — each in its own way: as it is, in lowerCamelCase (Swift and Java), in PascalCase (Go's
//! fields), with an escape of their own (Java's and PHP's `_`), in upper case (an enum's values),
//! and the columns of the SQL query are read by SQLite with case set aside and by PostgreSQL as far
//! as 63 bytes. Two identifiers that differ as written can be one in such a spelling, and a
//! language can refuse an identifier for where it lands: Ruby reads one that starts with a capital
//! as a constant, and Python's `NamedTuple` takes no field that starts with `_`.
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
}

impl Meeting<'_> {
    fn targets(&self) -> Vec<Target> {
        let mut ts: Vec<Target> = self.notes.iter().flat_map(|(t, _)| t.iter().copied()).collect();
        ts.sort();
        ts.dedup();
        ts
    }
}

/// The meetings found so far, and where to add one more spelling to.
#[derive(Default)]
struct Meetings<'a>(Vec<Meeting<'a>>);

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
                self.0.push(Meeting { first: first.clone(), second: second.clone(), notes: Vec::new() });
                self.0.len() - 1
            }
        };
        let m = &mut self.0[i];
        let said = m.targets();
        let new: Vec<Target> = targets.iter().copied().filter(|t| !said.contains(t)).collect();
        if !new.is_empty() {
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

/// Every value's name with what it is, for its mark: the inputs, the sequences, the outputs, and
/// what the items decide, each where it is first declared.
fn kinds(f: &RuleFile) -> Vec<(&Name, String)> {
    let mut out: Vec<(&Name, String)> = Vec::new();
    out.extend(f.inputs.iter().map(|i| (&i.name, tr!("入力", "input"))));
    out.extend(f.elements.iter().map(|el| (&el.name, tr!("並び", "sequence"))));
    out.extend(f.outputs.iter().map(|o| (&o.name, tr!("出力", "output"))));
    for it in &f.items {
        match it {
            Item::Derived(d) => out.push((&d.name, tr!("導出", "derived value"))),
            Item::Define(d) => out.push((&d.name, tr!("定義", "definition"))),
            Item::Agg(d) => out.push((&d.name, tr!("まとめ", "summary"))),
            Item::Fold(d) => out.extend(d.answer.iter().map(|a| (&a.name, tr!("畳み込みの答え", "fold's answer")))),
            Item::Table(t) => out.extend(t.outputs.iter().map(|o| (&o.name, tr!("表の出力", "table output")))),
        }
    }
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
    ms.meet(&passed, &[Target::Connect], json_name, |_, _, s| {
        tr!(
            "入力と並びは `.proto` の `DecideRequest` のフィールドになります。フィールドの JSON の名前は、`_` を除いてその次の字を大文字にしたもので、どちらも `{s}` です。buf と protoc は、JSON の名前が同じ二つのフィールドをエラーにします。",
            "the inputs and the sequences are the fields of `DecideRequest` in the `.proto`, and a field's JSON name drops each `_` and capitalizes the letter after it, so both are `{s}`: buf and protoc refuse two fields of one JSON name."
        )
    });

    // The outputs: the fields of Go's `Output` and of the `.proto`'s answer. PHP's `Output` holds
    // them under the names of their variables, which the values above were held to.
    let returned: Vec<Decl> = vs.iter().filter(|d| f.outputs.iter().any(|o| o.name.text == d.name.text)).cloned().collect();
    ms.meet(&returned, &[Target::Go], pascal, |_, _, s| {
        tr!(
            "出力は `Output` 構造体のフィールドになり、名前を PascalCase で書くので、どちらも `{s}` です。一つの構造体に同じ名前のフィールドは置けないので、コンパイルできません。",
            "the outputs are the fields of the `Output` struct, in PascalCase, so both are `{s}`, and a struct cannot hold two fields of one name: it does not compile."
        )
    });
    ms.meet(&returned, &[Target::Connect], json_name, |_, _, s| {
        tr!(
            "出力は `.proto` の `DecideResponse` のフィールドになります。フィールドの JSON の名前は、`_` を除いてその次の字を大文字にしたもので、どちらも `{s}` です。buf と protoc は、JSON の名前が同じ二つのフィールドをエラーにします。",
            "the outputs are the fields of `DecideResponse` in the `.proto`, and a field's JSON name drops each `_` and capitalizes the letter after it, so both are `{s}`: buf and protoc refuse two fields of one JSON name."
        )
    });

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
        ms.meet(&fields, &[Target::Connect], json_name, |_, _, s| {
            tr!(
                "要素のフィールドは `.proto` の `{ty}` のフィールドになります。フィールドの JSON の名前は、`_` を除いてその次の字を大文字にしたもので、どちらも `{s}` です。buf と protoc は、JSON の名前が同じ二つのフィールドをエラーにします。",
                "the fields of an element are the fields of `{ty}` in the `.proto`, and a field's JSON name drops each `_` and capitalizes the letter after it, so both are `{s}`: buf and protoc refuse two fields of one JSON name."
            )
        });
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
    ms.meet(&wire_types, &[Target::Connect], |s| crate::proto::upper_snake(&enum_type(s)), |_, _, s| {
        tr!(
            "`.proto` は列挙の値を、型の名前を UPPER_SNAKE_CASE にしたものを頭に付けて書き、パッケージのどの列挙の値も一つの場所に並べます。二つの頭はどちらも `{s}` なので、値が設定されていないことを表す `{s}_UNSPECIFIED` が二度になり、buf と protoc がエラーにします。",
            "the `.proto` names the values of an enum after its type in UPPER_SNAKE_CASE, and puts the values of every enum of the package in one place: both are `{s}`, so the zero value `{s}_UNSPECIFIED`, which a field that is not set holds, is declared twice, which buf and protoc refuse."
        )
    });
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
            ms.meet(&vals, &[Target::Connect], |s| format!("{prefix}_{}", upper(s)), |_, _, s| {
                tr!(
                    "`.proto` の列挙の値の名前は、列挙の名前と値の名前を大文字にしてつないだもので、どちらも `{s}` です。buf と protoc は、同じ名前の値をエラーにします。",
                    "a value of an enum in the `.proto` is named by the enum's name and the value's in upper case, so both are `{s}`, and buf and protoc refuse two values of one name."
                )
            });
        }
    }

    // Go's package of the rule, where the types of the enums and their members stand side by side:
    // a member is the type's name and the value's, joined (`TierGold`), so it can meet a member of
    // another enum (`a`'s `b_c` and `a_b`'s `c` are both `ABC`) or another enum's type.
    // What each is: the type, the function that reads a value of it (`ParseTier`), or a member,
    // with the enum it is of.
    #[derive(Clone, Copy, PartialEq)]
    enum Go {
        Type,
        Parse,
        Member,
    }
    let mut go: Vec<(String, Decl, Go, usize)> = Vec::new();
    for (k, e) in own.iter().enumerate() {
        let t = enum_type(&pub_name(&e.name));
        go.push((t.clone(), plain(&e.name, tr!("列挙", "enum")), Go::Type, k));
        go.push((format!("Parse{t}"), plain(&e.name, tr!("列挙", "enum")), Go::Parse, k));
        for v in &e.values {
            go.push((format!("{t}{}", member(&pub_name(v))), plain(v, tr!("列挙の値", "enum value")), Go::Member, k));
        }
    }
    let mut first_of: Vec<(String, usize)> = Vec::new();
    for (i, (s, d, kind, k)) in go.iter().enumerate() {
        match first_of.iter().find(|(x, _)| x == s) {
            // Two types, and two values of one enum, are E034's (`names::check`).
            Some((_, j)) if !(go[*j].2 == Go::Type && *kind == Go::Type) && !(go[*j].2 == Go::Member && *kind == Go::Member && go[*j].3 == *k) => {
                let first = go[*j].1.clone();
                let what = |x: &Decl, g: Go| match g {
                    Go::Type => tr!("列挙 `{}` の型", "the type of the enum `{}`", x.name.text),
                    Go::Parse => tr!("列挙 `{}` を読む関数", "the function that reads the enum `{}`", x.name.text),
                    Go::Member => tr!("列挙の値 `{}`", "the enum value `{}`", x.name.text),
                };
                let (a, b) = (what(&first, go[*j].2), what(d, *kind));
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
        .mark(b.name.span.clone(), label(b));
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

    // A name a language cannot take where its code puts it.
    let mut alone: Vec<(Decl, Diag)> = Vec::new();
    let capital = |id: &str| id.chars().next().is_some_and(char::is_uppercase);
    let ruby: Vec<Decl> = vs.iter().cloned().chain(shapes.iter().map(|s| plain(s, tr!("`shape`", "shape")))).filter(|d| capital(&d.id)).collect();
    for d in ruby {
        let diag = Diag::warning(
            "W121",
            tr!("`{}` は大文字で始まるので、Ruby の生成コードでは使えません", "`{}` starts with a capital letter, which the code generated for Ruby cannot take", d.id),
        )
        .at(at(d.name.span.line))
        .mark(d.name.span.clone(), label(&d))
        .note(tr!(
            "Ruby: 大文字で始まる名前は定数として読まれ、引数にも変数にもなりません。生成した Ruby のファイルは読み込めません。",
            "Ruby: a name that starts with a capital letter is read as a constant, which can be neither a parameter nor a variable, so the generated Ruby file does not load."
        ))
        .note(match d.name.ascii {
            Some(_) => tr!("使わない生成先なら、このままで構いません。使うなら、別名を小文字で始まるものに変えてください。", "For a target you do not generate, leave it. For one you do, change the alias to one that starts with a small letter."),
            None => tr!(
                "使わない生成先なら、このままで構いません。使うなら、小文字で始まる別名を付けてください（`{}(<別名>)`）。",
                "For a target you do not generate, leave it. For one you do, give it an alias that starts with a small letter (`{}(<alias>)`).",
                d.name.text
            ),
        });
        alone.push((d, diag));
    }
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
    alone.sort_by_key(|(d, _)| place(&d.name.span));
    out.extend(alone.into_iter().map(|(_, diag)| diag));
    out
}

/// Every W121 of a name of the rule's that its generated code writes as one it gives itself, and
/// of a name some language cannot take in a way §15.263 does not say (§15.265): said once the check
/// has typed every item and built every definition set, which what the generated code reads is
/// worked out from.
pub fn own_warnings(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    // An `enums` file's module declares its types and nothing else, and a contribution is checked
    // as part of the rule it adds to.
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
    // A name some languages cannot take, said with the languages and why (§15.265).
    let cannot = |d: &Decl, ts: &[Target], notes: Vec<(Vec<Target>, String)>| -> Diag {
        let listed_ts = listed(ts);
        let gap = if listed_ts.ends_with('）') { "" } else { " " };
        let mut diag = Diag::warning("W121", tr!("`{}` は、{listed_ts}{gap}の生成コードでは使えません", "`{}` is a name the code generated for {listed_ts} cannot take", d.id))
            .at(at(d.name.span.line))
            .mark(d.name.span.clone(), label(d));
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
    // pattern that binds nothing. Python's plain function discards its traced twin's second answer
    // into it, and its `NamedTuple` takes no field of it, which the service of Connect then cannot
    // load either.
    for d in vs.iter().filter(|d| d.id == "_") {
        let passed = f.inputs.iter().any(|i| i.name.text == d.name.text) || f.elements.iter().any(|el| el.name.text == d.name.text);
        let returned = f.outputs.iter().any(|o| o.name.text == d.name.text);
        let mut notes: Vec<(Vec<Target>, String)> = Vec::new();
        if passed {
            notes.push((
                vec![Target::Python],
                tr!(
                    "規則の関数は、二つ目の答えを捨てる値の名前として `_` を使い、引数の `_` を書き換えるので、型の検査（mypy）が通りません。",
                    "the rule's function discards a second answer into `_`, which writes over the parameter `_`, so the type check (mypy) does not pass."
                ),
            ));
        }
        let named_tuple = returned && f.outputs.len() >= 2;
        if named_tuple {
            notes.push((
                vec![Target::Python],
                tr!(
                    "出力が二つ以上ある規則の出力は `NamedTuple` の `Output` のフィールドになり、`NamedTuple` のフィールドの名前は `_` で始められません。生成した Python のファイルは読み込めません。",
                    "the outputs of a rule with two or more are the fields of `Output`, a `NamedTuple`, and a `NamedTuple` takes no field whose name starts with `_`, so the generated Python file does not load."
                ),
            ));
        }
        notes.push((vec![Target::Rust, Target::Swift, Target::Wasm], tr!("`_` は値を捨てるパターンで、変数の名前になりません。", "`_` is the pattern that binds nothing, and names no variable.")));
        notes.push((vec![Target::Go], tr!("`_` は空白の識別子で、変数の名前にもフィールドの名前にもなりません。", "`_` is the blank identifier, which names no variable and no field.")));
        notes.push((vec![Target::Java], tr!("`_` は予約語です。", "`_` is a keyword.")));
        if named_tuple {
            notes.push((vec![Target::Connect], tr!("サービスは Python のモジュールを動かすので、読み込めません。", "the service runs the Python module, which does not load.")));
        }
        let mut ts: Vec<Target> = notes.iter().flat_map(|(t, _)| t.iter().copied()).collect();
        ts.sort();
        notes.sort_by_key(|(t, _)| t.iter().min().copied());
        alone.push((d.clone(), cannot(d, &ts, notes)));
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
            alone.push((d.clone(), cannot(&d, &ts, notes)));
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
        if empty(&d.id) {
            alone.push((d.clone(), cannot(&d, &targets, vec![(targets.clone(), why.clone())])));
        }
        for v in &e.values {
            let dv = plain(v, tr!("列挙の値", "enum value"));
            if empty(&dv.id) {
                alone.push((dv.clone(), cannot(&dv, &targets, vec![(targets.clone(), why.clone())])));
            }
        }
    }
    // SQLite gives a column no name of `true` or `false`, whatever their case: the query's
    // `AS "True"` names it `column2`, and `"True"` then reads a string (§15.264).
    if sql {
        for d in vs.iter().filter(|d| d.id.eq_ignore_ascii_case("true") || d.id.eq_ignore_ascii_case("false")) {
            let why = tr!(
                "SQLite は、問い合わせの式に付けた名前 `AS \"{0}\"` を列の名前にしません（`true` と `false` は、大文字と小文字を問わず）。`\"{0}\"` と書いて読むと文字列になり、問い合わせは違う値で答えます。PostgreSQL は列として読みます。",
                "SQLite gives a column no name of `true` or `false`, whatever their case: the query's `AS \"{0}\"` leaves the column unnamed, and `\"{0}\"` then reads a string, so the query answers otherwise. PostgreSQL reads the column.",
                d.id
            );
            alone.push((d.clone(), cannot(d, &[Target::Sqlite], vec![(vec![Target::Sqlite], why)])));
        }
    }
    // The `.proto` takes ASCII letters, digits and `_` for its names, and an enum on the wire is
    // declared there with its values, by their identifiers.
    for e in own.iter().filter(|e| wire.contains(&e.name.text)) {
        let why = tr!("`.proto` の名前は ASCII の英字と数字と `_` だけで、この名前の列挙の型や値は書けません。buf と protoc がエラーにします。", "the names of the `.proto` take ASCII letters, digits and `_` alone, and an enum's type or value of this name cannot be written there, which buf and protoc refuse.");
        let d = plain(&e.name, tr!("列挙", "enum"));
        if !d.id.is_ascii() {
            alone.push((d.clone(), cannot(&d, &[Target::Connect], vec![(vec![Target::Connect], why.clone())])));
        }
        for v in &e.values {
            let dv = plain(v, tr!("列挙の値", "enum value"));
            if !dv.id.is_ascii() {
                alone.push((dv.clone(), cannot(&dv, &[Target::Connect], vec![(vec![Target::Connect], why.clone())])));
            }
        }
    }
    // A name rulec writes with `_` after it because a language keeps it as a keyword (§15.210)
    // keeps its capital: Ruby reads `True_` as a constant, which a function cannot assign.
    // A field of one element is a member of its struct, which Ruby names by a symbol: only the
    // parameters and the variables of the rule's function are held to this.
    let fields: Vec<&str> = f.elements.iter().flat_map(|el| el.fields.iter().map(|fd| fd.name.text.as_str())).collect();
    for (n, id) in made.values.iter().filter(|(n, id)| !super::brought_in(n) && !fields.contains(&n.as_str()) && id.starts_with(|ch: char| ch.is_ascii_uppercase())) {
        let Some((name, what)) = kinds(f).into_iter().find(|(x, _)| &x.text == n) else { continue };
        let d = Decl { name, id: id.clone(), what };
        let why = tr!(
            "rulec はこの名前を、予約語と分けるために `{id}` と書きます。大文字で始まる名前は定数として読まれ、引数にも変数にもならないので、生成した Ruby のファイルは読み込めません。",
            "rulec writes this name as `{id}` to keep it apart from a keyword, and a name that starts with a capital letter is read as a constant, which can be neither a parameter nor a variable, so the generated Ruby file does not load."
        );
        alone.push((d.clone(), cannot(&d, &[Target::Ruby], vec![(vec![Target::Ruby], why)])));
    }
    alone.sort_by_key(|(d, _)| place(&d.name.span));
    let mut out: Vec<Diag> = alone.into_iter().map(|(_, diag)| diag).collect();
    // What the generated code reads is worked out from the code rulec writes for the rule (the
    // SQL query's bound columns), which a rule the check stops at has none of.
    if !crate::has_error(&c.diags) {
        out.extend(takes_own(f, c, path, &vs, &own, &wire));
    }
    out
}

/// What a note of [`Takes`] says a name of the rule's meets: the target's own name, and the kind of
/// place it stands in.
#[derive(Clone, PartialEq, Eq)]
enum Meets {
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
}

/// What a word of a language is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Word {
    Keyword,
    Builtin,
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

impl Meets {
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
            Meets::Word { word, kind: Word::Builtin } => tr!(
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
        }
    }
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
struct Takes<'a>(Vec<(Decl<'a>, Vec<(Meets, Vec<Target>)>)>);

impl<'a> Takes<'a> {
    fn add(&mut self, d: &Decl<'a>, t: Target, m: Meets) {
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
            let (func, module): (String, Option<String>) = match t {
                Target::Python | Target::Rust | Target::Wasm | Target::TypeScript | Target::JavaScript | Target::Php => (id.clone(), (t == Target::Python).then(|| id.clone())),
                Target::Ruby => (id.clone(), Some(super::ruby_module(&id))),
                Target::Go => (pascal(&id), Some(crate::backend::go_package(&id))),
                Target::Swift => (camel(&id), None),
                Target::Java => (super::java::java_class(&id), None),
                _ => continue,
            };
            // The service of Connect imports the Python module by its name, which a keyword cannot
            // be; a builtin its helpers read is said above.
            let also: &[Target] = if t == Target::Python { &[Target::Python, Target::Connect] } else { std::slice::from_ref(&t) };
            if b.reserved.iter().any(|w| is(w, &func)) {
                for &t in also {
                    takes.add(&rule, t, Meets::Word { word: func.clone(), kind: Word::Keyword });
                }
            } else if b.globals.iter().any(|w| is(w, &func)) {
                takes.add(&rule, t, Meets::Word { word: func.clone(), kind: Word::Builtin });
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
            let caseless = t == Target::Php;
            let is = |w: &str| if caseless { w.eq_ignore_ascii_case(&ty) } else { w == ty };
            // Swift keeps four capitalized words of its own, which `sw_name` puts in backticks
            // where a value is written, and which no type can be.
            let swift = t == Target::Swift && ["Any", "Self", "Type", "Protocol"].contains(&ty.as_str());
            // Ruby nests the enum's module in the rule's, where it hides a class of the language's only
            // from the code there that reads that class.
            if swift || b.reserved.iter().any(|w| is(w)) {
                takes.add(&d, t, Meets::Word { word: ty.clone(), kind: Word::Keyword });
            } else if b.globals.iter().any(|w| is(w)) || (t == Target::Ruby && super::own::rb_reads().contains(&ty.as_str())) {
                takes.add(&d, t, Meets::Word { word: ty.clone(), kind: Word::Builtin });
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

    let mut out: Vec<Diag> = Vec::new();
    let at = |line: usize| format!("{path}:{line}");
    let mut found = takes.0;
    found.sort_by_key(|(d, _)| place(&d.name.span));
    for (d, notes) in found {
        let mut ts: Vec<Target> = notes.iter().flat_map(|(_, ts)| ts.iter().copied()).collect();
        ts.sort();
        ts.dedup();
        let listed_ts = listed(&ts);
        let gap = if listed_ts.ends_with('）') { "" } else { " " };
        let mut diag = Diag::warning(
            "W121",
            tr!("`{}` は、{listed_ts}{gap}の生成コードが自分で使う名前と同じになります", "`{}` takes a name the code generated for {listed_ts} uses itself", d.name.text),
        )
        .at(at(d.name.span.line))
        .mark(d.name.span.clone(), label(&d));
        let mut notes = notes;
        notes.sort_by_key(|(_, ts)| ts.iter().min().copied());
        for (m, mut nts) in notes {
            nts.sort();
            diag = diag.note(format!("{}: {}", listed(&nts), m.said(nts[0], &d.what, &d.name.text)));
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
