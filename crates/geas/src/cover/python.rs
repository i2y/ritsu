//! Reading what geas's `sitecustomize.py` wrote: one `python-<pid>.json` a process,
//! `{"files":{"<absolute path>":{"code":[…],"ran":[…]}}}`.

use super::{FileLines, Report};
use crate::json::{self, J};
use crate::tree;
use std::path::Path;

/// The files under `root` that one process reported. `root` is absolute and
/// canonical, as the hook was given it.
pub fn read(text: &str, root: &Path) -> Result<Report, String> {
    let v = json::parse(text)?;
    let J::Obj(top) = &v else {
        return Err("not a JSON object".into());
    };
    let Some((_, J::Obj(files))) = top.iter().find(|(k, _)| k == "files") else {
        return Err("no `files` object".into());
    };
    let mut out = Report::new();
    for (path, entry) in files {
        let Some(rel) = tree::relative(root, Path::new(path)) else {
            continue;
        };
        if !tree::is_source(&rel) {
            continue;
        }
        let lines = |key: &str| -> Result<crate::lines::Lines, String> {
            let J::Obj(e) = entry else {
                return Err(format!("the entry of {rel} is not an object"));
            };
            match e.iter().find(|(k, _)| k == key) {
                Some((_, J::Arr(items))) => items
                    .iter()
                    .map(|i| match i {
                        J::Num(n) if *n >= 1.0 && n.fract() == 0.0 => Ok(*n as u32),
                        _ => Err(format!("a line of {rel} is not a line number")),
                    })
                    .collect(),
                _ => Err(format!("the entry of {rel} has no `{key}` list")),
            }
        };
        let f = FileLines { code: lines("code")?, ran: lines("ran")? };
        super::add(&mut out, rel, f);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What geas's hook wrote for `python3 shapes.py square 3`.
    #[test]
    fn a_recorded_run() {
        use crate::cover::fixtures;
        let r = read(&fixtures::read("python", "python-1.json"), Path::new("/ROOT")).unwrap();
        fixtures::golden("python.txt", &fixtures::render(&r));
    }

    #[test]
    fn the_hooks_json() {
        let text = r#"{"files":{"/ROOT/calc.py":{"code":[1,3,4],"ran":[1,3]},"/ROOT/.geas/hook/x.py":{"code":[1],"ran":[1]},"/elsewhere/y.py":{"code":[1],"ran":[1]}}}"#;
        let r = read(text, Path::new("/ROOT")).unwrap();
        assert_eq!(r.keys().collect::<Vec<_>>(), ["calc.py"]);
        assert_eq!(crate::lines::to_ranges(&r["calc.py"].code), "1,3-4");
        assert_eq!(crate::lines::to_ranges(&r["calc.py"].ran), "1,3");
        assert!(read("[]", Path::new("/ROOT")).is_err());
        assert!(read(r#"{"files":{"/ROOT/a.py":{"code":[0],"ran":[]}}}"#, Path::new("/ROOT")).is_err());
    }
}
