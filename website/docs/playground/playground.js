// The playground: ritsu itself, compiled to wasm32 (crates/ritsu-wasm), running in this page.
//
// Nothing is sent anywhere. The project's files are held here, edited in tabs, and every request
// hands all of them to the module in this page, which runs the command on them as it would run in
// a directory holding the same files: `ritsu check .` over the whole project, and a language's
// generator and page on the file that is open. The projects the page opens come from
// projects.json, written by crates/ritsu/tests/playground.rs from the files they are made of: the
// shop (website/playground/), the examples of the playground rulec's site had (one rule each),
// those of dandori's (each flow with the files it reads), and sekisho's example (its gate with the
// files it reads). Each page lists the projects of its language.
//
// A link opens the page on a project as the reader made it: the project, the file, the view, the
// target, and what the reader changed, packed (see `pack`). The page reads the links dandori's
// playground gave too (#flow=…): its page sends them on here, as rulec's sends its readers on to
// rulec's first example.
//
// One convention crosses the boundary, the one rulec's and dandori's playgrounds kept: every buffer
// begins with its own length as a little-endian u32. `put` writes one, `take` reads one and frees
// it.

const TEXT = {
  en: {
    booting: "loading ritsu…",
    failed: "could not load ritsu",
    broken: "ritsu stopped on this input and was loaded again; please report the project that did it",
    status: (ms, ...what) => [`ritsu ${VERSION}`, `${ms} ms`, ...what].join(" · "),
    counts: (e, w) => (e || w ? `${e} error${e === 1 ? "" : "s"}, ${w} warning${w === 1 ? "" : "s"}` : "all pass"),
    files: (n) => `${n} file${n === 1 ? "" : "s"}`,
    projects: { shop: "A small shop", "shop.ja": "A small shop, in Japanese", empty: "An empty project", "sekisho/refunds": "Who may refund an order of a shop" },
    groups: { ritsu: "ritsu: every language in one project", own: "Start from one file", rulec: "rulec: one rule", dandori: "dandori: a flow and the files it reads", sekisho: "sekisho: a gate and the files it reads" },
    rules: { gap: "The table with a row missing", full: "The whole table", multi: "Tables in stages", big: "A bigger rule", walk: "Walking a list" },
    draft: "A first draft of the hotel booking, with errors",
    and: " · ",
    examples: { hotel: "Hotel booking", order: "Order", fulfillment: "Fulfillment", inquiry: "Inquiry", review: "Review", invoice: "Invoice" },
    versions: { temporal: "for Temporal", aws: "for AWS", "pydantic-graph": "for pydantic-graph", argo: "for Argo Workflows" },
    everywhere: "for every platform",
    beside: (name) => `${name}, the child flow, for every platform`,
    empty: "This project has no file yet. Press “add a file”, give the file a path that ends in the extension of its language (fee.rule, order.flow, days.cal, stock.book, needs.req, shop.ctx, refunds.gate, greeter.geas, order.proto), and paste the file into its tab. A file it reads, such as a rule a flow calls, is one more file of the project, at the path the first one names.",
    share: "copy a link",
    shared: "a link to this is copied, and in the address bar",
    sharedHere: "a link to this is in the address bar",
    unshared: "this browser cannot pack the edits into a link",
    unread: "the link's edits cannot be read",
    add: "add a file",
    addAsk: "The new file's path, from the project's root (billing/rules/fee.rule, say):",
    addBad: (p) => `${p} is not a path inside the project`,
    addTaken: (p) => `${p} is in the project already`,
    remove: "remove this file",
    removeAsk: (p) => `Remove ${p} from the project? The project as it opens comes back with "undo my edits".`,
    revert: "undo my edits",
    command: "$ ",
    noFile: "The project has no file. Add one.",
    noGen: (tool) => `${tool} generates nothing: ${TEXT.en.gen[tool] || ""}.`,
    noDoc: (tool) => `${tool} has no page for people to read.`,
    noLanguage: "This file is of no language. A file of the project that names it reads it.",
    refusedGen: "Nothing is written; this is what the command says:",
    refusedDoc: "No page is drawn; this is what the command says:",
    written: "written",
    notWritten: "nothing written",
    drawn: "drawn",
    notDrawn: "no page",
    board: (cmd) => ["The page ", cmd, " writes is laid out for a whole window, so it opens in a tab of its own. Edit the file, and the link below opens the new one."],
    open: "open the page →",
    markdown: "The Markdown it writes for a pull request",
    gen: {
      geas: "it holds the code it names to the claims a person has read, by running it, which a page cannot",
      proto: "a .proto is a contract the languages read",
    },
    targets: {
      asl: "Step Functions (ASL)",
      temporal: "Temporal (TypeScript)",
      "temporal-python": "Temporal (Python)",
      "temporal-go": "Temporal (Go)",
      durable: "Lambda durable functions",
      argo: "Argo Workflows",
      "pydantic-graph": "pydantic-graph",
      reqif: "ReqIF",
      prov: "W3C PROV",
    },
  },
  ja: {
    booting: "ritsu を読み込んでいます…",
    failed: "ritsu を読み込めませんでした",
    broken: "この入力で ritsu が止まったので、読み込み直しました。このプロジェクトを報告してください",
    status: (ms, ...what) => [`ritsu ${VERSION}`, `${ms} ms`, ...what].join(" ・ "),
    counts: (e, w) => (e || w ? `エラー ${e} 件、警告 ${w} 件` : "どれも検査を通りました"),
    files: (n) => `${n} ファイル`,
    projects: { shop: "小さな通販（英語）", "shop.ja": "小さな通販（日本語）", empty: "空のプロジェクト", "sekisho/refunds.ja": "店の注文を返金してよい人" },
    groups: { ritsu: "ritsu（全部の言語がそろうプロジェクト）", own: "ファイル一つから始める", rulec: "rulec（規則一つ）", dandori: "dandori（フローと、フローが読むファイル）", sekisho: "sekisho（ゲートと、ゲートが読むファイル）" },
    rules: { gap: "一行足りない表", full: "そろった表", multi: "表をつなぐ", big: "大きい規則", walk: "並びを歩く" },
    draft: "ホテルの予約の最初の下書き（エラーあり）",
    and: "・",
    examples: { hotel: "ホテルの予約", order: "注文", fulfillment: "引当と発送", inquiry: "問い合わせ", review: "審査", invoice: "請求" },
    versions: { temporal: "Temporal 版", aws: "AWS 版", "pydantic-graph": "pydantic-graph 版", argo: "Argo Workflows 版" },
    everywhere: "どのプラットフォームでもそのまま動く版",
    beside: (name) => `${name}（子のフロー。どのプラットフォームでもそのまま動く）`,
    empty: "このプロジェクトには、まだファイルがありません。「ファイルを足す」を押して、言語の拡張子で終わるパス（fee.rule、order.flow、days.cal、stock.book、needs.req、shop.ctx、refunds.gate、greeter.geas、order.proto）を付け、開いたタブにファイルの中身を貼り付けてください。フローが呼ぶ規則のように、そのファイルが読むファイルは、そこに書いてあるパスに、もう一つのファイルとして足してください。",
    share: "リンクをコピー",
    shared: "この状態へのリンクをコピーしました。アドレスバーにも入っています",
    sharedHere: "この状態へのリンクをアドレスバーに入れました",
    unshared: "このブラウザでは、編集をリンクに詰められません",
    unread: "リンクに詰めた編集を読めません",
    add: "ファイルを足す",
    addAsk: "新しいファイルのパス（プロジェクトのルートから。たとえば billing/rules/手数料.rule）:",
    addBad: (p) => `${p} は、プロジェクトの中のパスではありません`,
    addTaken: (p) => `${p} はもうプロジェクトにあります`,
    remove: "このファイルを消す",
    removeAsk: (p) => `${p} をプロジェクトから消しますか？「編集を取り消す」で、開いたときのプロジェクトに戻ります。`,
    revert: "編集を取り消す",
    command: "$ ",
    noFile: "プロジェクトにファイルがありません。足してください。",
    noGen: (tool) => `${tool} は何も生成しません。${TEXT.ja.gen[tool] || ""}。`,
    noDoc: (tool) => `${tool} には、人が読むページがありません。`,
    noLanguage: "このファイルは、どの言語のものでもありません。このファイルを参照するほかのファイルが読みます。",
    refusedGen: "何も書きません。コマンドはこう言います。",
    refusedDoc: "ページを描きません。コマンドはこう言います。",
    written: "書きました",
    notWritten: "書いていません",
    drawn: "描きました",
    notDrawn: "ページなし",
    board: (cmd) => ["", cmd, " が書くページはウィンドウ全体を使うので、別のタブで開きます。ファイルを直すと、下のリンクで開くのもその新しいほうになります。"],
    open: "ページを開く →",
    markdown: "プルリクエスト向けに書く Markdown",
    gen: {
      geas: "geas は、人が読んだ主張にコードを従わせる言語で、コードを走らせて確かめます。ページの中では走らせられません",
      proto: ".proto は、言語どうしが読む契約です",
    },
    targets: {
      asl: "Step Functions（ASL）",
      temporal: "Temporal（TypeScript）",
      "temporal-python": "Temporal（Python）",
      "temporal-go": "Temporal（Go）",
      durable: "Lambda durable functions",
      argo: "Argo Workflows",
      "pydantic-graph": "pydantic-graph",
      reqif: "ReqIF",
      prov: "W3C PROV",
    },
  },
};

