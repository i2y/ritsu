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
    let j = Joined::new();
    let undecided = Rc::new(ritsu_cross::UndecidedCalls::new(&j));
    dandori::cli::run_with_undecided(args, j.rules(), j.koyomi.clone(), j.chobo.clone(), undecided, out, err)
}
