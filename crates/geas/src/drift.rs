//! `geas snap` and `geas drift`: the baseline, and every change from it, claimed
//! (a check covers the field, so `geas check` decides) or unclaimed (nothing does).

use crate::diag::count;
use ritsu_base::text::{Lang, Text};
use crate::json::{self, J};
use crate::model::*;
use crate::run::{ClaimResult, Obs};
use std::collections::{HashMap, HashSet};
use std::path::Path;

// ---------- baseline io ----------

pub struct BaseRec {
    pub claim: String,
    pub idx: usize,
    pub call: String,
    pub obs: Obs,
}

/// Why a baseline cannot be used.
pub enum BaselineError {
    /// There is none: E050.
    Missing,
    /// The file is there and cannot be read: E081.
    Unreadable(std::io::Error),
    /// A line is not what `geas snap` writes: E051.
    Bad { line: usize, why: Text },
}

/// What a masked value is written as, in the journal and the baseline.
pub const MASKED: &str = "<masked>";

/// An observation as the journal and the baseline record it: the value of a masked
/// header, and of a masked JSON path in a body that is JSON, written as
/// `<masked>` (the body then written back compactly). A mask declares a value to be
/// noise, so a record that kept it would differ from run to run where the spec says
/// nothing changed. Checks see the observation itself.
pub fn recorded(obs: &Obs, masks: &[Mask]) -> Obs {
    if let Obs::Screen(screen) = obs {
        let patterns: Vec<&crate::screen::Pattern> = masks
            .iter()
            .filter_map(|m| match m {
                Mask::Screen(p) => Some(p),
                _ => None,
            })
            .collect();
        return Obs::Screen(crate::screen::masked(screen, &patterns));
    }
    let Obs::Http { status, headers, body } = obs else {
        return obs.clone();
    };
    let masked_header = |name: &str| masks.iter().any(|m| matches!(m, Mask::Header(h) if h == name));
    let headers = headers
        .iter()
        .map(|(k, v)| (k.clone(), if masked_header(k) { MASKED.to_string() } else { v.clone() }))
        .collect();
    let paths: Vec<&str> = masks
        .iter()
        .filter_map(|m| match m {
            Mask::BodyJson(p) => Some(p.as_str()),
            _ => None,
        })
        .collect();
    let mut body = body.clone();
    if !paths.is_empty()
        && let Ok(mut v) = json::parse(&body)
    {
        let mut changed = false;
        for p in paths {
            if let Some(slot) = json::path_get_mut(&mut v, p) {
                *slot = J::Str(MASKED.into());
                changed = true;
            }
        }
        if changed {
            body = json::render(&v);
        }
    }
    Obs::Http { status: *status, headers, body }
}

/// Headers as a JSON object, in the order they are kept (by name).
pub fn headers_json(headers: &[(String, String)]) -> String {
    let hs: Vec<String> = headers.iter().map(|(k, v)| format!("{}:{}", json::quote(k), json::quote(v))).collect();
    format!("{{{}}}", hs.join(","))
}

/// The format of the baseline's first line.
pub const BASELINE_FORMAT: u32 = 1;

/// A baseline as read: its observations, and the pins each target had when it was
/// taken (none from a baseline written before pins, which had none).
pub struct Baseline {
    pub recs: Vec<BaseRec>,
    /// Target by target, the pins as JSON.
    pub pins: Vec<(String, String)>,
}

/// The baseline's first line: the format, and the pins of every target that has
/// some, so that drift can say when they changed.
fn baseline_head(spec: &Spec) -> String {
    let pins: Vec<String> = spec
        .targets
        .iter()
        .filter(|t| !t.pins.is_empty())
        .map(|t| format!("{}:{}", json::quote(&t.name), t.pins.json()))
        .collect();
    format!("{{\"geas_baseline\":{BASELINE_FORMAT},\"pins\":{{{}}}}}", pins.join(","))
}

