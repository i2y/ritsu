// The playground: dandori itself, compiled to wasm32 (src/wasm.rs), running in this page.
//
// Nothing is sent anywhere. The flow typed here goes to the module in this page, and the answers
// come back from it. What a flow reads besides itself (its rules, the descriptions of the APIs it
// calls, a child flow) comes from presets.json: the files of the examples, and what rulec printed
// for their rules, recorded from the repository by tests/playground.rs. A page can run no rulec,
// so a flow here calls the examples' rules as they are.
//
// One convention crosses the boundary, the one rulec's playground keeps: every buffer begins
// with its own length as a little-endian u32. `put` writes one, `take` reads one and frees it.

const TEXT = {
  en: {
    booting: "loading dandori…",
    failed: "could not load dandori",
    broken: "dandori stopped on this input and was loaded again; please report the flow that did it",
    status: (ms, ...what) => [`dandori ${VERSION}`, `${ms} ms`, ...what].join(" · "),
    passes: (w) => (w ? `passes, ${w} warning${w === 1 ? "" : "s"}` : "passes"),
    fails: (e, w) => `${e} error${e === 1 ? "" : "s"}, ${w} warning${w === 1 ? "" : "s"}`,
    files: (n) => `${n} files`,
    notBuilt: "not built",
    drawn: "drawn",
    notDrawn: "nothing to draw",
    refusedCheck: "Nothing is built from a flow that does not pass check:",
    refusedTarget: (p) => `The flow asks for what ${p} cannot do, so nothing is built for it:`,
    undrawable: "A flow whose names and types do not resolve has nothing to draw:",
    board: ["The page ", "dandori doc --format html", " writes is laid out for a whole window, so it opens in a tab of its own. Edit the flow, and the link below opens the new one."],
    open: "open the page →",
    markdown: "The Markdown it writes for a pull request",
    rules: (n) => `${n} rule${n === 1 ? "" : "s"}`,
    rulesNote: "The rules this flow calls, as the examples have them; they cannot be changed here. The page rulec doc renders for whoever approves a rule opens from its name, and a case can be tried on it.",
    noRules: "This flow calls no rule.",
    unresolved: "The rules show once the flow's names and types resolve:",
    openRule: "open the rule's page →",
    revert: "undo my edits",
    draft: "A first draft of the hotel booking, with errors",
    and: " · ",
    examples: { hotel: "Hotel booking", order: "Order", fulfillment: "Fulfillment", inquiry: "Inquiry", review: "Review" },
    versions: { temporal: "for Temporal", aws: "for AWS", "pydantic-graph": "for pydantic-graph", argo: "for Argo Workflows" },
    beside: (name) => `${name}, the child flow, for every platform`,
    targets: {
      asl: "Step Functions (ASL)",
      temporal: "Temporal (TypeScript)",
      "temporal-python": "Temporal (Python)",
      "temporal-go": "Temporal (Go)",
      durable: "Lambda durable functions",
      argo: "Argo Workflows",
      "pydantic-graph": "pydantic-graph",
    },
  },
  ja: {
    booting: "dandori を読み込んでいます…",
    failed: "dandori を読み込めませんでした",
    broken: "この入力で dandori が止まったので、読み込み直しました。このフローを報告してください",
    status: (ms, ...what) => [`dandori ${VERSION}`, `${ms} ms`, ...what].join(" ・ "),
    passes: (w) => (w ? `検査を通りました（警告 ${w} 件）` : "検査を通りました"),
    fails: (e, w) => `エラー ${e} 件、警告 ${w} 件`,
    files: (n) => `${n} ファイル`,
    notBuilt: "ビルドしません",
    drawn: "図にしました",
    notDrawn: "図にできません",
    refusedCheck: "検査を通らないフローからは、何もビルドしません。",
    refusedTarget: (p) => `このフローには ${p} でできないことがあるので、ビルドしません。`,
    undrawable: "名前や型が解決できないフローは、図にできません。",
    board: ["", "dandori doc --format html", " が書くページはウィンドウ全体を使うので、別のタブで開きます。フローを直すと、下のリンクで開くのもその新しいほうになります。"],
    open: "ページを開く →",
    markdown: "プルリクエスト向けに書く Markdown",
    rules: (n) => `規則 ${n} 件`,
    rulesNote: "このフローが呼ぶ規則です。例にあるままで、ここでは書き換えられません。rulec doc が承認する人向けに描いたページを開くと、ケースを打って試せます。",
    noRules: "このフローは規則を呼びません。",
    unresolved: "名前や型が解決できるようになると、規則が出ます。",
    openRule: "規則のページを開く →",
    revert: "編集を取り消す",
    draft: "ホテルの予約の最初の下書き（エラーあり）",
    and: "・",
    examples: { hotel: "ホテルの予約", order: "注文", fulfillment: "引当と発送", inquiry: "問い合わせ", review: "審査" },
    versions: { temporal: "Temporal 版", aws: "AWS 版", "pydantic-graph": "pydantic-graph 版", argo: "Argo Workflows 版" },
    beside: (name) => `${name}（子のフロー。どのプラットフォームでもそのまま動く）`,
    targets: {
      asl: "Step Functions（ASL）",
      temporal: "Temporal（TypeScript）",
      "temporal-python": "Temporal（Python）",
      "temporal-go": "Temporal（Go）",
      durable: "Lambda durable functions",
      argo: "Argo Workflows",
      "pydantic-graph": "pydantic-graph",
    },
  },
};

