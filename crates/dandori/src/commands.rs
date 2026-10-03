//! What the commands print and build, for the binary and the playground to say the same.

use crate::diag::{Diag, Lang};
use crate::model::Model;

/// The platforms `build` writes for, as `--target` names them.
pub const TARGETS: [&str; 7] = ["asl", "temporal", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"];

/// The files a build writes, by their paths under `--out`, or the diagnostics of what the
/// platform cannot do.
pub type Built = Result<Vec<(String, String)>, Vec<Diag>>;

/// Build for a target; None for a target there is not.
pub fn build(m: &Model, target: &str) -> Option<Built> {
    Some(match target {
        "asl" => crate::asl::build(m),
        "temporal" => crate::temporal::build(m),
        "temporal-python" => crate::temporal_py::build(m),
        "temporal-go" => crate::temporal_go::build(m),
        "durable" => crate::temporal::build_flavor(m, crate::temporal::Flavor::Durable),
        "argo" => crate::argo::build(m),
        "pydantic-graph" => crate::pydantic_graph::build(m),
        _ => return None,
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