/// Writes the baseline; the number of observations it keeps.
pub fn write_baseline(path: &Path, spec: &Spec, results: &[ClaimResult]) -> std::io::Result<usize> {
    let mut lines = Vec::new();
    for r in results {
        for o in &r.observations {
            let obs_s = match recorded(&o.obs, &spec.masks) {
                Obs::Proc { stdout, stderr, exit } => format!(
                    "{{\"kind\":\"proc\",\"stdout\":{},\"stderr\":{},\"exit\":{}}}",
                    json::quote(&stdout),
                    json::quote(&stderr),
                    exit
                ),
                Obs::Http { status, headers, body } => format!(
                    "{{\"kind\":\"http\",\"status\":{},\"headers\":{},\"body\":{}}}",
                    status,
                    headers_json(&headers),
                    json::quote(&body)
                ),
                Obs::Screen(screen) => format!("{{\"kind\":\"screen\",\"screen\":{}}}", screen.json()),
            };
            lines.push(format!(
                "{{\"claim\":{},\"idx\":{},\"target\":{},\"call\":{},\"obs\":{}}}",
                json::quote(&r.name),
                o.idx,
                json::quote(&o.target),
                json::quote(&o.call),
                obs_s
            ));
        }
    }
    let kept = lines.len();
    lines.insert(0, baseline_head(spec));
    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(kept)
}

fn jget<'a>(j: &'a J, key: &str) -> Result<&'a J, Text> {
    match j {
        J::Obj(pairs) => pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .ok_or_else(|| tr!("この行に `{key}` がありません", "the line has no `{key}`")),
        _ => Err(tr!("この行は JSON のオブジェクトではありません", "the line is not a JSON object")),
    }
}

fn jstr(j: &J, key: &str) -> Result<String, Text> {
    match jget(j, key)? {
        J::Str(s) => Ok(s.clone()),
        _ => Err(tr!("`{key}` が文字列ではありません", "`{key}` is not a string")),
    }
}

fn jnum(j: &J, key: &str) -> Result<f64, Text> {
    match jget(j, key)? {
        J::Num(n) => Ok(*n),
        _ => Err(tr!("`{key}` が数ではありません", "`{key}` is not a number")),
    }
}

fn base_rec(line: &str) -> Result<BaseRec, Text> {
    let j = json::parse(line).map_err(|e| tr!("この行は JSON ではありません", "the line is not JSON: {e}"))?;
    let claim = jstr(&j, "claim")?;
    let idx = jnum(&j, "idx")? as usize;
    let call = jstr(&j, "call")?;
    let o = jget(&j, "obs")?;
    let obs = match jstr(o, "kind")?.as_str() {
        "proc" => Obs::Proc {
            stdout: jstr(o, "stdout")?,
            stderr: jstr(o, "stderr")?,
            exit: jnum(o, "exit")? as i32,
        },
        "http" => {
            let mut headers = Vec::new();
            if let J::Obj(pairs) = jget(o, "headers")? {
                for (k, v) in pairs {
                    if let J::Str(s) = v {
                        headers.push((k.clone(), s.clone()));
                    }
                }
            }
            Obs::Http { status: jnum(o, "status")? as u16, headers, body: jstr(o, "body")? }
        }
        "screen" => {
            let node = crate::screen::Node::from_json(jget(o, "screen")?)
                .map_err(|e| tr!("`screen` が画面になっていません: {e}", "`screen` is not a screen: {e}"))?;
            Obs::Screen(node)
        }
        _ => {
            return Err(tr!(
                "`kind` が `proc`・`http`・`screen` のどれでもありません",
                "`kind` is none of `proc`, `http` and `screen`",
            ));
        }
    };
    Ok(BaseRec { claim, idx, call, obs })
}

/// The pins of a baseline's first line, target by target; None for a line that is
/// not the first line `geas snap` writes.
fn base_head(line: &str) -> Option<Result<Vec<(String, String)>, Text>> {
    let j = json::parse(line).ok()?;
    let version = jget(&j, "geas_baseline").ok()?;
    if !matches!(version, J::Num(n) if *n == f64::from(BASELINE_FORMAT)) {
        let v = json::render(version);
        return Some(Err(tr!(
            "このベースラインの形式は {v} で、この geas が読めるのは形式 {BASELINE_FORMAT} です",
            "the baseline is of format {v}, and this geas reads format {BASELINE_FORMAT}",
        )));
    }
    let pins = match jget(&j, "pins") {
        Ok(J::Obj(pairs)) => pairs.iter().map(|(k, v)| (k.clone(), json::render(v))).collect(),
        _ => return Some(Err(tr!("最初の行に `pins` のオブジェクトがありません", "the first line has no `pins` object"))),
    };
    Some(Ok(pins))
}

