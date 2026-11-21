//! The `ringi` command as a function (ritsu's DESIGN 3.3): the binary of ringi's own crate runs it
//! with no other language joined, and `ritsu ringi` with every one. What exists and what it takes
//! is `cli.rs`'s table.

use crate::suite::Suite;
use std::io::Write;

thread_local! {
    /// The command line `run` was given, the words after the program's name, to say again with
    /// `ritsu ringi` in front when a language is not joined (E008).
    static COMMAND: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// The `ringi` command: `args` without the program's name, the languages `suite` joins (none for
/// the binary of ringi's own crate, every one for `ritsu ringi`), and where to print. The exit
/// code.
pub fn run(args: &[String], suite: Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    COMMAND.with(|c| *c.borrow_mut() = Some(args.to_vec()));
    let code = crate::cli::run(args, &suite, out, err);
    COMMAND.with(|c| *c.borrow_mut() = None);
    code
}

/// A word as a shell reads it back: as it is, or in single quotes.
pub(crate) fn shell_word(w: &str) -> String {
    if !w.is_empty() && w.chars().all(|c| !c.is_whitespace() && !"'\"\\$`;&|<>()*?[]#~".contains(c)) { w.to_string() } else { format!("'{}'", w.replace('\'', "'\\''")) }
}

/// The command to run instead, where a language is not joined: the one given, with `ritsu ringi`
/// in front (`ritsu ringi check <file.ringi>` when no command line is known, as for the library).
pub fn with_ritsu() -> String {
    match COMMAND.with(|c| c.borrow().clone()) {
        Some(words) => {
            let shown: Vec<String> = words.iter().map(|w| shell_word(w)).collect();
            format!("ritsu ringi {}", shown.join(" "))
        }
        None => "ritsu ringi check <file.ringi>".to_string(),
    }
}
