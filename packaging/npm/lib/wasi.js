// The one place @i2y/ritsu runs ritsu.wasm: `ritsu` built for WASI (wasm32-wasip1) in Node's
// `node:wasi` (ritsu's DESIGN 8.8). The API (index.js), its worker (worker.js) and the commands
// (cli.js) all come here.
//
// The module sees the whole disk at the same paths as the host (the root preopened as `/`), or,
// when the caller names `dirs`, those directories alone, each at its own path; it works in the
// directory the loader names in RITSU_WASI_CWD, so that every path it reads and prints is the one
// the native binary reads and prints there. RITSU_WASI_UTC_OFFSET is the offset of the host's
// clock from UTC, which a WebAssembly module cannot read (`yuen review` takes today's date from
// it). Those two are the only variables the loader adds; everything else in the environment is
// the caller's.

import { closeSync, readFileSync, readlinkSync, realpathSync, statSync, writeSync } from "node:fs";
import { createRequire } from "node:module";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);

// Node 22's V8 (12.4) calls WASI's functions as fast API calls, and uvwasi reports the memory it
// allocates in one, which can start a garbage collection there; the collection then reads a frame
// it cannot read, and the process dies of SIGSEGV (seen under lldb: Heap::CollectGarbage from
// AdjustAmountOfExternalAllocatedMemory in uvwasi's path_filestat_get, crashing in
// IterateTurbofanOptimizedFrame). Node 23 and later do not. On Node 22 those calls are made the
// ordinary way: the flag only tells the optimizer not to make fast calls in what it compiles next.
if (process.versions.node.split(".")[0] === "22") {
  require("node:v8").setFlagsFromString("--no-turbo-fast-api-calls");
}

/** The absolute path of ritsu.wasm in the package. */
export const wasmPath = fileURLToPath(new URL("../ritsu.wasm", import.meta.url));

/** The version of ritsu the module was built from: the package's, which build.sh holds to it. */
export const version = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version;

/** Stop where the module cannot run as the native binary does. */
export function supported() {
  if (process.platform === "win32") {
    // WASI sees the disk through one preopened root; a Windows path has a drive, and the
    // directory a command runs in could not be handed over as it is. Rather than run from the
    // root of a drive, it stops.
    throw new Error(
      "@i2y/ritsu does not run on Windows yet: the working directory and the paths cannot be handed to the WebAssembly module as they are. Use the native ritsu from a release, or run it under WSL.",
    );
  }
}

let compiled;
/** The module, compiled once on this thread. */
export function module() {
  compiled ??= new WebAssembly.Module(readFileSync(wasmPath));
  return compiled;
}

/** `f()`, with Node's ExperimentalWarning about WASI left unsaid; every other warning is said. */
function quietly(f) {
  const emit = process.emitWarning;
  process.emitWarning = function (warning, ...rest) {
    const type = typeof rest[0] === "string" ? rest[0] : rest[0]?.type ?? warning?.name;
    const text = typeof warning === "string" ? warning : warning?.message;
    if (type === "ExperimentalWarning" && /\bWASI\b/.test(String(text))) {
      return;
    }
    return emit.call(this, warning, ...rest);
  };
  try {
    return f();
  } finally {
    process.emitWarning = emit;
  }
}

let WASI;
function wasiClass() {
  // loaded here, not imported at the top: Node says its warning when the module is loaded
  WASI ??= quietly(() => require("node:wasi").WASI);
  return WASI;
}

/** A directory the caller names, as the module sees it: there, a directory, at its real path (symbolic links followed). */
function realDirectory(dir, what) {
  let real;
  try {
    real = realpathSync(dir);
  } catch {
    throw new Error(`@i2y/ritsu: the ${what} ${dir} is not there`);
  }
  if (!statSync(real).isDirectory()) {
    throw new Error(`@i2y/ritsu: the ${what} ${dir} is not a directory`);
  }
  return real;
}

/** The directory a run works in: there, and a directory, as the shell's would be (symbolic links followed). */
export function workingDirectory(cwd) {
  return realDirectory(cwd ?? process.cwd(), "working directory");
}

/**
 * The directories the module can open (WASI's preopens), each at its own path, so that every path
 * the module reads and prints is the one the native binary does: the whole filesystem when the
 * caller names none; else only those (a relative one from the working directory as the caller
 * wrote it, `from`), and the working directory `cwd` (its real path) has to be in one of them.
 * Each is opened at its real path, where `cwd` is, and at the path the caller gave when that goes
 * through a link (on macOS /var/folders, which is /private/var/folders), so that a path written
 * that way reads as it does natively.
 */
export function openedDirectories(dirs, cwd, from = cwd) {
  if (dirs === undefined || dirs === null) {
    return { "/": "/" };
  }
  if (!Array.isArray(dirs) || dirs.length === 0 || !dirs.every((d) => typeof d === "string")) {
    throw new TypeError("@i2y/ritsu: dirs is a list of one directory or more");
  }
  const given = dirs.map((d) => resolve(from, d));
  const preopens = {};
  for (const d of given) {
    const real = realDirectory(d, "directory of dirs");
    preopens[real] = real;
  }
  const inside = Object.keys(preopens).some((d) => d === "/" || cwd === d || cwd.startsWith(`${d}/`));
  if (!inside) {
    throw new Error(`@i2y/ritsu: the working directory ${cwd} is in none of dirs (${Object.keys(preopens).join(", ")})`);
  }
  for (const d of given) {
    preopens[d] ??= realpathSync(d);
  }
  return preopens;
}

