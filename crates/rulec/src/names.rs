//! One name, one declaration (§15.262).
//!
//! A name a file declares stands for one thing wherever the file reads it, and the identifier the
//! generated code writes for it — its alias, or the name itself where it has none — stands for one
//! thing there. A file that declares one name twice leaves every reader to whichever of the two it
//! meets: the check read the later one, `rulec schema` wrote both, and the generated code did not
//! compile or read the other one. So the file's own lines are held to it here, before anything
//! reads a name, and the check stops at the second of two declarations (E034), with the first one
//! marked:
//!
//! - **the values**: the inputs, the sequences and the fields of their elements, the outputs, and
//!   what the items decide — `derive`, `define`, `count`, `sum`, `max`, `min`, `open`, and the columns
//!   of the tables and the clauses. An output is decided by what has its name, and several tables
//!   and clauses may decide one name (§7); anything else declared twice is two values of one name.
//!   A fold's answer and the name an `apply` gives a callee output are held to the same under their
//!   own codes, with what else they are checked for (E021, E041).
//! - **the other names**: the tables and the clauses, the enums, the values of each enum, the
//!   groups and the values each lists, the shapes, the sources, the applies and the prerequisites
//!   blocks.
//! - **the identifiers**: two names given one identifier (the alias of one is the alias or the name
//!   of the other), and one name given two aliases, among the values, the tables and clauses, the
//!   enums, the values of each enum, the groups, the shapes and the prerequisites blocks — the names
//!   the generated code writes as their identifiers are. In an `enums` file an enum's name is not
//!   another's alias either: `import enum` finds an enum by its name or its alias.
//!
//! What another file brings in is held elsewhere: an enum `import enum` reads and the groups that
//! come with it (`enums::read_imports`), an input a contribution adds (`link::merge`). A heading that
//! names one column twice is said where the heading is read (`parse`).

use crate::ast::{AggKind, FileKind, Item, Name, RuleFile};
use crate::diag::Diag;
use std::collections::HashMap;

/// What a value of the rule is declared as.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Input,
    Sequence,
    Field,
    Output,
    Derive,
    Define,
    Summary(AggKind),
    Open,
    /// A column a table decides, or the output of a clause.
    Column {
        clause: bool,
    },
    /// A fold's answer: E021 holds it.
    Answer,
    /// The name an `apply` gives a callee's output: E041 holds it.
    Brought,
}

impl Kind {
    fn word(self) -> String {
        match self {
            Kind::Input => tr!("入力", "an input"),
            Kind::Sequence => tr!("たどる並び", "a sequence"),
            Kind::Field => tr!("要素のフィールド", "a field of an element"),
            Kind::Output => tr!("出力", "an output"),
            Kind::Derive => tr!("導出", "a derive"),
            Kind::Define => tr!("定義", "a definition"),
            Kind::Summary(a) => tr!("`{}` の値", "a `{}`", a.word()),
            Kind::Open => tr!("開いた値", "an open value"),
            Kind::Column { clause: false } => tr!("表の出力", "a table's output"),
            Kind::Column { clause: true } => tr!("節の出力", "a clause's output"),
            Kind::Answer => tr!("畳み込みの答え", "a fold's answer"),
            Kind::Brought => tr!("準用した出力の名前", "the name of an applied output"),
        }
    }

    /// Declared before anything decides it: what the caller passes, and what a walk reads.
    fn passed(self) -> bool {
        matches!(self, Kind::Input | Kind::Sequence | Kind::Field)
    }

    /// Held to one name under a code of its own (E021, E041).
    fn elsewhere(self) -> bool {
        matches!(self, Kind::Answer | Kind::Brought)
    }
}

/// One declaration of a value.
struct Decl<'a> {
    name: &'a Name,
    kind: Kind,
}

