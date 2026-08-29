use crate::json::{self, J};
use crate::model::*;
use crate::run::{ClaimResult, ClaimStatus, Obs};
use std::collections::{HashMap, HashSet};
use std::path::Path;

// ---------- baseline io ----------

pub struct BaseRec {
    pub claim: String,
    pub idx: usize,
    pub call: String,
    pub obs: Obs,
}

pub fn write_baseline(path: &Path, results: &[ClaimResult]) -> std::io::Result<usize> {
    let mut lines = Vec::new();
    for r in results {
        for o in &r.observations {
            let obs_s = match &o.obs {
                Obs::Proc { stdout, stderr, exit } => format!(
                    "{{\"kind\":\"proc\",\"stdout\":\"{}\",\"stderr\":\"{}\",\"exit\":{}}}",
                    json::esc(stdout),
                    json::esc(stderr),
                    exit
                ),
                Obs::Http { status, headers, body } => {
                    let hs: Vec<String> = headers
                        .iter()
                        .map(|(k, v)| format!("\"{}\":\"{}\"", json::esc(k), json::esc(v)))
                        .collect();
                    format!(
                        "{{\"kind\":\"http\",\"status\":{},\"headers\":{{{}}},\"body\":\"{}\"}}",
                        status,
                        hs.join(","),
                        json::esc(body)
                    )
                }
            };
            lines.push(format!(
                "{{\"claim\":\"{}\",\"idx\":{},\"target\":\"{}\",\"call\":\"{}\",\"obs\":{}}}",
                json::esc(&r.name),
                o.idx,
                json::esc(&o.target),
                json::esc(&o.call),
                obs_s
            ));
        }
    }
    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(lines.len())
}

fn jget<'a>(j: &'a J, key: &str) -> Result<&'a J, String> {
    match j {
        J::Obj(pairs) => pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .ok_or_else(|| format!("baseline record missing `{}`", key)),
        _ => Err("baseline record is not an object".into()),
    }
}

fn jstr(j: &J, key: &str) -> Result<String, String> {
    match jget(j, key)? {
        J::Str(s) => Ok(s.clone()),
        _ => Err(format!("baseline `{}` is not a string", key)),
    }
}

fn jnum(j: &J, key: &str) -> Result<f64, String> {
    match jget(j, key)? {
        J::Num(n) => Ok(*n),
        _ => Err(format!("baseline `{}` is not a number", key)),
    }
}

pub fn read_baseline(path: &Path) -> Result<Vec<BaseRec>, String> {
    let s = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read baseline {}: {} (run `geas snap` first)", path.display(), e))?;
    let mut out = Vec::new();
    for (n, line) in s.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let j = json::parse(line).map_err(|e| format!("baseline line {}: {}", n + 1, e))?;
        let claim = jstr(&j, "claim")?;
        let idx = jnum(&j, "idx")? as usize;
        let call = jstr(&j, "call")?;
        let o = jget(&j, "obs")?;
        let kind = jstr(o, "kind")?;
        let obs = if kind == "proc" {
            Obs::Proc {
                stdout: jstr(o, "stdout")?,
                stderr: jstr(o, "stderr")?,
                exit: jnum(o, "exit")? as i32,
            }
        } else {
            let mut headers = Vec::new();
            if let J::Obj(pairs) = jget(o, "headers")? {
                for (k, v) in pairs {
                    if let J::Str(s) = v {
                        headers.push((k.clone(), s.clone()));
                    }
                }
            }
            Obs::Http {
                status: jnum(o, "status")? as u16,
                headers,
                body: jstr(o, "body")?,
            }
        };
        out.push(BaseRec { claim, idx, call, obs });
    }
    Ok(out)
}

// ---------- claimed-subject map ----------

