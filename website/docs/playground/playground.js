// The playground: ritsu itself, compiled to wasm32 (crates/ritsu-wasm), running in the reader's
// browser. One component, on two pages: the page that explains it (website/docs/playground.md and the
// Japanese one), where it stacks the code above what ritsu makes of it, and the page that is nothing
// but the playground (website/docs/play/index.html and the Japanese one), where the two sit side by
// side. The pages differ only in how they lay the component out (DESIGN 8.7).
//
// Nothing is sent anywhere. The project's files are held here and edited in an editor (CodeMirror 6,
// bundled in libraries.js); the module runs in a Web Worker (worker.js), which is handed all of them
// with every request and runs the command on them as it would run in a directory holding the same
// files: `ritsu check .` over the whole project, and a language's generator and page on the file
// that is open. The projects come from projects.json, written by crates/ritsu/tests/playground.rs
// from the files they are made of; the words the editor colours in each language, from words.json,
// written by the same tests from each language's own list.
//
// A link opens the page on a project as the reader made it: the project, the file, the view, the
// target, and what the reader changed, packed (see `pack`). The page reads the links dandori's
// playground gave too (#flow=…), and those that the pages before it send on here.

import * as cm from "./libraries.js";

const PROJECTS = new URL("projects.json", import.meta.url).href;
const WORDS = new URL("words.json", import.meta.url).href;
const WORKER = new URL("worker.js", import.meta.url).href;
// The page that explains the playground is published where its files are (/ritsu/playground/), and
// the page of the playground alone beside it (/ritsu/play/), in each language.
const EXPLAINED = new URL("./", import.meta.url).href;
const ALONE = new URL("../play/", import.meta.url).href;
const STYLE = new URL("playground.css", import.meta.url).href;
if (![...document.querySelectorAll("link[rel=stylesheet]")].some((l) => l.href === STYLE)) {
  document.head.append(Object.assign(document.createElement("link"), { rel: "stylesheet", href: STYLE }));
}

const TEXT = {
  en: {
    booting: "loading ritsu…",
    failed: "could not load ritsu",
    broken: "ritsu stopped on this input and was loaded again; please report the project that did it",
    status: (version, ms, ...what) => [`ritsu ${version}`, `${ms} ms`, ...what].join(" · "),
    counts: (e, w) => (e || w ? `${e} error${e === 1 ? "" : "s"}, ${w} warning${w === 1 ? "" : "s"}` : "all pass"),
    files: (n) => `${n} file${n === 1 ? "" : "s"}`,
    projects: { shop: "A small shop", "shop.ja": "A small shop, in Japanese", empty: "An empty project", "sekisho/refunds": "Who may refund an order of a shop" },
    groups: { ritsu: "ritsu: every language in one project", own: "Start from one file", rulec: "rulec: one rule", dandori: "dandori: a flow and the files it reads", sekisho: "sekisho: a gate and the files it reads" },
    rules: { gap: "The table with a row missing", full: "The whole table", multi: "Tables in stages", big: "A bigger rule", walk: "Walking a list" },
    draft: "A first draft of the hotel booking, with errors",
    and: " · ",
    examples: { hotel: "Hotel booking", order: "Order", fulfillment: "Fulfillment", inquiry: "Inquiry", review: "Review", invoice: "Invoice", payout: "Payout" },
    versions: { temporal: "for Temporal", aws: "for AWS", "pydantic-graph": "for pydantic-graph", argo: "for Argo Workflows" },
    everywhere: "for every platform",
    beside: (name) => `${name}, the child flow, for every platform`,
    empty: "This project has no file yet. Press “add a file”, give the file a path that ends in the extension of its language (fee.rule, order.flow, days.cal, stock.book, needs.req, shop.ctx, refunds.gate, greeter.geas, order.proto), and write or paste the file in the editor. A file it reads, such as a rule a flow calls, is one more file of the project, at the path the first one names.",
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
    expand: "open full screen",
    about: "about this page",
    home: "ritsu",
    project: "The project to open",
    filesLabel: "The project's files",
    editor: (p) => `The file ${p}`,
    views: { doc: "the page for people", markdown: "Markdown", check: "problems", gen: "generated code" },
    viewsLabel: "What ritsu says of the file and makes of it",
    panes: { code: "code", result: "result" },
    panesLabel: "What the screen shows",
    split: "The border between the code and the result",
    target: "The target",
    picker: "The file written",
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
    save: "save as HTML",
    openTab: "open in a tab of its own",
    rendered: "rendered",
    source: "source",
    copy: "copy",
    copied: "copied",
    cannotCopy: "this browser does not let the page copy",
    pageTitle: (p) => `The page for people of ${p}`,
    markdownTitle: (p) => `The Markdown of ${p}, rendered`,
    stale: "Not drawn again: the file does not pass now, so this is the page drawn last.",
    first: (where, what) => `${where}: ${what}`,
    line: (n) => `line ${n}`,
    goTo: "go to the problem",
    severity: { error: "error", warning: "warning", info: "note" },
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
    phrases: {},
  },
  ja: {
    booting: "ritsu を読み込んでいます…",
    failed: "ritsu を読み込めませんでした",
    broken: "この入力で ritsu が止まったので、読み込み直しました。このプロジェクトを報告してください",
    status: (version, ms, ...what) => [`ritsu ${version}`, `${ms} ms`, ...what].join(" ・ "),
    counts: (e, w) => (e || w ? `エラー ${e} 件、警告 ${w} 件` : "どれも検査を通りました"),
    files: (n) => `${n} ファイル`,
    projects: { shop: "小さな通販（英語）", "shop.ja": "小さな通販（日本語）", empty: "空のプロジェクト", "sekisho/refunds.ja": "店の注文を返金してよい人" },
    groups: { ritsu: "ritsu（全部の言語がそろうプロジェクト）", own: "ファイル一つから始める", rulec: "rulec（規則一つ）", dandori: "dandori（フローと、フローが読むファイル）", sekisho: "sekisho（ゲートと、ゲートが読むファイル）" },
    rules: { gap: "一行足りない表", full: "そろった表", multi: "表をつなぐ", big: "大きい規則", walk: "並びを歩く" },
    draft: "ホテルの予約の最初の下書き（エラーあり）",
    and: "・",
    examples: { hotel: "ホテルの予約", order: "注文", fulfillment: "引当と発送", inquiry: "問い合わせ", review: "審査", invoice: "請求", payout: "支払い" },
    versions: { temporal: "Temporal 版", aws: "AWS 版", "pydantic-graph": "pydantic-graph 版", argo: "Argo Workflows 版" },
    everywhere: "どのプラットフォームでもそのまま動く版",
    beside: (name) => `${name}（子のフロー。どのプラットフォームでもそのまま動く）`,
    empty: "このプロジェクトには、まだファイルがありません。「ファイルを足す」を押して、言語の拡張子で終わるパス（fee.rule、order.flow、days.cal、stock.book、needs.req、shop.ctx、refunds.gate、greeter.geas、order.proto）を付け、エディターにファイルの中身を書くか貼り付けてください。フローが呼ぶ規則のように、そのファイルが読むファイルは、そこに書いてあるパスに、もう一つのファイルとして足してください。",
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
    expand: "全画面で開く",
    about: "解説",
    home: "ritsu",
    project: "開くプロジェクト",
    filesLabel: "プロジェクトのファイル",
    editor: (p) => `ファイル ${p}`,
    views: { doc: "人が読むページ", markdown: "Markdown", check: "問題", gen: "生成したコード" },
    viewsLabel: "ritsu がファイルについて言うことと、ファイルから作るもの",
    panes: { code: "コード", result: "結果" },
    panesLabel: "画面に出すもの",
    split: "コードと結果の境目",
    target: "生成先",
    picker: "書いたファイル",
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
    save: "HTML で保存",
    openTab: "別のタブで開く",
    rendered: "描いたもの",
    source: "元の文字",
    copy: "コピー",
    copied: "コピーしました",
    cannotCopy: "このブラウザでは、ページからコピーできません",
    pageTitle: (p) => `${p} の人が読むページ`,
    markdownTitle: (p) => `${p} の Markdown を描いたもの`,
    stale: "描き直せません。ファイルがいまは検査を通らないので、最後に描いたページを出しています。",
    first: (where, what) => `${where}：${what}`,
    line: (n) => `${n} 行目`,
    goTo: "問題の行へ移る",
    severity: { error: "エラー", warning: "警告", info: "注" },
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
    // The words of the editor's own panels: its search, its list of problems, its marks.
    phrases: {
      Find: "検索",
      Replace: "置換",
      next: "次へ",
      previous: "前へ",
      all: "すべて",
      "match case": "大文字と小文字を区別",
      regexp: "正規表現",
      "by word": "単語単位",
      replace: "置換",
      "replace all": "すべて置換",
      close: "閉じる",
      "current match": "いまの一致",
      "replaced $ matches": "$ か所を置換しました",
      "replaced match on line $": "$ 行目を置換しました",
      "on line": "行目",
      "Go to line": "行へ移る",
      go: "移る",
      Diagnostics: "指摘",
      "No diagnostics": "指摘はありません",
      "Control character": "制御文字",
      "Selection deleted": "選んだところを消しました",
    },
  },
};

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
const extension = (p) => (p.match(/\.([A-Za-z0-9]+)$/) || [])[1] || "";

