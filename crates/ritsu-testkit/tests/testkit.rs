//! What the testkit does, held to itself: directories that remove themselves and those of ended
//! test processes, finding a program, the time limit, golden files, the SKIP line and its log,
//! the levels, and what a test starts — PostgreSQL, TigerBeetle, Chrome, Mermaid, a small HTTP
//! server — each stopped when it is dropped.
//!
//! What reads an environment variable (`RITSU_SKIP_LOG`, `RITSU_TEST_LEVEL`, `RITSU_BLESS`) is
//! tried in a child: this test binary run again on one test, with the variable set for it alone.

use ritsu_testkit::{TempDir, chrome, golden, level, mermaid, pg, run, skip, tigerbeetle, tmp, tools};
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// This test binary, run again on `test` alone, with `envs`: what the child printed.
fn child(test: &str, envs: &[(&str, &str)]) -> run::Ran {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", test, "--nocapture", "--test-threads=1"]).env("RITSU_TESTKIT_CHILD", "1");
    for v in ["RITSU_SKIP_LOG", "RITSU_TEST_LEVEL", "RITSU_BLESS", "RITSU_TESTKIT_BLESS"] {
        c.env_remove(v);
    }
    for (k, v) in envs {
        c.env(k, v);
    }
    run::run(&mut c, Duration::from_secs(60))
}

fn in_child() -> bool {
    std::env::var_os("RITSU_TESTKIT_CHILD").is_some()
}

#[test]
fn a_directory_removes_itself() {
    let kept;
    {
        let t = TempDir::new("a b/c");
        let name = t.path().file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with(&format!("ritsu-test-ritsu-testkit-{}-", std::process::id())), "{name}");
        assert!(name.ends_with("-a-b-c"), "{name}");
        let p = t.write("x/y.txt", "中身");
        assert_eq!(t.read("x/y.txt"), "中身");
        assert!(p.starts_with(t.path()));
        kept = t.path().to_path_buf();
        assert!(kept.is_dir());
    }
    assert!(!kept.exists(), "dropped, it is gone");
}

