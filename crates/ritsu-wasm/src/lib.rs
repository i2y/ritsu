//! ritsu in a page in the browser (DESIGN 8.7, PLAN F.5): `ritsu check` on the files of a small
//! project the reader edits in tabs, with what one language checks read by the next, and each
//! language's `gen` and `doc` on one of the files.
//!
//! - [`playground`]: what the page asks and what it is answered, as functions the tests run
//!   natively too. The project's files are held in memory (`ritsu_base::fs::Memory`), and every
//!   command reads them as it would read them from a directory: what it answers is what the
//!   command answers there, word for word.
//! - `wasm` (wasm32 only): the page's side of the boundary, with the convention rulec's and
//!   dandori's pages keep — every buffer that crosses it begins with its own length, as a
//!   little-endian `u32`.

pub mod playground;

#[cfg(target_arch = "wasm32")]
mod wasm;