fn claimed_map(claim: &Claim) -> HashMap<usize, Vec<Subject>> {
    let mut m: HashMap<usize, Vec<Subject>> = HashMap::new();
    let mut idx: isize = -1;
    for s in &claim.steps {
        match s {
            Step::When { .. } => idx += 1,
            Step::Then(c) => {
                if idx >= 0 {
                    m.entry(idx as usize).or_default().push(c.subject.clone());
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

struct DiffE {
    field: String,
    old: Option<String>,
    new: Option<String>,
    claimed: bool,
}

fn short(s: &str) -> String {
    let t: String = s.chars().take(120).collect();
    if t.len() < s.len() { format!("{}…", t) } else { t }
}

fn q(s: &str) -> String {
    format!("\"{}\"", json::esc(&short(s)))
}

fn jdiff(old: &J, new: &J, path: &str, out: &mut Vec<(String, Option<String>, Option<String>)>) {
    match (old, new) {
        (J::Obj(a), J::Obj(b)) => {
            for (k, va) in a {
                match b.iter().find(|(kb, _)| kb == k) {
                    Some((_, vb)) => jdiff(va, vb, &format!("{}.{}", path, k), out),
                    None => out.push((format!("{}.{}", path, k), Some(json::render(va)), None)),
                }
            }
            for (k, vb) in b {
                if !a.iter().any(|(ka, _)| ka == k) {
                    out.push((format!("{}.{}", path, k), None, Some(json::render(vb))));
                }
            }
        }
        (J::Arr(a), J::Arr(b)) => {
            let n = a.len().min(b.len());
            for i in 0..n {
                jdiff(&a[i], &b[i], &format!("{}[{}]", path, i), out);
            }
            for (i, item) in a.iter().enumerate().skip(n) {
                out.push((format!("{}[{}]", path, i), Some(json::render(item)), None));
            }
            for (i, item) in b.iter().enumerate().skip(n) {
                out.push((format!("{}[{}]", path, i), None, Some(json::render(item))));
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
                out.push((path.to_string(), Some(json::render(old)), Some(json::render(new))));
            }
        }
    }
}

fn compare(
    base: &Obs,
    cur: &Obs,
    subjects: &[Subject],
    mask_headers: &HashSet<String>,
    mask_paths: &[String],
) -> Vec<DiffE> {
    let mut out = Vec::new();
    let has = |f: fn(&Subject) -> bool| subjects.iter().any(f);
    match (base, cur) {
        (
            Obs::Proc { stdout: so, stderr: eo, exit: xo },
            Obs::Proc { stdout: sn, stderr: en, exit: xn },
        ) => {
            if so != sn {
                out.push(DiffE {
                    field: "stdout".into(),
                    old: Some(q(so)),
                    new: Some(q(sn)),
                    claimed: has(|s| matches!(s, Subject::Stdout)),
                });
            }
            if eo != en {
                out.push(DiffE {
                    field: "stderr".into(),
                    old: Some(q(eo)),
                    new: Some(q(en)),
                    claimed: has(|s| matches!(s, Subject::Stderr)),
                });
            }
            if xo != xn {
                out.push(DiffE {
                    field: "exit".into(),
                    old: Some(xo.to_string()),
                    new: Some(xn.to_string()),
                    claimed: has(|s| matches!(s, Subject::Exit)),
                });
            }
        }
        (
            Obs::Http { status: so, headers: ho, body: bo },
            Obs::Http { status: sn, headers: hn, body: bn },
        ) => {
            if so != sn {
                out.push(DiffE {
                    field: "status".into(),
                    old: Some(so.to_string()),
                    new: Some(sn.to_string()),
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
                        old: a.map(|s| q(s)),
                        new: b.map(|s| q(s)),
                        claimed: false,
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
                            let field = if path.is_empty() {
                                "body".into()
                            } else {
                                format!("body json \"{}\"", path)
                            };
                            out.push(DiffE { field, old, new, claimed });
                        }
                    }
                    _ => out.push(DiffE {
                        field: "body".into(),
                        old: Some(q(bo)),
                        new: Some(q(bn)),
                        claimed: has(|s| matches!(s, Subject::Body)),
                    }),
                }
            }
        }
        _ => out.push(DiffE {
            field: "observation".into(),
            old: Some("<one adapter kind>".into()),
            new: Some("<another adapter kind>".into()),
            claimed: false,
        }),
    }
    out
}

// ---------- the drift command ----------

pub fn drift(spec: &Spec, results: &[ClaimResult], baseline: Vec<BaseRec>, file: &str) -> i32 {
    let mut base: HashMap<(String, usize), BaseRec> = baseline
        .into_iter()
        .map(|b| ((b.claim.clone(), b.idx), b))
        .collect();
    let claimed_by: HashMap<&str, HashMap<usize, Vec<Subject>>> = spec
        .claims
        .iter()
        .map(|c| (c.name.as_str(), claimed_map(c)))
        .collect();
    let mask_headers: HashSet<String> = spec
        .masks
        .iter()
        .filter_map(|m| match m {
            Mask::Header(h) => Some(h.clone()),
            _ => None,
        })
        .collect();
    let mask_paths: Vec<String> = spec
        .masks
        .iter()
        .filter_map(|m| match m {
            Mask::BodyJson(p) => Some(p.clone()),
            _ => None,
        })
        .collect();

    let mut compared = 0usize;
    let mut drifted = 0usize;
    let mut n_unclaimed = 0usize;
    let mut n_claimed = 0usize;
    let mut notes: Vec<String> = Vec::new();
    let mut had_error = false;

    for r in results {
        if let ClaimStatus::Error { message, line } = &r.status {
            notes.push(format!(
                "claim \"{}\" errored during replay ({}:{}: {})",
                r.name, file, line, message
            ));
            had_error = true;
        }
        for o in &r.observations {
            let key = (r.name.clone(), o.idx);
            let Some(b) = base.remove(&key) else {
                notes.push(format!(
                    "no baseline for claim \"{}\" when#{} (new claim? run `geas snap`)",
                    r.name,
                    o.idx + 1
                ));
                continue;
            };
            if b.call != o.call {
                notes.push(format!(
                    "claim \"{}\" when#{}: the call itself changed ({} → {}); re-snap to compare",
                    r.name,
                    o.idx + 1,
                    b.call,
                    o.call
                ));
                continue;
            }
            compared += 1;
            let empty: Vec<Subject> = Vec::new();
            let subjects = claimed_by
                .get(r.name.as_str())
                .and_then(|m| m.get(&o.idx))
                .unwrap_or(&empty);
            let diffs = compare(&b.obs, &o.obs, subjects, &mask_headers, &mask_paths);
            if diffs.is_empty() {
                continue;
            }
            drifted += 1;
            println!("claim \"{}\" when#{} {}", r.name, o.idx + 1, o.call);
            for d in diffs {
                let mark = match (&d.old, &d.new) {
                    (Some(_), Some(_)) => "~",
                    (None, Some(_)) => "+",
                    _ => "-",
                };
                let change = match (&d.old, &d.new) {
                    (Some(a), Some(b)) => format!("{} → {}", a, b),
                    (None, Some(b)) => format!("appeared: {}", b),
                    (Some(a), None) => format!("disappeared: {}", a),
                    (None, None) => String::new(),
                };
                let tag = if d.claimed {
                    n_claimed += 1;
                    "[claimed — `geas check` is the authority]"
                } else {
                    n_unclaimed += 1;
                    "[unclaimed]"
                };
                println!("  {} {}: {}   {}", mark, d.field, change, tag);
            }
        }
    }
    let mut leftovers: Vec<(String, usize)> = base.into_keys().collect();
    leftovers.sort();
    for (claim, idx) in leftovers {
        notes.push(format!(
            "baseline has claim \"{}\" when#{} but the current run does not (claim removed or errored)",
            claim,
            idx + 1
        ));
    }

    for n in &notes {
        println!("note: {}", n);
    }
    println!(
        "drift: {} interactions compared · {} drifted · {} unclaimed change(s) · {} claimed",
        compared, drifted, n_unclaimed, n_claimed
    );
    if had_error {
        2
    } else if n_unclaimed + n_claimed > 0 {
        1
    } else {
        0
    }
}
