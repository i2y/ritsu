// Sends calls of the generated workflows through the default Transport that dandori writes in
// TypeScript (io.ts: fetch, and the AWS SDK for JavaScript v3) to stand-ins on this machine, and
// writes down what arrived and what the Transport gave back. A server here answers HTTP and
// Lambda's Invoke as each case scripts, and hands every other AWS API to moto (a server that
// plays the AWS APIs, at the address given). Nothing leaves the machine: the server's address
// takes the place of the scheme and the host of an HTTP task's URL, and the AWS SDK's endpoint
// is the server. The Python twin is check.py.
//
//   node tools/wire/check.mjs <generated dir> <cases.json> <results.json> <moto's address>
//
// cases.json, made by tests/examples.rs from the scenarios' calls:
//   { "kind": "http", "request": { the HttpRequest }, "reply": { "status", "body" } }
//   { "kind": "lambda", "fn", "payload", "reply": { "ok" } | { "error", "message" } }
//   { "kind": "aws", "service", "action", "input", "topics": [name], "queues": [name],
//     "listen": { "queue", "topic"? } | null, "setup"?: [{ "method", "path", "headers", "body" }] }
// results.json, one for each case: { "received": the request as it arrived (HTTP, Lambda),
//   "returned": what the Transport gave back, "messages": what the listening queue got,
//   "moto": [{ "method", "path", "body", "answer" }], the requests that went to moto and its answers }

import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { SNS } from "@aws-sdk/client-sns";
import { SQS } from "@aws-sdk/client-sqs";
import { workDir } from "../work.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const [dir, casesFile, outFile, moto] = process.argv.slice(2);
const cases = JSON.parse(fs.readFileSync(casesFile, "utf8"));
const REGION = "ap-northeast-1";
const ACCOUNT = "123456789012";
const credentials = { accessKeyId: "wire", secretAccessKey: "wire" };

// io.ts beside the AWS SDK's clients, which it loads when a call first needs one
const work = workDir("dandori-wire-");
fs.copyFileSync(path.join(dir, "io.ts"), path.join(work, "io.ts"));
fs.symlinkSync(path.join(here, "node_modules"), path.join(work, "node_modules"));
const io = await import(path.join(work, "io.ts"));

/** What the case being sent scripts, and what arrived for it. */
let now = null;

function pairs(text) {
  return [...new URLSearchParams(text)];
}

