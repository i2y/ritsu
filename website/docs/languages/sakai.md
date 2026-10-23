# sakai

`.ctx` — the map of bounded contexts.

**sakai** maps which context owns which files and who may depend on whom, and checks the references that cross contexts, in the files of every language here, in the OpenAPI and AsyncAPI documents services keep as their contracts, and in the code. It does not count a `use book`, since another context reaches a book through the service in front of it.

sakai's documents are in the repository: [README](https://github.com/i2y/ritsu/blob/main/crates/sakai/README.md), [the reference](https://github.com/i2y/ritsu/blob/main/crates/sakai/docs/reference.md), [the targets](https://github.com/i2y/ritsu/blob/main/crates/sakai/docs/targets.md), [the codes of its diagnostics](https://github.com/i2y/ritsu/blob/main/crates/sakai/docs/codes.md).
