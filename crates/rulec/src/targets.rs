//! What `rulec gen` writes (§15.277): the targets it is given, each the library other code reads,
//! and beside them the extras it is asked for. Nothing is written that was not asked for, so the
//! files one command line writes do not change with the version of the binary that runs it.
//!
//! The languages are `src/backend.rs`'s to state (§15.20); this file adds the one target that is
//! no language, the `.proto`, and says which file of a language is its library and which an extra.

use ritsu_base::cli::shell_word;

/// What a generated file is to its target: the library, or the extra it is written for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The module other code reads, and what it needs to build (`go.mod`, `rulec_np.py`, the
    /// Ruby signature, the Wasm crate's root and world): written for every target given.
    Library,
    /// The runner `rulec test` puts the vectors through (`--extra runner`).
    Runner,
    /// The MCP server, and the page beside it that the server serves (`--extra mcp`).
    Mcp,
    /// The Connect service, and the buf configuration that builds its stubs (`--extra connect`).
    Connect,
    /// The runner of the Connect service, written when both of the two are asked for.
    ConnectRunner,
    /// The unit vectors of the rounding helpers, as a test in the language (`--extra round-test`).
    RoundTest,
    /// The proof harnesses (`--extra proof`).
    Proof,
}

impl Part {
    /// The extras a file of this part is written for: all of them have to be asked for.
    pub fn extras(self) -> &'static [&'static str] {
        match self {
            Part::Library => &[],
            Part::Runner => &["runner"],
            Part::Mcp => &["mcp"],
            Part::Connect => &["connect"],
            Part::ConnectRunner => &["runner", "connect"],
            Part::RoundTest => &["round-test"],
            Part::Proof => &["proof"],
        }
    }
}

/// The target that is no language: the rule's `.proto` (or the contracts it implements and
/// imports from), and the buf module they land in (`buf.yaml`, and `buf.lock` when one is needed).
pub const PROTO: &str = "proto";

/// The extras, in the order every list of them is written. `vectors` is one for the run (the
/// vectors and their expected values, which every runner reads); the others are written beside the
/// targets that have them.
pub const EXTRAS: &[&str] = &["runner", "mcp", "connect", "round-test", "proof", "vectors"];

/// The flags that make `gen` write what it wrote up to 0.40.0 with no `--target`: every target of
/// that version and every extra. Fixed at 0.40.0, so that what it writes stays what 0.40.0 wrote: a
/// target added later is not in it.
pub const UP_TO_0_40: &[&str] = &[
    "--target", "python", "--target", "numpy", "--target", "typescript", "--target", "javascript", "--target", "rust",
    "--target", "ruby", "--target", "php", "--target", "go", "--target", "swift", "--target", "java", "--target", "sql",
    "--target", "wasm", "--target", "proto", "--extra", "runner", "--extra", "mcp", "--extra", "connect", "--extra",
    "round-test", "--extra", "proof", "--extra", "vectors",
];

/// `args` with [`UP_TO_0_40`] after them: the command line of a caller that reads every target and
/// every extra, as 0.40.0 wrote them.
pub fn up_to_0_40<'a>(args: &[&'a str]) -> Vec<&'a str> {
    args.iter().copied().chain(UP_TO_0_40.iter().copied()).collect()
}

/// The names `--target` takes, in the order the targets are written: the languages of
/// `backend::ALL`, then `proto`.
pub fn target_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| crate::backend::ALL.iter().map(|b| b.id).chain([PROTO]).collect())
}

/// Whether `target` has the extra: what `--extra` may ask of it. Every language has a runner and
/// a round test; the MCP server, the Connect service and the proofs are where `src/backend.rs` says
/// they are run. The `.proto` has the buf configuration of the Connect stubs.
pub fn has_extra(target: &str, extra: &str) -> bool {
    if target == PROTO {
        return extra == "connect";
    }
    let Some(b) = crate::backend::by_id(target) else { return false };
    match extra {
        "runner" | "round-test" => true,
        "mcp" => b.mcp.is_some(),
        "connect" => b.connect.is_some(),
        "proof" => b.proof.is_some(),
        _ => false,
    }
}

/// What one run of `gen` was asked to write: the targets and the extras, each in the order of its
/// list rather than the order of the command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    targets: Vec<&'static str>,
    extras: Vec<&'static str>,
}