/// Every name the file's own lines declare twice, and every identifier given to two names (E034).
pub fn check(f: &RuleFile, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let at = |line: usize| format!("{path}:{line}");
    values(f, &at, &mut out);
    let id_note = |what: String| {
        tr!(
            "{what}生成するコードは、別名があれば別名を、無ければ名前そのものを識別子にします。どちらかに別の別名を付けてください。",
            "{what} The generated code takes the alias as the identifier, or the name itself where there is none. Give one of the two another alias."
        )
    };

    // The tables and the clauses: the trace and `overrides` name them, and their identifiers name
    // the SQL query's column of their row numbers and the Rust proof's rows.
    let tables: Vec<&Name> = f.items.iter().filter_map(|it| if let Item::Table(t) = it { t.name.as_ref() } else { None }).filter(|n| !n.text.contains(':')).collect();
    for (first, n) in twice(&tables) {
        out.push(
            Diag::error("E034", tr!("表か節の名前 `{}` が二度あります", "The table or clause name `{}` appears twice", n.text))
                .at(at(n.span.line))
                .mark(first.span.clone(), tr!("最初の宣言", "the first declaration"))
                .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                .note(tr!(
                    "表と節は同じ名前空間に入ります。記録の trace と `{}` の行は、その名前で表や節を指します。どちらかの名前を変えてください。",
                    "Tables and clauses share one namespace: the trace and `{}` lines refer to them by name. Change one of the two.",
                    crate::kw::OVERRIDES
                )),
        );
    }
    identifiers(
        &tables,
        &at,
        &mut out,
        id_note(tr!(
            "表と節の識別子は、SQL の問い合わせで行の番号を入れる列（`<識別子>_row`）と、Rust の証明の名前になります。",
            "The identifier of a table or a clause names the SQL query's column of its row numbers (`<identifier>_row`) and the Rust proof's rows."
        )),
    );

    // The enums the file declares: an enum `import enum` reads is held to them when it is read.
    let enums: Vec<&Name> = f.enums.iter().map(|e| &e.name).collect();
    for (first, n) in twice(&enums) {
        out.push(
            Diag::error("E034", tr!("列挙の名前 `{}` が二度あります", "The enum name `{}` appears twice", n.text))
                .at(at(n.span.line))
                .mark(first.span.clone(), tr!("最初の宣言", "the first declaration"))
                .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                .note(tr!(
                    "列挙の名前は、入力や列の型として書く名前です。同じ名前の列挙が二つあると、その型がどちらの値をとるかが決まりません。どちらかを消すか、名前を変えてください。",
                    "An enum's name is what an input or a column writes as its type. With two enums of one name, which values the type takes is not decided. Remove one of them, or rename it."
                )),
        );
    }
    identifiers(
        &enums,
        &at,
        &mut out,
        id_note(tr!("列挙の識別子は、生成するコードの型の名前になります。", "An enum's identifier names its type in the generated code.")),
    );
    // `import enum <enum> from "<file>"` finds the enum by its name or its alias, the first that has
    // either: a name that is another enum's alias leaves which one a rule reads to their order.
    if f.kind == FileKind::Enums {
        for e in &f.enums {
            // An enum with no alias has its name as its identifier, and was said with the identifiers.
            if e.name.ascii.is_none() {
                continue;
            }
            let Some(other) = f.enums.iter().find(|o| o.name.ascii.as_deref() == Some(e.name.text.as_str()) && o.name.text != e.name.text) else { continue };
            // Marked where each is declared: the enum whose name it is, and the one whose alias.
            let named = (&e.name, tr!("名前 {}", "the name {}", e.name.text));
            let aliased = (&other.name, tr!("{0} の別名 {1}", "the alias {1} of {0}", other.name.text, e.name.text));
            let ((first, l1), (second, l2)) = if (other.name.span.line, other.name.span.col) < (e.name.span.line, e.name.span.col) { (aliased, named) } else { (named, aliased) };
            out.push(
                Diag::error("E034", tr!("列挙 `{}` の名前が、列挙 `{}` の別名と同じです", "The name of enum `{}` is the alias of enum `{}`", e.name.text, other.name.text))
                    .at(at(second.span.line))
                    .mark(first.span.clone(), l1)
                    .mark(second.span.clone(), l2)
                    .note(tr!(
                        "`import {} … from` は名前か別名で列挙を探すので、この名前を書いた規則がどちらの列挙を読むかが決まりません。どちらかを変えてください。",
                        "`import {} … from` finds an enum by its name or by its alias, so which of the two a rule that writes this name reads is not decided. Change one of the two.",
                        crate::kw::ENUM
                    )),
            );
        }
    }

    // The values of each enum.
    for e in &f.enums {
        let vs: Vec<&Name> = e.values.iter().collect();
        for (first, v) in twice(&vs) {
            out.push(
                Diag::error("E034", tr!("列挙 `{}` の値 `{}` が二度書かれています", "Enum `{}` has the value `{}` twice", e.name.text, v.text))
                    .at(at(v.span.line))
                    .mark(first.span.clone(), tr!("一つ目", "the first"))
                    .mark(v.span.clone(), tr!("二つ目", "the second"))
                    .note(tr!(
                        "一つの列挙の値は、一つずつ書いてください。同じ値が二つあると、生成するコードの列挙が同じ値を二度宣言し、ほとんどの言語で組めません。",
                        "Write each value of an enum once. With one written twice, the generated code's enum declares it twice, which most languages do not compile."
                    )),
            );
        }
        identifiers(
            &vs,
            &at,
            &mut out,
            id_note(tr!("列挙の値の識別子は、生成するコードの列挙の値の名前になります。", "A value's identifier names it in the generated code's enum.")),
        );
    }

    // The groups, and the values each lists.
    let groups: Vec<&Name> = f.groups.iter().map(|g| &g.name).collect();
    for (first, n) in twice(&groups) {
        out.push(
            Diag::error("E034", tr!("グループの名前 `{}` が二度あります", "The group name `{}` appears twice", n.text))
                .at(at(n.span.line))
                .mark(first.span.clone(), tr!("最初の宣言", "the first declaration"))
                .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                .note(tr!(
                    "グループの名前は、セルに書いて値の集まりを指す名前です。同じ名前のグループが二つあると、セルがどちらの値を指すかが決まりません。どちらかを消すか、名前を変えてください。",
                    "A group's name is what a cell writes for its values. With two groups of one name, which values a cell means is not decided. Remove one of them, or rename it."
                )),
        );
    }
    identifiers(
        &groups,
        &at,
        &mut out,
        id_note(tr!("グループの識別子は、生成するコードでグループを判定する名前になります。", "A group's identifier names the test of the group in the generated code.")),
    );
    for g in &f.groups {
        let ms: Vec<&Name> = g.members.iter().collect();
        for (first, m) in twice(&ms) {
            out.push(
                Diag::error("E034", tr!("グループ `{}` に値 `{}` が二度書かれています", "Group `{}` lists the value `{}` twice", g.name.text, m.text))
                    .at(at(m.span.line))
                    .mark(first.span.clone(), tr!("一つ目", "the first"))
                    .mark(m.span.clone(), tr!("二つ目", "the second"))
                    .note(tr!(
                        "グループの値は、一つずつ書いてください。同じ値が二つあると、Go の生成コードが同じ値を二度比べることになり、組めません。",
                        "List each value of a group once. With one listed twice, the generated Go compares it twice, and does not compile."
                    )),
            );
        }
    }

    // The shapes: an input's `from` names the caller's object by the shape's name, and the shape's
    // identifier names a parameter of the function that reads it.
    let shapes: Vec<&Name> = f.shapes.iter().map(|s| &s.name).collect();
    for (first, n) in twice(&shapes) {
        out.push(
            Diag::error("E034", tr!("`shape` の名前 `{}` が二度あります", "The shape name `{}` appears twice", n.text))
                .at(at(n.span.line))
                .mark(first.span.clone(), tr!("最初の宣言", "the first declaration"))
                .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                .note(tr!(
                    "入力の `from` は、`shape` の名前で呼ぶ側のオブジェクトを指します。同じ名前の `shape` が二つあると、どちらの契約から読むかが決まりません。どちらかを消すか、名前を変えてください。",
                    "An input's `from` names the caller's object by its shape's name. With two shapes of one name, which contract it is read from is not decided. Remove one of them, or rename it."
                )),
        );
    }
    identifiers(
        &shapes,
        &at,
        &mut out,
        id_note(tr!(
            "`shape` の識別子は、呼ぶ側のオブジェクトを受け取る関数（`<規則>_from`）の引数の名前になります。",
            "A shape's identifier names a parameter of the function that takes the caller's objects (`<rule>_from`)."
        )),
    );

    // The sources: a citation `@<name>` names one.
    let sources: Vec<&Name> = f.sources.iter().map(|s| &s.name).collect();
    for (first, n) in twice(&sources) {
        out.push(
            Diag::error("E034", tr!("出典の名前 `{}` が二度あります", "The source name `{}` appears twice", n.text))
                .at(at(n.span.line))
                .mark(first.span.clone(), tr!("最初の宣言", "the first declaration"))
                .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                .note(tr!("引用 `@名前` は、この名前で出典を指します。どちらかの名前を変えてください。", "A citation `@name` refers to the source by this name. Change one of the two.")),
        );
    }

    // The applies: what one brings in is written `<apply>:<name>`. The identifiers of what it brings
    // in are rulec's to make, and are kept apart from every other (§15.260), so the aliases of two
    // applies are not held here.
    let applies: Vec<&Name> = f.applies.iter().map(|a| &a.name).collect();
    for (first, n) in twice(&applies) {
        out.push(
            Diag::error("E034", tr!("準用の名前 `{}` が二度あります", "The apply name `{}` appears twice", n.text))
                .at(at(n.span.line))
                .mark(first.span.clone(), tr!("最初の宣言", "the first declaration"))
                .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                .note(tr!(
                    "準用が持ち込む表や値は、`<準用の名前>:<名前>` と書きます。二つの準用が同じ名前だと、どちらの準用の表や値かが決まりません。どちらかの名前を変えてください。",
                    "What an apply brings in is written `<apply>:<name>`. With two applies of one name, which apply's tables and values those are is not decided. Rename one of the two."
                )),
        );
    }

    // The prerequisites blocks: the name names the functions generated from the block.
    let blocks: Vec<&Name> = f.prerequisites.iter().map(|p| &p.name).collect();
    for (first, n) in twice(&blocks) {
        out.push(
            Diag::error("E034", tr!("前提のつながりの名前 `{}` が二度あります", "The prerequisites name `{}` appears twice", n.text))
                .at(tr!("{path}:{} 前提のつながり {}", "{path}:{} prerequisites {}", n.span.line, n.text))
                .mark(first.span.clone(), tr!("最初の宣言", "the first declaration"))
                .mark(n.span.clone(), tr!("同じ名前", "the same name"))
                .note(tr!("名前は生成する関数の名前になるので、ファイルの中で一つずつ別にしてください。", "The name names the functions generated from it, so give each block of the file its own.")),
        );
    }
    identifiers(
        &blocks,
        &at,
        &mut out,
        id_note(tr!(
            "前提のつながりの識別子は、生成する関数（`<識別子>_requires`、`<識別子>_available`）の名前になります。",
            "A prerequisites block's identifier names the functions generated from it (`<identifier>_requires`, `<identifier>_available`)."
        )),
    );
    out
}

