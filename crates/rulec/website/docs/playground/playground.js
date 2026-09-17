// The playground: the checker itself, compiled to wasm32 (src/wasm.rs, §15.48).
//
// Nothing is sent anywhere. The table the reader types is handed to the module in this
// page and the answers come back from it, so the page works offline and a rule nobody is
// allowed to paste into a web form is safe to paste here.
//
// One convention crosses the boundary: every buffer begins with its own length as a
// little-endian u32. `put` writes one, `take` reads one and frees it.

const TEXT = {
  en: {
    booting: "loading the checker…",
    checked: (ms, e, w) =>
      `rulec ${VERSION} · ${ms} ms · ${e} error${e === 1 ? "" : "s"}, ${w} warning${w === 1 ? "" : "s"}`,
    passed: (ms) => `rulec ${VERSION} · ${ms} ms · passes`,
    refused: "Nothing is generated for a rule that does not pass — the findings are on the check tab.",
    generated: (n, ms) => `${n} files · ${ms} ms`,
    broken: "the checker stopped on this input and was reloaded; please report the table that did it",
    file: "file",
  },
  ja: {
    booting: "検査器を読み込んでいます…",
    checked: (ms, e, w) => `rulec ${VERSION} ・ ${ms} ms ・ エラー ${e} 件、警告 ${w} 件`,
    passed: (ms) => `rulec ${VERSION} ・ ${ms} ms ・ 通ります`,
    refused: "検査を通らない規則からは何も生成しません。指摘は「検査」のタブにあります。",
    generated: (n, ms) => `${n} ファイル ・ ${ms} ms`,
    broken: "この入力で検査器が止まったので読み直しました。その表を報告してください",
    file: "ファイル",
  },
};

// The table on the front page's opening diagram (website/tools/overview.rule and
// overview-ja.rule). A test holds these two to those files: a playground that shows a
// different table from the picture would be a second source of the same example.
const FULL = {
  en: `rule Fee(fee) v1

enum Zone(zone) = Domestic(domestic) | Overseas(overseas)

inputs
  Destination(dest) : Zone
  Weight(weight)    : mass[g]  range >=1g <=10kg

outputs
  Fee(fee) : money[USD, incl_tax]  round up(1USD)

table FeeTable(fee_table)
policy unique
| Destination | Weight     | -> Fee(fee) : money[USD, incl_tax] |
| Domestic    | <=2kg      | 8USD                               |
| Domestic    | >2kg <=5kg | 10USD                              |
| Domestic    | >5kg       | 13USD                              |
| Overseas    | <=2kg      | 12USD                              |
| Overseas    | >5kg       | 20USD                              |
| Overseas    | >2kg <=5kg | 15USD                              |
`,
  ja: `rule 運賃(fee) v1

enum あて先区分(zone) = 近畿圏(kinki) | 遠隔地(remote)

inputs
  あて先(dest)  : あて先区分
  重量(weight)  : mass[g]  range >=1g <=10kg

outputs
  運賃(fee) : money[円, incl_tax]  round up(10円)

table 運賃表(fee_table)
policy unique
| あて先 | 重量       | -> 運賃(fee) : money[円, incl_tax] |
| 近畿圏 | <=2kg      | 800円                              |
| 近畿圏 | >2kg <=5kg | 1000円                             |
| 近畿圏 | >5kg       | 1300円                             |
| 遠隔地 | <=2kg      | 1200円                             |
| 遠隔地 | >5kg       | 2000円                             |
| 遠隔地 | >2kg <=5kg | 1500円                             |
`,
};

// The picture's missing row is the last one, so the page opens on the table without it:
// the first thing the reader sees is the gap, named with the input that falls through it.
const gap = (src) => src.trimEnd().split("\n").slice(0, -1).join("\n") + "\n";

// The module and the stylesheet sit beside this script, so they are found from its own
// URL rather than the page's: the same three files are loaded by a page in each language.
const HERE = document.currentScript.src;
const WASM = new URL("rulec.wasm", HERE).href;
document.head.append(
  Object.assign(document.createElement("link"), {
    rel: "stylesheet",
    href: new URL("playground.css", HERE).href,
  })
);

let VERSION = "";
let mod = null; // the compiled module, kept so a trapped instance can be replaced
let inst = null;

