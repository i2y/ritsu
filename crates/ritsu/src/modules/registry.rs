//! What `ritsu mod` asks of a registry (DESIGN 17.12): the requests of the distribution-spec it
//! sends ([`Remote`]), and the commands that are nothing but those: `add` with a repository, `get`,
//! and `versions`, which ask a git repository the same of a dependency from git (DESIGN 17.12.1,
//! with [`super::git`]). The table reads their command lines before they are called, and sends `add`
//! here when its source is not a directory ([`super::cli::is_path`]).
//!
//! A registry is asked over https, a host of the loopback over plain HTTP (a registry a test
//! starts), and `docker.io` at `registry-1.docker.io` (a name of one part there is under `library/`).
//! A tag is looked up with an OCI manifest's `GET` (`HEAD` for whether it is there), the versions
//! with `GET /v2/<name>/tags/list?n=1000` and each `Link` with `rel="next"` after it, of which only
//! the tags that are versions count (a signature's old tag, `sha256-….sig`, is none). `@latest` is
//! the highest version that is not a prerelease; a registry with prereleases alone has none, and the
//! command asks for a version (exit 2). The versions of a git repository are its tags `v<version>`
//! (`<dir>/v<version>`), and `add` and `get` take the commit the tag leads to now.

use super::git::{Failed, Git};
use super::http::{self, Body, NoAnswer, Request, Response};
use super::{Ctx, auth, cli, edit, store_write};
use ritsu_base::cli::Args;
use ritsu_base::json::{self, Json};
use ritsu_base::modules::artifact;
use ritsu_base::modules::git as gitbase;
use ritsu_base::modules::graph::Origin;
use ritsu_base::modules::lock::{self as lockfile, Pinned};
use ritsu_base::modules::manifest::{self, Source};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::path::{Path, PathBuf};

/// The most bytes a page of tags may have, and an answer with nothing to read.
const MAX_TAGS: u64 = 16 * 1024 * 1024;
const SMALL: u64 = 1024 * 1024;

/// A repository of a registry: the module of a published name.
pub struct Remote {
    /// The registry as the name writes it (`127.0.0.1:5000`, `example.com`).
    pub registry: String,
    /// The repository as the registry's API names it.
    pub repository: String,
    /// `https://<registry>`, or `http://` for the loopback.
    pub base: String,
    tmp: PathBuf,
    /// `pull`, or `pull,push`.
    actions: &'static str,
}

/// The host of a registry as a name writes it, without its port.
fn host_of(registry: &str) -> String {
    ritsu_base::urls::scheme_and_host(&format!("http://{registry}/")).map(|(_, h)| h).unwrap_or_else(|| registry.to_string())
}

/// Whether the registry of a published name is on the loopback: asked over plain HTTP, and named to
/// cosign with `--allow-http-registry`.
pub fn on_loopback(name: &ModuleName) -> bool {
    match name {
        ModuleName::Published { registry, .. } => ritsu_base::urls::is_loopback(&host_of(registry)),
        ModuleName::Local(_) => false,
    }
}

impl Remote {
    /// The repository of the module `name` (a published one), to pull from or to push to too.
    pub fn of(name: &ModuleName, push: bool) -> Result<Remote, Text> {
        let ModuleName::Published { registry, repository } = name else {
            return Err(tr!(
                "`{name}` は配る名前ではないので、レジストリにありません。配る名前は `<レジストリ>/<リポジトリ>` です",
                "`{name}` is no published name, so no registry has it; a published name is `<registry>/<repository>`"
            ));
        };
        let loopback = on_loopback(name);
        let (base, repository) = if registry == "docker.io" {
            let r = if repository.contains('/') { repository.clone() } else { format!("library/{repository}") };
            ("https://registry-1.docker.io".to_string(), r)
        } else if loopback {
            (format!("http://{registry}"), repository.clone())
        } else {
            (format!("https://{registry}"), repository.clone())
        };
        Ok(Remote { registry: registry.clone(), repository, base, tmp: store_write::scratch()?, actions: if push { "pull,push" } else { "pull" } })
    }

