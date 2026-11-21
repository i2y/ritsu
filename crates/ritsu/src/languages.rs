//! The commands of the languages that read others, as `ritsu <language>` runs them: with the
//! languages each reads joined in the same process. The binary runs them after a language's name
//! (`src/main.rs`), and the page in the browser runs them for a file's generator and page
//! (ritsu-wasm, DESIGN 8.7), so that the two answer alike.

use ritsu_project::Joined;
use std::io::Write;
use std::rc::Rc;

/// `ritsu dandori …`, on the words after `dandori`: a flow reads its rules, dates files and books
/// in the same process (DESIGN 7.8), and the code dandori writes checks the preconditions ritsu
/// cannot decide at a flow's calls of rules when the workflow runs (DESIGN 7.4). The exit code.
pub fn dandori(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    dandori_in(&Joined::new(), args, out, err)
}

/// [`dandori`], with the languages `j` joins: two commands run with one `j` (the page's one question,
/// which draws a flow's page as HTML and as Markdown) check each file once between them (DESIGN 4.15).
pub fn dandori_in(j: &Joined, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let undecided = Rc::new(ritsu_cross::UndecidedCalls::new(j));
    // the flow is checked once: by the engine the questions of `undecided` ask too (DESIGN 4.15)
    j.dandori.lending(|| dandori::cli::run_with_undecided(args, j.rules(), j.koyomi.clone(), j.chobo.clone(), undecided, out, err))
}

/// `ritsu kaname …`, on the words after `kaname`: a module calls its rules, dates files and books in
/// the same process, and the effects of its functions are decided with what ritsu's checks across the
/// borders come to at those calls (kaname's DESIGN 4.3, 11.2). The exit code.
pub fn kaname(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let j = Joined::new();
    let verdicts = Rc::new(ritsu_cross::ProgramVerdicts::new(&j));
    let suite: kaname::suite::Suite = j.ports().into();
    kaname::run::run(args, suite.with_verdicts(verdicts), out, err)
}

/// `ritsu ringi …`, on the words after `ringi`: a request reads its calendars, books, flows and
/// gate, and is lowered to rules, dates files and a module of kaname's, all in the same process
/// (ringi's DESIGN 16.3). The exit code.
pub fn ringi(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    // `init` writes a project with what `ritsu gen` makes of it, which ringi's crate does not hold
    if args.first().is_some_and(|a| a == "init") {
        return crate::ringi_init::command(&args[1..], out, err);
    }
    // rulec reads in the language ringi's command says in, whatever the process's is: the claims
    // ringi asks of the files a request is lowered to read the `enums` files those files import from
    // what their check kept in that language, and each is checked once (DESIGN 4.15)
    let rl = if ringi::cli::lang_of(args) == ritsu_base::text::Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
    rulec::i18n::with(rl, || ringi::run::run(args, Joined::new().ringi(), out, err))
}
