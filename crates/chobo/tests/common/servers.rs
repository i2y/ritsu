//! The databases a test process starts are ritsu-testkit's: a throwaway PostgreSQL cluster, one
//! TigerBeetle replica and a throwaway MySQL server (or databases on a TiDB cluster of the
//! machine's), each stopped, and its files removed, when its value is dropped, a failing test's too
//! (a test process that is killed leaves them behind, and the next one stops them). What chobo's
//! tests ask of them besides is here.
#![allow(dead_code)]

pub use ritsu_testkit::mysql::MySql;
pub use ritsu_testkit::pg::Postgres;
pub use ritsu_testkit::tigerbeetle::TigerBeetle;
use serde_json::{Value, json};
use std::process::Command;

/// PostgreSQL as the tests use it: the books are built into the database `postgres`.
pub trait Pg {
    /// psql on that database, stopping at the first error.
    fn db(&self) -> Command;
    /// Run SQL there, and answer what it prints, unaligned and without headers; a failure fails
    /// the test.
    fn exec(&self, sql: &str) -> String;
    /// What a client is given to connect.
    fn connection(&self) -> Value;
}

impl Pg for Postgres {
    fn db(&self) -> Command {
        self.psql("postgres")
    }

    fn exec(&self, sql: &str) -> String {
        self.sql(sql).unwrap_or_else(|e| panic!("{sql}\n{e}"))
    }

    fn connection(&self) -> Value {
        json!({"host": self.socket.to_str().unwrap(), "port": self.port, "database": "postgres", "user": self.user})
    }
}

/// TigerBeetle as the tests use it.
pub trait Tb {
    /// What a client is given to connect.
    fn connection(&self) -> Value;
}

impl Tb for TigerBeetle {
    fn connection(&self) -> Value {
        json!({"cluster": "0", "addresses": [self.address]})
    }
}

/// A server of the MySQL family as the tests use it: each test makes a database of its own.
pub trait My {
    /// What a client is given to connect to `db`; `batch` lets a call send its statements in a
    /// row in one request.
    fn connection(&self, db: &str, batch: bool) -> Value;
    /// Run a file of SQL in `db` through the client, stopping at the first error.
    fn load(&self, db: &str, file: &std::path::Path) -> Result<(), String>;
}

impl My for MySql {
    fn connection(&self, db: &str, batch: bool) -> Value {
        json!({"host": self.host, "port": self.port, "database": db, "user": self.user, "batch": batch})
    }

    fn load(&self, db: &str, file: &std::path::Path) -> Result<(), String> {
        let f = std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let out = self.mysql(db).stdin(f).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        // on a sharded Vitess, every table of the book is sharded by its tenant (DESIGN 4.5)
        let sql = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        for line in sql.lines() {
            if let Some(rest) = line.strip_prefix("create table if not exists `") {
                let table = rest.split("` (").next().unwrap_or(rest).replace("``", "`");
                self.shard_by(&table, "tenant")?;
            }
        }
        Ok(())
    }
}
