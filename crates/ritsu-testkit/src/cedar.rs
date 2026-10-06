//! The official Cedar CLI (`cedar-policy-cli`), which sekisho's tests hold the Cedar it generates
//! to (sekisho's DESIGN 6.1): named by `RITSU_CEDAR` (or the crate's own `<CRATE>_CEDAR`), else
//! `cedar` on the PATH. Only the version the tests are written for is taken: what its `format` and
//! its `translate-*` print is compared a character at a time, and another version may print it
//! otherwise. The release's binaries for macOS and Linux are at
//! <https://github.com/cedar-policy/cedar/releases/tag/cedar-policy-cli-v4.13.0>; CI installs the
//! one for Linux on x86-64, held to its checksum (`.github/workflows/tools.yml`).

use crate::level::{self, Need};
use std::path::PathBuf;

/// The version of the CLI the tests are written for.
pub const VERSION: &str = "4.13.0";

/// The CLI, wherever it is and at whatever version: named by `RITSU_CEDAR` (or `<CRATE>_CEDAR`),
/// else `cedar` on the PATH when `cedar --version` runs.
pub fn find() -> Option<PathBuf> {
    crate::tools::find("CEDAR", None, "cedar", &["--version"])
}

/// What `cedar --version` prints at [`VERSION`].
pub fn version_line() -> String {
    format!("cedar-policy-cli {VERSION}")
}

/// The CLI a test may run, at [`VERSION`]. None when the level leaves it out, or the machine does
/// not have it at that version; the SKIP line then says which.
pub fn cli() -> Option<PathBuf> {
    if !level::need(Need::Cedar) {
        return None;
    }
    let Some(p) = find() else {
        crate::skip::skip(&format!("the Cedar CLI is not found (RITSU_CEDAR, or cedar on the PATH); install cedar-policy-cli {VERSION}, the release's binary for the machine"));
        return None;
    };
    let says = crate::tools::version(&p, "--version");
    if says != version_line() {
        let says = if says.is_empty() { "nothing".to_string() } else { format!("`{says}`") };
        crate::skip::skip(&format!("{} answers {says} to --version; the tests are written for cedar-policy-cli {VERSION}", p.display()));
        return None;
    }
    Some(p)
}