// The platforms, in the order `dandori build --target` lists them.
const TARGETS = ["asl", "temporal", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"];

// The module and the bundle sit beside this script, so they are found from its own URL rather
// than the page's: the same files are loaded by the page in each language.
const HERE = document.currentScript.src;
const WASM = new URL("dandori.wasm", HERE).href;
const PRESETS = new URL("presets.json", HERE).href;
document.head.append(
  Object.assign(document.createElement("link"), { rel: "stylesheet", href: new URL("playground.css", HERE).href })
);

let VERSION = "";
let mod = null; // the compiled module, kept so that a trapped instance can be replaced
let inst = null;
let bundle = null; // the text of presets.json, handed to every instance

async function boot() {
  if (!mod) {
    mod = await WebAssembly.compileStreaming(fetch(WASM)).catch(async () =>
      WebAssembly.compile(await (await fetch(WASM)).arrayBuffer())
    );
  }
  if (bundle === null) bundle = await (await fetch(PRESETS)).text();
  inst = await WebAssembly.instantiate(mod, {});
  VERSION = take(inst.exports.dandori_version());
  const refused = call("dandori_bundle", bundle);
  if (refused) throw new Error(refused);
}

function put(str) {
  const bytes = new TextEncoder().encode(str);
  const ptr = inst.exports.dandori_alloc(bytes.length);
  new Uint8Array(inst.exports.memory.buffer, ptr + 4, bytes.length).set(bytes);
  return ptr;
}

function take(ptr) {
  // Read the views after the call: allocating may have grown the memory, which detaches every
  // view taken before it.
  const len = new DataView(inst.exports.memory.buffer).getUint32(ptr, true);
  const bytes = new Uint8Array(inst.exports.memory.buffer, ptr + 4, len).slice();
  inst.exports.dandori_free(ptr);
  return new TextDecoder().decode(bytes);
}

// One call: hand over the request, read the answer, release both buffers.
function call(fn, text) {
  const ptr = put(text);
  try {
    return take(inst.exports[fn](ptr));
  } finally {
    inst.exports.dandori_free(ptr);
  }
}

// What the reader picks a flow by: which example, and which version of it.
function label(t, path) {
  if (path.startsWith("tests/")) return t.draft;
  const [, ex, dir, file] = path.split("/");
  const name = t.examples[ex] || ex;
  if (file === undefined) return name + t.and + t.beside(dir.replace(/(\.ja)?\.flow$/, ""));
  return name + t.and + (t.versions[dir] || dir);
}

function start(root, presets) {
  const lang = root.dataset.lang === "ja" ? "ja" : "en";
  const t = TEXT[lang];
  const flows = presets.flows[lang];
  const src = root.querySelector(".pg-src");
  const status = root.querySelector(".pg-status");
  const out = root.querySelector(".pg-out");
  const chooser = root.querySelector(".pg-preset");
  const where = root.querySelector(".pg-path");
  const revert = root.querySelector(".pg-revert");
  const target = root.querySelector(".pg-target");
  const picker = root.querySelector(".pg-picker");
  let view = "check";
  let flow = flows[0];
  let timer = null;
  let files = [];
  const edited = new Map(); // what the reader has made of each flow while the page is open

  revert.textContent = t.revert;
  chooser.replaceChildren(
    ...flows.map((f, i) => Object.assign(document.createElement("option"), { value: String(i), textContent: label(t, f.path) }))
  );
  target.replaceChildren(
    ...TARGETS.map((k) => Object.assign(document.createElement("option"), { value: k, textContent: t.targets[k] }))
  );

  const pre = (text, links) => {
    const el = document.createElement("pre");
    el.className = "pg-pre";
    if (!links) {
      el.textContent = text;
      return el;
    }
    // Where a finding is, as `path:line:col`, takes the reader to that line of the flow.
    const at = new RegExp(flow.path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + ":(\\d+):(\\d+)", "g");
    let last = 0;
    for (const m of text.matchAll(at)) {
      el.append(text.slice(last, m.index));
      const b = Object.assign(document.createElement("button"), { type: "button", className: "pg-at", textContent: m[0] });
      b.addEventListener("click", () => go(Number(m[1])));
      el.append(b);
      last = m.index + m[0].length;
    }
    el.append(text.slice(last));
    return el;
  };
  const para = (text) => Object.assign(document.createElement("p"), { className: "pg-note", textContent: text });

  function go(line) {
    const lines = src.value.split("\n");
    let from = 0;
    for (let i = 0; i < line - 1 && i < lines.length; i++) from += lines[i].length + 1;
    src.focus();
    src.setSelectionRange(from, from + (lines[line - 1] || "").length);
    const height = parseFloat(getComputedStyle(src).lineHeight) || 16;
    src.scrollTop = Math.max(0, (line - 4) * height);
  }

  // The address of the page `doc` writes, as it stands. A link, not `window.open`: a scripted
  // pop-up is blocked often enough to be unreliable, and a link the reader clicks is a plain
  // navigation. What it points at is a small page holding the one `doc` wrote in a sandboxed
  // frame, whose script runs with no access to this site.
  let pageUrls = [];
  function forget() {
    for (const u of pageUrls) URL.revokeObjectURL(u);
    pageUrls = [];
  }
  function pageLink(html) {
    const title = (html.match(/<title>([^<]*)<\/title>/) || [, "dandori doc"])[1];
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

  function open(i) {
    flow = flows[i];
    chooser.value = String(i);
    where.textContent = flow.path;
    src.value = edited.has(flow.path) ? edited.get(flow.path) : presets.files[flow.path];
    target.value = flow.target;
    picker.dataset.path = "";
    revert.hidden = !edited.has(flow.path);
  }

  function run() {
    if (!inst) return;
    const request = { path: flow.path, source: src.value, lang };
    if (view === "build") request.target = target.value;
    const started = performance.now();
    let a;
    try {
      a = JSON.parse(call("dandori_" + view, JSON.stringify(request)));
    } catch (e) {
      // A trap leaves the instance unusable; the module is still good, so take a new one. The
      // same text would stop it again, so the next edit is what runs on it.
      status.textContent = t.broken;
      inst = null;
      boot().catch(() => (status.textContent = t.failed));
      return;
    }
    const ms = Math.round(performance.now() - started);
    target.hidden = view !== "build";
    picker.hidden = true;
    forget();
    if (a.error) {
      status.textContent = t.status(ms, a.error);
      out.replaceChildren();
      return;
    }
    const counts = a.errors ? t.fails(a.errors, a.warnings) : t.passes(a.warnings);
    if (view === "check") {
      status.textContent = t.status(ms, counts);
      out.replaceChildren(pre(a.text, true));
      return;
    }
    if (view === "build") {
      if (!a.ok) {
        status.textContent = t.status(ms, counts, t.notBuilt);
        const why = a.errors ? t.refusedCheck : t.refusedTarget(t.targets[target.value]);
        out.replaceChildren(para(why), pre(a.text, true));
        return;
      }
      files = a.files;
      picker.replaceChildren(
        ...files.map((f, i) => Object.assign(document.createElement("option"), { value: String(i), textContent: f.path }))
      );
      picker.hidden = false;
      const keep = files.findIndex((f) => f.path === picker.dataset.path);
      picker.value = String(keep < 0 ? 0 : keep);
      status.textContent = t.status(ms, counts, t.files(files.length));
      out.replaceChildren(pre(files[picker.value].body, false));
      return;
    }
    if (view === "rules") {
      status.textContent = t.status(ms, counts, t.rules(a.rules.length));
      if (!a.rules.length) {
        out.replaceChildren(...(a.text && a.errors ? [para(t.unresolved), pre(a.text, true)] : [para(t.noRules)]));
        return;
      }
      const shown = a.rules.map((r) => {
        const box = Object.assign(document.createElement("section"), { className: "pg-rule" });
        const head = Object.assign(document.createElement("div"), { className: "pg-rule-head" });
        head.append(
          Object.assign(document.createElement("code"), { textContent: r.name }),
          ` · ${r.rule} v${r.version} · `,
          Object.assign(document.createElement("span"), { className: "pg-rule-file", textContent: r.file })
        );
        if (r.page !== null) head.append(opener(pageLink(r.page), t.openRule));
        box.append(head);
        if (r.page === null) box.append(pre(r.error, false));
        box.append(pre(r.source, false));
        return box;
      });
      out.replaceChildren(para(t.rulesNote), ...shown);
      return;
    }
    if (!a.drawn) {
      status.textContent = t.status(ms, counts, t.notDrawn);
      out.replaceChildren(para(t.undrawable), pre(a.text, true));
      return;
    }
    status.textContent = t.status(ms, counts, t.drawn);
    const box = Object.assign(document.createElement("div"), { className: "pg-board" });
    const p = document.createElement("p");
    p.append(t.board[0], Object.assign(document.createElement("code"), { textContent: t.board[1] }), t.board[2]);
    const link = opener(pageLink(a.html), t.open);
    const md = document.createElement("details");
    md.className = "pg-md";
    md.append(Object.assign(document.createElement("summary"), { textContent: t.markdown }), pre(a.markdown, false));
    box.append(p, link);
    out.replaceChildren(box, md);
  }

  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(run, 300);
  };

  src.addEventListener("input", () => {
    if (src.value === presets.files[flow.path]) edited.delete(flow.path);
    else edited.set(flow.path, src.value);
    revert.hidden = !edited.has(flow.path);
    later();
  });
  chooser.addEventListener("change", () => {
    open(Number(chooser.value));
    run();
  });
  revert.addEventListener("click", () => {
    edited.delete(flow.path);
    open(Number(chooser.value));
    run();
  });
  target.addEventListener("change", () => {
    picker.dataset.path = "";
    run();
  });
  picker.addEventListener("change", () => {
    picker.dataset.path = files[picker.value].path;
    out.replaceChildren(pre(files[picker.value].body, false));
  });
  const show = (v) => {
    view = v;
    for (const o of root.querySelectorAll("[data-view]")) o.classList.toggle("on", o.dataset.view === v);
  };
  for (const b of root.querySelectorAll("[data-view]")) {
    b.addEventListener("click", () => {
      show(b.dataset.view);
      run();
    });
  }

  // A link can open the page on a flow, a tab and a platform:
  // #flow=examples/hotel/temporal/hotel.flow&view=build&target=asl. The headings' own anchors
  // name none of them, and leave the page as it is.
  function follow() {
    const h = {};
    for (const kv of location.hash.replace(/^#/, "").split("&")) {
      const [k, v] = kv.split("=");
      if (k) h[k] = decodeURIComponent(v || "");
    }
    const i = flows.findIndex((f) => f.path === h.flow);
    if (i < 0 && !h.view && !h.target) return false;
    if (i >= 0) open(i);
    if (["check", "build", "doc", "rules"].includes(h.view)) show(h.view);
    if (TARGETS.includes(h.target)) target.value = h.target;
    return true;
  }
  window.addEventListener("hashchange", () => {
    if (!follow()) return;
    run();
    root.scrollIntoView({ block: "start" });
  });

  open(0);
  follow();
  run();
}

const roots = document.querySelectorAll(".pg");
for (const root of roots) root.querySelector(".pg-status").textContent = TEXT[root.dataset.lang === "ja" ? "ja" : "en"].booting;
boot()
  .then(() => {
    const presets = JSON.parse(bundle);
    for (const root of roots) start(root, presets);
  })
  .catch((e) => {
    for (const root of roots) root.querySelector(".pg-status").textContent = `${TEXT[root.dataset.lang === "ja" ? "ja" : "en"].failed}: ${e.message}`;
  });
