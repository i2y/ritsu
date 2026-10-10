//! The npm package `@i2y/ritsu` (DESIGN 8.8): `ritsu` built for WASI (wasm32-wasip1) and run by
//! Node, as `packaging/npm/build.sh` packs it.
//!
//! - `the_package_answers_as_the_binary_does`: the commands a person runs on the examples of the
//!   eight languages (rulec's corpus is its examples), ritsu's test projects, the shops of the
//!   playground and sekisho's example, in English and in Japanese, each run by the native binary
//!   (`ritsu`, and a link to it named for each language) and by the package (`node
//!   bin/<name>.js`) in the same copy of the files: standard output, standard error, exit code, and
//!   every file the command wrote, byte for byte.
//! - `the_api_in_the_dirs_of_a_project_answers_as_the_binary_does`: the same commands (but `explain`)
//!   through the API with `dirs`, the module opening the copy of the project alone.
//! - `what_needs_another_program_says_so`: a command that starts another program or reaches the
//!   network stops in the package with the language's own head and exit code, saying why and what
//!   to run instead (ritsu-base's `wasi`).
//! - `the_package_holds_what_it_should`: what `npm pack` puts in, and package.json's version,
//!   license, bins and engines, with no script npm would run on install.
//! - `the_javascript_tests_pass`: `node --test packaging/npm/test/*.test.js`: the API, the bins, and the
//!   package installed the way a job over many repositories installs one.
//! - `the_manifest_is_the_workspaces`: packaging/npm/package.json says the workspace's version
//!   and license (no node needed).
//! - `the_pages_on_node_show_what_the_package_prints`: the package's README and the site's page on
//!   Node, in English and in Japanese, show what the package prints.
//!
//! The package is the one `packaging/npm/build.sh <dir>` wrote: `RITSU_NPM` names the .tgz, else
//! `<target>/npm/i2y-ritsu-<version>.tgz`. Without node, or without that file, a test says SKIP
//! and passes; these are tests of the tools level.

mod common;

/// In a module of their own, so that a run picks them by name: `cargo xtask test --level tools -p
/// ritsu -- npm::` (tools.yml runs them so, with each Node, and the job of the other tests of ritsu
/// passes them over with `--skip npm::`).
mod npm {
    use super::common;
    use ritsu_testkit::{Need, TempDir, ready};
    use std::collections::BTreeMap;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;
    use std::time::Duration;