/**
 * The offset from UTC, in seconds, of the time zone the run's environment names (TZ, when it is
 * one Intl knows), else of the host's, now.
 */
export function utcOffset(env) {
  const now = new Date();
  const tz = typeof env.TZ === "string" ? env.TZ.replace(/^:/, "") : "";
  if (tz) {
    try {
      const parts = new Intl.DateTimeFormat("en-US", { timeZone: tz, timeZoneName: "longOffset" }).formatToParts(now);
      const name = parts.find((p) => p.type === "timeZoneName")?.value ?? "";
      const m = /^GMT(?:([+-])(\d{2}):(\d{2}))?$/.exec(name);
      if (m) {
        return m[1] ? (m[1] === "-" ? -1 : 1) * (Number(m[2]) * 3600 + Number(m[3]) * 60) : 0;
      }
    } catch {
      // a TZ Intl does not know: the host's
    }
  }
  return -now.getTimezoneOffset() * 60;
}

/** The caller's environment as WASI takes it (every value a string), with the loader's two. */
export function environment(env, cwd) {
  const out = {};
  for (const [k, v] of Object.entries(env ?? process.env)) {
    if (v !== undefined && v !== null) {
      out[k] = String(v);
    }
  }
  out.RITSU_WASI_CWD = cwd;
  out.RITSU_WASI_UTC_OFFSET = String(utcOffset(out));
  return out;
}

const EAGAIN = 6;
const pause = new Int32Array(new SharedArrayBuffer(4));

/**
 * A read or a write of WASI that waits while the descriptor has nothing to give or no room: a
 * pipe set non-blocking by a process that shares it (npm, a test runner, Node itself once it has
 * written to that pipe) answers EAGAIN where a blocking one would wait, and the module would stop
 * at the first full pipe.
 */
function patient(call) {
  return (...args) => {
    for (;;) {
      const errno = call(...args);
      if (errno !== EAGAIN) {
        return errno;
      }
      Atomics.wait(pause, 0, 0, 2);
    }
  };
}

const ENOTCAPABLE = 76;
const SYMLINK_FOLLOW = 1;
const utf8 = new TextDecoder("utf-8", { fatal: true });

/**
 * Where the host's open() of the path `p` lands: its real path, with every directory on the way
 * followed, and the last name too when `follow`; a name that is not there (yet), under the real
 * path of its directory. Null for links that go round.
 */
function landing(p, follow, links = 0) {
  if (links > 40) {
    return null;
  }
  if (follow) {
    try {
      return realpathSync.native(p);
    } catch {
      // not there, or a link to what is not there
    }
    let target;
    try {
      target = readlinkSync(p);
    } catch {
      // not a link
    }
    if (target !== undefined) {
      return landing(resolve(dirname(p), target), true, links + 1);
    }
  }
  const dir = dirname(p);
  if (dir === p) {
    return p;
  }
  const above = landing(dir, true, links);
  return above === null ? null : join(above, basename(p));
}

/**
 * WASI's functions that take a path, made to keep to the directories of `preopens` (`dirs`).
 * uvwasi, the WASI of Node, refuses a path that leaves the directory it is opened from by its
 * name (`../outside.proto`) and a symbolic link at its end that points out, but it leaves the
 * directories on the way to the host's open(): through a link in the project to a directory
 * outside it (`etc` to `/etc`), `etc/hosts` would be read. So each path is first followed here
 * as the host would follow it, and one that lands outside all of them is answered ENOTCAPABLE,
 * as uvwasi answers the others. This keeps what ritsu reads to `dirs`; it is no sandbox for a
 * module one does not trust (Node's WASI says it is none), and it trusts that what it followed
 * does not change before the host opens it.
 */
