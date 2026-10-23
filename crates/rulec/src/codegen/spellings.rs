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
enum Target {
    Python,
    TypeScript,
    JavaScript,
    Ruby,
    Php,
    Go,
    Swift,
    Java,
    Sqlite,
    Postgres,
    Connect,
}

impl Target {
    fn name(self) -> String {
        match self {
            Target::Python => "Python".into(),
            Target::TypeScript => "TypeScript".into(),
            Target::JavaScript => "JavaScript".into(),
            Target::Ruby => "Ruby".into(),
            Target::Php => "PHP".into(),
            Target::Go => "Go".into(),
            Target::Swift => "Swift".into(),
            Target::Java => "Java".into(),
            Target::Sqlite => tr!("SQL（SQLite）", "SQL (SQLite)"),
            Target::Postgres => tr!("SQL（PostgreSQL）", "SQL (PostgreSQL)"),
            Target::Connect => "Connect".into(),
        }
    }
}

/// The targets as the prose lists them: `A・B・C` in Japanese, `A, B and C` in English.
fn listed(ts: &[Target]) -> String {
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
    let wire = wire_enums(f, c);
    for e in &own {
        let vals: Vec<Decl> = e.values.iter().map(|v| plain(v, tr!("列挙の値", "enum value"))).collect();
        let upper = |s: &str| member(s).to_uppercase();
        ms.meet(&vals, &[Target::Python, Target::TypeScript, Target::JavaScript, Target::Java], upper, |_, _, s| {
            tr!(
                "列挙の値の名前を大文字で書くので、どちらも `{s}` です。一つの列挙に同じ名前の値が二つになり、読み込めないか、片方がもう片方を隠します。",
                "an enum's values are written in upper case, so both are `{s}`: one enum declares one name twice, which does not load, or one hides the other."
            )
        });
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
    let underscore = |d: &Decl| d.id.starts_with('_');
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
        let diag = Diag::warning("W121", tr!("`{}` は `_` で始まるので、Python の生成コードでは使えません", "`{}` starts with `_`, which the code generated for Python cannot take", d.id))
            .at(at(d.name.span.line))
            .mark(d.name.span.clone(), label(&d))
            .note(why)
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