impl Asked {
    /// The targets and extras as the command line gives them, held to what `gen` can write: at least
    /// one target, each name once, and every extra written beside a target given. `Err` is the
    /// sentence `gen` stops with (exit 2).
    pub fn new(targets: &[&str], extras: &[&str]) -> Result<Asked, String> {
        if targets.is_empty() {
            return Err(only_given());
        }
        let names = target_names();
        for (i, t) in targets.iter().enumerate() {
            if !names.contains(t) {
                return Err(tr!(
                    "`--target {t}` という出力先はありません。書けるのは {} だけです",
                    "`--target {t}` is not a target; the targets are {}",
                    names.join(" | ")
                ));
            }
            if targets[..i].contains(t) {
                return Err(tr!("`--target {t}` が二度書かれています", "`--target {t}` is given twice"));
            }
        }
        for (i, e) in extras.iter().enumerate() {
            if !EXTRAS.contains(e) {
                return Err(tr!(
                    "`--extra {e}` という付属物はありません。書けるのは {} だけです",
                    "`--extra {e}` is not an extra; the extras are {}",
                    EXTRAS.join(" | ")
                ));
            }
            if extras[..i].contains(e) {
                return Err(tr!("`--extra {e}` が二度書かれています", "`--extra {e}` is given twice"));
            }
        }
        let asked = Asked {
            targets: names.iter().copied().filter(|n| targets.contains(n)).collect(),
            extras: EXTRAS.iter().copied().filter(|n| extras.contains(n)).collect(),
        };
        // The service is Python, and imports the stubs buf builds from the `.proto`.
        if asked.extra("connect") && !(asked.target("python") && asked.target(PROTO)) {
            return Err(tr!(
                "`--extra connect` には `--target python` と `--target proto` の両方が要ります。サービスは Python で、buf が `.proto` から作るスタブを読みます",
                "`--extra connect` needs both `--target python` and `--target proto`: the service is Python, and it reads the stubs buf builds from the `.proto`"
            ));
        }
        for e in asked.extras.iter().filter(|e| **e != "vectors" && **e != "connect") {
            if asked.targets.iter().any(|t| has_extra(t, e)) {
                continue;
            }
            let of: Vec<&str> = names.iter().copied().filter(|t| *t != PROTO && has_extra(t, e)).collect();
            return Err(match of.as_slice() {
                [one] => tr!(
                    "`--extra {e}` は {one} の付属物です。`--target {one}` がありません",
                    "`--extra {e}` is an extra of {one}, and there is no `--target {one}`"
                ),
                _ => tr!(
                    "`--extra {e}` は {} の付属物です。どれも `--target` にありません",
                    "`--extra {e}` is an extra of {}, and none of them is given as a `--target`",
                    if crate::i18n::ja() { of.join("・") } else { and_list(&of) }
                ),
            });
        }
        Ok(asked)
    }

    /// The same, from flags as a command line writes them (`--target python --extra runner`).
    pub fn from_flags(words: &[&str]) -> Result<Asked, String> {
        let (mut targets, mut extras) = (Vec::new(), Vec::new());
        let mut it = words.iter();
        while let Some(w) = it.next() {
            let (flag, value) = match w.split_once('=') {
                Some((f, v)) => (f, Some(v)),
                None => (*w, it.next().copied()),
            };
            let Some(value) = value else {
                return Err(tr!("`{flag}` に値がありません", "`{flag}` is missing its value"));
            };
            match flag {
                "--target" => targets.push(value),
                "--extra" => extras.push(value),
                _ => return Err(tr!("`{flag}` は出力先の指定ではありません", "`{flag}` does not say what to write")),
            }
        }
        Asked::new(&targets, &extras)
    }

    /// What 0.40.0 wrote with no `--target`: [`UP_TO_0_40`].
    pub fn up_to_0_40() -> Asked {
        Asked::from_flags(UP_TO_0_40).expect("the flags of 0.40.0 name targets and extras that exist")
    }

    /// The targets given, in the order they are written.
    pub fn targets(&self) -> &[&'static str] {
        &self.targets
    }

    /// The extras asked for, in the order of [`EXTRAS`].
    pub fn extras(&self) -> &[&'static str] {
        &self.extras
    }

    pub fn target(&self, t: &str) -> bool {
        self.targets.contains(&t)
    }

    pub fn extra(&self, e: &str) -> bool {
        self.extras.contains(&e)
    }

    /// Whether a file of `part` is written for `target`.
    pub fn wants(&self, target: &str, part: Part) -> bool {
        self.target(target) && part.extras().iter().all(|e| self.extra(e))
    }

    /// The extras of `part` this run was not asked for.
    pub fn not_asked(&self, part: Part) -> Vec<&'static str> {
        part.extras().iter().copied().filter(|e| !self.extra(e)).collect()
    }

    /// Of files of one part, by their paths under the output directory (`python/catalog.py`), the
    /// ones this run writes, and the ones of the targets it was given that it does not: a file is
    /// under the directory of its target, which is the target's name.
    pub fn split(&self, files: Vec<(String, String)>, part: Part) -> (Vec<(String, String)>, Vec<String>) {
        let mut kept = Vec::new();
        let mut not = Vec::new();
        for (rel, body) in files {
            let target = rel.split('/').next().unwrap_or_default();
            if self.wants(target, part) {
                kept.push((rel, body));
            } else if self.target(target) {
                not.push(rel);
            }
        }
        (kept, not)
    }
}

