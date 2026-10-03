//! The databases a test process starts: a throwaway PostgreSQL cluster and one TigerBeetle
//! replica. Each is stopped, and its files removed, when its value is dropped, a failing test's
//! too. A test process that is killed leaves them behind; the next one stops them (`sweep`).
#![allow(dead_code)]

use super::*;
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Write a server's process ID where `sweep` finds it if this test process dies first.
fn note_server(dir: &TempDir, pid: u32) {
    let f = dir.path().join("servers");
    let mut text = std::fs::read_to_string(&f).unwrap_or_default();
    text.push_str(&format!("{pid}\n"));
    std::fs::write(f, text).unwrap();
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|f| f.is_file()))
}

pub struct Postgres {
    pub bin: PathBuf,
    /// the directory of the socket
    pub socket: PathBuf,
    pub port: u16,
    pub user: String,
    dir: TempDir,
}

impl Postgres {
    /// A throwaway cluster, or why there is none (for a SKIP line).
    pub fn start() -> Result<Postgres, String> {
        let bin = match std::env::var_os("CHOBO_PG_BIN") {
            Some(b) => PathBuf::from(b),
            None => on_path("initdb").and_then(|p| p.parent().map(Path::to_path_buf)).ok_or("PostgreSQL is not here: put initdb, pg_ctl and psql on the PATH, or set CHOBO_PG_BIN")?,
        };
        for tool in ["initdb", "pg_ctl", "psql"] {
            if !bin.join(tool).is_file() {
                return Err(format!("{} is not in {}; set CHOBO_PG_BIN to PostgreSQL's bin directory", tool, bin.display()));
            }
        }
        // the socket: under CHOBO_PG_SOCKET_DIR when it is set, for its path is kept to 103 bytes
        let base = std::env::var_os("CHOBO_PG_SOCKET_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        sweep_sockets(&base);
        let socket = base.join(format!("chobo-pg-{}", std::process::id()));
        let port = 5432;
        let longest = socket.join(format!(".s.PGSQL.{port}.lock"));
        if longest.as_os_str().len() > 103 {
            return Err(format!("the socket's path {} is longer than 103 bytes; set CHOBO_PG_SOCKET_DIR to a shorter directory", longest.display()));
        }
        std::fs::create_dir_all(&socket).map_err(|e| format!("{}: {e}", socket.display()))?;
        let dir = TempDir::new("postgres");
        let data = dir.path().join("data");
        let user = "chobo".to_string();
        let out = Command::new(bin.join("initdb"))
            .args(["-D", data.to_str().unwrap(), "-A", "trust", "-U", &user, "-E", "UTF8", "--no-locale", "--no-sync"])
            .output()
            .map_err(|e| format!("initdb: {e}"))?;
        if !out.status.success() {
            return Err(format!("initdb failed: {}", String::from_utf8_lossy(&out.stderr)));
        }
        let options = format!(
            "-k {} -p {port} -c listen_addresses='' -c max_connections=300 -c fsync=off -c synchronous_commit=off -c full_page_writes=off",
            socket.display()
        );
        let pg = Postgres { bin: bin.clone(), socket, port, user, dir };
        let out = Command::new(bin.join("pg_ctl"))
            .args(["-D", data.to_str().unwrap(), "-o", &options, "-l", pg.dir.path().join("pg.log").to_str().unwrap(), "-w", "start"])
            .stdout(Stdio::null())
            .output()
            .map_err(|e| format!("pg_ctl: {e}"))?;
        if let Ok(pid) = std::fs::read_to_string(data.join("postmaster.pid")) {
            if let Some(Ok(pid)) = pid.lines().next().map(|l| l.trim().parse::<u32>()) {
                note_server(&pg.dir, pid);
            }
        }
        if !out.status.success() {
            let log = std::fs::read_to_string(pg.dir.path().join("pg.log")).unwrap_or_default();
            return Err(format!("PostgreSQL did not start: {}{log}", String::from_utf8_lossy(&out.stderr)));
        }
        Ok(pg)
    }

    /// psql on the cluster's database, stopping at the first error.
    pub fn psql(&self) -> Command {
        let mut c = Command::new(self.bin.join("psql"));
        c.args(["-X", "-q", "-v", "ON_ERROR_STOP=1", "-h", self.socket.to_str().unwrap(), "-p", &self.port.to_string(), "-U", &self.user, "-d", "postgres"]);
        c
    }

    /// Run SQL, and answer what it prints, unaligned and without headers.
    pub fn sql(&self, sql: &str) -> String {
        let out = self.psql().args(["-A", "-t", "-c", sql]).output().unwrap();
        assert!(out.status.success(), "{sql}\n{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    pub fn connection(&self) -> serde_json::Value {
        serde_json::json!({"host": self.socket.to_str().unwrap(), "port": self.port, "database": "postgres", "user": self.user})
    }

    /// The cluster's log, for a failure to show.
    pub fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("pg.log")).unwrap_or_default()
    }
}