#[test]
fn the_directories_of_ended_processes_are_removed_and_their_servers_stopped() {
    // A process that has ended, and its pid.
    let mut gone = Command::new("true").spawn().unwrap();
    let pid = gone.id();
    gone.wait().unwrap();
    assert!(!tmp::alive(pid));
    assert!(tmp::alive(std::process::id()));
    // A server it left running: a sleep, noted under its program's name.
    let mut server = Command::new("sleep").arg("60").spawn().unwrap();
    let left = std::env::temp_dir().join(format!("ritsu-test-ritsu-testkit-{pid}-0-left"));
    std::fs::create_dir_all(&left).unwrap();
    std::fs::write(left.join(".servers"), format!("{} sleep\n", server.id())).unwrap();
    // A directory of this process is kept.
    let mine = TempDir::new("mine");
    tmp::sweep();
    assert!(!left.exists(), "the ended process's directory is removed");
    assert!(mine.path().exists(), "this process's is kept");
    let start = Instant::now();
    loop {
        if let Some(status) = server.try_wait().unwrap() {
            assert!(!status.success(), "the server was killed");
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "the server is still running");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn copying_a_directory() {
    let t = TempDir::new("copy");
    t.write("from/a.txt", "a");
    t.write("from/d/b.txt", "b");
    tmp::copy_dir(&t.path().join("from"), &t.path().join("to"));
    assert_eq!(t.read("to/d/b.txt"), "b");
    assert_eq!(t.read("to/a.txt"), "a");
}

#[test]
fn finding_a_program() {
    assert_eq!(tools::vars("CHROME"), ["RITSU_CHROME", "RITSU_TESTKIT_CHROME"]);
    assert!(tools::on_path("sh").is_some());
    assert!(tools::on_path("ritsu-no-such-program").is_none());
    assert!(tools::runs("sh", &["-c", "exit 0"]));
    assert!(!tools::runs("sh", &["-c", "exit 1"]));
    assert!(!tools::runs("ritsu-no-such-program", &[]));
    assert!(tools::version("sh", "-c").is_empty() || !tools::version("sh", "-c").contains('\n'));
    let t = TempDir::new("fallback");
    let f = t.write("tools/bin/x", "");
    assert_eq!(tools::find("RITSU_TESTKIT_NO_SUCH_TOOL", Some(&f), "ritsu-no-such-program", &[]), Some(f.canonicalize().unwrap()));
    assert_eq!(tools::find("RITSU_TESTKIT_NO_SUCH_TOOL", None, "sh", &["-c", "exit 0"]), tools::on_path("sh"));
    assert_eq!(tools::find("RITSU_TESTKIT_NO_SUCH_TOOL", None, "ritsu-no-such-program", &[]), None);
}

#[test]
fn a_program_runs_to_its_end_or_its_time_limit() {
    let r = run::run(Command::new("sh").args(["-c", "echo out; echo err >&2; exit 3"]), Duration::from_secs(10));
    assert_eq!((r.ok, r.code, r.stdout.as_str(), r.stderr.as_str(), r.timed_out), (false, Some(3), "out\n", "err\n", false));
    let start = Instant::now();
    let r = run::run(Command::new("sleep").arg("5"), Duration::from_millis(200));
    assert!(r.timed_out && !r.ok && r.code.is_none());
    assert!(start.elapsed() < Duration::from_secs(3), "killed at its limit");
    let r = run::run_with_input(&mut Command::new("cat"), Some("入力\n".as_bytes()), Duration::from_secs(10));
    assert_eq!(r.stdout, "入力\n");
    let mut seen = Vec::new();
    let r = run::pipe(
        Command::new("sh").args(["-c", "while read l; do echo \"<$l>\"; done; echo done >&2"]),
        Duration::from_secs(10),
        |w| {
            for i in 0..3 {
                writeln!(w, "{i}").unwrap();
            }
        },
        |l| seen.push(l.to_string()),
    );
    assert!(r.ok, "{}", r.stderr);
    assert_eq!(seen, ["<0>", "<1>", "<2>"]);
    assert_eq!(r.stderr, "done");
}

#[test]
fn a_golden_file_is_compared_and_says_how_it_differs() {
    let t = TempDir::new("golden");
    let p = t.write("g.txt", "a\nb\nc\n");
    assert!(golden::check(&p, "a\nb\nc\n").is_ok());
    let e = golden::check(&p, "a\nB\nc\n").unwrap_err();
    assert!(e.contains("  a\n+ B\n- b\n  c\n"), "{e}");
    let e = golden::check(&t.path().join("none.txt"), "x").unwrap_err();
    assert!(e.contains("is missing; write it with RITSU_BLESS=1"), "{e}");
    assert_eq!(golden::line_diff("x\ny\n", "y\nz\n"), "- x\n  y\n+ z\n");
}

#[test]
fn bless_writes_the_golden_instead() {
    if in_child() {
        let p = Path::new(&std::env::var("GOLDEN_AT").unwrap()).to_path_buf();
        golden::golden(&p, "new\n");
        return;
    }
    let t = TempDir::new("bless");
    let p = t.write("g.txt", "old\n");
    for var in ["RITSU_BLESS", "RITSU_TESTKIT_BLESS"] {
        std::fs::write(&p, "old\n").unwrap();
        let r = child("bless_writes_the_golden_instead", &[(var, "1"), ("GOLDEN_AT", p.to_str().unwrap())]);
        assert!(r.ok, "{var}: {}", r.both());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "new\n", "{var}");
    }
    let r = child("bless_writes_the_golden_instead", &[("GOLDEN_AT", p.to_str().unwrap()), ("RITSU_BLESS", "0")]);
    std::fs::write(&p, "old\n").unwrap();
    assert!(!r.ok || std::fs::read_to_string(&p).unwrap() == "old\n", "RITSU_BLESS=0 does not bless");
}

#[test]
fn a_skip_is_said_and_logged() {
    if in_child() {
        skip::skip("no\tsuch\ntool here");
        return;
    }
    let t = TempDir::new("skiplog");
    let log = t.path().join("skips.tsv");
    let r = child("a_skip_is_said_and_logged", &[("RITSU_SKIP_LOG", log.to_str().unwrap())]);
    assert!(r.ok, "{}", r.both());
    assert!(r.stdout.contains("SKIP: ritsu-testkit: no\tsuch\ntool here\n"), "{}", r.stdout);
    let logged = skip::read_log(&std::fs::read_to_string(&log).unwrap());
    assert_eq!(logged, [skip::Logged { krate: "ritsu-testkit".into(), test: "a_skip_is_said_and_logged".into(), why: skip::Why::Missing, reason: "no such tool here".into() }]);
    assert!(skip::read_log("a\tb\tsomething\tc\nonly three\tfields\there\n").is_empty(), "a line of another kind is passed over");
}

#[test]
fn the_levels() {
    use level::{Level, Need};
    if in_child() {
        assert!(!level::need(Need::Temporal), "a platform is above the tools level");
        assert!(level::need(Need::Postgres), "PostgreSQL is a tool");
        assert!(level::allows(Level::Fast) && level::allows(Level::Tools) && !level::allows(Level::Platforms));
        return;
    }
    assert_eq!(Level::parse("tools"), Some(Level::Tools));
    assert_eq!(Level::parse("all"), None);
    assert!(Level::Fast < Level::Tools && Level::Tools < Level::Platforms);
    assert_eq!(Need::Network.level(), Level::Platforms);
    assert_eq!(Need::Chrome.level(), Level::Tools);
    let t = TempDir::new("levellog");
    let log = t.path().join("skips.tsv");
    let r = child("the_levels", &[("RITSU_TEST_LEVEL", "tools"), ("RITSU_SKIP_LOG", log.to_str().unwrap())]);
    assert!(r.ok, "{}", r.both());
    let logged = skip::read_log(&std::fs::read_to_string(&log).unwrap());
    assert_eq!(logged.iter().map(|l| l.why).collect::<Vec<_>>(), [skip::Why::Level], "the level left it out");
    assert!(r.stdout.contains("SKIP: ritsu-testkit: needs temporal (the platforms level); RITSU_TEST_LEVEL is tools"), "{}", r.stdout);
    let r = child("the_levels", &[("RITSU_TEST_LEVEL", "everything")]);
    assert!(!r.ok, "a level that is not one stops the test");
    assert!(r.both().contains("RITSU_TEST_LEVEL=everything is not a level"), "{}", r.both());
}

/// `ready` asks the level first (a SKIP of the level, and the machine is not asked), then the
/// machine (a SKIP for what is missing). The binaries of the other languages and pixie's greeter
/// are tools.
#[test]
fn ready_asks_the_level_then_the_machine() {
    use level::{Level, Need};
    if in_child() {
        let fast = std::env::var("RITSU_TEST_LEVEL").as_deref() == Ok("fast");
        let mut asked = false;
        let went = level::ready(
            Need::Suite,
            || {
                asked = true;
                false
            },
            "no binaries of the suite here",
        );
        assert!(!went);
        assert_eq!(asked, !fast, "at the fast level the machine is not asked");
        assert_eq!(level::ready(Need::Pixie, || true, "not said"), !fast);
        return;
    }
    for n in [Need::Suite, Need::Pixie] {
        assert_eq!(n.level(), Level::Tools, "{n:?}");
    }
    let t = TempDir::new("readylog");
    for (top, want) in [("fast", vec![skip::Why::Level, skip::Why::Level]), ("tools", vec![skip::Why::Missing])] {
        let log = t.path().join(format!("{top}.tsv"));
        let r = child("ready_asks_the_level_then_the_machine", &[("RITSU_TEST_LEVEL", top), ("RITSU_SKIP_LOG", log.to_str().unwrap())]);
        assert!(r.ok, "{}", r.both());
        let logged = skip::read_log(&std::fs::read_to_string(&log).unwrap());
        assert_eq!(logged.iter().map(|l| l.why).collect::<Vec<_>>(), want, "RITSU_TEST_LEVEL={top}");
    }
}

#[test]
fn a_postgresql_cluster_is_started_and_stopped() {
    if !level::need(level::Need::Postgres) {
        return;
    }
    let pg = match pg::Postgres::start() {
        Ok(p) => p,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    assert_eq!(pg.sql("select 40 + 2").unwrap().trim(), "42");
    assert!(pg.sql("select nonsense").is_err());
    let socket = pg.socket.clone();
    assert!(socket.as_os_str().len() + ".s.PGSQL.65535.lock".len() < pg::SOCKET_PATH_MAX + 2);
    drop(pg);
    assert!(!socket.exists(), "the socket's directory is removed");
}

#[test]
fn a_tigerbeetle_replica_is_started_and_stopped() {
    if !level::need(level::Need::TigerBeetle) {
        return;
    }
    let tb = match tigerbeetle::TigerBeetle::start() {
        Ok(t) => t,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let addr = tb.address.clone();
    assert!(std::net::TcpStream::connect(&addr).is_ok(), "it takes connections");
    drop(tb);
    let start = Instant::now();
    while std::net::TcpStream::connect(&addr).is_ok() {
        assert!(start.elapsed() < Duration::from_secs(10), "it still takes connections");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn chrome_dumps_a_page_and_draws_it() {
    if !level::need(level::Need::Chrome) {
        return;
    }
    let Some(c) = chrome::find() else {
        skip("Chrome is not found (RITSU_CHROME, macOS's Google Chrome, or google-chrome or chromium on the PATH)");
        return;
    };
    let t = TempDir::new("chrome");
    let page = t.write("p.html", "<!doctype html><html><body><p id=\"x\">before</p><script>document.getElementById('x').textContent = 'after ' + (6 * 7);</script></body></html>");
    let url = format!("file://{}", page.display());
    let dom = chrome::dump_dom(&c, &url, 2000, Duration::from_secs(60));
    assert!(dom.contains("<p id=\"x\">after 42</p>"), "{dom}");
    let png = t.path().join("p.png");
    chrome::screenshot(&c, &url, &png, 400, 300, Duration::from_secs(60)).unwrap();
    assert_eq!(chrome::png_size(&std::fs::read(&png).unwrap()), Some((400, 300)));
    assert!(chrome::png_size(b"not a png").is_none());
    std::thread::sleep(Duration::from_millis(300));
    chrome::none_left(t.path()).unwrap();
}

#[test]
fn mermaid_draws_the_charts_of_a_page() {
    let md = "# x\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\ntext\n\n```mermaid\nflowchart LR\n  a -->\n```\n";
    let charts = mermaid::charts(md);
    assert_eq!(charts, ["flowchart LR\n  a --> b\n", "flowchart LR\n  a -->\n"]);
    if !level::need(level::Need::Mermaid) {
        return;
    }
    let Some(c) = chrome::find() else {
        skip("Chrome is not found, which Mermaid draws in");
        return;
    };
    // The Mermaid the crates install for their doc tests.
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let Some(nm) = ["chobo/tools/mermaid/node_modules", "dandori/tools/mermaid/node_modules"].iter().map(|p| here.join(p)).find(|p| p.is_dir()) else {
        skip("no tools/mermaid/node_modules in chobo or dandori (npm ci --prefix tools/mermaid)");
        return;
    };
    let scripts = match mermaid::scripts(&nm) {
        Ok(s) => s,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    for (major, script) in scripts {
        let got = mermaid::draw(&c, &script, &charts).unwrap_or_else(|| panic!("Mermaid {major} did not answer for every chart"));
        assert_eq!(got[0], "ok", "Mermaid {major}");
        assert!(got[1].starts_with("error: "), "Mermaid {major}: {}", got[1]);
    }
}

#[test]
fn a_small_http_server_answers_and_keeps_what_it_could_not() {
    let s = ritsu_testkit::http::HttpServer::start();
    s.set("/a?x=1", "答え");
    s.set_status("/broken", 500, "no");
    let get = |target: &str| -> String {
        let mut c = std::net::TcpStream::connect(s.addr.trim_start_matches("http://")).unwrap();
        write!(c, "GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        c.read_to_string(&mut out).unwrap();
        out
    };
    let r = get("/a?x=1");
    assert!(r.starts_with("HTTP/1.1 200 OK\r\n") && r.ends_with("\r\n\r\n答え"), "{r}");
    assert!(get("/broken").starts_with("HTTP/1.1 500 "));
    assert!(get("/none").starts_with("HTTP/1.1 404 Not Found"));
    assert_eq!(s.asked(), ["/a?x=1", "/broken", "/none"]);
    assert_eq!(s.missed(), ["/none"]);
}
