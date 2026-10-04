//! ritsu's ports, as geas answers them (ritsu's DESIGN 3.2): [`Engine`] implements the port of
//! claims (`ritsu_ports::Claims`) — the claims of a spec, the record `geas map` keeps of the
//! lines each claim ran, and what a diff comes to for them (`geas affected`) — and gives what a
//! `.geas` holds (`Items`). Nothing here runs a claim.

use crate::map::Record;
use ritsu_base::naming::{Name as Naming, Tool};
use ritsu_ports::{Claim, Item, MapRecord, RecordClaim, RecordFile, RecordRan, Said};
use std::path::{Path, PathBuf};

/// geas, as the ports reach it.
#[derive(Default)]
pub struct Engine;

/// A spec's claims, each its name, its line, and the lines of its block.
fn read(file: &Path) -> Result<(String, Vec<(String, usize, usize)>), Vec<Said>> {
    let path = file.to_string_lossy().to_string();
    let src = std::fs::read_to_string(file).map_err(|e| vec![Said::unreadable(&path, &e.to_string())])?;
    let spec = crate::parse::parse(&src).map_err(|ds| ds.iter().filter(|d| d.is_error()).map(|d| Said { file: path.clone(), ..Said::of(d) }).collect::<Vec<_>>())?;
    let lines: Vec<&str> = src.lines().collect();
    let claims = spec
        .claims
        .iter()
        .map(|c| {
            // the block: the claim's line and the lines under it, up to the next line that starts
            // at the margin, without the blank lines and comments at its end
            let start = c.pos.line;
            let mut end = start;
            for (i, l) in lines.iter().enumerate().skip(start) {
                if !l.trim().is_empty() && !l.starts_with([' ', '\t']) {
                    break;
                }
                if !l.trim().is_empty() && !l.trim_start().starts_with('#') {
                    end = i + 1;
                }
            }
            (c.name.clone(), start, end)
        })
        .collect();
    Ok((src, claims))
}

/// The lines of a block as its definition counts them: each without the spaces around it, the
/// blank lines and the comments left out.
fn steps(src: &str, from: usize, to: usize) -> Vec<String> {
    src.lines().skip(from.saturating_sub(1)).take(to + 1 - from).map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).collect()
}

impl Engine {
    /// `geas check` of each spec, as `ritsu check` prints it ([`crate::cli::checked`]).
    pub fn checked(&self, root: &Path, files: &[String], lang: ritsu_base::text::Lang) -> Vec<ritsu_ports::Checked> {
        crate::cli::checked(root, files, lang)
    }
}

impl ritsu_ports::Claims for Engine {
    fn claims(&self, file: &Path) -> Result<Vec<Claim>, Vec<Said>> {
        let (src, claims) = read(file)?;
        Ok(claims.into_iter().map(|(name, from, to)| Claim { name, line: from, steps: steps(&src, from + 1, to) }).collect())
    }

    /// The record is where `geas map` writes it when no `--out` says otherwise:
    /// `.geas/<stem>.map.jsonl` beside the spec.
    fn map_record(&self, file: &Path) -> Result<Option<MapRecord>, Vec<Said>> {
        let dir = file.parent().filter(|d| !d.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        let stem = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "spec".into());
        let at = dir.join(".geas").join(format!("{stem}.map.jsonl"));
        let shown = at.to_string_lossy().to_string();
        let Ok(text) = std::fs::read_to_string(&at) else { return Ok(None) };
        let r = Record::parse(&text).map_err(|(line, why)| vec![Said { code: String::new(), file: shown.clone(), line: Some(line), message: why }])?;
        let ranges = |ls: &crate::lines::Lines| -> Vec<(usize, usize)> {
            let mut out: Vec<(usize, usize)> = Vec::new();
            for &l in ls {
                match out.last_mut() {
                    Some((_, last)) if *last + 1 == l as usize => *last = l as usize,
                    _ => out.push((l as usize, l as usize)),
                }
            }
            out
        };
        Ok(Some(MapRecord {
            spec: r.spec.clone(),
            root: r.root.clone(),
            claims: r.claims.iter().map(|c| RecordClaim { name: c.name.clone(), status: c.status.clone(), targets: c.targets.clone() }).collect(),
            files: r.files.iter().map(|f| RecordFile { path: f.path.clone(), blob: f.blob.clone() }).collect(),
            ran: r.ran.iter().map(|x| RecordRan { claim: r.claims[x.claim].name.clone(), target: x.target.clone(), file: x.file.clone(), lines: ranges(&x.lines) }).collect(),
        }))
    }

    /// `geas affected`'s answer, as types: the same function the command prints (nothing runs).
    fn affected(&self, file: &Path, root: Option<&Path>, diff: &[u8], diff_shown: &str, records: &[String]) -> Result<ritsu_ports::Affected, Vec<Said>> {
        let spec = file.to_string_lossy().to_string();
        let root = root.map(|r| r.to_string_lossy().to_string());
        crate::affected::for_port(&spec, root.as_deref(), records, diff_shown, diff)
    }
}

impl ritsu_ports::Items for Engine {
    /// Each claim of a spec. Its definition is its block: the `claim` line and its steps, each
    /// line without the spaces around it, the blank lines and comments left out.
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        let (src, claims) = read(&ritsu_base::paths::on_disk(root, file))?;
        Ok(claims
            .into_iter()
            .map(|(name, from, to)| Item { naming: Naming::file(Tool::Geas, file).with("claim", &name), lines: (from, to), text: steps(&src, from, to).join("\n") })
            .collect())
    }
}
