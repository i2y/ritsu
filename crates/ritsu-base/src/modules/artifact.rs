//! A version of a module as an OCI artifact (DESIGN 17.11): an image manifest whose config is the
//! empty descriptor, whose `artifactType` is ritsu's module, and whose two layers are `ritsu.mod`
//! and the tar of the module's files.
//!
//! ritsu writes the manifest one way: no blank, its keys in the order below, no annotation that
//! `ritsu.mod` does not give, no date. The same files give the same bytes whenever they are pushed,
//! and so the same digest, which anyone can make again from the source and compare.
//!
//! A manifest fetched is checked before anything it names is fetched ([`check`]): its media types,
//! the empty config, the two layers in their order, and their sizes under the limits.

use super::digest;
use super::manifest::Manifest;
use super::tar;
use crate::json::{self, Json};
use crate::text::Text;
use crate::tr;

/// The media type of an OCI image manifest.
pub const MANIFEST_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
/// ritsu's module, the manifest's `artifactType`.
pub const ARTIFACT_TYPE: &str = "application/vnd.ritsu.module.v1";
/// The first layer: `ritsu.mod` as it is.
pub const MOD_TYPE: &str = "application/vnd.ritsu.mod.v1";
/// The second layer: the tar of the files, not compressed.
pub const FILES_TYPE: &str = "application/vnd.ritsu.files.v1.tar";
/// The empty descriptor's media type, digest, size and data (image-spec's `{}`).
pub const EMPTY_TYPE: &str = "application/vnd.oci.empty.v1+json";
pub const EMPTY_DIGEST: &str = "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
pub const EMPTY_DATA: &str = "e30=";
/// The bytes of the empty blob.
pub const EMPTY: &[u8] = b"{}";

/// The largest OCI manifest.
pub const MAX_MANIFEST: u64 = 64 * 1024;
/// The largest `ritsu.mod`.
pub const MAX_MOD: u64 = 1024 * 1024;

/// A version of a module made into its blobs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    /// The OCI manifest's bytes, and their digest.
    pub manifest: Vec<u8>,
    pub digest: String,
    /// `ritsu.mod`, the first layer.
    pub module: Vec<u8>,
    pub module_digest: String,
    /// The tar of the files, the second layer; its digest is the digest of the files (`files` of a
    /// lock).
    pub tar: Vec<u8>,
    pub files_digest: String,
}

/// The blobs of a module: `mod_bytes` is its `ritsu.mod` (as `manifest` was read from), `files`
/// every file it carries ([`digest::module_files`]). Its name has to be a published one and it has
/// to have a version: the manifest's annotations name both.
pub fn build(manifest: &Manifest, mod_bytes: &[u8], files: &[(String, Vec<u8>)]) -> Result<Built, Text> {
    if !manifest.module.value.is_published() {
        let n = manifest.module.value.to_string();
        return Err(tr!(
            "モジュール `{n}` は配らない名前なので、OCI の成果物にできません。`module` に `<レジストリ>/<リポジトリ>` を書いてください",
            "the module `{n}` has a name that is not published, so it is no OCI artifact; write `<registry>/<repository>` on its `module` line"
        ));
    }
    let Some(version) = &manifest.version else {
        let n = manifest.module.value.to_string();
        return Err(tr!("モジュール `{n}` の `ritsu.mod` に `version` がありません", "the `ritsu.mod` of the module `{n}` has no `version`"));
    };
    if mod_bytes.len() as u64 > MAX_MOD {
        let (n, max) = (mod_bytes.len(), MAX_MOD);
        return Err(tr!("`ritsu.mod` が {n} バイトあります。{max} バイトまでです", "`ritsu.mod` is {n} bytes, and it is {max} bytes at most"));
    }
    let tar = tar::write(files).map_err(|e| e.text())?;
    let module_digest = digest::of(mod_bytes);
    let files_digest = digest::of(&tar);
    let annotation = |a: &Option<super::manifest::At<String>>| a.as_ref().map(|x| x.value.clone());
    let bytes = manifest_bytes(&Annotations {
        title: manifest.module.value.to_string(),
        version: version.value.to_string(),
        description: annotation(&manifest.description),
        source: annotation(&manifest.source),
        licenses: annotation(&manifest.license),
    }, (&module_digest, mod_bytes.len() as u64), (&files_digest, tar.len() as u64));
    if bytes.len() as u64 > MAX_MANIFEST {
        let (n, max) = (bytes.len(), MAX_MANIFEST);
        return Err(tr!("OCI のマニフェストが {n} バイトになります。{max} バイトまでです", "the OCI manifest is {n} bytes, and it is {max} bytes at most"));
    }
    Ok(Built { digest: digest::of(&bytes), manifest: bytes, module: mod_bytes.to_vec(), module_digest, tar, files_digest })
}

/// The annotations of the manifest: the module's name and version, and what `ritsu.mod` gives of the
/// rest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Annotations {
    pub title: String,
    pub version: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub licenses: Option<String>,
}

