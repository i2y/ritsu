//! The databases a test process starts are ritsu-testkit's: a throwaway PostgreSQL cluster and one
//! TigerBeetle replica, each stopped, and its files removed, when its value is dropped, a failing
//! test's too (a test process that is killed leaves them behind, and the next one stops them).
//! What chobo's tests ask of them besides is here.
#![allow(dead_code)]

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