/// The first sentence of the refusal: what `gen` writes, and the names it takes.
fn only_given() -> String {
    let (t, e) = (target_names().join("|"), EXTRAS.join("|"));
    tr!(
        "`rulec gen` が書くのは、指定した出力先だけです。`--target {t}` を一つ以上付けてください。横に書く付属物は `--extra {e}` で指定します",
        "`rulec gen` writes only the targets it is given: give one or more `--target {t}`, and `--extra {e}` for what to write beside them"
    )
}

/// What `gen` says when it is given no `--target` (§15.277), from the words of its command line
/// after `rulec`: the names it takes, and the command that writes what 0.40.0 wrote with none — the
/// same words, its `--lang` too (the runners speak it), with [`UP_TO_0_40`] after them. An
/// `--extra` the line gives is in those already, and is left out.
pub fn no_target(typed: &[String]) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut i = 0;
    while i < typed.len() {
        if typed[i] == "--extra" {
            i += 2;
            continue;
        }
        if !typed[i].starts_with("--extra=") {
            words.push(shell_word(&typed[i]));
        }
        i += 1;
    }
    words.extend(UP_TO_0_40.iter().map(|w| w.to_string()));
    let cmd = format!("rulec {}", words.join(" "));
    let first = only_given();
    tr!(
        "{first}\n注記: 0.40.0 までの `rulec gen` は、`--target` が無ければ、全部の出力先を全部の付属物つきで書きました。同じファイルを書くのは次のコマンドです\n  {cmd}",
        "{first}\nnote: up to 0.40.0, `rulec gen` with no `--target` wrote every target with every extra; this writes the same files:\n  {cmd}"
    )
}

/// What `gen` says, once, after the files of extras it was not asked for that it found where it
/// would have written them: which `--extra` writes them, and that they are the reader's to delete.
pub fn left_note(extras: &[&str]) -> String {
    let flags: Vec<String> = EXTRAS.iter().filter(|e| extras.contains(e)).map(|e| format!("`--extra {e}`")).collect();
    let en = and_list(&flags);
    let ja = flags.join("・");
    // `--extra runner` writes, and is not asked for; `--extra runner` and `--extra mcp` write
    let (write, them) = if flags.len() == 1 { ("writes", "it") } else { ("write", "them") };
    tr!(
        "注記: これらは {ja} が書くファイルで、この実行は頼んでいません。gen は出力先のファイルを消さないので、要らなければ消してかまいません",
        "note: {en} {write} these files, and this run does not ask for {them}; gen deletes nothing in the output directory, so they can be deleted if they are not wanted"
    )
}

/// Names in an English sentence: `a`, `a and b`, `a, b and c`.
fn and_list<S: AsRef<str>>(names: &[S]) -> String {
    let names: Vec<&str> = names.iter().map(AsRef::as_ref).collect();
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => names.join(""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_targets_are_the_backends_and_the_proto() {
        let want: Vec<&str> = crate::backend::ids().into_iter().chain([PROTO]).collect();
        assert_eq!(target_names(), want.as_slice());
    }

    #[test]
    fn the_flags_of_0_40_name_every_target_and_extra_of_0_40() {
        let a = Asked::up_to_0_40();
        assert_eq!(a.targets().len(), 13);
        assert_eq!(a.extras(), EXTRAS);
    }

    #[test]
    fn a_word_is_quoted_only_when_a_shell_would_take_it_apart() {
        assert_eq!(shell_word("rules/送料.rule"), "rules/送料.rule");
        assert_eq!(shell_word("my rules"), "'my rules'");
        assert_eq!(shell_word("it's"), "'it'\\''s'");
        assert_eq!(shell_word(""), "''");
    }

    #[test]
    fn the_note_on_what_is_left_says_one_extra_as_one() {
        crate::i18n::with(crate::i18n::Lang::En, || {
            let tail = "these files, and this run does not ask for";
            assert_eq!(
                left_note(&["round-test"]),
                format!("note: `--extra round-test` writes {tail} it; gen deletes nothing in the output directory, so they can be deleted if they are not wanted")
            );
            assert_eq!(
                left_note(&["round-test", "runner"]),
                format!("note: `--extra runner` and `--extra round-test` write {tail} them; gen deletes nothing in the output directory, so they can be deleted if they are not wanted")
            );
        });
        crate::i18n::with(crate::i18n::Lang::Ja, || {
            assert_eq!(left_note(&["round-test"]), "注記: これらは `--extra round-test` が書くファイルで、この実行は頼んでいません。gen は出力先のファイルを消さないので、要らなければ消してかまいません");
        });
    }

    #[test]
    fn names_in_english_end_with_and() {
        assert_eq!(and_list(&["rust"]), "rust");
        assert_eq!(and_list(&["python", "typescript"]), "python and typescript");
        assert_eq!(and_list(&["python", "typescript", "javascript"]), "python, typescript and javascript");
    }
}