pub fn read_baseline(path: &Path) -> Result<Baseline, BaselineError> {
    let s = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(BaselineError::Missing),
        Err(e) => return Err(BaselineError::Unreadable(e)),
    };
    let mut out = Baseline { recs: Vec::new(), pins: Vec::new() };
    let mut first = true;
    for (n, line) in s.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        // a baseline from before pins has no first line of its own
        if std::mem::take(&mut first)
            && let Some(head) = base_head(line)
        {
            out.pins = head.map_err(|why| BaselineError::Bad { line: n + 1, why })?;
            continue;
        }
        out.recs.push(base_rec(line).map_err(|why| BaselineError::Bad { line: n + 1, why })?);
    }
    Ok(out)
}

// ---------- claimed-subject map ----------

/// The checks of each `when`, by its place in the claim.
fn claimed_map(claim: &Claim) -> HashMap<usize, Vec<Check>> {
    let mut m: HashMap<usize, Vec<Check>> = HashMap::new();
    let mut idx: isize = -1;
    for s in &claim.steps {
        match s {
            Step::When { .. } => idx += 1,
            Step::Then(c) => {
                if idx >= 0 {
                    m.entry(idx as usize).or_default().push(c.clone());
                }
            }
        }
    }
    m
}

fn segs(path: &str) -> Vec<String> {
    let b: Vec<char> = path.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            '.' => {
                i += 1;
                let start = i;
                while i < b.len() && b[i] != '.' && b[i] != '[' {
                    i += 1;
                }
                out.push(b[start..i].iter().collect());
            }
            '[' => {
                let start = i;
                while i < b.len() && b[i] != ']' {
                    i += 1;
                }
                if i < b.len() {
                    i += 1;
                }
                out.push(b[start..i].iter().collect());
            }
            _ => {
                i += 1;
            }
        }
    }
    out
}

fn path_covers(claimed: &str, diff_path: &str) -> bool {
    let c = segs(claimed);
    let d = segs(diff_path);
    c.len() <= d.len() && c.iter().zip(d.iter()).all(|(a, b)| a == b)
}

// ---------- comparison ----------

/// One side of a change: as a person reads it, and as JSON.
struct Side {
    shown: String,
    json: String,
}

struct DiffE {
    field: String,
    old: Option<Side>,
    new: Option<Side>,
    claimed: bool,
}

fn short(s: &str) -> String {
    let t: String = s.chars().take(120).collect();
    if t.len() < s.len() { format!("{}…", t) } else { t }
}

/// A text value: quoted and cut for a person, whole for JSON.
fn text_side(s: &str) -> Side {
    Side { shown: format!("\"{}\"", json::esc(&short(s))), json: json::quote(s) }
}

fn num_side(n: impl ToString) -> Side {
    let s = n.to_string();
    Side { shown: s.clone(), json: s }
}

fn json_side(v: &J) -> Side {
    let s = json::render(v);
    Side { shown: s.clone(), json: s }
}

fn jdiff<'a>(old: &'a J, new: &'a J, path: &str, out: &mut Vec<(String, Option<&'a J>, Option<&'a J>)>) {
    match (old, new) {
        (J::Obj(a), J::Obj(b)) => {
            for (k, va) in a {
                match b.iter().find(|(kb, _)| kb == k) {
                    Some((_, vb)) => jdiff(va, vb, &format!("{}.{}", path, k), out),
                    None => out.push((format!("{}.{}", path, k), Some(va), None)),
                }
            }
            for (k, vb) in b {
                if !a.iter().any(|(ka, _)| ka == k) {
                    out.push((format!("{}.{}", path, k), None, Some(vb)));
                }
            }
        }
        (J::Arr(a), J::Arr(b)) => {
            let n = a.len().min(b.len());
            for i in 0..n {
                jdiff(&a[i], &b[i], &format!("{}[{}]", path, i), out);
            }
            for (i, item) in a.iter().enumerate().skip(n) {
                out.push((format!("{}[{}]", path, i), Some(item), None));
            }
            for (i, item) in b.iter().enumerate().skip(n) {
                out.push((format!("{}[{}]", path, i), None, Some(item)));
            }
        }
        _ => {
            let same = match (old, new) {
                (J::Num(x), J::Num(y)) => (x - y).abs() < 1e-9,
                (J::Str(x), J::Str(y)) => x == y,
                (J::Bool(x), J::Bool(y)) => x == y,
                (J::Null, J::Null) => true,
                _ => false,
            };
            if !same {
                out.push((path.to_string(), Some(old), Some(new)));
            }
        }
    }
}

