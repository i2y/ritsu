//! What a fetch writes in the store (DESIGN 17.15), beside the entries ritsu-base's `store::place`
//! makes under `tmp/` and renames into their place: the directory curl's files go to, the record of
//! the signatures checked of an entry that is there already, an entry whose files differ from the
//! lock taken away so that a fetch puts it again, and the day a record says.

use ritsu_base::modules::store::{self, Record, Store};
use ritsu_base::text::Text;
use ritsu_base::tr;
use std::path::PathBuf;

/// The directory of the files a request writes and removes (curl's, a push's copy for its check):
/// the store's `tmp/`, made when it is not there; the system's when no store has a place (no
/// `HOME`).
pub fn scratch() -> Result<PathBuf, Text> {
    let dir = Store::located().map(|s| s.root.join("tmp")).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).map_err(|e| {
        let d = dir.display();
        tr!("{d} を作れません（{e}）", "cannot make {d} ({e})")
    })?;
    Ok(dir)
}

/// A new name in `dir` that no other run takes.
pub fn unique(dir: &std::path::Path, what: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    dir.join(format!("{what}-{}-{nanos}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)))
}

/// Today in UTC, `YYYY-MM-DD`: the day a record says.
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The record of the entry of `digest` in `store` written again with `verified`: to a new file
/// beside it, then renamed over it, so that a reader sees the old record or the new one.
pub fn set_verified(store: &Store, digest: &str, verified: Vec<(String, String)>) -> Result<(), Text> {
    let Some(mut r) = store.record(digest) else {
        let d = store.entry(digest).display().to_string();
        return Err(tr!("{d} の record.json を読めません", "cannot read the record.json of {d}"));
    };
    r.verified = verified;
    write_record(store, digest, &r)
}

fn write_record(store: &Store, digest: &str, r: &Record) -> Result<(), Text> {
    let dir = store.entry(digest);
    let file = dir.join("record.json");
    let tmp = unique(&dir, "record.json");
    let disk = |e: std::io::Error| {
        let d = file.display();
        tr!("{d} に書けません（{e}）", "cannot write {d} ({e})")
    };
    std::fs::write(&tmp, r.to_json().pretty() + "\n").map_err(disk)?;
    if let Err(e) = std::fs::rename(&tmp, &file) {
        let _ = std::fs::remove_file(&tmp);
        return Err(disk(e));
    }
    Ok(())
}

/// The entry of `digest` taken out of `store`, so that the files fetched again take its place.
pub fn remove_entry(store: &Store, digest: &str) -> Result<(), Text> {
    let dir = store.entry(digest);
    match store::remove(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => {
            let d = dir.display();
            Err(tr!("{d} を消せません（{e}）", "cannot remove {d} ({e})"))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn today_is_a_day() {
        let t = super::today();
        assert_eq!((t.len(), &t[4..5], &t[7..8]), (10, "-", "-"), "{t}");
        assert!(t.as_str() >= "2026-01-01", "{t}");
    }
}