async function boot() {
  if (!mod) mod = await WebAssembly.compileStreaming(fetch(WASM)).catch(async () =>
    WebAssembly.compile(await (await fetch(WASM)).arrayBuffer())
  );
  inst = await WebAssembly.instantiate(mod, {});
  VERSION = call("rulec_version");
  return inst;
}

function put(str) {
  const bytes = new TextEncoder().encode(str);
  const ptr = inst.exports.rulec_alloc(bytes.length);
  new Uint8Array(inst.exports.memory.buffer, ptr + 4, bytes.length).set(bytes);
  return ptr;
}

function take(ptr) {
  // Read the views after the call: allocating may have grown the memory, which detaches
  // every view taken before it.
  const len = new DataView(inst.exports.memory.buffer).getUint32(ptr, true);
  const bytes = new Uint8Array(inst.exports.memory.buffer, ptr + 4, len).slice();
  inst.exports.rulec_free(ptr);
  return new TextDecoder().decode(bytes);
}

// One call: hand over the source, read the answer, release both buffers.
function call(fn, src, ja) {
  if (src === undefined) return take(inst.exports[fn]());
  const ptr = put(src);
  try {
    return take(inst.exports[fn](ptr, ja ? 1 : 0));
  } finally {
    inst.exports.rulec_free(ptr);
  }
}

function start(root) {
  const lang = root.dataset.lang === "ja" ? "ja" : "en";
  const t = TEXT[lang];
  const src = root.querySelector(".pg-src");
  const status = root.querySelector(".pg-status");
  const out = root.querySelector(".pg-out");
  const picker = root.querySelector(".pg-picker");
  let view = "check";
  let timer = null;
  let files = [];

  src.value = gap(FULL[lang]);
  status.textContent = t.booting;

  const show = (node) => {
    out.replaceChildren(node);
  };
  const pre = (text) => {
    const el = document.createElement("pre");
    el.className = "pg-pre";
    el.textContent = text;
    return el;
  };

  function run() {
    if (!inst) return;
    const started = performance.now();
    let answer;
    try {
      answer = JSON.parse(call(view === "check" ? "rulec_check" : view === "gen" ? "rulec_gen" : "rulec_doc", src.value, lang === "ja"));
    } catch (e) {
      // A trap leaves the instance unusable; the module is still good, so take a new one.
      status.textContent = t.broken;
      inst = null;
      boot().then(run);
      return;
    }
    const ms = Math.round(performance.now() - started);
    picker.hidden = true;
    if (view === "check") {
      status.textContent = answer.errors + answer.warnings === 0 ? t.passed(ms) : t.checked(ms, answer.errors, answer.warnings);
      show(pre(answer.text));
      return;
    }
    if (!answer.ok) {
      status.textContent = t.refused;
      show(pre(answer.text));
      return;
    }
    if (view === "doc") {
      // The approver's page runs the generated JavaScript, so it gets a frame of its own
      // with nothing else in reach: scripts, and no same-origin.
      const frame = document.createElement("iframe");
      frame.className = "pg-frame";
      frame.setAttribute("sandbox", "allow-scripts");
      frame.srcdoc = answer.html;
      status.textContent = `${ms} ms`;
      show(frame);
      return;
    }
    files = answer.files;
    picker.replaceChildren(
      ...files.map((f, i) => {
        const o = document.createElement("option");
        o.value = String(i);
        o.textContent = f.path;
        return o;
      })
    );
    picker.hidden = false;
    const keep = files.findIndex((f) => f.path === picker.dataset.path);
    picker.value = String(keep < 0 ? 0 : keep);
    status.textContent = t.generated(files.length, ms);
    show(pre(files[picker.value].body));
  }

  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(run, 300);
  };

  src.addEventListener("input", later);
  picker.addEventListener("change", () => {
    picker.dataset.path = files[picker.value].path;
    show(pre(files[picker.value].body));
  });
  for (const b of root.querySelectorAll("[data-view]")) {
    b.addEventListener("click", () => {
      view = b.dataset.view;
      for (const o of root.querySelectorAll("[data-view]")) o.classList.toggle("on", o === b);
      run();
    });
  }
  for (const b of root.querySelectorAll("[data-preset]")) {
    b.addEventListener("click", () => {
      src.value = b.dataset.preset === "gap" ? gap(FULL[lang]) : FULL[lang];
      run();
    });
  }

  boot().then(run);
}

for (const root of document.querySelectorAll(".pg")) start(root);
