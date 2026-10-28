//! What `ritsu mod` asks of a registry (DESIGN 17.12): `add` with a repository, `get`, and
//! `versions`. Stage D fills the three functions (PLAN D.5); the table reads their command
//! lines before they are called, and sends `add` here when its source is not a directory
//! ([`super::cli::is_path`]).

use super::Ctx;
use ritsu_base::cli::Args;
use ritsu_base::tr;

/// `ritsu mod add <name> <repository>[@<version>|@latest]`: the exit code. `a.pos` holds the name
/// and the source; `add` with a directory is [`super::edit::add`].
pub fn add(ctx: &Ctx, _a: &Args) -> u8 {
    ctx.refuse(tr!(
        "`ritsu mod add` のレジストリの形は、この版にはまだありません（ディレクトリの形 `./…`・`../…` は使えます）",
        "`ritsu mod add` from a registry is not in this version yet (a directory, `./…` or `../…`, is)"
    ))
}

/// `ritsu mod get <name>@<version>` and `<name>@latest`: the exit code.
pub fn get(ctx: &Ctx, _a: &Args) -> u8 {
    ctx.not_yet()
}

/// `ritsu mod versions <name>`: the exit code.
pub fn versions(ctx: &Ctx, _a: &Args) -> u8 {
    ctx.not_yet()
}