    fn url(&self, rest: &str) -> String {
        format!("{}/v2/{}/{rest}", self.base, self.repository)
    }

    /// A request sent with the credentials the registry asks for, when it goes to the registry's own
    /// scheme, host and port; with none elsewhere (an upload's `Location` on another host).
    fn send(&self, req: Request<'_>) -> Result<Response, NoAnswer> {
        if http::origin(&req.url) == http::origin(&self.base) {
            auth::send(&self.registry, &self.repository, self.actions, req, &self.tmp)
        } else {
            http::send(&req, &self.tmp)
        }
    }

    /// The bytes of the OCI manifest of a tag or a digest; none when the registry has none. An
    /// answer over the most an OCI manifest may have is [`NoAnswer::Over`].
    pub fn manifest(&self, reference: &str) -> Result<Option<Vec<u8>>, NoAnswer> {
        let r = self.send(Request::new("GET", self.url(&format!("manifests/{reference}")), artifact::MAX_MANIFEST).header("Accept", artifact::MANIFEST_TYPE))?;
        match r.status {
            200 => Ok(Some(r.body)),
            404 => Ok(None),
            _ => Err(r.error("GET").into()),
        }
    }

    /// Whether the registry has a tag or a digest (an OCI manifest's `HEAD`).
    pub fn has_manifest(&self, reference: &str) -> Result<bool, Text> {
        let r = self.send(Request::new("HEAD", self.url(&format!("manifests/{reference}")), SMALL).header("Accept", artifact::MANIFEST_TYPE))?;
        match r.status {
            200 => Ok(true),
            404 => Ok(false),
            _ => Err(r.error("HEAD")),
        }
    }

    /// The bytes of a blob, `size` of them at most: more is [`NoAnswer::Over`].
    pub fn blob(&self, digest: &str, size: u64) -> Result<Vec<u8>, NoAnswer> {
        let r = self.send(Request::new("GET", self.url(&format!("blobs/{digest}")), size))?;
        if r.status != 200 {
            return Err(r.error("GET").into());
        }
        Ok(r.body)
    }

    /// Whether the registry has a blob.
    pub fn has_blob(&self, digest: &str) -> Result<bool, Text> {
        let r = self.send(Request::new("HEAD", self.url(&format!("blobs/{digest}")), SMALL))?;
        match r.status {
            200 => Ok(true),
            404 => Ok(false),
            _ => Err(r.error("HEAD")),
        }
    }

    /// A blob sent: `POST` starts the upload, a `PUT` to the `Location` it gives with the digest
    /// ends it. An upload that does not end is started over once.
    pub fn upload(&self, digest: &str, bytes: &[u8]) -> Result<(), Text> {
        let mut last = None;
        for _ in 0..2 {
            let start = self.send(Request::new("POST", self.url("blobs/uploads/"), SMALL).header("Content-Length", "0"))?;
            if start.status != 202 {
                return Err(start.error("POST"));
            }
            let Some(at) = start.header("Location") else {
                let u = http::without_query(&start.url).to_string();
                return Err(tr!("`POST {u}` の答えに `Location` がありません", "the answer to `POST {u}` has no `Location`"));
            };
            let to = http::resolve(&start.url, at);
            let sep = if to.contains('?') { '&' } else { '?' };
            let mut req = Request::new("PUT", format!("{to}{sep}digest={digest}"), SMALL).header("Content-Type", "application/octet-stream");
            req.body = Body::Bytes(bytes);
            match self.send(req) {
                Ok(r) if r.status == 201 => return Ok(()),
                Ok(r) => last = Some(r.error("PUT")),
                Err(e) => last = Some(e.into()),
            }
        }
        Err(last.unwrap_or_default())
    }

    /// An OCI manifest put under a tag: the digest the registry gives in `Docker-Content-Digest`,
    /// when it gives one.
    pub fn put_manifest(&self, tag: &str, bytes: &[u8]) -> Result<Option<String>, Text> {
        let mut req = Request::new("PUT", self.url(&format!("manifests/{tag}")), SMALL).header("Content-Type", artifact::MANIFEST_TYPE);
        req.body = Body::Bytes(bytes);
        let r = self.send(req)?;
        if r.status != 201 && r.status != 200 {
            return Err(r.error("PUT"));
        }
        Ok(r.header("Docker-Content-Digest").map(str::to_string))
    }