/// A node of a screen change, without its children: as a person reads it, and as
/// JSON.
fn node_side(n: &crate::screen::Node) -> Side {
    Side { shown: n.line(), json: n.json() }
}

fn compare(
    base: &Obs,
    cur: &Obs,
    checks: &[Check],
    masks: &Masks,
) -> Vec<DiffE> {
    let mut out = Vec::new();
    let subjects: Vec<Subject> = checks.iter().map(|c| c.subject.clone()).collect();
    let subjects = subjects.as_slice();
    let has = |f: fn(&Subject) -> bool| subjects.iter().any(f);
    let (mask_headers, mask_paths) = (&masks.headers, &masks.paths);
    match (base, cur) {
        (Obs::Screen(old), Obs::Screen(new)) => {
            // a change is claimed when a check of this `when` has a pattern
            // matching the node, before or after (DESIGN §8.6)
            let patterns: Vec<&crate::screen::Pattern> = checks
                .iter()
                .filter_map(|c| match &c.matcher {
                    Matcher::ContainsNode(p) | Matcher::NotContainsNode(p) => Some(p),
                    _ => None,
                })
                .collect();
            for ch in crate::screen::diff(old, new, &masks.screen) {
                out.push(DiffE {
                    field: ch.place(),
                    old: ch.old.as_ref().map(|(n, _)| node_side(n)),
                    new: ch.new.as_ref().map(|(n, _)| node_side(n)),
                    claimed: patterns.iter().any(|p| ch.claimed_by(p)),
                });
            }
        }
        (Obs::Proc { stdout: so, stderr: eo, exit: xo }, Obs::Proc { stdout: sn, stderr: en, exit: xn }) => {
            if so != sn {
                out.push(DiffE {
                    field: "stdout".into(),
                    old: Some(text_side(so)),
                    new: Some(text_side(sn)),
                    claimed: has(|s| matches!(s, Subject::Stdout)),
                });
            }
            if eo != en {
                out.push(DiffE {
                    field: "stderr".into(),
                    old: Some(text_side(eo)),
                    new: Some(text_side(en)),
                    claimed: has(|s| matches!(s, Subject::Stderr)),
                });
            }
            if xo != xn {
                out.push(DiffE {
                    field: "exit".into(),
                    old: Some(num_side(xo)),
                    new: Some(num_side(xn)),
                    claimed: has(|s| matches!(s, Subject::Exit)),
                });
            }
        }
        (Obs::Http { status: so, headers: ho, body: bo }, Obs::Http { status: sn, headers: hn, body: bn }) => {
            if so != sn {
                out.push(DiffE {
                    field: "status".into(),
                    old: Some(num_side(so)),
                    new: Some(num_side(sn)),
                    claimed: has(|s| matches!(s, Subject::Status)),
                });
            }
            let collect = |hs: &[(String, String)]| -> HashMap<String, String> {
                let mut m: HashMap<String, Vec<String>> = HashMap::new();
                for (k, v) in hs {
                    m.entry(k.clone()).or_default().push(v.clone());
                }
                m.into_iter().map(|(k, vs)| (k, vs.join(", "))).collect()
            };
            let mo = collect(ho);
            let mn = collect(hn);
            let mut names: Vec<&String> = mo.keys().chain(mn.keys()).collect();
            names.sort();
            names.dedup();
            for name in names {
                if name == "content-length" || mask_headers.contains(name.as_str()) {
                    continue;
                }
                match (mo.get(name), mn.get(name)) {
                    (Some(a), Some(b)) if a == b => {}
                    (a, b) => out.push(DiffE {
                        field: format!("header `{}`", name),
                        old: a.map(|s| text_side(s)),
                        new: b.map(|s| text_side(s)),
                        claimed: subjects.iter().any(|s| matches!(s, Subject::Header(h) if h == name)),
                    }),
                }
            }
            if bo != bn {
                match (json::parse(bo), json::parse(bn)) {
                    (Ok(jo), Ok(jn)) => {
                        let mut diffs = Vec::new();
                        jdiff(&jo, &jn, "", &mut diffs);
                        for (path, old, new) in diffs {
                            if mask_paths.iter().any(|m| path_covers(m, &path)) {
                                continue;
                            }
                            let claimed = subjects.iter().any(|s| match s {
                                Subject::Body => true,
                                Subject::BodyJson(p) => path_covers(p, &path),
                                _ => false,
                            });
                            let field = if path.is_empty() { "body".into() } else { format!("body json \"{}\"", path) };
                            out.push(DiffE { field, old: old.map(json_side), new: new.map(json_side), claimed });
                        }
                    }
                    _ => out.push(DiffE {
                        field: "body".into(),
                        old: Some(text_side(bo)),
                        new: Some(text_side(bn)),
                        claimed: has(|s| matches!(s, Subject::Body)),
                    }),
                }
            }
        }
        _ => out.push(DiffE {
            field: "observation".into(),
            old: Some(text_side("<one adapter kind>")),
            new: Some(text_side("<another adapter kind>")),
            claimed: false,
        }),
    }
    out
}

