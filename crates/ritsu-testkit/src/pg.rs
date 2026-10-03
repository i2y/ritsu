//! A PostgreSQL cluster made for a test (koyomi's and chobo's): `initdb` into a [`TempDir`], a
//! server on a Unix socket only (`listen_addresses=''`) on a port no one listens on, stopped
//! and removed when its value is dropped. A test process that is killed leaves it running; the
//! next test process stops it (the cluster is noted in its directory).
//!
//! The programs are under `RITSU_PG_BIN` or the crate's own `<CRATE>_PG_BIN`, else where
//! `initdb` is on the PATH. A socket's path is kept to 103 bytes, so its directory is under
//! `RITSU_PG_SOCKET_DIR` (or `<CRATE>_PG_SOCKET_DIR`), else `/tmp`: the system's temporary
//! directory on macOS is too deep.

use crate::tmp::TempDir;
use crate::tools;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// The longest path of a Unix socket.
pub const SOCKET_PATH_MAX: usize = 103;

pub struct Postgres {
    pub bin: PathBuf,
    /// The directory of the socket.
    pub socket: PathBuf,
    pub port: u16,
    pub user: String,
    dir: TempDir,
}

/// Where PostgreSQL's programs are, or why there are none (for a SKIP line).
pub fn bin() -> Result<PathBuf, String> {
    let bin = match tools::from_vars("PG_BIN") {
        Some(b) => PathBuf::from(b),
        None => tools::on_path("initdb").and_then(|p| p.parent().map(Path::to_path_buf)).ok_or("PostgreSQL is not here: put initdb, pg_ctl and psql on the PATH, or set RITSU_PG_BIN")?,
    };
    for tool in ["initdb", "pg_ctl", "psql"] {
        if !bin.join(tool).is_file() {
            return Err(format!("{tool} is not in {}; set RITSU_PG_BIN to PostgreSQL's bin directory", bin.display()));
        }
    }
    Ok(bin)
}

/// Remove the socket directories of test processes that have ended.
fn sweep_sockets(base: &Path) {
    let Ok(rd) = std::fs::read_dir(base) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(pid) = name.strip_prefix("ritsu-pg-").and_then(|p| p.split('-').next()).and_then(|p| p.parse::<u32>().ok()) else { continue };
        if pid != std::process::id() && !crate::tmp::alive(pid) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

impl Postgres {
    /// A cluster, or why there is none (for a SKIP line).
    pub fn start() -> Result<Postgres, String> {
        let bin = bin()?;
        let base = tools::from_vars("PG_SOCKET_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        sweep_sockets(&base);
        let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).map_err(|e| e.to_string())?;
        let socket = base.join(format!("ritsu-pg-{}-{port}", std::process::id()));
        let longest = socket.join(format!(".s.PGSQL.{port}.lock"));
        if longest.as_os_str().len() > SOCKET_PATH_MAX {
            return Err(format!("the socket's path {} is longer than {SOCKET_PATH_MAX} bytes; set RITSU_PG_SOCKET_DIR to a shorter directory", longest.display()));
        }
        std::fs::create_dir_all(&socket).map_err(|e| format!("{}: {e}", socket.display()))?;
        let dir = TempDir::new("postgres");
        let data = dir.path().join("data");
        let user = "ritsu".to_string();
        let r = crate::run(
            Command::new(bin.join("initdb")).args(["-D", &data.to_string_lossy(), "-A", "trust", "-U", &user, "-E", "UTF8", "--no-locale", "--no-sync"]),
            Duration::from_secs(120),
        );
        if !r.ok {
            let _ = std::fs::remove_dir_all(&socket);
            return Err(format!("initdb failed: {}", r.stderr));
        }
        let options = format!("-k {} -p {port} -c listen_addresses='' -c max_connections=300 -c fsync=off -c synchronous_commit=off -c full_page_writes=off", socket.display());
        let pg = Postgres { bin: bin.clone(), socket, port, user, dir };
        let r = crate::run(
            Command::new(bin.join("pg_ctl")).args(["-D", &data.to_string_lossy(), "-o", &options, "-l", &pg.dir.path().join("pg.log").to_string_lossy(), "-w", "start"]),
            Duration::from_secs(120),
        );
        if let Some(pid) = std::fs::read_to_string(data.join("postmaster.pid")).ok().and_then(|t| t.lines().next().and_then(|l| l.trim().parse::<u32>().ok())) {
            pg.dir.note_server(pid, "postgres");
        }
        if !r.ok {
            return Err(format!("PostgreSQL did not start: {}{}", r.stderr, pg.log()));
        }
        Ok(pg)
    }

    /// psql on a database of the cluster, quiet, stopping at the first error.
    pub fn psql(&self, db: &str) -> Command {
        let mut c = Command::new(self.bin.join("psql"));
        c.args(["-X", "-q", "-v", "ON_ERROR_STOP=1", "-h", &self.socket.to_string_lossy(), "-p", &self.port.to_string(), "-U", &self.user, "-d", db])
            .env_remove("PGDATABASE")
            .env_remove("PGUSER")
            .env_remove("PGHOST")
            .env_remove("PGPORT");
        c
    }

    /// Run SQL on the database `postgres` and answer what it prints, unaligned and without
    /// headers.
    pub fn sql(&self, sql: &str) -> Result<String, String> {
        let out = self.psql("postgres").args(["-A", "-t", "-c", sql]).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
        if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).to_string()) } else { Err(String::from_utf8_lossy(&out.stderr).to_string()) }
    }

    /// The cluster's log, for a failure to show.
    pub fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("pg.log")).unwrap_or_default()
    }
}

impl Drop for Postgres {
    fn drop(&mut self) {
        let data = self.dir.path().join("data");
        let _ = crate::run(Command::new(self.bin.join("pg_ctl")).args(["-D", &data.to_string_lossy(), "-m", "immediate", "-w", "stop"]), Duration::from_secs(60));
        let _ = std::fs::remove_dir_all(&self.socket);
    }
}