    /// Every tag, a page of a thousand at a time; none for a repository the registry does not know.
    pub fn tags(&self) -> Result<Vec<String>, Text> {
        let mut url = self.url("tags/list?n=1000");
        let mut out = Vec::new();
        for _ in 0..10_000 {
            let r = self.send(Request::new("GET", url.clone(), MAX_TAGS))?;
            if r.status == 404 {
                return Ok(out);
            }
            if r.status != 200 {
                return Err(r.error("GET"));
            }
            let j = std::str::from_utf8(&r.body).ok().and_then(|s| json::parse(s).ok());
            let Some(j) = j else {
                let u = http::without_query(&r.url).to_string();
                return Err(tr!("`GET {u}` の答えを JSON として読めません", "the answer to `GET {u}` does not read as JSON"));
            };
            out.extend(j.get("tags").and_then(Json::as_arr).unwrap_or(&[]).iter().filter_map(Json::as_str).map(str::to_string));
            match r.header("Link").and_then(next_link) {
                Some(next) => url = http::resolve(&r.url, &next),
                None => return Ok(out),
            }
        }
        Ok(out)
    }

    /// The versions among the tags, lowest first.
    pub fn versions(&self) -> Result<Vec<Version>, Text> {
        Ok(versions_of(&self.tags()?))
    }
}

/// The target of a `Link` whose `rel` is `next`.
fn next_link(header: &str) -> Option<String> {
    header.split(',').find_map(|part| {
        let (target, params) = part.trim().split_once(';')?;
        let target = target.trim().strip_prefix('<')?.strip_suffix('>')?;
        params.split(';').any(|p| p.trim().replace(' ', "") == "rel=\"next\"" || p.trim() == "rel=next").then(|| target.to_string())
    })
}

/// The tags that are versions, lowest first, each once.
pub fn versions_of(tags: &[String]) -> Vec<Version> {
    let mut vs: Vec<Version> = tags.iter().filter_map(|t| Version::parse(t).ok()).collect();
    vs.sort();
    vs.dedup();
    vs
}

/// The highest version that is not a prerelease.
pub fn latest(vs: &[Version]) -> Option<&Version> {
    vs.iter().rev().find(|v| !v.is_prerelease())
}

/// The versions of `module` from `origin`, lowest first: the tags of its registry, or of its git
/// repository.
fn versions_from(module: &ModuleName, origin: &Origin, git: &mut Git) -> Result<Vec<Version>, Text> {
    match origin {
        Origin::Oci => Remote::of(module, false)?.versions(),
        Origin::Git { url, dir } => Ok(git.versions(url, dir.as_deref()).map_err(Failed::text)?.into_iter().map(|(v, _)| v).collect()),
    }
}

/// What a command says when `origin` has no version of `module`.
fn no_versions(module: &ModuleName, origin: &Origin) -> Text {
    match origin {
        Origin::Oci => tr!("レジストリに `{module}` の版がありません", "the registry has no version of `{module}`"),
        Origin::Git { url, dir } => {
            let (ja, en) = match dir {
                Some(d) => (format!("{d}/v<版>"), format!("{d}/v<version>")),
                None => ("v<版>".to_string(), "v<version>".to_string()),
            };
            tr!(
                "git のリポジトリ `{url}` に、`{module}` の版のタグ（`{}`）がありません",
                "the git repository `{url}` has no tag of a version of `{module}` (`{}`)",
                ja; en
            )
        }
    }
}

/// The version `asked` names of `module` from `origin`: itself, or for `latest`, the latest of its
/// registry or of its git repository's tags.
fn version_asked(module: &ModuleName, origin: &Origin, asked: &str, git: &mut Git) -> Result<Version, Text> {
    if asked != "latest" {
        return Version::parse(asked).map_err(|e| e.text(asked));
    }
    let vs = versions_from(module, origin, git)?;
    match latest(&vs) {
        Some(v) => Ok(v.clone()),
        None if vs.is_empty() => Err(no_versions(module, origin)),
        None => Err(tr!(
            "`{module}` にはプレリリースの版しかありません。`@<版>` で版を書いてください",
            "`{module}` has prereleases alone; write the version, `@<version>`"
        )),
    }
}

