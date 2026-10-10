// The types of @i2y/ritsu (lib/index.js).

/** The version of ritsu the module was built from, such as "0.26.0". */
export const version: string;

/** The absolute path of ritsu.wasm in the package. */
export const wasmPath: string;

export interface RunOptions {
  /** The directory the command works in; else `process.cwd()`. One that is not there is an error. */
  cwd?: string;
  /** The environment; else `process.env`. Given, it replaces it, as `child_process` does. */
  env?: Record<string, string | undefined>;
  /** What the command reads from its standard input; else nothing (`-` reads an end at once). */
  stdin?: string | Uint8Array;
  /**
   * The only directories the module can open, each at its own path (WASI preopens); a relative one
   * is taken from `cwd`. Else ["/"]: the whole filesystem, as the native binary. `cwd` has to be
   * inside one of them. A path that leads out of them, by its name or through a symbolic link (to a
   * file, or to a directory on the way), can be neither read nor written: the command says it
   * cannot read it. This narrows what ritsu (code you trust) reads, so that a file of a project
   * cannot pull in one outside it; it is not a sandbox for code you do not trust, which Node's WASI
   * does not claim to be.
   */
  dirs?: string[];
}

export interface RunResult {
  /** The exit code, as the native binary's: 0 no errors, 1 errors, 2 bad arguments or a file that cannot be read; 2 too when the module stops with a trap. */
  code: number;
  stdout: string;
  stderr: string;
}

/**
 * Run `ritsu <args>` in a worker thread: the event loop goes on while it runs, and any number of
 * runs may go at once. `args` leaves out the program's name: `["check", ".", "--format", "json"]`,
 * and a language's command as `["rulec", "doc", "x.rule"]`.
 */
export function run(args: string[], options?: RunOptions): Promise<RunResult>;

/** Run `ritsu <args>` on the thread that calls it. */
export function runSync(args: string[], options?: RunOptions): RunResult;
