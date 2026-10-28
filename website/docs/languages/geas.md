# geas

`.geas` — claims about the code an agent wrote.

**geas** runs each claim about a program against it — a command line, an HTTP service, a page in a browser — and, given a diff, tells which claims it touches. Given an OpenSpec spec, it lists the scenarios that no claim of the same name checks.

A claim can wait for an answer that comes later, such as a job a service finishes after it answers, for as long as the claim says.

geas's documents are in the repository: [README](https://github.com/i2y/ritsu/blob/main/crates/geas/README.md).
