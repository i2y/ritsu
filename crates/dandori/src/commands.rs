//! What the commands print and build, for the binary and ritsu (its commands and its playground) to
//! say the same.

use crate::diag::{Diag, Lang};
use crate::model::Model;

/// The platforms `gen` writes for, as `--target` names them, in the order `gen` writes them
/// (DESIGN 21).
pub const TARGETS: [&str; 7] = ["asl", "temporal-typescript", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"];

/// The names of `--target` up to an earlier version, which `gen` and `run` stop at and say the name
/// of now (DESIGN 21): `(old, new, version)`.
pub const RENAMED: [(&str, &str, &str); 1] = [("temporal", "temporal-typescript", "0.41.0")];

/// The files a target's build writes, by their paths under its directory of `--out`
/// (`<out>/<target>/`), or the diagnostics of what the platform cannot do.
pub type Built = Result<Vec<(String, String)>, Vec<Diag>>;

/// Build for a target; None for a target there is not. A name the target's code cannot keep stops
/// it (E051), with whatever else the platform cannot do.
pub fn build(m: &Model, target: &str) -> Option<Built> {
    let built = match target {
        "asl" => crate::asl::build(m),
        "temporal-typescript" => crate::temporal::build(m),
        "temporal-python" => crate::temporal_py::build(m),
        "temporal-go" => crate::temporal_go::build(m),
        "durable" => crate::temporal::build_flavor(m, crate::temporal::Flavor::Durable),
        "argo" => crate::argo::build(m),
        "pydantic-graph" => crate::pydantic_graph::build(m),
        _ => return None,
    };
    let stops = crate::spellings::stops(m, target);
    Some(match built {
        Ok(files) if stops.is_empty() => Ok(files),
        Ok(_) => Err(stops),
        Err(mut diags) => {
            diags.extend(stops);
            Err(diags)
        }
    })
}

/// The diagnostics as a person reads them.
pub fn render(diags: &[Diag], file: &str, src: &str, lang: Lang) -> String {
    diags.iter().map(|d| d.render(file, src, lang)).collect()
}

/// The line `check` ends a file that passes with.
pub fn passed(file: &str, warnings: usize, lang: Lang) -> String {
    match lang {
        Lang::Ja => format!("{file}: 検査を通りました{}\n", if warnings > 0 { format!("（警告 {warnings} 件）") } else { String::new() }),
        Lang::En => format!("{file}: ok{}\n", if warnings > 0 { format!(" ({warnings} warning(s))") } else { String::new() }),
    }
}
