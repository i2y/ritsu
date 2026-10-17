//! A database of the MySQL family for a test (chobo's): a throwaway MySQL server made for the test
//! (`mysqld --initialize-insecure` into a [`TempDir`], on 127.0.0.1 and a port no one listens on,
//! stopped and removed when its value is dropped); a throwaway Vitess, vttestserver with one
//! keyspace (vtcombo and one MySQL server under it), which makes no database, so that a test uses
//! the keyspace; or a server of the machine's that a variable names, such as a TiDB cluster
//! (`RITSU_TIDB`), where a test makes databases of its own and drops them when its value is
//! dropped.
//!
//! MySQL's programs are under `RITSU_MYSQL_BIN` or the crate's own `<CRATE>_MYSQL_BIN` (the `bin`
//! directory of a MySQL build, which the server's `--basedir` is the parent of), else under the
//! crate's `tools/mysql/mysql/bin` (chobo's `tools/mysql/fetch.sh` puts MySQL 8.4 there), else where
//! `mysqld` is on the PATH. The client that talks to a server named by a variable is `mysql` of
//! that directory, or of the PATH. Vitess's are under `RITSU_VITESS_BIN`, the `bin` directory of a
//! Vitess build (vttestserver, vtcombo and mysqlctl), whose parent holds Vitess's `config`.

use crate::tmp::TempDir;
use crate::tools;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct MySql {
    /// What it is, for a line of the test's output: `MySQL 8.4.11`, `TiDB v8.5.8`.
    pub label: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    client: PathBuf,
    server: Option<(Child, TempDir)>,
    /// The databases made on a server of the machine's, dropped with the value.
    made: Mutex<Vec<String>>,
    /// On Vitess, the keyspace, which stands for every database the test asks for, and how many
    /// shards it has.
    keyspace: Option<(String, u32)>,
    /// On Vitess, the short name under /tmp of the directory its MySQL server keeps its data and
    /// its socket in, removed with the value.
    link: Option<PathBuf>,
}

/// Where MySQL's programs are, or why there are none (for a SKIP line).
pub fn bin() -> Result<PathBuf, String> {
    let fetched = PathBuf::from("tools/mysql/mysql/bin");
    let bin = match tools::from_vars("MYSQL_BIN") {
        Some(b) => PathBuf::from(b),
        None if fetched.join("mysqld").is_file() => std::fs::canonicalize(&fetched).unwrap_or(fetched),
        None => tools::on_path("mysqld").and_then(|p| p.parent().map(Path::to_path_buf)).ok_or("MySQL is not here: run tools/mysql/fetch.sh, put mysqld and mysql on the PATH, or set RITSU_MYSQL_BIN to the bin directory of a MySQL build")?,
    };
    for tool in ["mysqld", "mysql"] {
        if !bin.join(tool).is_file() {
            return Err(format!("{tool} is not in {}; set RITSU_MYSQL_BIN to the bin directory of a MySQL build", bin.display()));
        }
    }
    Ok(bin)
}

/// The client `mysql`: of `RITSU_MYSQL_BIN`, of the crate's `tools/mysql/mysql/bin`, or of the PATH.
fn client() -> Result<PathBuf, String> {
    if let Ok(b) = bin() {
        let c = b.join("mysql");
        if c.is_file() {
            return Ok(c);
        }
    }
    tools::on_path("mysql").ok_or_else(|| "the client mysql is not here: set RITSU_MYSQL_BIN to the bin directory of a MySQL build, or put mysql on the PATH".to_string())
}

