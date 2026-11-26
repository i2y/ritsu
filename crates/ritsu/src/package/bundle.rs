//! `--extra bundle` (ringi's DESIGN 12.6): `<out>/requests/` in one zip, `<out>/requests.zip`, for a
//! server to take a version of the kinds of request with no build of its own. The zip holds every
//! file the run writes into `<out>/requests/` and nothing else there: the kinds' descriptions, the
//! API, the tools, the organization's schema, the vectors, the screens, `wasm.json` and the module
//! (`--extra wasm`, which a bundle needs), and for the people who read what changed, the files the
//! kinds are made from (`source/`), with `bundle.json`, which lists them, and with `--diff-from`,
//! what `ringi diff` says of each kind against the bundle before (`diff/`). It is written by
//! [`super::zip`], so that the same files give the same bytes and the same digest.
//!
//! - `source/<path from the root>`: each kind's `.ringi`, and every file of a language it reads,
//!   followed through what each of those reads (the index of the project, DESIGN 6.4): the
//!   organization's and the masters' `.ringi`, a shared route, the rules, the calendars, the books,
//!   the flows and the gates. The data of the organization and of the masters is read at the
//!   server, and is not held.
//! - `diff/<name>.en.md`, `diff/<name>.ja.md`: for each kind of the bundle before that has the same
//!   name, `ringi diff` of its `.ringi` there (as its `source/` holds it, with what it reads) and the
//!   one now, as Markdown in each language, as a page writes it.
//! - `bundle.json`: the head, the version of its shape, each file of `source/` with its digest, and
//!   what was compared (the digest of the zip before, and each kind's two versions, whether they
//!   differ and the pages), or null.

use super::{Refusal, refusal};
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_project::{Joined, Project};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The zip, in `<out>/`, beside `<out>/requests/`.
pub const ZIP: &str = "requests.zip";

/// What lists the files of `source/` and `diff/`, in `<out>/requests/`.
pub const ABOUT: &str = "bundle.json";

/// The directory of the files the kinds are made from, and that of what changed.
pub const SOURCE: &str = "source";
pub const DIFF: &str = "diff";

/// The version of the shape of `bundle.json`.
pub const SHAPE: u32 = 1;

/// The files the kinds of request are made from, by their paths from the project's root, in the
/// order of the bytes of their paths: each kind's `.ringi`, and each file of a language that one
/// reads, and so on.
pub fn sources(project: &Project, joined: &Joined, kinds: &[ritsu_project::requests::Kind], said: Lang) -> Result<Vec<(String, Vec<u8>)>, Refusal> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut todo: Vec<String> = kinds.iter().map(|k| k.request.rel.clone()).collect();
    while let Some(rel) = todo.pop() {
        let Some(tool) = ritsu_project::project::kind_of(&rel) else { continue };
        if !seen.insert(rel.clone()) {
            continue;
        }
        if let Some(Ok(reads)) = joined.index.reads(tool, &project.root, &rel) {
            todo.extend(reads.into_iter().map(|r| r.target.path).filter(|p| ritsu_project::project::kind_of(p).is_some() && !seen.contains(p)));
        }
    }
    let mut out = Vec::new();
    for rel in seen {
        // a file outside the root has no path in the bundle a server could take safely
        if ritsu_base::modules::tar::path_why(&rel).is_some() {
            return Err(refusal(
                2,
                tr!(
                    "申請の種類が `{rel}` を読みますが、バンドルにはルートの下のファイルしか入りません。`--root` を、読むファイルが全部その下に来るディレクトリにしてください",
                    "a kind of request reads `{rel}`, and a bundle holds only files under the root: give a `--root` that every file it reads is under"
                ),
                said,
            ));
        }
        let at = ritsu_base::paths::absolute(&project.root.join(&rel));
        let bytes = ritsu_base::fs::read(&at).map_err(|e| refusal(2, ritsu_base::cli::cannot_read(&rel, &e), said))?;
        out.push((format!("{SOURCE}/{rel}"), bytes));
    }
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(out)
}

/// The bundle a run compares with (`--diff-from`): its files by their paths, and the digest of its
/// zip (None for a directory).
pub struct Base {
    files: BTreeMap<String, Vec<u8>>,
    digest: Option<String>,
}