/// The second of every two names of one text in `names`, each with the first.
fn twice<'a>(names: &[&'a Name]) -> Vec<(&'a Name, &'a Name)> {
    let mut seen: HashMap<&str, &Name> = HashMap::new();
    let mut out = Vec::new();
    for n in names {
        match seen.get(n.text.as_str()) {
            Some(first) => out.push((*first, *n)),
            None => {
                seen.insert(&n.text, n);
            }
        }
    }
    out
}

/// The identifier the generated code writes for a name, when the author chose it: the alias, or the
/// name itself. `None` for a name rulec makes the identifier of: one that is a keyword of a language
/// that writes a name as it is (`class_`, §15.210).
fn author_identifier(n: &Name) -> Option<String> {
    match &n.ascii {
        Some(a) => Some(a.clone()),
        None if n.text.is_ascii() && !crate::codegen::keyword_in(&n.text).is_empty() => None,
        None => Some(n.text.clone()),
    }
}

/// Two names of one list with one identifier (E034), each said at the later of the two.
fn identifiers(names: &[&Name], at: &impl Fn(usize) -> String, out: &mut Vec<Diag>, note: String) {
    let mut seen: HashMap<String, &Name> = HashMap::new();
    for n in names {
        let Some(id) = author_identifier(n) else { continue };
        match seen.get(&id) {
            // One name twice is said by the names; the identifier comes with it.
            Some(first) if first.text == n.text => {}
            Some(first) => out.push(same_identifier(first, n, &id, at, note.clone())),
            None => {
                seen.insert(id, n);
            }
        }
    }
}