/// The bytes of the OCI manifest, from the two layers' digests and sizes.
pub fn manifest_bytes(a: &Annotations, module: (&str, u64), files: (&str, u64)) -> Vec<u8> {
    let layer = |media: &str, (d, n): (&str, u64), title: &str| {
        Json::obj([
            ("mediaType", Json::str(media)),
            ("digest", Json::str(d)),
            ("size", Json::int(n)),
            ("annotations", Json::obj([("org.opencontainers.image.title", Json::str(title))])),
        ])
    };
    let mut notes = vec![("org.opencontainers.image.title", Json::str(&a.title)), ("org.opencontainers.image.version", Json::str(&a.version))];
    for (k, v) in [("org.opencontainers.image.description", &a.description), ("org.opencontainers.image.source", &a.source), ("org.opencontainers.image.licenses", &a.licenses)] {
        if let Some(v) = v {
            notes.push((k, Json::str(v)));
        }
    }
    Json::obj([
        ("schemaVersion", Json::int(2)),
        ("mediaType", Json::str(MANIFEST_TYPE)),
        ("artifactType", Json::str(ARTIFACT_TYPE)),
        ("config", Json::obj([("mediaType", Json::str(EMPTY_TYPE)), ("digest", Json::str(EMPTY_DIGEST)), ("size", Json::int(2)), ("data", Json::str(EMPTY_DATA))])),
        ("layers", Json::arr([layer(MOD_TYPE, module, "ritsu.mod"), layer(FILES_TYPE, files, "files.tar")])),
        ("annotations", Json::obj(notes)),
    ])
    .compact()
    .into_bytes()
}

/// What a fetched OCI manifest says of the module's two layers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layers {
    pub module_digest: String,
    pub module_size: u64,
    pub files_digest: String,
    pub files_size: u64,
    /// The annotations `org.opencontainers.image.title` and `.version`, when it has them.
    pub title: Option<String>,
    pub version: Option<String>,
}

/// A fetched OCI manifest, checked: its size, an image manifest of ritsu's module with the empty
/// config, and two layers of the types and in the order ritsu writes, each under its limit.
pub fn check(bytes: &[u8]) -> Result<Layers, Text> {
    if bytes.len() as u64 > MAX_MANIFEST {
        let (n, max) = (bytes.len(), MAX_MANIFEST);
        return Err(tr!("OCI のマニフェストが {n} バイトあります。{max} バイトまでです", "the OCI manifest is {n} bytes, and it is {max} bytes at most"));
    }
    let not = |why: Text| tr!("取ってきた OCI のマニフェストは、ritsu のモジュールのものではありません。", "the OCI manifest fetched is not one of a ritsu module: ").then(&why);
    let text = std::str::from_utf8(bytes).map_err(|_| not(tr!("UTF-8 ではありません", "it is not UTF-8")))?;
    let j = json::parse(text).map_err(|e| not(e.message))?;
    let s = |j: &Json, k: &str| j.get(k).and_then(Json::as_str).map(str::to_string);
    let want = |k: &str, v: &str| -> Result<(), Text> {
        if s(&j, k).as_deref() == Some(v) {
            Ok(())
        } else {
            Err(not(tr!("`{k}` が `{v}` ではありません", "`{k}` is not `{v}`")))
        }
    };
    if j.get("schemaVersion").and_then(Json::as_int) != Some(2) {
        return Err(not(tr!("`schemaVersion` が 2 ではありません", "`schemaVersion` is not 2")));
    }
    want("mediaType", MANIFEST_TYPE)?;
    want("artifactType", ARTIFACT_TYPE)?;
    let config = j.get("config").ok_or_else(|| not(tr!("`config` がありません", "it has no `config`")))?;
    if s(config, "mediaType").as_deref() != Some(EMPTY_TYPE) || s(config, "digest").as_deref() != Some(EMPTY_DIGEST) || config.get("size").and_then(Json::as_int) != Some(2) {
        return Err(not(tr!("`config` が空の記述子ではありません", "its `config` is not the empty descriptor")));
    }
    let layers = j.get("layers").and_then(Json::as_arr).ok_or_else(|| not(tr!("`layers` がありません", "it has no `layers`")))?;
    if layers.len() != 2 {
        let n = layers.len();
        return Err(not(tr!("層が {n} あります。二つです", "it has {n} layers, and a module has two")));
    }
    let mut got: Vec<(String, u64)> = Vec::new();
    for (l, (media, max)) in layers.iter().zip([(MOD_TYPE, MAX_MOD), (FILES_TYPE, tar::MAX_TAR)]) {
        if s(l, "mediaType").as_deref() != Some(media) {
            return Err(not(tr!("層の種類が `{media}` ではありません", "a layer's type is not `{media}`")));
        }
        let d = s(l, "digest").filter(|d| digest::is_digest(d)).ok_or_else(|| not(tr!("層 `{media}` のダイジェストが読めません", "the digest of the layer `{media}` does not read")))?;
        let n = l.get("size").and_then(Json::as_int).filter(|n| *n >= 0).ok_or_else(|| not(tr!("層 `{media}` の大きさが読めません", "the size of the layer `{media}` does not read")))? as u64;
        if n > max {
            return Err(tr!("層 `{media}` が {n} バイトあります。{max} バイトまでです", "the layer `{media}` is {n} bytes, and it is {max} bytes at most"));
        }
        got.push((d, n));
    }
    let notes = j.get("annotations");
    let note = |k: &str| notes.and_then(|n| s(n, k));
    Ok(Layers {
        module_digest: got[0].0.clone(),
        module_size: got[0].1,
        files_digest: got[1].0.clone(),
        files_size: got[1].1,
        title: note("org.opencontainers.image.title"),
        version: note("org.opencontainers.image.version"),
    })
}
