//! The playground on the site (website/docs/playground, DESIGN 8.7): the page answers what the
//! `ritsu` binary answers in a directory holding the same files, the module it runs answers what the
//! library answers, and the page starts in Chrome, shows what `ritsu check` prints, lists every
//! project it opens, and opens the links it reads and the links it gives. The shop's gates write
//! Cedar that the official Cedar CLI holds, in both versions of the shop.
//!
//! It took over from the playgrounds rulec's and dandori's sites had (DESIGN 13.2): it opens every
//! example they opened, and their pages (website/rulec/docs/playground.md, website/dandori/docs/
//! playground.md and the Japanese ones) are kept only to send a reader on to it, with the link that
//! reader followed, which it reads as those pages did.
//!
//! Two of its files are committed products: projects.json, the projects the page opens, written
//! from the files they are made of (`RITSU_BLESS=1` writes it anew), and ritsu.wasm, built by
//! website/tools/make_wasm.sh. Both can go stale, and these tests are what says so. The projects
//! are the shop (website/playground/), an empty one, the examples of rulec's playground (one rule
//! each, from rulec's corpus and the table on rulec's front page), the examples of dandori's (each
//! flow with every file it reads, from dandori's examples), chobo's examples (one book each, which
//! chobo's site opens) and sekisho's example (each version of its gate with every file it reads).
//! Node drives the module, Chrome the page (ritsu-testkit's: `RITSU_CHROME`, else where macOS keeps
//! it, else on the PATH), and the Cedar CLI is ritsu-testkit's too (`RITSU_CEDAR`, else on the
//! PATH, at 4.13.0). A test that cannot find what it needs prints `SKIP: ritsu: …` and passes.

use ritsu_base::fs::{Files, Kind, Memory, Meta};
use ritsu_base::naming::Tool;
use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir, need, ready, skip};
use ritsu_wasm::playground::{self, Request};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn site() -> PathBuf {
    repo().join("website/docs/playground")
}

/// A file of the workspace, by its path from the workspace's root.
fn read_repo(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// One project the page opens: the name a link opens it by, the group its list puts it in, the
/// page that lists it (`en` or `ja`; both when None), the file it opens on (none for the empty
/// project), the target the generator of its flow starts on, and its files by their paths in the
/// project. `from` says where the text of each file comes from (what projects.json keys it by),
/// and is empty in what the page has.
#[derive(Clone, Debug, Default)]
struct Project {
    name: String,
    group: String,
    lang: Option<String>,
    open: String,
    target: Option<String>,
    files: Vec<(String, String)>,
    from: Vec<String>,
}

impl Project {
    /// The languages the page answers this project in: those of the pages that list it.
    fn langs(&self) -> Vec<&'static str> {
        match self.lang.as_deref() {
            Some("ja") => vec!["ja"],
            Some(_) => vec!["en"],
            None => vec!["en", "ja"],
        }
    }
}

/// The order the page shows a project's files in: the files of the languages in the order `ritsu
/// check` checks them (DESIGN 6.1), then by path.
fn in_order(files: &mut [(String, String)]) {
    let rank = |p: &str| ritsu_project::project::kind_of(p).and_then(|t| ritsu_project::ORDER.iter().position(|o| *o == t)).unwrap_or(ritsu_project::ORDER.len());
    files.sort_by(|(a, _), (b, _)| (rank(a), a).cmp(&(rank(b), b)));
}

/// Every file under a directory, by its path from it, in the order the page shows them.
fn files_of(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
                out.push((rel, std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))));
            }
        }
    }
    in_order(&mut out);
    out
}

/// The shop the page opens on, in English and then in Japanese (website/playground/<name>/), open
/// on the contract that has just gained a value. Its flow is written for Temporal, and the generator
/// of the flow starts there, as the version of an example of dandori's starts on its platform.
fn shop(name: &str) -> Project {
    let files = files_of(&repo().join("website/playground").join(name));
    let open = "proto/shop/v1/order.proto";
    assert!(files.iter().any(|(p, _)| p == open), "{name} has no {open}");
    let from = files.iter().map(|(p, _)| format!("website/playground/{name}/{p}")).collect();
    Project { name: name.into(), group: "ritsu".into(), open: open.into(), target: Some("temporal".into()), files, from, ..Project::default() }
}

/// The examples of the playground rulec's site had (until its page sent its readers on to this one),
/// one rule each, in the order of its buttons, with the file each comes from in English and in
/// Japanese: the page carried a copy of each, which rulec's tests held to these files. The first two
/// are the table on rulec's front page, its last row taken out and whole. A project's name is the
/// button's (`rulec/gap`, and `rulec/gap.ja` in Japanese), which is where rulec's page sends a
/// reader.
const RULEC: [(&str, &str, &str); 5] = [
    ("gap", "website/rulec/tools/overview.rule", "website/rulec/tools/overview-ja.rule"),
    ("full", "website/rulec/tools/overview.rule", "website/rulec/tools/overview-ja.rule"),
    ("multi", "crates/rulec/tests/corpus/parcel_rate.rule", "crates/rulec/tests/corpus/送料.rule"),
    ("big", "crates/rulec/tests/corpus/Claude利用料.rule", "crates/rulec/tests/corpus/クーポン割引.rule"),
    ("walk", "crates/rulec/tests/corpus/shipment_surcharge.rule", "crates/rulec/tests/corpus/買物かごの送料.rule"),
];

/// The name of a rule's file, as rulec's corpus names it: the rule's name on its first line (`rule
/// 送料(shipping_fee) v4` is in 送料.rule, `rule parcel_rate v1` in parcel_rate.rule).
fn rule_file(text: &str) -> String {
    let name = text.lines().next().and_then(|l| l.strip_prefix("rule ")).and_then(|l| l.split(['(', ' ']).next()).expect("a rule begins with its name");
    format!("{name}.rule")
}

/// One example of rulec's page, as a project of one file: the rule as the page showed it. The
/// front page's table opens with comments about the picture, which the page left out.
fn rulec_example(key: &str, lang: &str, source: &str) -> Project {
    let whole = read_repo(source);
    let (text, from) = match key {
        "gap" | "full" => {
            let full = whole.lines().skip_while(|l| l.starts_with('#') || l.is_empty()).collect::<Vec<_>>().join("\n") + "\n";
            if key == "full" {
                (full, format!("{source}, below its comments"))
            } else {
                let rows: Vec<&str> = full.trim_end().lines().collect();
                (rows[..rows.len() - 1].join("\n") + "\n", format!("{source}, below its comments, without its last row"))
            }
        }
        _ => (whole, source.to_string()),
    };
    let path = rule_file(&text);
    let name = if lang == "ja" { format!("rulec/{key}.ja") } else { format!("rulec/{key}") };
    Project { name, group: "rulec".into(), lang: Some(lang.into()), open: path.clone(), files: vec![(path, text)], from: vec![from], ..Project::default() }
}

/// The root of dandori's crate, from which its examples name each other.
fn dandori_root() -> PathBuf {
    repo().join("crates/dandori")
}

/// dandori's examples in the order dandori's playground listed them, and the versions of each in the
/// order it listed them.
const EXAMPLES: [&str; 7] = ["hotel", "order", "fulfillment", "inquiry", "review", "invoice", "payout"];
const VERSIONS: [&str; 4] = ["temporal", "aws", "pydantic-graph", "argo"];
/// The first draft of the hotel booking dandori's playground opened on, whose check finds errors.
/// Both of its pages opened it; it has no Japanese version.
const DRAFT: &str = "tests/fixtures/hotel_naive.flow";

/// The flows dandori's playground listed (until its page sent its readers on to this one), in the
/// order it listed them, with the platform it built each for: the first draft, then every version of
/// every example and the flows beside the versions. Its Japanese page listed the same draft and the
/// Japanese version of each example (`<name>.ja.flow`). A link it gave names one of these paths
/// (`#flow=examples/hotel/temporal/hotel.flow&view=build`), so this page keeps every one of them.
const DANDORI_PAGE: [(&str, &str); 19] = [
    (DRAFT, "temporal"),
    ("examples/hotel/temporal/hotel.flow", "temporal"),
    ("examples/hotel/aws/hotel.flow", "asl"),
    ("examples/hotel/pydantic-graph/hotel.flow", "pydantic-graph"),
    ("examples/order/temporal/order.flow", "temporal"),
    ("examples/order/aws/order.flow", "asl"),
    ("examples/order/pydantic-graph/order.flow", "pydantic-graph"),
    ("examples/fulfillment/temporal/fulfillment.flow", "temporal"),
    ("examples/fulfillment/aws/fulfillment.flow", "asl"),
    ("examples/fulfillment/pydantic-graph/fulfillment.flow", "pydantic-graph"),
    ("examples/fulfillment/arrange_delivery.flow", "temporal"),
    ("examples/inquiry/temporal/inquiry.flow", "temporal"),
    ("examples/inquiry/aws/inquiry.flow", "asl"),
    ("examples/inquiry/pydantic-graph/inquiry.flow", "pydantic-graph"),
    ("examples/review/temporal/review.flow", "temporal"),
    ("examples/review/aws/review.flow", "durable"),
    ("examples/review/pydantic-graph/review.flow", "pydantic-graph"),
    ("examples/review/argo/review.flow", "argo"),
    ("examples/invoice/invoice.flow", "temporal"),
];

/// The flows the playground lists of dandori's examples for each of its languages, by their paths
/// from dandori's crate, by the rule dandori's playground listed them by: the first draft, then
/// every version of every example, and the flows beside the versions. The Japanese page lists the
/// Japanese versions (`<name>.ja.flow`).
fn dandori_flows(tag: &str) -> Vec<String> {
    let root = dandori_root();
    let mine = |p: &Path| {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        name.ends_with(".flow") && (name.ends_with(".ja.flow") == (tag == "ja"))
    };
    let rel = |p: &Path| p.strip_prefix(&root).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    let mut list = vec![DRAFT.to_string()];
    for ex in EXAMPLES {
        for v in VERSIONS {
            let mut found: Vec<PathBuf> = std::fs::read_dir(root.join(format!("examples/{ex}/{v}"))).map(|rd| rd.map(|e| e.unwrap().path()).filter(|p| mine(p)).collect()).unwrap_or_default();
            found.sort();
            list.extend(found.iter().map(|p| rel(p)));
        }
        let mut beside: Vec<PathBuf> = std::fs::read_dir(root.join(format!("examples/{ex}"))).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_file() && mine(p)).collect();
        beside.sort();
        list.extend(beside.iter().map(|p| rel(p)));
    }
    list
}

/// Where the examples are held in memory while what a flow reads is found.
const EXAMPLES_HOME: &str = "/dandori";

/// Files in memory, with every file read out of them noted, by its path from the working directory
/// (`home`).
struct Noting {
    home: &'static str,
    inner: Memory,
    read: RefCell<BTreeSet<String>>,
}

impl Files for Noting {
    fn read(&self, p: &Path) -> std::io::Result<Vec<u8>> {
        let got = self.inner.read(p)?;
        let abs = ritsu_base::paths::absolute(p);
        if let Ok(rel) = abs.strip_prefix(self.home) {
            self.read.borrow_mut().insert(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"));
        }
        Ok(got)
    }
    fn metadata(&self, p: &Path) -> std::io::Result<Meta> {
        self.inner.metadata(p)
    }
    fn read_dir(&self, p: &Path) -> std::io::Result<Vec<(OsString, Kind)>> {
        self.inner.read_dir(p)
    }
    fn write(&self, p: &Path, bytes: &[u8]) -> std::io::Result<()> {
        self.inner.write(p, bytes)
    }
    fn create_dir_all(&self, p: &Path) -> std::io::Result<()> {
        self.inner.create_dir_all(p)
    }
    fn current_dir(&self) -> std::io::Result<PathBuf> {
        self.inner.current_dir()
    }
    fn canonicalize(&self, p: &Path) -> std::io::Result<PathBuf> {
        self.inner.canonicalize(p)
    }
}

/// What a flow reads of dandori's examples (the flow too), by their paths from dandori's crate:
/// what `ritsu check` reads when it checks the flow, then the files it read with it, and so on until
/// it reads nothing new (a rule's own imports, the calendar a dates file uses), and what `dandori
/// doc` reads to draw it. Every example is held in memory at its path, and nothing else, so what the
/// commands read is what a project of these files needs and no file of the workspace around them.
fn reads_of(flow: &str, every: &[(String, String)]) -> BTreeSet<String> {
    let mut members = BTreeSet::from([flow.to_string()]);
    loop {
        let inner = Memory::new(EXAMPLES_HOME);
        for (p, text) in every {
            inner.add(p, text.as_bytes().to_vec());
        }
        let noting = Rc::new(Noting { home: EXAMPLES_HOME, inner, read: RefCell::default() });
        ritsu_base::fs::with(noting.clone(), || {
            let mut args: Vec<String> = vec!["--root".into(), ".".into()];
            args.extend(members.iter().filter(|p| ritsu_project::project::kind_of(p).is_some()).cloned());
            args.extend(["--lang".to_string(), "en".to_string()]);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            ritsu::check::run(&args, Lang::En, &mut o, &mut e);
            let joined = ritsu_project::Joined::new();
            let doc: Vec<String> = ["doc", flow, "--format", "html", "--lang", "en"].map(String::from).to_vec();
            dandori::cli::run(&doc, joined.rules(), &mut o, &mut e);
        });
        let before = members.len();
        members.extend(noting.read.borrow().iter().cloned());
        if members.len() == before {
            return members;
        }
    }
}

/// The platform a version is written for, which the page builds for when it opens it, as dandori's
/// page did: the one its directory names. A version for AWS is built for Step Functions, unless it
/// asks for what Step Functions cannot run; then for Lambda durable functions.
fn target_for(flow: &str, files: &[(String, String)]) -> &'static str {
    if flow.contains("/aws/") {
        let r = Request { files: files.to_vec(), lang: Lang::En, path: flow.into(), target: Some("asl".into()) };
        if playground::generate(&r)["code"] == 0 { "asl" } else { "durable" }
    } else if flow.contains("/pydantic-graph/") {
        "pydantic-graph"
    } else if flow.contains("/argo/") {
        "argo"
    } else {
        "temporal"
    }
}

/// The examples of dandori's page: each flow it listed, as a project of the flow and every file it
/// reads, at their paths in dandori's crate (so the paths the findings name are the ones dandori's
/// page named), open on the flow and building for its version's platform. The first draft is listed
/// on both pages; every other flow, on the page of its language.
fn dandori_examples() -> Vec<Project> {
    let every = files_of(&dandori_root().join("examples")).into_iter().map(|(p, t)| (format!("examples/{p}"), t)).chain([(DRAFT.to_string(), std::fs::read_to_string(dandori_root().join(DRAFT)).unwrap())]).collect::<Vec<_>>();
    let mut out: Vec<Project> = Vec::new();
    for tag in ["en", "ja"] {
        for flow in dandori_flows(tag) {
            if out.iter().any(|p| p.open == flow) {
                continue;
            }
            let mut files: Vec<(String, String)> = reads_of(&flow, &every).into_iter().map(|p| {
                let text = every.iter().find(|(q, _)| *q == p).map(|(_, t)| t.clone()).unwrap_or_else(|| panic!("{flow} reads {p}, which is not one of dandori's examples"));
                (p, text)
            }).collect();
            in_order(&mut files);
            let from = files.iter().map(|(p, _)| format!("crates/dandori/{p}")).collect();
            let target = target_for(&flow, &files).to_string();
            let lang = (flow != DRAFT).then(|| tag.to_string());
            out.push(Project { name: format!("dandori/{flow}"), group: "dandori".into(), lang, open: flow, target: Some(target), files, from });
        }
    }
    out
}

/// chobo's examples, in the order its README lists them. Each is a project of one book, in English
/// and in its Japanese version (`examples/<name>/<name>.book` and `<name>.ja.book`), at its file's
/// name, listed on the page of its language. A project's name is `chobo/<name>`, and
/// `chobo/<name>.ja` for the Japanese version, as rulec's are; chobo's site opens `chobo/refunds` and
/// `chobo/refunds.ja` from its nav and its home page.
const CHOBO: [&str; 4] = ["inventory", "points", "refunds", "marketplace"];