// The module and the projects sit beside this script, so they are found from its own URL rather
// than the page's: the same files are loaded by the page in each language.
const HERE = document.currentScript.src;
const WASM = new URL("ritsu.wasm", HERE).href;
const PROJECTS = new URL("projects.json", HERE).href;
document.head.append(
  Object.assign(document.createElement("link"), { rel: "stylesheet", href: new URL("playground.css", HERE).href })
);

let VERSION = "";
let mod = null; // the compiled module, kept so that a trapped instance can be replaced
let inst = null;

async function boot() {
  if (!mod) {
    mod = await WebAssembly.compileStreaming(fetch(WASM)).catch(async () =>
      WebAssembly.compile(await (await fetch(WASM)).arrayBuffer())
    );
  }
  inst = await WebAssembly.instantiate(mod, {});
  VERSION = take(inst.exports.ritsu_version());
}

function put(str) {
  const bytes = new TextEncoder().encode(str);
  const ptr = inst.exports.ritsu_alloc(bytes.length);
  new Uint8Array(inst.exports.memory.buffer, ptr + 4, bytes.length).set(bytes);
  return ptr;
}

function take(ptr) {
  // Read the views after the call: allocating may have grown the memory, which detaches every
  // view taken before it.
  const len = new DataView(inst.exports.memory.buffer).getUint32(ptr, true);
  const bytes = new Uint8Array(inst.exports.memory.buffer, ptr + 4, len).slice();
  inst.exports.ritsu_free(ptr);
  return new TextDecoder().decode(bytes);
}

