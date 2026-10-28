//! `ritsu mod fetch` (DESIGN 17.12): the modules of the lock, or the versions named, fetched from
//! their registries into the store, each checked against its digests and its signatures. Stage D
//! fills [`fetch`] (PLAN D.5), and [`for_choosing`], which `ritsu mod lock`, `add` and `remove`
//! call when a version they choose by is in neither the store nor the module's copy of it.

use super::Ctx;
use ritsu_base::cli::Args;
use ritsu_base::modules::names::ModuleName;
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::Store;
use ritsu_base::text::Text;
use ritsu_base::tr;

/// `ritsu mod fetch [<name>@<version>...]`: the exit code.
pub fn fetch(ctx: &Ctx, _a: &Args) -> u8 {
    ctx.not_yet()
}

/// The version `version` of the module `name`, which the choice of the versions reaches (DESIGN
/// 17.7) and which is in neither the store nor the module's copy: its `ritsu.mod` layer fetched from
/// its registry and placed in `store` (DESIGN 17.11, 17.15), and the digest of its OCI manifest, from
/// which the store gives the `ritsu.mod` and the record. In this version nothing is fetched: Err says
/// what is missing, and the command stops (exit 2).
pub fn for_choosing(_store: Option<&Store>, name: &ModuleName, version: &Version) -> Result<String, Text> {
    Err(tr!(
        "依存 `{name}` {version} が、置き場にも同梱のコピーにもありません。`ritsu mod fetch` を走らせてください",
        "the dependency `{name}` {version} is neither in the store nor in this module's copy of it; run `ritsu mod fetch`"
    ))
}