/// chobo's examples, as projects of the page: every example in English, then every one in Japanese.
fn chobo_examples() -> Vec<Project> {
    let mut out = Vec::new();
    for (lang, tail) in [("en", ""), ("ja", ".ja")] {
        for ex in CHOBO {
            let path = format!("{ex}{tail}.book");
            let from = format!("crates/chobo/examples/{ex}/{path}");
            let files = vec![(path.clone(), read_repo(&from))];
            out.push(Project { name: format!("chobo/{ex}{tail}"), group: "chobo".into(), lang: Some(lang.into()), open: path, files, from: vec![from], ..Project::default() });
        }
    }
    out
}

/// sekisho's example, whose gate reads a rule, a dates file, a calendar and its data, a contract and
/// a flow, each version (English, then Japanese) at its path in the example's directory.
const SEKISHO: &str = "crates/sekisho/examples/refunds";
const GATES: [(&str, &str); 2] = [("en", "refunds.gate"), ("ja", "refunds.ja.gate")];
/// Where the example is held in memory while what a gate reads is found.
const GATES_HOME: &str = "/sekisho";

/// What a gate reads of its example (the gate too): what `ritsu check` reads when it checks the
/// gate, then the files it read with it, until it reads nothing new, and what `sekisho doc` reads
/// to draw it, as [`reads_of`] finds what a flow reads.
fn gate_reads(gate: &str, every: &[(String, String)]) -> BTreeSet<String> {
    let mut members = BTreeSet::from([gate.to_string()]);
    loop {
        let inner = Memory::new(GATES_HOME);
        for (p, text) in every {
            inner.add(p, text.as_bytes().to_vec());
        }
        let noting = Rc::new(Noting { home: GATES_HOME, inner, read: RefCell::default() });
        ritsu_base::fs::with(noting.clone(), || {
            let mut args: Vec<String> = vec!["--root".into(), ".".into()];
            args.extend(members.iter().filter(|p| ritsu_project::project::kind_of(p).is_some()).cloned());
            args.extend(["--lang".to_string(), "en".to_string()]);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            ritsu::check::run(&args, Lang::En, &mut o, &mut e);
            let doc: Vec<String> = ["doc", gate, "--root", ".", "--format", "html", "--lang", "en"].map(String::from).to_vec();
            sekisho::run::run(&doc, ritsu_project::Joined::new().sekisho().into(), &mut o, &mut e);
        });
        let before = members.len();
        members.extend(noting.read.borrow().iter().cloned());
        if members.len() == before {
            return members;
        }
    }
}

/// sekisho's example, as projects of the page: each version of the gate with every file it reads,
/// open on the gate, listed on the page of its language.
fn sekisho_examples() -> Vec<Project> {
    let every = files_of(&repo().join(SEKISHO));
    GATES
        .iter()
        .map(|(lang, gate)| {
            let mut files: Vec<(String, String)> = gate_reads(gate, &every)
                .into_iter()
                .map(|p| {
                    let text = every.iter().find(|(q, _)| *q == p).map(|(_, t)| t.clone()).unwrap_or_else(|| panic!("{gate} reads {p}, which is not a file of sekisho's example"));
                    (p, text)
                })
                .collect();
            in_order(&mut files);
            let from = files.iter().map(|(p, _)| format!("{SEKISHO}/{p}")).collect();
            let name = if *lang == "ja" { "sekisho/refunds.ja" } else { "sekisho/refunds" };
            Project { name: name.into(), group: "sekisho".into(), lang: Some(lang.to_string()), open: gate.to_string(), files, from, ..Project::default() }
        })
        .collect()
}

/// The projects the page opens, in the order its list has them (each page lists those of its
/// language): the shop in English and in Japanese, an empty project to start from one file, the
/// examples of rulec's page in English and then in Japanese, the examples of dandori's page,
/// chobo's examples in English and then in Japanese, and sekisho's example in English and in
/// Japanese.
fn made() -> Vec<Project> {
    let mut out = vec![shop("shop"), shop("shop.ja")];
    out.push(Project { name: "empty".into(), group: "own".into(), ..Project::default() });
    for (i, lang) in ["en", "ja"].into_iter().enumerate() {
        for (key, en, ja) in RULEC {
            out.push(rulec_example(key, lang, if i == 0 { en } else { ja }));
        }
    }
    out.extend(dandori_examples());
    out.extend(chobo_examples());
    out.extend(sekisho_examples());
    out
}

/// projects.json: `{"projects": [{"name", "group", "lang"?, "open", "target"?, "files": [[path,
/// from], ...]}], "texts": {from: text}}`, a file to a line. A text several projects hold (the
/// description of Stripe's API, a rule of an example) is written once.
fn record() -> String {
    let projects = made();
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    let mut s = String::from("{\n  \"projects\": [\n");
    for (i, p) in projects.iter().enumerate() {
        let mut head = vec![format!("\"name\": {}", json!(p.name)), format!("\"group\": {}", json!(p.group))];
        if let Some(l) = &p.lang {
            head.push(format!("\"lang\": {}", json!(l)));
        }
        head.push(format!("\"open\": {}", json!(p.open)));
        if let Some(t) = &p.target {
            head.push(format!("\"target\": {}", json!(t)));
        }
        s.push_str(&format!("    {{\n      {},\n      \"files\": [", head.join(",\n      ")));
        let lines: Vec<String> = p
            .files
            .iter()
            .zip(&p.from)
            .map(|((path, text), from)| {
                if let Some(was) = texts.insert(from.clone(), text.clone()) {
                    assert_eq!(&was, text, "two texts come from {from}");
                }
                format!("        {}", json!([path, from]))
            })
            .collect();
        if lines.is_empty() {
            s.push_str("]\n");
        } else {
            s.push_str(&format!("\n{}\n      ]\n", lines.join(",\n")));
        }
        s.push_str(&format!("    }}{}\n", if i + 1 < projects.len() { "," } else { "" }));
    }
    s.push_str("  ],\n  \"texts\": {\n");
    let lines: Vec<String> = texts.iter().map(|(k, v)| format!("    {}: {}", json!(k), json!(v))).collect();
    s.push_str(&lines.join(",\n"));
    s.push_str("\n  }\n}\n");
    s
}

/// The projects as the page has them (the committed projects.json), each file with its text.
fn committed() -> Vec<Project> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(site().join("projects.json")).expect("website/docs/playground/projects.json")).unwrap();
    let texts = v["texts"].as_object().expect("projects.json has its texts");
    v["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let files = p["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| {
                    let from = f[1].as_str().unwrap();
                    (f[0].as_str().unwrap().to_string(), texts[from].as_str().unwrap_or_else(|| panic!("projects.json has no text from {from}")).to_string())
                })
                .collect();
            Project {
                name: p["name"].as_str().unwrap().into(),
                group: p["group"].as_str().unwrap().into(),
                lang: p["lang"].as_str().map(str::to_string),
                open: p["open"].as_str().unwrap().into(),
                target: p["target"].as_str().map(str::to_string),
                files,
                from: Vec::new(),
            }
        })
        .collect()
}

#[test]
fn the_projects_are_what_the_page_opens() {
    let now = record();
    let file = site().join("projects.json");
    if ritsu_testkit::golden::bless() {
        std::fs::write(&file, &now).unwrap();
        return;
    }
    let was = std::fs::read_to_string(&file).unwrap_or_default();
    assert!(was == now, "website/docs/playground/projects.json is not what website/playground, rulec's corpus, dandori's examples, chobo's examples and sekisho's example hold now; write it anew with RITSU_BLESS=1 cargo test -p ritsu --test playground");
}

/// The quoted words between `start` and the first `end` after it, as dandori's tests read its
/// KEYWORDS, which its crate keeps to itself.
fn quoted(text: &str, start: &str, end: &str) -> Vec<String> {
    let from = text.find(start).unwrap_or_else(|| panic!("no {start:?}")) + start.len();
    let body = &text[from..from + text[from..].find(end).unwrap()];
    body.split('"').skip(1).step_by(2).map(String::from).collect()
}

/// The words of each of ritsu's languages, by the extension of its files, as each language lists
/// the words it reserves: rulec's reserved words with the words that start a line (`kw::RESERVED`,
/// `LINE_HEAD`, `CONTRIBUTION_HEAD`), dandori's `KEYWORDS` (src/syntax.rs), koyomi's
/// `kw::RESERVED`, chobo's `syntax::KEYWORDS`, every word of sekisho's, yuen's and sakai's
/// `kw::TABLE`, and the words of geas's grammar (crates/geas/DESIGN.md, 6), which geas keeps in
/// no table of its code. A keyword of several words (`at least`, `shared kernel with`) gives each
/// of them; a mark (`->`, `sha256:`) gives the word in it, if any. Sorted, each once.
fn language_words() -> BTreeMap<&'static str, Vec<String>> {
    let set = |words: Vec<String>| -> Vec<String> {
        let mut out: BTreeSet<String> = BTreeSet::new();
        for w in words.iter().flat_map(|w| w.split([' ', ':']).map(str::to_string).collect::<Vec<_>>()) {
            if !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && w.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                out.insert(w);
            }
        }
        out.into_iter().collect()
    };
    let strs = |ws: &[&str]| ws.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    let table = |t: &[(&str, &[&str])]| t.iter().flat_map(|(_, ws)| strs(ws)).collect::<Vec<_>>();
    let mut out = BTreeMap::new();
    out.insert("rule", set([strs(rulec::kw::RESERVED), strs(rulec::kw::LINE_HEAD), strs(rulec::kw::CONTRIBUTION_HEAD)].concat()));
    out.insert("flow", set(quoted(&read_repo("crates/dandori/src/syntax.rs"), "const KEYWORDS: &[&str] = &[", "];")));
    out.insert("cal", set(strs(koyomi::kw::RESERVED)));
    out.insert("book", set(chobo::syntax::KEYWORDS.iter().map(|(w, _)| w.to_string()).collect()));
    out.insert("gate", set(table(sekisho::kw::TABLE)));
    out.insert("req", set(table(yuen::kw::TABLE)));
    out.insert("ctx", set(table(sakai::kw::TABLE)));
    let design = read_repo("crates/geas/DESIGN.md");
    let surface = design.split("\n## 6. Surface, v1\n").nth(1).expect("geas's DESIGN.md has its grammar in 6");
    let grammar = surface.split("```").nth(1).expect("the grammar is the first block of 6");
    out.insert("geas", set(grammar.split('"').skip(1).step_by(2).map(String::from).collect()));
    out
}

/// words.json: `{"<extension>": [word, …]}`, a language to a line.
fn words_json() -> String {
    let lines: Vec<String> = language_words().iter().map(|(ext, ws)| format!("  {}: {}", json!(ext), json!(ws))).collect();
    format!("{{\n{}\n}}\n", lines.join(",\n"))
}

/// The editor colours the words of each of the eight languages the page runs (kaname is not on it
/// yet: kaname's DESIGN 19), and they are the words the
/// language itself reserves (`language_words`): words.json, which the page reads, is written from
/// the languages' own lists (`RITSU_BLESS=1`), so a word a language gains or loses is a word the
/// page colours or stops colouring. Each language has words, and words a file of it starts with.
#[test]
fn the_editor_colours_the_words_each_language_reserves() {
    let words = language_words();
    let want: BTreeMap<&str, &str> = [("rule", "rule"), ("flow", "workflow"), ("cal", "calendar"), ("book", "book"), ("gate", "gate"), ("req", "requirements"), ("ctx", "context"), ("geas", "target")].into();
    assert_eq!(words.keys().copied().collect::<Vec<_>>(), want.keys().copied().collect::<Vec<_>>(), "the eight languages, by their extensions");
    for (ext, first) in want {
        assert!(words[ext].iter().any(|w| w == first), "the words of .{ext} have no `{first}`: {:?}", words[ext]);
        assert_eq!(ritsu_project::project::kind_of(&format!("x.{ext}")).map(|t| t.word()), Some(match ext {
            "rule" => "rulec",
            "flow" => "dandori",
            "cal" => "koyomi",
            "book" => "chobo",
            "gate" => "sekisho",
            "req" => "yuen",
            "ctx" => "sakai",
            _ => "geas",
        }), ".{ext} is not the language the words are of");
    }
    // geas keeps its words in the grammar of its DESIGN.md: each is one its parser reads, or a state
    // of a node, which it reads where it reads a node (src/screen.rs)
    let parser = format!("{}{}", read_repo("crates/geas/src/parse.rs"), read_repo("crates/geas/src/screen.rs"));
    for w in &words["geas"] {
        assert!(parser.contains(&format!("\"{w}\"")), "geas's grammar has `{w}`, which its code does not read");
    }
    let now = words_json();
    let file = site().join("words.json");
    if ritsu_testkit::golden::bless() {
        std::fs::write(&file, &now).unwrap();
        return;
    }
    let was = std::fs::read_to_string(&file).unwrap_or_default();
    assert!(was == now, "website/docs/playground/words.json is not the words the languages reserve now; write it anew with RITSU_BLESS=1 cargo test -p ritsu --test playground");
    eprintln!("compared: the words of {} languages ({} words) with the lists of the languages", words.len(), words.values().map(Vec::len).sum::<usize>());
}

