// The playground: ritsu itself, compiled to wasm32 (crates/ritsu-wasm), running in this page.
//
// Nothing is sent anywhere. The project's files are held here, edited in tabs, and every request
// hands all of them to the module in this page, which runs the command on them as it would run in
// a directory holding the same files: `ritsu check .` over the whole project, and a language's
// generator and page on the file that is open. The projects the page opens come from
// projects.json, written from website/playground/ by crates/ritsu/tests/playground.rs.
//
// One convention crosses the boundary, the one rulec's and dandori's pages keep: every buffer
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
    projects: { shop: "A small shop", "shop.ja": "A small shop, in Japanese" },
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
    noDoc: (tool) => `${tool} has no page for whoever approves a file.`,
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
    projects: { shop: "小さな通販（英語）", "shop.ja": "小さな通販（日本語）" },
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
    noDoc: (tool) => `${tool} には、ファイルを承認する人に見せるページがありません。`,
    noLanguage: "このファイルは、どの言語のものでもありません。このファイルを名指すほかのファイルが読みます。",
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
  [".req", "yuen"],
  [".ctx", "sakai"],
];
const kind = (p) => (KINDS.find(([ext]) => p.endsWith(ext)) || [])[1] || null;

function start(root, bundle) {
  const lang = root.dataset.lang === "ja" ? "ja" : "en";
  const t = TEXT[lang];
  const projects = bundle.projects;
  const chooser = root.querySelector(".pg-project");
  const add = root.querySelector(".pg-add");
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
  const kept = new Map(); // what the reader has made of each project while the page is open

  add.textContent = t.add;
  remove.textContent = t.remove;
  revert.textContent = t.revert;
  chooser.replaceChildren(
    ...projects.map((p, i) => Object.assign(document.createElement("option"), { value: String(i), textContent: t.projects[p.name] || p.name }))
  );

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
  }

  function choose(i) {
    project = projects[i];
    chooser.value = String(i);
    const k = kept.get(project.name);
    files = k ? k.files.map((f) => f.slice()) : project.files.map((f) => f.slice());
    marks = new Map();
    show(k ? k.open : project.open);
    keep();
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
      else if (view === "gen") a = ask("gen", target.value ? { target: target.value } : {});
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
    run();
  });
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

  // A link can open the page on a project, a file, a view and a target:
  // #project=shop&file=ordering/ship_order.flow&view=gen&target=asl. The headings' own anchors name
  // none of them, and leave the page as it is.
  function follow() {
    const h = {};
    for (const kv of location.hash.replace(/^#/, "").split("&")) {
      const [k, v] = kv.split("=");
      if (k) h[k] = decodeURIComponent(v || "");
    }
    const i = projects.findIndex((p) => p.name === h.project);
    if (i < 0 && !h.file && !h.view) return false;
    if (i >= 0) choose(i);
    if (h.file && files.some(([p]) => p === h.file)) show(h.file);
    if (["check", "gen", "doc"].includes(h.view)) switchTo(h.view);
    if (h.target) {
      target.replaceChildren(Object.assign(document.createElement("option"), { value: h.target, textContent: h.target }));
      target.value = h.target;
    }
    return true;
  }
  window.addEventListener("hashchange", () => {
    if (!follow()) return;
    run();
    root.scrollIntoView({ block: "start" });
  });

  choose(0);
  follow();
  run();
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