const motoUrl = new URL(moto);
const server = http.createServer(async (req, res) => {
  const chunks = [];
  for await (const c of req) chunks.push(c);
  const body = Buffer.concat(chunks);
  const url = new URL(req.url, "http://here");
  const lambda = url.pathname.match(/^\/2015-03-31\/functions\/([^/]+)\/invocations$/);
  if (url.pathname.startsWith("/http/")) {
    // /http/<host><path>: an HTTP task's request
    const type = req.headers["content-type"] ?? "";
    const text = body.toString("utf8");
    // Node reads a header's bytes as latin-1; the Transports send each value's UTF-8
    const headers = Object.fromEntries(Object.entries(req.headers).map(([k, v]) => [k, typeof v === "string" ? Buffer.from(v, "latin1").toString("utf8") : v]));
    const at = req.url.indexOf("?");
    now.received = {
      method: req.method,
      path: url.pathname.slice("/http".length).replace(/^\/[^/]+/, ""),
      query: pairs(url.search),
      headers,
      body: text === "" ? null : type.includes("json") ? JSON.parse(text) : type.includes("x-www-form-urlencoded") ? pairs(text) : text,
      raw: text,
      // the query as it arrived, before a URL's parser writes it again
      rawQuery: at < 0 ? "" : req.url.slice(at),
    };
    const r = now.case.reply;
    const json = typeof r.body !== "string";
    res.writeHead(r.status, { "Content-Type": json ? "application/json" : "text/plain; charset=utf-8" });
    res.end(json ? JSON.stringify(r.body) : r.body);
  } else if (lambda && req.method === "POST") {
    now.received = { fn: decodeURIComponent(lambda[1]), invocationType: req.headers["x-amz-invocation-type"] ?? null, payload: JSON.parse(body.toString("utf8")), raw: body.toString("utf8") };
    const r = now.case.reply;
    if ("ok" in r) {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify(r.ok));
    } else {
      res.writeHead(200, { "Content-Type": "application/json", "X-Amz-Function-Error": "Unhandled" });
      res.end(JSON.stringify({ errorType: r.error, errorMessage: r.message }));
    }
  } else {
    // any other AWS API: moto's, and what went and came back written down
    const headers = { ...req.headers, host: motoUrl.host };
    const out = http.request({ host: motoUrl.hostname, port: motoUrl.port, method: req.method, path: req.url, headers }, async (m) => {
      const parts = [];
      for await (const c of m) parts.push(c);
      const answer = Buffer.concat(parts);
      now?.moto.push({ method: req.method, path: req.url, body: body.toString("utf8"), answer: answer.toString("utf8") });
      res.writeHead(m.statusCode, m.headers);
      res.end(answer);
    });
    out.end(body);
  }
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
/** This server's address, in place of an HTTP task's scheme and host, and the AWS SDK's endpoint. */
const front = `http://127.0.0.1:${server.address().port}`;

// a header of the options' own; for the library of tests/flows/http_places.flow, one that takes the
// place of the request's of its name, which it gives in another case; and a cookie that joins the
// request's (DESIGN 43)
const transport = io.transport({
  headers: (url) => ({ "X-Dandori-Check": "wire", ...(url.includes("/library.example.com/") ? { "x-library-key": "wire-key" } : {}), Cookie: "wire_session=1" }),
  aws: { endpoint: front, region: REGION, credentials },
});
const sns = new SNS({ endpoint: moto, region: REGION, credentials });
const sqs = new SQS({ endpoint: moto, region: REGION, credentials });

/** The queue's URL on moto. */
async function queueUrl(name) {
  return (await sqs.getQueueUrl({ QueueName: name })).QueueUrl;
}

const results = [];
try {
  for (const c of cases) {
    now = { case: c, received: null, moto: [] };
    let returned;
    let messages = null;
    if (c.kind === "http") {
      const u = new URL(c.request.url);
      const req = { ...c.request, url: `${front}/http/${u.host}${u.pathname}${u.search}` };
      returned = await transport.http(req);
    } else if (c.kind === "lambda") {
      returned = await transport.lambda(c.fn, c.payload);
    } else {
      // moto as the case wants it: the topics and queues that exist, and a queue that listens
      await fetch(`${moto}/moto-api/reset`, { method: "POST" });
      // what else the call needs there, as the case's requests to moto make it
      for (const r of c.setup ?? []) {
        const made = await fetch(`${moto}${r.path}`, { method: r.method, headers: r.headers, body: r.body });
        if (!made.ok) throw new Error(`moto did not take ${r.method} ${r.path}: ${made.status} ${await made.text()}`);
      }
      for (const name of c.topics) await sns.createTopic({ Name: name });
      for (const name of c.queues) await sqs.createQueue({ QueueName: name });
      if (c.listen?.topic) {
        await sqs.createQueue({ QueueName: c.listen.queue });
        await sns.subscribe({
          TopicArn: `arn:aws:sns:${REGION}:${ACCOUNT}:${c.listen.topic}`,
          Protocol: "sqs",
          Endpoint: `arn:aws:sqs:${REGION}:${ACCOUNT}:${c.listen.queue}`,
          Attributes: { RawMessageDelivery: "true" },
        });
      }
      returned = await transport.aws(c.service, c.action, c.input);
      if (c.listen) {
        const got = await sqs.receiveMessage({ QueueUrl: await queueUrl(c.listen.queue), MaxNumberOfMessages: 10 });
        messages = (got.Messages ?? []).map((m) => m.Body);
      }
    }
    results.push({ received: now.received, returned, messages, moto: now.moto });
  }
} finally {
  server.close();
  fs.rmSync(work, { recursive: true, force: true });
}
fs.writeFileSync(outFile, JSON.stringify(results, null, 2) + "\n");
