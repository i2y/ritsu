//! Chrome, the second driver (DESIGN §8.4): a `serve` target driven as a page in
//! headless Chrome, over the DevTools Protocol on one WebSocket on 127.0.0.1.
//!
//! One Chrome per worker, started the first time a claim of that worker opens a
//! page, with a profile directory of its own under `.geas/`, in a process group of
//! its own; stopped, and its profile removed, when the run ends, panics, or a
//! signal ends geas. Each claim gets a browser context of its own, so cookies and
//! storage do not pass from one claim to the next, and each target a page in it.
//!
//! The page's clock is virtual: paused before the page loads (at the pinned clock,
//! or else now), it runs one second of the page's own time after each action, and
//! waits while a fetch is pending; a page whose second does not run out within 10 s
//! is E036. The screen is `Accessibility.getFullAXTree`, put through the rules of
//! §8.4 and then §8.1's.


use ritsu_base::text::Text;
use crate::http;
use crate::json::{self, J};
use crate::model::{Call, Place, Target};
use crate::proc::{self, Env, Failure, Launch, Proc};
use crate::screen::{self, Node};
use crate::ws::Ws;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long Chrome has to open its debugging port.
const START: Duration = Duration::from_secs(15);
/// How long a command may take to be answered.
const ANSWER: Duration = Duration::from_secs(10);
/// How long a page's virtual time has, in real time, to run out (E036).
const SETTLE: Duration = Duration::from_secs(10);
/// The page's own time an action grants it, in milliseconds.
const STEP_MS: u64 = 1000;

fn e034(en: String, ja: String) -> Failure {
    Failure { code: "E034", msg: Text::new(ja, en), notes: vec![] }
}

/// The Chrome geas starts: `GEAS_CHROME` when it is set (and then only that), else
/// the macOS application, else `google-chrome`, `chromium` or `chromium-browser` on
/// PATH.
pub fn find_chrome() -> Result<PathBuf, Failure> {
    if let Some(p) = std::env::var_os("GEAS_CHROME") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
        let shown = p.display().to_string();
        return Err(e034(
            format!("GEAS_CHROME names `{shown}`, which is not a file geas can start"),
            format!("GEAS_CHROME が指す `{shown}` は、geas が起動できるファイルではありません"),
        ));
    }
    let mac = Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
    if mac.is_file() {
        return Ok(mac.to_path_buf());
    }
    for name in ["google-chrome", "chromium", "chromium-browser"] {
        if let Some(p) = crate::cover::on_path(name) {
            return Ok(p);
        }
    }
    Err(Failure {
        code: "E034",
        msg: tr!(
            "ページを開く Chrome が見つかりません",
            "geas found no Chrome to open the page in",
        ),
        notes: vec![tr!(
            "geas は GEAS_CHROME、macOS のアプリケーション、PATH の google-chrome・chromium・chromium-browser の順に探します",
            "geas looks at GEAS_CHROME, then the macOS application, then google-chrome, chromium and chromium-browser on PATH",
        )],
    })
}

/// Why a command got no answer.
enum CdpError {
    Timeout,
    /// The socket closed, or Chrome said the command failed.
    Failed(String),
}

impl CdpError {
    fn why(&self) -> String {
        match self {
            CdpError::Timeout => format!("no answer within {} s", ANSWER.as_secs()),
            CdpError::Failed(s) => s.clone(),
        }
    }
}

