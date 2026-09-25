// A Transport for the code dandori writes for Temporal, durable functions and Argo, in place of
// Lambda, HTTP and the AWS APIs: every call goes to `run.take(call, callbackId)` as Step
// Functions would send it, which writes it down and gives the scenario's answer. Shared by
// tools/temporal/run.mjs, tools/durable/run.mjs, and the pods of tools/argo/run.mjs.
//
// spec.http: [ { method, url, errors: { <error>: <status> } } ]  (url with {placeholders})
// spec.aws:  [ { api: "<service>:<action>", errors: { <error>: <exception> }, keyParam } ]
//
// A callback task's submit hands on `callback_id` (in the Lambda payload, or in the SQS
// message); the call is written down without it, `take` gets the id, and `run.answerLater(id,
// answer)` is called with the answer the scenario gives the callback.

export function makeTransport(spec, run) {
  const httpTasks = (spec.http ?? []).map((t) => ({
    ...t,
    re: new RegExp("^" + t.url.split(/\{[^}]*\}/).map((p) => p.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[^/?]*") + "$"),
  }));
  const awsTasks = spec.aws ?? [];

  function errorName(kind) {
    return kind === "failure" ? "Dandori.Test.Failure" : kind;
  }

  return {
    async lambda(fn, payload) {
      const { callback_id, ...rest } = payload;
      const ans = await run.take({ lambda: fn, payload: rest }, callback_id);
      if (callback_id !== undefined) {
        run.answerLater(callback_id, ans);
        return { ok: null };
      }
      if ("ok" in ans) return { ok: ans.ok };
      return { error: errorName(ans.error), message: "scripted" };
    },
    async http(req) {
      const { form, ...w } = req;
      const ans = await run.take(w);
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
        run.answerLater(callbackId, ans);
        return { ok: { MessageId: "message-1" } };
      }
      if ("ok" in ans) return { ok: ans.ok };
      const task = awsTasks.find((t) => t.api === api);
      return { error: task?.errors?.[ans.error] ?? errorName(ans.error), message: "scripted" };
    },
  };
}