/// The source of a `require … git`, with its module's name: what is wrong with it (E302's sentence)
/// when its URL or its `dir` is not one ritsu takes.
fn git_source(url: &str, dir: Option<&str>) -> Result<(ModuleName, Origin), Text> {
    let module = gitbase::name_of(url, dir).map_err(|p| p.text)?;
    Ok((module, Origin::Git { url: url.to_string(), dir: dir.map(str::to_string) }))
}

/// The git repository the tree of the module at `root` takes `name` from, if it does: its lock's
/// line, else a `require` of the module at hand.
pub fn git_origin(root: &Path, name: &ModuleName) -> Option<Origin> {
    let pinned = std::fs::read(root.join(lockfile::FILE)).ok().and_then(|b| lockfile::parse(&b, lockfile::FILE).ok()).and_then(|l| l.get(name).map(|l| l.pinned.clone()));
    match pinned {
        Some(p @ Pinned::Git { .. }) => return Origin::of_pinned(&p, name),
        Some(_) => return None,
        None => {}
    }
    let m = std::fs::read(root.join(manifest::FILE)).ok().and_then(|b| manifest::parse(&b, manifest::FILE).ok())?;
    m.requires.iter().find(|r| matches!(r.source, Source::Git { .. }) && r.source.module().as_ref() == Some(name)).and_then(|r| Origin::of(&r.source))
}

/// The module a name on the command line names, and where it comes from: a dependency of the module
/// at hand from a registry or a git repository, by its name; or a module by its published name, from
/// the git repository the module at hand's tree takes it from (its lock, else its `require`), else
/// from its registry (DESIGN 17.9.1).
pub fn module_named(ctx: &Ctx, name: &str) -> Result<(ModuleName, Origin), Text> {
    let root = ctx.root().ok();
    if let Some(root) = &root
        && let Ok(bytes) = std::fs::read(root.join(manifest::FILE))
        && let Ok(m) = manifest::parse(&bytes, manifest::FILE)
        && let Some(r) = m.require(name)
    {
        return match &r.source {
            Source::Oci { repository, .. } => ModuleName::parse(repository).map(|n| (n, Origin::Oci)).map_err(|e| e.text()),
            Source::Git { url, dir, .. } => git_source(url, dir.as_deref()),
            Source::Path { path } => Err(tr!(
                "依存 `{name}` はパス `{path}` から読むので、レジストリにも git のリポジトリにもありません",
                "the dependency `{name}` is read from the path `{path}`, so neither a registry nor a git repository has it"
            )),
        };
    }
    match ModuleName::parse(name) {
        Ok(m) if m.is_published() => {
            let origin = root.as_deref().and_then(|r| git_origin(r, &m)).unwrap_or(Origin::Oci);
            Ok((m, origin))
        }
        _ => Err(tr!(
            "`{name}` は、レジストリか git のリポジトリから取る依存の名前でも、配るモジュールの名前（`<レジストリ>/<リポジトリ>`）でもありません",
            "`{name}` is neither the name of a dependency from a registry or a git repository nor a published module's name (`<registry>/<repository>`)"
        )),
    }
}

/// A git source of `add`, split into its URL and the version written after its last `@`: the part
/// after the last `@` is a version (or `latest`) when it has neither a `/` nor a `:`, since the `@`
/// of an ssh URL's user is followed by its host and path (DESIGN 17.19).
pub fn git_and_version(source: &str) -> (&str, &str) {
    match source.rsplit_once('@') {
        Some((url, v)) if !v.contains('/') && !v.contains(':') => (url, v),
        _ => (source, "latest"),
    }
}

/// What `add` says of `--dir` with a source that is not a git URL (exit 2).
pub fn dir_without_git() -> Text {
    tr!(
        "`--dir` は、git のリポジトリから取る依存にだけ付けられます（リポジトリの中の、モジュールのディレクトリ）",
        "`--dir` goes with a dependency from a git repository alone: the module's directory in the repository"
    )
}

