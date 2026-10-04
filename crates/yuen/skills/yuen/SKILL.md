---
name: yuen
description: Write and check yuen files (`.req`), which say where each requirement comes from — an article of a law (e-Gov or the eCFR), a file, or a person's decision — who owns it, what meets it (a rulec rule, a koyomi date, a chobo account or transfer, a dandori workflow, a `.proto`, a file of code) and what checks it (a geas or koyomi claim, a tool's own check), with each link recorded under the hashes of its two ends when a person looked. Use when a requirement and its provenance have to be written or changed as a `.req`; when `yuen check` stops on a yuen diagnostic (E001-E409, W101-W401), above all a mark (E301-E304) that shows what changed; when a diff has to be traced to the requirements and owners it touches; or when the page of a project has to be made for the people who check what the code is meant to do.
compatibility: Requires `ritsu` on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`); run yuen as `ritsu yuen`. `yuen source fetch` and `yuen source outdated` also need `curl`.
license: MIT OR Apache-2.0
---

## When this applies

The job is **where a requirement comes from, and whether what meets it is still what a person
looked at**. A requirement read from an article of a law, or decided by someone, is met by a rule's
table, a calendar's date, an account of a book or a file of code, and checked by a claim. yuen
writes those links in a `.req`, records under each the hashes of both ends when a person looked,
and stops the check when either end moves: an article amended, a rule rewritten, a line of code
changed.

It does not apply to what the rule, the date or the code says: that is rulec, koyomi, chobo,
dandori or the code itself. yuen never reads what a requirement means.

# Working with yuen

Run yuen as `ritsu yuen <command>`, so the other languages a project names are read in the same
process (`yuen`, the link ritsu installs, is the same). Give `check` one project: a `.req`, or a
directory of them.

## 1. The loop

1. **Write the requirement** with the person who owns it: its sentence, where it comes from
   (`from @<source> <article>` or `decided <date> by <role> "<why>"`), its owner, what meets it
   (`satisfied by`) and what checks it (`verified by`), or a waiver with its reason.
2. **`ritsu yuen check <path>`** until only marks remain: every link without a record is E301.
3. **Show a person the diff** of each mark (E302, E303) and the article or the artifact it points
   at (`ritsu yuen trace <path> --requirement <name>`, or the page of `ritsu yuen doc`).
4. **The person looks.** If they ask, run `ritsu yuen review <path> --at <file.req>:<line> --by
   <their role>` for exactly what they looked at, so the record carries their role and today's
   date.
5. **`ritsu yuen check`** again, and **`ritsu yuen doc`** for the people who have to understand and
   check what the code is meant to do.

When code changes, `ritsu yuen affected <path> --diff <file>` answers which requirements the diff
touches and whom to ask (the owners), following the code through geas's records to the claims.

## 2. The language on one page

```req
requirements osha v1
description "Where the distances of rulec's rule osha_extinguisher.rule come from: 29 CFR 1910.157(d), read from the eCFR, copied and pinned here. Written as an example; it does not say how the regulation is to be read"

role safety "Decides how the regulation reads"
role development "Writes and fixes the rule"

source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269

scope rulec "osha_extinguisher.rule" table

requirement extinguisher_distance
  text "Portable fire extinguishers for Class A hazards are placed so that the travel distance to any extinguisher is 75 feet or less, and for Class B hazards 50 feet or less"
  in force 2026-01-01..
  owner safety
  from @osha "§1910.157"
    reviewed 2026-10-04 by safety sha256:c2a9ce966c7e2269 -> sha256:849dc53f1d112c23
  satisfied by rulec "osha_extinguisher.rule" table distance
    reviewed 2026-10-04 by development sha256:849dc53f1d112c23 -> sha256:5681500476af8a56
  verified by rulec "osha_extinguisher.rule"
    reviewed 2026-10-04 by development sha256:849dc53f1d112c23 -> sha256:a52e955b88af88d6
