//! A process rulec starts and does not simply run to its end, stopped and waited for however
//! the code that started it returns (§15.163).
//!
//! `std::process::Child` does nothing when it is dropped. A process left behind by an early
//! return, a `?` or a `return Err(…)`, goes on running after rulec has exited, with nobody to
//! wait for it. `rulec test` stood the generated MCP server up on a port and returned on the
//! first answer that disagreed with the reference evaluator, and every such run left a server
//! listening. So every process rulec starts and talks to on the way — the generated MCP server,
//! an adapter, an extractor, the rulec a call of `rulec mcp` runs — is an [`Owned`]. A command
//! that is only run to its end (`output`, `status`) needs none: those wait for it already.

use std::ops::{Deref, DerefMut};
use std::process::{Child, Command};

/// A child process that is killed and waited for when its value is dropped, unless it has
/// ended by then. It is the `Child` otherwise: its pipes, `wait`, `try_wait`, `id`.
pub struct Owned {
    child: Child,
    /// Started in a process group of its own, which is stopped with it.
    group: bool,
}

impl Owned {
    /// Start `cmd`.
    pub fn spawn(cmd: &mut Command) -> std::io::Result<Owned> {
        Ok(Owned { child: cmd.spawn()?, group: false })
    }

    /// Start `cmd` in a process group of its own, so that what it starts in turn — an adapter
    /// `verify` stood up, a compiler `test` ran — is stopped with it.
    pub fn spawn_group(cmd: &mut Command) -> std::io::Result<Owned> {
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(cmd, 0);
        Ok(Owned { child: cmd.spawn()?, group: cfg!(unix) })
    }

    /// Stop it now, with its group when it has one of its own, and wait for it.
    pub fn stop(&mut self) {
        if self.group {
            // std has no call that signals a group, and rulec depends on nothing that has one.
            #[cfg(unix)]
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", self.child.id())])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Deref for Owned {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.child
    }
}

impl DerefMut for Owned {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // One that has ended is waited for by `try_wait` itself, and leaves nothing to stop.
        if let Ok(Some(_)) = self.child.try_wait() {
            return;
        }
        self.stop();
    }
}