/// The finding of two names given one identifier: `first`, declared first, and `second`.
fn same_identifier(first: &Name, second: &Name, id: &str, at: &impl Fn(usize) -> String, note: String) -> Diag {
    let label = |n: &Name| match &n.ascii {
        Some(a) => tr!("{} の別名 {a}", "the alias {a} of {}", n.text),
        None => tr!("別名の無い名前 {}", "{}, a name with no alias", n.text),
    };
    Diag::error("E034", tr!("`{}` と `{}` が、生成するコードで同じ識別子 `{id}` になります", "`{}` and `{}` become the one identifier `{id}` in the generated code", first.text, second.text))
        .at(at(second.span.line))
        .mark(first.span.clone(), label(first))
        .mark(second.span.clone(), label(second))
        .note(note)
}

/// The values: their names, and the identifiers the generated code writes for them.
fn values(f: &RuleFile, at: &impl Fn(usize) -> String, out: &mut Vec<Diag>) {
    let mut ds: Vec<Decl> = Vec::new();
    for i in &f.inputs {
        ds.push(Decl { name: &i.name, kind: Kind::Input });
    }
    for el in &f.elements {
        ds.push(Decl { name: &el.name, kind: Kind::Sequence });
        for fd in &el.fields {
            ds.push(Decl { name: &fd.name, kind: Kind::Field });
        }
    }
    for o in &f.outputs {
        ds.push(Decl { name: &o.name, kind: Kind::Output });
    }
    for it in &f.items {
        match it {
            Item::Derived(d) => ds.push(Decl { name: &d.name, kind: Kind::Derive }),
            Item::Define(d) => ds.push(Decl { name: &d.name, kind: Kind::Define }),
            Item::Agg(d) => ds.push(Decl { name: &d.name, kind: Kind::Summary(d.kind) }),
            Item::Fold(d) => ds.extend(d.answer.iter().map(|a| Decl { name: &a.name, kind: Kind::Answer })),
            Item::Table(t) => ds.extend(t.outputs.iter().map(|oc| Decl { name: &oc.name, kind: Kind::Column { clause: t.clause } })),
        }
    }
    for o in &f.opens {
        ds.push(Decl { name: &o.name, kind: Kind::Open });
    }
    for a in &f.applies {
        ds.extend(a.outputs.iter().map(|ob| Decl { name: &ob.name, kind: Kind::Brought }));
    }
    // A name with `:` is one another file brought in, which is that file's to declare.
    ds.retain(|d| !d.name.text.contains(':'));
    ds.sort_by_key(|d| (d.name.span.line, d.name.span.col));

    // The names, in the order each was first declared.
    let mut order: Vec<&str> = Vec::new();
    let mut of: HashMap<&str, Vec<&Decl>> = HashMap::new();
    for d in &ds {
        let n = d.name.text.as_str();
        if !of.contains_key(n) {
            order.push(n);
        }
        of.entry(n).or_default().push(d);
    }

    // One value per name. An output is decided by what has its name, so it shares the name with
    // that, and with nothing the caller passes; the tables and the clauses that decide one name are
    // one definition of it (§7). A heading that names one column twice was said where it was read.
    let mut clashed: Vec<&str> = Vec::new();
    for n in &order {
        let mut def: Option<&Decl> = None;
        let mut output: Option<&Decl> = None;
        // The tables and clauses of one name are one definition: said once against what else has it.
        let mut set_said = false;
        for d in &of[n] {
            let first = match d.kind {
                Kind::Output => output.or(def.filter(|x| x.kind.passed())),
                Kind::Column { .. } => def.filter(|x| !matches!(x.kind, Kind::Column { .. })),
                k if k.passed() => def.or(output),
                _ => def,
            };
            match first {
                Some(first) => {
                    if !clashed.contains(n) {
                        clashed.push(n);
                    }
                    let again = matches!(d.kind, Kind::Column { .. }) && std::mem::replace(&mut set_said, true);
                    if !first.kind.elsewhere() && !d.kind.elsewhere() && !again {
                        out.push(declared_twice(first, d, at));
                    }
                }
                None if d.kind == Kind::Output => output = Some(d),
                None => {
                    def.get_or_insert(d);
                }
            }
        }
    }

    // One identifier per name, and one name per identifier.
    let mut ids: Vec<(String, &Decl)> = Vec::new();
    for n in &order {
        if clashed.contains(n) {
            continue;
        }
        let ds = &of[n];
        let mut alias: Option<&Decl> = None;
        let mut two = false;
        for d in ds {
            let Some(a) = &d.name.ascii else { continue };
            match alias {
                None => alias = Some(d),
                Some(first) if first.name.ascii.as_deref() != Some(a.as_str()) => {
                    out.push(
                        Diag::error(
                            "E034",
                            tr!(
                                "`{}` に、二つの別名 `{}` と `{a}` が書かれています",
                                "`{}` is given two aliases, `{}` and `{a}`",
                                d.name.text,
                                first.name.ascii.as_deref().unwrap_or("")
                            ),
                        )
                        .at(at(d.name.span.line))
                        .mark(first.name.span.clone(), tr!("{}（{}）", "{} ({})", first.name.ascii.as_deref().unwrap_or(""), first.kind.word()))
                        .mark(d.name.span.clone(), tr!("{a}（{}）", "{a} ({})", d.kind.word()))
                        .note(tr!(
                            "別名は、生成するコードがその値に使う識別子で、一つの値に一つです。どちらかにそろえてください。出力と、それを決める定義や表の列なら、出力の行にだけ書けば足ります。",
                            "An alias is the identifier the generated code gives the value, and a value has one. Make the two the same; for an output and what decides it, writing it on the output's line is enough."
                        )),
                    );
                    two = true;
                    break;
                }
                Some(_) => {}
            }
        }
        if two {
            continue;
        }
        let at_decl = alias.unwrap_or(ds[0]);
        let id = match alias {
            Some(d) => d.name.ascii.clone().unwrap_or_default(),
            None => match author_identifier(ds[0].name) {
                Some(id) => id,
                None => continue,
            },
        };
        ids.push((id, at_decl));
    }
    ids.sort_by_key(|(_, d)| (d.name.span.line, d.name.span.col));
    let mut seen: HashMap<&str, &Decl> = HashMap::new();
    for (id, d) in &ids {
        match seen.get(id.as_str()) {
            Some(first) => out.push(same_identifier(
                first.name,
                d.name,
                id,
                at,
                tr!(
                    "値の識別子は、生成するコードの引数や変数の名前になります。二つの値が一つの識別子になると、同じ名前の引数や変数が二度宣言されて組めないか、一方がもう一方を上書きします。生成するコードは、別名があれば別名を、無ければ名前そのものを識別子にします。どちらかに別の別名を付けてください。",
                    "A value's identifier names a parameter or a variable of the generated code. With two values of one identifier, the code declares one name twice and does not compile, or one value overwrites the other. The generated code takes the alias as the identifier, or the name itself where there is none. Give one of the two another alias."
                ),
            )),
            None => {
                seen.insert(id, d);
            }
        }
    }
}

