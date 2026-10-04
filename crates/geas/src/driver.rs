//! The driver protocol (DESIGN §8.5): a GUI geas does not drive itself is reached
//! through a program of the spec's choosing, `driver "<command>"`, that speaks JSON
//! lines on its stdin and stdout. geas starts it once per claim, in the spec's
//! directory with the target's pins, hands it the pins, then one action a line as
//! the claim reaches them, each answered by the screen; last `{"do":"close"}`. An
//! answer that does not come within 5 s, or is not of that shape, is E037.

use crate::diag;
use ritsu_base::text::Text;
use crate::gui;
use crate::json::{self, J};
use crate::model::{Call, Place, Target};
use crate::proc::{self, Env, Failure, Launch, Proc};
use crate::screen::{self, Node};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long geas waits for each answer.
pub const ANSWER: Duration = Duration::from_secs(5);

/// A driver running for one claim.
pub struct Live {
    proc: Proc,
    lines: Receiver<String>,
    target: String,
}

/// What a message calls the line or the action an answer was owed for.
struct Owed {
    en: String,
    ja: String,
}

fn owed_for(call: Option<&Call>) -> Owed {
    match call {
        // in Japanese, code is set off from the particle after it by a space
        Some(c) => Owed { en: format!("`{}`", c.display()), ja: format!("`{}` ", c.display()) },
        None => Owed { en: "the first line, which hands it the pins".into(), ja: "固定を渡す最初の行".into() },
    }
}

/// The line that asks a driver for an action.
pub fn action_line(call: &Call) -> String {
    use json::quote;
    let place = |p: &Place| match p {
        Place::First => ",\"field\":1".to_string(),
        Place::Field(n) => format!(",\"field\":{n}"),
        Place::Into { name, nth } => format!(",\"into\":{},\"nth\":{}", quote(name), nth.unwrap_or(1)),
    };
    match call {
        Call::Open(None) => "{\"do\":\"open\"}".into(),
        Call::Open(Some(p)) => format!("{{\"do\":\"open\",\"path\":{}}}", quote(p)),
        Call::Click { name, nth } => format!("{{\"do\":\"click\",\"name\":{},\"nth\":{}}}", quote(name), nth.unwrap_or(1)),
        Call::Input { text, place: p } => format!("{{\"do\":\"input\",\"text\":{}{}}}", quote(text), place(p)),
        Call::Submit(p) => format!("{{\"do\":\"submit\"{}}}", place(p)),
        Call::Press(k) => format!("{{\"do\":\"press\",\"key\":{}}}", quote(k)),
        Call::Advance(ms) => format!("{{\"do\":\"advance\",\"ms\":{ms}}}"),
        Call::Run(_) | Call::Get(_) | Call::Post { .. } => unreachable!("E006 refuses these on a driver"),
    }
}

/// The screen a driver sent: its node is the screen, whose children are what the
/// app shows; a node without a role hands its children up (DESIGN §8.1).
pub fn screen_of(j: &J) -> Result<Node, String> {
    let root = Node::from_json(j)?;
    Ok(screen::screen(screen::normalize(vec![root])))
}

