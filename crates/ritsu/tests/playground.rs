//! The playground on the site (website/docs/playground, DESIGN 8.7): the page answers what the
//! `ritsu` binary answers in a directory holding the same files, the module it runs answers what the
//! library answers, and the page starts in Chrome, shows what `ritsu check` prints, lists every
//! project it opens, and opens the links it reads and the links it gives.
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
//! flow with every file it reads, from dandori's examples) and sekisho's example (each version of
//! its gate with every file it reads). Node drives the module, and
//! Chrome the page (ritsu-testkit's: `RITSU_CHROME`, else where macOS keeps it, else on the PATH).
//! A test that cannot find what it needs prints `SKIP: ritsu: …` and passes.

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
use std::time::Duration;

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
/// on the contract that has just gained a value.
fn shop(name: &str) -> Project {
    let files = files_of(&repo().join("website/playground").join(name));
    let open = "proto/shop/v1/order.proto";
    assert!(files.iter().any(|(p, _)| p == open), "{name} has no {open}");
    let from = files.iter().map(|(p, _)| format!("website/playground/{name}/{p}")).collect();
    Project { name: name.into(), group: "ritsu".into(), open: open.into(), files, from, ..Project::default() }
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
/// examples of rulec's page in English and then in Japanese, the examples of dandori's page, and
/// sekisho's example in English and in Japanese.
fn made() -> Vec<Project> {
    let mut out = vec![shop("shop"), shop("shop.ja")];
    out.push(Project { name: "empty".into(), group: "own".into(), ..Project::default() });
    for (i, lang) in ["en", "ja"].into_iter().enumerate() {
        for (key, en, ja) in RULEC {
            out.push(rulec_example(key, lang, if i == 0 { en } else { ja }));
        }
    }
    out.extend(dandori_examples());
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
    assert!(was == now, "website/docs/playground/projects.json is not what website/playground, rulec's corpus, dandori's examples and sekisho's example hold now; write it anew with RITSU_BLESS=1 cargo test -p ritsu --test playground");
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
            Some(Tool::Chobo) => vec!["postgres", "postgres-typescript", "postgres-python", "postgres-go", "tigerbeetle-typescript", "tigerbeetle-python", "tigerbeetle-go"],
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
/// then whole), a rule's output renamed under the flow that reads it, a flow that does not parse, a
/// file of no language added, and a geas spec that does not parse (a spec that parses runs
/// programs, which the page cannot: it is not held to the binary). Then the steps the playgrounds of
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
    for (name, lang, rule, rounding) in [("rulec/full", "en", "Fee.rule", "  round up(1USD)"), ("rulec/full.ja", "ja", "運賃.rule", "  round up(10円)")] {
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

/// Chrome, when the level lets the test run it and the machine has it; a SKIP line says why not.
fn chrome() -> Option<PathBuf> {
    if !need(Need::Chrome) {
        return None;
    }
    let found = ritsu_testkit::chrome::find();
    if found.is_none() {
        skip("Chrome is not found; set RITSU_CHROME to run this test");
    }
    found
}

/// A server for the pages: each at its path (`/` and the others), and the playground's files beside
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
    let mut buf = [0u8; 4096];
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
        std::fs::read(site().join(asked.trim_start_matches('/'))).ok().map(|b| (b, kind))
    };
    let (status, body, kind) = match found {
        Some((b, k)) => ("200 OK", b, k),
        None => ("404 Not Found", Vec::new(), "text/plain"),
    };
    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    let _ = s.write_all(&body);
}

/// The DOM of a page after its scripts ran (up to 20 s of virtual time), as headless Chrome dumps it.
fn dump_dom(chrome: &Path, url: &str) -> String {
    ritsu_testkit::chrome::dump_dom(chrome, url, 20_000, Duration::from_secs(120))
}

/// The text inside the first element of a class, up to the tag that closes it, as a person reads it.
fn text_in(dom: &str, class: &str, close: &str) -> String {
    let from = dom.find(&format!("class=\"{class}\"")).map(|i| i + dom[i..].find('>').unwrap() + 1).unwrap_or(dom.len());
    let inner = &dom[from..from + dom[from..].find(close).unwrap_or(0)];
    let mut text = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", "\u{a0}").replace("&amp;", "&")
}

/// The widget as a page has it, with the script served from the test's server.
fn widget_of(page: &str) -> String {
    let md = std::fs::read_to_string(repo().join(page)).unwrap();
    let from = md.find("<div class=\"pg\"").unwrap_or_else(|| panic!("{page} has no playground"));
    let tail = "<script src=\"playground/playground.js\" defer></script>";
    let to = md[from..].find(tail).unwrap_or_else(|| panic!("{page} does not load playground.js")) + from + tail.len();
    md[from..to].replace("playground/playground.js", "/playground.js")
}

/// A script for a test's page, run once the page has answered for the project it opens on: the
/// steps `then` takes (an async function's body, with `root`, the widget, and `until`, which waits
/// for a condition), and what `location.hash` is after them, written into the page as
/// `<pre id="shared">`.
fn steps(then: &str) -> String {
    format!(
        r#"<script>
(async () => {{
  const until = async (f) => {{ for (let i = 0; i < 600; i++) {{ if (f()) return true; await new Promise((r) => setTimeout(r, 50)); }} return false; }};
  const root = document.querySelector(".pg");
  await until(() => / ms/.test(root.querySelector(".pg-status").textContent));
  {then}
  await until(() => location.hash.includes("edits="));
  document.body.append(Object.assign(document.createElement("pre"), {{ id: "shared", textContent: location.hash }}));
}})();
</script>"#
    )
}

/// What a page of [`steps`] wrote of `location.hash`.
fn shared(dom: &str) -> String {
    dom.split("<pre id=\"shared\">").nth(1).and_then(|r| r.split("</pre>").next()).unwrap_or_default().replace("&amp;", "&")
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

/// The page starts in Chrome, as the site's pages have it: it loads the module and the projects,
/// shows what `ritsu check` prints for the project it opens with a mark on the files it names, lists
/// the projects of its language in their groups, opens a project, a file, a view and a target from a
/// link (the page's own, and the links dandori's page gave), and gives a link to what the reader has
/// made of a project that opens it as it was, the shop edited and a file of the reader's own in the
/// empty project.
#[test]
fn the_page_starts_in_chrome() {
    let Some(chrome) = chrome() else {
        return;
    };
    if !site().join("ritsu.wasm").exists() {
        skip("website/docs/playground/ritsu.wasm is missing; run website/tools/make_wasm.sh");
        return;
    }
    let projects = committed();
    let project = |name: &str| projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("projects.json has no {name}")).clone();
    let shop = project("shop");
    let mut looked = 0;
    for (tag, page) in [("en", "website/docs/playground.md"), ("ja", "website/docs-ja/playground.md")] {
        let widget = widget_of(page);
        let doc = |extra: &str| format!("<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{widget}{extra}</body></html>");
        let contract = "proto/shop/v1/order.proto";
        let returned = "  ORDER_STATUS_RETURNED = 5;\n";
        let edit_shop = format!(
            "const src = root.querySelector(\".pg-src\");\n  src.value = src.value.replace({}, \"\");\n  src.dispatchEvent(new Event(\"input\"));\n  root.querySelector(\".pg-share\").click();",
            json!(returned)
        );
        let rule = project(if tag == "ja" { "rulec/full.ja" } else { "rulec/full" }).files[0].1.clone();
        let own = if tag == "ja" { "運賃.rule" } else { "fee.rule" };
        let paste = format!(
            "const chooser = root.querySelector(\".pg-project\");\n  chooser.value = chooser.querySelector('option[data-name=\"empty\"]').value;\n  chooser.dispatchEvent(new Event(\"change\"));\n  window.prompt = () => {};\n  root.querySelector(\".pg-add\").click();\n  const src = root.querySelector(\".pg-src\");\n  src.value = {};\n  src.dispatchEvent(new Event(\"input\"));\n  root.querySelector(\".pg-share\").click();",
            json!(own),
            json!(rule)
        );
        let port = serve(vec![("/".to_string(), doc("")), ("/edit".to_string(), doc(&steps(&edit_shop))), ("/own".to_string(), doc(&steps(&paste)))]);
        let ask = |files: &[(String, String)], path: &str, target: Option<&str>| ask_in(tag, files, path, target);
        let counts = |a: &Value| counts_in(tag, a);
        let base = format!("http://127.0.0.1:{port}");

        // the shop, as the page opens
        let dom = dump_dom(&chrome, &format!("{base}/"));
        let status = text_in(&dom, "pg-status", "</span>");
        assert!(status.starts_with(&format!("ritsu {}", env!("CARGO_PKG_VERSION"))), "{page}: the status says {status:?}");
        let want = playground::check(&ask(&shop.files, "", None));
        assert!(status.ends_with(&counts(&want)), "{page}: the status says {status:?}");
        assert_eq!(text_in(&dom, "pg-out", "</div>"), said(&want), "{page}: the page shows another check");
        for marked in ["billing/rules/billing_need.rule", "requirements/billing.req"] {
            assert!(dom.contains(&format!("class=\"pg-file err\" role=\"tab\" aria-selected=\"false\">{marked}")), "{page}: the tab of {marked} is not marked");
        }
        assert!(dom.contains(&format!("aria-selected=\"true\">{contract}")), "{page}: the page does not open on the contract");
        // the list: the projects of this page's language, in their five groups
        let listed: Vec<&str> = dom.match_indices("data-name=\"").map(|(i, m)| &dom[i + m.len()..i + m.len() + dom[i + m.len()..].find('"').unwrap()]).collect();
        let mine: Vec<&str> = projects.iter().filter(|p| p.langs().contains(&tag)).map(|p| p.name.as_str()).collect();
        assert_eq!(listed, mine, "{page}: the list has other projects than projects.json has for this page");
        assert_eq!(dom.matches("<optgroup label=").count(), 5, "{page}: the list does not have its five groups");
        looked += 1;

        // a link to a file, a view and a target
        let flow = "ordering/ship_order.flow";
        let dom = dump_dom(&chrome, &format!("{base}/#project=shop&file={flow}&view=gen&target=temporal"));
        let want = playground::generate(&ask(&shop.files, flow, Some("temporal")));
        assert_eq!(text_in(&dom, "pg-out", "</div>"), want["files"][0]["body"].as_str().unwrap(), "{page}: the page shows another first file of {flow} for Temporal");
        let urgency = "ordering/rules/urgency.rule";
        let dom = dump_dom(&chrome, &format!("{base}/#project=shop&file={urgency}&view=doc"));
        assert!(dom.contains("class=\"pg-open\" href=\"blob:"), "{page}: the page has no link to the page rulec doc draws of {urgency}");
        looked += 2;

        // an example of rulec's page
        let gap = project(if tag == "ja" { "rulec/gap.ja" } else { "rulec/gap" });
        let dom = dump_dom(&chrome, &format!("{base}/#project={}", gap.name));
        let want = playground::check(&ask(&gap.files, "", None));
        assert_eq!(text_in(&dom, "pg-out", "</div>"), said(&want), "{page}: the page shows another check of {}", gap.name);
        assert!(text_in(&dom, "pg-status", "</span>").ends_with(&counts(&want)), "{page}: the status of {} is not its check's", gap.name);
        looked += 1;

        // sekisho's example: the check of its gate, and the first file of the Cedar it writes
        let gate = project(if tag == "ja" { "sekisho/refunds.ja" } else { "sekisho/refunds" });
        let dom = dump_dom(&chrome, &format!("{base}/#project={}", gate.name));
        let want = playground::check(&ask(&gate.files, "", None));
        assert_eq!(text_in(&dom, "pg-out", "</div>"), said(&want), "{page}: the page shows another check of {}", gate.name);
        assert!(dom.contains(&format!("aria-selected=\"true\">{}", gate.open)), "{page}: {} does not open on its gate", gate.name);
        let dom = dump_dom(&chrome, &format!("{base}/#project={}&file={}&view=gen", gate.name, gate.open));
        let want = playground::generate(&ask(&gate.files, &gate.open, None));
        assert_eq!(text_in(&dom, "pg-out", "</div>"), want["files"][0]["body"].as_str().unwrap(), "{page}: the page shows another first file of the Cedar of {}", gate.open);
        looked += 2;

        // the links dandori's page gave: the first draft, a version built for a platform it cannot
        // run on, and the rules tab
        let draft = project(&format!("dandori/{DRAFT}"));
        let dom = dump_dom(&chrome, &format!("{base}/#flow={DRAFT}"));
        let want = playground::check(&ask(&draft.files, "", None));
        assert_eq!(text_in(&dom, "pg-out", "</div>"), said(&want), "{page}: the page shows another check of the first draft");
        assert!(dom.contains(&format!("aria-selected=\"true\">{DRAFT}")), "{page}: the page does not open on the first draft");
        let hotel = if tag == "ja" { "examples/hotel/temporal/hotel.ja.flow" } else { "examples/hotel/temporal/hotel.flow" };
        let version = project(&format!("dandori/{hotel}"));
        let dom = dump_dom(&chrome, &format!("{base}/#flow={hotel}&view=build&target=asl"));
        let want = playground::generate(&ask(&version.files, hotel, Some("asl")));
        assert_ne!(want["code"], 0, "{hotel} is built for Step Functions after all");
        assert!(text_in(&dom, "pg-out", "</div>").ends_with(&said(&want)), "{page}: the page does not say why {hotel} is not built for Step Functions");
        let dom = dump_dom(&chrome, &format!("{base}/#flow={hotel}&view=build"));
        let want = playground::generate(&ask(&version.files, hotel, version.target.as_deref()));
        assert_eq!(text_in(&dom, "pg-out", "</div>"), want["files"][0]["body"].as_str().unwrap(), "{page}: the page does not build {hotel} for the platform its version is for");
        let dom = dump_dom(&chrome, &format!("{base}/#flow={hotel}&view=rules"));
        let first_rule = version.files.iter().map(|(p, _)| p.as_str()).find(|p| p.ends_with(".rule")).unwrap();
        assert!(dom.contains(&format!("aria-selected=\"true\">{first_rule}")) && dom.contains("class=\"pg-open\" href=\"blob:"), "{page}: the rules tab of dandori's page does not open {first_rule} and its page");
        looked += 4;

        // the empty project, which asks for a file
        let dom = dump_dom(&chrome, &format!("{base}/#project=empty"));
        assert!(dom.contains("pg-add pg-cta"), "{page}: the empty project does not point at adding a file");
        looked += 1;

        // a link to the shop as the reader edited it, given by the page, opens it as it was
        let dom = dump_dom(&chrome, &format!("{base}/edit"));
        let hash = shared(&dom);
        assert!(hash.starts_with("#project=shop&") && hash.contains("&edits="), "{page}: the page gives no link to the edited shop: {hash:?}");
        let mut fixed = shop.files.clone();
        fixed.iter_mut().find(|(p, _)| p == contract).unwrap().1 = fixed.iter().find(|(p, _)| p == contract).unwrap().1.replace(returned, "");
        let dom = dump_dom(&chrome, &format!("{base}/{hash}"));
        let want = playground::check(&ask(&fixed, "", None));
        assert_eq!(text_in(&dom, "pg-out", "</div>"), said(&want), "{page}: the link {hash} does not open the shop as it was edited");
        assert!(dom.contains("class=\"pg-revert\" type=\"button\">"), "{page}: the edited shop has no way back to the shop as it opens");
        looked += 1;

        // one file of the reader's own, pasted into the empty project, and the link to it
        let dom = dump_dom(&chrome, &format!("{base}/own"));
        let hash = shared(&dom);
        assert!(hash.starts_with("#project=empty&") && hash.contains("&edits="), "{page}: the page gives no link to the file pasted into the empty project: {hash:?}");
        let dom = dump_dom(&chrome, &format!("{base}/{hash}"));
        let want = playground::check(&ask(&[(own.to_string(), rule.clone())], "", None));
        assert_eq!(text_in(&dom, "pg-out", "</div>"), said(&want), "{page}: the link {hash} does not open the file pasted into the empty project");
        looked += 1;
    }
    eprintln!("compared: {looked} views of the page in Chrome with what the library answers");
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

/// A page that opens `src` in a frame and, once the frame has come to the playground and the
/// playground has answered, writes into itself where the frame came to (`<pre id="landed">`), the
/// tabs of the files (the open one marked `*`), the status and the output, as the frame shows them.
/// Chrome does not dump a page that sends it on (`--dump-dom` waits for the page it was given), so
/// the page sent on is the frame's, and the page that dumps stays.
fn framing(src: &str) -> String {
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"></head><body>
<iframe id="frame" src="{src}" style="width:1200px;height:900px"></iframe>
<script>
(async () => {{
  const f = document.getElementById("frame");
  const answered = () => {{
    const w = f.contentWindow;
    const status = /^\/ritsu\/(ja\/)?playground\/$/.test(w.location.pathname) && w.document.querySelector(".pg-status");
    return status && / ms/.test(status.textContent);
  }};
  for (let i = 0; i < 600; i++) {{
    try {{
      if (answered()) break;
    }} catch (e) {{}}
    await new Promise((r) => setTimeout(r, 50));
  }}
  const w = f.contentWindow, d = w.document;
  const put = (tag, props) => document.body.append(Object.assign(document.createElement(tag), props));
  put("pre", {{ id: "landed", textContent: w.location.pathname + w.location.hash }});
  put("pre", {{ id: "tabs", textContent: [...d.querySelectorAll(".pg-file")].map((b) => (b.classList.contains("on") ? "*" : "") + b.textContent).join("\n") }});
  put("span", {{ className: "pg-status", textContent: (d.querySelector(".pg-status") || {{}}).textContent || "" }});
  put("div", {{ className: "pg-out", innerHTML: (d.querySelector(".pg-out") || {{}}).innerHTML || "" }});
}})();
</script></body></html>"#
    )
}

/// The text of `<pre id="…">` in a page [`framing`] wrote.
fn pre_of(dom: &str, id: &str) -> String {
    let open = format!("<pre id=\"{id}\">");
    dom.split(&open).nth(1).and_then(|r| r.split("</pre>").next()).unwrap_or_default().replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
}

/// The pages the playgrounds of rulec's and dandori's sites had send a reader on to this page, in
/// their language, with the link the reader followed: one that opened something there opens the same
/// here (the links dandori's page gave, `#flow=…&view=…&target=…`, which this page reads), and any
/// other (none, or a heading of the page that was there) opens what that page opened on, rulec's
/// table with a row missing and dandori's first draft. The pages are served where the site publishes
/// them, /ritsu/<site>/playground/ and /ritsu/<site>/ja/playground/, beside this page at
/// /ritsu/playground/ and /ritsu/ja/playground/: a link that climbed out of /ritsu/ would land on no
/// page here, as on the site.
#[test]
fn the_pages_before_send_their_links_on() {
    let Some(chrome) = chrome() else {
        return;
    };
    if !site().join("ritsu.wasm").exists() {
        skip("website/docs/playground/ritsu.wasm is missing; run website/tools/make_wasm.sh");
        return;
    }
    let projects = committed();
    let project = |name: &str| projects.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("projects.json has no {name}")).clone();
    let draft = format!("#flow={DRAFT}");
    let (hotel, hotel_ja) = ("examples/hotel/temporal/hotel.flow", "examples/hotel/temporal/hotel.ja.flow");
    // the page that was there, the link followed to it, and where it should send the reader: the
    // link this page opens on, the project, the file open, and the target of the generator shown
    let visits: Vec<(&str, &str, String, String, String, Option<&str>)> = vec![
        ("/ritsu/rulec/playground/", "", "#project=rulec/gap".into(), "rulec/gap".into(), String::new(), None),
        ("/ritsu/rulec/playground/", "#what-to-try", "#project=rulec/gap".into(), "rulec/gap".into(), String::new(), None),
        ("/ritsu/rulec/ja/playground/", "", "#project=rulec/gap.ja".into(), "rulec/gap.ja".into(), String::new(), None),
        ("/ritsu/dandori/playground/", "", draft.clone(), format!("dandori/{DRAFT}"), DRAFT.into(), None),
        ("/ritsu/dandori/playground/", "#flow=examples/hotel/temporal/hotel.flow&view=build&target=asl", "#flow=examples/hotel/temporal/hotel.flow&view=build&target=asl".into(), format!("dandori/{hotel}"), hotel.into(), Some("asl")),
        ("/ritsu/dandori/ja/playground/", "", draft.clone(), format!("dandori/{DRAFT}"), DRAFT.into(), None),
        ("/ritsu/dandori/ja/playground/", "#flow=examples/hotel/temporal/hotel.ja.flow&view=build", "#flow=examples/hotel/temporal/hotel.ja.flow&view=build".into(), format!("dandori/{hotel_ja}"), hotel_ja.into(), Some("temporal")),
    ];
    let mut pages: Vec<(String, String)> = Vec::new();
    for (at, page) in [("/ritsu/playground/", "website/docs/playground.md"), ("/ritsu/ja/playground/", "website/docs-ja/playground.md")] {
        pages.push((at.to_string(), format!("<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{}</body></html>", widget_of(page))));
    }
    for s in ["rulec", "dandori"] {
        pages.push((format!("/ritsu/{s}/playground/"), page_before(s, "en")));
        pages.push((format!("/ritsu/{s}/ja/playground/"), page_before(s, "ja")));
    }
    for (i, (before, link, _, _, _, _)) in visits.iter().enumerate() {
        pages.push((format!("/visit/{i}"), framing(&format!("{before}{link}"))));
    }
    let base = format!("http://127.0.0.1:{}", serve(pages));
    for (i, (before, link, lands, name, open, target)) in visits.iter().enumerate() {
        let tag = if before.contains("/ja/") { "ja" } else { "en" };
        let dom = dump_dom(&chrome, &format!("{base}/visit/{i}"));
        let came = format!("{}{lands}", if tag == "ja" { "/ritsu/ja/playground/" } else { "/ritsu/playground/" });
        assert_eq!(pre_of(&dom, "landed"), came, "{before}{link} sends the reader elsewhere");
        let p = project(name);
        let open = if open.is_empty() { p.open.clone() } else { open.clone() };
        assert!(pre_of(&dom, "tabs").lines().any(|t| t == format!("*{open}")), "{before}{link} does not open {name} on {open}");
        let shown = text_in(&dom, "pg-out", "</div>");
        match target {
            None => {
                let want = playground::check(&ask_in(tag, &p.files, "", None));
                assert_eq!(shown, said(&want), "{before}{link} opens another check than that of {name}");
                assert!(text_in(&dom, "pg-status", "</span>").ends_with(&counts_in(tag, &want)), "{before}{link}: the status is not that of the check of {name}");
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
    eprintln!("compared: {} links to the pages rulec's and dandori's playgrounds had, sent on to the page in Chrome, with what the library answers", visits.len());
}