/// The page that explains the playground and the page of the playground alone hold one component, in
/// each language: the explaining page writes the component's element and loads its script; the page
/// alone loads the same stylesheet and script from beside the explaining page (`../playground/`),
/// opens on a small rule of rulec's examples listed in its language, which draws a page for people,
/// and is the other language's page but for its words. sync.sh copies every file of the component to
/// the Japanese site, and the component names nothing to load from elsewhere: its only import is the
/// libraries bundled beside it.
#[test]
fn the_two_pages_hold_one_component() {
    let projects = committed();
    let mut shapes = Vec::new();
    for (tag, explaining, alone) in [("en", "website/docs/playground.md", "website/docs/play/index.html"), ("ja", "website/docs-ja/playground.md", "website/docs-ja/play/index.html")] {
        let md = read_repo(explaining);
        assert_eq!(md.matches("<div class=\"pg\"").count(), 1, "{explaining} does not hold the component once");
        for want in [format!("<div class=\"pg\" data-lang=\"{tag}\"></div>"), "<script type=\"module\" src=\"playground/playground.js\"></script>".to_string()] {
            assert!(md.contains(&want), "{explaining} has no {want}");
        }
        let html = read_repo(alone);
        for want in [
            format!("<html lang=\"{tag}\">"),
            "<link rel=\"stylesheet\" href=\"../playground/playground.css\">".to_string(),
            "<script type=\"module\" src=\"../playground/playground.js\"></script>".to_string(),
            "<body class=\"pg-page\">".to_string(),
        ] {
            assert!(html.contains(&want), "{alone} has no {want}");
        }
        let element = html.split("<div class=\"pg pg-full\"").nth(1).and_then(|r| r.split('>').next()).unwrap_or_else(|| panic!("{alone} has no component"));
        assert!(element.contains(&format!(" data-lang=\"{tag}\"")) && element.contains(" data-view=\"doc\""), "{alone}: the component is not in its language, on the page for people");
        let name = element.split(" data-project=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or_else(|| panic!("{alone} names no project to open on"));
        let p = projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("{alone} opens on {name}, which projects.json does not have"));
        assert_eq!((p.group.as_str(), p.lang.as_deref(), p.files.len()), ("rulec", Some(tag), 1), "{alone} does not open on one rule of rulec's examples in its language");
        assert_eq!(playground::doc(&ask_in(tag, &p.files, &p.open, None))["html"]["code"], 0, "{name} draws no page for people");
        shapes.push(html.split('<').skip(1).map(|t| t.split([' ', '>', '\n']).next().unwrap().to_string()).collect::<Vec<_>>());
    }
    assert_eq!(shapes[0], shapes[1], "the two pages alone are not one page in two languages");
    let sync = read_repo("website/sync.sh");
    let copied: BTreeSet<String> = sync.split_whitespace().filter_map(|w| w.strip_prefix("docs/playground/")).map(String::from).collect();
    let files: BTreeSet<String> = std::fs::read_dir(site()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect();
    assert_eq!(copied, files, "website/sync.sh copies other files than website/docs/playground holds to the Japanese site");
    for f in ["playground.js", "worker.js", "playground.css"] {
        let text = std::fs::read_to_string(site().join(f)).unwrap();
        assert!(!text.contains("https://") && !text.contains("http://"), "website/docs/playground/{f} names a URL; the page loads nothing from elsewhere");
    }
    let js = std::fs::read_to_string(site().join("playground.js")).unwrap();
    let imports: Vec<&str> = js.lines().filter(|l| l.starts_with("import ")).collect();
    assert_eq!(imports, ["import * as cm from \"./libraries.js\";"], "playground.js imports more than the libraries bundled beside it");
}

/// The libraries the page loads (libraries.js, website/tools/make_libraries.sh) hold the packages
/// they say they hold, at the versions website/tools/libraries/package-lock.json pins, and none for
/// development; package.json asks for each package at one version, and the lock pins it there; every
/// package the page's code depends on is in the file; the text of each license is there, every
/// package under one heading; THIRD_PARTY_NOTICES gives the same list and the same texts; and the
/// packages npm installs are not committed.
#[test]
fn the_libraries_are_what_the_lock_pins() {
    let lib = std::fs::read_to_string(site().join("libraries.js")).unwrap();
    let header = lib.strip_prefix("/*!\n").and_then(|r| r.split_once("\n*/\n")).map(|(h, _)| h).expect("libraries.js begins with its notice");
    let body = &header[header.find("\n\n  ").expect("the notice lists the packages") + 2..];
    let listed: Vec<(String, String, String)> = body
        .lines()
        .take_while(|l| l.starts_with("  "))
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            (w[0].to_string(), w[1].to_string(), w[2].to_string())
        })
        .collect();
    assert!(listed.iter().any(|(n, _, _)| n == "@codemirror/view") && listed.iter().any(|(n, _, _)| n == "marked"), "libraries.js lists no CodeMirror or marked: {listed:?}");
    let lock: Value = serde_json::from_str(&read_repo("website/tools/libraries/package-lock.json")).unwrap();
    let manifest: Value = serde_json::from_str(&read_repo("website/tools/libraries/package.json")).unwrap();
    for deps in ["dependencies", "devDependencies"] {
        for (name, v) in manifest[deps].as_object().unwrap() {
            let v = v.as_str().unwrap();
            assert!(v.split('.').count() == 3 && v.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())), "package.json asks for {name} at `{v}`, not at one version");
            assert_eq!(lock["packages"][format!("node_modules/{name}")]["version"], v, "the lock pins {name} otherwise than package.json asks");
        }
    }
    for (name, version, license) in &listed {
        let p = &lock["packages"][format!("node_modules/{name}")];
        assert_eq!(p["version"], version.as_str(), "libraries.js holds {name} {version}, which the lock does not pin");
        assert!(p["dev"].is_null(), "libraries.js holds {name}, which is for development");
        assert_eq!(p["license"], license.as_str(), "libraries.js says {name} is under {license}, and the lock otherwise");
    }
    for name in manifest["dependencies"].as_object().unwrap().keys() {
        assert!(listed.iter().any(|(n, _, _)| n == name), "libraries.js does not hold {name}, which the page's code depends on");
    }
    let mut carried: Vec<String> = body.lines().filter_map(|l| l.strip_prefix("---- ")).flat_map(|l| l.split(", ").map(|c| c.split(" (").next().unwrap().to_string()).collect::<Vec<_>>()).collect();
    carried.sort();
    let mut names: Vec<String> = listed.iter().map(|(n, _, _)| n.clone()).collect();
    names.sort();
    assert_eq!(carried, names, "the texts of the licenses in libraries.js are not each package's, once");
    let notices = read_repo("THIRD_PARTY_NOTICES");
    let rule = "-".repeat(80);
    let section = notices.split(&format!("\n{rule}\nCodeMirror 6 and marked\n{rule}\n\n")).nth(1).expect("THIRD_PARTY_NOTICES has no section for the libraries of the page");
    let text = section.split(&format!("\n{rule}\n")).next().unwrap().split_once("\n\n").map(|(_, t)| t).expect("the section has a head, then the text");
    assert!(text.trim_end() == body.trim_end(), "THIRD_PARTY_NOTICES gives another list or other texts than libraries.js: {}", ritsu_testkit::golden::line_diff(body.trim_end(), text.trim_end()));
    assert!(read_repo("website/.gitignore").lines().any(|l| l == "tools/libraries/node_modules/"), "website/.gitignore does not leave out the packages npm installs for make_libraries.sh");
    eprintln!("compared: {} packages of libraries.js with the lock and the notices", listed.len());
}

/// The page opens every example the playgrounds of rulec's and dandori's sites opened, as they
/// opened it (their pages send their readers on here: `the_pages_before_send_their_links_on`):
/// rulec's five rules in both languages, in the order of its buttons, each word for word as that
/// page carried it, held here to the files it was copied from (the table on rulec's front page below
/// its comments about the picture, whole and without its last row, and three rules of rulec's corpus
/// as they are); and every flow dandori's playground listed, in the order it listed them, on the
/// platform it built each for. Every flow of dandori's examples is a project too.
#[test]
fn every_example_of_the_pages_before_is_a_project() {
    let projects = committed();
    let find = |name: &str| projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("projects.json has no {name}"));
    let mut looked = 0;
    let listed: Vec<&str> = projects.iter().filter(|p| p.group == "rulec").map(|p| p.name.as_str()).collect();
    let buttons: Vec<String> = ["", ".ja"].iter().flat_map(|tail| RULEC.iter().map(move |(key, _, _)| format!("rulec/{key}{tail}"))).collect();
    assert_eq!(listed, buttons, "the page lists other rules of rulec's than rulec's playground offered, or in another order");
    // the table on the front page, as the page showed it: below the comments about the picture
    let table = |rel: &str| read_repo(rel).lines().skip_while(|l| l.starts_with('#') || l.is_empty()).collect::<Vec<_>>().join("\n") + "\n";
    for (i, lang) in ["en", "ja"].into_iter().enumerate() {
        for (key, en, ja) in RULEC {
            let source = if i == 0 { en } else { ja };
            let want = match key {
                "full" => table(source),
                "gap" => {
                    let full = table(source);
                    let rows: Vec<&str> = full.trim_end().lines().collect();
                    rows[..rows.len() - 1].join("\n") + "\n"
                }
                _ => read_repo(source),
            };
            let name = if lang == "ja" { format!("rulec/{key}.ja") } else { format!("rulec/{key}") };
            let p = find(&name);
            assert_eq!(p.files.len(), 1, "{name} is one rule");
            assert_eq!(p.files[0].1, want, "{name} is not the rule rulec's playground showed, {source} as it is now");
            assert_eq!((p.lang.as_deref(), p.open.as_str()), (Some(lang), p.files[0].0.as_str()), "{name} is not listed on the {lang} page, open on its rule");
            looked += 1;
        }
    }
    for tag in ["en", "ja"] {
        let theirs: Vec<(String, String)> = DANDORI_PAGE
            .iter()
            .map(|(path, target)| (if tag == "ja" && *path != DRAFT { path.replace(".flow", ".ja.flow") } else { path.to_string() }, target.to_string()))
            .collect();
        let mine: Vec<(String, String)> = projects.iter().filter(|p| p.group == "dandori" && p.lang.as_deref().is_none_or(|l| l == tag)).map(|p| (p.open.clone(), p.target.clone().unwrap())).collect();
        // what dandori's playground listed, in its order, among what this page lists
        let kept: Vec<(String, String)> = mine.iter().filter(|(open, _)| theirs.iter().any(|(path, _)| path == open)).cloned().collect();
        assert_eq!(kept, theirs, "the {tag} page does not list every flow dandori's playground listed, in its order, built for the platform it built it for");
        looked += theirs.len();
    }
    let mut stack = vec![dandori_root().join("examples")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "flow") {
                let rel = p.strip_prefix(dandori_root()).unwrap().to_string_lossy().replace('\\', "/");
                assert!(projects.iter().any(|q| q.group == "dandori" && q.open == rel), "{rel} is not among the projects the page opens");
            }
        }
    }
    eprintln!("compared: {looked} examples of the playgrounds of rulec and dandori with the projects the page opens");
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[String]) -> (u8, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1) as u8, String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// A directory holding the project's files.
fn written_out(files: &[(String, String)]) -> TempDir {
    let t = TempDir::new("playground");
    for (p, text) in files {
        t.write(p, text);
    }
    t
}

/// The files in `dir` that are not the project's, by their paths from it.
fn new_files(dir: &Path, files: &[(String, String)]) -> Vec<(String, String)> {
    let mut all = files_of(dir);
    all.retain(|(p, _)| !files.iter().any(|(q, _)| q == p));
    all.sort();
    all
}

/// The words of the command the page says it ran, with the language it ran in.
fn words_of(command: &str, lang: &str) -> Vec<String> {
    let mut w: Vec<String> = command.strip_prefix("ritsu ").expect("a command of ritsu").split_whitespace().map(str::to_string).collect();
    if !w.iter().any(|x| x == "--lang") {
        w.push("--lang".into());
        w.push(lang.into());
    }
    w
}

/// One of what the page answered, held to the binary run as the page says, in a directory holding
/// the same files: the exit code, what it prints, and (for a generator) what it writes.
fn holds(files: &[(String, String)], lang: &str, what: &str, answer: &Value, failures: &mut Vec<String>) {
    let dir = written_out(files);
    let (code, out, err) = ritsu_in(dir.path(), &words_of(answer["command"].as_str().unwrap(), lang));
    let got = (answer["code"].as_u64().unwrap() as u8, answer["out"].as_str().unwrap(), answer["err"].as_str().unwrap());
    if (code, out.as_str(), err.as_str()) != got {
        failures.push(format!("{what} ({lang}): `{}`\n--- the command (exit {code})\n{out}{err}\n--- the page (exit {})\n{}{}", answer["command"], got.0, got.1, got.2));
    }
    if let Some(written) = answer["files"].as_array() {
        let mine: Vec<(String, String)> = written.iter().map(|f| (f["path"].as_str().unwrap().to_string(), f["body"].as_str().unwrap().to_string())).collect();
        let mut sorted = mine.clone();
        sorted.sort();
        let theirs = new_files(dir.path(), files);
        if sorted != theirs {
            failures.push(format!("{what} ({lang}): the command wrote {} file(s), the page {}", theirs.len(), mine.len()));
        }
    }
}

/// The requests a project's page makes: `ritsu check`, and for every file its generator (with
/// every target the generator takes) and its page. The files go in with [`files_json`].
fn requests(files: &[(String, String)], lang: &str) -> Vec<(&'static str, Value)> {
    let base = |path: &str, target: Option<&str>| json!({ "files": {}, "lang": lang, "path": path, "target": target });
    let mut out = vec![("check", base("", None))];
    for (p, _) in files {
        let targets: Vec<&str> = match ritsu_project::project::kind_of(p) {
            Some(Tool::Dandori) => vec!["asl", "temporal", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"],
            Some(Tool::Chobo) => vec!["postgres", "postgres-typescript", "postgres-python", "postgres-go", "tigerbeetle-typescript", "tigerbeetle-python", "tigerbeetle-go", "mysql", "mysql-typescript", "mysql-python", "mysql-go"],
            Some(Tool::Yuen) => vec!["reqif", "prov"],
            _ => vec![],
        };
        if targets.is_empty() {
            out.push(("gen", base(p, None)));
        }
        for t in targets {
            out.push(("gen", base(p, Some(t))));
        }
        out.push(("doc", base(p, None)));
    }
    out
}

fn files_json(files: &[(String, String)]) -> Value {
    Value::Object(files.iter().map(|(p, t)| (p.clone(), Value::String(t.clone()))).collect())
}

/// Edits a reader might make, each reaching a way the page answers that the projects as they open
/// do not. In the shop: the contract put right, the rule given the value instead (one row short,
/// then whole), a rule's output renamed under the flow that reads it, the orders' gate's permit for
/// the customers let to cancel a paid order (an expectation of the gate's broken, and a
/// requirement's link to the permit no longer as it was looked at), the warehouse's gate's permit
/// for the workflow without the operation the workflow reads an order by (each call of it in the
/// flow allowed in no combination, X16's E908, and a requirement's link to the permit no longer as
/// it was looked at), a flow that does not parse, a file of no language added, and a geas spec
/// that does not parse (a spec that parses runs programs, which the page cannot: it is not held to
/// the binary). Then the steps the playgrounds of
/// rulec and dandori had the reader take (a band widened over the next one, a rounding taken off,
/// a failure that hands the case over, an arm of a `match` taken out), one file of the reader's own
/// in the empty project, and what those playgrounds were held to beyond their examples: rules of
/// rulec's that reach checks the examples do not, and flows of dandori's that name what their
/// project does not have.
fn edits() -> Vec<(&'static str, &'static str, Vec<(String, String)>)> {
    let mut out = Vec::new();
    let projects = committed();
    for p in projects.iter().filter(|p| p.group == "ritsu") {
        let (name, files) = (p.name.as_str(), p.files.clone());
        let swap = |files: &[(String, String)], path: &str, from: &str, to: &str| -> Vec<(String, String)> {
            let mut f = files.to_vec();
            let x = f.iter_mut().find(|(p, _)| p == path).unwrap_or_else(|| panic!("{name} has no {path}"));
            assert!(x.1.contains(from), "{path} no longer has `{from}`");
            x.1 = x.1.replacen(from, to, 1);
            f
        };
        let (billing, urgency, flow) = if name == "shop" {
            ("billing/rules/billing_need.rule", "ordering/rules/urgency.rule", "ordering/ship_order.flow")
        } else {
            ("billing/rules/請求の要否.rule", "ordering/rules/出荷の急ぎ.rule", "ordering/出荷.flow")
        };
        let lang = if name == "shop" { "en" } else { "ja" };
        let fixed = swap(&files, "proto/shop/v1/order.proto", "  ORDER_STATUS_RETURNED = 5;\n", "");
        out.push(("the contract put right", lang, fixed.clone()));
        let (enum_from, enum_to, row_from, row_to) = if name == "shop" {
            ("| shipped | cancelled", "| shipped | cancelled | returned", "| cancelled | skip        |\n", "| cancelled | skip        |\n| returned  | skip        |\n")
        } else {
            ("| 取消(cancelled)", "| 取消(cancelled) | 返品(returned)", "| 取消   | 請求しない  |\n", "| 取消   | 請求しない  |\n| 返品   | 請求しない  |\n")
        };
        let given = swap(&files, billing, enum_from, enum_to);
        out.push(("the rule given the value, a row short", lang, given.clone()));
        out.push(("the rule given the value and its row", lang, swap(&given, billing, row_from, row_to)));
        // the output renamed in the rule, its table and its examples: rulec passes, dandori does not
        let renames: [(&str, &str); 2] = if name == "shop" {
            [("  carrier : carrier\n", "  courier : carrier\n"), ("| -> urgent | carrier  |", "| -> urgent | courier  |")]
        } else {
            [("  便(carrier)  : 便\n", "  運び方(courier) : 便\n"), ("| -> 急ぎ | 便     |", "| -> 急ぎ | 運び方 |")]
        };
        let mut renamed = fixed.clone();
        for (from, to) in renames {
            let x = renamed.iter_mut().find(|(p, _)| p == urgency).unwrap();
            assert!(x.1.contains(from), "{urgency} no longer has `{from}`");
            x.1 = x.1.replace(from, to);
        }
        out.push(("a rule's output renamed under the flow", lang, renamed));
        // the customers' permit let to cancel a paid order: sekisho's expectation does not hold,
        // and yuen's link to the permit is no longer as it was looked at
        let (gate, from, to) = if name == "shop" {
            ("gates/orders.gate", "  when cancellable\n  unless resource.state is paid\n\npermit clerks_cancel_unpaid_orders\n", "  when cancellable\n\npermit clerks_cancel_unpaid_orders\n")
        } else {
            ("gates/注文.gate", "  when 取り消せる\n  unless resource.状態 is 入金済\n\npermit 係は入金前の注文を取り消せる", "  when 取り消せる\n\npermit 係は入金前の注文を取り消せる")
        };
        out.push(("a customer let to cancel a paid order", lang, swap(&fixed, gate, from, to)));
        // the warehouse's gate takes the reading of an order away from the workflow: the gate
        // passes (the pickers read the orders), ritsu says the flow's calls of the operation are
        // allowed in no combination, and yuen's link to the permit is no longer as it was looked at
        let (gate, from, to) = if name == "shop" {
            ("gates/warehouse.gate", "  action view_order, request_shipment, request_cancellation\n", "  action request_shipment, request_cancellation\n")
        } else {
            ("gates/倉庫.gate", "  action 注文を見る, 出荷を頼む, 取消を頼む\n", "  action 出荷を頼む, 取消を頼む\n")
        };
        out.push(("an operation taken away from the workflow", lang, swap(&fixed, gate, from, to)));
        out.push(("a flow that does not parse", lang, swap(&fixed, flow, "flow\n", "flow =\n")));
        let mut more = fixed.clone();
        more.push(("notes.txt".into(), "what the shop has yet to decide\n".into()));
        out.push(("a file of no language", lang, more));
        // geas reads a spec the page holds, and stops at its syntax before any program starts
        let mut spec = fixed.clone();
        spec.push(("checks/greeter.geas".into(), "target greeter {\n  run \"python3 greeter.py\"\n}\n\nclaim \"greets\" {\n".into()));
        out.push(("a spec that does not parse", lang, spec));
    }
    let edited = |name: &str, path: &str, from: &str, to: &str| -> Vec<(String, String)> {
        let p = projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("projects.json has no {name}"));
        let mut f = p.files.clone();
        let x = f.iter_mut().find(|(q, _)| q == path).unwrap_or_else(|| panic!("{name} has no {path}"));
        assert!(x.1.contains(from), "{path} no longer has `{from}`");
        x.1 = x.1.replacen(from, to, 1);
        f
    };
    for (name, lang, rule, rounding) in [("rulec/full", "en", "fee.rule", "  round up(1USD)"), ("rulec/full.ja", "ja", "運賃.rule", "  round up(10円)")] {
        out.push(("a band of the table widened over the next", lang, edited(name, rule, "<=2kg", "<=6kg")));
        out.push(("the rounding taken off the output", lang, edited(name, rule, rounding, "")));
    }
    let draft = format!("dandori/{DRAFT}");
    for lang in ["en", "ja"] {
        out.push(("the declined card handed over", lang, edited(&draft, DRAFT, "fail CardDeclined \"The card was declined\"", "fail CardDeclined \"The card was declined\" leaving pi")));
    }
    let (en, ja) = ("examples/hotel/temporal/hotel.flow", "examples/hotel/temporal/hotel.ja.flow");
    out.push(("an arm of the match taken out", "en", edited(&format!("dandori/{en}"), en, "    canceled => fail PaymentCanceled \"The PaymentIntent was canceled\"\n", "")));
    out.push(("an arm of the match taken out", "ja", edited(&format!("dandori/{ja}"), ja, "    canceled => fail 決済の取消 \"PaymentIntent が取り消されていました\"\n", "")));
    let rule_of = |name: &str| projects.iter().find(|p| p.name == name).unwrap().files[0].1.clone();
    out.push(("one file of the reader's own", "en", vec![("fee.rule".to_string(), rule_of("rulec/full"))]));
    out.push(("one file of the reader's own", "ja", vec![("運賃.rule".to_string(), rule_of("rulec/full.ja"))]));
    // rulec's: a rule whose answer the elimination decides, and one whose answer it cannot (a module
    // that answers none of them cannot be told stale from fresh), and a finding on a row with ℃
    // before it (where the carets go depends on how wide ℃ is counted)
    let corpus = |name: &str| read_repo(&format!("crates/rulec/tests/corpus/{name}"));
    for (lang, rule) in [("en", "coupon_stacking.rule"), ("ja", "クーポン併用.rule")] {
        out.push(("a rule whose answer the elimination decides", lang, vec![(rule.to_string(), corpus(rule))]));
    }
    let undecided = read_repo("crates/rulec/tests/mutants/m_w114.rule");
    for lang in ["en", "ja"] {
        out.push(("a rule whose answer the elimination cannot decide", lang, vec![("m_w114.rule".to_string(), undecided.clone())]));
    }
    for (lang, rule, row) in [("en", "food_storage_standard.rule", "| frozen  | <=-15℃      | true "), ("ja", "保存基準.rule", "| 冷凍     | <=-15℃      | true ")] {
        let text = corpus(rule);
        assert_eq!(text.matches(row).count(), 1, "{rule} no longer has the row `{row}`");
        out.push(("a finding on a row with ℃ before it", lang, vec![(rule.to_string(), text.replace(row, &row.replace("true ", "yes  ")))]));
    }
    // dandori's: a rule and a child flow the project does not have, a flow that runs itself, and a
    // flow of the reader's own beside the first draft
    for (lang, flow, rule, missing) in [
        ("en", "examples/hotel/temporal/hotel.flow", "\"../rules/hold_amount.rule\"", "\"../rules/deposit.rule\""),
        ("ja", "examples/hotel/temporal/hotel.ja.flow", "\"../rules/宿泊の与信額.rule\"", "\"../rules/預かり金.rule\""),
    ] {
        out.push(("a rule the project does not have", lang, edited(&format!("dandori/{flow}"), flow, rule, missing)));
    }
    for (lang, flow, child, missing) in [
        ("en", "examples/fulfillment/temporal/fulfillment.flow", "flow \"../arrange_delivery.flow\"", "flow \"../arrange.flow\""),
        ("ja", "examples/fulfillment/temporal/fulfillment.ja.flow", "flow \"../arrange_delivery.ja.flow\"", "flow \"../arrange.ja.flow\""),
    ] {
        let itself = format!("flow \"{}\"", flow.rsplit('/').next().unwrap());
        out.push(("a flow that runs itself", lang, edited(&format!("dandori/{flow}"), flow, child, &itself)));
        out.push(("a child flow the project does not have", lang, edited(&format!("dandori/{flow}"), flow, child, missing)));
    }
    let draft = projects.iter().find(|p| p.name == format!("dandori/{DRAFT}")).expect("projects.json has the first draft").files.clone();
    for lang in ["en", "ja"] {
        let mut more = draft.clone();
        more.push(("scratch/new.flow".to_string(), "workflow new v1\n\ninputs\n  n : int\n\nflow\n  pass\n".to_string()));
        out.push(("a flow of the reader's own beside the first draft", lang, more));
    }
    out
}