// ---------- the drift command ----------

/// One change, in claim order.
pub struct Change {
    pub claim: String,
    /// The `when`'s place in its claim, from 1.
    pub when: usize,
    pub line: usize,
    /// `api.get("/total")`.
    pub call: String,
    pub field: String,
    old: Option<Side>,
    new: Option<Side>,
    pub claimed: bool,
}

pub struct Report {
    pub changes: Vec<Change>,
    pub notes: Vec<Text>,
    pub compared: usize,
    pub drifted: usize,
    pub unclaimed: usize,
    pub claimed: usize,
}

/// The masks of a spec, by what they mask.
struct Masks<'a> {
    headers: HashSet<String>,
    paths: Vec<String>,
    screen: Vec<&'a crate::screen::Pattern>,
}

pub fn drift(spec: &Spec, results: &[ClaimResult], baseline: Baseline) -> Report {
    let mut base: HashMap<(String, usize), BaseRec> =
        baseline.recs.into_iter().map(|b| ((b.claim.clone(), b.idx), b)).collect();
    let claimed_by: HashMap<&str, HashMap<usize, Vec<Check>>> =
        spec.claims.iter().map(|c| (c.name.as_str(), claimed_map(c))).collect();
    let masks = Masks {
        headers: spec
            .masks
            .iter()
            .filter_map(|m| match m {
                Mask::Header(h) => Some(h.clone()),
                _ => None,
            })
            .collect(),
        paths: spec
            .masks
            .iter()
            .filter_map(|m| match m {
                Mask::BodyJson(p) => Some(p.clone()),
                _ => None,
            })
            .collect(),
        screen: spec.screen_masks(),
    };

    let mut r = Report { changes: vec![], notes: vec![], compared: 0, drifted: 0, unclaimed: 0, claimed: 0 };
    // pins that changed since the baseline: what the targets saw is not what they saw
    for tg in &spec.targets {
        let now = json::parse(&tg.pins.json()).map(|v| json::render(&v)).unwrap_or_default();
        let then = baseline.pins.iter().find(|(name, _)| *name == tg.name).map_or("{}".to_string(), |(_, p)| p.clone());
        if now != then {
            let name = &tg.name;
            r.notes.push(tr!(
                "ターゲット `{name}` の固定が、ベースラインと違います: {then} → {now}",
                "the pins of `{name}` differ from the baseline's: {then} → {now}",
            ));
        }
    }
    for res in results {
        for o in &res.observations {
            let key = (res.name.clone(), o.idx);
            let (name, n) = (&res.name, o.idx + 1);
            let Some(b) = base.remove(&key) else {
                r.notes.push(tr!(
                    "主張 \"{name}\" の when#{n} はベースラインにありません（新しい主張なら `geas snap` を走らせてください）",
                    "no baseline for claim \"{name}\" when#{n} (new claim? run `geas snap`)",
                ));
                continue;
            };
            if b.call != o.call {
                r.notes.push(tr!(
                    "主張 \"{name}\" の when#{n} は呼び出しそのものが変わりました（{} → {}）。比べるには `geas snap` をやり直してください",
                    "claim \"{name}\" when#{n}: the call itself changed ({} → {}); re-snap to compare",
                    b.call,
                    o.call,
                ));
                continue;
            }
            r.compared += 1;
            let empty: Vec<Check> = Vec::new();
            let checks = claimed_by.get(res.name.as_str()).and_then(|m| m.get(&o.idx)).unwrap_or(&empty);
            let diffs = compare(&b.obs, &recorded(&o.obs, &spec.masks), checks, &masks);
            if diffs.is_empty() {
                continue;
            }
            r.drifted += 1;
            for d in diffs {
                if d.claimed {
                    r.claimed += 1;
                } else {
                    r.unclaimed += 1;
                }
                r.changes.push(Change {
                    claim: res.name.clone(),
                    when: n,
                    line: o.line,
                    call: format!("{}.{}", o.target, o.call),
                    field: d.field,
                    old: d.old,
                    new: d.new,
                    claimed: d.claimed,
                });
            }
        }
    }
    let mut leftovers: Vec<(String, usize)> = base.into_keys().collect();
    leftovers.sort();
    for (claim, idx) in leftovers {
        let n = idx + 1;
        r.notes.push(tr!(
            "ベースラインには主張 \"{claim}\" の when#{n} がありますが、今回の実行にはありません（主張を消したか、主張がエラーで止まったためです）",
            "baseline has claim \"{claim}\" when#{n} but the current run does not (claim removed or errored)",
        ));
    }
    r
}

