//! `chobo check --diff-base <rev>`: what changed since a revision that the data already in a
//! database cannot follow (DESIGN 2.8). An account's identity and a transfer's ID leave the
//! book's version out, so the accounts and holds made under the old book go on under the new.

use crate::diag::{self, DiagExt, Diag};
use ritsu_base::text::Text;
use crate::ids;
use crate::model::{self, *};
use std::path::Path;

/// The book at `rev`, read with git; Ok(None) when the revision has no such file. What git
/// answers is read from its exit codes, not its messages, which follow the user's locale. Built
/// for WASI (ritsu's npm package), where no program starts, the error says so (ritsu-base's
/// `wasi`).
pub fn read_at(path: &Path, rev: &str) -> Result<Option<String>, Text> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().ok_or(Text::same("no file name"))?.to_string_lossy().to_string();
    let git = |args: &[&str]| {
        std::process::Command::new("git").arg("-C").arg(dir).args(args).output().map_err(|e| {
            if ritsu_base::wasi::unsupported(&e) { ritsu_base::wasi::cannot_start("git") } else { Text::same(format!("cannot run git: {e}")) }
        })
    };
    let found = git(&["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])?;
    if !found.status.success() {
        return Err(Text::same(format!("git: no revision `{rev}` in {}", dir.display())));
    }
    let spec = format!("{rev}:./{name}");
    if !git(&["cat-file", "-e", &spec])?.status.success() {
        return Ok(None);
    }
    let out = git(&["show", &spec])?;
    if !out.status.success() {
        return Err(Text::same(format!("git show {spec}: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).to_string()))
}

fn bound_word(b: &Option<Bound>) -> Text {
    match b {
        Some(b) => Text::same(b.value.to_string()),
        None => tr!("なし", "none"),
    }
}

/// The diagnostics of the change from `old` to `new`. `rev` is how the revision is named in them.
pub fn compare(old: &Book, new: &Book, rev: &str) -> Vec<Diag> {
    let mut d = Vec::new();
    if old.name != new.name {
        let (o, n) = (&old.name, &new.name);
        d.push(diag::warning(
            "W107",
            1,
            1,
            tr!(
                "帳簿の名前が {rev} の `{o}` から `{n}` に変わりました。勘定と仮押さえの ID には帳簿の名前が入るので、`{o}` の残高と仮押さえはデータベースに残ったままになります",
                "the book's name changed from `{o}` at {rev} to `{n}`: the book's name is part of every account's and hold's ID, so the balances and holds under `{o}` stay in the database"
            ),
        ));
        return d;
    }
    for oa in &old.accounts {
        let n = &oa.name;
        let Some(na) = new.accounts.iter().find(|a| a.name == oa.name) else {
            d.push(diag::warning(
                "W107",
                1,
                1,
                tr!(
                    "{rev} にあった勘定 `{n}` がありません。その残高はデータベースに残ったままになります",
                    "the account `{n}` of {rev} is gone: its balances stay in the database"
                ),
            ));
            continue;
        };
        let (ou, nu) = (&old.units[oa.unit], &new.units[na.unit]);
        let mut changes: Vec<Text> = Vec::new();
        if ou.name != nu.name {
            let (a, b) = (&ou.name, &nu.name);
            changes.push(tr!("単位（`{a}` → `{b}`）", "its unit (`{a}` → `{b}`)"));
        }
        if ou.scale != nu.scale {
            let (a, b) = (ou.scale, nu.scale);
            changes.push(tr!("`scale`（{a} → {b}）", "its `scale` ({a} → {b})"));
        }
        if oa.outside != na.outside {
            changes.push(tr!("外の勘定かどうか", "whether it is outside"));
        }
        if oa.params != na.params {
            let (a, b) = (oa.params.join(", "), na.params.join(", "));
            changes.push(tr!("引数（{a} → {b}）", "its parameters ({a} → {b})"));
        }
        if oa.lower.as_ref().map(|b| b.value) != na.lower.as_ref().map(|b| b.value) {
            let (a, b) = (bound_word(&oa.lower), bound_word(&na.lower));
            changes.push(tr!("下限（{{a}} → {{b}}）", "its lower bound ({{a}} → {{b}})").sub("a", &a).sub("b", &b));
        }
        if oa.upper.as_ref().map(|b| b.value) != na.upper.as_ref().map(|b| b.value) {
            let (a, b) = (bound_word(&oa.upper), bound_word(&na.upper));
            changes.push(tr!("上限（{{a}} → {{b}}）", "its upper bound ({{a}} → {{b}})").sub("a", &a).sub("b", &b));
        }
        if !changes.is_empty() {
            let what = Text::join(&changes, "、", ", ");
            d.push(
                diag::error(
                    "E050",
                    na.line,
                    na.col,
                    tr!(
                        "勘定 `{n}` の{{what}}が {rev} のときと違います。すでにある勘定は前の定義のまま残るので、chobo はこの変更を扱えません",
                        "the account `{n}` differs from {rev} in {{what}}: the accounts that already exist keep the old definition, so chobo cannot make this change"
                    )
                    .sub("what", &what),
                )
                .hint(tr!(
                    "新しい名前の勘定を宣言し、残高を移す振替を書いてください",
                    "declare an account under a new name, and write a transfer that moves the balances over"
                )),
            );
        }
    }
    for ot in &old.transfers {
        let n = &ot.name;
        let Some(nt) = new.transfers.iter().find(|t| t.name == ot.name) else {
            let msg = if ot.is_pending() {
                tr!(
                    "{rev} にあった振替 `{n}` がありません。押さえ中の仮押さえは、確定も取消もできないまま残ります",
                    "the transfer `{n}` of {rev} is gone: a hold of it still held can be neither posted nor voided"
                )
            } else {
                tr!(
                    "{rev} にあった振替 `{n}` がありません。呼んでいる途中の操作があれば、リトライできなくなります",
                    "the transfer `{n}` of {rev} is gone: a retry of it still in flight can no longer be made"
                )
            };
            d.push(diag::warning("W107", 1, 1, msg));
            continue;
        };
        let (a, b) = (ids::transfer_text(old, ot), ids::transfer_text(new, nt));
        if a == b {
            continue;
        }
        let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        let mut changes: Vec<Text> = Vec::new();
        if al.first() != bl.first() {
            changes.push(tr!("引数", "its parameters"));
        }
        let line_of = |ls: &[&str], head: &str| ls.iter().find(|l| l.starts_with(head)).map(|s| s.to_string());
        if line_of(&al, "key ") != line_of(&bl, "key ") {
            changes.push(tr!("キー", "its key"));
        }
        if line_of(&al, "pending ") != line_of(&bl, "pending ") {
            changes.push(tr!("仮押さえの終わり方", "how its holds end"));
        }
        let moves = |ls: &[&str]| ls.iter().filter(|l| l.starts_with("move ")).map(|s| s.to_string()).collect::<Vec<_>>();
        if moves(&al) != moves(&bl) {
            changes.push(tr!("移動", "its moves"));
        }
        let what = Text::join(&changes, "、", ", ");
        let msg = if ot.is_pending() {
            tr!(
                "振替 `{n}` の{{what}}が {rev} のときと違います。呼んでいる途中の操作をリトライすると key_conflict で拒否され、押さえ中の仮押さえを確定できなくなります",
                "the transfer `{n}` differs from {rev} in {{what}}: a retry in flight would be refused with key_conflict, and a hold still held could not be posted"
            )
        } else {
            tr!(
                "振替 `{n}` の{{what}}が {rev} のときと違います。呼んでいる途中の操作をリトライすると key_conflict で拒否されます",
                "the transfer `{n}` differs from {rev} in {{what}}: a retry in flight would be refused with key_conflict"
            )
        };
        d.push(
            diag::error("E051", nt.line, nt.col, msg.sub("what", &what))
            .hint(tr!(
                "新しい名前の振替を宣言し、前の振替は、その仮押さえがどれも終わるまで残しておいてください",
                "declare the new form under a new name, and keep the old one until every hold it made has ended"
            )),
        );
    }
    d
}

/// The diagnostics of comparing `src` with `before` (the book at `rev`), and a note when
/// there is nothing to compare.
pub fn against(src_book: &Book, before: Option<&str>, rev: &str) -> (Vec<Diag>, Option<Text>) {
    let Some(before) = before else {
        return (vec![], Some(tr!("{rev} にこのファイルは無いので、比べるものがありません", "{rev} has no such file; there is nothing to compare")));
    };
    let (old, _) = model::load(before);
    match old {
        Some(old) => (compare(&old, src_book, rev), None),
        None => (vec![], Some(tr!("{rev} の帳簿にエラーがあるので、比べられません", "the book at {rev} has errors; it cannot be compared"))),
    }
}