// One call: hand over the request, read the answer, release both buffers.
function call(fn, text) {
  const ptr = put(text);
  try {
    return take(inst.exports[fn](ptr));
  } finally {
    inst.exports.ritsu_free(ptr);
  }
}

// The language of a file, by its extension, as ritsu sorts a project's files.
const KINDS = [
  [".rule", "rulec"],
  [".cal", "koyomi"],
  [".book", "chobo"],
  [".geas", "geas"],
  [".proto", "proto"],
  [".flow", "dandori"],
  [".gate", "sekisho"],
  [".req", "yuen"],
  [".ctx", "sakai"],
];
const kind = (p) => (KINDS.find(([ext]) => p.endsWith(ext)) || [])[1] || null;

// What the reader picks a flow of dandori's examples by: which example, and which version of it.
function flowLabel(t, path) {
  if (path.startsWith("tests/")) return t.draft;
  const [, ex, dir, file] = path.split("/");
  const name = t.examples[ex] || ex;
  if (file === undefined) {
    // a flow beside the versions: the example itself when it has the example's name, else its child
    const stem = dir.replace(/(\.ja)?\.flow$/, "");
    return name + t.and + (stem === ex ? t.everywhere : t.beside(stem));
  }
  return name + t.and + (t.versions[dir] || dir);
}

