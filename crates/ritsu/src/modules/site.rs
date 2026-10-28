//! `ritsu mod site <list>` (DESIGN 17.17): the site that lists modules, their versions, the pages of
//! their files and the differences between versions. Stage F fills [`site`] (PLAN F.2); the table
//! reads its command line (`--out`, `--playground`, `--lang`) before it is called.

use super::Ctx;
use ritsu_base::cli::Args;

/// `ritsu mod site <list>`: the exit code. `--out` is required, and this function is the one to say
/// so.
pub fn site(ctx: &Ctx, _a: &Args) -> u8 {
    ctx.not_yet()
}
