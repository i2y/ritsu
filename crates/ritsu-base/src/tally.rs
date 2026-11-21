//! How many times each file has been checked whole on this thread, by the language and the file
//! (DESIGN 4.15): what the tests of "a file is checked once in a run" read. A language's port counts
//! each check it makes here, where it counts it for its own `checks()`; rulec counts, too, each time
//! it reads a file of the run to tell whether it is a contribution (`rulec.contributions`), and each
//! time it walks a contribution file of the run (`rulec.walks`), which a run does once for each; and
//! ringi counts each file of data an organization or masters read (`ringi.data`), which the check of
//! that `.ringi` reads once. The file is told by its path made absolute and folded as the system
//! opens it
//! ([`crate::paths::opened`]), so two spellings of one path are one file.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

thread_local! {
    static TALLY: RefCell<BTreeMap<(&'static str, PathBuf), usize>> = const { RefCell::new(BTreeMap::new()) };
}

/// One whole check of `file` by `tool` (or, under `rulec.contributions`, one read of it to tell
/// whether it is a contribution, and under `rulec.walks`, one walk of it).
pub fn count(tool: &'static str, file: &Path) {
    let key = (tool, crate::paths::opened(file));
    TALLY.with(|t| *t.borrow_mut().entry(key).or_insert(0) += 1);
}

/// Every count since the last time they were taken, by the language and the file; the counts start
/// over.
pub fn take() -> Vec<(&'static str, PathBuf, usize)> {
    TALLY.with(|t| std::mem::take(&mut *t.borrow_mut())).into_iter().map(|((tool, file), n)| (tool, file, n)).collect()
}
