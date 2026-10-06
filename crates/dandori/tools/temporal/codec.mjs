// Runs a workflow whose history is encrypted (`history encrypted`, dandori's DESIGN 1.18) on the
// Temporal CLI's dev server, through the worker.ts and client.ts dandori generated for it, with a
// codec that writes every payload as base64 under an encoding of its own. Then it reads the history
// the server keeps with a client that has no codec, and says what it finds there: the encoding of the
// workflow's input, whether a value of the input shows in the clear anywhere in the history, and the
// message of the failure the second run ends with.
//
//   node tools/temporal/codec.mjs <generated dir> <results.json>
//
// The workflow is tests/encrypted/pay.flow: its one task, `pay`, is the user's, and fails with
// `refused` for an account whose number starts with 5500. Two runs: one that pays, one that is refused.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { TestWorkflowEnvironment } from "@temporalio/testing";
import { DefaultLogger, Runtime } from "@temporalio/worker";
import { ApplicationFailure } from "@temporalio/common";
import { Client, WorkflowFailedError } from "@temporalio/client";
import { workDir } from "../work.mjs";

Runtime.install({ logger: new DefaultLogger("WARN", (entry) => process.stderr.write(`${entry.level} ${entry.message}\n`)) });

const here = path.dirname(fileURLToPath(import.meta.url));
const [dir, outFile] = process.argv.slice(2);

/** The encoding this codec writes; the encoding a payload had is kept beside it. */
const ENCODING = "binary/dandori-test-base64";
const bytes = (s) => new TextEncoder().encode(s);
const text = (b) => new TextDecoder().decode(b);

const codec = {
  async encode(payloads) {
    return payloads.map((p) => ({
      metadata: { encoding: bytes(ENCODING), "dandori-encoding": p.metadata?.encoding ?? bytes("") },
      data: bytes(Buffer.from(p.data ?? new Uint8Array()).toString("base64")),
    }));
  },
  async decode(payloads) {
    return payloads.map((p) => {
      if (text(p.metadata?.encoding ?? new Uint8Array()) !== ENCODING) return p;
      return { metadata: { encoding: p.metadata["dandori-encoding"] }, data: new Uint8Array(Buffer.from(text(p.data), "base64")) };
    });
  },
};

// A copy of the generated code that Node runs by stripping the types: relative imports take their
// extension, and the failure converter is named by its path, which a worker's bundle needs.
const work = workDir("dandori-codec-");
for (const f of fs.readdirSync(dir)) {
  if (!f.endsWith(".ts")) continue;
  const t = fs
    .readFileSync(path.join(dir, f), "utf8")
    .replace(/from "(\.\/[^"]+)"/g, (_, p) => `from "${p}.ts"`)
    .replaceAll('require.resolve("./failure")', JSON.stringify(path.join(fs.realpathSync(work), "failure.ts")))
    .replaceAll('require.resolve("./workflow")', JSON.stringify(path.join(fs.realpathSync(work), "workflow.ts")));
  fs.writeFileSync(path.join(work, f), t);
}
fs.symlinkSync(path.join(here, "node_modules"), path.join(work, "node_modules"));
const bundlerOptions = {
  webpackConfigHook: (config) => {
    config.resolve = config.resolve ?? {};
    config.resolve.modules = [...(config.resolve.modules ?? ["node_modules"]), path.join(here, "node_modules")];
    config.resolve.extensionAlias = { ".js": [".ts", ".js"] };
    return config;
  },
};

const { makeWorker } = await import(path.join(work, "worker.ts"));
const { encryptedClient, start } = await import(path.join(work, "client.ts"));

const own = {
  async pay({ account }) {
    if (account.number.startsWith("5500")) throw ApplicationFailure.create({ type: "refused", message: "the account is closed", nonRetryable: true });
    return `receipt for ${account.id}`;
  },
};

const env = await TestWorkflowEnvironment.createLocal({ server: { ui: false, log: { format: "pretty", level: "error" } } });
const results = {};
try {
  const worker = await makeWorker(own, { codec, connection: env.nativeConnection, bundlerOptions });
  const client = encryptedClient(env.connection, codec);
  const plain = new Client({ connection: env.connection });
  await worker.runUntil(async () => {
    for (const [id, number] of [
      ["paid", "4111-1111-1111-1111"],
      ["refused", "5500-0000-0000-0004"],
    ]) {
      const handle = await start(client, id, { account: { id: `acct-${id}`, number } });
      let end;
      try {
        end = { succeed: await handle.result() };
      } catch (e) {
        if (!(e instanceof WorkflowFailedError)) throw e;
        end = { fail: { error: e.cause?.type ?? null, cause: e.cause?.message ?? null } };
      }
      // the history as the server keeps it, read with no codec
      const history = await plain.workflow.getHandle(id).fetchHistory();
      const started = history.events[0].workflowExecutionStartedEventAttributes;
      const failed = history.events.at(-1).workflowExecutionFailedEventAttributes;
      const kept = JSON.stringify(history, (_, v) => (v instanceof Uint8Array ? text(v) : v));
      results[id] = {
        end,
        input_encoding: text(started.input.payloads[0].metadata.encoding),
        number_in_the_clear: kept.includes(number),
        failure_message: failed?.failure?.message ?? null,
        failure_encoded: Boolean(failed?.failure?.encodedAttributes),
      };
    }
  });
} finally {
  await env.teardown();
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
fs.rmSync(work, { recursive: true, force: true });
