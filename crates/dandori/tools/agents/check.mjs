// Runs the agent call of the default Transport that dandori writes for TypeScript (io.ts), so
// that nothing leaves the machine: OpenAI's agents with OpenAI's Agents SDK and a scripted model
// in place of OpenAI's, Claude's with Anthropic's SDK and a stand-in of the Messages API on
// 127.0.0.1. For each case it writes down what the model was asked — for OpenAI's, the model's
// name, the instructions, the input, the schema of the output, the model settings and the
// tools; for Claude's, every request the stand-in got — and what the Transport answered, or the
// error it threw. A case with a `status` has the stand-in answer every request with that error
// status, to see that the SDK's client does not retry by itself; an OpenAI agent's then goes to
// a stand-in of the Responses API, through the client the Transport makes (OPENAI_BASE_URL). An
// agent on another server of Open Responses (a call with `url`) goes to a stand-in of that
// server on 127.0.0.1, at the same path, which writes down every request; a case that says
// `live` goes to the server the call names, as it is (a model on Ollama on this machine).
//
//   node tools/agents/check.mjs <io.ts> <cases.json> <results.json>
//
// cases.json: [ { "call": <AgentCall>, "text": <the model's answer> } | { "call", "refusal": <text> }
//               | { "call", "status": <an HTTP error status the stand-in answers with> }
//               | { "call", "live": true } ]

import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ScriptedModel, assistantMessage } from "@openai/agents/testing";

/** The Messages API's answer: the text, and how the turn ended; a refusal has no content, as the API reference shows it. */
function message(model, text, stop) {
  const refused = stop === "refusal";
  return {
    id: "msg_test",
    type: "message",
    role: "assistant",
    model,
    content: refused ? [] : [{ type: "text", text }],
    stop_reason: stop,
    stop_sequence: null,
    stop_details: refused ? { type: "refusal", category: null, explanation: null } : null,
    usage: { input_tokens: 10, output_tokens: 10 },
  };
}

/** Run `f` with the address of a stand-in of the Messages API that answers every request with `reply`. */
async function standIn(reply, f) {
  const asked = [];
  const server = http.createServer((req, res) => {
    let body = "";
    req.on("data", (d) => (body += d));
    req.on("end", () => {
      asked.push({ method: req.method, path: req.url, version: req.headers["anthropic-version"], body: body ? JSON.parse(body) : null });
      res.writeHead(reply.status, { "content-type": "application/json" });
      res.end(JSON.stringify(reply.body));
    });
  });
  await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
  try {
    return { asked, out: await f(`http://127.0.0.1:${server.address().port}`) };
  } finally {
    server.close();
  }
}

/** An OpenAI agent against a stand-in of the Responses API that answers with an error status. */
async function openaiFailing(io, c) {
  const reply = { status: c.status, body: { error: { message: "scripted", type: "server_error" } } };
  const { asked, out } = await standIn(reply, async (url) => {
    process.env.OPENAI_BASE_URL = `${url}/v1`;
    process.env.OPENAI_API_KEY = "test";
    const transport = io.transport({ agents: { tracingDisabled: true } });
    try {
      return { answer: await transport.agent(c.call) };
    } catch (e) {
      return { error: e?.constructor?.name ?? String(e) };
    }
  });
  return { ...out, asked: asked.map(({ method, path }) => ({ method, path })) };
}

/** The Responses API's answer, as a server of Open Responses gives it: a message whose content is `content`. */
function response(model, content) {
  return {
    id: "resp_test",
    object: "response",
    status: "completed",
    model,
    output: [{ id: "msg_test", type: "message", role: "assistant", status: "completed", content: [content] }],
  };
}

/** An agent on a server of Open Responses: a stand-in of it at the call's path, or, `live`, the server itself. */
async function openResponses(io, c) {
  const transport = io.transport();
  const run = async (call) => {
    try {
      return { answer: await transport.agent(call) };
    } catch (e) {
      return { error: e?.constructor?.name ?? String(e) };
    }
  };
  if (c.live) return run(c.call);
  const reply =
    c.status !== undefined
      ? { status: c.status, body: { error: { message: "scripted", type: "server_error" } } }
      : { status: 200, body: c.refusal !== undefined ? response(c.call.model, { type: "refusal", refusal: c.refusal }) : response(c.call.model, { type: "output_text", text: c.text, annotations: [] }) };
  const { asked, out } = await standIn(reply, (url) => run({ ...c.call, url: url + new URL(c.call.url).pathname }));
  return { ...out, asked: asked.map(({ method, path, body }) => ({ method, path, body })) };
}

async function claude(io, c) {
  const reply =
    c.status !== undefined
      ? { status: c.status, body: { type: "error", error: { type: "api_error", message: "scripted" } } }
      : { status: 200, body: c.refusal !== undefined ? message(c.call.model, c.refusal, "refusal") : message(c.call.model, c.text, "end_turn") };
  const { asked, out } = await standIn(reply, async (url) => {
    const transport = io.transport({ claude: { baseURL: url, apiKey: "test" } });
    try {
      return { answer: await transport.agent(c.call) };
    } catch (e) {
      return { error: e?.constructor?.name ?? String(e) };
    }
  });
  return { ...out, asked };
}

const here = path.dirname(fileURLToPath(import.meta.url));
const [ioFile, casesFile, outFile] = process.argv.slice(2);
const cases = JSON.parse(fs.readFileSync(casesFile, "utf8"));

// io.ts imports the SDK when an agent is called; let it find this directory's copy
const work = fs.mkdtempSync(path.join(os.tmpdir(), "dandori-agents-"));
fs.copyFileSync(ioFile, path.join(work, "io.ts"));
fs.symlinkSync(path.join(here, "node_modules"), path.join(work, "node_modules"));
const io = await import(path.join(work, "io.ts"));

const results = [];
try {
  for (const c of cases) {
    if (c.call.url !== undefined) {
      results.push(await openResponses(io, c));
      continue;
    }
    if (c.call.provider === "claude") {
      results.push(await claude(io, c));
      continue;
    }
    if (c.status !== undefined) {
      results.push(await openaiFailing(io, c));
      continue;
    }
    const message =
      c.refusal !== undefined
        ? { type: "message", role: "assistant", status: "completed", content: [{ type: "refusal", refusal: c.refusal }] }
        : assistantMessage(c.text);
    const model = new ScriptedModel([[message]]);
    const names = [];
    const provider = { getModel: (name) => (names.push(name), model) };
    const transport = io.transport({ agents: { modelProvider: provider, tracingDisabled: true } });
    const out = {};
    try {
      out.answer = await transport.agent(c.call);
    } catch (e) {
      out.error = e instanceof Error ? e.name : String(e);
    }
    const req = model.calls[0]?.request;
    if (req) {
      out.asked = {
        models: names,
        calls: model.calls.length,
        instructions: req.systemInstructions,
        input: req.input,
        outputType: req.outputType,
        modelSettings: req.modelSettings,
        tools: req.tools,
      };
    }
    results.push(out);
  }
} finally {
  fs.rmSync(work, { recursive: true, force: true });
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
