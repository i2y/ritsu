//! `ritsu mod push` (DESIGN 17.12): the module checked, built into its OCI artifact and pushed to the
//! registry its name says. Stage D fills [`push`] (PLAN D.4); the table reads its command line
//! (`--dry-run`, `--format text|json`) before it is called.

use super::Ctx;
use ritsu_base::cli::Args;

/// `ritsu mod push`: the exit code.
pub fn push(ctx: &Ctx, _a: &Args) -> u8 {
    ctx.not_yet()
}
