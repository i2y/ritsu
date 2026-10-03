//! What a buf module says beside its `.proto` files: the BSR modules its `buf.yaml` depends on,
//! and the commit and digest its `buf.lock` pins each to (rulec's §15.161).

/// The BSR modules a `buf.yaml` declares under `deps`, by name: a label a ref carries is not
/// part of it (`buf.build/bufbuild/protovalidate:v1.0.0` is `buf.build/bufbuild/protovalidate`).
/// The block form and the flow form (`deps: [a, b]`) are both read; `version: v1` and `v2` put
/// `deps` at the top in the same way.
pub fn deps(yaml: &str) -> Vec<String> {
    let name = |s: &str| -> Option<String> {
        let s = s.trim().trim_matches(|c| c == '"' || c == '\'');
        let s = s.split(':').next().unwrap_or("").trim();
        (!s.is_empty()).then(|| s.to_string())
    };
    let mut out = Vec::new();
    let mut inside = false;
    for line in yaml.lines() {
        let line = line.split(" #").next().unwrap_or("");
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with([' ', '\t', '-']) {
            inside = false;
            if let Some(rest) = line.strip_prefix("deps:") {
                match rest.trim().strip_prefix('[') {
                    Some(flow) => out.extend(flow.trim_end_matches(']').split(',').filter_map(name)),
                    None => inside = true,
                }
            }
            continue;
        }
        if let Some(item) = line.trim_start().strip_prefix('-').filter(|_| inside) {
            out.extend(name(item));
        }
    }
    out
}

/// One module a `buf.lock` pins: its name, and the commit and digest it was resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub name: String,
    pub commit: String,
    pub digest: String,
}

/// What a `buf.lock` pins, and the version of its shape. A `version: v1` lock names a module by
/// `remote`, `owner` and `repository` and digests it with shake256; a v2 module takes only v2
/// pins (b5), so the version is what says whether these can be carried over.
pub fn lock(yaml: &str) -> (String, Vec<Pin>) {
    let mut version = String::new();
    let mut pins: Vec<Pin> = Vec::new();
    let mut fields: Vec<(String, String)> = Vec::new();
    let flush = |fields: &mut Vec<(String, String)>, pins: &mut Vec<Pin>| {
        let get = |k: &str| fields.iter().find(|(f, _)| f == k).map(|(_, v)| v.clone()).unwrap_or_default();
        let name = match get("name") {
            n if !n.is_empty() => n,
            _ => [get("remote"), get("owner"), get("repository")].join("/"),
        };
        if !fields.is_empty() && !name.trim_matches('/').is_empty() {
            pins.push(Pin { name, commit: get("commit"), digest: get("digest") });
        }
        fields.clear();
    };
    for line in yaml.lines() {
        let line = line.split(" #").next().unwrap_or("");
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(v) = line.strip_prefix("version:") {
            version = v.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
            continue;
        }
        let t = line.trim_start();
        let t = match t.strip_prefix('-') {
            Some(rest) => {
                flush(&mut fields, &mut pins);
                rest.trim_start()
            }
            None => t,
        };
        if let Some((k, v)) = t.split_once(':').filter(|_| line.starts_with([' ', '\t', '-'])) {
            fields.push((k.trim().to_string(), v.trim().trim_matches(|c| c == '"' || c == '\'').to_string()));
        }
    }
    flush(&mut fields, &mut pins);
    (version, pins)
}
