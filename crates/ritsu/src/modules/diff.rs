//! `ritsu mod diff <old> [<new>]` (DESIGN 17.9.6): the differences between two versions of a module,
//! its files, its `require`s, the behavior of each file that changed and its interface. Stage C
//! fills [`diff`] (PLAN C.4); the table reads its command line (`<old>`, `<new>`, `--format
//! text|markdown|json`, `--budget`) before it is called.

use super::Ctx;
use ritsu_base::cli::Args;

/// `ritsu mod diff`: the exit code. `a.pos` holds `<old>` and maybe `<new>`.
pub fn diff(ctx: &Ctx, _a: &Args) -> u8 {
    ctx.not_yet()
}
