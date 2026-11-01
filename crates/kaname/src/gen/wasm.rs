//! The package's Wasm (DESIGN 36.10, 36.11): the crate of a package of Rust, built for
//! wasm32-unknown-unknown with `ritsu_emit::wasm::CARGO_LINE`, is one module that imports nothing and
//! exports `call: func(input: string) -> string`. A call is a function a host calls of one of the
//! package's modules, written as a line of JSON, and the answer is the line `kaname vectors` writes
//! for that input without its `why`: the module answers through the door (`glue::door::answer`), as
//! the native harness of the tests does.
//!
//! - [`abi`]: the file [`FILE`], with the three functions of the canonical ABI, which the crate's root
//!   declares for wasm32 alone ([`DECLARE`]): built for another target, or read by a crate that makes
//!   a Wasm of its own, the crate exports none of them.
//! - [`wit`]: the world the package's `.wit` declares, `package kaname:<world>@<version>`, with the
//!   modules and the functions a call reaches in its doc comments.
//!
//! A transfer a function builds is answered as a value; the functions that make the transfers in the
//! books, and the books' clients, are left out of the crate built for wasm32 (`glue/books.rs`, and
//! the crate's root for the clients), since they wait on a database's driver.

use crate::check::Checked;
use ritsu_base::text::Lang;
use ritsu_emit::package::Place;
use std::rc::Rc;

/// The file of the three functions of the ABI, at the crate's root.
pub const FILE: &str = "wasm.rs";

/// What declares [`FILE`] in the crate's root: for wasm32 alone.
pub const DECLARE: &str = "#[cfg(target_arch = \"wasm32\")]\nmod wasm;\n";

/// What declares the books' clients in the crate's root: for every target but wasm32.
pub const BOOKS: &str = "#[cfg(not(target_arch = \"wasm32\"))]\npub mod books;\n";

/// The file of the `.wit` of the package `name`.
pub fn wit_file(name: &str) -> String {
    format!("{name}.wit")
}

/// The world of the package `name` (`ritsu gen`'s `--name`): `ritsu_emit::wasm::world`.
pub fn world(name: &str) -> String {
    ritsu_emit::wasm::world(name)
}

/// The package the `.wit` declares: `kaname:<world>`, with `@<version>` when the package has one.
pub fn package(name: &str, version: Option<&str>) -> String {
    ritsu_emit::wasm::package("kaname", name, version)
}

/// [`FILE`]: the three functions of the canonical ABI, which hand the input to the door, with their
/// comments in `lang`.
pub fn abi(lang: Lang) -> String {
    let head = ritsu_emit::header::Comment::Slashes.line(&ritsu_emit::header::generated("kaname"));
    let what = match lang {
        Lang::Ja => {
            "パッケージの Wasm のモジュール：call は、パッケージの kaname のモジュールの、ホストが呼ぶ関数の呼び出しを JSON の一行で受け取り、kaname vectors がその入力に書く行（`why` を除く）で答える。クレートの根は、wasm32 に組むときだけこのファイルを読む。"
        }
        Lang::En => {
            "The package as one Wasm module: call takes a call of a function a host calls of the package's modules of kaname, written as a line of JSON, and answers the line kaname vectors writes for that input, without its `why`. The crate's root reads this file when it is built for wasm32 alone."
        }
    };
    format!(
        "{head}//! {what}\n\n{}\n{}",
        ritsu_emit::wasm::ALLOC_USE,
        ritsu_emit::wasm::abi(lang, "crate::glue::door::answer(text)", "crate::glue::door::not_utf8()")
    )
}