/// The page answers as the binary does: for every project, in the language of each page that lists
/// it, and every edit, `ritsu check` (its text and its JSON), and every file's generator (with every
/// target it takes) and page. The jobs are shared among as many threads as the machine has cores.
#[test]
fn the_page_answers_as_the_command_does() {
    let mut jobs: Vec<(String, &'static str, Vec<(String, String)>)> = Vec::new();
    for p in committed() {
        for lang in p.langs() {
            jobs.push((format!("{} as it opens", p.name), lang, p.files.clone()));
        }
    }
    for (what, lang, files) in edits() {
        jobs.push((what.to_string(), lang, files));
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let width = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let (failures, n) = std::thread::scope(|s| {
        let handles: Vec<_> = (0..width)
            .map(|_| {
                s.spawn(|| {
                    let (mut failures, mut n) = (Vec::new(), 0);
                    while let Some((what, lang, files)) = jobs.get(next.fetch_add(1, std::sync::atomic::Ordering::SeqCst)) {
                        n += compare(what, lang, files, &mut failures);
                    }
                    (failures, n)
                })
            })
            .collect();
        let (mut all, mut n) = (Vec::new(), 0);
        for h in handles {
            let (f, k) = h.join().unwrap();
            all.extend(f);
            n += k;
        }
        (all, n)
    });
    eprintln!("compared: {n} answers of the page with what the binary prints and writes, on {} projects and edits", jobs.len());
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every request of one project's page, each held to the binary; how many answers were compared.
fn compare(what: &str, lang: &str, files: &[(String, String)], failures: &mut Vec<String>) -> usize {
    let mut n = 0;
    for (kind, req) in requests(files, lang) {
        let mut req = req;
        req["files"] = files_json(files);
        let r = Request::from_json(&req).unwrap();
        let path = r.path.clone();
        let label = format!("{what}: {kind} {path}");
        match kind {
            "check" => {
                let a = playground::check(&r);
                holds(files, lang, &label, &a, failures);
                let dir = written_out(files);
                let (_, out, _) = ritsu_in(dir.path(), &words_of("ritsu check . --format json", lang));
                let want: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
                if want != a["json"] {
                    failures.push(format!("{label} ({lang}): the JSON differs"));
                }
                n += 2;
            }
            "gen" => {
                let a = playground::generate(&r);
                if a["none"] != true {
                    holds(files, lang, &label, &a, failures);
                    n += 1;
                }
            }
            _ => {
                let a = playground::doc(&r);
                if a["none"] != true {
                    holds(files, lang, &format!("{label} (html)"), &a["html"], failures);
                    holds(files, lang, &format!("{label} (markdown)"), &a["markdown"], failures);
                    n += 2;
                }
            }
        }
    }
    n
}

/// The shop's gates (sekisho's DESIGN 6.1), the orders' and the warehouse's, in English and in
/// Japanese: the Cedar `sekisho gen --target cedar` writes from each is what the official CLI
/// (ritsu-testkit's, at 4.13.0) validates strictly, in its text and in its JSON, without a warning,
/// and lays out as it is written (`format --check`); and the CLI answers every combination the
/// check walks as sekisho does (`sekisho vectors`, through `run-tests`: the decision, and the
/// policies that decide it). The two versions of a gate write the same Cedar but for the names and
/// the descriptions: `@name`, `@doc`, the lines of the header, and the names of the book and of its
/// transfers in the references of `@guards`, which the Japanese version writes in Japanese
/// (`chobo "warehouse/在庫.book" transfer 引当 operation hold`).
#[test]
fn the_shop_s_gates_are_cedar_the_official_cli_holds() {
    let Some(cli) = ritsu_testkit::cedar::cli() else {
        return;
    };
    // what the CLI prints, its colours taken out
    let cedar = |dir: &Path, args: &[&str]| -> (bool, String) {
        let o = Command::new(&cli).current_dir(dir).args(args).env("CEDAR_ERROR_FORMAT", "plain").env("NO_COLOR", "1").output().expect("could not run the Cedar CLI");
        let said = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let mut plain = String::new();
        let mut cs = said.chars();
        while let Some(c) = cs.next() {
            if c == '\u{1b}' {
                cs.by_ref().find(|x| x.is_ascii_alphabetic());
            } else {
                plain.push(c);
            }
        }
        (o.status.success(), plain)
    };
    let words = |s: &[&str]| -> Vec<String> { s.iter().map(|x| x.to_string()).collect() };
    let mut versions: Vec<Vec<Value>> = Vec::new();
    // each version's two gates, with the alias each writes its Cedar under
    let shops = [("shop", "en", [("gates/orders.gate", "orders"), ("gates/warehouse.gate", "warehouse")]), ("shop.ja", "ja", [("gates/注文.gate", "orders"), ("gates/倉庫.gate", "warehouse")])];
    // the names the Japanese version gives the book and its transfers, in the references of
    // `@guards`, with the English version's
    let book = [("warehouse/在庫.book", "warehouse/inventory.book"), ("transfer 入荷 ", "transfer receive "), ("transfer 返品 ", "transfer take_back "), ("transfer 引当 ", "transfer reserve ")];
    let english = |text: &str| -> String { book.iter().fold(text.to_string(), |t, (ja, en)| t.replace(ja, en)) };
    // the files, but for the names, the descriptions and the header
    let bare = |text: &str| -> Value { Value::String(english(&text.lines().filter(|l| !l.starts_with("// ") && !l.trim_start().starts_with("@doc(") && !l.trim_start().starts_with("@name(")).collect::<Vec<_>>().join("\n"))) };
    fn unnamed(v: &mut Value, english: &dyn Fn(&str) -> String) {
        match v {
            Value::Object(m) => {
                if let Some(Value::Object(a)) = m.get_mut("annotations") {
                    a.remove("doc");
                    a.remove("name");
                    // what had only those has nothing left: one version writes none of them
                    if a.is_empty() {
                        m.remove("annotations");
                    }
                }
                m.values_mut().for_each(|x| unnamed(x, english));
            }
            Value::Array(xs) => xs.iter_mut().for_each(|x| unnamed(x, english)),
            Value::String(s) => *s = english(s),
            _ => {}
        }
    }
    for (name, lang, gates) in shops {
        let dir = written_out(&files_of(&repo().join("website/playground").join(name)));
        let at = dir.path().join("generated/cedar");
        let mut files = Vec::new();
        for (gate, alias) in gates {
            let (code, out, err) = ritsu_in(dir.path(), &words(&["sekisho", "gen", gate, "--target", "cedar", "--root", ".", "--out", "generated", "--lang", lang]));
            assert_eq!(code, 0, "{name}: {out}{err}");
            let (code, vectors, err) = ritsu_in(dir.path(), &words(&["sekisho", "vectors", gate, "--root", "."]));
            assert_eq!(code, 0, "{name}: {err}");
            let tests = serde_json::from_str::<Value>(&vectors).unwrap().as_array().map_or(0, Vec::len);
            let cases = format!("{alias}.vectors.json");
            std::fs::write(at.join(&cases), &vectors).unwrap();
            let [text, schema_text, schema_json, policies_json] = [".cedar", ".cedarschema", ".cedarschema.json", ".policies.json"].map(|x| format!("{alias}{x}"));
            for json in [false, true] {
                let (schema, policies) = if json { (schema_json.as_str(), policies_json.as_str()) } else { (schema_text.as_str(), text.as_str()) };
                let formats = if json { vec!["--schema-format", "json", "--policy-format", "json"] } else { vec![] };
                let mut args = vec!["validate", "--schema", schema, "--policies", policies, "--validation-mode", "strict", "--deny-warnings"];
                args.extend(&formats);
                let (ok, said) = cedar(&at, &args);
                assert!(ok && said.contains("no errors or warnings"), "{name}: {gate}: cedar {}\n{said}", args.join(" "));
                let mut args = vec!["run-tests", "--policies", policies, "--schema", schema, "--tests", cases.as_str()];
                args.extend(&formats);
                let (ok, said) = cedar(&at, &args);
                assert!(ok && said.lines().any(|l| l == format!("results: {tests} passed, 0 failed")), "{name}: {gate}: cedar {}\n{said}", args.join(" "));
            }
            let written = std::fs::read_to_string(at.join(&text)).unwrap();
            let (ok, laid) = cedar(&at, &["format", "--check", "--policies", text.as_str()]);
            assert!(ok && laid == written, "{name}: {gate}: cedar format --check lays the policies out otherwise:\n{}", ritsu_testkit::golden::line_diff(&written, &laid));
            for f in [&text, &schema_text, &schema_json, &policies_json] {
                let body = std::fs::read_to_string(at.join(f)).unwrap_or_else(|e| panic!("{name}: {f}: {e}"));
                files.push(if f.ends_with(".json") {
                    let mut v: Value = serde_json::from_str(&body).unwrap();
                    unnamed(&mut v, &english);
                    v
                } else {
                    bare(&body)
                });
            }
            eprintln!("compared: {name}: the Cedar of {gate} validated in both of its formats, and {tests} combinations answered alike by the Cedar CLI");
        }
        versions.push(files);
    }
    assert_eq!(versions[0], versions[1], "the shop's two versions of its gates write other Cedar than for the names and the descriptions");
}

/// The page's side of the boundary: allocate, write, call, read the length out of the header,
/// free. It is here rather than in a committed file because it is a test's: playground.js is the
/// copy that ships, and if the two ever disagree about the convention, this one fails.
const DRIVER: &str = r#"
import { readFileSync, writeFileSync } from "node:fs";
const [wasmPath, requestsPath, outPath] = process.argv.slice(2);
const inst = await WebAssembly.instantiate(await WebAssembly.compile(readFileSync(wasmPath)), {});
const e = inst.exports;
function put(text) {
  const bytes = Buffer.from(text, "utf8");
  const ptr = e.ritsu_alloc(bytes.length);
  new Uint8Array(e.memory.buffer, ptr + 4, bytes.length).set(bytes);
  return ptr;
}
function take(ptr) {
  const len = new DataView(e.memory.buffer).getUint32(ptr, true);
  const text = Buffer.from(new Uint8Array(e.memory.buffer, ptr + 4, len)).toString("utf8");
  e.ritsu_free(ptr);
  return text;
}
function call(fn, text) {
  const ptr = put(text);
  try {
    return take(e[fn](ptr));
  } finally {
    e.ritsu_free(ptr);
  }
}
const version = take(e.ritsu_version());
const imports = WebAssembly.Module.imports(await WebAssembly.compile(readFileSync(wasmPath))).length;
const answers = JSON.parse(readFileSync(requestsPath, "utf8")).map(([what, req]) => call("ritsu_" + what, req));
writeFileSync(outPath, JSON.stringify({ version, imports, answers }));
"#;

fn node_available() -> bool {
    Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The module answers as the library does: every request of the comparison above for the shop, for
/// every other project `check` and the generator (on the target it opens on) and the page of the
/// file it opens on, `check` on every edit and every request on an edit of one file (the reader's
/// own, and the rules of rulec's that reach what the examples do not), and a few the page can be
/// made to send by hand (a path outside the project, a request that is not JSON).
#[test]
fn the_module_answers_as_the_library_does() {
    let wasm = site().join("ritsu.wasm");
    if !ready(Need::Node, || node_available() && wasm.exists(), "node or website/docs/playground/ritsu.wasm is missing") {
        return;
    }
    let mut requests: Vec<(String, String)> = Vec::new();
    for p in committed() {
        for lang in p.langs() {
            for (kind, mut req) in requests_of(&p.files, lang) {
                let opened = req["path"] == p.open.as_str() && (req["target"].is_null() || req["target"].as_str() == p.target.as_deref());
                if p.group != "ritsu" && kind != "check" && !opened {
                    continue;
                }
                req["files"] = files_json(&p.files);
                requests.push((kind.to_string(), req.to_string()));
            }
        }
    }
    for (_, lang, files) in edits() {
        if files.len() == 1 {
            for (kind, mut req) in requests_of(&files, lang) {
                req["files"] = files_json(&files);
                requests.push((kind.to_string(), req.to_string()));
            }
            continue;
        }
        let mut req = json!({ "lang": lang, "path": "" });
        req["files"] = files_json(&files);
        requests.push(("check".into(), req.to_string()));
    }
    requests.push(("check".into(), json!({ "files": { "../outside.rule": "" }, "lang": "en" }).to_string()));
    requests.push(("gen".into(), "not a request".into()));

    let tmp = TempDir::new("wasm");
    let dir = tmp.path();
    std::fs::write(dir.join("driver.mjs"), DRIVER).unwrap();
    std::fs::write(dir.join("requests.json"), serde_json::to_string(&requests).unwrap()).unwrap();
    let o = Command::new("node")
        .current_dir(dir)
        .args(["driver.mjs", wasm.to_str().unwrap(), "requests.json", "answers.json"])
        .output()
        .expect("could not run node");
    assert!(o.status.success(), "the driver failed: {}", String::from_utf8_lossy(&o.stderr));
    let got: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("answers.json")).unwrap()).unwrap();

    assert_eq!(got["version"], env!("CARGO_PKG_VERSION"), "website/docs/playground/ritsu.wasm is of another version; run website/tools/make_wasm.sh");
    assert_eq!(got["imports"], 0, "the module asks the page for imports; the page gives it none");
    let answers = got["answers"].as_array().unwrap();
    assert_eq!(answers.len(), requests.len());
    let mut stale = Vec::new();
    for ((what, req), a) in requests.iter().zip(answers) {
        let want = playground::answer(what, req);
        if a.as_str() != Some(want.as_str()) {
            let r: Value = serde_json::from_str(req).unwrap_or(Value::Null);
            stale.push(format!("{what} {} ({}) {}", r["path"], r["lang"], r["target"]));
            if stale.len() == 1 {
                let pretty = |s: &str| serde_json::from_str::<Value>(s).map(|v| serde_json::to_string_pretty(&v).unwrap()).unwrap_or_else(|_| s.to_string());
                stale.push(ritsu_testkit::golden::line_diff(&pretty(&want), &pretty(a.as_str().unwrap_or_default())));
            }
        }
    }
    assert!(stale.is_empty(), "website/docs/playground/ritsu.wasm answers otherwise than the library now; run website/tools/make_wasm.sh:\n{}", stale.join("\n"));
    eprintln!("compared: {} requests answered by the module as by the library", requests.len());
}

fn requests_of(files: &[(String, String)], lang: &str) -> Vec<(&'static str, Value)> {
    requests(files, lang)
}

/// Chrome, when the level lets the test run it and the machine has it, with node, which drives it,
/// and the module the page loads; a SKIP line says why not.
fn chrome() -> Option<PathBuf> {
    if !need(Need::Chrome) {
        return None;
    }
    let Some(found) = ritsu_testkit::chrome::find() else {
        skip("Chrome is not found; set RITSU_CHROME to run this test");
        return None;
    };
    if !node_available() {
        skip("node is not found; it drives Chrome in this test");
        return None;
    }
    if !site().join("ritsu.wasm").exists() {
        skip("website/docs/playground/ritsu.wasm is missing; run website/tools/make_wasm.sh");
        return None;
    }
    Some(found)
}

/// Where the site publishes the playground's files, in each language (sync.sh copies them to the
/// Japanese site): the page that explains the playground is published at the same path, and the page
/// of the playground alone loads them from beside it (`../playground/`).
const PUBLISHED: [&str; 2] = ["/ritsu/playground/", "/ritsu/ja/playground/"];

/// A server for the pages, each at its path, and the playground's files where the site publishes
/// them.
fn serve(pages: Vec<(String, String)>) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let pages = std::sync::Arc::new(pages);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let pages = pages.clone();
            std::thread::spawn(move || respond(stream, &pages));
        }
    });
    port
}