/// The bundle at `path`: a zip ritsu wrote, or a directory a bundle is in (`<out>/requests/` of an
/// earlier run, or a zip opened). Err with why it is not one.
pub fn base(path: &str, said: Lang) -> Result<Base, Refusal> {
    let p = Path::new(path);
    let not = |why: Text| refusal(2, tr!("`--diff-from {path}` はバンドルとして読めません: ", "`--diff-from {path}` cannot be read as a bundle: ").then(&why), said);
    if ritsu_base::fs::is_dir(p) {
        let mut files = BTreeMap::new();
        let mut todo = vec![PathBuf::from(p)];
        while let Some(d) = todo.pop() {
            let rd = std::fs::read_dir(&d).map_err(|e| refusal(2, ritsu_base::cli::cannot_read(&d.display().to_string(), &e), said))?;
            for e in rd.flatten() {
                let q = e.path();
                if q.is_dir() {
                    todo.push(q);
                } else if let Ok(rel) = q.strip_prefix(p) {
                    let rel = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
                    let bytes = std::fs::read(&q).map_err(|e| refusal(2, ritsu_base::cli::cannot_read(&q.display().to_string(), &e), said))?;
                    files.insert(rel, bytes);
                }
            }
        }
        if !files.contains_key(super::wasm::ABOUT) {
            return Err(not(no_about()));
        }
        return Ok(Base { files, digest: None });
    }
    let bytes = std::fs::read(p).map_err(|e| refusal(2, ritsu_base::cli::cannot_read(path, &e), said))?;
    let files = super::zip::read(&bytes).map_err(not)?;
    let files: BTreeMap<String, Vec<u8>> = files.into_iter().collect();
    if !files.contains_key(super::wasm::ABOUT) {
        return Err(not(no_about()));
    }
    Ok(Base { files, digest: Some(ritsu_base::sha256::hex(&bytes)) })
}

/// Why a zip or a directory is no bundle: it holds no `wasm.json`.
fn no_about() -> Text {
    let f = super::wasm::ABOUT;
    tr!("`{f}` がありません", "it holds no `{f}`")
}

/// What `ringi diff` says of one kind against the bundle before.
pub struct Compared {
    pub name: String,
    pub from: u32,
    pub to: u32,
    pub differs: bool,
    /// The page in English and in Japanese.
    pub en: String,
    pub ja: String,
}

/// Each kind of `kinds` the bundle before has too, by its name, compared with it: the `.ringi` there,
/// read with what it reads as the bundle's `source/` holds them, and the one now. A kind the bundle
/// before does not have is not compared. Err when a side cannot be compared (one that does not pass
/// its check, say).
pub fn compare(base: &Base, project: &Project, kinds: &[ritsu_project::requests::Kind], said: Lang) -> Result<Vec<Compared>, Refusal> {
    let mut out = Vec::new();
    for k in kinds {
        let Some(old) = base.files.get(&format!("types/{}.json", k.name)) else { continue };
        let Some(described) = std::str::from_utf8(old).ok().and_then(|t| ritsu_base::json::parse(t).ok()) else { continue };
        let (Some(file), Some(from)) = (described.get("file").and_then(Json::as_str), described.get("version").and_then(Json::as_int)) else { continue };
        // the bundle before, its files of `source/` laid at their paths from the root over the
        // disk: the data of the organization and of the masters, which a bundle does not hold, are
        // read as they are now, as the server reads them
        let root = ritsu_base::paths::absolute(&project.root);
        let prefix = format!("{SOURCE}/");
        if !base.files.contains_key(&format!("{prefix}{file}")) {
            continue;
        }
        let mut laid = ritsu_base::fs::Overlay::new(None);
        for (p, bytes) in &base.files {
            if let Some(rel) = p.strip_prefix(&prefix) {
                laid.put(&root.join(rel), bytes.clone());
            }
        }
        let files: std::rc::Rc<dyn ritsu_base::fs::Files> = std::rc::Rc::new(laid);
        let old_side = ringi::diff::Side { read: root.join(file).to_string_lossy().into_owned(), files: Some(files), shown: file.to_string() };
        let new_side = ringi::diff::Side { read: ritsu_base::paths::absolute(&project.root.join(&k.request.rel)).to_string_lossy().into_owned(), files: None, shown: k.request.rel.clone() };
        let mut pages = Vec::new();
        let mut differs = false;
        for made in [Lang::En, Lang::Ja] {
            // a run of its own, so that nothing the run of gen keeps of its files is read for the
            // other side
            let joined = Joined::new();
            let rl = if made == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
            let d = rulec::i18n::with(rl, || ringi::diff::diff(&old_side, &new_side, &joined.ringi(), &ringi::check::Options::default(), None, made, made)).map_err(|r| {
                let why = match r {
                    ringi::diff::Refused::Said(t) => t,
                    ringi::diff::Refused::Check { shown, errors, .. } => {
                        let n = errors.max(1);
                        tr!("`{shown}` は検査を通りません（エラー {n} 件）", "`{shown}` does not pass check ({n} {})", ; if n == 1 { "error" } else { "errors" })
                    }
                };
                let name = &k.name;
                refusal(2, tr!("`--diff-from` の種類 `{name}` を比べられません: ", "the kind `{name}` cannot be compared with `--diff-from`: ").then(&why), said)
            })?;
            differs |= d.differs();
            pages.push(ringi::diff::markdown(&d));
        }
        let ja = pages.pop().unwrap_or_default();
        let en = pages.pop().unwrap_or_default();
        out.push(Compared { name: k.name.clone(), from: u32::try_from(from).unwrap_or(0), to: k.version, differs, en, ja });
    }
    Ok(out)
}

