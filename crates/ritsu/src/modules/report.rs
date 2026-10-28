//! `ritsu mod report --base <revision>` (DESIGN 17.10): what raising the dependencies changes, for a
//! pull request. Stage C fills [`report`] (PLAN C.5); the table reads its command line (`--base`,
//! `--head`, `--format text|markdown|json`, `--fail-on-change`, `--budget`) before it is called.

use super::Ctx;
use ritsu_base::cli::Args;

/// `ritsu mod report`: the exit code. `--base` is required, and this function is the one to say so.
pub fn report(ctx: &Ctx, _a: &Args) -> u8 {
    ctx.not_yet()
}
