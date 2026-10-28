# jobs

A job service in Python's standard library ([server.py](server.py)). `POST /jobs` with a whole
number starts a job that squares it and answers at once, `202` and `"state": "running"`, as a
service that hands its work to a queue does; the job finishes half a second later, and
`GET /jobs/<id>` answers its state, and its result once it is done. [jobs.geas](jobs.geas) holds
four claims, and one of them waits:

```geas
target api {
  serve "python3 server.py {port}"
  port auto
}
…
claim "a job finishes in the background" {
  when api.post("/jobs", body: "12")
  then status is 202
  when api.get("/jobs/1") within 3 seconds
  then body json ".state" is "done"
  and  body json ".result" is 144
}
```

```console
$ geas check examples/jobs/jobs.geas
```

It needs python3. `within 3 seconds` makes geas call `get("/jobs/1")` again, every 100 ms, until
both checks under it hold on one answer, and for up to 3 seconds from the first call. Here they hold
after about half a second; the claim reads that answer, and the journal writes it with
`"within":3` and nothing of how many calls it took, so the same service gives the same bytes in
every run, and drift compares the answer the wait kept.

What to look for in the claims file:

- **The wait is on the read.** Each call is made again as it is written, so a `post` that waits
  starts a job on every call. The claim posts once and waits on the `get`.
- **Every check under the `when` is waited for.** One answer has to have both the state and the
  result; `then status is 202` belongs to the `post` above it, and is read once.
- **The claims that do not wait.** The other three read the first answer, as any `when` does.

[tests/changes/jobs/after/server.py](../../tests/changes/jobs/after/server.py) is the service after
an agent renamed the state of a finished job to `"finished"`. Copied over `server.py`, it fails the
claim that waits: no answer in the 3 seconds has `"done"`, and the checks read the last one.

```text
not ok 2 - a job finishes in the background
    examples/jobs/jobs.geas:20: within 3 seconds — no answer held every check under it (30 calls in 3.0 s); the checks below read the last one
        20 |   when api.get("/jobs/1") within 3 seconds
    examples/jobs/jobs.geas:21: body json ".state" is "done" — got "finished"
        21 |   then body json ".state" is "done"
      the run that gets there:
          18  when api.post("/jobs", body: "12")  →  202, body "{\"id\": \"1\", \"state\": \"running\"}"
          20  when api.get("/jobs/1") within 3 seconds  →  200, body "{\"id\": \"1\", \"state\": \"finished\", \"result\": 144}"
```

The journal writes that wait with the calls and the time, `"within":3,"tries":30,"ms":3005`, since
how long it waited is part of what happened. The top-level [README](../../README.md#an-answer-that-comes-later)
shows the same run.
