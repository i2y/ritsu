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

/// The function is the other door on the same query (§15.80). What matters most is that its
/// body **is** the query: the file holds the text of `<alias>.sql` from the first CTE to the
/// last `ORDER BY`, unchanged, so the two shapes cannot drift apart.
#[test]
fn 関数の本体は問い合わせそのものである() {
    let dir = generate("function", "tests/corpus/送料.rule");
    let query = std::fs::read_to_string(dir.join("sql/shipping_fee.sql")).unwrap();
    let f = std::fs::read_to_string(dir.join("sql/shipping_fee_function.sql")).unwrap();
    let body = {
        let from = query.find("\"_c0\" AS (").expect("問い合わせに _c0 が無い");
        let to = query.rfind("ORDER BY \"_id\";").expect("問い合わせに ORDER BY が無い");
        &query[from..to + "ORDER BY \"_id\"".len()]
    };
    assert!(f.contains(body), "関数の本体が問い合わせと違う:\n{f}");
    for want in [
        // The arguments are the inputs, and what comes back is the outputs and the rows.
        "CREATE FUNCTION \"shipping_fee\"(\"dest\" text, \"weight\" bigint, \"total\" bigint, \"member\" text)",
        "RETURNS TABLE (\"fee\" bigint, \"base_fee_row\" int, \"payer_row\" int)",
        "LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE",
        // The argument reaches the query through this one CTE and nowhere else.
        "\"shipping_fee_input\" AS (\n  SELECT 0 AS \"_id\", \"dest\" AS \"dest\"",
        "#variable_conflict use_column",
        // It raises where the query returns a column.
        "RAISE EXCEPTION '%', \"_r\".\"_input_error\" USING ERRCODE = '22023';",
        // Any older signature of the same name goes first: `CREATE OR REPLACE` would leave it.
        "WHERE p.proname = 'shipping_fee' AND n.nspname = current_schema()",
        // A record's column is whatever the expression made it, and RETURN QUERY is exact.
        "RETURN QUERY SELECT \"_r\".\"fee\"::bigint,",
    ] {
        assert!(f.contains(want), "無い: {want}\n{f}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apiはsqlの関数を言う() {
    let (c, out, e) = run(&["api", "tests/corpus/送料.rule"]);
    assert_eq!(c, 0, "{e}");
    let j = rulec::json::parse(out.trim()).unwrap();
    let f = j.get("sql").and_then(|s| s.get("function")).expect("sql.function が無い");
    assert_eq!(f.get("file").and_then(|v| v.as_str()), Some("shipping_fee_function.sql"));
    assert_eq!(f.get("name").and_then(|v| v.as_str()), Some("shipping_fee"));
    assert_eq!(f.get("language").and_then(|v| v.as_str()), Some("plpgsql"));
    assert_eq!(f.get("raises").and_then(|v| v.as_str()), Some("22023"));
    let sig = f.get("signature").and_then(|v| v.as_str()).expect("signature が無い");
    assert!(sig.starts_with("\"shipping_fee\"(\"dest\" text,"), "{sig}");
    assert!(sig.contains("RETURNS TABLE (\"fee\" bigint,"), "{sig}");
    // And it is the signature the file really declares, not a second spelling of it.
    let dir = generate("apisig", "tests/corpus/送料.rule");
    let decl = std::fs::read_to_string(dir.join("sql/shipping_fee_function.sql")).unwrap().replace('\n', " ");
    assert!(decl.contains(sig), "api の signature が生成物と違う: {sig}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The function on a real PostgreSQL: the three examples by argument name, and an input
/// outside the declaration raising rather than answering. `rulec test` runs the whole vector
/// set the same way; this is the shape of one call, and it is skipped where there is no
/// server, as everything that needs a toolchain is.
#[test]
fn 関数は宣言の外の入力に投げる() {
    if !have("psql") || !rulec::backend::psql_ready() {
        eprintln!("注意: psql が無いか繋がらないので飛ばした");
        return;
    }
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let dir = generate("pg", "tests/corpus/送料.rule");
    let script = r#"
import json, subprocess
PSQL = ["psql", "-X", "-q", "-A", "-t", "-v", "ON_ERROR_STOP=1"]
sql = open("sql/shipping_fee_function.sql", encoding="utf-8").read()
call = 'SELECT row_to_json(t) FROM "shipping_fee"("dest" => %s, "weight" => %s, "total" => %s, "member" => %s) AS t;'
ok = subprocess.run(PSQL, input=sql + "\n" + "\n".join([
    call % ("'沖縄県'", 2500, 40000, "'一般'"),
    call % ("'東京都'", 1999, 12000, "'プラチナ'"),
    call % ("'北海道'", 500, 5000, "'一般'"),
]), capture_output=True, text=True)
bad = subprocess.run(PSQL, input=call % ("'東京都'", 50000, 100, "'一般'"), capture_output=True, text=True)
subprocess.run(PSQL, input='DROP FUNCTION IF EXISTS "shipping_fee"(text, bigint, bigint, text);', capture_output=True, text=True)
print(json.dumps({
    "answers": [json.loads(l) for l in ok.stdout.splitlines() if l.startswith("{")],
    "refused": bad.returncode != 0 and "重量 が範囲の外です" in bad.stderr,
}, ensure_ascii=False))
"#;
    let o = Command::new("python3").current_dir(&dir).args(["-c", script]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let got = String::from_utf8_lossy(&o.stdout).trim().to_string();
    assert_eq!(
        got,
        r#"{"answers": [{"fee": 0, "base_fee_row": 2, "payer_row": 1}, {"fee": 400, "base_fee_row": 3, "payer_row": 2}, {"fee": 1200, "base_fee_row": 1, "payer_row": 3}], "refused": true}"#
    );
    let _ = std::fs::remove_dir_all(&dir);
}
