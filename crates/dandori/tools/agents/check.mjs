// Runs the agent call of the default Transport that dandori writes for TypeScript (io.ts) with
// OpenAI's Agents SDK, and a scripted model in place of OpenAI's, so that nothing leaves the
// machine. For each case it writes down what the model was asked (the model's name, the
// instructions, the input, the schema of the output, the model settings, the tools) and what
// the Transport answered, or the error it threw.
//
//   node tools/agents/check.mjs <io.ts> <cases.json> <results.json>
//
// cases.json: [ { "call": <AgentCall>, "text": <the model's answer> } | { "call", "refusal": <text> } ]

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ScriptedModel, assistantMessage } from "@openai/agents/testing";

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
