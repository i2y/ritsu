//! SQL as the eighth target (§15.46). `rulec test` already holds the query to the reference
//! evaluator over every vector of every corpus rule, through SQLite. What is checked here is
//! the rest of the contract: the shape of the file, the entry in `rulec api`, and that the
//! query really runs as a relation — many rows in one statement, in order.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(args).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn generate(tag: &str, rule: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-sql-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (c, _, e) = run(&["gen", rule, "--out", dir.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    dir
}

#[test]
fn 問い合わせは一つの関係に対して書かれている() {
    let dir = generate("shape", "tests/corpus/送料.rule");
    let sql = std::fs::read_to_string(dir.join("sql/shipping_fee.sql")).unwrap();
    // The input relation, its numbers widened, the row comments, the row column per table,
    // the rounding as arithmetic, and the order of the rows.
    for want in [
        "FROM \"shipping_fee_input\"",
        "CAST(\"weight\" AS BIGINT) AS \"weight\"",
        "-- 行1: 遠隔地 | <=2000g | 1200円",
        "\"dest\" IN ('北海道', '沖縄県')",
        "END AS \"base_fee_row\"",
        "END AS \"_input_error\"",
        "CASE \"base_fee_row\" WHEN 1 THEN 1200",
        "ABS(\"_raw_fee\") % 20",
        "ORDER BY \"_id\";",
    ] {
        assert!(sql.contains(want), "無い: {want}\n{sql}");
    }
    assert!(!sql.contains("_round_up("), "丸めが関数呼び出しのまま: {sql}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apiはsqlの入口を言う() {
    let (c, out, e) = run(&["api", "tests/corpus/送料.rule"]);
    assert_eq!(c, 0, "{e}");
    let j = rulec::json::parse(out.trim()).unwrap();
    let sql = j.get("sql").expect("sql が無い");
    assert_eq!(sql.get("file").and_then(|v| v.as_str()), Some("shipping_fee.sql"));
    assert_eq!(sql.get("input").and_then(|v| v.as_str()), Some("shipping_fee_input"));
    assert_eq!(sql.get("id").and_then(|v| v.as_str()), Some("_id"));
    let cols = match sql.get("columns") {
        Some(rulec::json::Json::Arr(a)) => a,
        _ => panic!("columns が無い"),
    };
    assert_eq!(cols.len(), 4);
    assert_eq!(cols[1].get("alias").and_then(|v| v.as_str()), Some("weight"));
    assert_eq!(cols[1].get("type").and_then(|v| v.as_str()), Some("bigint"));
    assert_eq!(cols[0].get("type").and_then(|v| v.as_str()), Some("text"));
    let rows = match sql.get("rows") {
        Some(rulec::json::Json::Arr(a)) => a,
        _ => panic!("rows が無い"),
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get("table").and_then(|v| v.as_str()), Some("基本送料"));
    assert_eq!(rows[0].get("column").and_then(|v| v.as_str()), Some("base_fee_row"));
}

/// The query is one statement over a relation: many rows in, many rows out, in order — the
/// thing the per-row functions cannot do. Run it on SQLite over a small relation built by
/// hand, not through the runner, and read the answers back by id.
#[test]
fn 問い合わせは関係ごと答える() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let dir = generate("relation", "tests/corpus/送料.rule");
    let script = r#"
import json, sqlite3, sys
sql = open("sql/shipping_fee.sql", encoding="utf-8").read()
db = sqlite3.connect(":memory:")
db.create_function("LEAST", 2, min)
db.create_function("GREATEST", 2, max)
db.execute('CREATE TABLE "shipping_fee_input" ("_id" INTEGER, "dest" TEXT, "weight" INTEGER, "total" INTEGER, "member" TEXT)')
db.executemany('INSERT INTO "shipping_fee_input" VALUES (?, ?, ?, ?, ?)', [
    (2, "東京都", 1999, 12000, "プラチナ"),
    (1, "沖縄県", 2500, 40000, "一般"),
    (3, "北海道", 500, 5000, "一般"),
])
print(json.dumps([list(r) for r in db.execute(sql)], ensure_ascii=False))
"#;
    let o = Command::new("python3").current_dir(&dir).args(["-c", script]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let got = String::from_utf8_lossy(&o.stdout).trim().to_string();
    // The three examples of the rule, back in id order, each with its fee and its rows.
    assert_eq!(
        got,
        "[[1, \"沖縄県\", 2500, 40000, \"一般\", 0, 2, 1, null], [2, \"東京都\", 1999, 12000, \"プラチナ\", 400, 3, 2, null], [3, \"北海道\", 500, 5000, \"一般\", 1200, 1, 3, null]]"
    );

    // Outside the declared domain, the row comes back with the sentence, not a number the
    // proof never covered; a float in a column of steps is refused, not truncated.
    let script = r#"
import json, sqlite3
sql = open("sql/shipping_fee.sql", encoding="utf-8").read()
db = sqlite3.connect(":memory:")
db.create_function("LEAST", 2, min)
db.create_function("GREATEST", 2, max)
db.execute('CREATE TABLE "shipping_fee_input" ("_id" INTEGER, "dest" TEXT, "weight" NUMERIC, "total" INTEGER, "member" TEXT)')
db.executemany('INSERT INTO "shipping_fee_input" VALUES (?, ?, ?, ?, ?)', [
    (1, "江戸", 500, 100, "一般"),
    (2, "東京都", 50000, 100, "一般"),
    (3, "東京都", 18.3, 100, "一般"),
    (4, "東京都", None, 100, "一般"),
])
print(json.dumps([r[-1] for r in db.execute(sql)], ensure_ascii=False))
"#;
    let o = Command::new("python3").current_dir(&dir).args(["-c", script]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        String::from_utf8_lossy(&o.stdout).trim(),
        "[\"届け先 が列挙 都道府県 の値ではありません\", \"重量 が範囲の外です\", \"重量 が整数ではありません\", \"重量 がありません\"]"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