fn respond(mut s: std::net::TcpStream, pages: &[(String, String)]) {
    let mut buf = [0u8; 8192];
    let n = s.read(&mut buf).unwrap_or(0);
    let head = String::from_utf8_lossy(&buf[..n]);
    let asked = head.split_whitespace().nth(1).unwrap_or("/").split(['?', '#']).next().unwrap_or("/").to_string();
    let found = if let Some((_, page)) = pages.iter().find(|(at, _)| *at == asked) {
        Some((page.as_bytes().to_vec(), "text/html; charset=utf-8"))
    } else {
        let kind = match asked.rsplit('.').next() {
            Some("js") => "text/javascript",
            Some("css") => "text/css",
            Some("wasm") => "application/wasm",
            Some("json") => "application/json",
            _ => "application/octet-stream",
        };
        PUBLISHED
            .iter()
            .find_map(|at| asked.strip_prefix(at))
            .filter(|f| !f.is_empty() && !f.contains(".."))
            .and_then(|f| std::fs::read(site().join(f)).ok())
            .map(|b| (b, kind))
    };
    let (status, body, kind) = match found {
        Some((b, k)) => ("200 OK", b, k),
        None => ("404 Not Found", Vec::new(), "text/plain"),
    };
    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    let _ = s.write_all(&body);
}

/// The page that explains the playground, in a language, as the site publishes it at
/// /ritsu/playground/ (and /ritsu/ja/playground/): the component and its script as the Markdown
/// writes them, in a page whose body carries the scheme as Zensical marks it. Zensical publishes the
/// page a directory deeper than its file and writes the script's path from there
/// (tests/website.rs holds the built page to it), which is the directory of the playground's files.
fn explaining(page: &str) -> String {
    let md = read_repo(page);
    let from = md.find("<div class=\"pg\"").unwrap_or_else(|| panic!("{page} has no playground"));
    let tail = "<script type=\"module\" src=\"playground/playground.js\"></script>";
    let to = md[from..].find(tail).unwrap_or_else(|| panic!("{page} does not load playground.js")) + from + tail.len();
    let widget = md[from..to].replace("src=\"playground/playground.js\"", "src=\"playground.js\"");
    format!("<!doctype html><html><head><meta charset=\"utf-8\"></head><body data-md-color-scheme=\"default\">{widget}</body></html>")
}

/// The four pages of the playground where the site publishes them: the two that explain it and the
/// two of the playground alone, as they are committed, and `more`.
fn playground_pages(more: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut out = vec![
        ("/ritsu/playground/".to_string(), explaining("website/docs/playground.md")),
        ("/ritsu/ja/playground/".to_string(), explaining("website/docs-ja/playground.md")),
        ("/ritsu/play/".to_string(), read_repo("website/docs/play/index.html")),
        ("/ritsu/ja/play/".to_string(), read_repo("website/docs-ja/play/index.html")),
    ];
    out.extend(more);
    out
}

/// Drives Chrome over its DevTools protocol, on the pipe `--remote-debugging-pipe` opens (fds 3 and
/// 4): `node cdp.mjs <chrome> <profile> <visits.json> <out.json>`. Each visit opens a tab of the
/// given size, in the given scheme, goes to the page and takes its steps: `wait` until an
/// expression is true (in real time: the playground answers from a worker), `run` a script, `get`
/// a value (what comes back, in order), `key` press a key, `type` text, `click` the middle of what a
/// selector names, `wheel` turn the mouse's wheel at a place in it, `sleep`, `shot` a picture. A step
/// that
/// fails ends the visit, and the errors, with what the page threw, come back with the values.
const CDP: &str = r#"
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
const [chromePath, profile, visitsPath, outPath] = process.argv.slice(2);
const visits = JSON.parse(readFileSync(visitsPath, "utf8"));
const chrome = spawn(chromePath, ["--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check", "--remote-debugging-pipe", `--user-data-dir=${profile}`, "about:blank"], { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] });
const [toChrome, fromChrome] = [chrome.stdio[3], chrome.stdio[4]];
let nextId = 1;
const waiting = new Map();
const events = [];
let buf = Buffer.alloc(0);
fromChrome.on("data", (chunk) => {
  buf = Buffer.concat([buf, chunk]);
  let at;
  while ((at = buf.indexOf(0)) >= 0) {
    const msg = JSON.parse(buf.subarray(0, at).toString("utf8"));
    buf = buf.subarray(at + 1);
    if (msg.id && waiting.has(msg.id)) {
      const { ok, no } = waiting.get(msg.id);
      waiting.delete(msg.id);
      msg.error ? no(new Error(JSON.stringify(msg.error))) : ok(msg.result);
    } else if (msg.method) events.push(msg);
  }
});
const send = (method, params = {}, sessionId) => new Promise((ok, no) => {
  const id = nextId++;
  waiting.set(id, { ok, no });
  toChrome.write(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }) + "\0");
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function evaluate(s, expression) {
  const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }, s);
  if (r.exceptionDetails) throw new Error(`${expression.slice(0, 160)}: ${(r.exceptionDetails.exception && r.exceptionDetails.exception.description) || r.exceptionDetails.text}`);
  return r.result.value;
}
const out = [];
try {
  for (const visit of visits) {
    const { targetId } = await send("Target.createTarget", { url: "about:blank" });
    const { sessionId: s } = await send("Target.attachToTarget", { targetId, flatten: true });
    await send("Page.enable", {}, s);
    await send("Runtime.enable", {}, s);
    await send("Network.enable", {}, s);
    await send("Emulation.setDeviceMetricsOverride", { width: visit.width, height: visit.height, deviceScaleFactor: 1, mobile: false }, s);
    await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: visit.dark ? "dark" : "light" }] }, s);
    const got = { values: [], errors: [] };
    try {
      await send("Page.navigate", { url: visit.url }, s);
      for (const step of visit.steps) {
        if (step.wait) {
          const until = Date.now() + 60000;
          let ok = false;
          while (Date.now() < until) {
            try { if (await evaluate(s, step.wait)) { ok = true; break; } } catch (e) {}
            await sleep(40);
          }
          if (!ok) throw new Error(`waited 60 s for ${step.wait.slice(0, 200)}`);
        } else if (step.run) await evaluate(s, `(async () => { ${step.run} })()`);
        else if (step.get) got.values.push(await evaluate(s, step.get));
        else if (step.key) {
          for (const type of ["keyDown", "keyUp"]) await send("Input.dispatchKeyEvent", { type, key: step.key, code: step.key, windowsVirtualKeyCode: { ArrowRight: 39, ArrowLeft: 37, Home: 36, End: 35, Enter: 13 }[step.key] || 0 }, s);
        } else if (step.type) await send("Input.insertText", { text: step.type }, s);
        else if (step.click) {
          const at = await evaluate(s, `(() => { const r = document.querySelector(${JSON.stringify(step.click)}).getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()`);
          for (const type of ["mousePressed", "mouseReleased"]) await send("Input.dispatchMouseEvent", { type, x: at[0], y: at[1], button: "left", clickCount: 1 }, s);
        } else if (step.wheel) {
          const at = await evaluate(s, `(() => { const r = document.querySelector(${JSON.stringify(step.wheel)}).getBoundingClientRect(); return [r.left + r.width * ${step.at[0]}, r.top + r.height * ${step.at[1]}]; })()`);
          await send("Input.dispatchMouseEvent", { type: "mouseWheel", x: at[0], y: at[1], deltaX: 0, deltaY: step.dy }, s);
        } else if (step.sleep) await sleep(step.sleep);
        else if (step.shot) {
          const r = await send("Page.captureScreenshot", { format: "png" }, s);
          writeFileSync(step.shot, Buffer.from(r.data, "base64"));
        }
      }
    } catch (e) {
      got.errors.push(String(e.message || e));
    }
    got.thrown = events.filter((e) => e.sessionId === s && e.method === "Runtime.exceptionThrown").map((e) => JSON.stringify(e.params.exceptionDetails).slice(0, 400));
    got.requests = events.filter((e) => e.sessionId === s && e.method === "Network.requestWillBeSent").map((e) => e.params.request.url);
    out.push(got);
    await send("Target.closeTarget", { targetId });
  }
} finally {
  writeFileSync(outPath, JSON.stringify(out));
  await Promise.race([send("Browser.close").catch(() => {}), sleep(2000)]);
  chrome.kill("SIGKILL");
  process.exit(0);
}
"#;

/// Until the playground has answered for what it shows: nothing is asked of the module or waiting
/// to be (`aria-busy`), and the status says how long the last answer took.
const READY: &str = r#"(() => { const r = document.querySelector(".pg"); const s = r && r.querySelector(".pg-status"); return !!(r && s && r.getAttribute("aria-busy") !== "true" && / ms/.test(s.textContent)); })()"#;

/// What the playground shows, as a reader sees it: the project and the file open (by the tab
/// selected, which is there even when a project of one file hides the tabs), the file's text in the
/// editor, the view, the tabs marked, the list of projects, the status, what the panel of the check
/// or of the generator prints, the problems and their count, the page for people as its frame holds
/// it, whether it is faded, the band under it, the Markdown, the line the cursor is on, and where
/// the page is.
const STATE: &str = r#"(async () => {
  const r = document.querySelector(".pg");
  const lib = new URL("libraries.js", document.querySelector('script[src$="playground.js"]').src).href;
  const { EditorView } = await import(lib);
  const v = EditorView.findFromDOM(r.querySelector(".cm-editor"));
  const tab = r.querySelector('.pg-tab[aria-selected="true"]');
  const view = tab ? tab.dataset.view : null;
  const said = view === "check" || view === "gen" ? r.querySelector(`.pg-panel[data-view="${view}"] .pg-said`).textContent : null;
  const frame = r.querySelector('.pg-panel[data-view="doc"] .pg-stage iframe:not(.pg-coming)');
  const band = r.querySelector(".pg-band");
  const save = r.querySelector(".pg-save");
  const open = r.querySelector('.pg-file[aria-selected="true"]');
  return {
    status: r.querySelector(".pg-status").textContent,
    project: (r.querySelector(".pg-project").selectedOptions[0] || { dataset: {} }).dataset.name || null,
    open: open ? open.dataset.path : null,
    text: v.state.doc.toString(),
    line: v.state.doc.lineAt(v.state.selection.main.head).number,
    view,
    marked: [...r.querySelectorAll(".pg-file.err")].map((b) => b.dataset.path),
    listed: [...r.querySelectorAll(".pg-project option")].map((o) => o.dataset.name),
    groups: r.querySelectorAll(".pg-project optgroup").length,
    said,
    problems: [...r.querySelectorAll(".pg-problem")].map((b) => b.textContent),
    count: r.querySelector(".pg-count").textContent,
    page: frame ? frame.srcdoc : null,
    stale: r.querySelector('.pg-panel[data-view="doc"] .pg-stage').classList.contains("pg-stale"),
    markdownStale: r.querySelector('.pg-panel[data-view="markdown"] .pg-stage').classList.contains("pg-stale"),
    band: band.hidden ? null : band.textContent,
    markdown: r.querySelector(".pg-mdtext").textContent,
    revert: !r.querySelector(".pg-revert").hidden,
    cta: r.querySelector(".pg-add").classList.contains("pg-cta"),
    tabs: getComputedStyle(r.querySelector(".pg-files")).display !== "none",
    save: save.href.startsWith("blob:") ? save.download : null,
    focused: document.activeElement && document.activeElement.dataset ? document.activeElement.dataset.view || null : null,
    path: location.pathname,
    hash: location.hash,
  };
})()"#;

