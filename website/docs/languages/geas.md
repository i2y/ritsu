# geas

`.geas` — claims about the code an agent wrote.

**geas** runs each claim about a program against it — a command line, an HTTP service, a page in a browser — and, given a diff, tells which claims it touches. Given an OpenSpec spec, it lists the scenarios that no claim of the same name checks.

A claim can wait for an answer that comes later, such as a job a service finishes after it answers, for as long as the claim says.

A service with an OpenAPI document can be held to it with one line on its target: every answer's status, required headers, media type and body are checked against the document, and a call the document does not have stops the spec before anything runs.

A claim calls with `get`, `post`, `put`, `patch` and `delete`, sends headers, and goes on from what a service answers: `keep body json ".id" as id` holds the number a service chose, and `{id}` is the path of the next call. A token read with `keep env` is shown nowhere. The journal and the baseline write these values as `{id}` and `{token}`, so they stay the same from run to run, and drift on new numbers is quiet.

geas's documents are in the repository: [README](https://github.com/i2y/ritsu/blob/main/crates/geas/README.md).