impl Drop for Postgres {
    fn drop(&mut self) {
        let data = self.dir.path().join("data");
        let _ = Command::new(self.bin.join("pg_ctl")).args(["-D", data.to_str().unwrap(), "-m", "immediate", "-w", "stop"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = std::fs::remove_dir_all(&self.socket);
    }
}

/// Remove the socket directories of test processes that have ended.
fn sweep_sockets(base: &Path) {
    let Ok(rd) = std::fs::read_dir(base) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(pid) = name.strip_prefix("chobo-pg-").and_then(|p| p.parse::<u32>().ok()) else { continue };
        if pid != std::process::id() && !alive(pid) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

pub struct TigerBeetle {
    child: Child,
    pub address: String,
    dir: TempDir,
}

impl TigerBeetle {
    /// One replica on a data file of its own, or why there is none (for a SKIP line).
    pub fn start() -> Result<TigerBeetle, String> {
        let bin = match std::env::var_os("CHOBO_TIGERBEETLE") {
            Some(b) => PathBuf::from(b),
            None => root().join("tools/tigerbeetle/tigerbeetle"),
        };
        if !bin.is_file() {
            return Err("TigerBeetle is not here: run tools/tigerbeetle/fetch.sh, or set CHOBO_TIGERBEETLE".into());
        }
        let dir = TempDir::new("tigerbeetle");
        let data = dir.path().join("0_0.tigerbeetle");
        // TigerBeetle copies itself into the temporary directory to run (256 MB a time), and a killed
        // replica leaves the copy there: give it the test's own, which goes with the test
        let out = Command::new(&bin)
            .env("TMPDIR", dir.path())
            .args(["format", "--cluster=0", "--replica=0", "--replica-count=1", "--development", data.to_str().unwrap()])
            .output()
            .map_err(|e| format!("{}: {e}", bin.display()))?;
        if !out.status.success() {
            return Err(format!("tigerbeetle format failed: {}", String::from_utf8_lossy(&out.stderr)));
        }
        let port = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).map_err(|e| e.to_string())?;
        let address = format!("127.0.0.1:{port}");
        let log = std::fs::File::create(dir.path().join("tigerbeetle.log")).unwrap();
        let child = Command::new(&bin)
            .env("TMPDIR", dir.path())
            .args(["start", &format!("--addresses={address}"), "--development", data.to_str().unwrap()])
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .map_err(|e| format!("{}: {e}", bin.display()))?;
        note_server(&dir, child.id());
        let tb = TigerBeetle { child, address, dir };
        let started = Instant::now();
        while TcpStream::connect(&tb.address).is_err() {
            if started.elapsed() > Duration::from_secs(60) {
                return Err(format!("TigerBeetle did not take connections within a minute:\n{}", tb.log()));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(tb)
    }

    pub fn connection(&self) -> serde_json::Value {
        serde_json::json!({"cluster": "0", "addresses": [self.address]})
    }

    pub fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("tigerbeetle.log")).unwrap_or_default()
    }
}

impl Drop for TigerBeetle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