/// A script that edits the open file as a reader would, by the editor (CodeMirror's own
/// transaction, which is what typing makes): the first `from` in it becomes `to`.
fn edit(from: &str, to: &str) -> String {
    format!(
        r#"const lib = new URL("libraries.js", document.querySelector('script[src$="playground.js"]').src).href;
const {{ EditorView }} = await import(lib);
const v = EditorView.findFromDOM(document.querySelector(".pg .cm-editor"));
const at = v.state.doc.toString().indexOf({from});
if (at < 0) throw new Error("the file has no " + {from});
v.dispatch({{ changes: {{ from: at, to: at + {from}.length, insert: {to} }} }});"#,
        from = json!(from),
        to = json!(to)
    )
}

/// A script that presses a button of the playground, by its class (or a selector).
fn press(selector: &str) -> String {
    format!("document.querySelector({}).click();", json!(selector))
}

/// One visit of Chrome to a page ([`drive`]): the size of the window, the scheme of the system, and
/// the steps it takes.
struct Visit {
    url: String,
    width: u32,
    height: u32,
    dark: bool,
    steps: Vec<Value>,
}

impl Visit {
    fn to(url: impl Into<String>) -> Visit {
        Visit { url: url.into(), width: 1280, height: 900, dark: false, steps: Vec::new() }
    }
    fn step(mut self, s: Value) -> Visit {
        self.steps.push(s);
        self
    }
    fn ready(self) -> Visit {
        self.step(json!({ "wait": READY }))
    }
    fn wait(self, js: &str) -> Visit {
        self.step(json!({ "wait": js }))
    }
    fn run(self, js: &str) -> Visit {
        self.step(json!({ "run": js }))
    }
    fn get(self, js: &str) -> Visit {
        self.step(json!({ "get": js }))
    }
    fn state(self) -> Visit {
        self.get(STATE)
    }
    fn key(self, key: &str) -> Visit {
        self.step(json!({ "key": key }))
    }
    /// A click, as a mouse makes it, in the middle of what `selector` names.
    fn click(self, selector: &str) -> Visit {
        self.step(json!({ "click": selector }))
    }
    /// The mouse's wheel turned `dy` down, at `(x, y)` of what `selector` names (from its top left,
    /// as fractions of its width and height).
    fn wheel(self, selector: &str, at: (f64, f64), dy: i64) -> Visit {
        self.step(json!({ "wheel": selector, "at": [at.0, at.1], "dy": dy }))
    }
    fn sleep(self, ms: u64) -> Visit {
        self.step(json!({ "sleep": ms }))
    }
    /// An edit of the open file, and the playground's answer to it.
    fn edited(self, from: &str, to: &str) -> Visit {
        self.run(&edit(from, to)).ready()
    }
    /// A button pressed, and the playground's answer.
    fn pressed(self, selector: &str) -> Visit {
        self.run(&press(selector)).ready()
    }
}

/// What each visit read, in order. A visit whose step failed, or whose page threw, fails the test
/// with what Chrome said; so does one whose page asked for anything from anywhere but the test's
/// server (the page loads nothing from elsewhere: its libraries are bundled beside it).
fn drive(chrome: &Path, visits: Vec<Visit>) -> Vec<Vec<Value>> {
    let t = TempDir::new("cdp");
    let dir = t.path();
    std::fs::write(dir.join("cdp.mjs"), CDP).unwrap();
    let plan: Vec<Value> = visits.iter().map(|v| json!({ "url": v.url, "width": v.width, "height": v.height, "dark": v.dark, "steps": v.steps })).collect();
    std::fs::write(dir.join("visits.json"), serde_json::to_string(&plan).unwrap()).unwrap();
    std::fs::create_dir_all(dir.join("profile")).unwrap();
    let o = Command::new("node")
        .current_dir(dir)
        .args(["cdp.mjs", chrome.to_str().unwrap(), dir.join("profile").to_str().unwrap(), "visits.json", "out.json"])
        .output()
        .expect("could not run node");
    let out: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(dir.join("out.json")).unwrap_or_else(|_| panic!("the driver wrote nothing: {}", String::from_utf8_lossy(&o.stderr)))).unwrap();
    assert_eq!(out.len(), visits.len(), "the driver came back from {} of {} visits: {}", out.len(), visits.len(), String::from_utf8_lossy(&o.stderr));
    let mut wrong = Vec::new();
    for (v, got) in visits.iter().zip(&out) {
        let errors = got["errors"].as_array().cloned().unwrap_or_default();
        let thrown = got["thrown"].as_array().cloned().unwrap_or_default();
        if !errors.is_empty() || !thrown.is_empty() {
            wrong.push(format!("{}: {errors:?} {thrown:?}", v.url));
        }
        let server = v.url.split('/').take(3).collect::<Vec<_>>().join("/") + "/";
        for r in got["requests"].as_array().cloned().unwrap_or_default() {
            let r = r.as_str().unwrap_or_default();
            if !(r.starts_with(&server) || r.starts_with("blob:") || r.starts_with("data:") || r.starts_with("about:")) {
                wrong.push(format!("{}: asked for {r}", v.url));
            }
        }
    }
    assert!(wrong.is_empty(), "Chrome:\n{}", wrong.join("\n"));
    out.iter().map(|g| g["values"].as_array().cloned().unwrap_or_default()).collect()
}

/// What the page asks the module of a project's files, in a page's language.
fn ask_in(tag: &str, files: &[(String, String)], path: &str, target: Option<&str>) -> Request {
    let mut req = json!({ "lang": tag, "path": path, "target": target });
    req["files"] = files_json(files);
    Request::from_json(&req).unwrap()
}

/// What the status says of a check, in a page's language.
fn counts_in(tag: &str, a: &Value) -> String {
    let d = a["json"]["diagnostics"].as_array().cloned().unwrap_or_default();
    let (e, w) = (d.iter().filter(|x| x["severity"] == "error").count(), d.iter().filter(|x| x["severity"] == "warning").count());
    match (tag, e + w) {
        ("ja", 0) => "どれも検査を通りました".to_string(),
        ("ja", _) => format!("エラー {e} 件、警告 {w} 件"),
        (_, 0) => "all pass".to_string(),
        _ => format!("{e} error{}, {w} warning{}", if e == 1 { "" } else { "s" }, if w == 1 { "" } else { "s" }),
    }
}

/// A command as the output shows it: the command, then what it printed.
fn said(a: &Value) -> String {
    format!("$ {}\n{}{}", a["command"].as_str().unwrap(), a["out"].as_str().unwrap(), a["err"].as_str().unwrap())
}

/// A page as its frame holds it, without what the playground adds: the scheme on its root, and the
/// script that keeps where the reader scrolled to.
fn unframed(srcdoc: &str) -> String {
    let mut s = srcdoc.to_string();
    for scheme in ["light", "dark"] {
        s = s.replacen(&format!("<html data-theme=\"{scheme}\""), "<html", 1);
    }
    if let Some(i) = s.find("<script data-pg-scroll>") {
        let j = i + s[i..].find("</script>").expect("the script closes") + "</script>".len();
        s.replace_range(i..j, "");
    }
    s
}

/// The scheme a frame's page is drawn in.
fn scheme_of(srcdoc: &str) -> &'static str {
    if srcdoc.contains("<html data-theme=\"dark\"") {
        "dark"
    } else if srcdoc.contains("<html data-theme=\"light\"") {
        "light"
    } else {
        "none"
    }
}

fn text_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

