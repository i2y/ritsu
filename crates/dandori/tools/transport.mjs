// A Transport for the code dandori writes for Temporal, durable functions and Argo, in place of
// Lambda, HTTP, the AWS APIs and OpenAI's agents: every call goes to `run.take(call, callbackId)`
// as Step Functions would send it (an agent's, as the generated code hands it over), which
// writes it down and gives the scenario's answer. Shared by tools/temporal/run.mjs,
// tools/durable/run.mjs, and the pods of tools/argo/run.mjs.
//
// spec.http: [ { method, url, errors: { <error>: <status> } } ]  (url with {placeholders})
// spec.aws:  [ { api: "<service>:<action>", errors: { <error>: <exception> }, keyParam } ]
//
// A callback task's submit hands on `callback_id` (in the Lambda payload, or in the SQS
// message); the call is written down without it, `take` gets the id, and `run.answerLater(id,
// answer)` is called (and awaited) with the answer the scenario gives the callback.
//
// An agent answers { "answer": <the scenario's value> }, as the model would under the schema;
// its failure is thrown, as the Agents SDK throws a refusal.
//
// A call the scenario times out goes to `run.late()`, when the runner has one: the runner keeps
// the call from answering until the platform times it out. Without one, it answers with the
// error "timeout".

export function makeTransport(spec, run) {
  const httpTasks = (spec.http ?? []).map((t) => ({
    ...t,
    re: new RegExp("^" + t.url.split(/\{[^}]*\}/).map((p) => p.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[^/?]*") + "$"),
  }));
  const awsTasks = spec.aws ?? [];

  function errorName(kind) {
    return kind === "failure" ? "Dandori.Test.Failure" : kind;
  }

  const late = (ans) => !("ok" in ans) && ans.error === "timeout" && typeof run.late === "function";

  return {
    async lambda(fn, payload) {
      const { callback_id, ...rest } = payload;
      const ans = await run.take({ lambda: fn, payload: rest }, callback_id);
      if (callback_id !== undefined) {
        await run.answerLater(callback_id, ans);
        return { ok: null };
      }
      if (late(ans)) return run.late();
      if ("ok" in ans) return { ok: ans.ok };
      return { error: errorName(ans.error), message: "scripted" };
    },
    async http(req) {
      const { form, ...w } = req;
      const ans = await run.take(w);
      if (late(ans)) return run.late();
      if ("ok" in ans) return { status: 200, body: ans.ok };
      const task = httpTasks.find((t) => t.method === req.http && t.re.test(req.url));
      const status = task?.errors?.[ans.error] ?? 500;
      return { status, body: "scripted" };
    },
    async aws(service, action, input) {
      const api = `${service}:${action}`;
      let args = input;
      let callbackId;
      if (api === "sqs:sendMessage" && input.MessageBody && typeof input.MessageBody === "object" && "callback_id" in input.MessageBody) {
        const { callback_id, ...body } = input.MessageBody;
        callbackId = callback_id;
        args = { ...input, MessageBody: body };
      }
      const ans = await run.take({ aws: api, args }, callbackId);
      if (callbackId !== undefined) {
        await run.answerLater(callbackId, ans);
        return { ok: { MessageId: "message-1" } };
      }
      if (late(ans)) return run.late();
      if ("ok" in ans) return { ok: ans.ok };
      const task = awsTasks.find((t) => t.api === api);
      return { error: task?.errors?.[ans.error] ?? errorName(ans.error), message: "scripted" };
    },
    async agent(call) {
      const ans = await run.take(call);
      if (late(ans)) return run.late();
      if ("ok" in ans) return { answer: ans.ok };
      const e = new Error("scripted");
      e.name = errorName(ans.error);
      throw e;
    },
  };
}