/// The finding of a value declared twice: `first`, then `second`.
fn declared_twice(first: &Decl, second: &Decl, at: &impl Fn(usize) -> String) -> Diag {
    let n = &second.name.text;
    let note = match (first.kind, second.kind) {
        (Kind::Output, Kind::Output) => tr!(
            "出力は、呼ぶ側に返す値の名前です。同じ名前の出力が二つあると、`rulec schema` と生成するコードに二つ並び、どちらの型と丸めで返すかが決まりません。どちらかを消すか、名前を変えてください。",
            "An output names a value returned to the caller. With two outputs of one name, `rulec schema` and the generated code write both, and which type and rounding the value is returned in is not decided. Remove one of them, or rename it."
        ),
        (k, Kind::Output) | (Kind::Output, k) => tr!(
            "出力は、同じ名前の定義（define、表や節の出力など）が決める値を返します。{}をそのまま返すなら、出力に別の名前を付けて `define <出力の名前> : <型> = {n}` と書いてください。",
            "An output returns the value the definition of its name decides (a `define`, a table's or a clause's output, …). To return {} as it is, give the output a name of its own and write `define <output> : <type> = {n}`.",
            k.word()
        ),
        _ => tr!(
            "一つの名前は、規則の中で一つの値を指します。宣言が二つあると、検査と生成するコードがどちらを読むかが決まりません（生成するコードは組めないこともあります）。どちらかを消すか、名前を変えてください。",
            "A name names one value in a rule. With two declarations, which of the two the check and the generated code read is not decided (the generated code may not even compile). Remove one of them, or rename it."
        ),
    };
    Diag::error("E034", tr!("名前 `{n}` が二度宣言されています", "The name `{n}` is declared twice"))
        .at(at(second.name.span.line))
        .mark(first.name.span.clone(), tr!("最初の宣言（{}）", "the first declaration ({})", first.kind.word()))
        .mark(second.name.span.clone(), tr!("同じ名前（{}）", "the same name ({})", second.kind.word()))
        .note(note)
}
