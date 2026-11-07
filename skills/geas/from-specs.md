# Proposing claims from what the project already says

Claims written by hand hold what their author thought of. A project in ritsu's languages has already
said much more: who may do what, where a rule changes its answer, what a book refuses, every way a
workflow can go. Each of those files can list what it decides, and each entry can be a claim, held
against the running service from outside. This page is how to get from those lists to claims a person
can accept.

Everything here is a proposal (SKILL.md §1). Write it in a file of its own, `<name>.proposed.geas`,
beside the claims file, run it, and show it to the person with what §4 says. Do not add it to their
claims file until they say so.

Use this when the person asks for more claims than they have, or when `ritsu geas affected` keeps
finding code no claim runs and the project has specs that say what that code should do. If the
project has no such files, write claims from what the person asks for instead.

## 1. Before any claim

**Hold the answers first.** If the service has an OpenAPI document, propose a `contract` line on its
target ([language.md](language.md)). It costs one line, and every claim below then checks the whole
answer, not only what it names. Run the existing claims with it first, in a copy: anything it says now
is a disagreement to show the person, and you want those apart from what your claims find. Once the
line is there, an answer the document does not list fails its claim even when the claim expects it, and
a call it does not have stops the file (E016). Calls like those go through a second target, the same
service with no `contract` line.

**Work out how to drive the service from outside.** The existing claims, the README and the OpenAPI
document say how. Write down, for yourself, three small tables:

- **Who.** Each kind of caller the gate speaks of, and the token or login that plays it.
- **Which state.** Each state a resource can be in, and the calls that reach it from the start of a
  claim, on a fresh service.
- **Where it shows.** Each answer a rule or a book gives, and the member of which response shows it.

What a fresh service starts with (its invoices, its users) can be read through its API as set-up: run a
throwaway claim and read `.geas/<name>.journal.jsonl`, which keeps bodies whole. Whatever cannot be
reached from outside goes on the list of what was not reached (§4). Do not seed a database or call a
function to get there: a claim says what anyone could see from outside.

**The expected answer comes from the spec.** Take it from the command's output. Never run the service
and copy what it says, and never read the hand-written code to decide what it should say. That is what
makes a claim independent of the code it checks.

## 2. What each file gives

| File | Ask | Each entry is | The claim |
|---|---|---|---|
| `.gate` | `ritsu sekisho vectors <file.gate> [--action <action>]` | a caller, a resource and a context, with the `decision` | reach that state, make the call as that caller; `allow` is the success status, `deny` is the status the service refuses with |
| `.rule` | `ritsu rulec vectors <file.rule>` | `in`, `out` and `why`, on each side of each boundary | make the call with those inputs; read the member that shows `out` |
| `.book` | `ritsu chobo scenarios <file.book>` | a list of steps with what each answers: each bound just before, at and past it, each key used twice, every way a hold ends | make the same operations through the service; read what they answer and what is left |
| `.flow` | `ritsu dandori scenarios <file.flow>` | an input and scripted answers that take one arm | the arms a caller can bring about; what each one ends in |
| `.kn` | `ritsu kaname run <file.kn> <function> --input - --format json` | the answer of a function for an input you choose | where a function's answer shows in a response, the inputs its branches turn on |

The lists are long (a gate with a few roles and attributes gives thousands). Do not write one claim per
entry. Group the entries by what differs, and write one claim per group:

- **A gate:** for each action, each kind of caller against each state you can reach. For each `forbid`,
  a claim in which that condition alone is what refuses: an entry whose `reason` lists that one policy.
  A combination the service cannot be brought to is not reached.
- **A rule:** each row of each table on both sides of its boundary. `why` says which boundary an entry
  is for.
- **A book:** every scenario the service's operations can express. Each refusal, and each key used
  twice, with the balance read afterwards.
- **A flow:** every arm that ends differently for the caller.
- **A function:** where a rule's inputs are state that earlier calls build up, `kaname run` gives the
  answer along the same chain: feed each call's result into the next input.

An entry says what should come back, in the words of its spec. A claim also says what must not have
changed: after a refusal, read what the refused call would have moved.

## 3. Writing them

Name each claim for where it came from, so a person can read the list against the spec:

```geas
target api {
  serve "python3 server.py {port}"
  port auto
}

claim "gate approve_refund: an auditor is refused" {
  when api.post("/refund-requests/R-1/approve?access_token=demo-auditor")
  then status is 403
}
```

One claim, one decision, and the fewest calls that reach it. Each claim gets a fresh service, so it
starts from nothing and sets up what it needs.

**Do not read a value before the work behind it is done.** A service that hands work to a workflow or a
queue answers first. A claim that reads a total right after such a step passes or fails by timing, and
nothing in one run says so.

- Wait with `within` on a read, for a member that changes when the work finishes.
- A state can be written before the rest of the work is done. When a claim says what the work ended in
  (a balance given back, a total), wait on that value as well: `within` on the read of it, not one read
  after the state.
- When a value is the same before and after the work (a refusal that changes nothing), it says nothing
  about the work being done. Wait for the state that says it is, and read the value after that.
- Do not claim an order between two things the service may do side by side. Say what holds for either
  order, or make the second call after the first has finished.

## 4. Before you show them

**Run the whole file ten times on the code as it is, with the machine busy** (several copies side by
side). A claim that reads too early fails only when the service is slow: two such claims held in seven
quiet runs, and failed in three of fifteen with other copies running.

| What you see | What it is | What to do |
|---|---|---|
| the same failure every run | the claim is wrong, or the service disagrees with the spec | read the spec, and decide which; do not change the claim until it passes. A disagreement goes to the person (SKILL.md §9), quoted, with the entry it came from |
| a failure in some runs | a wait is missing, or an order is assumed | fix the wait. If you cannot, drop the claim and say so: an unreliable claim costs more than a missing one |
| `(error)`, or E032 | the machine: services that did not start in time | fewer `--jobs` |

**Then check that they can fail.** In a throwaway copy, change the first check of every claim (a status
plus one, another state) and run it. Every claim should fail. One that does not is not reading what it
seems to.

Then hand over:

- the proposed file, and how long it takes to run;
- for each spec, how many entries there were, how many claims you wrote, and what was not reached and why;
- each disagreement, as a question, and each claim you dropped.

A long file is the person's to weigh. It may belong in a nightly run rather than on every change, and
they may want the claims that found something kept apart from the rest.

## 5. What this does and does not do

An agent following this page wrote 49 claims for a refund service in under ten minutes. With the 11
claims and the tests the project had, they caught 24 of 31 breaks another agent had made, against 16
without them, and said nothing of six harmless changes.

It holds what the specs say. A mistake no spec speaks of (a call a gate never sees, a value read without
the lock that protects it, a tool that passes the wrong argument to the right call) gets past these
claims as it gets past the specs. And the claims choose what they exercise: a thing no entry led you to
is not held, and what needs a clock or two callers at once is out of reach. Say so when you hand them
over.