/// `ritsu mod add <name> <repository>[@<version>|@latest]` and `<name> <git URL>[@<version>|@latest]
/// [--dir <dir>]`: the exit code. `a.pos` holds the name and the source; `add` with a directory is
/// [`super::edit::add`].
pub fn add(ctx: &Ctx, a: &Args) -> u8 {
    let (name, written) = (a.pos[0].as_str(), a.pos[1].as_str());
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let dir = a.get("--dir");
    let git_url = cli::is_git(written);
    if dir.is_some() && !git_url {
        return ctx.refuse(dir_without_git());
    }
    let (module, origin, asked) = if git_url {
        let (url, asked) = git_and_version(written);
        match git_source(url, dir) {
            Ok((m, o)) => (m, o, asked),
            Err(t) => return ctx.refuse(t),
        }
    } else {
        let (repository, asked) = written.rsplit_once('@').unwrap_or((written, "latest"));
        match ModuleName::parse(repository) {
            Ok(m) if m.is_published() => (m, Origin::Oci, asked),
            _ => {
                return ctx.refuse(tr!(
                    "`{repository}` は配るモジュールの名前でも、git の URL でもありません。レジストリの依存は `<レジストリ>/<リポジトリ>`、git の依存は `https://…` か `ssh://…` か `<ユーザー>@<ホスト>:<パス>`、パスの依存は `./…` か `../…` で書いてください",
                    "`{repository}` is neither a published module's name nor a git URL: a dependency from a registry is `<registry>/<repository>`, one from git `https://…`, `ssh://…` or `<user>@<host>:<path>`, and one by a path starts with `./` or `../`"
                ));
            }
        }
    };
    let file = root.join(manifest::FILE);
    let (before, m) = match edit::read(ctx, &root, &file) {
        Ok(x) => x,
        Err(code) => return code,
    };
    // the name is checked before the registry or the repository is asked
    if let Err(e) = m.added(name, &origin.source(&module, &Version::new(0, 0, 0))) {
        return ctx.refuse(e.text());
    }
    let mut git = Git::new();
    let version = match version_asked(&module, &origin, asked, &mut git) {
        Ok(v) => v,
        Err(t) => return ctx.refuse(t),
    };
    let source = origin.source(&module, &version);
    let text = match m.added(name, &source) {
        Ok(t) => t,
        Err(e) => return ctx.refuse(e.text()),
    };
    let plan = match edit::change(ctx, &root, &file, &before, &text, Some(&module), &mut git) {
        Ok(p) => p,
        Err(code) => return code,
    };
    if ctx.json() {
        println!("{}", edit::added_json(ctx, &root, name, &module.to_string(), Some(version.to_string()), &source, plan.done).pretty());
        return 0;
    }
    let line = format!("require {name} {}", source.words());
    println!("{}", ctx.say(&tr!("ritsu.mod に `{line}` を足しました（{module} {version}）", "added `{line}` to ritsu.mod ({module} {version})")));
    edit::lock_line(ctx, &plan);
    0
}

