//! What `ritsu mod` asks of a registry (DESIGN 17.12): the requests of the distribution-spec it
//! sends ([`Remote`]), and the commands that are nothing but those: `add` with a repository, `get`,
//! and `versions`. The table reads their command lines before they are called, and sends `add` here
//! when its source is not a directory ([`super::cli::is_path`]).
//!
//! A registry is asked over https, a host of the loopback over plain HTTP (a registry a test
//! starts), and `docker.io` at `registry-1.docker.io` (a name of one part there is under `library/`).
//! A tag is looked up with an OCI manifest's `GET` (`HEAD` for whether it is there), the versions
//! with `GET /v2/<name>/tags/list?n=1000` and each `Link` with `rel="next"` after it, of which only
//! the tags that are versions count (a signature's old tag, `sha256-….sig`, is none). `@latest` is
//! the highest version that is not a prerelease; a registry with prereleases alone has none, and the
//! command asks for a version (exit 2).

use super::http::{self, Body, NoAnswer, Request, Response};
use super::{Ctx, auth, edit, store_write};
use ritsu_base::cli::Args;
use ritsu_base::json::{self, Json};
use ritsu_base::modules::artifact;
use ritsu_base::modules::manifest::{self, Source};
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::path::PathBuf;

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

/// The version `asked` names of `module`: itself, or for `latest`, the registry's latest.
fn version_asked(module: &ModuleName, asked: &str) -> Result<Version, Text> {
    if asked != "latest" {
        return Version::parse(asked).map_err(|e| e.text(asked));
    }
    let vs = Remote::of(module, false)?.versions()?;
    match latest(&vs) {
        Some(v) => Ok(v.clone()),
        None if vs.is_empty() => Err(tr!("レジストリに `{module}` の版がありません", "the registry has no version of `{module}`")),
        None => Err(tr!(
            "`{module}` にはプレリリースの版しかありません。`@<版>` で版を書いてください",
            "`{module}` has prereleases alone; write the version, `@<version>`"
        )),
    }
}

/// The module a name on the command line names: a dependency of the module at hand from a registry,
/// by its name, or a module by its published name.
pub fn module_named(ctx: &Ctx, name: &str) -> Result<ModuleName, Text> {
    if let Ok(root) = ctx.root()
        && let Ok(bytes) = std::fs::read(root.join(manifest::FILE))
        && let Ok(m) = manifest::parse(&bytes, manifest::FILE)
        && let Some(r) = m.require(name)
    {
        return match &r.source {
            Source::Oci { repository, .. } => ModuleName::parse(repository).map_err(|e| e.text()),
            Source::Path { path } => Err(tr!(
                "依存 `{name}` はパス `{path}` から読むので、レジストリにはありません",
                "the dependency `{name}` is read from the path `{path}`, so no registry has it"
            )),
        };
    }
    match ModuleName::parse(name) {
        Ok(m) if m.is_published() => Ok(m),
        _ => Err(tr!(
            "`{name}` は、レジストリから取る依存の名前でも、配るモジュールの名前（`<レジストリ>/<リポジトリ>`）でもありません",
            "`{name}` is neither the name of a dependency from a registry nor a published module's name (`<registry>/<repository>`)"
        )),
    }
}

/// `ritsu mod add <name> <repository>[@<version>|@latest]`: the exit code. `a.pos` holds the name
/// and the source; `add` with a directory is [`super::edit::add`].
pub fn add(ctx: &Ctx, a: &Args) -> u8 {
    let (name, source) = (a.pos[0].as_str(), a.pos[1].as_str());
    let root = match ctx.root() {
        Ok(r) => r,
        Err(t) => return ctx.refuse(t),
    };
    let (repository, asked) = source.rsplit_once('@').unwrap_or((source, "latest"));
    let module = match ModuleName::parse(repository) {
        Ok(m) if m.is_published() => m,
        _ => {
            return ctx.refuse(tr!(
                "`{repository}` は配るモジュールの名前ではありません。レジストリの依存は `<レジストリ>/<リポジトリ>`、パスの依存は `./…` か `../…` で書きます",
                "`{repository}` is no published module's name: a dependency from a registry is `<registry>/<repository>`, and one by a path starts with `./` or `../`"
            ));
        }
    };
    let file = root.join(manifest::FILE);
    let (before, m) = match edit::read(ctx, &root, &file) {
        Ok(x) => x,
        Err(code) => return code,
    };
    // the name is checked before the registry is asked
    if let Err(e) = m.added(name, &Source::Oci { repository: module.to_string(), min: Version::new(0, 0, 0) }) {
        return ctx.refuse(e.text());
    }
    let version = match version_asked(&module, asked) {
        Ok(v) => v,
        Err(t) => return ctx.refuse(t),
    };
    let source = Source::Oci { repository: module.to_string(), min: version.clone() };
    let text = match m.added(name, &source) {
        Ok(t) => t,
        Err(e) => return ctx.refuse(e.text()),
    };
    let plan = match edit::change(ctx, &root, &file, &before, &text) {
        Ok(p) => p,
        Err(code) => return code,
    };
    if ctx.json() {
        println!("{}", edit::added_json(ctx, &root, name, &module.to_string(), Some(version.to_string()), None, plan.done).pretty());
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
    let (module, old) = match m.require(name).map(|r| &r.source) {
        Some(Source::Oci { repository, min }) => match ModuleName::parse(repository) {
            Ok(n) => (n, min.clone()),
            Err(e) => return ctx.refuse(e.text()),
        },
        Some(Source::Path { .. }) => return ctx.refuse(manifest::EditError::NotOci(name.to_string()).text()),
        None => return ctx.refuse(manifest::EditError::NotRequired(name.to_string()).text()),
    };
    let version = match version_asked(&module, asked) {
        Ok(v) => v,
        Err(t) => return ctx.refuse(t),
    };
    let text = match m.with_version(name, &version) {
        Ok(t) => t,
        Err(e) => return ctx.refuse(e.text()),
    };
    let plan = match edit::change(ctx, &root, &file, &before, &text) {
        Ok(p) => p,
        Err(code) => return code,
    };
    if ctx.json() {
        println!("{}", edit::added_json(ctx, &root, name, &module.to_string(), Some(version.to_string()), None, plan.done).pretty());
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

/// `ritsu mod versions <name>`: the exit code.
pub fn versions(ctx: &Ctx, a: &Args) -> u8 {
    let name = a.pos[0].as_str();
    let module = match module_named(ctx, name) {
        Ok(m) => m,
        Err(t) => return ctx.refuse(t),
    };
    let vs = match Remote::of(&module, false).and_then(|r| r.versions()) {
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
        println!("{}", ctx.say(&tr!("レジストリに `{module}` の版はありません", "the registry has no version of `{module}`")));
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