// What the list calls a project.
function title(t, p) {
  if (t.projects[p.name]) return t.projects[p.name];
  if (p.group === "rulec") return t.rules[p.name.replace(/^rulec\//, "").replace(/\.ja$/, "")] || p.name;
  if (p.group === "dandori") return flowLabel(t, p.open);
  return p.name;
}

// What the reader changed of a project, as a link carries it: every file changed or added, with
// its text, and every file removed, with null, by its path, in the order of the tabs.
function changes(base, now) {
  const was = new Map(base);
  const out = now.filter(([p, text]) => was.get(p) !== text).map(([p, text]) => [p, text]);
  for (const [p] of base) if (!now.some(([q]) => q === p)) out.push([p, null]);
  return out;
}

// A path a reader may give a file: inside the project, with no empty, `.` or `..` part.
const inside = (p) => typeof p === "string" && p !== "" && !p.startsWith("/") && !p.split("/").some((s) => s === ".." || s === "." || s === "");

// The project as it opens, with the changes of a link made to it.
function changed(base, edits) {
  const files = base.map((f) => f.slice());
  for (const [p, text] of edits) {
    if (!inside(p) || !(text === null || typeof text === "string")) throw new Error(p);
    const at = files.findIndex(([q]) => q === p);
    if (text === null) {
      if (at >= 0) files.splice(at, 1);
    } else if (at >= 0) files[at][1] = text;
    else files.push([p, text]);
  }
  return files;
}

// The changes as a link carries them: their JSON, compressed as raw DEFLATE (RFC 1951) by the
// browser's CompressionStream, in base64url with no padding. `unpack` reads one back.
async function pack(value) {
  const stream = new Blob([JSON.stringify(value)]).stream().pipeThrough(new CompressionStream("deflate-raw"));
  const bytes = new Uint8Array(await new Response(stream).arrayBuffer());
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

async function unpack(text) {
  const bin = atob(text.replace(/-/g, "+").replace(/_/g, "/"));
  const stream = new Blob([Uint8Array.from(bin, (c) => c.charCodeAt(0))]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
  return JSON.parse(await new Response(stream).text());
}

function start(root, bundle) {
  const lang = root.dataset.lang === "ja" ? "ja" : "en";
  const t = TEXT[lang];
  // every project with its files' texts; the page lists those of its language, and opens another's
  // when a link names it
  const all = bundle.projects.map((p) => ({ ...p, files: p.files.map(([path, from]) => [path, bundle.texts[from]]) }));
  const projects = all.filter((p) => !p.lang || p.lang === lang);
  const chooser = root.querySelector(".pg-project");
  const add = root.querySelector(".pg-add");
  const share = root.querySelector(".pg-share");
  const remove = root.querySelector(".pg-remove");
  const revert = root.querySelector(".pg-revert");
  const status = root.querySelector(".pg-status");
  const tabs = root.querySelector(".pg-files");
  const src = root.querySelector(".pg-src");
  const out = root.querySelector(".pg-out");
  const target = root.querySelector(".pg-target");
  const picker = root.querySelector(".pg-picker");
  let view = "check";
  let project = projects[0];
  let files = []; // [path, text], in the order the tabs show them
  let open = null; // the path of the file in the box
  let marks = new Map(); // path -> "err" | "warn", from the last check
  let written = [];
  let timer = null;
  let targets = new Map(); // language -> the target its generator is asked for in this project
  let unread = false; // the edits of the link the page opened on could not be read
  const kept = new Map(); // what the reader has made of each project while the page is open

  add.textContent = t.add;
  remove.textContent = t.remove;
  revert.textContent = t.revert;
  if (share) share.textContent = t.share;

  // The list, in the groups of the projects, in the order projects.json has them.
  function drawChooser() {
    const groups = new Map();
    projects.forEach((p, i) => {
      if (!groups.has(p.group)) groups.set(p.group, Object.assign(document.createElement("optgroup"), { label: t.groups[p.group] || p.group }));
      const o = Object.assign(document.createElement("option"), { value: String(i), textContent: title(t, p) });
      o.dataset.name = p.name;
      groups.get(p.group).append(o);
    });
    chooser.replaceChildren(...groups.values());
  }
  drawChooser();

  const pre = (text, links) => {
    const el = document.createElement("pre");
    el.className = "pg-pre";
    if (!links || !files.length) {
      el.textContent = text;
      return el;
    }
    // Where a finding is, as `path:line` or `path:line:col`, takes the reader to that line of
    // that file. yuen writes the path from where it runs, with `./` before it.
    const names = files.map(([p]) => p).sort((a, b) => b.length - a.length);
    const at = new RegExp("(?:\\./)?(" + names.map((p) => p.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|") + "):(\\d+)(?::(\\d+))?", "g");
    let last = 0;
    for (const m of text.matchAll(at)) {
      el.append(text.slice(last, m.index));
      const b = Object.assign(document.createElement("button"), { type: "button", className: "pg-at", textContent: m[0] });
      b.addEventListener("click", () => go(m[1], Number(m[2])));
      el.append(b);
      last = m.index + m[0].length;
    }
    el.append(text.slice(last));
    return el;
  };
  const para = (text) => Object.assign(document.createElement("p"), { className: "pg-note", textContent: text });
  const said = (a) => pre(t.command + a.command + "\n" + a.out + a.err, true);

  function go(path, line) {
    show(path);
    const lines = src.value.split("\n");
    let from = 0;
    for (let i = 0; i < line - 1 && i < lines.length; i++) from += lines[i].length + 1;
    src.focus();
    src.setSelectionRange(from, from + (lines[line - 1] || "").length);
    const height = parseFloat(getComputedStyle(src).lineHeight) || 16;
    src.scrollTop = Math.max(0, (line - 4) * height);
    if (view !== "check") run();
  }

  // The tabs of the files, each marked with the worst the last check said of it.
  function drawTabs() {
    tabs.replaceChildren(
      ...files.map(([p]) => {
        const b = Object.assign(document.createElement("button"), { type: "button", className: "pg-file", textContent: p });
        b.setAttribute("role", "tab");
        b.setAttribute("aria-selected", String(p === open));
        if (p === open) b.classList.add("on");
        if (marks.has(p)) b.classList.add(marks.get(p));
        if (!kind(p)) b.classList.add("pg-other");
        b.addEventListener("click", () => {
          show(p);
          if (view !== "check") run();
        });
        return b;
      })
    );
  }

  function show(path) {
    open = path;
    const f = files.find(([p]) => p === path);
    src.value = f ? f[1] : "";
    src.disabled = !f;
    picker.dataset.path = "";
    drawTabs();
  }

  function edited() {
    const was = JSON.stringify(project.files);
    return JSON.stringify(files) !== was;
  }

  function keep() {
    kept.set(project.name, { files: files.map((f) => f.slice()), open });
    revert.hidden = !edited();
    remove.disabled = !open;
    // an empty project points at the one way to put a file into it
    add.classList.toggle("pg-cta", !files.length);
  }

  function choose(i) {
    project = projects[i];
    chooser.value = String(i);
    const k = kept.get(project.name);
    files = k ? k.files.map((f) => f.slice()) : project.files.map((f) => f.slice());
    marks = new Map();
    targets = new Map(project.target ? [["dandori", project.target]] : []);
    target.replaceChildren();
    show(k ? k.open : project.open);
    keep();
  }

  // A project by its name: one this page lists, or another page's, which the list then takes in.
  function named(name) {
    let i = projects.findIndex((p) => p.name === name);
    if (i < 0) {
      const p = all.find((q) => q.name === name);
      if (!p) return -1;
      projects.push(p);
      drawChooser();
      i = projects.length - 1;
    }
    return i;
  }

  // The page `doc` writes, as it stands. A link, not `window.open`: a scripted pop-up is blocked
  // often enough to be unreliable, and a link the reader clicks is a plain navigation. What it
  // points at is a small page holding the one `doc` wrote in a sandboxed frame, whose script runs
  // with no access to this site.
  let pageUrls = [];
  function forget() {
    for (const u of pageUrls) URL.revokeObjectURL(u);
    pageUrls = [];
  }
  function pageLink(html) {
    const title = (html.match(/<title>([^<]*)<\/title>/) || [, "ritsu"])[1];
    const host =
      '<!doctype html><meta charset="utf-8"><title>' + title + "</title>" +
      "<style>html,body{margin:0;height:100%}iframe{display:block;border:0;width:100%;height:100%}</style>" +
      '<iframe sandbox="allow-scripts" srcdoc="' + html.replace(/&/g, "&amp;").replace(/"/g, "&quot;") + '"></iframe>';
    const url = URL.createObjectURL(new Blob([host], { type: "text/html" }));
    pageUrls.push(url);
    return url;
  }
  const opener = (href, text) =>
    Object.assign(document.createElement("a"), { className: "pg-open", href, target: "_blank", rel: "noopener", textContent: text });

  // A link to the project as it stands: its name, the file, the view, the target, and what the
  // reader changed. It goes into the address bar, and onto the clipboard where the browser lets it.
  async function link() {
    const h = [["project", project.name]];
    if (open && open !== project.open) h.push(["file", open]);
    if (view !== "check") h.push(["view", view]);
    if (view === "gen" && !target.hidden && target.value) h.push(["target", target.value]);
    const c = changes(project.files, files);
    if (c.length) h.push(["edits", await pack(c)]);
    return "#" + h.map(([k, v]) => k + "=" + encodeURIComponent(v)).join("&");
  }

  function ask(what, extra) {
    const request = { files: Object.fromEntries(files), lang, path: open || "", ...extra };
    return JSON.parse(call("ritsu_" + what, JSON.stringify(request)));
  }

  function run() {
    if (!inst) return;
    const started = performance.now();
    let a;
    try {
      if (view === "check") a = ask("check", {});
      else if (view === "gen") a = ask("gen", targets.has(kind(open || "")) ? { target: targets.get(kind(open || "")) } : {});
      else a = ask("doc", {});
    } catch (e) {
      // A trap leaves the instance unusable; the module is still good, so take a new one. The
      // same project would stop it again, so the next edit is what runs on it.
      status.textContent = t.broken;
      inst = null;
      boot().catch(() => (status.textContent = t.failed));
      return;
    }
    const ms = Math.round(performance.now() - started);
    target.hidden = true;
    picker.hidden = true;
    forget();
    if (a.error) {
      status.textContent = t.status(ms, a.error);
      out.replaceChildren();
      return;
    }
    if (!files.length) {
      // nothing to check yet: what to do, and what the command says of an empty directory
      drawTabs();
      status.textContent = t.status(ms);
      out.replaceChildren(para(t.empty), ...(view === "check" ? [said(a)] : []));
      return;
    }
    if (view === "check") {
      const d = (a.json && a.json.diagnostics) || [];
      const errors = d.filter((x) => x.severity === "error").length;
      const warnings = d.filter((x) => x.severity === "warning").length;
      marks = new Map();
      for (const x of d) {
        const f = String(x.file || "").replace(/^\.\//, "");
        if (x.severity === "error") marks.set(f, "err");
        else if (x.severity === "warning" && marks.get(f) !== "err") marks.set(f, "warn");
      }
      for (const f of (a.json && a.json.files) || []) if (!f.ok && !marks.has(f.file)) marks.set(f.file, "err");
      drawTabs();
      status.textContent = t.status(ms, t.counts(errors, warnings));
      out.replaceChildren(said(a));
      return;
    }
    const tool = a.tool || kind(open || "");
    if (!open) {
      status.textContent = t.status(ms);
      out.replaceChildren(para(t.noFile));
      return;
    }
    if (a.none) {
      status.textContent = t.status(ms);
      out.replaceChildren(para(tool ? (view === "gen" ? t.noGen(tool) : t.noDoc(tool)) : t.noLanguage));
      return;
    }
    if (view === "gen") {
      if (a.targets && a.targets.length) {
        target.replaceChildren(
          ...a.targets.map((k) => Object.assign(document.createElement("option"), { value: k, textContent: t.targets[k] || k }))
        );
        target.value = a.target;
        target.hidden = false;
      }
      if (a.code !== 0) {
        status.textContent = t.status(ms, t.notWritten);
        out.replaceChildren(para(t.refusedGen), said(a));
        return;
      }
      // A generator that prints what it writes (`sakai export cml`, `yuen export`) writes no file.
      written = a.files.length ? a.files : [{ path: a.command, body: a.out }];
      picker.replaceChildren(
        ...written.map((f, i) => Object.assign(document.createElement("option"), { value: String(i), textContent: f.path }))
      );
      picker.hidden = written.length < 2;
      const keepAt = written.findIndex((f) => f.path === picker.dataset.path);
      picker.value = String(keepAt < 0 ? 0 : keepAt);
      status.textContent = t.status(ms, a.files.length ? t.files(a.files.length) : t.written);
      out.replaceChildren(pre(written[picker.value].body, false));
      return;
    }
    const page = a.html;
    if (page.code !== 0) {
      status.textContent = t.status(ms, t.notDrawn);
      out.replaceChildren(para(t.refusedDoc), said(page));
      return;
    }
    status.textContent = t.status(ms, t.drawn);
    const box = Object.assign(document.createElement("div"), { className: "pg-board" });
    const p = document.createElement("p");
    const [before, cmd, after] = t.board(page.command.replace(/^ritsu /, ""));
    p.append(before, Object.assign(document.createElement("code"), { textContent: cmd }), after);
    box.append(p, opener(pageLink(page.out), t.open));
    if (page.err) box.append(pre(page.err, true));
    const md = document.createElement("details");
    md.className = "pg-md";
    md.append(Object.assign(document.createElement("summary"), { textContent: t.markdown }), pre(a.markdown.out, false));
    out.replaceChildren(box, md);
  }

  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(run, 300);
  };

  src.addEventListener("input", () => {
    const f = files.find(([p]) => p === open);
    if (!f) return;
    f[1] = src.value;
    keep();
    later();
  });
  chooser.addEventListener("change", () => {
    choose(Number(chooser.value));
    run();
  });
  add.addEventListener("click", () => {
    const asked = (window.prompt(t.addAsk) || "").trim().replace(/^\.\//, "");
    if (!asked) return;
    if (asked.startsWith("/") || asked.split("/").some((s) => s === ".." || s === "." || s === "")) {
      window.alert(t.addBad(asked));
      return;
    }
    if (files.some(([p]) => p === asked)) {
      window.alert(t.addTaken(asked));
      return;
    }
    files.push([asked, ""]);
    show(asked);
    keep();
    src.focus();
    run();
  });
  remove.addEventListener("click", () => {
    if (!open || !window.confirm(t.removeAsk(open))) return;
    const at = files.findIndex(([p]) => p === open);
    files.splice(at, 1);
    show(files.length ? files[Math.min(at, files.length - 1)][0] : null);
    keep();
    run();
  });
  revert.addEventListener("click", () => {
    kept.delete(project.name);
    choose(Number(chooser.value));
    run();
  });
  target.addEventListener("change", () => {
    picker.dataset.path = "";
    targets.set(kind(open || ""), target.value);
    run();
  });
  if (share) {
    share.addEventListener("click", async () => {
      let h;
      try {
        h = await link();
      } catch (e) {
        status.textContent = t.unshared;
        return;
      }
      history.replaceState(null, "", h);
      try {
        await navigator.clipboard.writeText(location.href);
        status.textContent = t.shared;
      } catch (e) {
        status.textContent = t.sharedHere;
      }
    });
  }
  picker.addEventListener("change", () => {
    picker.dataset.path = written[picker.value].path;
    out.replaceChildren(pre(written[picker.value].body, false));
  });
  const switchTo = (v) => {
    view = v;
    for (const o of root.querySelectorAll("[data-view]")) o.classList.toggle("on", o.dataset.view === v);
  };
  for (const b of root.querySelectorAll("[data-view]")) {
    b.addEventListener("click", () => {
      switchTo(b.dataset.view);
      run();
    });
  }

  // A link can open the page on a project, a file, a view and a target, with what the reader
  // changed: #project=shop&file=ordering/ship_order.flow&view=gen&target=asl&edits=…. It reads the
  // links dandori's page gave as well: #flow=examples/hotel/temporal/hotel.flow&view=build&target=asl
  // opens the project of that flow (build is the generator; rules, the page of its first rule). The
  // headings' own anchors name none of them, and leave the page as it is.
  async function follow() {
    const h = {};
    for (const kv of location.hash.replace(/^#/, "").split("&")) {
      const at = kv.indexOf("=");
      const [k, v] = at < 0 ? [kv, ""] : [kv.slice(0, at), kv.slice(at + 1)];
      try {
        if (k) h[k] = decodeURIComponent(v);
      } catch (e) {
        h[k] = v;
      }
    }
    let i = h.project ? named(h.project) : -1;
    if (i < 0 && h.flow) {
      const p = all.find((q) => q.group === "dandori" && q.open === h.flow);
      if (p) i = named(p.name);
      h.view = { build: "gen", rules: "rules" }[h.view] || h.view;
    }
    if (i < 0 && !h.file && !h.view && !h.target) return false;
    if (i >= 0) choose(i);
    if (i >= 0 && h.edits) {
      try {
        files = changed(project.files, await unpack(h.edits));
      } catch (e) {
        unread = true;
      }
      show(files.some(([p]) => p === project.open) ? project.open : files.length ? files[0][0] : null);
      keep();
    }
    if (h.file && files.some(([p]) => p === h.file)) show(h.file);
    if (h.view === "rules") {
      // dandori's tab of the rules a flow calls: here, each rule is a file of the project
      const rule = files.find(([p]) => kind(p) === "rulec");
      if (rule) show(rule[0]);
      h.view = rule ? "doc" : "check";
    }
    if (["check", "gen", "doc"].includes(h.view)) switchTo(h.view);
    if (h.target) targets.set(kind(open || ""), h.target);
    keep();
    return true;
  }
  // run, and say after what it found that a link's edits could not be read
  const runFollowed = () => {
    run();
    if (unread) status.textContent = [status.textContent, t.unread].join(t.and);
    unread = false;
  };
  window.addEventListener("hashchange", () => {
    follow().then((did) => {
      if (!did) return;
      runFollowed();
      root.scrollIntoView({ block: "start" });
    });
  });

  choose(0);
  follow().finally(runFollowed);
}

const roots = document.querySelectorAll(".pg");
for (const root of roots) root.querySelector(".pg-status").textContent = TEXT[root.dataset.lang === "ja" ? "ja" : "en"].booting;
Promise.all([boot(), fetch(PROJECTS).then((r) => r.json())])
  .then(([, bundle]) => {
    for (const root of roots) start(root, bundle);
  })
  .catch((e) => {
    for (const root of roots) root.querySelector(".pg-status").textContent = `${TEXT[root.dataset.lang === "ja" ? "ja" : "en"].failed}: ${e.message}`;
  });