// A column of a finding counts characters, as every language counts them but one: rulec counts the
// bytes of the line's UTF-8.
const BYTES = new Set(["rulec"]);

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

// What the reader changed of a project, as a link carries it: every file changed or added, with its
// text, and every file removed, with null, by its path, in the order of the tabs.
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

// ── The editor ──────────────────────────────────────────────────────────────────────────────

// A language of ritsu's in the editor: `#` starts a comment outside a string, a string is between
// `"`s, and a word is one of the language's own (words.json) or a name. Every language writes these
// the same way, so one mode serves the eight, each with its words.
function ritsuMode(name, words) {
  const own = new Set(words);
  return cm.StreamLanguage.define({
    name,
    token(stream) {
      if (stream.eatSpace()) return null;
      const c = stream.peek();
      if (c === "#") {
        stream.skipToEnd();
        return "comment";
      }
      if (c === '"') {
        stream.next();
        while (!stream.eol()) {
          const d = stream.next();
          if (d === "\\") stream.next();
          else if (d === '"') break;
        }
        return "string";
      }
      if (stream.match(/^\d{4}-\d{2}-\d{2}/) || stream.match(/^\d[\d,_]*(?:\.\d+)?/)) return "number";
      if (stream.match(/^[\p{L}\p{M}\p{N}_]+/u)) return own.has(stream.current()) ? "keyword" : "variableName";
      if (stream.match(/^(?:->|=>|<=|>=|→|[|=<>+\-*\/×÷−≦≧])/)) return "operator";
      stream.next();
      return "punctuation";
    },
    languageData: { commentTokens: { line: "#" } },
  });
}

// The colours, from the component's stylesheet, so that they follow light and dark.
const COLOURS = cm.HighlightStyle.define([
  { tag: cm.tags.keyword, color: "var(--pg-c-keyword)", fontWeight: "600" },
  { tag: [cm.tags.string, cm.tags.special(cm.tags.string), cm.tags.regexp], color: "var(--pg-c-string)" },
  { tag: [cm.tags.number, cm.tags.integer, cm.tags.float], color: "var(--pg-c-number)" },
  { tag: [cm.tags.comment, cm.tags.lineComment, cm.tags.blockComment], color: "var(--pg-c-comment)", fontStyle: "italic" },
  { tag: [cm.tags.operator, cm.tags.punctuation, cm.tags.separator, cm.tags.bracket], color: "var(--pg-c-punct)" },
  { tag: [cm.tags.atom, cm.tags.bool, cm.tags.null], color: "var(--pg-c-atom)" },
  { tag: [cm.tags.propertyName, cm.tags.attributeName], color: "var(--pg-c-property)" },
  { tag: [cm.tags.typeName, cm.tags.className, cm.tags.definition(cm.tags.variableName)], color: "var(--pg-c-type)" },
  { tag: cm.tags.meta, color: "var(--pg-c-comment)" },
]);

// A wide character takes two columns, as the languages count them when they line a table up (rulec's
// `diag::char_width`: East Asian Width W and F, and ℃ and ℉, which every monospace face that has them
// draws two columns wide). A monospace face seldom has the CJK characters, and the face the browser
// falls back to for them does not draw them two columns wide, so each wide character is set in a box
// two columns wide, and the `|` of a table written in Japanese line up.
const WIDE = /[\u1100-\u115F\u2E80-\uA4CF\uAC00-\uD7A3\uF900-\uFAFF\uFE30-\uFE6F\uFF00-\uFF60\uFFE0-\uFFE6\u2103\u2109\u{20000}-\u{2FFFD}\u{30000}-\u{3FFFD}]/gu;
const wideBoxes = new cm.MatchDecorator({ regexp: WIDE, decoration: cm.Decoration.mark({ class: "pg-wide" }) });
const COLUMNS = cm.ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = wideBoxes.createDeco(view);
    }
    update(u) {
      this.decorations = wideBoxes.updateDeco(u, this.decorations);
    }
  },
  { decorations: (v) => v.decorations }
);

const THEME = cm.EditorView.theme({
  "&": { height: "100%", color: "var(--pg-fg)", backgroundColor: "var(--pg-bg)" },
  ".cm-scroller": { fontFamily: "var(--pg-mono)", lineHeight: "1.55", overflow: "auto" },
  ".cm-content": { caretColor: "var(--pg-fg)" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--pg-fg)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": { backgroundColor: "var(--pg-selection)" },
  ".cm-activeLine": { backgroundColor: "var(--pg-active)" },
  ".cm-gutters": { backgroundColor: "var(--pg-bar)", color: "var(--pg-dim)", borderRight: "1px solid var(--pg-line)" },
  ".cm-activeLineGutter": { backgroundColor: "var(--pg-active)", color: "var(--pg-fg)" },
  ".cm-panels": { backgroundColor: "var(--pg-bar)", color: "var(--pg-fg)" },
  ".cm-tooltip": { backgroundColor: "var(--pg-bg)", color: "var(--pg-fg)", border: "1px solid var(--pg-line)" },
  ".cm-searchMatch": { backgroundColor: "var(--pg-match)" },
  ".pg-wide": { display: "inline-block", width: "2ch", textAlign: "center" },
  ".cm-selectionMatch": { backgroundColor: "var(--pg-match-soft)" },
});