fn member<'a>(j: &'a J, key: &str) -> Option<&'a J> {
    match j {
        J::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

impl Live {
    /// Starts the driver of `tg` and hands it the pins; E011 when it cannot keep
    /// them, E030 when it does not start, E037 when its answer does not read.
    pub fn start(tg: &Target, words: &[String], dir: &Path) -> Result<Live, Failure> {
        let env = Env::of(&tg.pins, None);
        let launch = Launch { dir, env: &env, term: false };
        let mut p = proc::start(&tg.name, words, &launch, true, true)?;
        let stdout = p.stdout.take().expect("piped");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut live = Live { proc: p, lines: rx, target: tg.name.clone() };
        let pins = tg.pins.json();
        let answer = live.ask(&format!("{{\"geas\":1,\"pins\":{pins}}}"), None)?;
        if let Some(J::Str(why)) = member(&answer, "error") {
            let name = &tg.name;
            return Err(Failure {
                code: "E011",
                msg: tr!(
                    "`{name}` のドライバーは、渡した固定を受け付けませんでした: {why}",
                    "the driver of `{name}` cannot keep the pins it was handed: {why}",
                ),
                notes: vec![tr!("渡した固定: {pins}", "the pins: {pins}")],
            });
        }
        if !matches!(member(&answer, "ok"), Some(J::Bool(true))) {
            let line = json::render(&answer);
            return Err(live.unreadable(&line, None));
        }
        Ok(live)
    }

    /// Sends a line and reads the answer, as JSON.
    fn ask(&mut self, line: &str, call: Option<&Call>) -> Result<J, Failure> {
        let sent = match self.proc.stdin.as_mut() {
            Some(w) => w.write_all(format!("{line}\n").as_bytes()).and_then(|_| w.flush()).is_ok(),
            None => false,
        };
        let got = if sent { self.lines.recv_timeout(ANSWER) } else { Err(RecvTimeoutError::Disconnected) };
        match got {
            Ok(text) => json::parse(&text).map_err(|_| self.unreadable(&text, call)),
            Err(RecvTimeoutError::Timeout) => {
                let o = owed_for(call);
                let name = &self.target;
                Err(Failure {
                    code: "E037",
                    msg: tr!(
                        "`{name}` のドライバーから、{}への応答が 5 秒のうちにありませんでした",
                        "the driver of `{name}` did not answer {} within 5 s",
                        o.ja;
                        o.en,
                    ),
                    notes: proc::stderr_tail(&self.proc.stderr_so_far()),
                })
            }
            Err(RecvTimeoutError::Disconnected) => {
                let o = owed_for(call);
                let name = &self.target;
                let how = match self.proc.wait_until(Instant::now() + Duration::from_secs(1)) {
                    Some(st) => proc::how_it_ended(st),
                    None => tr!("標準出力が閉じました", "its stdout closed"),
                };
                Err(Failure {
                    code: "E037",
                    msg: tr!(
                        "`{name}` のドライバーが、{}に応答する前に終了しました（{}）",
                        "the driver of `{name}` exited before answering {} ({})",
                        o.ja,
                        how.ja;
                        o.en,
                        how.en,
                    ),
                    notes: proc::stderr_tail(&self.proc.stderr_so_far()),
                })
            }
        }
    }

    /// E037 for an answer geas cannot read.
    fn unreadable(&self, line: &str, call: Option<&Call>) -> Failure {
        let o = owed_for(call);
        let name = &self.target;
        let l = diag::cut(line, 80);
        let shape: Text = if call.is_some() {
            tr!(
                "ドライバーは、操作ごとに JSON を一行返す必要があります。`{{\"screen\":…}}` か、アプリが操作を拒否したときは `{{\"error\":\"…\",\"screen\":…}}` です",
                "a driver answers each action with one line of JSON: `{{\"screen\":…}}`, or `{{\"error\":\"…\",\"screen\":…}}` when the app refused it",
            )
        } else {
            tr!(
                "ドライバーは、最初の行に `{{\"ok\":true}}` を返す必要があります。守れない固定があれば、`{{\"error\":\"…\"}}` を返してください",
                "a driver answers the first line with `{{\"ok\":true}}`, or with `{{\"error\":\"…\"}}` for a pin it cannot keep",
            )
        };
        Failure {
            code: "E037",
            msg: tr!(
                "`{name}` のドライバーは、{}に geas の読めない行を返しました: `{l}`",
                "the driver of `{name}` answered {} with a line geas cannot read: `{l}`",
                o.ja;
                o.en,
            ),
            notes: vec![shape],
        }
    }

    /// One action, and the screen after it; E035 when the app refused it, E037 when
    /// the answer does not come or does not read.
    pub fn act(&mut self, call: &Call) -> Result<Node, Failure> {
        let answer = self.ask(&action_line(call), Some(call))?;
        let screen = match member(&answer, "screen") {
            Some(s) => Some(screen_of(s).map_err(|_| self.unreadable(&json::render(&answer), Some(call)))?),
            None => None,
        };
        match (member(&answer, "error"), screen) {
            (Some(J::Str(why)), screen) => {
                let name = &self.target;
                let shown = call.display();
                Err(Failure {
                    code: "E035",
                    msg: tr!("`{name}` が `{shown}` を拒否しました: {why}", "`{name}` refused `{shown}`: {why}"),
                    notes: gui::refused_notes(screen.as_ref(), call),
                })
            }
            (None, Some(screen)) => Ok(screen),
            _ => Err(self.unreadable(&json::render(&answer), Some(call))),
        }
    }

    /// Says goodbye, gives the driver 5 s to exit, then stops what is left of its
    /// group. Its stderr.
    pub fn close(mut self) -> String {
        if let Some(w) = self.proc.stdin.as_mut() {
            let _ = w.write_all(b"{\"do\":\"close\"}\n");
            let _ = w.flush();
        }
        self.proc.stdin = None;
        let _ = self.proc.wait_until(Instant::now() + ANSWER);
        self.proc.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_of_each_action() {
        let c = |call: Call| action_line(&call);
        assert_eq!(c(Call::Open(None)), r#"{"do":"open"}"#);
        assert_eq!(c(Call::Open(Some("/".into()))), r#"{"do":"open","path":"/"}"#);
        assert_eq!(c(Call::Click { name: "greet".into(), nth: None }), r#"{"do":"click","name":"greet","nth":1}"#);
        assert_eq!(c(Call::Input { text: "Ada".into(), place: Place::First }), r#"{"do":"input","text":"Ada","field":1}"#);
        assert_eq!(
            c(Call::Input { text: "a\"b".into(), place: Place::Into { name: "name".into(), nth: Some(2) } }),
            r#"{"do":"input","text":"a\"b","into":"name","nth":2}"#
        );
        assert_eq!(c(Call::Submit(Place::Field(2))), r#"{"do":"submit","field":2}"#);
        assert_eq!(c(Call::Press("cmd-s".into())), r#"{"do":"press","key":"cmd-s"}"#);
        assert_eq!(c(Call::Advance(500)), r#"{"do":"advance","ms":500}"#);
    }

    #[test]
    fn a_screen_sent_by_a_driver() {
        let j = json::parse(r#"{"children":[{"role":"button","name":"greet","children":[{"role":"text","name":"greet"}]},{"children":[{"role":"text","name":"inside"}]}]}"#).unwrap();
        assert_eq!(screen_of(&j).unwrap().text(0), "button \"greet\"\ntext \"inside\"\n");
        // a root with a role is a node of the screen
        let j = json::parse(r#"{"role":"window","name":"Greeter","children":[{"role":"button","name":"greet"}]}"#).unwrap();
        assert_eq!(screen_of(&j).unwrap().text(0), "window \"Greeter\"\n  button \"greet\"\n");
    }
}