/// The pages that explain the playground start in Chrome, as the site has them: each loads the
/// module in its worker and the projects, opens on the shop with what `ritsu check` prints in the
/// problems and the files it names marked, lists the projects of its language in their groups, and
/// opens a project, a file, a view and a target from a link (its own, and the links dandori's page
/// gave), and gives a link to what the reader has made of a project that opens it as it was: the
/// shop edited, and a file of the reader's own in the empty project. The page for people follows the
/// site's switch to dark.
#[test]
fn the_page_starts_in_chrome() {
    let Some(chrome) = chrome() else {
        return;
    };
    let projects = committed();
    let project = |name: &str| projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("projects.json has no {name}")).clone();
    let shop = project("shop");
    let base = format!("http://127.0.0.1:{}", serve(playground_pages(Vec::new())));
    let mut looked = 0;
    for tag in ["en", "ja"] {
        let at = format!("{base}{}", if tag == "ja" { PUBLISHED[1] } else { PUBLISHED[0] });
        let page = if tag == "ja" { "the Japanese page" } else { "the English page" };
        let ask = |files: &[(String, String)], path: &str, target: Option<&str>| ask_in(tag, files, path, target);
        let contract = "proto/shop/v1/order.proto";
        let returned = "  ORDER_STATUS_RETURNED = 5;\n";
        let flow = "ordering/ship_order.flow";
        let urgency = "ordering/rules/urgency.rule";
        let gap = project(if tag == "ja" { "rulec/gap.ja" } else { "rulec/gap" });
        let gate = project(if tag == "ja" { "sekisho/refunds.ja" } else { "sekisho/refunds" });
        let draft = project(&format!("dandori/{DRAFT}"));
        let hotel = if tag == "ja" { "examples/hotel/temporal/hotel.ja.flow" } else { "examples/hotel/temporal/hotel.flow" };
        let version = project(&format!("dandori/{hotel}"));
        let rule = project(if tag == "ja" { "rulec/full.ja" } else { "rulec/full" }).files[0].1.clone();
        let own = if tag == "ja" { "運賃.rule" } else { "fee.rule" };
        let shared = r#"!!location.hash && location.hash.includes("edits=")"#;
        let paste = format!(
            "const chooser = document.querySelector('.pg-project');\nchooser.value = chooser.querySelector('option[data-name=\"empty\"]').value;\nchooser.dispatchEvent(new Event('change'));\nwindow.prompt = () => {};\ndocument.querySelector('.pg-add').click();",
            json!(own)
        );
        let visits = vec![
            // the shop, as the page opens; then the site switched to dark, which the page for people of
            // the open file follows
            Visit::to(&at).ready().state().pressed(".pg-tab[data-view=\"doc\"]").run("document.body.setAttribute('data-md-color-scheme', 'slate');").ready().state(),
            // a link to a file and a view, the generator starting on the shop's platform, and to a
            // rule's page for people
            Visit::to(format!("{at}#project=shop&file={flow}&view=gen")).ready().state(),
            Visit::to(format!("{at}#project=shop&file={urgency}&view=doc")).ready().state(),
            // an example of rulec's page, and sekisho's example: the check of its gate, and the
            // first file of the Cedar it writes
            Visit::to(format!("{at}#project={}", gap.name)).ready().state(),
            Visit::to(format!("{at}#project={}", gate.name)).ready().state(),
            Visit::to(format!("{at}#project={}&file={}&view=gen", gate.name, gate.open)).ready().state(),
            // the links dandori's page gave: the first draft, a version built for a platform it
            // cannot run on and for its own, and the rules tab
            Visit::to(format!("{at}#flow={DRAFT}")).ready().state(),
            Visit::to(format!("{at}#flow={hotel}&view=build&target=asl")).ready().state(),
            Visit::to(format!("{at}#flow={hotel}&view=build")).ready().state(),
            Visit::to(format!("{at}#flow={hotel}&view=rules")).ready().state(),
            // the empty project, which asks for a file
            Visit::to(format!("{at}#project=empty")).ready().state(),
            // the shop edited, and the link the page gives to it
            Visit::to(&at).ready().edited(returned, "").run(&press(".pg-share")).wait(shared).state(),
            // one file of the reader's own, pasted into the empty project, and the link to it
            Visit::to(&at).ready().run(&paste).ready().run(&format!("{}\n", edit_into_empty(&rule))).ready().run(&press(".pg-share")).wait(shared).state(),
        ];
        let got = drive(&chrome, visits);

        // the shop, as the page opens: the check in the problems, the files it names marked
        let (opened, dark) = (&got[0][0], &got[0][1]);
        let want = playground::check(&ask(&shop.files, "", None));
        let status = text_of(opened, "status");
        assert!(status.starts_with(&format!("ritsu {}", env!("CARGO_PKG_VERSION"))) && status.ends_with(&counts_in(tag, &want)), "{page}: the status says {status:?}");
        assert_eq!(text_of(opened, "view"), "check", "{page}: the page does not open on the problems");
        assert_eq!(text_of(opened, "said"), said(&want), "{page}: the page shows another check");
        let diagnostics = want["json"]["diagnostics"].as_array().unwrap();
        assert_eq!(opened["problems"].as_array().unwrap().len(), diagnostics.len(), "{page}: the problems list another number of findings than the check says");
        let first = opened["problems"][0].as_str().unwrap_or_default();
        assert!(first.contains("rulec E032") && first.contains("billing/rules/billing_need.rule:5"), "{page}: the first problem is not rulec's E032 at billing/rules/billing_need.rule:5: {first:?}");
        assert_eq!(text_of(opened, "count"), diagnostics.len().to_string(), "{page}: the tab of the problems counts otherwise");
        for marked in ["billing/rules/billing_need.rule", "requirements/billing.req"] {
            assert!(opened["marked"].as_array().unwrap().iter().any(|m| m == marked), "{page}: the tab of {marked} is not marked");
        }
        assert_eq!(text_of(opened, "open"), contract, "{page}: the page does not open on the contract");
        assert_eq!(text_of(opened, "text"), shop.files.iter().find(|(p, _)| p == contract).unwrap().1, "{page}: the editor holds another contract");
        // the list: the projects of this page's language, in their six groups
        let mine: Vec<&str> = projects.iter().filter(|p| p.langs().contains(&tag)).map(|p| p.name.as_str()).collect();
        assert_eq!(opened["listed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>(), mine, "{page}: the list has other projects than projects.json has for this page");
        assert_eq!(opened["groups"], 6, "{page}: the list does not have its six groups");
        // a .proto has no page for people, which the page says; under the site's dark scheme, a rule's
        // page is drawn dark (the next visit opens one)
        assert_eq!(dark["page"], Value::Null, "{page}: a page is drawn of the contract");
        looked += 3;

        // a link to a file and a view: the flow's generator starts on Temporal, the shop's platform
        let want = playground::generate(&ask(&shop.files, flow, Some("temporal")));
        assert_eq!(text_of(&got[1][0], "said"), want["files"][0]["body"].as_str().unwrap(), "{page}: the page shows another first file of {flow} than Temporal's");
        assert_eq!((text_of(&got[1][0], "open"), text_of(&got[1][0], "view")), (flow, "gen"), "{page}: the link does not open {flow} on the generator");
        let n = want["files"].as_array().unwrap().len();
        let files = if tag == "ja" { format!("{n} ファイル") } else { format!("{n} files") };
        assert!(text_of(&got[1][0], "status").ends_with(&files), "{page}: the status does not count the {n} files written for Temporal: {}", text_of(&got[1][0], "status"));
        let want = playground::doc(&ask(&shop.files, urgency, None));
        let shown = got[2][0]["page"].as_str().unwrap_or_else(|| panic!("{page}: no page for people of {urgency}"));
        assert!(unframed(shown) == want["html"]["out"].as_str().unwrap(), "{page}: the frame holds another page than rulec doc draws of {urgency}");
        assert_eq!(scheme_of(shown), "light", "{page}: the page of {urgency} is not drawn in the site's light scheme");
        looked += 2;

        // an example of rulec's page
        let want = playground::check(&ask(&gap.files, "", None));
        assert_eq!(text_of(&got[3][0], "said"), said(&want), "{page}: the page shows another check of {}", gap.name);
        assert!(text_of(&got[3][0], "status").ends_with(&counts_in(tag, &want)), "{page}: the status of {} is not its check's", gap.name);
        // sekisho's example
        let want = playground::check(&ask(&gate.files, "", None));
        assert_eq!(text_of(&got[4][0], "said"), said(&want), "{page}: the page shows another check of {}", gate.name);
        assert_eq!(text_of(&got[4][0], "open"), gate.open, "{page}: {} does not open on its gate", gate.name);
        let want = playground::generate(&ask(&gate.files, &gate.open, None));
        assert_eq!(text_of(&got[5][0], "said"), want["files"][0]["body"].as_str().unwrap(), "{page}: the page shows another first file of the Cedar of {}", gate.open);
        looked += 3;

        // the links dandori's page gave
        let want = playground::check(&ask(&draft.files, "", None));
        assert_eq!(text_of(&got[6][0], "said"), said(&want), "{page}: the page shows another check of the first draft");
        assert_eq!(text_of(&got[6][0], "open"), DRAFT, "{page}: the page does not open on the first draft");
        let want = playground::generate(&ask(&version.files, hotel, Some("asl")));
        assert_ne!(want["code"], 0, "{hotel} is built for Step Functions after all");
        assert!(text_of(&got[7][0], "said").ends_with(&said(&want)), "{page}: the page does not say why {hotel} is not built for Step Functions");
        let want = playground::generate(&ask(&version.files, hotel, version.target.as_deref()));
        assert_eq!(text_of(&got[8][0], "said"), want["files"][0]["body"].as_str().unwrap(), "{page}: the page does not build {hotel} for the platform its version is for");
        let first_rule = version.files.iter().map(|(p, _)| p.as_str()).find(|p| p.ends_with(".rule")).unwrap();
        let want = playground::doc(&ask(&version.files, first_rule, None));
        assert_eq!((text_of(&got[9][0], "open"), text_of(&got[9][0], "view")), (first_rule, "doc"), "{page}: the rules tab of dandori's page does not open {first_rule} on its page");
        assert!(got[9][0]["page"].as_str().is_some_and(|p| unframed(p) == want["html"]["out"].as_str().unwrap()), "{page}: the page of {first_rule} is not what rulec doc draws");
        looked += 4;

        // the empty project, which asks for a file
        assert_eq!(got[10][0]["cta"], true, "{page}: the empty project does not point at adding a file");
        looked += 1;

        // links to what the reader made, given by the page, open it as it was
        let edited_hash = text_of(&got[11][0], "hash").to_string();
        assert!(edited_hash.starts_with("#project=shop&") && edited_hash.contains("&edits="), "{page}: the page gives no link to the edited shop: {edited_hash:?}");
        let own_hash = text_of(&got[12][0], "hash").to_string();
        assert!(own_hash.starts_with("#project=empty&") && own_hash.contains("&edits="), "{page}: the page gives no link to the file pasted into the empty project: {own_hash:?}");
        let again = drive(&chrome, vec![Visit::to(format!("{at}{edited_hash}")).ready().state(), Visit::to(format!("{at}{own_hash}")).ready().state()]);
        let mut fixed = shop.files.clone();
        fixed.iter_mut().find(|(p, _)| p == contract).unwrap().1 = shop.files.iter().find(|(p, _)| p == contract).unwrap().1.replace(returned, "");
        let want = playground::check(&ask(&fixed, "", None));
        assert_eq!(text_of(&again[0][0], "said"), said(&want), "{page}: the link {edited_hash} does not open the shop as it was edited");
        assert_eq!(again[0][0]["revert"], true, "{page}: the edited shop has no way back to the shop as it opens");
        let want = playground::check(&ask(&[(own.to_string(), rule.clone())], "", None));
        assert_eq!(text_of(&again[1][0], "said"), said(&want), "{page}: the link {own_hash} does not open the file pasted into the empty project");
        assert_eq!((text_of(&again[1][0], "open"), text_of(&again[1][0], "text")), (own, rule.as_str()), "{page}: the link {own_hash} opens another file");
        looked += 2;
    }
    // the site switched to dark draws the page of a rule dark: the shop's first rule, from a link
    let dark = drive(
        &chrome,
        vec![Visit::to(format!("{base}{}#project=shop&file=ordering/rules/urgency.rule&view=doc", PUBLISHED[0])).ready().run("document.body.setAttribute('data-md-color-scheme', 'slate');").wait(r#"(() => { const f = document.querySelector('.pg-panel[data-view="doc"] .pg-stage iframe:not(.pg-coming)'); return !!f && f.srcdoc.includes('<html data-theme="dark"'); })()"#).ready().state()],
    );
    assert_eq!(scheme_of(text_of(&dark[0][0], "page")), "dark", "the page of a rule does not follow the site's switch to dark");
    looked += 1;
    eprintln!("compared: {looked} views of the pages that explain the playground, in Chrome, with what the library answers");
}

/// A script that writes `text` into the file the empty project has just been given.
fn edit_into_empty(text: &str) -> String {
    format!(
        r#"const lib = new URL("libraries.js", document.querySelector('script[src$="playground.js"]').src).href;
const {{ EditorView }} = await import(lib);
const v = EditorView.findFromDOM(document.querySelector(".pg .cm-editor"));
v.dispatch({{ changes: {{ from: 0, to: v.state.doc.length, insert: {} }} }});"#,
        json!(text)
    )
}

/// The page of the playground alone (website/docs/play/index.html, and the Japanese one) starts in
/// Chrome on a small rule of rulec's examples in its language, with the page for people drawn of
/// it: the rule in the editor (a project of one file shows no tabs), and the page `rulec doc` draws
/// in the frame on the right, in the system's scheme, to save as HTML. An edit that breaks the rule
/// leaves the last page drawn there, faded, and the Markdown too, with the first problem in the band
/// under it, which takes the reader to its line; putting it right draws the page again. The tabs on
/// the right move with the arrow keys. In the Japanese rule, the wide characters of the table's rows
/// are each in a box two columns wide, so that the last `|` of every row is drawn at one place, and
/// those of the other lines are not.
#[test]
fn the_page_alone_opens_on_a_rule_and_keeps_the_last_page_drawn() {
    let Some(chrome) = chrome() else {
        return;
    };
    let projects = committed();
    let base = format!("http://127.0.0.1:{}", serve(playground_pages(Vec::new())));
    let mut looked = 0;
    for tag in ["en", "ja"] {
        let at = format!("{base}{}", if tag == "ja" { "/ritsu/ja/play/" } else { "/ritsu/play/" });
        let page = format!("the {} page alone", if tag == "ja" { "Japanese" } else { "English" });
        let name = if tag == "ja" { "rulec/full.ja" } else { "rulec/full" };
        let p = projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("projects.json has no {name}"));
        let (file, rule) = p.files[0].clone();
        let row = if tag == "ja" { "| 遠隔地 | >2kg <=5kg | 1500円       |\n" } else { "| overseas    | >2kg <=5kg | 15USD  |\n" };
        assert!(rule.ends_with(row), "{name} no longer ends with the row `{row}`");
        let broken = rule.replacen(row, "", 1);
        let ask = |text: &str| ask_in(tag, &[(file.clone(), text.to_string())], &file, None);
        let good = playground::doc(&ask(&rule));
        assert_eq!(good["html"]["code"], 0, "{name} draws no page");
        let bad = playground::check(&ask_in(tag, &[(file.clone(), broken.clone())], "", None));
        let first = bad["json"]["diagnostics"].as_array().unwrap().iter().find(|d| d["severity"] == "error").expect("the rule without its row has an error").clone();
        let line = first["line"].as_u64().unwrap();
        let visits = vec![
            Visit::to(&at).ready().state()
                // the rule broken, the Markdown of it, and the band pressed
                .edited(row, "").state()
                .pressed(".pg-tab[data-view=\"markdown\"]").state()
                .pressed(".pg-tab[data-view=\"doc\"]").click(".pg-band").state()
                // put right again
                .run(&format!("{}\n", edit_into_end(row))).ready().state()
                // the reader scrolls the page for people (rulec's scrolls its side, on the left of
                // it), and edits: the page drawn again starts where the reader left it
                .wheel(".pg-panel[data-view=\"doc\"] .pg-stage", (0.15, 0.6), 800).sleep(400).edited(from_fee(tag).0, from_fee(tag).1).state().edited(from_fee(tag).1, from_fee(tag).0)
                // the tabs on the right, by the keys
                .run("document.querySelector('.pg-tab[aria-selected=\"true\"]').focus();").key("ArrowRight").ready().state()
                // the rule as it was, its rows and its other lines in the editor
                .get(LINES_DRAWN),
            // the system's dark scheme
            Visit { dark: true, ..Visit::to(&at) }.ready().state(),
        ];
        let got = drive(&chrome, visits);
        let [opened, broken_page, broken_markdown, banded, fixed, scrolled, keyed] = [&got[0][0], &got[0][1], &got[0][2], &got[0][3], &got[0][4], &got[0][5], &got[0][6]];
        assert_eq!((text_of(opened, "project"), text_of(opened, "open"), text_of(opened, "view")), (name, file.as_str(), "doc"), "{page} does not open on {name} and its page for people");
        assert_eq!(text_of(opened, "text"), rule, "{page}: the editor holds another rule");
        assert_eq!(opened["tabs"], false, "{page}: a project of one file shows its tabs");
        assert!(unframed(text_of(opened, "page")) == good["html"]["out"].as_str().unwrap(), "{page}: the frame holds another page than rulec doc draws of {file}");
        assert_eq!(scheme_of(text_of(opened, "page")), "light", "{page}: the page is not drawn in the system's light scheme");
        assert_eq!(text_of(opened, "save"), format!("{}.html", file.trim_end_matches(".rule")), "{page}: the page is not saved under the name of its file");
        let mine: Vec<&str> = projects.iter().filter(|q| q.langs().contains(&tag)).map(|q| q.name.as_str()).collect();
        assert_eq!(opened["listed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>(), mine, "{page}: the list has other projects than projects.json has for this page");
        // broken: the last page stays, faded, with the first problem under it
        assert_eq!(broken_page["stale"], true, "{page}: the page is not faded while the rule does not pass");
        assert!(unframed(text_of(broken_page, "page")) == good["html"]["out"].as_str().unwrap(), "{page}: the frame does not keep the last page drawn");
        let band = text_of(broken_page, "band");
        let (code, message) = (first["code"].as_str().unwrap(), first["title"].as_str().or(first["message"].as_str()).unwrap());
        assert!(band.contains(code) && band.contains(message), "{page}: the band does not say the first problem, {code} {message}: {band:?}");
        assert_eq!(text_of(broken_page, "count"), bad["json"]["diagnostics"].as_array().unwrap().len().to_string(), "{page}: the tab of the problems counts otherwise");
        assert_eq!(broken_markdown["markdownStale"], true, "{page}: the Markdown is not faded while the rule does not pass");
        assert_eq!(text_of(broken_markdown, "markdown"), good["markdown"]["out"].as_str().unwrap(), "{page}: the Markdown is not the last one drawn");
        assert_eq!(banded["line"].as_u64(), Some(line), "{page}: the band does not take the reader to line {line}");
        // put right: drawn again, not faded, no band
        assert_eq!((fixed["stale"].clone(), fixed["band"].clone()), (Value::Bool(false), Value::Null), "{page}: the page put right is still faded, or has the band");
        assert!(unframed(text_of(fixed, "page")) == good["html"]["out"].as_str().unwrap(), "{page}: the page put right is not drawn again");
        assert_eq!(text_of(fixed, "text"), rule, "{page}: the rule is not what it was");
        // the page drawn after the edit starts where the reader scrolled to, which its frame said
        let drawn = text_of(scrolled, "page");
        let at: Value = drawn.split("const at = ").nth(1).and_then(|r| r.split(';').next()).and_then(|r| serde_json::from_str(r).ok()).unwrap_or_else(|| panic!("{page}: the page drawn again does not say where to start"));
        let y = at.as_array().unwrap().iter().filter_map(|p| p[2].as_i64()).max().unwrap_or(0);
        assert!(y > 200, "{page}: the page drawn again starts at {at}, not where the reader scrolled to");
        assert!(unframed(drawn) == playground::doc(&ask(&rule.replacen(from_fee(tag).0, from_fee(tag).1, 1)))["html"]["out"].as_str().unwrap(), "{page}: the page drawn again is not the page of the rule edited");
        // the arrow key moved the selection to the next tab, and the focus with it
        assert_eq!((text_of(keyed, "view"), text_of(keyed, "focused")), ("markdown", "markdown"), "{page}: the arrow key does not move from the page for people to the Markdown");
        // the wide characters of the rows are in boxes, and the rows' last `|` under one another
        let (rows, others) = (got[0][7]["rows"].as_array().unwrap(), got[0][7]["others"].as_array().unwrap());
        if tag == "ja" {
            assert!(!rows.is_empty() && !others.is_empty(), "{page}: {name} has no row of a table, or no other line, in Japanese");
            for r in rows {
                assert!(r["boxes"].as_u64() >= r["wide"].as_u64(), "{page}: a wide character of the row {} is not in a box two columns wide", r["text"]);
            }
            for o in others {
                assert_eq!(o["boxes"], 0, "{page}: the line {}, not a row of a table, has its wide characters in boxes", o["text"]);
            }
            let ends: Vec<f64> = rows.iter().map(|r| r["end"].as_f64().unwrap()).collect();
            let (lo, hi) = ends.iter().fold((f64::MAX, f64::MIN), |(lo, hi), e| (lo.min(*e), hi.max(*e)));
            assert!(hi - lo < 0.5, "{page}: the last `|` of the rows of {name} are drawn at {ends:?}, not under one another");
        }
        // the system's dark scheme draws the page dark
        assert_eq!(scheme_of(text_of(&got[1][0], "page")), "dark", "{page}: the page is not drawn in the system's dark scheme");
        looked += got.iter().map(Vec::len).sum::<usize>();
    }
    eprintln!("compared: {looked} views of the page of the playground alone, in Chrome, with what the library answers");
}

/// The lines of the open file as the editor draws them: each row of a table (a line that starts with
/// `|`), with its wide characters (CJK, kana), the boxes they are in, and where its last `|` is
/// drawn; and each other line that has wide characters, with its boxes.
const LINES_DRAWN: &str = r#"(() => {
  const wide = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]/gu;
  const rows = [], others = [];
  for (const l of document.querySelectorAll(".pg .cm-line")) {
    const text = l.textContent;
    const line = { text, wide: (text.match(wide) || []).length, boxes: l.querySelectorAll(".pg-wide").length };
    if (/^\s*\|/.test(text)) {
      let last = null;
      const walk = document.createTreeWalker(l, NodeFilter.SHOW_TEXT);
      for (let n = walk.nextNode(); n; n = walk.nextNode()) if (n.data.includes("|")) last = [n, n.data.lastIndexOf("|")];
      const r = document.createRange();
      r.setStart(last[0], last[1]);
      r.setEnd(last[0], last[1] + 1);
      rows.push({ ...line, end: r.getBoundingClientRect().left });
    } else if (line.wide) others.push(line);
  }
  return { rows, others };
})()"#;

/// The page for people that rulec draws reads in a narrow frame (rulec's DESIGN 15.246). In the
/// playground's half of a window, its side is wide enough for the tables of the inputs and the
/// outputs, which break a cell between words only and never set a heading or a name one character
/// to a line, and the form to try the rule keeps its inputs inside it; on a phone the form, the
/// picture it lights up and the rest of the document are one column, in that order, and nothing is
/// wider than the page. The rules are the one the page alone opens on and the biggest of rulec's
/// examples, in Japanese, which breaks a line between any two characters.
#[test]
fn the_page_for_people_reads_in_a_narrow_frame() {
    let Some(chrome) = chrome() else {
        return;
    };
    let projects = committed();
    let mut pages = Vec::new();
    for name in ["rulec/full.ja", "rulec/big.ja"] {
        let p = projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("projects.json has no {name}"));
        let (file, rule) = p.files[0].clone();
        let page = playground::doc(&ask_in("ja", &[(file.clone(), rule)], &file, None));
        assert_eq!(page["html"]["code"], 0, "{name} draws no page");
        pages.push((format!("/{name}/"), page["html"]["out"].as_str().unwrap().to_string()));
    }
    let base = format!("http://127.0.0.1:{}", serve(pages.clone()));
    // the whole window, the playground's half of a window of 1400, and a phone
    let sizes = [(1400, 860), (761, 748), (390, 844)];
    let mut visits = Vec::new();
    for (path, _) in &pages {
        for (width, height) in sizes {
            visits.push(Visit { width, height, ..Visit::to(format!("{base}{path}")) }.sleep(500).get(LAID_OUT));
        }
    }
    let got = drive(&chrome, visits);
    let mut looked = 0;
    for (i, (path, _)) in pages.iter().enumerate() {
        for (j, (width, _)) in sizes.iter().enumerate() {
            let at = &got[i * sizes.len() + j][0];
            let page = format!("the page for people of {path} at {width}px");
            assert!(at["broken"].as_array().unwrap().is_empty(), "{page} sets a cell of its side one character to a line: {}", at["broken"]);
            assert_eq!(at["over"], 0, "{page}: an input of the form to try the rule is wider than the form");
            assert!(at["wide"].as_f64() <= at["vw"].as_f64(), "{page} is wider than the window: {} > {}", at["wide"], at["vw"]);
            let (form, picture, rest) = (at["form"].as_f64().unwrap(), at["picture"].as_f64().unwrap(), at["rest"].as_f64().unwrap());
            if *width < 720 {
                assert!(form < picture && picture < rest, "{page} is not one column of the form, the picture and the rest, in that order: {form}, {picture}, {rest}");
            } else {
                assert!(at["side"].as_f64().unwrap() >= 320.0, "{page}: the side is {} wide, too narrow for its tables", at["side"]);
                assert!(picture < rest, "{page} does not put the picture beside the side");
            }
            looked += 1;
        }
    }
    eprintln!("compared: {looked} views of rulec's page for people, in Chrome, at three widths");
}