/// The pages of what changed, by their paths under `<out>/requests/`.
pub fn pages(compared: &[Compared]) -> Vec<(String, String)> {
    compared.iter().flat_map(|c| [(format!("{DIFF}/{}.en.md", c.name), c.en.clone()), (format!("{DIFF}/{}.ja.md", c.name), c.ja.clone())]).collect()
}

/// `bundle.json`: the head, the version of its shape, each file of `source/` with its digest, and
/// what was compared, or null.
pub fn about(sources: &[(String, Vec<u8>)], compared: Option<(&Base, &[Compared])>) -> String {
    let diff = match compared {
        None => Json::Null,
        Some((base, cs)) => Json::obj([
            ("base", base.digest.as_deref().map(Json::str).unwrap_or(Json::Null)),
            (
                "kinds",
                Json::arr(cs.iter().map(|c| {
                    Json::obj([
                        ("name", Json::str(&c.name)),
                        ("from", Json::int(i128::from(c.from))),
                        ("to", Json::int(i128::from(c.to))),
                        ("differs", Json::Bool(c.differs)),
                        ("en", Json::str(format!("{DIFF}/{}.en.md", c.name))),
                        ("ja", Json::str(format!("{DIFF}/{}.ja.md", c.name))),
                    ])
                })),
            ),
        ]),
    };
    let j = Json::obj([
        ("//", Json::str(ritsu_emit::header::generated("ritsu"))),
        ("bundle", Json::int(i128::from(SHAPE))),
        ("sources", Json::arr(sources.iter().map(|(p, b)| Json::obj([("path", Json::str(p)), ("sha256", Json::str(ritsu_base::sha256::hex(b)))])))),
        ("diff", diff),
    ]);
    format!("{}\n", j.pretty())
}

/// The files of `<out>/requests/` (`dir`) an earlier run wrote for an extra, by the extra: `wasm`'s
/// module and `wasm.json`, and `bundle`'s `bundle.json` with each file it lists under `source/` and
/// `diff/`; with `<out>/requests.zip` (`zip`) for `bundle` too. Only those that are there.
pub fn written_before(dir: &Path, zip: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    for f in [super::wasm::ABOUT, super::wasm::MODULE] {
        if dir.join(f).is_file() {
            out.push(("wasm", dir.join(f)));
        }
    }
    let about = dir.join(ABOUT);
    if about.is_file() {
        out.push(("bundle", about.clone()));
        for rel in listed(&about) {
            if dir.join(&rel).is_file() {
                out.push(("bundle", dir.join(rel)));
            }
        }
    }
    if zip.is_file() {
        out.push(("bundle", zip.to_path_buf()));
    }
    out
}

/// The files `bundle.json` at `about` lists, by their paths under its directory: each of
/// `sources`, and each page of `diff`. Only paths under `source/` and `diff/`, each a path a bundle
/// holds, so that nothing else is taken away for them.
pub fn listed(about: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(about) else { return Vec::new() };
    let Ok(j) = ritsu_base::json::parse(&text) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for s in j.get("sources").and_then(Json::as_arr).unwrap_or_default() {
        if let Some(p) = s.get("path").and_then(Json::as_str) {
            out.push(p.to_string());
        }
    }
    for k in j.get("diff").and_then(|d| d.get("kinds")).and_then(Json::as_arr).unwrap_or_default() {
        for lang in ["en", "ja"] {
            if let Some(p) = k.get(lang).and_then(Json::as_str) {
                out.push(p.to_string());
            }
        }
    }
    out.retain(|p| (p.starts_with(&format!("{SOURCE}/")) || p.starts_with(&format!("{DIFF}/"))) && ritsu_base::modules::tar::path_why(p).is_none());
    out.sort();
    out.dedup();
    out
}