function guarded(own, preopens, memory) {
  const kept = Object.keys(preopens).map((d) => realpathSync.native(d));
  const inside = (r) => r !== null && kept.some((d) => r === d || r.startsWith(d.endsWith("/") ? d : `${d}/`));
  // the place of each directory the module has open: the preopens (from 3, in their order), and
  // what path_open opened; null where the guard could not tell, so that nothing under it is opened
  const places = new Map(Object.keys(preopens).map((d, i) => [3 + i, d]));
  const name = (ptr, len) => utf8.decode(new Uint8Array(memory().buffer, ptr, len));
  const may = (fd, ptr, len, follow) => {
    const place = places.get(fd);
    if (place === undefined) {
      return true; // no directory: uvwasi answers (EBADF, ENOTDIR)
    }
    let p;
    try {
      p = name(ptr, len);
    } catch (e) {
      return e instanceof RangeError; // outside the memory: uvwasi answers EFAULT; not UTF-8: refused
    }
    if (isAbsolute(p)) {
      return true; // uvwasi refuses it
    }
    // a slash at the end has the host follow a link at the end
    return place !== null && inside(landing(resolve(place, p), follow || /\/\.?$/.test(p)));
  };
  const follows = (flags) => (flags & SYMLINK_FOLLOW) !== 0;
  return {
    ...own,
    path_open(fd, dirflags, path, len, oflags, base, inheriting, fdflags, opened) {
      if (!may(fd, path, len, follows(dirflags))) {
        return ENOTCAPABLE;
      }
      const errno = own.path_open(fd, dirflags, path, len, oflags, base, inheriting, fdflags, opened);
      if (errno === 0) {
        const at = new DataView(memory().buffer).getUint32(opened, true);
        const place = places.get(fd);
        places.set(at, typeof place === "string" ? landing(resolve(place, name(path, len)), true) : null);
      }
      return errno;
    },
    fd_close(fd) {
      const errno = own.fd_close(fd);
      if (errno === 0) {
        places.delete(fd);
      }
      return errno;
    },
    fd_renumber(from, to) {
      const errno = own.fd_renumber(from, to);
      if (errno === 0) {
        if (places.has(from)) {
          places.set(to, places.get(from));
        } else {
          places.delete(to);
        }
        places.delete(from);
      }
      return errno;
    },
    path_filestat_get: (fd, flags, path, len, buf) => (may(fd, path, len, follows(flags)) ? own.path_filestat_get(fd, flags, path, len, buf) : ENOTCAPABLE),
    path_filestat_set_times: (fd, flags, path, len, atim, mtim, fst) =>
      may(fd, path, len, follows(flags)) ? own.path_filestat_set_times(fd, flags, path, len, atim, mtim, fst) : ENOTCAPABLE,
    path_readlink: (fd, path, len, buf, size, used) => (may(fd, path, len, false) ? own.path_readlink(fd, path, len, buf, size, used) : ENOTCAPABLE),
    path_create_directory: (fd, path, len) => (may(fd, path, len, false) ? own.path_create_directory(fd, path, len) : ENOTCAPABLE),
    path_remove_directory: (fd, path, len) => (may(fd, path, len, false) ? own.path_remove_directory(fd, path, len) : ENOTCAPABLE),
    path_unlink_file: (fd, path, len) => (may(fd, path, len, false) ? own.path_unlink_file(fd, path, len) : ENOTCAPABLE),
    path_rename: (fd, path, len, to, toPath, toLen) =>
      may(fd, path, len, false) && may(to, toPath, toLen, false) ? own.path_rename(fd, path, len, to, toPath, toLen) : ENOTCAPABLE,
    path_link: (fd, flags, path, len, to, toPath, toLen) =>
      may(fd, path, len, follows(flags)) && may(to, toPath, toLen, false) ? own.path_link(fd, flags, path, len, to, toPath, toLen) : ENOTCAPABLE,
    path_symlink: (target, targetLen, fd, path, len) => (may(fd, path, len, false) ? own.path_symlink(target, targetLen, fd, path, len) : ENOTCAPABLE),
  };
}

/**
 * One run of ritsu on this thread: `args` after the program's name `argv0` (`ritsu`, or a
 * language's, which makes it that language's command), in the directory `cwd` with the
 * environment `env` (from `environment`), reading and writing the descriptors given, opening the
 * directories of `preopens` (from `openedDirectories`; when they are not the whole filesystem,
 * nothing outside them, as `guarded` says). The exit code: the module's, or 2 when it
 * stops with a trap (a panic aborts there, after its hook has written what it says to the
 * standard error).
 */
export function start({ argv0 = "ritsu", args, env, stdin = 0, stdout = 1, stderr = 2, mod = module(), preopens = { "/": "/" } }) {
  const Wasi = wasiClass();
  const wasi = quietly(
    () =>
      new Wasi({
        version: "preview1",
        args: [argv0, ...args],
        env,
        preopens,
        returnOnExit: true,
        stdin,
        stdout,
        stderr,
      }),
  );
  const own = wasi.getImportObject().wasi_snapshot_preview1;
  let memory;
  const calls = Object.hasOwn(preopens, "/") ? { ...own } : guarded(own, preopens, () => memory);
  const imports = { wasi_snapshot_preview1: { ...calls, fd_read: patient(own.fd_read), fd_write: patient(own.fd_write) } };
  const instance = new WebAssembly.Instance(mod, imports);
  memory = instance.exports.memory;
  try {
    return wasi.start(instance);
  } catch (e) {
    if (e instanceof WebAssembly.RuntimeError || e instanceof RangeError) {
      // `unreachable` is an abort: a panic, or memory that ran out, whose words are written
      // already. Anything else (the stack of the engine used up) says what it was.
      if (!(e instanceof WebAssembly.RuntimeError && /unreachable/.test(e.message))) {
        writeSync(stderr, `ritsu: the WebAssembly module stopped: ${e.message}\n`);
      }
      return 2;
    }
    throw e;
  }
}

/** Close the descriptors, whatever happens to one. */
export function closeAll(fds) {
  for (const fd of fds) {
    try {
      closeSync(fd);
    } catch {
      // already closed
    }
  }
}
