// A tally kept in memory, over HTTP: `node server.ts <port>`.
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";

let total: number = 0;

function send(res: ServerResponse, status: number, value: object): void {
  const body = JSON.stringify(value);
  res.setHeader("cache-control", "no-store");
  res.writeHead(status, { "content-type": "application/json" });
  res.end(body);
}

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve) => {
    let text = "";
    req.on("data", (chunk: Buffer) => (text += chunk.toString()));
    req.on("end", () => resolve(text));
  });
}

async function handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
  if (req.method === "GET" && req.url === "/total") {
    send(res, 200, { total });
  } else if (req.method === "POST" && req.url === "/add") {
    const text = (await readBody(req)).trim();
    if (!/^-?\d+$/.test(text)) {
      send(res, 400, { error: `not a number: ${text}` });
      return;
    }
    total += Number(text);
    send(res, 200, { total });
  } else if (req.method === "POST" && req.url === "/reset") {
    total = 0;
    send(res, 200, { total });
  } else {
    send(res, 404, { error: "not found" });
  }
}

const port: number = Number(process.argv[2] ?? 8124);
createServer((req, res) => {
  handle(req, res).catch(() => send(res, 500, { error: "internal" }));
}).listen(port, "127.0.0.1");