```

- Sections in order: heading, `description`, `role`, `source`, `scope`, `requirement`. Lines of a
  requirement in order: `text`, `in force`, `owner`, `replaces`, `from`, `decided`, `satisfied
  by`/`not satisfied`, `verified by`/`not verified`.
- **Sources**: `law "<e-Gov ID>" asof <date>` or `law ecfr "<title> CFR <part>" asof <date>`, an
  article pinned per line (`yuen source fetch` copies, `yuen source pin` writes the pins); `file
  "<path>" sha256:<16>`; or borrowed from a rule or a calendar that already pins it:
  `source 民法 = koyomi "civil_code_period_end.ja.cal" source 民法`. Borrow rather than copy twice.
- **Namings**: `<tool> "<path>" [<kind> <name>]`, the path from the `.req`'s directory:
  `rulec "x.rule" table distance`, `koyomi "x.cal" date payment`, `chobo "x.book" transfer
  refund`, `geas "x.geas" claim "rejects an empty name"`, `dandori "x.flow"`, `proto "x.proto"
  service S method M`, `file "server.py"`. Names are the language's own, never its aliases.
- `verified by` takes only what can fail: a geas claim, a koyomi claim, a tool's check of a file
  (`rulec "x.rule"`), or a test file.
- A name that is not ASCII carries an alias: `requirement 満了日_142条(last_day_142)`. Versions
  `v1`, `v2` each have `in force`, back to back with no gap.
- Never write `reviewed` or `approved` lines by hand, nor a hash: `yuen review` writes them.

The whole language is in [reference.md](reference.md).

## 3. What stays with a person

Ask; do not guess:

- **the sentence** of the requirement, and **which article** it is read from (and how to read it,
  when the article can be read two ways: that is a `decided` line, with who and why);
- **the owner**, a declared role;
- **the reason for a waiver** (`not satisfied` / `not verified`): who accepts that nothing meets or
  checks it, and why;
- **who looked**: the role passed to `yuen review --by`.

**Do not run `yuen review` on your own judgment**, not even to clear a mark you are sure about. A
record says that a person, in that role, looked at both ends on that day; written by an agent, it
says something that did not happen and silences the check that exists to make a person look. Run
it only when the person who looked asks, with their role, and only for what they looked at (`--at`
or `--requirement` rather than `--all`).

## 4. From a diagnostic to a fix

`ritsu yuen explain <CODE>` gives when a code comes, how to fix it and the smallest reproduction;
every code is in [codes.md](codes.md).

- **E301** (no record yet): show the person both ends; once they have looked and ask, `review`.
- **E302** (what it comes from changed: an article amended, a requirement rewritten) and **E303**
  (the artifact changed): read the diff the diagnostic shows. If the change is right, the person
  looks and asks for `review`; if not, fix the artifact or the requirement. The page shows the
  same diff.
- **E304** (a waiver not approved, or approved before the requirement changed): the owner approves
  again through `review`, or the waiver goes.
- **E101–E107** (copies and pins): `ritsu yuen source fetch` then `ritsu yuen source pin` for a
  source the `.req` copies itself; a borrowed source is fixed in its rule or calendar.
- **E201–E203, E205** (a naming that does not resolve, or a file its language does not pass): fix
  the path or the name (E202 suggests one), or fix the file in its own language first.
- **E206**: the yuen that ran holds no other language; run the same command as `ritsu yuen`.
- **E401/E402** (nothing meets or checks a requirement): add a link, or a waiver whose reason the
  owner gives. **E404** (an artifact of a scope traces to nothing): link it from the requirement it
  serves, or narrow the scope with the person.

The example that stops on purpose, as `check` prints it:

```console
$ ritsu yuen check examples/civil_code_periods_reread/civil_code_periods_reread.ja.req --root examples/civil_code_periods_reread
error[E303]: examples/civil_code_periods_reread/civil_code_periods_reread.ja.req:40:3: koyomi "civil_code_period_end.ja.cal" date 満了日_142条 changed after 開発 looked at this link on 2026-10-04
    40 |   satisfied by koyomi "civil_code_period_end.ja.cal" date 満了日_142条
  what changed in koyomi "civil_code_period_end.ja.cal" date 満了日_142条:
      @@ -1,2 +1,2 @@
        date 満了日_142条(last_day_142) = 満了日                 @民法 第142条
      - if closed + 1 day
      + roll following
  = Once a person has looked: yuen review examples/civil_code_periods_reread/civil_code_periods_reread.ja.req --root examples/civil_code_periods_reread --at examples/civil_code_periods_reread/civil_code_periods_reread.ja.req:40 --by <role>
examples/civil_code_periods_reread/civil_code_periods_reread.ja.req: 1 error
```

Here the calendar was rewritten to read article 142 another way after the person who owns the
requirement decided how to read it. Show them the diff and the decision; whether the calendar or
the decision changes is theirs to say.

## 5. For a machine

`ritsu yuen check --format json` gives one object with the diagnostics and their diffs;
`ritsu yuen api` gives the whole graph (requirements, links and their states, artifacts, scopes);
`ritsu yuen trace --format json` and `ritsu yuen affected --format json` give their answers as
JSON. Exit codes: 0 no errors, 1 errors (a mark is one), 2 bad arguments, a file that cannot be
read, or a language not joined.

## 6. The files bundled with this skill

- [reference.md](reference.md): the whole language, the commands, the exit codes and the JSON
- [codes.md](codes.md): every diagnostic code
