//! Running claims side by side (DESIGN §10). Up to N workers take claims in spec
//! order; a claim waits while a fixed port of a service it starts is in use by
//! another claim, or while another claim on a `serial` target it touches runs, and
//! a worker takes the first claim that does not have to wait. Whatever finishes
//! first, the results come back in claim order, so the report, the journal and the
//! baseline are written as one worker writes them.

use crate::model::{Port, Spec, Step, TargetKind};
use std::collections::BTreeSet;
use std::sync::{Condvar, Mutex};
use std::thread;

/// How many claims run at once: `--jobs`, then `GEAS_JOBS`, then 1. A `GEAS_JOBS`
/// that is not a whole number from 1 counts as unset, as `GEAS_LANG` does: a
/// variable left in an environment should not stop every command.
pub fn jobs(flag: Option<usize>) -> usize {
    flag.or_else(|| std::env::var("GEAS_JOBS").ok()?.trim().parse().ok().filter(|n| *n >= 1)).unwrap_or(1)
}

/// What a claim holds while it runs: the fixed ports of the services it starts,
/// and its `serial` targets.
#[derive(Default)]
struct Holds {
    ports: BTreeSet<u16>,
    serial: BTreeSet<String>,
}

fn holds(spec: &Spec, claim: usize) -> Holds {
    let mut h = Holds::default();
    for step in &spec.claims[claim].steps {
        let Step::When { target, .. } = step else {
            continue;
        };
        let Some(tg) = spec.target(target) else {
            continue;
        };
        if let TargetKind::Serve { port: Port::Fixed(p), .. } = tg.kind {
            h.ports.insert(p);
        }
        if tg.serial {
            h.serial.insert(tg.name.clone());
        }
    }
    h
}

struct State {
    started: Vec<bool>,
    ports: BTreeSet<u16>,
    serial: BTreeSet<String>,
}

impl State {
    fn free(&self, h: &Holds) -> bool {
        h.ports.is_disjoint(&self.ports) && h.serial.is_disjoint(&self.serial)
    }
}

/// Gives back what a claim held when its run ends, also when it panics, so that no
/// other worker waits for it for ever.
struct Release<'a> {
    state: &'a Mutex<State>,
    turn: &'a Condvar,
    holds: &'a Holds,
}

impl Drop for Release<'_> {
    fn drop(&mut self) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        for p in &self.holds.ports {
            st.ports.remove(p);
        }
        for s in &self.holds.serial {
            st.serial.remove(s);
        }
        self.turn.notify_all();
    }
}

/// Runs `run(worker, i)` for every claim `i`, up to `jobs` at once, and returns the
/// results in claim order. `worker` (from 0) says which of the workers runs it, so
/// that what a worker keeps for its claims (a Chrome) is used by one claim at a
/// time. With one job, one claim after another on this thread, as v0 ran.
pub fn each<T: Send>(spec: &Spec, jobs: usize, run: impl Fn(usize, usize) -> T + Sync) -> Vec<T> {
    let n = spec.claims.len();
    if jobs <= 1 || n <= 1 {
        return (0..n).map(|i| run(0, i)).collect();
    }
    let holds: Vec<Holds> = (0..n).map(|i| holds(spec, i)).collect();
    let state = Mutex::new(State { started: vec![false; n], ports: BTreeSet::new(), serial: BTreeSet::new() });
    let turn = Condvar::new();
    let results: Vec<Mutex<Option<T>>> = (0..n).map(|_| Mutex::new(None)).collect();
    thread::scope(|scope| {
        for worker in 0..jobs.min(n) {
            let (state, turn, holds, results, run) = (&state, &turn, &holds, &results, &run);
            scope.spawn(move || {
                loop {
                    let i = {
                        let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
                        loop {
                            if st.started.iter().all(|s| *s) {
                                return;
                            }
                            if let Some(i) = (0..n).find(|&i| !st.started[i] && st.free(&holds[i])) {
                                st.started[i] = true;
                                st.ports.extend(holds[i].ports.iter().copied());
                                st.serial.extend(holds[i].serial.iter().cloned());
                                break i;
                            }
                            st = turn.wait(st).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    let _release = Release { state, turn, holds: &holds[i] };
                    let r = run(worker, i);
                    *results[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                }
            });
        }
    });
    results
        .into_iter()
        .map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()).expect("every claim ran"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn spec(src: &str) -> Spec {
        parse(src).unwrap_or_else(|d| panic!("{d:?}"))
    }

    /// Runs every claim for 30 ms, and returns the most that ran at once.
    fn most_at_once(s: &Spec, jobs: usize) -> usize {
        let now = AtomicUsize::new(0);
        let most = AtomicUsize::new(0);
        let order = each(s, jobs, |_, i| {
            let k = now.fetch_add(1, Ordering::SeqCst) + 1;
            most.fetch_max(k, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
            now.fetch_sub(1, Ordering::SeqCst);
            i
        });
        assert_eq!(order, (0..s.claims.len()).collect::<Vec<_>>(), "results in claim order");
        most.load(Ordering::SeqCst)
    }

    fn claims(target: &str, n: usize) -> String {
        (0..n).map(|i| format!("claim \"c{i}\" {{\n  when {target}.get(\"/\")\n}}\n")).collect()
    }

    #[test]
    fn claims_run_side_by_side_up_to_the_jobs() {
        let s = spec(&format!("target a {{\n  serve \"x {{port}}\"\n  port auto\n}}\n{}", claims("a", 6)));
        assert_eq!(most_at_once(&s, 3), 3);
        assert_eq!(most_at_once(&s, 1), 1);
    }

    #[test]
    fn a_fixed_port_and_a_serial_target_run_one_claim_at_a_time() {
        let fixed = spec(&format!("target a {{\n  serve \"x\"\n  port 8123\n}}\n{}", claims("a", 4)));
        assert_eq!(most_at_once(&fixed, 4), 1);
        let serial = spec(&format!("target a {{\n  serve \"x {{port}}\"\n  port auto\n  serial\n}}\n{}", claims("a", 4)));
        assert_eq!(most_at_once(&serial, 4), 1);
        // the others run beside them
        let mixed = spec(&format!(
            "target a {{\n  serve \"x\"\n  port 8123\n}}\ntarget b {{\n  serve \"y {{port}}\"\n  port auto\n}}\n{}{}",
            claims("a", 2),
            claims("b", 2).replace("\"c", "\"d")
        ));
        assert_eq!(most_at_once(&mixed, 4), 3);
    }
}
