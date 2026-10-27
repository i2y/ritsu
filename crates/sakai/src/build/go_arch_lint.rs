//! `.go-arch-lint.yml` for Go (DESIGN 7.5; PLAN C.10): one component a group, `in` its
//! directories, and under `deps` the components it may depend on (DESIGN 7.1). go-arch-lint reads
//! a file into the deepest component whose directory holds it, as the map does, and stops a file
//! that no component holds, so a directory the map gains and the settings lack does not pass.
//!
//! The directories the map's `except` leaves out go in `exclude`, from `workdir`. go-arch-lint
//! v1.19.0 reads only directories there, and leaves out every Go file whose path begins with one,
//! compared as strings: `legacy` leaves out `legacy2/x.go` and `legacy.go` too. Its `excludeFiles`
//! are matched against a file's absolute path, which the settings cannot know. So an `except` of
//! one Go file, and an `except` directory that the path of other code begins with, cannot be
//! written (E501, [`unwritable`]).

use super::areas::{self, Areas};
use crate::check::Checked;
use crate::diag::{self, Diag};
use crate::paths;
use ritsu_base::text::Lang;

/// A YAML string in double quotes (JSON's strings are YAML's).
fn yaml(s: &str) -> String {
    serde_json::to_string(s).expect("a string is JSON")
}

fn glob(root: &str) -> String {
    if root == "." { "**".to_string() } else { format!("{root}/**") }
}

/// The directories of `except` under the code directory, from it.
fn excluded(c: &Checked, a: &Areas) -> Vec<String> {
    a.except.iter().filter(|e| ritsu_base::fs::is_dir(paths::on_disk(&c.model.root, &paths::join(&a.dir, e).unwrap_or_default()))).cloned().collect()
}

/// Why the map's `except` cannot be written for go-arch-lint (E501), at each entry that cannot.
pub fn unwritable(c: &Checked, a: &Areas) -> Vec<Diag> {
    let m = &c.model;
    let dirs = excluded(c, a);
    let mut out = Vec::new();
    for (p, s) in m.map.except.iter().zip(&m.map.ast.except) {
        if !paths::contains(&a.dir, p) || p == &a.dir {
            continue;
        }
        let e = paths::relative(&a.dir, p);
        let se = paths::shown(p);
        if !dirs.contains(&e) {
            if e.ends_with(".go") {
                out.push(diag::at("E501", &m.map.file, s.pos.line, s.pos.col, tr!("`except` の {se} は Go のファイルを一つだけ指していて、go-arch-lint の設定に書けません", "The `except` {se} names one Go file, and the settings of go-arch-lint cannot hold it")).source(&m.map.src).note(tr!(
                    "go-arch-lint の `exclude` はディレクトリしか読みません。`excludeFiles` の正規表現はファイルの絶対パスに当たるので、リポジトリがどこにあってもこのファイルだけを外す書き方がありません。外すファイルをディレクトリにまとめ、そのディレクトリを `except` に書いてください。",
                    "go-arch-lint's `exclude` takes directories only, and its `excludeFiles` are matched against a file's absolute path, so no setting leaves out this one file wherever the repository is. Put the files to leave out in a directory, and write that directory in `except`."
                )));
            }
            continue;
        }
        if let Some(f) = a.areas.iter().flat_map(|x| x.files.iter()).find(|f| f.starts_with(e.as_str()) && !paths::contains(&e, f)) {
            let sf = paths::shown(&paths::join(&a.dir, f).unwrap_or_default());
            out.push(diag::at("E501", &m.map.file, s.pos.line, s.pos.col, tr!("go-arch-lint は `except` のディレクトリ {se} をパスの頭の文字で比べるので、{sf} も外れてしまいます", "go-arch-lint compares the `except` directory {se} with the start of each path, so it would leave out {sf} too")).source(&m.map.src).note(tr!(
                "go-arch-lint v1.19.0 は、`exclude` のディレクトリで始まるパスのファイルを、そのあとの `/` を見ずに外します。検査するはずのファイルが検査されなくなるので、ほかのファイルやディレクトリの名前がその名前で始まらないよう、どちらかの名前を変えてください。",
                "go-arch-lint v1.19.0 leaves out each file whose path starts with a directory of `exclude`, without looking for a `/` after it, and a file meant to be checked would not be. Rename one of them, so that no other file's or directory's name starts with the directory's."
            )));
        }
    }
    out
}

pub fn render(c: &Checked, a: &Areas, lang: Lang) -> (String, usize) {
    let m = &c.model;
    let names: Vec<String> = a.areas.iter().map(|x| areas::name(m, x)).collect();
    let mut s = String::from("version: 3\nworkdir: .\n\nallow:\n  depOnAnyVendor: true\n");
    let dirs = excluded(c, a);
    if !dirs.is_empty() {
        s.push_str("\nexclude:\n");
        for e in &dirs {
            s.push_str(&format!("  - {}\n", yaml(e)));
        }
    }
    s.push_str("\ncomponents:\n");
    for (i, x) in a.areas.iter().enumerate() {
        s.push_str(&format!("  # {}\n", ritsu_base::text::spaced(&areas::phrase(m, x), lang)));
        if x.roots.len() == 1 {
            s.push_str(&format!("  {}:\n    in: {}\n", names[i], yaml(&glob(&x.roots[0]))));
        } else {
            s.push_str(&format!("  {}:\n    in:\n", names[i]));
            for r in &x.roots {
                s.push_str(&format!("      - {}\n", yaml(&glob(r))));
            }
        }
    }
    s.push_str("\ndeps:\n");
    for (i, _) in a.areas.iter().enumerate() {
        // A component lists itself: go-arch-lint v1.19.0 stops an import between two packages of
        // one component that does not (`inventory` importing `inventory/ledger`).
        let may: Vec<&str> = (0..a.areas.len()).filter(|&j| a.allowed[i][j]).map(|j| names[j].as_str()).collect();
        s.push_str(&format!("  {}:\n    mayDependOn:\n", names[i]));
        for d in may {
            s.push_str(&format!("      - {d}\n"));
        }
    }
    (s, a.areas.len())
}
