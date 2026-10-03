//! One TigerBeetle replica on a data file of its own (chobo's), stopped when its value is
//! dropped. The program is `RITSU_TIGERBEETLE` or the crate's own `<CRATE>_TIGERBEETLE`, else
//! `tools/tigerbeetle/tigerbeetle` in the crate (`tools/tigerbeetle/fetch.sh` brings it).

use crate::tmp::TempDir;
use crate::tools;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub struct TigerBeetle {
    child: Child,
    pub address: String,
    dir: TempDir,
}

/// The program, or why there is none (for a SKIP line).
pub fn bin() -> Result<PathBuf, String> {
    let bin = tools::from_vars("TIGERBEETLE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("tools/tigerbeetle/tigerbeetle"));
    if bin.is_file() { Ok(bin) } else { Err("TigerBeetle is not here: run tools/tigerbeetle/fetch.sh, or set RITSU_TIGERBEETLE".into()) }
}

impl TigerBeetle {
    /// A replica, or why there is none (for a SKIP line).
    pub fn start() -> Result<TigerBeetle, String> {
        let bin = bin()?;
        let dir = TempDir::new("tigerbeetle");
        let data = dir.path().join("0_0.tigerbeetle");
        // TigerBeetle copies itself into the temporary directory to run (256 MB a time), and a
        // killed replica leaves the copy there: it gets the test's own, which goes with the test.
        let out = Command::new(&bin)
            .env("TMPDIR", dir.path())
            .args(["format", "--cluster=0", "--replica=0", "--replica-count=1", "--development", &data.to_string_lossy()])
            .output()
            .map_err(|e| format!("{}: {e}", bin.display()))?;
        if !out.status.success() {
            return Err(format!("tigerbeetle format failed: {}", String::from_utf8_lossy(&out.stderr)));
        }
        let port = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).map_err(|e| e.to_string())?;
        let address = format!("127.0.0.1:{port}");
        let log = std::fs::File::create(dir.path().join("tigerbeetle.log")).map_err(|e| e.to_string())?;
        let child = Command::new(&bin)
            .env("TMPDIR", dir.path())
            .args(["start", &format!("--addresses={address}"), "--development", &data.to_string_lossy()])
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            .spawn()
            .map_err(|e| format!("{}: {e}", bin.display()))?;
        dir.note_server(child.id(), "tigerbeetle");
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

    /// The replica's log, for a failure to show.
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