/// How the page for people is laid out: the cells of its side's tables set one character to a line,
/// the inputs of the form wider than the form, how wide the document is against the window, where
/// the form, the picture and the first table of the rest of the document begin on the page, and how
/// wide the side is.
const LAID_OUT: &str = r##"(() => {
  const at = (q) => { const e = document.querySelector(q); return e ? e.getBoundingClientRect().top + scrollY : null; };
  const broken = [];
  for (const c of document.querySelectorAll(".app .side th, .app .side td")) {
    const text = c.textContent.replace(/\s+/g, "");
    if (text.length < 2) continue;
    const r = document.createRange();
    r.selectNodeContents(c);
    if (new Set([...r.getClientRects()].map((x) => Math.round(x.top))).size >= text.length) broken.push(text);
  }
  const form = document.querySelector("#try").getBoundingClientRect().right;
  const over = [...document.querySelectorAll("#try-form input, #try-form select")].filter((e) => e.getBoundingClientRect().right > form + 0.5).length;
  return {
    broken, over,
    vw: document.documentElement.clientWidth, wide: document.documentElement.scrollWidth,
    form: at("#try"), picture: at(".app .stage"), rest: at('.app section[data-sec="inputs"]'),
    side: document.querySelector(".app .side").getBoundingClientRect().width,
  };
})()"##;

/// A small edit of the rule the page alone opens on, which it passes after: a fee of its table.
fn from_fee(tag: &str) -> (&'static str, &'static str) {
    if tag == "ja" { ("| 近畿圏 | <=2kg      | 800円 ", "| 近畿圏 | <=2kg      | 900円 ") } else { ("| domestic    | <=2kg      | 8USD ", "| domestic    | <=2kg      | 9USD ") }
}

/// A script that adds `text` at the end of the open file.
fn edit_into_end(text: &str) -> String {
    format!(
        r#"const lib = new URL("libraries.js", document.querySelector('script[src$="playground.js"]').src).href;
const {{ EditorView }} = await import(lib);
const v = EditorView.findFromDOM(document.querySelector(".pg .cm-editor"));
v.dispatch({{ changes: {{ from: v.state.doc.length, insert: {} }} }});"#,
        json!(text)
    )
}

/// A link carries the playground from one of its pages to the other, as the reader left it: the
/// link the page alone gives (the project, the file, the view and the edits) opens the same on the
/// page that explains it, and that page's "open full screen" opens the page alone on what the
/// reader has made there, in each language. The links the pages before gave, and the ones their
/// pages send on, open on the page alone too.
#[test]
fn a_link_opens_the_same_file_and_tab_on_either_page() {
    let Some(chrome) = chrome() else {
        return;
    };
    let projects = committed();
    let base = format!("http://127.0.0.1:{}", serve(playground_pages(Vec::new())));
    let mut looked = 0;
    for tag in ["en", "ja"] {
        let (explained, alone) = if tag == "ja" { ("/ritsu/ja/playground/", "/ritsu/ja/play/") } else { ("/ritsu/playground/", "/ritsu/play/") };
        let name = if tag == "ja" { "rulec/full.ja" } else { "rulec/full" };
        let (file, rule) = projects.iter().find(|p| p.name == name).unwrap().files[0].clone();
        let (from, to) = if tag == "ja" { ("| 近畿圏 | <=2kg      | 800円 ", "| 近畿圏 | <=2kg      | 900円 ") } else { ("| domestic    | <=2kg      | 8USD ", "| domestic    | <=2kg      | 9USD ") };
        let changed = rule.replacen(from, to, 1);
        let shop = projects.iter().find(|p| p.name == "shop").unwrap();
        let urgency = "ordering/rules/urgency.rule";
        let text = shop.files.iter().find(|(p, _)| p == urgency).unwrap().1.clone();
        let (u_from, u_to) = ("anyone else's from 30,000 yen", "anyone else's from 20,000 yen");
        let shared = r#"!!location.hash && location.hash.includes("edits=")"#;
        let landed = format!("location.pathname === {} && {READY}", json!(alone));
        let got = drive(
            &chrome,
            vec![
                // the page alone: the rule edited, the Markdown, and the link it gives
                Visit::to(format!("{base}{alone}")).ready().edited(from, to).pressed(".pg-tab[data-view=\"markdown\"]").run(&press(".pg-share")).wait(shared).state(),
                // the page that explains it: another file, the generator, an edit, and full screen
                Visit::to(format!("{base}{explained}")).ready().pressed(&format!(".pg-file[data-path=\"{urgency}\"]")).pressed(".pg-tab[data-view=\"gen\"]").edited(u_from, u_to).run(&press(".pg-expand")).wait(&landed).state(),
            ],
        );
        let (given, expanded) = (&got[0][0], &got[1][0]);
        let hash = text_of(given, "hash").to_string();
        assert!(hash.starts_with(&format!("#project={}&view=markdown&edits=", name.replace('/', "%2F"))) || hash.starts_with(&format!("#project={name}&view=markdown&edits=")), "{tag}: the page alone gives another link: {hash:?}");
        let opened = drive(&chrome, vec![Visit::to(format!("{base}{explained}{hash}")).ready().state()]);
        let opened = &opened[0][0];
        assert_eq!((text_of(opened, "project"), text_of(opened, "open"), text_of(opened, "view")), (name, file.as_str(), "markdown"), "{tag}: the link of the page alone opens another project, file or view on the page that explains it");
        assert_eq!(text_of(opened, "text"), changed, "{tag}: the link does not carry the edit");
        let want = playground::doc(&ask_in(tag, &[(file.clone(), changed.clone())], &file, None));
        assert_eq!(text_of(opened, "markdown"), want["markdown"]["out"].as_str().unwrap(), "{tag}: the Markdown of the edited rule is not what rulec doc writes");
        looked += 1;
        // full screen, from the page that explains it
        assert_eq!(text_of(expanded, "path"), alone, "{tag}: full screen does not open the page alone");
        assert_eq!((text_of(expanded, "project"), text_of(expanded, "open"), text_of(expanded, "view")), ("shop", urgency, "gen"), "{tag}: full screen opens another project, file or view");
        let mut files = shop.files.clone();
        files.iter_mut().find(|(p, _)| p == urgency).unwrap().1 = text.replacen(u_from, u_to, 1);
        assert_eq!(text_of(expanded, "text"), files.iter().find(|(p, _)| p == urgency).unwrap().1, "{tag}: full screen does not carry the edit");
        let want = playground::generate(&ask_in(tag, &files, urgency, None));
        assert_eq!(text_of(expanded, "said"), want["files"][0]["body"].as_str().unwrap(), "{tag}: full screen shows another first file of the generator");
        looked += 1;
        // the links the pages before gave, and those their pages send on, open on the page alone as
        // on the page that explains it: a link with no view opens the problems there too
        let gap = projects.iter().find(|p| p.name == if tag == "ja" { "rulec/gap.ja" } else { "rulec/gap" }).unwrap();
        let draft = projects.iter().find(|p| p.name == format!("dandori/{DRAFT}")).unwrap();
        let hotel = if tag == "ja" { "examples/hotel/temporal/hotel.ja.flow" } else { "examples/hotel/temporal/hotel.flow" };
        let version = projects.iter().find(|p| p.name == format!("dandori/{hotel}")).unwrap();
        let old = drive(
            &chrome,
            vec![
                Visit::to(format!("{base}{alone}#project={}", gap.name)).ready().state(),
                Visit::to(format!("{base}{alone}#flow={DRAFT}")).ready().state(),
                Visit::to(format!("{base}{alone}#flow={hotel}&view=build&target=asl")).ready().state(),
            ],
        );
        let want = playground::check(&ask_in(tag, &gap.files, "", None));
        assert_eq!((text_of(&old[0][0], "view"), text_of(&old[0][0], "said")), ("check", said(&want).as_str()), "{tag}: the page alone does not open {} on its check", gap.name);
        let want = playground::check(&ask_in(tag, &draft.files, "", None));
        assert_eq!((text_of(&old[1][0], "open"), text_of(&old[1][0], "said")), (DRAFT, said(&want).as_str()), "{tag}: the page alone does not open the first draft dandori's page linked to");
        let want = playground::generate(&ask_in(tag, &version.files, hotel, Some("asl")));
        assert!(text_of(&old[2][0], "said").ends_with(&said(&want)), "{tag}: the page alone does not say why {hotel} is not built for Step Functions");
        looked += 3;
    }
    eprintln!("compared: {looked} links between the two pages of the playground and of the pages before, in Chrome, with what the library answers");
}

/// A page the playground of rulec's or dandori's site had, kept to send a reader on to this page
/// (website/<site>/docs/playground.md, and docs-ja/ for the Japanese one), as its site publishes it
/// at <site>/playground/: its paragraph with the link `#moved` and its script, which the Markdown holds
/// as HTML. Zensical reads a relative link of a page from the page's file, and publishes the page a
/// directory deeper, so the link it writes there goes up once more (tests/website.rs holds the built
/// page to that).
fn page_before(site: &str, tag: &str) -> String {
    let md = read_repo(&format!("website/{site}/{}/playground.md", if tag == "ja" { "docs-ja" } else { "docs" }));
    let cut = |open: &str, close: &str| -> String {
        let from = md.find(open).unwrap_or_else(|| panic!("{site}'s {tag} page has no {open}"));
        let to = md[from..].find(close).unwrap_or_else(|| panic!("{site}'s {tag} page does not close {open}")) + from + close.len();
        md[from..to].to_string()
    };
    let link = cut("<p>", "</p>");
    let href = link.find(" href=\"").expect("the paragraph has a link") + " href=\"".len();
    assert!(!link[href..].starts_with('/') && !link[href..].starts_with('#') && !link[href..].starts_with("http"), "{site}'s {tag} page links to ritsu's playground by no relative path");
    let published = format!("{}../{}", &link[..href], &link[href..]);
    format!("<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{published}{}</body></html>", cut("<script>", "</script>"))
}

/// The pages the playgrounds of rulec's and dandori's sites had send a reader on to the page that
/// explains ritsu's playground, in their language, with the link the reader followed: one that
/// opened something there opens the same here (the links dandori's page gave,
/// `#flow=…&view=…&target=…`, which this page reads), and any other (none, or a heading of the page
/// that was there) opens what that page opened on, rulec's table with a row missing and dandori's
/// first draft. The pages are served where the site publishes them, /ritsu/<site>/playground/ and
/// /ritsu/<site>/ja/playground/, beside the playground at /ritsu/playground/ and
/// /ritsu/ja/playground/: a link that climbed out of /ritsu/ would land on no page here, as on the
/// site.
#[test]
fn the_pages_before_send_their_links_on() {
    let Some(chrome) = chrome() else {
        return;
    };
    let projects = committed();
    let project = |name: &str| projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("projects.json has no {name}")).clone();
    let draft = format!("#flow={DRAFT}");
    let (hotel, hotel_ja) = ("examples/hotel/temporal/hotel.flow", "examples/hotel/temporal/hotel.ja.flow");
    // the page that was there, the link followed to it, and where it should send the reader: the
    // link the playground opens on, the project, the file open, and the target of the generator shown
    let visits: Vec<(&str, &str, String, String, String, Option<&str>)> = vec![
        ("/ritsu/rulec/playground/", "", "#project=rulec/gap".into(), "rulec/gap".into(), String::new(), None),
        ("/ritsu/rulec/playground/", "#what-to-try", "#project=rulec/gap".into(), "rulec/gap".into(), String::new(), None),
        ("/ritsu/rulec/ja/playground/", "", "#project=rulec/gap.ja".into(), "rulec/gap.ja".into(), String::new(), None),
        ("/ritsu/dandori/playground/", "", draft.clone(), format!("dandori/{DRAFT}"), DRAFT.into(), None),
        ("/ritsu/dandori/playground/", "#flow=examples/hotel/temporal/hotel.flow&view=build&target=asl", "#flow=examples/hotel/temporal/hotel.flow&view=build&target=asl".into(), format!("dandori/{hotel}"), hotel.into(), Some("asl")),
        ("/ritsu/dandori/ja/playground/", "", draft.clone(), format!("dandori/{DRAFT}"), DRAFT.into(), None),
        ("/ritsu/dandori/ja/playground/", "#flow=examples/hotel/temporal/hotel.ja.flow&view=build", "#flow=examples/hotel/temporal/hotel.ja.flow&view=build".into(), format!("dandori/{hotel_ja}"), hotel_ja.into(), Some("temporal")),
    ];
    let mut more: Vec<(String, String)> = Vec::new();
    for s in ["rulec", "dandori"] {
        more.push((format!("/ritsu/{s}/playground/"), page_before(s, "en")));
        more.push((format!("/ritsu/{s}/ja/playground/"), page_before(s, "ja")));
    }
    let base = format!("http://127.0.0.1:{}", serve(playground_pages(more)));
    let plan = visits
        .iter()
        .map(|(before, link, _, _, _, _)| {
            let to = if before.contains("/ja/") { PUBLISHED[1] } else { PUBLISHED[0] };
            Visit::to(format!("{base}{before}{link}")).wait(&format!("location.pathname === {} && {READY}", json!(to))).state()
        })
        .collect();
    let got = drive(&chrome, plan);
    for ((before, link, lands, name, open, target), got) in visits.iter().zip(&got) {
        let tag = if before.contains("/ja/") { "ja" } else { "en" };
        let state = &got[0];
        let came = format!("{}{lands}", if tag == "ja" { PUBLISHED[1] } else { PUBLISHED[0] });
        assert_eq!(format!("{}{}", text_of(state, "path"), text_of(state, "hash")), came, "{before}{link} sends the reader elsewhere");
        let p = project(name);
        let open = if open.is_empty() { p.open.clone() } else { open.clone() };
        assert_eq!((text_of(state, "project"), text_of(state, "open")), (name.as_str(), open.as_str()), "{before}{link} does not open {name} on {open}");
        let shown = text_of(state, "said");
        match target {
            None => {
                let want = playground::check(&ask_in(tag, &p.files, "", None));
                assert_eq!(shown, said(&want), "{before}{link} opens another check than that of {name}");
                assert!(text_of(state, "status").ends_with(&counts_in(tag, &want)), "{before}{link}: the status is not that of the check of {name}");
            }
            Some(t) => {
                let want = playground::generate(&ask_in(tag, &p.files, &open, Some(t)));
                if want["code"] == 0 {
                    assert_eq!(shown, want["files"][0]["body"].as_str().unwrap(), "{before}{link} shows another first file of {open} for {t}");
                } else {
                    assert!(shown.ends_with(&said(&want)), "{before}{link} does not say why {open} is not built for {t}");
                }
            }
        }
    }
    eprintln!("compared: {} links to the pages rulec's and dandori's playgrounds had, sent on to the playground in Chrome, with what the library answers", visits.len());
}