fn member<'a>(j: &'a J, key: &str) -> Option<&'a J> {
    match j {
        J::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn text_of(j: Option<&J>) -> String {
    match j {
        Some(J::Str(s)) => s.clone(),
        Some(J::Num(n)) => json::render_num(*n),
        Some(J::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// The DevTools Protocol over the browser's WebSocket: commands answered by id,
/// the events geas waits for kept until it asks for them.
struct Cdp {
    ws: Ws,
    next: u64,
    events: Vec<J>,
}

impl Cdp {
    fn send(&mut self, session: Option<&str>, method: &str, params: &str) -> Result<u64, CdpError> {
        self.next += 1;
        let id = self.next;
        let sess = session.map(|s| format!(",\"sessionId\":{}", json::quote(s))).unwrap_or_default();
        self.ws
            .send_text(&format!("{{\"id\":{id},\"method\":{},\"params\":{params}{sess}}}", json::quote(method)))
            .map_err(|e| CdpError::Failed(e.to_string()))?;
        Ok(id)
    }

    /// One message, the events geas waits for kept, and a JavaScript dialog,
    /// which would stop the page, accepted.
    fn read(&mut self, deadline: Instant) -> Result<Option<J>, CdpError> {
        let Some(text) = self.ws.recv_text(deadline).map_err(|e| CdpError::Failed(e.to_string()))? else {
            return Ok(None);
        };
        let j = json::parse(&text).map_err(|e| CdpError::Failed(format!("Chrome sent something that is not JSON: {e}")))?;
        match text_of(member(&j, "method")).as_str() {
            "Emulation.virtualTimeBudgetExpired" => self.events.push(j.clone()),
            "Page.javascriptDialogOpening" => {
                let s = text_of(member(&j, "sessionId"));
                let _ = self.send(Some(&s), "Page.handleJavaScriptDialog", "{\"accept\":true}");
            }
            _ => {}
        }
        Ok(Some(j))
    }

    fn call(&mut self, session: Option<&str>, method: &str, params: &str) -> Result<J, CdpError> {
        let id = self.send(session, method, params)?;
        let deadline = Instant::now() + ANSWER;
        loop {
            let Some(j) = self.read(deadline)? else {
                return Err(CdpError::Timeout);
            };
            if matches!(member(&j, "id"), Some(J::Num(n)) if *n as u64 == id) {
                if let Some(e) = member(&j, "error") {
                    return Err(CdpError::Failed(format!("{method}: {}", text_of(member(e, "message")))));
                }
                return Ok(member(&j, "result").cloned().unwrap_or(J::Obj(vec![])));
            }
        }
    }

    /// Waits for an event of a session; false when it does not come by `deadline`.
    fn event(&mut self, session: &str, method: &str, deadline: Instant) -> Result<bool, CdpError> {
        loop {
            if let Some(i) = self
                .events
                .iter()
                .position(|e| text_of(member(e, "method")) == method && text_of(member(e, "sessionId")) == session)
            {
                self.events.remove(i);
                return Ok(true);
            }
            if self.read(deadline)?.is_none() {
                return Ok(false);
            }
        }
    }
}

/// A Chrome geas started.
struct Chrome {
    proc: Proc,
    cdp: Cdp,
    profile: PathBuf,
}

impl Chrome {
    fn launch(geas_dir: &Path, worker: usize) -> Result<Chrome, Failure> {
        let exe = find_chrome()?;
        let profile = geas_dir.join(format!("chrome-{}", worker + 1));
        let _ = std::fs::remove_dir_all(&profile);
        if let Err(e) = std::fs::create_dir_all(&profile) {
            return Err(e034(
                format!("cannot make Chrome's profile directory under .geas/: {e}"),
                format!(".geas/ の下に Chrome のプロファイルのディレクトリを作れません: {e}"),
            ));
        }
        proc::remove_on_signal(&profile);
        let words: Vec<String> = [
            exe.to_string_lossy().into_owned(),
            "--headless=new".into(),
            "--remote-debugging-port=0".into(),
            format!("--user-data-dir={}", profile.display()),
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
            "--disable-gpu".into(),
            "--disable-extensions".into(),
            "--disable-background-networking".into(),
            "--disable-sync".into(),
            "--mute-audio".into(),
            "about:blank".into(),
        ]
        .into();
        let env = Env { clear: false, vars: vec![] };
        let launch = Launch { dir: geas_dir, env: &env, term: false };
        let gone = |profile: &Path| {
            let _ = std::fs::remove_dir_all(profile);
            proc::forget_on_signal(profile);
        };
        let mut p = match proc::start("chrome", &words, &launch, false, false) {
            Ok(p) => p,
            Err(f) => {
                gone(&profile);
                return Err(Failure { code: "E034", msg: f.msg, notes: vec![] });
            }
        };
        let file = profile.join("DevToolsActivePort");
        let deadline = Instant::now() + START;
        let (port, path) = loop {
            if let Ok(text) = std::fs::read_to_string(&file) {
                let lines: Vec<&str> = text.lines().collect();
                if let [port, path, ..] = lines.as_slice()
                    && let Ok(port) = port.trim().parse::<u16>()
                {
                    break (port, path.trim().to_string());
                }
            }
            if let Some(st) = p.wait_until(Instant::now()) {
                let how = proc::how_it_ended(st);
                let stderr = p.finish();
                gone(&profile);
                return Err(Failure {
                    code: "E034",
                    msg: tr!(
                        "Chrome がデバッグ用のポートを開く前に終了しました（{}）",
                        "Chrome exited before it opened its debugging port ({})",
                        how.ja;
                        how.en,
                    ),
                    notes: proc::stderr_tail(&stderr),
                });
            }
            if Instant::now() >= deadline {
                let stderr = p.finish();
                gone(&profile);
                return Err(Failure {
                    code: "E034",
                    msg: tr!(
                        "Chrome が {} 秒のうちにデバッグ用のポートを開かなかったので、geas が止めました",
                        "Chrome did not open its debugging port within {} s, so geas stopped it",
                        START.as_secs(),
                    ),
                    notes: proc::stderr_tail(&stderr),
                });
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        match Ws::connect(port, &path, Duration::from_secs(5)) {
            Ok(ws) => Ok(Chrome { proc: p, cdp: Cdp { ws, next: 0, events: vec![] }, profile }),
            Err(e) => {
                let _ = p.finish();
                gone(&profile);
                Err(e034(
                    format!("Chrome opened its debugging port, and geas cannot speak to it there: {e}"),
                    format!("Chrome がデバッグ用のポートを開きましたが、geas はそこで通信できません: {e}"),
                ))
            }
        }
    }

    /// Stops Chrome and every process of its group, then removes its profile.
    fn stop(mut self) {
        self.cdp.ws.close();
        let _ = self.proc.finish();
        for _ in 0..50 {
            let _ = std::fs::remove_dir_all(&self.profile);
            if !self.profile.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        proc::forget_on_signal(&self.profile);
    }
}

enum Slot {
    Idle,
    Up(Chrome),
    /// Chrome could not be had; every claim of the worker that needs it says why.
    Failed(Failure),
}

/// The browsers of a run, one slot per worker, each started when first needed and
/// stopped when the run ends, also by a panic.
pub struct Browsers {
    geas_dir: PathBuf,
    slots: Vec<Mutex<Slot>>,
}

impl Browsers {
    pub fn new(geas_dir: &Path, workers: usize) -> Browsers {
        Browsers { geas_dir: geas_dir.to_path_buf(), slots: (0..workers.max(1)).map(|_| Mutex::new(Slot::Idle)).collect() }
    }

    /// The pages of a claim run by `worker`: a browser context of its own in the
    /// worker's Chrome, started when it is not running yet.
    pub fn pages(&self, worker: usize) -> Result<Pages<'_>, Failure> {
        let mut guard = self.slots[worker.min(self.slots.len() - 1)].lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*guard, Slot::Idle) {
            *guard = match Chrome::launch(&self.geas_dir, worker) {
                Ok(c) => Slot::Up(c),
                Err(f) => Slot::Failed(f),
            };
        }
        let chrome = match &mut *guard {
            Slot::Up(c) => c,
            Slot::Failed(f) => return Err(Failure { code: f.code, msg: f.msg.clone(), notes: f.notes.clone() }),
            Slot::Idle => unreachable!("launched above"),
        };
        let r = chrome.cdp.call(None, "Target.createBrowserContext", "{\"disposeOnDetach\":true}").map_err(|e| unanswered("Target.createBrowserContext", &e))?;
        let context = text_of(member(&r, "browserContextId"));
        Ok(Pages { guard, context, pages: HashMap::new() })
    }
}

impl Drop for Browsers {
    fn drop(&mut self) {
        for slot in &self.slots {
            let mut g = slot.lock().unwrap_or_else(|e| e.into_inner());
            if let Slot::Up(c) = std::mem::replace(&mut *g, Slot::Idle) {
                c.stop();
            }
        }
    }
}

/// E034 for a command Chrome did not answer.
fn unanswered(method: &str, e: &CdpError) -> Failure {
    let why = e.why();
    e034(
        format!("Chrome did not carry out {method}: {why}"),
        format!("Chrome が {method} を実行しませんでした: {why}"),
    )
}

/// A target's page in a claim's browser context.
struct Page {
    session: String,
    port: u16,
    auto: bool,
}

/// A claim's pages: its browser context in the worker's Chrome, held for the claim,
/// and a page per target.
pub struct Pages<'a> {
    guard: MutexGuard<'a, Slot>,
    context: String,
    pages: HashMap<String, Page>,
}

/// The time a page's clock starts at: the pinned one, or now.
fn start_time(tg: &Target) -> f64 {
    if let Some(at) = tg.pins.clock.as_ref().and_then(|(c, _)| crate::pins::rfc3339(c)) {
        return at;
    }
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// The script that makes the page's `Math.random` a generator seeded with `seed`
/// (an xorshift on 32 bits), run before any of the page's own scripts.
fn seeded_random(seed: u64) -> String {
    format!(
        "(() => {{ let x = {} >>> 0 || 1; Math.random = function () {{ x ^= x << 13; x >>>= 0; x ^= x >>> 17; x ^= x << 5; x >>>= 0; return x / 4294967296; }}; }})();",
        seed % 4_294_967_296
    )
}

/// A key's DevTools description: `key`, `code`, the key code, and the text it types.
fn key_of(name: &str, shift: bool) -> (String, String, u32, Option<String>) {
    let named: &[(&str, &str, &str, u32, Option<&str>)] = &[
        ("enter", "Enter", "Enter", 13, Some("\r")),
        ("escape", "Escape", "Escape", 27, None),
        ("tab", "Tab", "Tab", 9, None),
        ("backspace", "Backspace", "Backspace", 8, None),
        ("delete", "Delete", "Delete", 46, None),
        ("space", " ", "Space", 32, Some(" ")),
        ("up", "ArrowUp", "ArrowUp", 38, None),
        ("down", "ArrowDown", "ArrowDown", 40, None),
        ("left", "ArrowLeft", "ArrowLeft", 37, None),
        ("right", "ArrowRight", "ArrowRight", 39, None),
        ("home", "Home", "Home", 36, None),
        ("end", "End", "End", 35, None),
        ("pageup", "PageUp", "PageUp", 33, None),
        ("pagedown", "PageDown", "PageDown", 34, None),
    ];
    if let Some((_, key, code, kc, text)) = named.iter().find(|k| k.0 == name) {
        return (key.to_string(), code.to_string(), *kc, text.map(String::from));
    }
    if let Some(n) = name.strip_prefix('f').and_then(|d| d.parse::<u32>().ok()) {
        let f = format!("F{n}");
        return (f.clone(), f, 111 + n, None);
    }
    let c = name.chars().next().unwrap_or(' ');
    if c.is_ascii_digit() {
        return (c.to_string(), format!("Digit{c}"), c as u32, Some(c.to_string()));
    }
    // a capital letter is the letter with shift
    let up = c.to_ascii_uppercase();
    let key = if shift || c.is_ascii_uppercase() { up.to_string() } else { c.to_ascii_lowercase().to_string() };
    (key.clone(), format!("Key{up}"), up as u32, Some(key))
}

impl Pages<'_> {
    fn cdp(&mut self) -> &mut Cdp {
        match &mut *self.guard {
            Slot::Up(c) => &mut c.cdp,
            _ => unreachable!("a claim's pages hold a running Chrome"),
        }
    }

    fn call(&mut self, session: &str, method: &str, params: &str) -> Result<J, Failure> {
        self.cdp().call(Some(session), method, params).map_err(|e| unanswered(method, &e))
    }

    /// The page of a target, made in the claim's context the first time: the
    /// domains enabled, the pins kept, the clock paused at its start.
    fn page(&mut self, tg: &Target, port: u16, auto: bool) -> Result<String, Failure> {
        if let Some(p) = self.pages.get(&tg.name) {
            return Ok(p.session.clone());
        }
        let ctx = self.context.clone();
        let r = self
            .cdp()
            .call(None, "Target.createTarget", &format!("{{\"url\":\"about:blank\",\"browserContextId\":{}}}", json::quote(&ctx)))
            .map_err(|e| unanswered("Target.createTarget", &e))?;
        let target_id = text_of(member(&r, "targetId"));
        let r = self
            .cdp()
            .call(None, "Target.attachToTarget", &format!("{{\"targetId\":{},\"flatten\":true}}", json::quote(&target_id)))
            .map_err(|e| unanswered("Target.attachToTarget", &e))?;
        let session = text_of(member(&r, "sessionId"));
        for m in ["Page.enable", "Accessibility.enable", "DOM.enable", "Runtime.enable"] {
            self.call(&session, m, "{}")?;
        }
        let pins = &tg.pins;
        if let Some(l) = &pins.locale {
            self.pin(&session, "Emulation.setLocaleOverride", &format!("{{\"locale\":{}}}", json::quote(l)), "locale", l)?;
        }
        if let Some(z) = &pins.tz {
            self.pin(&session, "Emulation.setTimezoneOverride", &format!("{{\"timezoneId\":{}}}", json::quote(z)), "tz", z)?;
        }
        if let Some((seed, _)) = pins.seed {
            self.call(&session, "Page.addScriptToEvaluateOnNewDocument", &format!("{{\"source\":{}}}", json::quote(&seeded_random(seed))))?;
        }
        let start = start_time(tg);
        self.call(&session, "Emulation.setVirtualTimePolicy", &format!("{{\"policy\":\"pause\",\"initialVirtualTime\":{start}}}"))?;
        self.pages.insert(tg.name.clone(), Page { session: session.clone(), port, auto });
        Ok(session)
    }

    /// A pin the page takes through an Emulation command; E011 when Chrome refuses
    /// its value.
    fn pin(&mut self, session: &str, method: &str, params: &str, word: &str, value: &str) -> Result<(), Failure> {
        match self.cdp().call(Some(session), method, params) {
            Ok(_) => Ok(()),
            Err(CdpError::Failed(why)) => Err(Failure {
                code: "E011",
                msg: tr!(
                    "Chrome は、ページの `{word} \"{value}\"` を固定できません: {why}",
                    "Chrome cannot keep `{word} \"{value}\"` for the page: {why}",
                ),
                notes: vec![],
            }),
            Err(e) => Err(unanswered(method, &e)),
        }
    }

    /// Lets the page's clock run `ms` of its own time, and waits, at most 10 s of
    /// real time, for it to run out (E036 otherwise).
    fn settle(&mut self, tg: &Target, session: &str, ms: u64) -> Result<(), Failure> {
        self.call(session, "Emulation.setVirtualTimePolicy", &format!("{{\"policy\":\"pauseIfNetworkFetchesPending\",\"budget\":{ms}}}"))?;
        let ran_out = self
            .cdp()
            .event(session, "Emulation.virtualTimeBudgetExpired", Instant::now() + SETTLE)
            .map_err(|e| unanswered("Emulation.setVirtualTimePolicy", &e))?;
        if ran_out {
            return Ok(());
        }
        let name = &tg.name;
        let secs = SETTLE.as_secs();
        Err(Failure {
            code: "E036",
            msg: tr!(
                "`{name}` のページが {secs} 秒のうちに落ち着きませんでした",
                "the page of `{name}` did not settle within {secs} s",
            ),
            notes: vec![tr!(
                "geas は操作のたびにページの時刻を進め、その時刻はページのリクエストが終わるまで止まります。レスポンスの返らないリクエストがあると、時刻は進みません",
                "geas lets a page's clock run after each action, and the clock waits while a fetch is pending: a request the service never answers holds it",
            )],
        })
    }

    /// The screen: the page's accessibility tree, normalized, with the port of a
    /// `port auto` service written `{port}` (DESIGN §10).
    fn screen(&mut self, session: &str, port: u16, auto: bool) -> Result<Node, Failure> {
        let r = self.call(session, "Accessibility.getFullAXTree", "{}")?;
        let Some(J::Arr(nodes)) = member(&r, "nodes") else {
            return Ok(screen::screen(vec![]));
        };
        let mut s = ax_screen(nodes);
        if auto {
            port_written(&mut s, port);
        }
        Ok(s)
    }

    /// `open("/path")`: the page of the target loads that path of its service.
    pub fn open(&mut self, tg: &Target, port: u16, auto: bool, path: &str) -> Result<Node, Failure> {
        let session = self.page(tg, port, auto)?;
        let url = format!("http://127.0.0.1:{port}{path}");
        let name = &tg.name;
        // Chrome answers a navigation once the service has begun to answer it
        let r = match self.cdp().call(Some(&session), "Page.navigate", &format!("{{\"url\":{}}}", json::quote(&url))) {
            Ok(r) => r,
            Err(CdpError::Timeout) => {
                let secs = ANSWER.as_secs();
                return Err(Failure {
                    code: "E036",
                    msg: tr!(
                        "`{name}` のページ `{path}` が {secs} 秒のうちに読み込まれませんでした",
                        "the page `{path}` of `{name}` did not load within {secs} s",
                    ),
                    notes: vec![tr!("サービスが、ページのリクエストにレスポンスを返しませんでした", "the service did not answer the request for the page")],
                });
            }
            Err(e) => return Err(unanswered("Page.navigate", &e)),
        };
        let err = text_of(member(&r, "errorText"));
        if !err.is_empty() {
            return Err(Failure {
                code: "E036",
                msg: tr!(
                    "`{name}` のページ `{path}` を読み込めませんでした: {err}",
                    "the page `{path}` of `{name}` did not load: {err}",
                ),
                notes: vec![],
            });
        }
        self.settle(tg, &session, STEP_MS)?;
        self.screen(&session, port, auto)
    }

    /// Any other action on the target's page, and the screen after it.
    pub fn act(&mut self, tg: &Target, call: &Call) -> Result<Node, Failure> {
        let p = self.pages.get(&tg.name).expect("E013 has the page opened first");
        let (session, port, auto) = (p.session.clone(), p.port, p.auto);
        let mut ms = STEP_MS;
        match call {
            Call::Click { name, nth } => {
                let now = self.screen(&session, port, auto)?;
                let target = find(&now, screen::CLICKABLE, Some(name), nth.unwrap_or(1));
                let Some(handle) = target else {
                    return Err(not_there(tg, call, &now));
                };
                self.click(tg, call, &session, handle, &now)?;
            }
            Call::Input { text, place } => {
                let now = self.screen(&session, port, auto)?;
                let handle = field(&now, place).ok_or_else(|| not_there(tg, call, &now))?;
                self.call(&session, "DOM.focus", &format!("{{\"backendNodeId\":{handle}}}"))?;
                // what the field holds is replaced, as a person selecting it all and typing would
                let r = self.call(&session, "DOM.resolveNode", &format!("{{\"backendNodeId\":{handle}}}"))?;
                let object = text_of(member(member(&r, "object").unwrap_or(&J::Null), "objectId"));
                if !object.is_empty() {
                    let select = "function () { if (typeof this.select === 'function') { this.select(); } else { const r = document.createRange(); r.selectNodeContents(this); const s = getSelection(); s.removeAllRanges(); s.addRange(r); } }";
                    self.call(&session, "Runtime.callFunctionOn", &format!("{{\"objectId\":{},\"functionDeclaration\":{}}}", json::quote(&object), json::quote(select)))?;
                }
                if text.is_empty() {
                    self.key(&session, "backspace", 0)?;
                } else {
                    self.call(&session, "Input.insertText", &format!("{{\"text\":{}}}", json::quote(text)))?;
                }
            }
            Call::Submit(place) => {
                let now = self.screen(&session, port, auto)?;
                let handle = field(&now, place).ok_or_else(|| not_there(tg, call, &now))?;
                self.call(&session, "DOM.focus", &format!("{{\"backendNodeId\":{handle}}}"))?;
                self.key(&session, "enter", 0)?;
            }
            Call::Press(chord) => {
                let parts: Vec<&str> = chord.split(['-', '+']).collect();
                let (key, mods) = parts.split_last().expect("E013 refuses an empty key");
                let mut m = 0;
                for w in mods {
                    m |= match *w {
                        "alt" => 1,
                        "ctrl" => 2,
                        "cmd" => 4,
                        _ => 8,
                    };
                }
                self.key(&session, key, m)?;
            }
            Call::Advance(n) => ms = *n,
            Call::Open(_) | Call::Run(_) | Call::Get(_) | Call::Post { .. } => unreachable!("handled apart"),
        }
        self.settle(tg, &session, ms)?;
        self.screen(&session, port, auto)
    }

    /// Presses the mouse at the centre of the node's box, scrolled into view, so
    /// that whatever a person's click would land on gets it.
    fn click(&mut self, tg: &Target, call: &Call, session: &str, handle: i64, now: &Node) -> Result<(), Failure> {
        let _ = self.cdp().call(Some(session), "DOM.scrollIntoViewIfNeeded", &format!("{{\"backendNodeId\":{handle}}}"));
        let model = match self.cdp().call(Some(session), "DOM.getBoxModel", &format!("{{\"backendNodeId\":{handle}}}")) {
            Ok(r) => r,
            Err(CdpError::Failed(_)) => {
                let mut f = not_there(tg, call, now);
                f.msg = tr!(
                    "`{}` が指すものは `{}` のページにありますが、表示されていないのでクリックできません",
                    "what `{}` names is on the page of `{}`, and it is not shown, so it has no place to click",
                    call.display(),
                    tg.name,
                );
                return Err(f);
            }
            Err(e) => return Err(unanswered("DOM.getBoxModel", &e)),
        };
        let quad: Vec<f64> = match member(member(&model, "model").unwrap_or(&J::Null), "content") {
            Some(J::Arr(a)) => a.iter().map(|v| if let J::Num(n) = v { *n } else { 0.0 }).collect(),
            _ => vec![],
        };
        if quad.len() < 8 {
            return Err(not_there(tg, call, now));
        }
        let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
        let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;
        for kind in ["mousePressed", "mouseReleased"] {
            self.call(session, "Input.dispatchMouseEvent", &format!("{{\"type\":\"{kind}\",\"x\":{x},\"y\":{y},\"button\":\"left\",\"clickCount\":1}}"))?;
        }
        Ok(())
    }

    /// A key down and up, with modifiers (alt 1, ctrl 2, cmd 4, shift 8); the text
    /// it types goes with it unless a modifier other than shift is held.
    fn key(&mut self, session: &str, name: &str, modifiers: u32) -> Result<(), Failure> {
        let (key, code, kc, text) = key_of(name, modifiers & 8 != 0);
        let typed = match text {
            Some(tx) if modifiers & 7 == 0 => format!(",\"text\":{}", json::quote(&tx)),
            _ => String::new(),
        };
        let common = format!("\"key\":{},\"code\":{},\"windowsVirtualKeyCode\":{kc},\"modifiers\":{modifiers}", json::quote(&key), json::quote(&code));
        self.call(session, "Input.dispatchKeyEvent", &format!("{{\"type\":\"keyDown\",{common}{typed}}}"))?;
        self.call(session, "Input.dispatchKeyEvent", &format!("{{\"type\":\"keyUp\",{common}}}"))?;
        Ok(())
    }

    /// The claim is over: its browser context goes, with its pages and storage.
    pub fn close(mut self) {
        let ctx = self.context.clone();
        let _ = self.cdp().call(None, "Target.disposeBrowserContext", &format!("{{\"browserContextId\":{}}}", json::quote(&ctx)));
    }
}

/// The handle of the n-th node (from 1) of these roles, and of this name when one
/// is given, in tree order.
fn find(s: &Node, roles: &[&str], name: Option<&str>, nth: usize) -> Option<i64> {
    s.walk()
        .into_iter()
        .filter(|(n, _)| roles.contains(&n.role.as_str()) && name.is_none_or(|w| n.name == w))
        .nth(nth.saturating_sub(1))
        .and_then(|(n, _)| n.handle)
}

/// The text field a place names.
fn field(s: &Node, place: &Place) -> Option<i64> {
    match place {
        Place::First => find(s, screen::FIELDS, None, 1),
        Place::Field(n) => find(s, screen::FIELDS, None, *n),
        Place::Into { name, nth } => find(s, screen::FIELDS, Some(name), nth.unwrap_or(1)),
    }
}

/// E035: the page has nothing the action can act on.
fn not_there(tg: &Target, call: &Call, now: &Node) -> Failure {
    let name = &tg.name;
    let (en, ja) = match call {
        Call::Click { name: w, nth } => match nth.unwrap_or(1) {
            1 => (
                format!("the page of `{name}` has no button, link, checkbox, radio, switch, tab, menu item or option named \"{w}\""),
                format!("`{name}` のページに、\"{w}\" という名前のボタン、リンク、チェックボックス、ラジオボタン、スイッチ、タブ、メニュー項目、選択肢がありません"),
            ),
            n => (
                format!("the page of `{name}` has fewer than {n} buttons, links, checkboxes, radios, switches, tabs, menu items and options named \"{w}\""),
                format!("`{name}` のページで、\"{w}\" という名前のボタン、リンク、チェックボックス、ラジオボタン、スイッチ、タブ、メニュー項目、選択肢は、合わせて {n} 個に足りません"),
            ),
        },
        _ => (
            format!("the page of `{name}` has no text field that `{}` names", call.display()),
            format!("`{name}` のページに、`{}` が指すテキストフィールドがありません", call.display()),
        ),
    };
    Failure { code: "E035", msg: Text::new(ja, en), notes: crate::gui::refused_notes(Some(now), call) }
}

/// Every name and value with the service's own address written `{port}`.
fn port_written(n: &mut Node, port: u16) {
    n.name = http::with_port_written(&n.name, port);
    n.value = http::with_port_written(&n.value, port);
    for c in &mut n.children {
        port_written(c, port);
    }
}

/// The roles Chrome reports that hand their children up when they have no name, as
/// layout: DESIGN §8.4's, and Chrome's own names for a closed `<select>`'s popup and
/// a header or footer that is not a landmark.
const LAYOUT: &[&str] = &["generic", "none", "presentation", "paragraph", "LabelText", "MenuListPopup", "sectionheader", "sectionfooter"];

/// Chrome's roles that say nothing a person reads: the boxes of a text's lines, a
/// list's bullet, a line break. Dropped with what is under them.
const DROPPED: &[&str] = &["InlineTextBox", "ListMarker", "LineBreak"];

/// Chrome's accessibility tree as a screen (DESIGN §8.4): ignored nodes and
/// unnamed layout hand their children up, `StaticText` is `text`, `image` is
/// `img`, the root (`RootWebArea`) is the screen, and the states geas knows are
/// read from the properties; then §8.1's rules.
pub fn ax_screen(nodes: &[J]) -> Node {
    let mut by_id: HashMap<String, &J> = HashMap::new();
    let mut root = None;
    for n in nodes {
        let id = text_of(member(n, "nodeId"));
        if member(n, "parentId").is_none() && root.is_none() {
            root = Some(id.clone());
        }
        by_id.insert(id, n);
    }
    fn build(id: &str, by_id: &HashMap<String, &J>, depth: usize) -> Option<Node> {
        let n = by_id.get(id)?;
        let role = text_of(member(n, "role").and_then(|r| member(r, "value")));
        if DROPPED.contains(&role.as_str()) || depth > 512 {
            return None;
        }
        let name = text_of(member(n, "name").and_then(|r| member(r, "value")));
        let value = text_of(member(n, "value").and_then(|r| member(r, "value")));
        let ignored = matches!(member(n, "ignored"), Some(J::Bool(true)));
        let children: Vec<Node> = match member(n, "childIds") {
            Some(J::Arr(ids)) => ids.iter().filter_map(|c| build(&text_of(Some(c)), by_id, depth + 1)).collect(),
            _ => vec![],
        };
        let hand_up = ignored || role == "RootWebArea" || (name.is_empty() && LAYOUT.contains(&role.as_str()));
        if hand_up {
            return Some(Node { children, ..Node::default() });
        }
        let mut states = Vec::new();
        if let Some(J::Arr(props)) = member(n, "properties") {
            for p in props {
                let v = text_of(member(p, "value").and_then(|v| member(v, "value")));
                match (text_of(member(p, "name")).as_str(), v.as_str()) {
                    ("disabled", "true") => states.push("disabled".to_string()),
                    ("checked", "true") => states.push("checked".to_string()),
                    ("checked", "false") => states.push("unchecked".to_string()),
                    _ => {}
                }
            }
        }
        let role = match role.as_str() {
            "StaticText" => "text".to_string(),
            "image" => "img".to_string(),
            _ => role,
        };
        let handle = match member(n, "backendDOMNodeId") {
            Some(J::Num(x)) => Some(*x as i64),
            _ => None,
        };
        Some(Node { role, name, value, states, children, masked: false, handle })
    }
    let top = root.and_then(|r| build(&r, &by_id, 0)).unwrap_or_default();
    screen::screen(screen::normalize(vec![top]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ax(nodes: &str) -> Node {
        let j = json::parse(nodes).unwrap();
        let J::Arr(a) = j else { panic!() };
        ax_screen(&a)
    }

    #[test]
    fn chromes_tree_as_a_screen() {
        // as measured on Chrome 154: a heading, a form with a label, a field holding
        // what was typed, a button, an image, a checkbox, a disabled button
        let s = ax(r#"[
          {"nodeId":"1","ignored":false,"role":{"value":"RootWebArea"},"name":{"value":"Probe page"},"childIds":["2"],"backendDOMNodeId":1},
          {"nodeId":"2","parentId":"1","ignored":true,"role":{"value":"none"},"childIds":["3","4","20","21","22","23"]},
          {"nodeId":"3","parentId":"2","ignored":false,"role":{"value":"heading"},"name":{"value":"Shopping list"},"childIds":["5"],"backendDOMNodeId":3},
          {"nodeId":"5","parentId":"3","ignored":false,"role":{"value":"StaticText"},"name":{"value":"Shopping list"},"childIds":["6"],"backendDOMNodeId":5},
          {"nodeId":"6","parentId":"5","ignored":false,"role":{"value":"InlineTextBox"},"name":{"value":"Shopping list"}},
          {"nodeId":"4","parentId":"2","ignored":false,"role":{"value":"form"},"name":{"value":""},"childIds":["7","8","9"],"backendDOMNodeId":4},
          {"nodeId":"7","parentId":"4","ignored":false,"role":{"value":"LabelText"},"name":{"value":""},"childIds":["10"]},
          {"nodeId":"10","parentId":"7","ignored":false,"role":{"value":"StaticText"},"name":{"value":"Item"}},
          {"nodeId":"8","parentId":"4","ignored":false,"role":{"value":"textbox"},"name":{"value":"Item"},"value":{"value":"milk"},"childIds":["11"],"backendDOMNodeId":8},
          {"nodeId":"11","parentId":"8","ignored":false,"role":{"value":"generic"},"name":{"value":""},"childIds":["12"]},
          {"nodeId":"12","parentId":"11","ignored":false,"role":{"value":"StaticText"},"name":{"value":"milk"}},
          {"nodeId":"9","parentId":"4","ignored":false,"role":{"value":"button"},"name":{"value":"Add"},"childIds":["13"],"backendDOMNodeId":9},
          {"nodeId":"13","parentId":"9","ignored":false,"role":{"value":"StaticText"},"name":{"value":"Add"}},
          {"nodeId":"20","parentId":"2","ignored":false,"role":{"value":"image"},"name":{"value":"a picture"}},
          {"nodeId":"21","parentId":"2","ignored":false,"role":{"value":"checkbox"},"name":{"value":"Agree"},"properties":[{"name":"checked","value":{"type":"tristate","value":"true"}}]},
          {"nodeId":"22","parentId":"2","ignored":false,"role":{"value":"button"},"name":{"value":"Off"},"properties":[{"name":"disabled","value":{"type":"boolean","value":true}}]},
          {"nodeId":"23","parentId":"2","ignored":false,"role":{"value":"listitem"},"name":{"value":""},"childIds":["24","25"]},
          {"nodeId":"24","parentId":"23","ignored":false,"role":{"value":"ListMarker"},"name":{"value":"• "}},
          {"nodeId":"25","parentId":"23","ignored":false,"role":{"value":"StaticText"},"name":{"value":"eggs"}}
        ]"#);
        assert_eq!(
            s.text(0),
            "heading \"Shopping list\"\nform\n  text \"Item\"\n  textbox \"Item\" value \"milk\"\n  button \"Add\"\nimg \"a picture\"\ncheckbox \"Agree\" checked\nbutton \"Off\" disabled\nlistitem\n  text \"eggs\"\n"
        );
        // the handles stay for acting on the nodes
        assert_eq!(find(&s, screen::CLICKABLE, Some("Add"), 1), Some(9));
        assert_eq!(field(&s, &Place::First), Some(8));
        assert_eq!(field(&s, &Place::Into { name: "Item".into(), nth: None }), Some(8));
        assert_eq!(find(&s, screen::CLICKABLE, Some("Add"), 2), None);
    }

    #[test]
    fn keys_as_devtools_takes_them() {
        assert_eq!(key_of("enter", false), ("Enter".into(), "Enter".into(), 13, Some("\r".into())));
        assert_eq!(key_of("a", false), ("a".into(), "KeyA".into(), 65, Some("a".into())));
        assert_eq!(key_of("a", true), ("A".into(), "KeyA".into(), 65, Some("A".into())));
        assert_eq!(key_of("A", false), ("A".into(), "KeyA".into(), 65, Some("A".into())));
        assert_eq!(key_of("7", false), ("7".into(), "Digit7".into(), 55, Some("7".into())));
        assert_eq!(key_of("f5", false), ("F5".into(), "F5".into(), 116, None));
        use crate::parse::known_key;
        assert!(known_key("cmd-s") && known_key("shift-tab") && known_key("ctrl+a"));
        assert!(!known_key("hyper-x") && !known_key("enterr") && !known_key(""));
    }

    #[test]
    fn the_seeded_random_numbers() {
        assert!(seeded_random(7).starts_with("(() => { let x = 7 >>> 0 || 1;"));
    }
}