impl MySql {
    /// A throwaway MySQL server, or why there is none (for a SKIP line).
    pub fn start() -> Result<MySql, String> {
        let bin = bin()?;
        let base = bin.parent().ok_or("RITSU_MYSQL_BIN has no parent directory")?.to_path_buf();
        let dir = TempDir::new("mysql");
        let data = dir.path().join("data");
        let r = crate::run(
            Command::new(bin.join("mysqld")).args([
                "--no-defaults",
                &format!("--basedir={}", base.display()),
                &format!("--datadir={}", data.display()),
                "--initialize-insecure",
            ]),
            Duration::from_secs(180),
        );
        if !r.ok {
            return Err(format!("mysqld --initialize-insecure failed: {}{}", r.stdout, r.stderr));
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).map_err(|e| e.to_string())?;
        let child = Command::new(bin.join("mysqld"))
            .args([
                "--no-defaults",
                &format!("--basedir={}", base.display()),
                &format!("--datadir={}", data.display()),
                &format!("--port={port}"),
                "--bind-address=127.0.0.1",
                // MySQL keeps the path of a socket to 103 bytes, which the system's temporary
                // directory can pass: a bare name is in the directory the server works in, its
                // data directory
                "--socket=mysql.sock",
                "--mysqlx=OFF",
                "--skip-log-bin",
                "--performance-schema=OFF",
                "--innodb-buffer-pool-size=128M",
                "--innodb-flush-log-at-trx-commit=2",
                "--innodb-doublewrite=OFF",
                "--max-connections=1500",
                &format!("--log-error={}", dir.path().join("mysqld.log").display()),
                &format!("--pid-file={}", dir.path().join("mysqld.pid").display()),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{}: {e}", bin.join("mysqld").display()))?;
        dir.note_server(child.id(), "mysqld");
        let version = tools::version(bin.join("mysqld"), "--version");
        let label = version.split_whitespace().skip_while(|w| *w != "Ver").nth(1).map(|v| format!("MySQL {v}")).unwrap_or_else(|| "MySQL".into());
        let my = MySql { label, host: "127.0.0.1".into(), port, user: "root".into(), client: bin.join("mysql"), server: Some((child, dir)), made: Mutex::new(Vec::new()), keyspace: None, link: None };
        let started = Instant::now();
        while my.sql("", "select 1").is_err() {
            if started.elapsed() > Duration::from_secs(120) {
                return Err(format!("MySQL did not take connections within two minutes:\n{}", my.log()));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Ok(my)
    }

    /// A server of the machine's that `RITSU_<var>` names as `host:port` (user root), or why
    /// there is none. `what` names it for the label, with the version the server gives.
    pub fn at(var: &str, what: &str) -> Result<MySql, String> {
        let addr = tools::from_vars(var).ok_or_else(|| format!("{what} is not here: set RITSU_{var} to the host:port of a running server"))?;
        let (host, port) = addr.rsplit_once(':').ok_or_else(|| format!("RITSU_{var}={addr} is not host:port"))?;
        let port: u16 = port.parse().map_err(|_| format!("RITSU_{var}={addr} is not host:port"))?;
        let mut my = MySql { label: what.into(), host: host.into(), port, user: "root".into(), client: client()?, server: None, made: Mutex::new(Vec::new()), keyspace: None, link: None };
        let v = my.sql("", "select version()").map_err(|e| format!("{what} at {addr} does not answer: {e}"))?;
        let v = v.trim();
        my.label = match v.split_once("-TiDB-") {
            Some((_, t)) => format!("{what} {}", t.split('-').next().unwrap_or(t)),
            None => format!("{what} {v}"),
        };
        Ok(my)
    }

    /// A throwaway Vitess, or why there is none (for a SKIP line): vttestserver from
    /// `RITSU_VITESS_BIN`, with vtcombo and one MySQL server of `RITSU_MYSQL_BIN` under it, and one
    /// keyspace, `chobo`, of `shards` shards. A transaction may touch one shard only
    /// (`--transaction-mode SINGLE`), so that one which crosses shards fails. A keyspace of more
    /// than one shard starts with the vindex `xxhash` and no tables: [`MySql::shard_by`] gives each
    /// table the column it is sharded by. Vitess makes no database: [`MySql::database`] answers the
    /// keyspace.
    pub fn vitess(shards: u32) -> Result<MySql, String> {
        let vt = tools::from_vars("VITESS_BIN").map(PathBuf::from).ok_or("Vitess is not here: build vttestserver, vtcombo and mysqlctl from Vitess's source, and set RITSU_VITESS_BIN to the bin directory they are in")?;
        for tool in ["vttestserver", "vtcombo", "mysqlctl"] {
            if !vt.join(tool).is_file() {
                return Err(format!("{tool} is not in {}; RITSU_VITESS_BIN names the bin directory of a Vitess build", vt.display()));
            }
        }
        let root = vt.parent().filter(|r| r.join("config/mycnf").is_dir()).ok_or_else(|| format!("{} has no config/mycnf beside it; RITSU_VITESS_BIN names the bin directory of Vitess's source tree", vt.display()))?.to_path_buf();
        let mysql_bin = bin()?;
        let mysql_root = mysql_bin.parent().ok_or("RITSU_MYSQL_BIN has no parent directory")?.to_path_buf();
        let dir = TempDir::new("vitess");
        let schema = dir.path().join("schema/chobo");
        std::fs::create_dir_all(&schema).map_err(|e| e.to_string())?;
        // MySQL keeps the path of a socket to 103 bytes, and the socket is in the data directory:
        // the server is given a short name of it under /tmp, which goes with the value, or with
        // the first Vitess of a test process after this one if this one is killed
        sweep_links();
        static N: AtomicUsize = AtomicUsize::new(0);
        let data = dir.path().join("data");
        std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
        let link = PathBuf::from(format!("{LINK}{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&data, &link).map_err(|e| format!("{}: {e}", link.display()))?;
        if shards > 1 {
            std::fs::write(schema.join("vschema.json"), "{\"sharded\": true, \"vindexes\": {\"xxhash\": {\"type\": \"xxhash\"}}, \"tables\": {}}\n").map_err(|e| e.to_string())?;
        }
        // one at a time in this process, from choosing the ports until it takes connections: two
        // that chose at once could choose the same ones
        static STARTING: Mutex<()> = Mutex::new(());
        let _turn = STARTING.lock().unwrap_or_else(|e| e.into_inner());
        // vtcombo takes the base port and the one after it, its MySQL server the next, and vtgate's
        // MySQL listener the one after that
        let base = free_ports(4)?;
        let log = std::fs::File::create(dir.path().join("vttestserver.log")).map_err(|e| e.to_string())?;
        let err = log.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(vt.join("vttestserver"));
        cmd.args([
            "--port",
            &base.to_string(),
            "--mysql-bind-host",
            "127.0.0.1",
            "--vtcombo-bind-host",
            "127.0.0.1",
            "--tablet-hostname",
            "127.0.0.1",
            "--keyspaces",
            "chobo",
            "--num-shards",
            &shards.to_string(),
            "--replica-count",
            "1",
            "--rdonly-count",
            "0",
            "--transaction-mode",
            "SINGLE",
            "--vschema-ddl-authorized-users",
            "%",
        ])
        .arg("--schema-dir")
        .arg(dir.path().join("schema"))
        .arg("--data-dir")
        .arg(&link)
        .env("VTROOT", &root)
        .env("VTDATAROOT", &link)
        .env("VT_MYSQL_ROOT", &mysql_root)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(err);
        // a process group of its own, so that what it starts under it stops with it
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        let mut child = cmd.spawn().map_err(|e| {
            let _ = std::fs::remove_file(&link);
            format!("{}: {e}", vt.join("vttestserver").display())
        })?;
        // if this process is killed, the first test process after it kills the group: vttestserver
        // and what it starts under it, vtcombo, mysqlctl and the MySQL server
        dir.note_group(child.id(), &["vttestserver", "vtcombo", "mysqld"]);
        let version = tools::version(vt.join("vtcombo"), "--version");
        let v = version.split_whitespace().skip_while(|w| *w != "Version:").nth(1).unwrap_or("").to_string();
        let label = if shards > 1 { format!("Vitess {v}, {shards} shards") } else { format!("Vitess {v}, unsharded") };
        let mut my = MySql { label, host: "127.0.0.1".into(), port: base + 3, user: "root".into(), client: mysql_bin.join("mysql"), server: None, made: Mutex::new(Vec::new()), keyspace: Some(("chobo".into(), shards)), link: Some(link.clone()) };
        let started = Instant::now();
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                // what it started before it stopped, its MySQL server, goes with it
                crate::tmp::kill_group(child.id());
                let _ = std::fs::remove_file(&link);
                let text = std::fs::read_to_string(dir.path().join("vttestserver.log")).unwrap_or_default();
                return Err(format!("vttestserver stopped ({status}) before it took connections:\n{}", errors(&text)));
            }
            if my.sql("chobo", "select 1").is_ok() {
                break;
            }
            if started.elapsed() > Duration::from_secs(300) {
                crate::tmp::kill_group(child.id());
                let _ = child.wait();
                let _ = std::fs::remove_file(&link);
                let text = std::fs::read_to_string(dir.path().join("vttestserver.log")).unwrap_or_default();
                return Err(format!("Vitess did not take connections within five minutes:\n{}", errors(&text)));
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        my.server = Some((child, dir));
        Ok(my)
    }

    /// On Vitess with more than one shard, shard `table` of the keyspace by `column`, with the
    /// vindex `xxhash` (the VSchema DDL of vtgate); elsewhere nothing.
    pub fn shard_by(&self, table: &str, column: &str) -> Result<(), String> {
        match &self.keyspace {
            Some((ks, shards)) if *shards > 1 => {
                let quote = |n: &str| format!("`{}`", n.replace('`', "``"));
                self.sql(ks, &format!("alter vschema on {}.{} add vindex `xxhash`({})", quote(ks), quote(table), quote(column))).map(|_| ())
            }
            _ => Ok(()),
        }
    }

    /// Whether this is Vitess.
    pub fn is_vitess(&self) -> bool {
        self.keyspace.is_some()
    }

    /// The client on a database of the server (none when `db` is empty), stopping at the first
    /// error, with `/*T! … */` and the like sent as written.
    pub fn mysql(&self, db: &str) -> Command {
        let mut c = Command::new(&self.client);
        c.args(["--no-defaults", "--comments", "--batch", "--raw", "--skip-column-names", "-h", &self.host, "-P", &self.port.to_string(), "-u", &self.user]);
        if !db.is_empty() {
            c.arg(db);
        }
        c
    }

    /// Run SQL on a database of the server and answer what it prints, a row a line, its columns
    /// separated by tabs.
    pub fn sql(&self, db: &str, sql: &str) -> Result<String, String> {
        let out = self.mysql(db).args(["-e", sql]).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
        if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).to_string()) } else { Err(String::from_utf8_lossy(&out.stderr).to_string()) }
    }

    /// A new database of the server, named after the test process: on a server of the
    /// machine's it is dropped with the value. On Vitess, the keyspace.
    pub fn database(&self, what: &str) -> Result<String, String> {
        static N: AtomicUsize = AtomicUsize::new(0);
        if let Some((ks, _)) = &self.keyspace {
            return Ok(ks.clone());
        }
        let what: String = what.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
        let name = format!("ritsu_{}_{}_{}_{what}", crate::crate_var_prefix().to_ascii_lowercase(), std::process::id(), N.fetch_add(1, Ordering::SeqCst));
        self.sql("", &format!("create database `{name}`"))?;
        if self.server.is_none() {
            self.made.lock().unwrap().push(name.clone());
        }
        Ok(name)
    }

    /// The server's log, for a failure to show (none for a server of the machine's).
    pub fn log(&self) -> String {
        match &self.server {
            Some((_, dir)) if self.keyspace.is_some() => std::fs::read_to_string(dir.path().join("vttestserver.log")).unwrap_or_default(),
            Some((_, dir)) => std::fs::read_to_string(dir.path().join("mysqld.log")).unwrap_or_default(),
            None => String::new(),
        }
    }
}

/// A port with the `n - 1` after it free, on 127.0.0.1.
fn free_ports(n: u16) -> Result<u16, String> {
    for _ in 0..50 {
        let first = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).map_err(|e| e.to_string())?;
        if first > u16::MAX - n {
            continue;
        }
        if (1..n).all(|i| std::net::TcpListener::bind(("127.0.0.1", first + i)).is_ok()) {
            return Ok(first);
        }
    }
    Err(format!("no {n} ports in a row are free on 127.0.0.1"))
}

/// The start of the short name under /tmp of a Vitess's data, before `<pid>-<n>`.
const LINK: &str = "/tmp/ritsu-vt-";

/// Remove the short names under /tmp of the data of test processes that have ended.
fn sweep_links() {
    let Ok(rd) = std::fs::read_dir("/tmp") else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(pid) = name.strip_prefix(&LINK["/tmp/".len()..]).and_then(|p| p.split('-').next()).and_then(|p| p.parse::<u32>().ok()) else { continue };
        if pid != std::process::id() && !crate::tmp::alive(pid) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The lines of a log of Vitess that say what went wrong, the last 20: its errors, not the list of
/// flags it prints when it stops.
fn errors(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| l.contains("\"level\":\"ERROR\"") || l.contains("\"level\":\"FATAL\"") || l.starts_with("Error:")).collect();
    lines[lines.len().saturating_sub(20)..].join("\n")
}

impl Drop for MySql {
    fn drop(&mut self) {
        for db in self.made.lock().unwrap().drain(..) {
            let _ = self.mysql("").args(["-e", &format!("drop database if exists `{db}`")]).stdin(Stdio::null()).output();
        }
        let vitess = self.keyspace.is_some();
        if let Some((child, _)) = &mut self.server {
            if vitess {
                // vttestserver stops vtcombo and its MySQL server when it is asked to; what is left
                // of its process group after half a minute is killed
                let _ = Command::new("kill").args(["-TERM", &child.id().to_string()]).status();
                let started = Instant::now();
                while child.try_wait().ok().flatten().is_none() && started.elapsed() < Duration::from_secs(30) {
                    std::thread::sleep(Duration::from_millis(200));
                }
                crate::tmp::kill_group(child.id());
            } else {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        if let Some(link) = &self.link {
            let _ = std::fs::remove_file(link);
        }
    }
}
