# geas

`.geas` — claims about the code an agent wrote.

**geas** runs each claim about a program against it — a command line, an HTTP service, a page in a browser — and, given a diff, tells which claims it touches. Given an OpenSpec spec, it lists the scenarios that no claim of the same name checks.

A claim can wait for an answer that comes later, such as a job a service finishes after it answers, for as long as the claim says.

A service with an OpenAPI document can be held to it with one line on its target: every answer's status, required headers, media type and body are checked against the document, and a call the document does not have stops the spec before anything runs.

A claim calls with `get`, `post`, `put`, `patch` and `delete`, sends headers, and goes on from what a service answers: `keep body json ".id" as id` holds the number a service chose, and `{id}` is the path of the next call. A token read with `keep env` is shown nowhere. The journal and the baseline write these values as `{id}` and `{token}`, so they stay the same from run to run, and drift on new numbers is quiet.

A claim can send its calls in every order. `in any order { … }` holds the steps whose order is not decided, such as the events a payment provider may deliver in another order than they happened, or twice; geas runs the claim once for each order, each on a new service, so a handler that writes an older event over a newer one fails in the orders that end on it. geas makes one call at a time, so a handler that goes wrong only when two events are handled at once is not caught.

geas's documents are in the repository: [README](https://github.com/i2y/ritsu/blob/main/crates/geas/README.md).