/// The `.wit` of the package `name` of `modules` (each with its place in the package): the world
/// that exports `call`, with what to send and what comes back, and each module a call reaches with
/// the functions it calls there, in its doc comments in `lang`.
pub fn wit(modules: &[(Rc<Checked>, String, Place)], name: &str, version: Option<&str>, lang: Lang) -> String {
    let ja = lang == Lang::Ja;
    let mut o = ritsu_emit::header::Comment::Slashes.line(&ritsu_emit::header::generated("kaname"));
    o.push('\n');
    o.push_str(&format!("package {};\n\n", package(name, version)));
    let head = if ja { format!("パッケージ {name} の kaname のモジュールにある、ホストが呼ぶ関数") } else { format!("The functions a host calls in the kaname modules of the package {name}") };
    o.push_str(&format!("/// {head}\nworld {} {{\n", world(name)));
    let callable: Vec<&(Rc<Checked>, String, Place)> = modules.iter().filter(|(c, _, _)| c.decls.fns.iter().any(|f| !f.inner())).collect();
    let books = callable.iter().any(|(c, _, _)| !crate::value::Program::of(c).books.is_empty());
    let mut lines: Vec<String> = Vec::new();
    if ja {
        lines.push("一回の呼び出し。input は JSON のオブジェクト {\"module\": <モジュール>, \"function\": <関数>, \"in\": {<引数>: <値>, …}} で、名前はどれもモジュールが書いたとおり、値は kaname vectors の書き方で書く。kaname vectors の行も同じに読む。".into());
        if callable.len() == 1 {
            lines.push("モジュールは一つなので、\"module\" は省ける。".into());
        }
        lines.push("返すのは、kaname vectors がその入力に書く行から why を除いたもの {\"function\":…,\"in\":…,\"out\":…} で、関数が失敗すれば out の代わりに \"fails\": {\"kind\",\"module\",\"line\",\"message\"} を持つ。入力が、ホストが呼ぶ関数の呼び出しとして読めなければ、拒否の行 {\"refused\":\"input\",\"error\":\"<文>\"} を返す。".into());
        lines.push("引数の範囲と要素の数、関数がどう失敗しうるかは、モジュールごとの kaname api が言う。".into());
        if books {
            lines.push("関数が作る振替は、値 {\"book\",\"transfer\",\"op\",\"args\"} で答える。帳簿に当てる関数は、このモジュールに入らない。".into());
        }
        lines.push("参照インタプリタが止まらないと言う呼び出し（深すぎる再帰）では、モジュールのスタックが尽きてトラップすることがある。".into());
        lines.push(String::new());
        lines.push("モジュールと関数:".into());
    } else {
        lines.push("One call. `input` is a JSON object, {\"module\": <module>, \"function\": <function>, \"in\": {<parameter>: <value>, …}}, each name as the module writes it and each value as kaname vectors writes it; a line of kaname vectors is read the same way.".into());
        if callable.len() == 1 {
            lines.push("With one module in the package, \"module\" may be left out.".into());
        }
        lines.push("Back comes the line kaname vectors writes for that input, without its why, {\"function\":…,\"in\":…,\"out\":…}, with \"fails\": {\"kind\",\"module\",\"line\",\"message\"} in the place of out where the function fails; or a refusal, {\"refused\":\"input\",\"error\":\"<sentence>\"}, where the input is no call of a function a host calls.".into());
        lines.push("The ranges and the counts of the parameters, and how each function can fail, are what kaname api says of each module.".into());
        if books {
            lines.push("A transfer a function builds comes back as a value, {\"book\",\"transfer\",\"op\",\"args\"}; the functions that make the transfers in the books are not in the module.".into());
        }
        lines.push("A call the reference interpreter says does not stop (a recursion too deep) may use up the module's stack and trap.".into());
        lines.push(String::new());
        lines.push("Modules and functions:".into());
    }
    for (c, _, place) in callable {
        lines.push(format!("  {}", super::door::module_key(c, place)));
        let p = crate::value::Program::of(c);
        for f in c.decls.fns.iter().filter(|f| !f.inner()) {
            let params: Vec<String> = f.params.iter().map(|prm| format!("{}: {}", prm.name.text, crate::api::type_text(&p, c, &prm.slot.ty))).collect();
            lines.push(format!("    {}({}) -> {}", f.name.text, params.join(", "), crate::api::type_text(&p, c, &f.ret.ty)));
        }
    }
    for l in &lines {
        if l.is_empty() {
            o.push_str("    ///\n");
        } else {
            o.push_str(&format!("    /// {l}\n"));
        }
    }
    o.push_str(&format!("    export {};\n}}\n", ritsu_emit::wasm::CALL_SIGNATURE));
    o
}
