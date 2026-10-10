//! What a command says when it needs another program or the network and runs where it can have
//! neither (`ritsu_base::wasi`, ritsu's DESIGN 8.8): ritsu built for WASI, the npm package. Natively
//! nothing answers `Unsupported`, and a program can be started.

use ritsu_base::text::Lang;
use ritsu_base::wasi;
use std::io;

#[test]
fn the_platform_without_processes_is_told_by_the_kind_of_error() {
    assert!(wasi::unsupported(&io::Error::new(io::ErrorKind::Unsupported, "operation not supported on this platform")));
    assert!(!wasi::unsupported(&io::Error::new(io::ErrorKind::NotFound, "No such file or directory (os error 2)")));
    assert!(wasi::can_start_programs(), "a native test can start a program");
    let missing = std::process::Command::new("ritsu-no-such-program").output().unwrap_err();
    assert_eq!(wasi::starting("ritsu-no-such-program", &missing).get(Lang::En), missing.to_string(), "a program not installed is said as std says it");
}

#[test]
fn the_sentences_say_what_cannot_run_and_what_to_run_instead() {
    for (t, what) in [(wasi::cannot_start("git"), "`git`"), (wasi::cannot_connect("`api`"), "`api`")] {
        assert!(t.get(Lang::En).starts_with("the WebAssembly build of ritsu (the npm package) cannot "), "{}", t.get(Lang::En));
        assert!(t.get(Lang::En).contains(what) && t.get(Lang::Ja).contains(what));
        assert!(t.get(Lang::En).ends_with("Run this command with the native ritsu (a release's archive, Homebrew, the .deb or the .rpm)"));
        assert!(t.get(Lang::Ja).ends_with("このコマンドは、ネイティブの ritsu（リリースのアーカイブ、Homebrew、.deb、.rpm）で走らせてください"));
    }
    let any = wasi::cannot_start_programs();
    assert!(any.get(Lang::En).contains("cannot start the programs this command runs"));
    assert!(any.get(Lang::Ja).contains("このコマンドが使うほかのプログラムを起動できません"));
    let unsupported = io::Error::new(io::ErrorKind::Unsupported, "operation not supported on this platform");
    assert_eq!(wasi::starting("curl", &unsupported).get(Lang::En), wasi::cannot_start("curl").get(Lang::En));
}