    const LANGUAGES: [&str; 8] = ["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai", "sekisho"];
    const LANG_VARS: [&str; 9] = ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG"];
    /// A run of the debug build on the largest rule of the corpus takes seconds; one that takes this
    /// long is stuck.
    const LIMIT: Duration = Duration::from_secs(600);

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
    }

    fn ritsu() -> &'static str {
        env!("CARGO_BIN_EXE_ritsu")
    }

    /// The package's .tgz: `RITSU_NPM`, else where packaging/npm/build.sh writes it when it is given
    /// `<target>/npm`.
    fn tgz() -> PathBuf {
        match std::env::var_os("RITSU_NPM").filter(|v| !v.is_empty()) {
            Some(p) => PathBuf::from(p),
            None => Path::new(env!("CARGO_TARGET_TMPDIR")).parent().unwrap().join("npm").join(format!("i2y-ritsu-{}.tgz", env!("CARGO_PKG_VERSION"))),
        }
    }

    /// The package unpacked into `t` (`<t>/package`); None, with a SKIP, when node or the package is
    /// not here.
    fn package(t: &TempDir) -> Option<PathBuf> {
        if !ready(Need::Node, || ritsu_testkit::tools::runs("node", &["--version"]), "node is not on the PATH, so the npm package is not run") {
            return None;
        }
        let tgz = tgz();
        let why = format!("there is no package at {} (packaging/npm/build.sh <dir> writes one; RITSU_NPM names it)", tgz.display());
        if !ready(Need::Node, || tgz.is_file(), &why) {
            return None;
        }
        let st = Command::new("tar").arg("-xzf").arg(&tgz).arg("-C").arg(t.path()).status().unwrap();
        assert!(st.success(), "cannot unpack {}", tgz.display());
        Some(t.path().join("package"))
    }

    /// What a run said.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Said {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    }

    fn quiet(c: &mut Command) {
        for v in LANG_VARS {
            c.env_remove(v);
        }
    }

    /// `<name> <args>` by the native binary: `ritsu`, or a link to it named for a language.
    fn native(links: &Path, name: &str, args: &[String], dir: &Path, stdin: Option<&[u8]>) -> Said {
        let mut c = Command::new(links.join(name));
        c.args(args).current_dir(dir);
        quiet(&mut c);
        let r = ritsu_testkit::run::run_with_input(&mut c, stdin, LIMIT);
        assert!(!r.timed_out, "native `{name} {}` in {} ran past the limit", args.join(" "), dir.display());
        Said { code: r.code, stdout: r.stdout, stderr: r.stderr }
    }

    /// `<name> <args>` by the package: `node <package>/bin/<name>.js`.
    fn npm(pkg: &Path, name: &str, args: &[String], dir: &Path, stdin: Option<&[u8]>) -> Said {
        let mut c = Command::new("node");
        c.arg(pkg.join("bin").join(format!("{name}.js"))).args(args).current_dir(dir);
        quiet(&mut c);
        let r = ritsu_testkit::run::run_with_input(&mut c, stdin, LIMIT);
        assert!(!r.timed_out, "npm `{name} {}` in {} ran past the limit", args.join(" "), dir.display());
        Said { code: r.code, stdout: r.stdout, stderr: r.stderr }
    }

    /// A directory holding a link to the native `ritsu` for `ritsu` and for each of the eight
    /// languages, as a release's archive does.
    fn links(t: &TempDir) -> PathBuf {
        for name in std::iter::once("ritsu").chain(LANGUAGES) {
            symlink(ritsu(), t.path().join(name)).unwrap();
        }
        t.path().to_path_buf()
    }

    /// Every file under `dir`, by its path from it, with what it holds.
    fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut todo = vec![dir.to_path_buf()];
        while let Some(d) = todo.pop() {
            for e in std::fs::read_dir(&d).unwrap().map(|e| e.unwrap()) {
                let p = e.path();
                let t = e.file_type().unwrap();
                if t.is_dir() {
                    todo.push(p);
                } else {
                    let rel = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                    out.insert(rel, if t.is_symlink() { std::fs::read_link(&p).unwrap().to_string_lossy().as_bytes().to_vec() } else { std::fs::read(&p).unwrap() });
                }
            }
        }
        out
    }

    /// One command of a case.
    #[derive(Clone, Debug)]
    struct Step {
        name: &'static str,
        args: Vec<String>,
        stdin: Option<Vec<u8>>,
    }

    impl Step {
        fn shown(&self) -> String {
            format!("{} {}", self.name, self.args.join(" "))
        }
    }

    /// `<name> <args>` in English, by the language's own name (`rulec check x.rule`).
    fn en(name: &'static str, args: &[&str]) -> Step {
        Step { name, args: args.iter().map(|s| s.to_string()).collect(), stdin: None }
    }

    /// The same in Japanese, as `ritsu <name> <args> --lang ja` (or `ritsu <args> --lang ja`): the
    /// other way to call a language.
    fn ja(name: &'static str, args: &[&str]) -> Step {
        let mut a: Vec<String> = if name == "ritsu" { vec![] } else { vec![name.to_string()] };
        a.extend(args.iter().map(|s| s.to_string()));
        a.extend(["--lang".to_string(), "ja".to_string()]);
        Step { name: "ritsu", args: a, stdin: None }
    }

    /// Both.
    fn both(name: &'static str, args: &[&str]) -> [Step; 2] {
        [en(name, args), ja(name, args)]
    }

    /// The commands of a case, run in order in one copy; what each said is held to the other side's.
    #[derive(Clone, Debug)]
    struct Case {
        /// What the counts are kept under: a language, or ritsu.
        of: &'static str,
        /// Where in the copy the commands run.
        at: String,
        steps: Vec<Step>,
        /// After the steps, `run` on every scenario they wrote: `(command, flow, directory)`.
        scenarios: Option<(&'static str, String, String)>,
    }

    /// Files copied from the repository (`from`, into `into` under the copy), and what is run there.
    /// Cases that only read share one copy, which must be as it was copied when they are done; a case
    /// that writes gets a copy of its own for each side, and what it wrote is held to the other's.
    struct Group {
        from: String,
        into: String,
        reads: Vec<Case>,
        writes: Vec<Case>,
    }

    /// A fresh copy: `from` under `<t>/<into>`, and an empty `.git` at `<t>`, so that a command finds
    /// the root of a repository there as it would in a clone.
    fn copy(from: &str, into: &str) -> TempDir {
        let t = TempDir::new("npm-copy");
        std::fs::create_dir_all(t.path().join(".git")).unwrap();
        ritsu_testkit::tmp::copy_dir(&root().join(from), &t.path().join(into));
        t
    }

    /// Run a case's steps (and the runs of the scenarios they wrote) by one side; what each said.
    /// A side of a comparison: what runs one step in `at`, in a copy whose root is the other path.
    type Side<'a> = dyn Fn(&Step, &Path, &Path) -> Said + Sync + 'a;

    fn run_case(c: &Case, dir: &Path, side: &Side) -> Vec<(String, Said)> {
        let at = dir.join(&c.at);
        let mut out: Vec<(String, Said)> = c.steps.iter().map(|s| (s.shown(), side(s, &at, dir))).collect();
        if let Some((name, flow, scen)) = &c.scenarios {
            let mut files: Vec<String> =
                std::fs::read_dir(at.join(scen)).map(|rd| rd.map(|e| e.unwrap().file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".json")).collect()).unwrap_or_default();
            files.sort();
            for f in files {
                let path = format!("{scen}/{f}");
                for extra in [&[][..], &["--format", "json"][..]] {
                    let mut args = vec!["run", flow.as_str(), "--scenario", path.as_str()];
                    args.extend(extra);
                    let s = if *name == "ritsu" { en("ritsu", &args) } else { en(name, &args) };
                    out.push((s.shown(), side(&s, &at, dir)));
                }
            }
        }
        out
    }

    /// How two sides differ, or None.
    fn differs(cmd: &str, a: &Said, b: &Said) -> Option<String> {
        if a == b {
            return None;
        }
        let first = |x: &str, y: &str| {
            let (xl, yl): (Vec<&str>, Vec<&str>) = (x.lines().collect(), y.lines().collect());
            let i = (0..xl.len().max(yl.len())).find(|&i| xl.get(i) != yl.get(i)).unwrap_or(0);
            format!("line {}: native {:?}, npm {:?}", i + 1, xl.get(i).unwrap_or(&"<end>"), yl.get(i).unwrap_or(&"<end>"))
        };
        let mut s = format!("`{cmd}`: native exit {:?}, npm exit {:?}", a.code, b.code);
        if a.stdout != b.stdout {
            s.push_str(&format!("; stdout {}", first(&a.stdout, &b.stdout)));
        }
        if a.stderr != b.stderr {
            s.push_str(&format!("; stderr {}", first(&a.stderr, &b.stderr)));
        }
        Some(s)
    }

    /// The files under `dir` (from the repository's root) that end with `ext`, by their path from it,
    /// in path order.
    fn files(dir: &str, ext: &str) -> Vec<String> {
        let base = root().join(dir);
        let mut out: Vec<String> = tree(&base).into_keys().filter(|p| p.ends_with(ext)).collect();
        out.sort();
        out
    }

    /// The codes `<name> explain --all` lists, in its order.
    fn codes(links: &Path, name: &str) -> Vec<String> {
        let s = native(links, name, &["explain".into(), "--all".into()], &root(), None);
        let mut out = Vec::new();
        for l in s.stdout.lines() {
            // `error[E001]: …` (rulec, chobo, geas) or `E001 (error) — …` (koyomi, yuen, sakai)
            let code = l.split_once('[').filter(|(h, _)| ["error", "warning", "note", "info"].contains(h)).map(|(_, r)| r.split(']').next().unwrap_or("").to_string()).or_else(|| {
                let w = l.split(' ').next().unwrap_or("");
                (l.contains(" (") && w.len() == 4 && w[1..].bytes().all(|b| b.is_ascii_digit())).then(|| w.to_string())
            });
            if let Some(c) = code
                && c.len() == 4
                && !out.contains(&c)
            {
                out.push(c);
            }
        }
        if out.is_empty() {
            // dandori, sekisho and ritsu list their codes as JSON
            let j = native(links, name, &["explain".into(), "--all".into(), "--format".into(), "json".into()], &root(), None);
            let v: serde_json::Value = serde_json::from_str(&j.stdout).unwrap_or_default();
            out = v.as_array().into_iter().flatten().filter_map(|e| e["code"].as_str().map(str::to_string)).collect();
        }
        assert!(!out.is_empty(), "{name} explain --all listed no code");
        out
    }

    /// What is run on the examples of each language, ritsu's projects and the playground's shops.
    fn groups(links: &Path) -> Vec<Group> {
        let mut gs = Vec::new();
        let case = |of: &'static str, at: &str, steps: Vec<Step>| Case { of, at: at.to_string(), steps, scenarios: None };

        // ritsu itself: every code, every Agent Skill, the pages of help
        {
            let mut reads = Vec::new();
            for code in codes(links, "ritsu") {
                reads.push(case("ritsu", ".", both("ritsu", &["explain", &code]).to_vec()));
            }
            for args in [
                &["explain", "--all"][..],
                &["explain", "--all", "--format", "markdown"],
                &["explain", "--all", "--format", "json"],
                &["skills", "list"],
                &["--help"],
                &["--version"],
                &["help", "check"],
                &["help", "gen"],
                &["help", "run"],
                &["help", "skills"],
                &["gen", "--help"],
            ] {
                reads.push(case("ritsu", ".", both("ritsu", args).to_vec()));
            }
            for l in LANGUAGES {
                reads.push(case(l, ".", vec![en(l, &["--help"]), en(l, &["--version"]), en("ritsu", &["help", l])]));
            }
            let writes = vec![
                case("ritsu", ".", both("ritsu", &["skills", "install", "--dir", "skills-out"]).to_vec()),
                case("ritsu", ".", vec![en("ritsu", &["skills", "install", "rulec", "dandori", "--dir", "two"]), en("ritsu", &["skills", "install", "--dir", "two"])]),
            ];
            gs.push(Group { from: "crates/ritsu/tests/projects/stockroom".into(), into: ".".into(), reads, writes });
        }

        // ritsu's projects and the playground's shops: check, gen, run
        for (from, gates) in [
            ("crates/ritsu/tests/projects/shop", false),
            ("crates/ritsu/tests/projects/通販", false),
            ("crates/ritsu/tests/projects/invoice", false),
            ("crates/ritsu/tests/projects/stockroom", false),
            ("website/playground/shop", true),
            ("website/playground/shop.ja", true),
        ] {
            let mut reads = vec![case("ritsu", ".", vec![en("ritsu", &["check"]), en("ritsu", &["check", ".", "--format", "json"]), ja("ritsu", &["check", "."])])];
            reads.push(case("ritsu", ".", both("ritsu", &["gen", "--check"]).to_vec()));
            reads.push(case("ritsu", ".", both("ritsu", &["gen", "--check", "--format", "json"]).to_vec()));
            let mut writes = Vec::new();
            for t in [&[][..], &["--target", "typescript"], &["--target", "python"], &["--target", "go", "--module", "example.com/shop/generated"]] {
                let mut args = vec!["gen"];
                args.extend(t);
                writes.push(case("ritsu", ".", vec![en("ritsu", &args)]));
                writes.push(case("ritsu", ".", vec![ja("ritsu", &args)]));
            }
            writes.push(case("ritsu", ".", vec![en("ritsu", &["gen", "--format", "json"]), en("ritsu", &["gen", "--check"]), en("ritsu", &["gen", "--check", "--format", "json"])]));
            writes.push(case(
                "ritsu",
                ".",
                vec![en("ritsu", &["gen", "--books", "tigerbeetle", "--name", "store", "--out", "pkg"]), ja("ritsu", &["gen", "--books", "tigerbeetle", "--name", "store", "--out", "pkg", "--check"])],
            ));
            // a package made, then changed by hand: --check says so, and the JSON says which
            writes.push(case("ritsu", ".", vec![en("ritsu", &["gen", "--target", "typescript"]), en("ritsu", &["gen", "--target", "typescript", "--check", "--format", "json"])]));
            if gates {
                writes.push(case("ritsu", ".", both("ritsu", &["gen", "--authorizer", "avp"]).to_vec()));
                writes.push(case("ritsu", ".", vec![en("ritsu", &["gen", "--authorizer", "avp", "--format", "json"])]));
            }
            if from.ends_with("invoice") {
                for f in files(from, ".json") {
                    let flow = if f.contains(".ja.") { "invoice.ja.flow" } else { "invoice.flow" };
                    reads.push(case(
                        "ritsu",
                        ".",
                        vec![
                            en("ritsu", &["run", flow, "--scenario", &f]),
                            en("ritsu", &["run", flow, "--scenario", &f, "--format", "json"]),
                            en("ritsu", &["run", flow, "--scenario", &f, "--target", "temporal"]),
                        ],
                    ));
                }
            }
            gs.push(Group { from: from.into(), into: ".".into(), reads, writes });
        }

        // rulec: its corpus is its examples
        {
            let mut reads = Vec::new();
            let mut writes = Vec::new();
            for f in files("crates/rulec/tests/corpus", ".rule") {
                let p = format!("corpus/{f}");
                let p = p.as_str();
                for args in [
                    &["check", p][..],
                    &["check", p, "--format", "json"],
                    &["doc", p],
                    &["doc", p, "--format", "html"],
                    &["doc", p, "--audience", "customer"],
                    &["api", p],
                    &["graph", p],
                    &["certificate", p],
                    &["schema", p],
                    &["vectors", p],
                    &["coverage", p],
                    &["adapter", p],
                    &["fmt", "--check", p],
                ] {
                    reads.push(case("rulec", ".", both("rulec", args).to_vec()));
                }
                writes.push(case("rulec", ".", vec![en("rulec", &["gen", p, "--out", "out"])]));
                writes.push(case("rulec", ".", vec![ja("rulec", &["gen", p, "--out", "out"])]));
            }
            let twins = std::fs::read_to_string(root().join("crates/rulec/tests/corpus/twins.tsv")).unwrap();
            for l in twins.lines().filter(|l| !l.starts_with('#') && l.contains('\t')) {
                let mut it = l.split('\t');
                let (a, b) = (format!("corpus/{}", it.next().unwrap()), format!("corpus/{}", it.next().unwrap()));
                reads.push(case("rulec", ".", both("rulec", &["diff", &a, &b]).to_vec()));
            }
            for code in codes(links, "rulec") {
                reads.push(case("rulec", ".", both("rulec", &["explain", &code]).to_vec()));
            }
            reads.push(case("rulec", ".", vec![en("rulec", &["explain", "--all", "--format", "json"]), ja("rulec", &["explain", "--all", "--format", "markdown"])]));
            gs.push(Group { from: "crates/rulec/tests/corpus".into(), into: "corpus".into(), reads, writes });
        }

        // dandori: every flow, every platform, its scenarios run
        {
            let mut reads = Vec::new();
            let mut writes = Vec::new();
            for f in files("crates/dandori/examples", ".flow") {
                let p = format!("examples/{f}");
                let p = p.as_str();
                for args in [&["check", p][..], &["check", p, "--format", "json"], &["doc", p], &["doc", p, "--format", "html"], &["scenarios", p]] {
                    reads.push(case("dandori", ".", both("dandori", args).to_vec()));
                }
                for t in ["asl", "temporal", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"] {
                    writes.push(case("dandori", ".", vec![en("dandori", &["build", p, "--target", t, "--out", "out"])]));
                    writes.push(case("dandori", ".", vec![ja("dandori", &["build", p, "--target", t, "--out", "out"])]));
                }
                writes.push(Case { of: "dandori", at: ".".into(), steps: vec![en("dandori", &["scenarios", p, "--out", "sc"])], scenarios: Some(("dandori", p.to_string(), "sc".into())) });
                writes.push(Case { of: "ritsu", at: ".".into(), steps: vec![en("dandori", &["scenarios", p, "--out", "sc"])], scenarios: Some(("ritsu", p.to_string(), "sc".into())) });
            }
            for code in codes(links, "dandori") {
                reads.push(case("dandori", ".", both("dandori", &["explain", &code]).to_vec()));
            }
            gs.push(Group { from: "crates/dandori/examples".into(), into: "examples".into(), reads, writes });
        }

        // koyomi
        {
            let mut reads = vec![case("koyomi", ".", both("koyomi", &["check", "examples/"]).to_vec())];
            let mut writes = Vec::new();
            for f in files("crates/koyomi/examples", ".cal") {
                let p = format!("examples/{f}");
                let p = p.as_str();
                for args in [&["check", p][..], &["check", p, "--format", "json"], &["vectors", p], &["doc", p], &["doc", p, "--format", "html"], &["api", p]] {
                    reads.push(case("koyomi", ".", both("koyomi", args).to_vec()));
                }
                writes.push(case("koyomi", ".", vec![en("koyomi", &["gen", p, "--out", "generated"])]));
                writes.push(case("koyomi", ".", vec![ja("koyomi", &["gen", p, "--out", "generated"])]));
                writes.push(case("koyomi", ".", vec![en("koyomi", &["source", "pin", p])]));
            }
            reads.push(case("koyomi", ".", both("koyomi", &["eval", "examples/net30.cal", "invoice_date=2026-03-04"]).to_vec()));
            for code in codes(links, "koyomi") {
                reads.push(case("koyomi", ".", both("koyomi", &["explain", &code]).to_vec()));
            }
            gs.push(Group { from: "crates/koyomi/examples".into(), into: "examples".into(), reads, writes });
        }

        // chobo
        {
            let mut reads = Vec::new();
            let mut writes = Vec::new();
            for f in files("crates/chobo/examples", ".book") {
                let p = format!("examples/{f}");
                let p = p.as_str();
                for args in [&["check", p][..], &["check", p, "--format", "json"], &["doc", p], &["doc", p, "--format", "html"], &["api", p], &["scenarios", p]] {
                    reads.push(case("chobo", ".", both("chobo", args).to_vec()));
                }
                for t in ["postgres", "postgres-typescript", "postgres-python", "postgres-go", "tigerbeetle-typescript", "tigerbeetle-python", "tigerbeetle-go"] {
                    writes.push(case("chobo", ".", vec![en("chobo", &["build", p, "--target", t, "--out", "db"])]));
                }
                writes.push(case("chobo", ".", vec![ja("chobo", &["build", p, "--target", "postgres-typescript", "--out", "db"])]));
                writes.push(Case { of: "chobo", at: ".".into(), steps: vec![en("chobo", &["scenarios", p, "--out", "sc"])], scenarios: Some(("chobo", p.to_string(), "sc".into())) });
            }
            for (book, sc) in [("examples/refunds/refunds.book", "examples/refunds/refunds.more.json"), ("examples/refunds/refunds.ja.book", "examples/refunds/refunds.ja.more.json")] {
                for args in [
                    &["run", book, "--scenario", sc][..],
                    &["run", book, "--scenario", sc, "--format", "json"],
                    &["run", book, "--scenario", sc, "--show", "postgres"],
                    &["run", book, "--scenario", sc, "--show", "tigerbeetle"],
                ] {
                    reads.push(case("chobo", ".", both("chobo", args).to_vec()));
                }
            }
            for code in codes(links, "chobo") {
                reads.push(case("chobo", ".", both("chobo", &["explain", &code]).to_vec()));
            }
            gs.push(Group { from: "crates/chobo/examples".into(), into: "examples".into(), reads, writes });
        }

        // geas: what reads its specs without running a claim (running one starts a program; that is
        // `what_needs_another_program_says_so`)
        {
            let mut reads = vec![
                case("geas", ".", both("geas", &["scenarios", "examples/greeter/greeter.geas", "--openspec", "examples/greeter/openspec/specs"]).to_vec()),
                case("geas", ".", both("geas", &["scenarios", "examples/greeter/greeter.geas", "--openspec", "examples/greeter/openspec/changes/trim-names", "--draft"]).to_vec()),
                case("geas", ".", both("geas", &["scenarios", "examples/greeter/greeter.ja.geas", "--openspec", "examples/greeter/ja/openspec/specs", "--json"]).to_vec()),
                case("geas", ".", both("geas", &["skill"]).to_vec()),
                case("geas", ".", vec![en("geas", &["explain", "--all"]), ja("geas", &["explain", "--all"])]),
            ];
            for code in codes(links, "geas") {
                reads.push(case("geas", ".", both("geas", &["explain", &code]).to_vec()));
            }
            let writes =
                vec![case("geas", ".", vec![en("geas", &["skill", "--install", "skills"]), en("geas", &["skill", "--install", "skills"]), en("geas", &["skill", "--install", "skills", "--force"])])];
            gs.push(Group { from: "crates/geas/examples".into(), into: "examples".into(), reads, writes });
        }

        // yuen: every project of its examples
        {
            let mut reads = Vec::new();
            let mut writes = Vec::new();
            for f in files("crates/yuen/examples", ".req") {
                let dir = format!("examples/{}", f.rsplit_once('/').map(|(d, _)| d).unwrap_or("."));
                let p = format!("examples/{f}");
                let (p, d) = (p.as_str(), dir.as_str());
                for args in [
                    &["check", p, "--root", d][..],
                    &["check", p, "--root", d, "--format", "json"],
                    &["trace", p, "--root", d],
                    &["trace", p, "--root", d, "--format", "json"],
                    &["doc", p, "--root", d],
                    &["doc", p, "--root", d, "--format", "html"],
                    &["api", p, "--root", d],
                    &["export", "reqif", p, "--root", d],
                    &["export", "prov", p, "--root", d, "--time", "2026-10-10T09:00:00Z"],
                    &["export", "prov", p, "--root", d, "--format", "json", "--time", "2026-10-10T09:00:00Z"],
                ] {
                    reads.push(case("yuen", ".", both("yuen", args).to_vec()));
                }
                writes.push(case("yuen", ".", vec![en("yuen", &["review", p, "--root", d, "--all", "--date", "2026-10-10", "--by", "qa"])]));
                // without --date: today, which the package reads from the offset the loader hands over
                writes.push(case("yuen", ".", vec![ja("yuen", &["review", p, "--root", d, "--all", "--by", "qa"])]));
            }
            for (req, root, diff, map) in [
                ("examples/greeter/greeter.req", "examples/greeter", "examples/greeter/changes/change.diff", Some("examples/greeter/greeter.geas=examples/greeter/changes/after.map.jsonl")),
                ("examples/greeter/greeter.ja.req", "examples/greeter", "examples/greeter/changes/change.diff", None),
                ("examples/openspec_greeter/greeter.req", "examples/openspec_greeter", "examples/openspec_greeter/diffs/propose.diff", None),
                ("examples/openspec_greeter/greeter.ja.req", "examples/openspec_greeter", "examples/openspec_greeter/diffs/propose.diff", None),
            ] {
                let mut args = vec!["affected", req, "--root", root, "--diff", diff];
                if let Some(m) = map {
                    args.extend(["--map", m]);
                }
                reads.push(case("yuen", ".", both("yuen", &args).to_vec()));
                let mut json = args.clone();
                json.extend(["--format", "json"]);
                reads.push(case("yuen", ".", vec![en("yuen", &json)]));
                // the diff on the standard input
                let mut piped = args.clone();
                piped[5] = "-";
                let body = std::fs::read(root_of_examples("yuen").join(diff.trim_start_matches("examples/"))).unwrap();
                let mut s = en("yuen", &piped);
                s.stdin = Some(body.clone());
                let mut t = ja("yuen", &piped);
                t.stdin = Some(body);
                reads.push(case("yuen", ".", vec![s, t]));
            }
            // geas's records of the greeter: which claims a diff touches, from a file and from a pipe
            let diff = std::fs::read(root_of_examples("yuen").join("greeter/changes/change.diff")).unwrap();
            reads.push(case("geas", "examples/greeter", both("geas", &["affected", "greeter.geas", "changes/change.diff", "--root", "."]).to_vec()));
            reads.push(case("geas", "examples/greeter", both("geas", &["affected", "greeter.geas", "changes/change.diff", "--root", ".", "--json"]).to_vec()));
            reads.push(case("geas", "examples/greeter", both("geas", &["affected", "greeter.ja.geas", "changes/change.diff", "--root", "."]).to_vec()));
            reads.push(case(
                "geas",
                "examples/greeter",
                both("geas", &["affected", "greeter.geas", "changes/change.diff", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"]).to_vec(),
            ));
            reads.push(case("geas", "examples/greeter", {
                let mut s = en("geas", &["affected", "greeter.geas", "-", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"]);
                s.stdin = Some(diff.clone());
                let mut t = ja("geas", &["affected", "greeter.geas", "-", "--root", ".", "--map", ".geas/greeter.map.jsonl", "--map", "changes/after.map.jsonl"]);
                t.stdin = Some(diff);
                vec![s, t]
            }));
            for code in codes(links, "yuen") {
                reads.push(case("yuen", ".", both("yuen", &["explain", &code]).to_vec()));
            }
            gs.push(Group { from: "crates/yuen/examples".into(), into: "examples".into(), reads, writes });
        }

        // sakai
        {
            let mut reads = Vec::new();
            let mut writes = Vec::new();
            for f in files("crates/sakai/examples", ".ctx").into_iter().filter(|f| !f.contains("/contexts/")) {
                let p = format!("examples/{f}");
                let p = p.as_str();
                for args in [&["check", p][..], &["check", p, "--format", "json"], &["api", p], &["doc", p], &["doc", p, "--format", "html"], &["export", "cml", p]] {
                    reads.push(case("sakai", ".", both("sakai", args).to_vec()));
                }
                for t in ["import-linter", "dependency-cruiser", "archunit", "go-arch-lint"] {
                    reads.push(case("sakai", ".", vec![en("sakai", &["build", p, "--target", t, "--check"])]));
                    writes.push(case("sakai", ".", vec![en("sakai", &["build", p, "--target", t, "--out", "settings"])]));
                    writes.push(case("sakai", ".", vec![ja("sakai", &["build", p, "--target", t])]));
                }
                writes.push(case("sakai", ".", vec![en("sakai", &["doc", p, "--format", "html", "--out", "site"])]));
            }
            reads.push(case(
                "ritsu",
                ".",
                vec![en("ritsu", &["check", "examples/shop"]), en("ritsu", &["check", "examples/webshop", "--format", "json"]), ja("ritsu", &["check", "examples/shop.ja"])],
            ));
            for code in codes(links, "sakai") {
                reads.push(case("sakai", ".", both("sakai", &["explain", &code]).to_vec()));
            }
            gs.push(Group { from: "crates/sakai/examples".into(), into: "examples".into(), reads, writes });
        }

        // sekisho
        {
            let mut reads = vec![case(
                "ritsu",
                ".",
                vec![en("ritsu", &["check", "examples/refunds"]), en("ritsu", &["check", "examples/refunds", "--format", "json"]), ja("ritsu", &["check", "examples/refunds"])],
            )];
            let mut writes = vec![case("ritsu", "examples/refunds", vec![en("ritsu", &["gen"]), en("ritsu", &["gen", "--check", "--format", "json"])])];
            for f in files("crates/sekisho/examples", ".gate") {
                let p = format!("examples/{f}");
                let p = p.as_str();
                let r = "examples/refunds";
                for args in [
                    &["check", p, "--root", r][..],
                    &["check", p, "--root", r, "--format", "json"],
                    &["vectors", p, "--root", r],
                    &["vectors", p, "--root", r, "--action", "refund_order"],
                    &["api", p, "--root", r],
                    &["doc", p, "--root", r],
                    &["doc", p, "--root", r, "--format", "html"],
                ] {
                    reads.push(case("sekisho", ".", both("sekisho", args).to_vec()));
                }
                for t in ["cedar", "typescript", "python", "go"] {
                    writes.push(case("sekisho", ".", vec![en("sekisho", &["gen", p, "--target", t, "--root", r, "--out", "generated"])]));
                    writes.push(case("sekisho", ".", vec![ja("sekisho", &["gen", p, "--target", t, "--root", r, "--out", "generated", "--authorizer", "avp"])]));
                }
            }
            for code in codes(links, "sekisho") {
                reads.push(case("sekisho", ".", both("sekisho", &["explain", &code]).to_vec()));
            }
            gs.push(Group { from: "crates/sekisho/examples".into(), into: "examples".into(), reads, writes });
        }
        gs
    }

    /// A language's examples in the repository.
    fn root_of_examples(lang: &str) -> PathBuf {
        root().join("crates").join(lang).join("examples")
    }

    /// A job of the comparison: one copy that cases only read, or one case that writes.
    enum Job<'a> {
        Reads { from: &'a str, into: &'a str, cases: Vec<Case> },
        Writes { from: &'a str, into: &'a str, case: &'a Case },
    }

    /// Every case of `gs` that `keep` keeps, run by one side (the native binary) and by the other
    /// (the package) in the same copy of its files: the commands counted by language, the
    /// differences, and the seconds it took.
    fn compare(gs: &[Group], keep: &(dyn Fn(&Case) -> bool + Sync), one: &Side, two: &Side) -> (BTreeMap<&'static str, usize>, Vec<String>, u64) {
        let mut jobs: Vec<Job> = Vec::new();
        for g in gs {
            let reads: Vec<Case> = g.reads.iter().filter(|c| keep(c)).cloned().collect();
            for chunk in reads.chunks(24) {
                jobs.push(Job::Reads { from: &g.from, into: &g.into, cases: chunk.to_vec() });
            }
            for c in g.writes.iter().filter(|c| keep(c)) {
                jobs.push(Job::Writes { from: &g.from, into: &g.into, case: c });
            }
        }
        let queue = Mutex::new(jobs.into_iter().collect::<std::collections::VecDeque<_>>());
        let counts: Mutex<BTreeMap<&'static str, usize>> = Mutex::new(BTreeMap::new());
        let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let started = std::time::Instant::now();
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16);
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| {
                    loop {
                        let Some(job) = queue.lock().unwrap().pop_front() else { return };
                        match job {
                            Job::Reads { from, into, cases } => {
                                let copy = copy(from, into);
                                let before = tree(copy.path());
                                for c in &cases {
                                    let a = run_case(c, copy.path(), one);
                                    let b = run_case(c, copy.path(), two);
                                    assert_eq!(a.len(), b.len(), "the two sides ran a different number of commands");
                                    for ((cmd, x), (_, y)) in a.iter().zip(&b) {
                                        *counts.lock().unwrap().entry(c.of).or_default() += 1;
                                        if let Some(d) = differs(cmd, x, y) {
                                            failures.lock().unwrap().push(format!("{from} ({}): {d}", c.at));
                                        }
                                    }
                                }
                                let after = tree(copy.path());
                                if after != before {
                                    let changed: Vec<&String> = after.keys().chain(before.keys()).filter(|k| after.get(*k) != before.get(*k)).collect();
                                    failures.lock().unwrap().push(format!("{from}: commands taken to only read wrote {changed:?}"));
                                }
                            }
                            Job::Writes { from, into, case } => {
                                let first = copy(from, into);
                                let a = run_case(case, first.path(), one);
                                let ta = tree(first.path());
                                drop(first);
                                let second = copy(from, into);
                                let b = run_case(case, second.path(), two);
                                let tb = tree(second.path());
                                for ((cmd, x), (_, y)) in a.iter().zip(&b) {
                                    *counts.lock().unwrap().entry(case.of).or_default() += 1;
                                    if let Some(d) = differs(cmd, x, y) {
                                        failures.lock().unwrap().push(format!("{from} ({}): {d}", case.at));
                                    }
                                }
                                if a.len() != b.len() {
                                    failures.lock().unwrap().push(format!("{from}: the two sides ran {} and {} commands", a.len(), b.len()));
                                }
                                if ta != tb {
                                    let changed: Vec<&String> = ta.keys().chain(tb.keys()).filter(|k| ta.get(*k) != tb.get(*k)).collect();
                                    let cmds: Vec<String> = case.steps.iter().map(Step::shown).collect();
                                    failures.lock().unwrap().push(format!("{from} ({}): `{}` wrote otherwise: {changed:?}", case.at, cmds.join("; ")));
                                }
                            }
                        }
                    }
                });
            }
        });
        (counts.into_inner().unwrap(), failures.into_inner().unwrap(), started.elapsed().as_secs())
    }

    /// What `compare` came to, said and held: no difference.
    fn report(what: &str, counts: &BTreeMap<&'static str, usize>, failures: &[String], secs: u64) {
        let total: usize = counts.values().sum();
        let by: Vec<String> = counts.iter().map(|(k, v)| format!("{k} {v}")).collect();
        println!("npm: {total} commands run by the native binary and by {what}, each held to the other ({}), in {secs} s", by.join(", "));
        assert!(failures.is_empty(), "{} of {total} differ:\n{}", failures.len(), failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn the_package_answers_as_the_binary_does() {
        let t = TempDir::new("npm-package");
        let Some(pkg) = package(&t) else { return };
        let bins = TempDir::new("npm-links");
        let links = links(&bins);
        let gs = groups(&links);
        let n = |s: &Step, at: &Path, _: &Path| native(&links, s.name, &s.args, at, s.stdin.as_deref());
        let w = |s: &Step, at: &Path, _: &Path| npm(&pkg, s.name, &s.args, at, s.stdin.as_deref());
        let (counts, failures, secs) = compare(&gs, &|_| true, &n, &w);
        report("the package", &counts, &failures, secs);
    }

    /// `<args>` of a step as `ritsu` takes them: a language's command after its name.
    fn after_ritsu(s: &Step) -> Vec<String> {
        if s.name == "ritsu" { s.args.clone() } else { std::iter::once(s.name.to_string()).chain(s.args.iter().cloned()).collect() }
    }

    /// The API with `dirs` (DESIGN 8.8): the module opens the copy of the project alone, works in
    /// it, and answers every command a person runs there as the native binary does, the same output,
    /// exit code and files written; the paths it reads, and the root it finds, are inside the copy
    /// (macOS's temporary directory is under /var, a link to /private/var, so the directories above
    /// the copy are ones the module cannot see). Both sides call a language as `ritsu <language> …`,
    /// since the API names no program; `explain`, which reads no file, is left out.
    #[test]
    fn the_api_in_the_dirs_of_a_project_answers_as_the_binary_does() {
        let t = TempDir::new("npm-package");
        let Some(pkg) = package(&t) else { return };
        let bins = TempDir::new("npm-links");
        let links = links(&bins);
        let gs = groups(&links);
        // runSync, opening the directory it is given alone, printing what it answered as a command would
        let helper = bins.write(
            "in-dirs.mjs",
            "import { readFileSync } from \"node:fs\";\nconst [index, dir, ...args] = process.argv.slice(2);\nconst { runSync } = await import(index);\nconst r = runSync(args, { cwd: process.cwd(), dirs: [dir], stdin: readFileSync(0) });\nprocess.stdout.write(r.stdout);\nprocess.stderr.write(r.stderr);\nprocess.exitCode = r.code;\n",
        );
        let index = format!("file://{}", pkg.join("lib/index.js").display());
        let n = |s: &Step, at: &Path, _: &Path| native(&links, "ritsu", &after_ritsu(s), at, s.stdin.as_deref());
        let w = |s: &Step, at: &Path, root: &Path| {
            let mut c = Command::new("node");
            c.arg(&helper).arg(&index).arg(root).args(after_ritsu(s)).current_dir(at);
            quiet(&mut c);
            let r = ritsu_testkit::run::run_with_input(&mut c, s.stdin.as_deref(), LIMIT);
            assert!(!r.timed_out, "`{}` in {} ran past the limit", s.shown(), at.display());
            Said { code: r.code, stdout: r.stdout, stderr: r.stderr }
        };
        let keep = |c: &Case| !c.steps.iter().any(|s| s.args.iter().any(|a| a == "explain"));
        let (counts, failures, secs) = compare(&gs, &keep, &n, &w);
        report("the API opening the project's directory alone", &counts, &failures, secs);
    }

    /// What a command that starts another program or reaches the network says in the package, and
    /// the code it exits with: the language's own head and code, and the sentence of ritsu-base's
    /// `wasi` in place of std's "operation not supported on this platform".
    #[test]
    fn what_needs_another_program_says_so() {
        use ritsu_base::wasi::{cannot_start, cannot_start_programs};
        let t = TempDir::new("npm-package");
        let Some(pkg) = package(&t) else { return };
        let work = TempDir::new("npm-unsupported");
        let w = work.path();
        for (from, into) in [
            ("crates/yuen/examples", "yuen"),
            ("crates/koyomi/examples", "koyomi"),
            ("crates/rulec/tests/corpus", "corpus"),
            ("crates/geas/examples", "geas"),
            ("crates/chobo/examples", "chobo"),
            ("crates/sakai/tests/maps/rust", "sakai"),
        ] {
            ritsu_testkit::tmp::copy_dir(&root().join(from), &w.join(into));
        }
        std::fs::create_dir_all(w.join(".git")).unwrap();
        let made = npm(&pkg, "rulec", &["gen".into(), "corpus/parcel_rate.rule".into(), "--out".into(), "gen".into()], w, None);
        assert_eq!(made.code, Some(0), "{made:?}");
        let curl = cannot_start("curl");
        let git = cannot_start("git");
        let programs = cannot_start_programs();
        let python = cannot_start("python3");
        let cargo = cannot_start("cargo");
        // (where, the command, its exit code, what it says, in Japanese)
        let cases: Vec<(&str, Vec<&str>, i32, &ritsu_base::text::Text, bool)> = vec![
            (".", vec!["yuen", "source", "fetch", "yuen/osha/osha.req", "--root", "yuen/osha"], 2, &curl, false),
            (".", vec!["yuen", "source", "outdated", "yuen/osha/osha.req", "--root", "yuen/osha"], 2, &curl, false),
            (".", vec!["ritsu", "yuen", "source", "fetch", "yuen/osha/osha.req", "--root", "yuen/osha", "--lang", "ja"], 2, &curl, true),
            (".", vec!["koyomi", "source", "fetch", "koyomi/calendars/england_and_wales.cal"], 2, &curl, false),
            (".", vec!["koyomi", "source", "outdated", "koyomi/calendars/england_and_wales.cal"], 2, &curl, false),
            (".", vec!["rulec", "source", "fetch", "corpus/japan_stamp_duty_split.rule"], 2, &curl, false),
            (".", vec!["rulec", "source", "outdated", "corpus/japan_stamp_duty_split.rule"], 2, &curl, false),
            (".", vec!["rulec", "test", "gen"], 2, &programs, false),
            (".", vec!["ritsu", "rulec", "test", "gen", "--lang", "ja"], 2, &programs, true),
            (".", vec!["rulec", "mcp"], 2, &programs, false),
            (".", vec!["rulec", "check", "--diff-base", "HEAD", "corpus/parcel_rate.rule"], 2, &git, false),
            (".", vec!["rulec", "diff", "corpus/parcel_rate.rule@HEAD", "corpus/parcel_rate.rule"], 2, &git, false),
            (".", vec!["rulec", "replay", "corpus/parcel_rate.rule@HEAD"], 2, &git, false),
            (".", vec!["rulec", "verify", "corpus/parcel_rate.rule", "--adapter", "python3", "adapter.py"], 2, &python, false),
            (".", vec!["chobo", "check", "--diff-base", "HEAD", "chobo/refunds/refunds.book"], 2, &git, false),
            (".", vec!["ritsu", "chobo", "check", "--diff-base", "HEAD", "chobo/refunds/refunds.book", "--lang", "ja"], 2, &git, true),
            (".", vec!["geas", "check", "geas/calc/calc.geas"], 1, &python, false),
            (".", vec!["ritsu", "geas", "check", "geas/calc/calc.geas", "--lang", "ja"], 1, &python, true),
            (".", vec!["geas", "check", "--jobs", "3", "geas/greeter/greeter.geas"], 1, &programs, false),
            (".", vec!["geas", "map", "geas/calc/calc.geas", "--root", "geas/calc"], 1, &python, false),
            (".", vec!["geas", "snap", "geas/greeter/greeter.geas"], 1, &programs, false),
            (".", vec!["geas", "drift", "geas/greeter/greeter.geas"], 2, &programs, false),
            ("geas/calc", vec!["ritsu", "check", "."], 1, &python, false),
            ("sakai", vec!["sakai", "check", "shop.ctx"], 1, &cargo, false),
            ("sakai", vec!["ritsu", "sakai", "check", "shop.ctx", "--lang", "ja"], 1, &cargo, true),
            ("sakai", vec!["ritsu", "check", "shop.ctx"], 1, &cargo, false),
        ];
        let mut wrong = Vec::new();
        for (at, cmd, code, said, japanese) in &cases {
            let (name, args) = cmd.split_first().unwrap();
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let s = npm(&pkg, name, &args, &w.join(at), None);
            let both = format!("{}{}", s.stdout, s.stderr);
            // yuen begins its sentences with a capital
            let want = if *japanese { said.ja.clone() } else { said.en[1..].to_string() };
            if s.code != Some(*code) || !both.contains(&want) || both.contains("a bug in ritsu") || both.contains("not supported on this platform") {
                wrong.push(format!("`{}` in {at}: exit {:?} (wanted {code}), said:\n{both}", cmd.join(" "), s.code));
            }
        }
        println!("npm: {} commands that need another program or the network stop, each saying so", cases.len());
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// The bins of the package, each by its name.
    fn bins() -> Vec<(String, String)> {
        std::iter::once("ritsu").chain(LANGUAGES).map(|n| (n.to_string(), format!("bin/{n}.js"))).collect()
    }

    /// packaging/npm/package.json: the workspace's version and the binary's license, the nine bins,
    /// Node 22 and later, ESM with its types, and nothing npm runs when it installs the package.
    #[test]
    fn the_manifest_is_the_workspaces() {
        let dir = root().join("packaging/npm");
        let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
        assert_eq!(m["name"], "@i2y/ritsu");
        assert_eq!(m["version"], env!("CARGO_PKG_VERSION"), "packaging/npm/package.json's version is the workspace's (DESIGN 13.1)");
        let cargo = std::fs::read_to_string(root().join("crates/ritsu/Cargo.toml")).unwrap();
        let license = cargo.lines().find_map(|l| l.strip_prefix("license = ")).unwrap().trim_matches('"');
        assert_eq!(m["license"], license, "the package's license is the binary's");
        assert_eq!(m["type"], "module");
        assert_eq!(m["engines"]["node"], ">=22");
        assert_eq!(m["main"], "./lib/index.js");
        assert_eq!(m["exports"]["."]["types"], "./lib/index.d.ts");
        assert_eq!(m["exports"]["."]["default"], "./lib/index.js");
        let bin: Vec<(String, String)> = m["bin"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        assert_eq!(bin, bins());
        for (name, path) in &bin {
            let text = std::fs::read_to_string(dir.join(path)).unwrap();
            assert!(text.starts_with("#!/usr/bin/env node\n") && text.contains(&format!("main(\"{name}\");")), "{path} runs {name}");
        }
        for key in ["scripts", "dependencies", "devDependencies", "optionalDependencies", "peerDependencies", "bundleDependencies"] {
            assert!(m.get(key).is_none(), "package.json has no {key}: the package runs nothing on install and needs nothing");
        }
        assert_eq!(m["files"], serde_json::json!(["bin/", "lib/", "ritsu.wasm", "LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES"]));
        assert_eq!(m["repository"]["url"], "git+https://github.com/i2y/ritsu.git");
        assert_eq!(m["homepage"], "https://i2y.github.io/ritsu/");
    }

    /// What `npm pack` puts in the package, each file the one in the repository (the module aside),
    /// and a module that names no one's machine.
    #[test]
    fn the_package_holds_what_it_should() {
        let t = TempDir::new("npm-package");
        let Some(pkg) = package(&t) else { return };
        let cache = TempDir::new("npm-cache");
        let out = Command::new("npm").args(["pack", "--dry-run", "--json", "--ignore-scripts", "--cache"]).arg(cache.path()).current_dir(&pkg).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let mut got: Vec<String> = v[0]["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_string()).collect();
        got.sort();
        let mut want: Vec<String> =
            ["LICENSE-APACHE", "LICENSE-MIT", "README.md", "THIRD_PARTY_NOTICES", "package.json", "ritsu.wasm", "lib/cli.js", "lib/index.d.ts", "lib/index.js", "lib/wasi.js", "lib/worker.js"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        want.extend(bins().into_iter().map(|(_, p)| p));
        want.sort();
        assert_eq!(got, want, "the files of the package");
        assert_eq!(v[0]["name"], "@i2y/ritsu");
        assert_eq!(v[0]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(v[0]["filename"], format!("i2y-ritsu-{}.tgz", env!("CARGO_PKG_VERSION")));
        for f in want.iter().filter(|f| *f != "ritsu.wasm") {
            let from = if f.starts_with("LICENSE") || f == "THIRD_PARTY_NOTICES" { root().join(f) } else { root().join("packaging/npm").join(f) };
            assert!(std::fs::read(pkg.join(f)).unwrap() == std::fs::read(&from).unwrap(), "{f} of the package is not {} (built before it changed?)", from.display());
        }
        // the module names no one's machine: not the workspace's place, nor a home, nor Cargo's
        let wasm = std::fs::read(pkg.join("ritsu.wasm")).unwrap();
        let mut local = vec![root().to_string_lossy().to_string()];
        for var in ["HOME", "CARGO_HOME"] {
            if let Some(v) = std::env::var_os(var).filter(|v| v.len() > 1) {
                local.push(v.to_string_lossy().to_string());
            }
        }
        if let Ok(user) = std::env::var("USER") {
            local.extend([format!("/Users/{user}"), format!("/home/{user}")]);
        }
        for l in &local {
            assert!(!wasm.windows(l.len()).any(|w| w == l.as_bytes()), "ritsu.wasm holds {l}");
        }
        let v = npm(&pkg, "ritsu", &["--version".into()], &root(), None);
        assert_eq!(v.stdout, format!("ritsu {}\n", env!("CARGO_PKG_VERSION")));
    }

    /// `node --test packaging/npm/test/*.test.js`: the API and the bins as a program calls them, and the
    /// package installed as a job over many repositories installs one (DESIGN 8.8).
    #[test]
    fn the_javascript_tests_pass() {
        let t = TempDir::new("npm-package");
        let Some(pkg) = package(&t) else { return };
        // a temporary directory of their own: they count what the package leaves in it, and the
        // other tests here run the package beside them
        let tmp = TempDir::new("npm-js-tmp");
        let out = Command::new("node")
            .args(["--test", "--test-reporter=tap", "--test-concurrency=1"])
            // a pattern node expands itself (a directory is taken for a file)
            .arg(root().join("packaging/npm/test/*.test.js"))
            .env("RITSU_NPM_PACKAGE", &pkg)
            .env("RITSU_NPM", tgz())
            .env("TMPDIR", tmp.path())
            .current_dir(root())
            .output()
            .unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        println!("{said}");
        assert!(out.status.success(), "node --test failed");
        assert!(said.contains("\n# fail 0\n") && said.contains("\n# skipped 0\n"), "node --test says it failed none and skipped none");
    }

    /// The pages that show the package at work: its README, and the site's page on Node in English and
    /// in Japanese. A block of commands (`console`) or of JavaScript (`js`) under a line
    /// `<!-- run in <dir> -->` runs in a copy of that directory of the repository, made at the line and
    /// kept for the blocks after it on the page: `npx <name>` is the package's bin, and a `js` block runs
    /// with node, `"@i2y/ritsu"` being the package. What a command prints, and what a `js` block prints
    /// when a `text` block follows it, is held to the page, a line `…` standing for any number of lines.
    #[test]
    fn the_pages_on_node_show_what_the_package_prints() {
        let t = TempDir::new("npm-package");
        let Some(pkg) = package(&t) else { return };
        let shims = TempDir::new("npm-npx");
        let npx = shims.write("npx", format!("#!/bin/sh\nname=\"$1\"; shift\nexec node \"{}/bin/$name.js\" \"$@\"\n", pkg.display()));
        std::fs::set_permissions(&npx, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let index = format!("\"file://{}\"", pkg.join("lib/index.js").display());
        let scripts = TempDir::new("npm-scripts");
        let (mut ran, mut wrong) = (0, Vec::new());
        for name in ["packaging/npm/README.md", "website/docs/node.md", "website/docs-ja/node.md"] {
            let p = common::page(name);
            let marks: Vec<(usize, String)> =
                p.text.lines().enumerate().filter_map(|(i, l)| l.trim().strip_prefix("<!-- run in ").and_then(|r| r.strip_suffix(" -->")).map(|d| (i + 1, d.to_string()))).collect();
            assert!(!marks.is_empty(), "{name} says nowhere where its commands run");
            let mut copy: Option<(usize, TempDir)> = None;
            for (i, b) in p.blocks.iter().enumerate() {
                let Some((at, dir)) = marks.iter().filter(|(l, _)| *l < b.at).next_back() else { continue };
                if copy.as_ref().map(|(l, _)| l) != Some(at) {
                    let c = TempDir::new("npm-pages");
                    ritsu_testkit::tmp::copy_dir(&root().join(dir), c.path());
                    std::fs::create_dir_all(c.path().join(".git")).unwrap();
                    copy = Some((*at, c));
                }
                let cwd = copy.as_ref().unwrap().1.path().to_path_buf();
                match b.info.as_str() {
                    "console" => {
                        for (cmd, shown) in common::runs(b) {
                            let got = common::sh(shims.path(), &cwd, &cmd);
                            ran += 1;
                            if !shown.is_empty() && !common::same(&common::lines_of(&shown.join("\n")), &common::lines_of(&got)) {
                                wrong.push(format!("{name}:{}: $ {cmd}\n--- the page shows\n{}\n--- it prints\n{got}", b.at, shown.join("\n")));
                            }
                        }
                    }
                    "js" => {
                        let file = scripts.write(&format!("block{i}.mjs"), b.lines.join("\n").replace("\"@i2y/ritsu\"", &index));
                        let mut c = Command::new("node");
                        c.arg(&file).current_dir(&cwd).env("TMPDIR", scripts.path());
                        quiet(&mut c);
                        let o = c.output().unwrap();
                        let got = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
                        ran += 1;
                        match p.blocks.get(i + 1).filter(|n| n.info == "text") {
                            Some(shown) if !common::same(&common::lines_of(&shown.lines.join("\n")), &common::lines_of(&got)) => {
                                wrong.push(format!("{name}:{}: the code prints\n{got}\n--- the page shows\n{}", b.at, shown.lines.join("\n")));
                            }
                            None if !o.status.success() => wrong.push(format!("{name}:{}: the code fails:\n{got}", b.at)),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        // the commands of the three pages and their code
        assert!(ran >= 3 * 6, "{ran} blocks and commands run: were the pages or their marks changed?");
        assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    }
}
