//! The Lean models of ritsu's languages and of the checks across them, held to the Rust
//! implementations (DESIGN 11.3).
//!
//! `proofs/` states, as Lean functions, what the core of chobo, koyomi and dandori means and what
//! the checks across the languages decide, and proves what each settles. `lake build` there also
//! makes `ritsu-model`, a program that runs those functions: it reads the file a test writes for
//! it (a book, a dates or calendar file, a flow, resolved the way the model reads them), then
//! answers every line of its standard input with one line. The tests of this crate make the
//! inputs — every scenario chobo writes for its books, every input of koyomi's vectors, the
//! scenarios of dandori's flows that stay inside the core, the checks of `ritsu-cross` over the
//! languages' own answers — run each one through the Rust and through the model, and compare the
//! answers a line at a time.
//!
//! This library is the part that does not know any language: where the program is, and running
//! it over a stream of lines while they are made. The languages are the tests' own
//! dev-dependencies (DESIGN 3.1, 3.3); nothing depends on this crate.

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where `lake build` in `proofs/` leaves the program.
pub fn program() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proofs/.lake/build/bin/ritsu-model")
}

/// What a comparison came to: how many lines, how many differed, and the first five that did,
/// each as (input, the language's answer, the model's answer).
#[derive(Debug, Default)]
pub struct Compared {
    pub lines: usize,
    pub differ: usize,
    pub first: Vec<(String, String, String)>,
}

impl Compared {
    /// The first lines that differed, as a test prints them when it fails.
    pub fn report(&self, what: &str) -> String {
        let mut s = format!("{what}: {} of {} lines differ from the model; the first {}:\n", self.differ, self.lines, self.first.len());
        for (input, ours, model) in &self.first {
            s.push_str(&format!("  input: {input}\n  rust:  {ours}\n  lean:  {model}\n"));
        }
        s
    }
}

/// Run `ritsu-model <what> <file>` over `rows` — each an input line and the answer the language
/// gives it — while they are made, and compare the model's answer to each with `same`. The rows
/// are written from a thread of their own as the answers are read, so neither side waits for the
/// other however many there are. Err when the program cannot be run, stops early, or exits with
/// an error.
pub fn compare<I>(program: &Path, what: &str, file: &Path, rows: I, same: &(dyn Fn(&str, &str) -> bool + Sync)) -> Result<Compared, String>
where
    I: Iterator<Item = (String, String)> + Send,
{
    let mut child = Command::new(program)
        .arg(what)
        .arg(file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", program.display()))?;
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let (tx, rx) = std::sync::mpsc::channel::<(String, String)>();
    let compared = std::thread::scope(|scope| {
        let writer = scope.spawn(move || {
            let mut w = BufWriter::new(stdin);
            for (input, ours) in rows {
                if writeln!(w, "{input}").is_err() {
                    break;
                }
                if tx.send((input, ours)).is_err() {
                    break;
                }
            }
            let _ = w.flush();
        });
        let err = scope.spawn(move || {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s);
            s
        });
        let mut reader = BufReader::new(stdout);
        let mut out = Compared::default();
        let mut early = None;
        for (input, ours) in rx {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    early = Some(format!("ritsu-model stopped before answering `{input}`"));
                    break;
                }
                Ok(_) => {}
            }
            let got = line.trim_end_matches(['\n', '\r']);
            out.lines += 1;
            if !same(&ours, got) {
                out.differ += 1;
                if out.first.len() < 5 {
                    out.first.push((input, ours, got.to_string()));
                }
            }
        }
        drop(reader);
        let _ = writer.join();
        let said = err.join().unwrap_or_default();
        match early {
            Some(e) => Err(format!("{e}\n{said}")),
            None => Ok((out, said)),
        }
    });
    let status = child.wait().map_err(|e| format!("ritsu-model did not end: {e}"))?;
    let (out, said) = compared?;
    if !status.success() {
        return Err(format!("ritsu-model {what} {} exited with {status}:\n{said}", file.display()));
    }
    Ok(out)
}
