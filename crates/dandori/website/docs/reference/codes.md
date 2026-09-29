# Diagnostics

`dandori check` finds all of these but E040 and E050, which `dandori build` finds for the platform
it builds for. An error stops the build; a warning (W) does not. Each diagnostic comes with a run that
gets there, and `--lang ja` gives the same in Japanese.

| Code | What it finds |
|---|---|
| E001 | a syntax error |
| E002 | a name that is not there: a type, a variable, a field, a rule or a task (a unit of the wrong kind, too) |
| E003 | types that do not match: a value that may be absent used as it is, `none` where it cannot go, a list of lists, a `{…}` or `[]` whose type cannot be told, a range on what is not a number, a range no number is in |
| E004 | too many or too few arguments or outputs, a field left out of a `{…}` record |
| E005 | a rule rulec could not read |
| E006 | a name declared twice |
| E007 | a task's clauses that do not go together: two ways of calling, a `flow` task with another one or with an `image`, `form` on a call to an OpenAPI operation, a Connect error code that is not one, an HTTP error without its status or two errors of one status, a `key` or a `callback` the way of calling cannot have (an agent takes no `key`), an AWS service it does not know, `model` without an `agent`, an agent without the type of its answer or with declared errors, an answer its schema cannot say, a provider it does not know, enum values that differ only in case for Claude, an `event` task's parameters, calls, `retry` or `key`, or one that starts a case, a `url` on a task that is not an agent or on Claude's, a `url` that is not http or https, an `effort` on a task that is not an agent or at a level its provider does not take; for Jev, an answer it cannot give (not an enum, a `bool` or a record of them), a record's field left unasked or asked twice, a meaning for a value the enum does not have, a `score` without the meaning of every level or with more than 10, what only `true` or only `false` means, a confidence that goes into what is not a rate of whole steps, `confidence` without `jev` or with `failure` or a declared error, a yes or no floor of 0.5 or less, `retry` on the error of `confidence`, an error without its status, `key`, `url`, `effort` |
| E008 | a case declared wrong, or a task that does to a case what it cannot |
| E009 | a statement where it cannot be: a `yield` that is not the last line of the body of `let <name> = for …`, or such a `for` without one; `break`, `succeed`, a case's call or an event's wait in a round of `for … in parallel`; `succeed` in `on failure` or `on cancel`; a variable given a value both inside a round and outside; a rule called without `let` |
| E010 | a value that no arm of a `match` takes |
| E011 | an arm that can never be taken |
| E012 | a variable read where it may have no value yet |
| E013 | a task called on a case that has not started, or a case started twice |
| E014 | a value that can be outside the range where it goes: a rule's input, a task's parameter, a field of a record written out, an output |
| E015 | a task that runs another `.flow` and does not fit it: its parameters and the child's inputs, its answer and the child's outputs, its errors and the child's `fail`s (a child that cannot be read or does not pass the checks, and a flow that runs itself, too) |
| E016 | a task that does not fit the API description it calls: an operation that is not there, a parameter it does not take or one it needs left out, a type, range or enum that differs, a field the answer may leave out that is not `T?`, a status or an exception it does not answer with, a `key` that is not its idempotency token, a method that streams (and a description that cannot be read, or a `.proto` without `url`) |
| E020 | the workflow can end with a case in a state that is not final (also when `on cancel` ends it as cancelled) |
| E021 | an event sent that every state refuses |
| E022 | an event sent that can be refused, with nothing to handle the refusal |
| E030 | a call that changes the other side, retried without a `key` |
| E031 | what an Express workflow cannot do: a wait longer than five minutes, a callback, a nested execution, a call that changes the other side without a `key` |
| E040 | one run can grow too large for the platform: its history over the limit (25,000 events on Step Functions, 51,200 on Temporal, 3,000 operations on Lambda durable functions), or on Argo Workflows more than 10,000 nodes |
| E050 | what the platform needs is missing, or the platform cannot do it: on Step Functions, a way of calling or a `connection` (an agent's and Jev's too), a nested execution's declared errors, a `timeout` over 60 seconds on `http`, `agent` and `jev`, a destination that is not HTTPS; off Temporal, `on cancel` and `event` tasks; on Step Functions and Lambda durable functions, a called rule's `lambda`; on Lambda durable functions, a `timeout` on a function it invokes; on Argo, a way of calling or an `image`, a `workflow template`'s declared errors, `retry` on a `callback` task |
| W030 | a call that may change the other side, retried without a `key` |
| W032 | a Jev task that sets or reads a confidence, and names an alias of the model (`jev-latest`, `jev-preview`) rather than its version |
| W101 | an error nothing handles can fail the run with a case in a state that is not final (while `on failure` or `on cancel` settles cases, too) |
| W102 | an `on <refusal>` that can never happen |
| W103 | a task that starts a case without a `key` |
| W104 | a value whose range nothing says, where a range is |

[What it checks](../checks.md) says what the checker looks at, with examples.