impl Report {
    pub fn text(&self, lang: Lang) -> String {
        let mut out = String::new();
        let mut last: Option<(&str, usize)> = None;
        for c in &self.changes {
            if last != Some((c.claim.as_str(), c.when)) {
                out.push_str(&match lang {
                    Lang::En => format!("claim \"{}\" when#{} {}\n", c.claim, c.when, c.call),
                    Lang::Ja => format!("主張 \"{}\" の when#{} {}\n", c.claim, c.when, c.call),
                });
                last = Some((c.claim.as_str(), c.when));
            }
            let (mark, change) = match (&c.old, &c.new) {
                (Some(a), Some(b)) => ("~", format!("{} → {}", a.shown, b.shown)),
                (None, Some(b)) => ("+", format!("{}: {}", tr!("現れた", "appeared").get(lang), b.shown)),
                (Some(a), None) => ("-", format!("{}: {}", tr!("消えた", "disappeared").get(lang), a.shown)),
                (None, None) => ("-", String::new()),
            };
            let tag = if c.claimed {
                tr!("[主張あり — 判定は geas check]", "[claimed — `geas check` is the authority]").get(lang).to_string()
            } else {
                tr!("[主張なし]", "[unclaimed]").get(lang).to_string()
            };
            out.push_str(&format!("  {} {}: {}   {}\n", mark, c.field, change, tag));
        }
        for n in &self.notes {
            out.push_str(&format!("{} {}\n", tr!("注意:", "note:").get(lang), n.get(lang)));
        }
        out.push_str(&match lang {
            Lang::En => format!(
                "drift: {} compared · {} drifted · {} unclaimed change(s) · {} claimed\n",
                count(self.compared, "interaction", "interactions"),
                self.drifted,
                self.unclaimed,
                self.claimed
            ),
            Lang::Ja => format!(
                "ドリフト: 比べたやりとり {} 件 · 変わったやりとり {} 件 · 主張なしの変化 {} 件 · 主張ありの変化 {} 件\n",
                self.compared, self.drifted, self.unclaimed, self.claimed
            ),
        });
        out
    }

    /// The JSON of Appendix A; `diagnostics` holds the claims that could not run.
    pub fn json(&self, file: &str, diagnostics: &[String], lang: Lang) -> String {
        let side = |s: &Option<Side>| s.as_ref().map(|s| s.json.clone()).unwrap_or_else(|| "null".into());
        let changes: Vec<String> = self
            .changes
            .iter()
            .map(|c| {
                format!(
                    "{{\"claim\":{},\"when\":{},\"line\":{},\"call\":{},\"field\":{},\"old\":{},\"new\":{},\"claimed\":{}}}",
                    json::quote(&c.claim),
                    c.when,
                    c.line,
                    json::quote(&c.call),
                    json::quote(&c.field),
                    side(&c.old),
                    side(&c.new),
                    c.claimed
                )
            })
            .collect();
        let notes: Vec<String> = self.notes.iter().map(|n| json::quote(n.get(lang))).collect();
        format!(
            "{{\"geas\":1,\"file\":{},\"compared\":{},\"drifted\":{},\"unclaimed\":{},\"claimed\":{},\"changes\":[{}],\"notes\":[{}],\"diagnostics\":[{}]}}",
            json::quote(file),
            self.compared,
            self.drifted,
            self.unclaimed,
            self.claimed,
            changes.join(","),
            notes.join(","),
            diagnostics.join(",")
        )
    }
}