/// `ritsu mod get <name>@<version>` and `<name>@latest`: the exit code.
pub fn get(ctx: &Ctx, a: &Args) -> u8 {
    let arg = a.pos[0].as_str();
    let Some((name, asked)) = arg.rsplit_once('@') else {
        return ctx.refuse(tr!("`{arg}` に版がありません。`<名前>@<版>` か `<名前>@latest` で書いてください", "`{arg}` has no version; write `<name>@<version>` or `<name>@latest`"));
    };
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let file = root.join(manifest::FILE);
    let (before, m) = match edit::read(ctx, &root, &file) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let (module, origin, old) = match m.require(name).map(|r| &r.source) {
        Some(Source::Oci { repository, min }) => match ModuleName::parse(repository) {
            Ok(n) => (n, Origin::Oci, min.clone()),
            Err(e) => return ctx.refuse(e.text()),
        },
        Some(Source::Git { url, min, dir }) => match git_source(url, dir.as_deref()) {
            Ok((n, o)) => (n, o, min.clone()),
            Err(t) => return ctx.refuse(t),
        },
        Some(Source::Path { .. }) => return ctx.refuse(manifest::EditError::ByPath(name.to_string()).text()),
        None => return ctx.refuse(manifest::EditError::NotRequired(name.to_string()).text()),
    };
    let mut git = Git::new();
    let version = match version_asked(&module, &origin, asked, &mut git) {
        Ok(v) => v,
        Err(t) => return ctx.refuse(t),
    };
    let text = match m.with_version(name, &version) {
        Ok(t) => t,
        Err(e) => return ctx.refuse(e.text()),
    };
    let plan = match edit::change(ctx, &root, &file, &before, &text, Some(&module), &mut git) {
        Ok(p) => p,
        Err(code) => return code,
    };
    if ctx.json() {
        let source = origin.source(&module, &version);
        println!("{}", edit::added_json(ctx, &root, name, &module.to_string(), Some(version.to_string()), &source, plan.done).pretty());
        return 0;
    }
    if old == version {
        println!("{}", ctx.say(&tr!("依存 `{name}` は、もう {version} です", "the dependency `{name}` is at {version} already")));
    } else {
        println!("{}", ctx.say(&tr!("依存 `{name}` の版を {old} から {version} に変えました（{module}）", "changed the version of the dependency `{name}` from {old} to {version} ({module})")));
    }
    edit::lock_line(ctx, &plan);
    0
}

/// `ritsu mod versions <name>` and `<git URL> [--dir <dir>]`: the exit code.
pub fn versions(ctx: &Ctx, a: &Args) -> u8 {
    let name = a.pos[0].as_str();
    let dir = a.get("--dir");
    let named = if cli::is_git(name) {
        git_source(name, dir)
    } else if dir.is_some() {
        return ctx.refuse(tr!(
            "`--dir` は、git の URL にだけ付けられます（リポジトリの中の、モジュールのディレクトリ）",
            "`--dir` goes with a git URL alone: the module's directory in the repository"
        ));
    } else {
        module_named(ctx, name)
    };
    let (module, origin) = match named {
        Ok(m) => m,
        Err(t) => return ctx.refuse(t),
    };
    let vs = match versions_from(&module, &origin, &mut Git::new()) {
        Ok(v) => v,
        Err(t) => return ctx.refuse(t),
    };
    let top = latest(&vs).map(Version::to_string);
    if ctx.json() {
        let o = Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("module", Json::str(module.to_string())),
            ("versions", Json::arr(vs.iter().map(|v| Json::str(v.to_string())))),
            ("latest", Json::opt_str(top)),
        ]);
        println!("{}", o.pretty());
        return 0;
    }
    if vs.is_empty() {
        let none = match origin {
            Origin::Oci => tr!("レジストリに `{module}` の版はありません", "the registry has no version of `{module}`"),
            Origin::Git { .. } => no_versions(&module, &origin),
        };
        println!("{}", ctx.say(&none));
    }
    for v in &vs {
        println!("{v}");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_page_and_the_latest_version() {
        assert_eq!(next_link("</v2/acme/tax/tags/list?n=2&last=1.0.0>; rel=\"next\""), Some("/v2/acme/tax/tags/list?n=2&last=1.0.0".to_string()));
        assert_eq!(next_link("<https://h/x>; rel=\"prev\""), None);
        let vs = versions_of(&["1.0.0", "sha256-abc.sig", "2.0.0-rc.1", "1.10.0", "1.2.0", "latest"].map(String::from));
        assert_eq!(vs.iter().map(Version::to_string).collect::<Vec<_>>(), ["1.0.0", "1.2.0", "1.10.0", "2.0.0-rc.1"]);
        assert_eq!(latest(&vs).map(Version::to_string).as_deref(), Some("1.10.0"));
        assert_eq!(latest(&versions_of(&["2.0.0-rc.1".to_string()])), None);
        assert_eq!(host_of("127.0.0.1:5000"), "127.0.0.1");
        assert_eq!(host_of("[::1]:5000"), "[::1]");
    }
}
