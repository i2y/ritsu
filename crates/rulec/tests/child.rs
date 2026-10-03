//! `src/child.rs` (§15.163): a process rulec starts and talks to is stopped and waited for
//! however the code that started it returns. `std::process::Child` leaves it running.

use rulec::child::Owned;
use std::io::BufRead;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Whether the process `pid` is there. `kill -0` also finds one that has ended and has not
/// been waited for, so "not there" means stopped and waited for.
fn there(pid: u32) -> bool {
    Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Wait up to five seconds for `pid` to go: a process whose parent was stopped is waited for by
/// the system, a moment later.
fn gone(pid: u32) -> bool {
    let start = Instant::now();
    while there(pid) {
        if start.elapsed() > Duration::from_secs(5) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

/// What the code in rulec does: start a process, then return on the first thing that goes wrong.
fn start_then_fail(cmd: &mut Command, group: bool, seen: &mut Vec<u32>) -> Result<(), String> {
    let mut child = if group { Owned::spawn_group(cmd) } else { Owned::spawn(cmd) }.map_err(|e| e.to_string())?;
    seen.push(child.id());
    // A process that starts others says their numbers on its first line.
    if let Some(out) = child.stdout.take() {
        let mut line = String::new();
        std::io::BufReader::new(out).read_line(&mut line).map_err(|e| e.to_string())?;
        seen.extend(line.split_whitespace().filter_map(|p| p.parse::<u32>().ok()));
    }
    // Returned without a word about the child, as a `return Err(…)` in rulec does.
    Err("途中で戻る".to_string())
}

/// Stop what a failed assertion would leave running.
fn stop(pids: &[u32]) {
    for p in pids {
        let _ = Command::new("kill").args(["-9", &p.to_string()]).stderr(Stdio::null()).status();
    }
}

#[test]
fn 途中で戻っても子を止めて待つ() {
    let mut seen = Vec::new();
    let started = Instant::now();
    let r = start_then_fail(Command::new("sleep").arg("30"), false, &mut seen);
    assert!(r.is_err());
    let left: Vec<u32> = seen.iter().copied().filter(|p| there(*p)).collect();
    stop(&left);
    assert!(left.is_empty(), "途中で戻ったのに sleep が残っている: {left:?}");
    assert!(started.elapsed() < Duration::from_secs(10), "止めずに、終わるのを待った");
}

#[test]
fn 自分のグループで立てた子は_それが立てたものごと止める() {
    // `rulec mcp` starts rulec this way, and that rulec starts compilers and adapters of its own.
    let mut seen = Vec::new();
    let r = start_then_fail(Command::new("sh").args(["-c", "sleep 30 & echo $!; wait"]).stdout(Stdio::piped()), true, &mut seen);
    assert!(r.is_err());
    assert_eq!(seen.len(), 2, "sh が立てた sleep の番号が読めない: {seen:?}");
    let left: Vec<u32> = seen.iter().copied().filter(|p| !gone(*p)).collect();
    stop(&left);
    assert!(left.is_empty(), "sh か、sh が立てた sleep が残っている: {left:?}");
}
