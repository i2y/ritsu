// A Transport for the code dandori writes for Temporal and durable functions, in place of
// Lambda, HTTP and the AWS APIs: it writes down every call as Step Functions would send it,
// and answers from the scenario. Shared by tools/temporal/run.mjs and tools/durable/run.mjs.
//
// spec.http: [ { method, url, errors: { <error>: <status> } } ]  (url with {placeholders})
// spec.aws:  [ { api: "<service>:<action>", errors: { <error>: <exception> }, keyParam } ]
//
// A callback task's submit hands on `callback_id` (in the Lambda payload, or in the SQS
// message); the call is written down without it, and `answerLater(id, answer)` is called
// with the answer the scenario gives the callback.

export function makeTransport(spec, run) {
  const httpTasks = (spec.http ?? []).map((t) => ({
    ...t,
    re: new RegExp("^" + t.url.split(/\{[^}]*\}/).map((p) => p.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[^/?]*") + "$"),
  }));
  const awsTasks = spec.aws ?? [];

  function recorded(ans) {
    return "ok" in ans ? { ok: ans.ok } : { error: ans.error, as: ans.error };
  }

  function errorName(kind) {
    return kind === "failure" ? "Dandori.Test.Failure" : kind;
  }

  return {
    async lambda(fn, payload) {
      const { callback_id, ...rest } = payload;
      const ans = run.take(fn);
      run.steps.push({ call: { lambda: fn, payload: rest }, answer: recorded(ans) });
      if (callback_id !== undefined) {
        run.answerLater(callback_id, ans);
        return { ok: null };
      }
      if ("ok" in ans) return { ok: ans.ok };
      return { error: errorName(ans.error), message: "scripted" };
    },
    async http(req) {
      const { form, ...w } = req;
      const ans = run.take(`${req.http} ${req.url}`);
      run.steps.push({ call: w, answer: recorded(ans) });
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
      const ans = run.take(api);
      run.steps.push({ call: { aws: api, args }, answer: recorded(ans) });
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