// ── The component ───────────────────────────────────────────────────────────────────────────

const VIEWS = ["doc", "markdown", "check", "gen"];
let seqs = 0;

function start(root, bundle, words, worker) {
  const lang = root.dataset.lang === "ja" ? "ja" : "en";
  const t = TEXT[lang];
  const alone = root.classList.contains("pg-full");
  const element = (tag, props = {}, ...children) => {
    const e = Object.assign(document.createElement(tag), props);
    for (const c of children) if (c != null) e.append(c);
    return e;
  };
  const uid = `pg${++seqs}`;

  // every project with its files' texts; the page lists those of its language, and opens another's
  // when a link names it
  const all = bundle.projects.map((p) => ({ ...p, files: p.files.map(([path, from]) => [path, bundle.texts[from]]) }));
  const projects = all.filter((p) => !p.lang || p.lang === lang);

  // ── the parts
  const chooser = element("select", { className: "pg-project" });
  chooser.setAttribute("aria-label", t.project);
  const add = element("button", { type: "button", className: "pg-add", textContent: t.add });
  const remove = element("button", { type: "button", className: "pg-remove", textContent: t.remove });
  const revert = element("button", { type: "button", className: "pg-revert", textContent: t.revert, hidden: true });
  const share = element("button", { type: "button", className: "pg-share", textContent: t.share });
  const status = element("span", { className: "pg-status", textContent: t.booting });
  status.setAttribute("role", "status");
  const expand = alone ? null : element("a", { className: "pg-expand", href: ALONE, textContent: t.expand });
  const about = alone ? element("a", { className: "pg-about", href: EXPLAINED, textContent: t.about }) : null;
  const home = alone ? element("a", { className: "pg-home", href: new URL("../", EXPLAINED).href, textContent: t.home }) : null;
  // on a narrow screen, the page alone shows the code or the result
  const panes = alone ? element("div", { className: "pg-panes" }) : null;
  const paneTabs = {};
  if (panes) {
    panes.setAttribute("role", "tablist");
    panes.setAttribute("aria-label", t.panesLabel);
    for (const p of ["code", "result"]) {
      paneTabs[p] = element("button", { type: "button", className: "pg-pane", textContent: t.panes[p], id: `${uid}-pane-${p}` });
      paneTabs[p].setAttribute("role", "tab");
      paneTabs[p].dataset.pane = p;
      panes.append(paneTabs[p]);
    }
  }
  const bar = element("div", { className: "pg-bar" }, home, chooser, add, remove, revert, share, expand, status, panes, about);

  const tabs = element("div", { className: "pg-files" });
  tabs.setAttribute("role", "tablist");
  tabs.setAttribute("aria-label", t.filesLabel);
  const editorBox = element("div", { className: "pg-editor", id: `${uid}-editor` });
  editorBox.setAttribute("role", "tabpanel");
  const emptyNote = element("p", { className: "pg-note pg-empty", textContent: t.empty, hidden: true });
  const left = element("section", { className: "pg-left", id: `${uid}-left` }, tabs, editorBox, emptyNote);

  const split = alone ? element("div", { className: "pg-split", tabIndex: 0 }) : null;
  if (split) {
    split.setAttribute("role", "separator");
    split.setAttribute("aria-orientation", "vertical");
    split.setAttribute("aria-label", t.split);
    split.setAttribute("aria-valuemin", "15");
    split.setAttribute("aria-valuemax", "85");
  }

  const viewTabs = element("div", { className: "pg-tabs" });
  viewTabs.setAttribute("role", "tablist");
  viewTabs.setAttribute("aria-label", t.viewsLabel);
  const tab = {};
  const panel = {};
  for (const v of VIEWS) {
    tab[v] = element("button", { type: "button", className: "pg-tab", id: `${uid}-tab-${v}` }, element("span", { textContent: t.views[v] }));
    tab[v].setAttribute("role", "tab");
    tab[v].dataset.view = v;
    tab[v].setAttribute("aria-controls", `${uid}-panel-${v}`);
    viewTabs.append(tab[v]);
    panel[v] = element("div", { className: "pg-panel", id: `${uid}-panel-${v}`, hidden: true });
    panel[v].dataset.view = v;
    panel[v].setAttribute("role", "tabpanel");
    panel[v].setAttribute("aria-labelledby", tab[v].id);
  }
  const count = element("span", { className: "pg-count" });
  tab.check.append(count);

  // the page for people: its tools, and the frames it is drawn in
  const save = element("a", { className: "pg-save", textContent: t.save });
  const openTab = element("a", { className: "pg-opentab", textContent: t.openTab, target: "_blank", rel: "noopener" });
  const docTools = element("div", { className: "pg-tools" }, save, openTab);
  const docStage = element("div", { className: "pg-stage" });
  const docNote = element("div", { className: "pg-said", hidden: true });
  panel.doc.append(docTools, docStage, docNote);
  // the Markdown: rendered in a frame that runs no script, or its text
  const mdRendered = element("button", { type: "button", className: "pg-toggle", textContent: t.rendered });
  const mdSource = element("button", { type: "button", className: "pg-toggle", textContent: t.source });
  const mdCopy = element("button", { type: "button", className: "pg-copy", textContent: t.copy, hidden: true });
  const mdTools = element("div", { className: "pg-tools" }, mdRendered, mdSource, mdCopy);
  const mdStage = element("div", { className: "pg-stage" });
  const mdText = element("pre", { className: "pg-pre pg-mdtext", hidden: true });
  const mdNote = element("div", { className: "pg-said", hidden: true });
  panel.markdown.append(mdTools, mdStage, mdText, mdNote);
  // the problems: the list, then what `ritsu check` prints
  const problems = element("ul", { className: "pg-problems" });
  const checkOut = element("div", { className: "pg-said" });
  panel.check.append(problems, checkOut);
  // the generated code: the target, the file, and the file's text
  const target = element("select", { className: "pg-target", hidden: true });
  target.setAttribute("aria-label", t.target);
  const picker = element("select", { className: "pg-picker", hidden: true });
  picker.setAttribute("aria-label", t.picker);
  const genCopy = element("button", { type: "button", className: "pg-copy", textContent: t.copy, hidden: true });
  const genTools = element("div", { className: "pg-tools" }, target, picker, genCopy);
  const genOut = element("div", { className: "pg-said" });
  panel.gen.append(genTools, genOut);

  const band = element("button", { type: "button", className: "pg-band", hidden: true });
  const right = element("section", { className: "pg-right", id: `${uid}-right` }, viewTabs, ...VIEWS.map((v) => panel[v]), band);
  const main = element("div", { className: "pg-main" }, left, split, right);
  root.replaceChildren(bar, main);
  if (panes) {
    paneTabs.code.setAttribute("aria-controls", left.id);
    paneTabs.result.setAttribute("aria-controls", right.id);
  }

  // ── the state
  let view = root.dataset.view && VIEWS.includes(root.dataset.view) ? root.dataset.view : "check";
  let project = projects[0];
  let files = []; // [path, text], in the order the tabs show them
  let open = null; // the path of the file in the editor
  let marks = new Map(); // path -> "err" | "warn", from the last check
  let targets = new Map(); // language -> the target its generator is asked for in this project
  let unread = false; // the edits of the link the page opened on could not be read
  let mdRaw = false; // the Markdown shows its text rather than what it renders
  let pane = "code"; // on a narrow screen, which of the two the page alone shows
  const kept = new Map(); // what the reader has made of each project while the page is open
  let version = ""; // of the module that answers
  let states = new Map(); // path -> the editor's state of the file
  let edits = 0; // grows with every change to the files: what the answers are for
  let cache = { at: -1, check: null, ms: 0, doc: new Map(), gen: new Map() };
  const good = new Map(); // project/path -> the last page drawn of the file, while the page is open
  const scrolled = new Map(); // project/path -> where the reader left the page for people
  let shown = { doc: null, markdown: null }; // what each frame shows now

  // ── the list of projects, in the groups of the projects, in the order projects.json has them
  function drawChooser() {
    const groups = new Map();
    projects.forEach((p, i) => {
      if (!groups.has(p.group)) groups.set(p.group, element("optgroup", { label: t.groups[p.group] || p.group }));
      const o = element("option", { value: String(i), textContent: title(t, p) });
      o.dataset.name = p.name;
      groups.get(p.group).append(o);
    });
    chooser.replaceChildren(...groups.values());
  }
  drawChooser();

  // ── the editor
  const languages = new Map();
  const modeOf = (path) => {
    const ext = extension(path);
    if (!languages.has(ext)) {
      let mode = null;
      if (words[ext]) mode = ritsuMode(ext, words[ext]);
      else if (ext === "proto") mode = cm.StreamLanguage.define(cm.protobuf);
      else if (ext === "json") mode = cm.StreamLanguage.define(cm.json);
      else if (ext === "yaml" || ext === "yml") mode = cm.StreamLanguage.define(cm.yaml);
      languages.set(ext, mode);
    }
    return languages.get(ext);
  };
  const stateOf = (path, text) =>
    cm.EditorState.create({
      doc: text,
      extensions: [
        cm.lineNumbers(),
        cm.highlightActiveLineGutter(),
        cm.highlightSpecialChars(),
        cm.history(),
        cm.drawSelection(),
        cm.highlightActiveLine(),
        cm.highlightSelectionMatches(),
        cm.search({ top: true }),
        cm.lintGutter(),
        cm.syntaxHighlighting(COLOURS),
        cm.keymap.of([...cm.defaultKeymap, ...cm.historyKeymap, ...cm.searchKeymap, ...cm.lintKeymap]),
        cm.EditorState.tabSize.of(2),
        cm.indentUnit.of("  "),
        cm.EditorState.phrases.of(t.phrases),
        cm.EditorView.contentAttributes.of({ "aria-label": t.editor(path), spellcheck: "false", autocapitalize: "off", autocorrect: "off" }),
        cm.EditorView.updateListener.of((u) => {
          if (u.docChanged) typed(path, u.state);
        }),
        THEME,
        COLUMNS,
        modeOf(path) || [],
      ],
    });
  const editor = new cm.EditorView({ parent: editorBox, state: stateOf("", "") });

  function typed(path, state) {
    states.set(path, state);
    const f = files.find(([p]) => p === path);
    if (!f) return;
    f[1] = state.doc.toString();
    changedFiles();
    keep();
    later();
  }

  // The findings of a check, each where it is: the file, the line, and the column and the length
  // where the language gives them, in the units it counts them in.
  function findings(a) {
    const out = [];
    for (const x of (a && a.json && a.json.diagnostics) || []) {
      const span = (x.spans || [])[0];
      out.push({
        tool: x.tool || "",
        code: x.code || "",
        severity: x.severity || "error",
        file: String(x.file || "").replace(/^\.\//, ""),
        line: Number(x.line) || 0,
        col: Number((span && span.column) || x.column || x.col) || 0,
        len: Number(span && span.length) || 0,
        message: x.title || x.message || "",
      });
    }
    return out;
  }

  // A finding's place in a line of the editor: the column (and the length) counted in characters
  // or in bytes, as an offset in the line's UTF-16.
  function inLine(text, n, bytes) {
    let units = 0;
    let counted = 0;
    for (const ch of text) {
      if (counted >= n) break;
      counted += bytes ? new TextEncoder().encode(ch).length : 1;
      units += ch.length;
    }
    return units;
  }

  function underline(path, a) {
    if (!a || a.at !== edits) return; // the marks stay where the edits moved them until a new check
    const doc = editor.state.doc;
    const list = [];
    for (const f of findings(a)) {
      if (f.file !== path || !f.line || f.line > doc.lines) continue;
      const line = doc.line(f.line);
      let from = line.from;
      let to = line.to;
      if (f.col) {
        const bytes = BYTES.has(f.tool);
        from = line.from + Math.min(line.length, inLine(line.text, f.col - 1, bytes));
        if (f.len) to = line.from + Math.min(line.length, inLine(line.text, f.col - 1 + f.len, bytes));
        else {
          // the word the column is at, or one character
          const rest = line.text.slice(from - line.from);
          const word = rest.match(/^[\p{L}\p{M}\p{N}_.]+/u);
          to = from + (word ? word[0].length : Math.min(1, rest.length));
        }
      }
      if (to <= from) to = Math.min(line.to, from + 1);
      if (to <= from) from = Math.max(line.from, to - 1);
      list.push({ from, to, severity: f.severity === "warning" ? "warning" : f.severity === "error" ? "error" : "info", source: `${f.tool} ${f.code}`.trim(), message: f.message });
    }
    editor.dispatch(cm.setDiagnostics(editor.state, list));
  }

  // ── the files
  function drawTabs() {
    tabs.replaceChildren(
      ...files.map(([p]) => {
        const on = p === open;
        const b = element("button", { type: "button", className: "pg-file", textContent: p, tabIndex: on ? 0 : -1 });
        b.setAttribute("role", "tab");
        b.setAttribute("aria-selected", String(on));
        b.setAttribute("aria-controls", editorBox.id);
        b.dataset.path = p;
        if (on) b.classList.add("on");
        if (marks.has(p)) b.classList.add(marks.get(p));
        if (!kind(p)) b.classList.add("pg-other");
        return b;
      })
    );
    tabs.classList.toggle("pg-one", files.length < 2);
    if (!files.some(([p]) => p === open) && files.length) tabs.firstChild.tabIndex = 0;
  }

  function show(path) {
    if (open && states.has(open)) states.set(open, editor.state);
    open = path;
    const f = files.find(([p]) => p === path);
    if (f && !states.has(path)) states.set(path, stateOf(path, f[1]));
    editor.setState(f ? states.get(path) : stateOf("", ""));
    editorBox.hidden = !files.length;
    emptyNote.hidden = !!files.length;
    picker.dataset.path = "";
    drawTabs();
    underline(path, cache.check && { ...cache.check, at: cache.at });
  }

  function keep() {
    kept.set(project.name, { files: files.map((f) => f.slice()), open });
    revert.hidden = !changes(project.files, files).length;
    remove.disabled = !open;
    // an empty project points at the one way to put a file into it
    add.classList.toggle("pg-cta", !files.length);
  }

  function changedFiles() {
    edits++;
    cache = { at: -1, check: null, ms: 0, doc: new Map(), gen: new Map() };
  }

  function choose(i) {
    project = projects[i];
    chooser.value = String(i);
    const k = kept.get(project.name);
    files = k ? k.files.map((f) => f.slice()) : project.files.map((f) => f.slice());
    marks = new Map();
    targets = new Map(project.target ? [["dandori", project.target]] : []);
    target.replaceChildren();
    states = new Map();
    open = null;
    changedFiles();
    show(k ? k.open : project.open || (files[0] || [])[0] || null);
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

  // ── links
  // A link to the project as it stands: its name, the file, the view, the target, and what the
  // reader changed. The view is left out when it is the check, which is what a link without one
  // opens on, on either page.
  async function link() {
    const h = [["project", project.name]];
    if (open && open !== project.open) h.push(["file", open]);
    if (view !== "check") h.push(["view", view]);
    if (view === "gen" && !target.hidden && target.value) h.push(["target", target.value]);
    const c = changes(project.files, files);
    if (c.length) h.push(["edits", await pack(c)]);
    return "#" + h.map(([k, v]) => k + "=" + encodeURIComponent(v)).join("&");
  }
  let linked = 0;
  async function refreshLinks() {
    if (!expand) return;
    const mine = ++linked;
    try {
      const h = await link();
      if (mine === linked) expand.href = ALONE + h;
    } catch (e) {
      expand.href = ALONE;
    }
  }

  // ── asking the module
  const request = (extra) => JSON.stringify({ files: Object.fromEntries(files), lang, path: open || "", ...extra });
  const genTarget = () => (targets.has(kind(open || "")) ? { target: targets.get(kind(open || "")) } : {});
  const genKey = () => `${open}\n${JSON.stringify(genTarget())}`;
  let inFlight = null; // { seq, at }
  let timer = null;
  let failed = null;

  // What the view needs that the cache does not have: always the check (the marks, the problems and
  // their count, the first problem of a file that stops passing), and the open file's page or
  // generator.
  function needs() {
    const jobs = [];
    if (!cache.check) jobs.push(["check", request({ path: "" })]);
    if (open && (view === "doc" || view === "markdown") && !cache.doc.has(open)) jobs.push(["doc", request({})]);
    if (open && view === "gen" && !cache.gen.has(genKey())) jobs.push(["gen", request(genTarget())]);
    return jobs;
  }

  // The playground is busy (aria-busy) from a change until what it shows is drawn: nothing asked of
  // the module or waiting to be, and every frame loaded.
  let loading = 0;
  const settle = () => {
    if (!inFlight && !timer && !loading) root.removeAttribute("aria-busy");
  };

  function refresh() {
    clearTimeout(timer);
    timer = null;
    if (failed) return;
    const jobs = needs();
    if (!jobs.length) {
      if (!inFlight) {
        draw();
        settle();
      }
      return;
    }
    root.setAttribute("aria-busy", "true");
    if (inFlight) return; // the answer comes back to a page that has moved on, which asks again
    const seq = ++seqs;
    inFlight = { seq, at: edits, jobs: jobs.map(([what]) => what), path: open, key: genKey() };
    worker.postMessage({ seq, jobs });
  }

  function answered(data) {
    if (!inFlight || data.seq !== inFlight.seq) return;
    const asked = inFlight;
    inFlight = null;
    if (data.failed) {
      failed = data.failed;
      status.textContent = `${t.failed}: ${data.failed}`;
      root.removeAttribute("aria-busy");
      return;
    }
    version = data.version || version;
    if (data.broken) {
      status.textContent = t.broken;
      root.removeAttribute("aria-busy");
      return;
    }
    // an answer to files that have changed since is dropped, and the page asks again
    if (asked.at === edits) {
      asked.jobs.forEach((what, i) => {
        const a = data.answers[i];
        if (what === "check") {
          cache.check = a;
          cache.at = asked.at;
          cache.ms = data.ms;
        } else if (what === "doc") cache.doc.set(asked.path, a);
        else cache.gen.set(asked.key, a);
      });
      cache.lastMs = data.ms;
    }
    refresh();
  }
  worker.addEventListener("message", (e) => answered(e.data));

  const later = () => {
    clearTimeout(timer);
    root.setAttribute("aria-busy", "true");
    timer = setTimeout(refresh, 300);
  };

  // ── drawing what the module said
  const pre = (text, links) => {
    const el = element("pre", { className: "pg-pre" });
    if (!links || !files.length) {
      el.textContent = text;
      return el;
    }
    // Where a finding is, as `path:line` or `path:line:col`, takes the reader to that line of that
    // file. yuen writes the path from where it runs, with `./` before it.
    const names = files.map(([p]) => p).sort((a, b) => b.length - a.length);
    const at = new RegExp("(?:\\./)?(" + names.map((p) => p.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|") + "):(\\d+)(?::(\\d+))?", "g");
    let last = 0;
    for (const m of text.matchAll(at)) {
      el.append(text.slice(last, m.index));
      const b = element("button", { type: "button", className: "pg-at", textContent: m[0] });
      b.addEventListener("click", () => go(m[1], Number(m[2])));
      el.append(b);
      last = m.index + m[0].length;
    }
    el.append(text.slice(last));
    return el;
  };
  const para = (text) => element("p", { className: "pg-note", textContent: text });
  const said = (a) => pre(t.command + a.command + "\n" + a.out + a.err, true);

  function go(path, line) {
    if (!files.some(([p]) => p === path)) return;
    if (path !== open) {
      show(path);
      refresh();
    }
    const doc = editor.state.doc;
    const l = doc.line(Math.min(Math.max(line, 1), doc.lines));
    editor.dispatch({ selection: { anchor: l.from, head: l.to }, effects: cm.EditorView.scrollIntoView(l.from, { y: "center" }) });
    if (alone) showPane("code");
    editor.focus();
  }

  // The scheme the page draws in: the site's switch on the page that explains, the system's on the
  // page alone. The page for people and the Markdown are drawn in it too.
  const dark = () => (alone ? matchMedia("(prefers-color-scheme: dark)").matches : document.body.getAttribute("data-md-color-scheme") === "slate");

  // The page a language's `doc` writes, as the frame shows it: in the page's scheme (each page reads
  // `data-theme` on its root), and with a script that tells this page where the reader has scrolled
  // to and goes back there when the page is drawn again. A page may scroll as a whole or in parts of
  // it (rulec's scrolls its side, its board and its dock), so the script keeps every part that has
  // scrolled, by where it is among its parents' children (`""` is the document). The frame runs its
  // scripts with no access to this site (`allow-scripts` without `allow-same-origin`).
  const SCROLL = (at) =>
    `<script data-pg-scroll>(() => { const at = ${JSON.stringify(at)};` +
    ` const find = (p) => { if (!p) return document.scrollingElement; let e = document.body; for (const i of p.split(".")) e = e && e.children[Number(i)]; return e; };` +
    ` const back = () => { for (const [p, x, y] of at) { const e = find(p); if (e) { e.scrollLeft = x; e.scrollTop = y; } } };` +
    ` if (at.length) { back(); addEventListener("load", back); setTimeout(back, 60); }` +
    ` const path = (e) => { const parts = []; while (e && e.parentElement && e !== document.body && e !== document.documentElement) { parts.unshift([...e.parentElement.children].indexOf(e)); e = e.parentElement; } return e === document.body ? parts.join(".") : ""; };` +
    ` const now = new Map(at.map(([p, x, y]) => [p, [x, y]])); let f = 0;` +
    ` addEventListener("scroll", (ev) => { const t = ev.target === document ? document.scrollingElement : ev.target; if (!t || t.scrollTop === undefined) return; now.set(t === document.scrollingElement ? "" : path(t), [t.scrollLeft, t.scrollTop]); cancelAnimationFrame(f); f = requestAnimationFrame(() => parent.postMessage({ ritsuScroll: [...now].map(([p, [x, y]]) => [p, x, y]) }, "*")); }, { capture: true, passive: true });` +
    ` })();</script>`;
  function framed(html, at) {
    const themed = html.replace(/<html(\s[^>]*)?>/i, (m, attrs) => `<html data-theme="${dark() ? "dark" : "light"}"${attrs || ""}>`);
    const script = SCROLL(at);
    const end = themed.toLowerCase().lastIndexOf("</body>");
    return end < 0 ? themed + script : themed.slice(0, end) + script + themed.slice(end);
  }

  // A frame is drawn behind the one shown and takes its place once it has loaded, so that the page
  // does not flash empty between two edits.
  function frameIn(stage, srcdoc, sandbox, label) {
    // a frame still loading is overtaken by this one
    for (const old of stage.querySelectorAll("iframe.pg-coming")) {
      old.remove();
      loading--;
    }
    const next = element("iframe", { className: "pg-frame pg-coming", title: label });
    next.setAttribute("sandbox", sandbox);
    next.srcdoc = srcdoc;
    loading++;
    root.setAttribute("aria-busy", "true");
    next.addEventListener("load", () => {
      if (!next.classList.contains("pg-coming")) return;
      for (const old of stage.querySelectorAll("iframe")) if (old !== next) old.remove();
      next.classList.remove("pg-coming");
      loading--;
      settle();
    });
    stage.append(next);
    return next;
  }
  // a stage with no frame: a frame still loading will not load
  const empty = (stage) => {
    loading -= stage.querySelectorAll("iframe.pg-coming").length;
    stage.replaceChildren();
  };
  // where a frame says the reader has scrolled to: what it sends is held to that shape before it is
  // kept, since a page's own scripts can send this page anything
  window.addEventListener("message", (e) => {
    const at = e.data && e.data.ritsuScroll;
    const place = (p) => Array.isArray(p) && p.length === 3 && typeof p[0] === "string" && /^[0-9.]{0,200}$/.test(p[0]) && Number.isFinite(p[1]) && Number.isFinite(p[2]);
    if (!Array.isArray(at) || at.length > 64 || !at.every(place)) return;
    const frame = [...docStage.querySelectorAll("iframe")].find((f) => f.contentWindow === e.source);
    if (frame && frame.dataset.key) scrolled.set(frame.dataset.key, at.map(([p, x, y]) => [p, Math.round(x), Math.round(y)]));
  });

  const keyOf = (path) => `${project.name}\n${path}`;
  let saveUrl = null;
  let tabUrl = null;
  function drawPage(html) {
    const key = keyOf(open);
    const want = `${key}\n${dark()}\n${html}`;
    if (shown.doc !== want) {
      shown.doc = want;
      const frame = frameIn(docStage, framed(html, scrolled.get(key) || []), "allow-scripts", t.pageTitle(open));
      frame.dataset.key = key;
      if (saveUrl) URL.revokeObjectURL(saveUrl);
      saveUrl = URL.createObjectURL(new Blob([html], { type: "text/html" }));
      save.href = saveUrl;
      save.download = (open.split("/").pop() || "page").replace(/\.[^.]+$/, "") + ".html";
      // a page of its own holds the page in a frame sandboxed as this one
      const title = (html.match(/<title>([^<]*)<\/title>/) || [, "ritsu"])[1];
      const host =
        '<!doctype html><meta charset="utf-8"><title>' + title + "</title>" +
        "<style>html,body{margin:0;height:100%}iframe{display:block;border:0;width:100%;height:100%}</style>" +
        '<iframe sandbox="allow-scripts" srcdoc="' + html.replace(/&/g, "&amp;").replace(/"/g, "&quot;") + '"></iframe>';
      if (tabUrl) URL.revokeObjectURL(tabUrl);
      tabUrl = URL.createObjectURL(new Blob([host], { type: "text/html" }));
      openTab.href = tabUrl;
    }
    docStage.hidden = false;
    docTools.hidden = false;
    docNote.hidden = true;
  }

  const markdownPage = (md) => {
    const d = dark();
    const css =
      `:root{color-scheme:${d ? "dark" : "light"}}` +
      `body{margin:0;padding:1.2em 1.5em;font:15px/1.6 system-ui,-apple-system,"Segoe UI",sans-serif;background:${d ? "#16191c" : "#fff"};color:${d ? "#dfe6e8" : "#1d2428"}}` +
      `a{color:${d ? "#7cc4e8" : "#1d5f8a"}}h1,h2,h3{line-height:1.3}` +
      `table{border-collapse:collapse;margin:1em 0;font-size:.92em}th,td{border:1px solid ${d ? "#3a4247" : "#d5dade"};padding:.3em .6em;text-align:left;vertical-align:top}th{background:${d ? "#1f2428" : "#f4f6f7"}}` +
      `code,pre{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.9em}pre{background:${d ? "#1f2428" : "#f4f6f7"};padding:.8em 1em;overflow:auto}` +
      `:not(pre)>code{background:${d ? "#1f2428" : "#f4f6f7"};padding:.1em .3em;border-radius:3px}blockquote{margin:1em 0;padding:0 1em;border-left:3px solid ${d ? "#3a4247" : "#d5dade"};color:${d ? "#96a2a8" : "#5c676d"}}details{margin:1em 0}`;
    const body = new cm.Marked({ gfm: true, async: false }).parse(md);
    return `<!doctype html><html><head><meta charset="utf-8"><style>${css}</style></head><body>${body}</body></html>`;
  };
  function drawMarkdown(md) {
    const want = `${keyOf(open)}\n${dark()}\n${mdRaw}\n${md}`;
    if (shown.markdown !== want) {
      shown.markdown = want;
      mdText.textContent = md;
      if (!mdRaw) frameIn(mdStage, markdownPage(md), "", t.markdownTitle(open));
    }
    mdStage.hidden = mdRaw;
    mdText.hidden = !mdRaw;
    mdCopy.hidden = !mdRaw;
    mdTools.hidden = false;
    mdNote.hidden = true;
  }
  function drawToggle() {
    mdRendered.setAttribute("aria-pressed", String(!mdRaw));
    mdSource.setAttribute("aria-pressed", String(mdRaw));
  }

  // The first problem a reader has to put right before the page is drawn again: the first error of
  // the open file, else the first of the project, by where it is.
  function firstProblem() {
    const all = findings(cache.check).filter((f) => f.severity === "error");
    return all.find((f) => f.file === open) || all[0] || null;
  }
  function drawBand(stale) {
    const f = stale ? firstProblem() : null;
    band.hidden = !stale;
    if (!stale) return;
    band.replaceChildren(
      element("span", { className: "pg-band-head", textContent: t.stale }),
      f ? element("span", { className: "pg-band-first", textContent: t.first(f.file === open ? t.line(f.line) : `${f.file}:${f.line}`, `${f.tool} ${f.code} ${f.message}`) }) : null
    );
    band.dataset.file = f ? f.file : "";
    band.dataset.line = f ? String(f.line) : "";
    band.disabled = !f;
    band.title = f ? t.goTo : "";
  }

  function draw() {
    const a = cache.check;
    if (a && a.error) {
      // a request the module cannot read: the page made it, so the page says what it was told
      status.textContent = t.status(version, cache.ms, a.error);
      return;
    }
    const d = findings(a);
    const errors = d.filter((x) => x.severity === "error").length;
    const warnings = d.filter((x) => x.severity === "warning").length;
    // the marks on the tabs, and on the lines of the open file
    marks = new Map();
    for (const x of d) {
      if (x.severity === "error") marks.set(x.file, "err");
      else if (x.severity === "warning" && marks.get(x.file) !== "err") marks.set(x.file, "warn");
    }
    for (const f of (a && a.json && a.json.files) || []) if (!f.ok && !marks.has(f.file)) marks.set(f.file, "err");
    drawTabs();
    underline(open, { ...a, at: cache.at });
    count.textContent = d.length ? String(d.length) : "";
    count.className = "pg-count" + (errors ? " err" : warnings ? " warn" : "");
    // the problems
    problems.replaceChildren(
      ...d.map((x) => {
        const b = element(
          "button",
          { type: "button", className: `pg-problem ${x.severity === "error" ? "err" : x.severity === "warning" ? "warn" : "info"}` },
          element("span", { className: "pg-sev", textContent: t.severity[x.severity] || x.severity }),
          element("code", { textContent: `${x.tool} ${x.code}`.trim() }),
          element("span", { className: "pg-where", textContent: x.line ? `${x.file}:${x.line}` : x.file }),
          element("span", { className: "pg-msg", textContent: x.message })
        );
        b.addEventListener("click", () => go(x.file, x.line));
        return element("li", {}, b);
      })
    );
    checkOut.replaceChildren(said(a));
    let what = [t.counts(errors, warnings)];

    if (view === "doc" || view === "markdown") what = what.concat(drawDoc());
    if (view === "gen") what = what.concat(drawGen());
    if (view === "check") drawBand(false);
    status.textContent = t.status(version, cache.lastMs != null ? cache.lastMs : cache.ms, ...what);
    if (unread) {
      status.textContent = [status.textContent, t.unread].join(t.and);
      unread = false;
    }
    refreshLinks();
  }

  function drawDoc() {
    const into = view === "doc" ? docNote : mdNote;
    const hideAll = () => {
      docStage.hidden = docTools.hidden = true;
      mdStage.hidden = mdText.hidden = mdTools.hidden = true;
    };
    const note = (...children) => {
      hideAll();
      shown = { doc: null, markdown: null };
      empty(docStage);
      empty(mdStage);
      into.replaceChildren(...children);
      into.hidden = false;
      drawBand(false);
    };
    if (!open) {
      note(para(t.noFile));
      return [];
    }
    const a = cache.doc.get(open);
    const tool = (a && a.tool) || kind(open);
    if (a.none) {
      note(para(tool ? t.noDoc(tool) : t.noLanguage));
      return [];
    }
    const key = keyOf(open);
    if (a.html.code === 0) {
      good.set(key, { html: a.html.out, markdown: a.markdown.out });
    }
    const last = good.get(key);
    const stale = a.html.code !== 0;
    if (stale && !last) {
      note(para(t.refusedDoc), said(a.html));
      return [t.notDrawn];
    }
    if (view === "doc") drawPage(last.html);
    else drawMarkdown(last.markdown);
    docStage.classList.toggle("pg-stale", stale);
    mdStage.classList.toggle("pg-stale", stale);
    mdText.classList.toggle("pg-stale", stale);
    drawBand(stale);
    return [stale ? t.notDrawn : t.drawn];
  }

  let written = [];
  function drawGen() {
    drawBand(false);
    target.hidden = true;
    picker.hidden = true;
    genCopy.hidden = true;
    if (!open) {
      genOut.replaceChildren(para(t.noFile));
      return [];
    }
    const a = cache.gen.get(genKey());
    const tool = (a && a.tool) || kind(open);
    if (a.none) {
      genOut.replaceChildren(para(tool ? t.noGen(tool) : t.noLanguage));
      return [];
    }
    if (a.targets && a.targets.length) {
      target.replaceChildren(...a.targets.map((k) => element("option", { value: k, textContent: t.targets[k] || k })));
      target.value = a.target;
      target.hidden = false;
    }
    if (a.code !== 0) {
      genOut.replaceChildren(para(t.refusedGen), said(a));
      return [t.notWritten];
    }
    // A generator that prints what it writes (`sakai export cml`, `yuen export`) writes no file.
    written = a.files.length ? a.files : [{ path: a.command, body: a.out }];
    picker.replaceChildren(...written.map((f, i) => element("option", { value: String(i), textContent: f.path })));
    picker.hidden = written.length < 2;
    const at = written.findIndex((f) => f.path === picker.dataset.path);
    picker.value = String(at < 0 ? 0 : at);
    genCopy.hidden = false;
    genOut.replaceChildren(pre(written[picker.value].body, false));
    return [a.files.length ? t.files(a.files.length) : t.written];
  }

  // ── the views and the panes
  function switchTo(v) {
    view = v;
    for (const w of VIEWS) {
      const on = w === v;
      tab[w].setAttribute("aria-selected", String(on));
      tab[w].tabIndex = on ? 0 : -1;
      tab[w].classList.toggle("on", on);
      panel[w].hidden = !on;
    }
    root.dataset.showing = v;
  }
  function showPane(p) {
    pane = p;
    root.dataset.pane = p;
    for (const q of ["code", "result"]) {
      if (!paneTabs[q]) continue;
      paneTabs[q].setAttribute("aria-selected", String(q === p));
      paneTabs[q].tabIndex = q === p ? 0 : -1;
      paneTabs[q].classList.toggle("on", q === p);
    }
  }

  // Tabs move with the arrow keys, Home and End, as a list of tabs does.
  function arrows(list, pick) {
    list.addEventListener("keydown", (e) => {
      const all = [...list.querySelectorAll('[role="tab"]')];
      const at = all.indexOf(document.activeElement);
      if (at < 0) return;
      const to = { ArrowRight: at + 1, ArrowDown: at + 1, ArrowLeft: at - 1, ArrowUp: at - 1, Home: 0, End: all.length - 1 }[e.key];
      if (to === undefined) return;
      e.preventDefault();
      const next = all[(to + all.length) % all.length];
      next.focus();
      pick(next);
    });
  }

  // ── what the reader does
  chooser.addEventListener("change", () => {
    choose(Number(chooser.value));
    refresh();
  });
  tabs.addEventListener("click", (e) => {
    const b = e.target.closest(".pg-file");
    if (!b) return;
    show(b.dataset.path);
    refresh();
  });
  arrows(tabs, (b) => {
    show(b.dataset.path);
    refresh();
    tabs.querySelector(`[data-path="${CSS.escape(b.dataset.path)}"]`)?.focus();
  });
  viewTabs.addEventListener("click", (e) => {
    const b = e.target.closest(".pg-tab");
    if (!b) return;
    switchTo(b.dataset.view);
    refresh();
  });
  arrows(viewTabs, (b) => {
    switchTo(b.dataset.view);
    refresh();
  });
  if (panes) {
    panes.addEventListener("click", (e) => {
      const b = e.target.closest(".pg-pane");
      if (b) showPane(b.dataset.pane);
    });
    arrows(panes, (b) => showPane(b.dataset.pane));
  }
  add.addEventListener("click", () => {
    const asked = (window.prompt(t.addAsk) || "").trim().replace(/^\.\//, "");
    if (!asked) return;
    if (!inside(asked)) {
      window.alert(t.addBad(asked));
      return;
    }
    if (files.some(([p]) => p === asked)) {
      window.alert(t.addTaken(asked));
      return;
    }
    files.push([asked, ""]);
    changedFiles();
    show(asked);
    keep();
    if (alone) showPane("code");
    editor.focus();
    refresh();
  });
  remove.addEventListener("click", () => {
    if (!open || !window.confirm(t.removeAsk(open))) return;
    const at = files.findIndex(([p]) => p === open);
    files.splice(at, 1);
    states.delete(open);
    open = null;
    changedFiles();
    show(files.length ? files[Math.min(at, files.length - 1)][0] : null);
    keep();
    refresh();
  });
  revert.addEventListener("click", () => {
    kept.delete(project.name);
    choose(Number(chooser.value));
    refresh();
  });
  target.addEventListener("change", () => {
    picker.dataset.path = "";
    targets.set(kind(open || ""), target.value);
    refresh();
  });
  picker.addEventListener("change", () => {
    picker.dataset.path = written[picker.value].path;
    genOut.replaceChildren(pre(written[picker.value].body, false));
  });
  const copied = async (button, text) => {
    try {
      await navigator.clipboard.writeText(text);
      status.textContent = t.copied;
    } catch (e) {
      status.textContent = t.cannotCopy;
    }
  };
  genCopy.addEventListener("click", () => copied(genCopy, written[picker.value] ? written[picker.value].body : ""));
  mdCopy.addEventListener("click", () => copied(mdCopy, mdText.textContent));
  mdRendered.addEventListener("click", () => {
    mdRaw = false;
    drawToggle();
    refresh();
  });
  mdSource.addEventListener("click", () => {
    mdRaw = true;
    drawToggle();
    refresh();
  });
  band.addEventListener("click", () => {
    if (band.dataset.file) go(band.dataset.file, Number(band.dataset.line));
  });
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
  if (expand) {
    // the page alone, on the project as it stands; this page keeps the link too, so that coming back
    // finds it as it was
    expand.addEventListener("click", async (e) => {
      if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      e.preventDefault();
      let h = "";
      try {
        h = await link();
        history.replaceState(null, "", h);
      } catch (err) {
        h = "";
      }
      location.assign(ALONE + h);
    });
  }

  // the border between the code and the result, which the reader drags or moves with the keys
  if (split) {
    let share0 = 45;
    try {
      share0 = Number(localStorage.getItem("ritsu-playground-split")) || 45;
    } catch (e) {}
    const place = (pct) => {
      const p = Math.min(85, Math.max(15, Math.round(pct)));
      root.style.setProperty("--pg-left", `${p}%`);
      split.setAttribute("aria-valuenow", String(p));
      try {
        localStorage.setItem("ritsu-playground-split", String(p));
      } catch (e) {}
      return p;
    };
    let now = place(share0);
    split.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      split.setPointerCapture(e.pointerId);
      root.classList.add("pg-dragging");
      const box = main.getBoundingClientRect();
      const move = (m) => (now = place(((m.clientX - box.left) / box.width) * 100));
      const up = () => {
        split.removeEventListener("pointermove", move);
        split.removeEventListener("pointerup", up);
        root.classList.remove("pg-dragging");
      };
      split.addEventListener("pointermove", move);
      split.addEventListener("pointerup", up);
    });
    split.addEventListener("keydown", (e) => {
      const step = { ArrowLeft: -5, ArrowRight: 5, Home: -100, End: 100 }[e.key];
      if (step === undefined) return;
      e.preventDefault();
      now = place(now + step);
    });
  }

  // the scheme changes: the frames are drawn again in the new one
  const redraw = () => {
    shown = { doc: null, markdown: null };
    if (!failed && cache.check && !root.hasAttribute("aria-busy")) draw();
  };
  if (alone) matchMedia("(prefers-color-scheme: dark)").addEventListener("change", redraw);
  else new MutationObserver(redraw).observe(document.body, { attributes: true, attributeFilter: ["data-md-color-scheme"] });

  // ── links the page is opened with
  // A link can open the page on a project, a file, a view and a target, with what the reader
  // changed: #project=shop&file=ordering/ship_order.flow&view=gen&target=asl&edits=…. A link with
  // no view opens on the check. It reads the links dandori's page gave as well:
  // #flow=examples/hotel/temporal/hotel.flow&view=build&target=asl opens the project of that flow
  // (build is the generator; rules, the page of its first rule). The headings' own anchors name none
  // of them, and leave the page as it is.
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
      states = new Map();
      open = null;
      changedFiles();
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
    switchTo(VIEWS.includes(h.view) ? h.view : "check");
    if (h.target) targets.set(kind(open || ""), h.target);
    keep();
    return true;
  }
  window.addEventListener("hashchange", () => {
    root.setAttribute("aria-busy", "true");
    follow().then((did) => {
      refresh();
      if (did && !alone) root.scrollIntoView({ block: "start" });
    });
  });

  drawToggle();
  showPane(pane);
  switchTo(view);
  const first = root.dataset.project ? named(root.dataset.project) : -1;
  choose(first < 0 ? 0 : first);
  follow().finally(refresh);
}

const roots = document.querySelectorAll(".pg");
for (const root of roots) {
  const status = Object.assign(document.createElement("span"), { className: "pg-status", textContent: TEXT[root.dataset.lang === "ja" ? "ja" : "en"].booting });
  status.setAttribute("role", "status");
  root.replaceChildren(status);
  root.setAttribute("aria-busy", "true");
}
Promise.all([fetch(PROJECTS).then((r) => r.json()), fetch(WORDS).then((r) => r.json())])
  .then(([bundle, words]) => {
    // one module for every playground of the page
    const worker = new Worker(WORKER);
    for (const root of roots) start(root, bundle, words, worker);
  })
  .catch((e) => {
    for (const root of roots) {
      root.querySelector(".pg-status").textContent = `${TEXT[root.dataset.lang === "ja" ? "ja" : "en"].failed}: ${e.message}`;
      root.removeAttribute("aria-busy");
    }
  });
