// The playground's computing, off the page's thread: ritsu itself, compiled to wasm32
// (crates/ritsu-wasm), held here once, and asked what the page asks.
//
// The page sends one message at a time, `{ seq, jobs: [[what, request], …] }`, where `what` is
// check, gen or doc and `request` the JSON the module takes; it sends the next only once this one is
// answered, and drops an answer that is older than what it shows (playground.js, `refresh`). Every
// answer says the version of the module, so that a stale ritsu.wasm in a cache shows on the page
// rather than answering quietly.
//
// One convention crosses the boundary, the one rulec's and dandori's playgrounds kept: every buffer
// begins with its own length as a little-endian u32. `put` writes one, `take` reads one and frees it.

const WASM = new URL("ritsu.wasm", self.location.href).href;
let mod = null; // the compiled module, kept so that a trapped instance can be replaced
let inst = null;
let version = "";

async function boot() {
  if (!mod) {
    mod = await WebAssembly.compileStreaming(fetch(WASM)).catch(async () =>
      WebAssembly.compile(await (await fetch(WASM)).arrayBuffer())
    );
  }
  inst = await WebAssembly.instantiate(mod, {});
  version = take(inst.exports.ritsu_version());
}

function put(str) {
  const bytes = new TextEncoder().encode(str);
  const ptr = inst.exports.ritsu_alloc(bytes.length);
  new Uint8Array(inst.exports.memory.buffer, ptr + 4, bytes.length).set(bytes);
  return ptr;
}

function take(ptr) {
  // Read the views after the call: allocating may have grown the memory, which detaches every view
  // taken before it.
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

const booted = boot();

self.onmessage = async ({ data }) => {
  const { seq, jobs } = data;
  try {
    await booted;
  } catch (e) {
    self.postMessage({ seq, failed: String((e && e.message) || e) });
    return;
  }
  if (!inst) {
    try {
      await boot();
    } catch (e) {
      self.postMessage({ seq, failed: String((e && e.message) || e) });
      return;
    }
  }
  const started = performance.now();
  const answers = [];
  for (const [what, request] of jobs) {
    try {
      answers.push(JSON.parse(call("ritsu_" + what, request)));
    } catch (e) {
      // A trap leaves the instance unusable; the module is still good, so the next message takes a
      // new one. The same project would stop it again, so the next edit is what runs on it.
      inst = null;
      self.postMessage({ seq, version, broken: true });
      return;
    }
  }
  self.postMessage({ seq, version, ms: Math.round(performance.now() - started), answers });
};
