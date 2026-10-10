//! What ritsu built for WASI cannot do, and what a command says when it comes to it (DESIGN 8.8).
//!
//! The npm package runs `ritsu` compiled to WebAssembly for WASI preview 1, which has neither
//! processes nor sockets: there, std answers every `Command::spawn` and `output`,
//! `TcpStream::connect`, `TcpListener::bind` and `current_exe` with `ErrorKind::Unsupported`
//! ("operation not supported on this platform"), which tells a person nothing of what to do. A
//! command that needs another program (git, curl, cargo, the programs a claim of geas runs) or
//! the network says the sentence here instead, in its own frame: the head of its diagnostic and
//! its exit code stay what they are when the program is missing on a native machine. On every
//! other target nothing answers `Unsupported`, and nothing changes.

use crate::text::Text;
use std::io;

/// Whether `e` is the platform saying it has no processes or sockets.
pub fn unsupported(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::Unsupported
}

/// Whether a program can be started here at all: false only where the platform answers that it
/// has no processes. The start asked for (a program with no name) fails everywhere, at once and
/// without starting anything, and only such a platform fails it with `Unsupported`. For a command
/// that would otherwise read every missing program as a toolchain not installed (`rulec test`).
pub fn can_start_programs() -> bool {
    !matches!(std::process::Command::new("").output(), Err(e) if unsupported(&e))
}

/// Why, and what to do: the second half of every sentence here.
fn because() -> (&'static str, &'static str) {
    (
        "WASI では、ほかのプログラムを起動することも、ネットワークに接続することもできないからです。このコマンドは、ネイティブの ritsu（リリースのアーカイブ、Homebrew、.deb、.rpm）で走らせてください",
        "under WASI a program can neither start another nor reach the network. Run this command with the native ritsu (a release's archive, Homebrew, the .deb or the .rpm)",
    )
}

/// What a command says when it needs to start `program` (as a person runs it: `git`, `curl`,
/// `cargo`) and cannot.
pub fn cannot_start(program: &str) -> Text {
    let (ja, en) = because();
    Text::new(
        format!("WebAssembly 版の ritsu（npm のパッケージ）は `{program}` を起動できません。{ja}"),
        format!("the WebAssembly build of ritsu (the npm package) cannot start `{program}`: {en}"),
    )
}

/// What a command says when the programs it starts are many, or are what it is given to run (the
/// toolchains `rulec test` runs the generated code with, the programs a claim of geas starts).
pub fn cannot_start_programs() -> Text {
    let (ja, en) = because();
    Text::new(
        format!("WebAssembly 版の ritsu（npm のパッケージ）は、このコマンドが使うほかのプログラムを起動できません。{ja}"),
        format!("the WebAssembly build of ritsu (the npm package) cannot start the programs this command runs: {en}"),
    )
}

/// What a command says when it needs to reach `address` over the network and cannot.
pub fn cannot_connect(address: &str) -> Text {
    let (ja, en) = because();
    Text::new(
        format!("WebAssembly 版の ritsu（npm のパッケージ）は {address} に接続できません。{ja}"),
        format!("the WebAssembly build of ritsu (the npm package) cannot connect to {address}: {en}"),
    )
}

/// `e`, from starting `program`, as a message quotes it: [`cannot_start`] when the platform has
/// no processes, else the error's own words.
pub fn starting(program: &str, e: &io::Error) -> Text {
    if unsupported(e) { cannot_start(program) } else { Text::same(e.to_string()) }
}
