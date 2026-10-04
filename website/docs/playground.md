# Try it in the browser

This is ritsu itself, compiled to wasm32 and running in this page: `ritsu check` over a whole
project, each file checked by its own language with what one language checks read by the next,
and each language's generator and page. **Nothing is sent anywhere.** The files you edit stay in
the browser.

<div class="pg" data-lang="en">
  <div class="pg-bar">
    <select class="pg-project" aria-label="The project to open"></select>
    <button class="pg-add" type="button"></button>
    <button class="pg-remove" type="button"></button>
    <button class="pg-revert" type="button" hidden></button>
    <span class="pg-status"></span>
  </div>
  <div class="pg-files" role="tablist" aria-label="The project's files"></div>
  <textarea class="pg-src" spellcheck="false" autocapitalize="off" autocorrect="off" aria-label="The file"></textarea>
  <div class="pg-tabs">
    <button data-view="check" class="on" type="button">ritsu check</button>
    <button data-view="gen" type="button">generate</button>
    <button data-view="doc" type="button">the approver's page</button>
    <select class="pg-target" aria-label="The target" hidden></select>
    <select class="pg-picker" aria-label="The file written" hidden></select>
  </div>
  <div class="pg-out"></div>
</div>

<script src="playground/playground.js" defer></script>

A project here is a handful of files, one to a tab. Every time a file changes, the page hands all of
them to the module, which runs the command as it would run in a directory holding the same files:
`ritsu check .` for the project, and the generator or the page of the file that is open. The tests
hold what this page answers to what the `ritsu` binary prints and writes on the same files, word for
word (`crates/ritsu/tests/playground.rs`).

## What to try

**It opens on a small shop where ordering has just added a value to its contract**:
`ORDER_STATUS_RETURNED = 5;` in `proto/shop/v1/order.proto`. Billing's rule imports that enum, the
requirement in `requirements/billing.req` names the rule's table, and the map names the rule as
billing's layer against ordering. One line in one file, and three languages say what it does to them.

1. **Read the findings.** Each headline carries the tool that says it. rulec:
   `Enum order_status does not agree with OrderStatus in ../../proto/shop/v1/order.proto`, with the
   value the rule does not have. yuen: `rulec cannot answer for rulec "billing/rules/billing_need.rule"
   table decide`, the end of a requirement it can no longer hash. sakai: `The file
   billing/rules/billing_need.rule does not pass rulec's check, or cannot be read`. Click where a
   finding is, such as `billing/rules/billing_need.rule:5`, to go to that line.
2. **Put it right in the contract.** Take the line out of the `.proto`, and every file passes.
3. **Or put it right in the rule.** Press *undo my edits*, and in `billing/rules/billing_need.rule`
   add `| returned` to the end of the enum. rulec now says `` A value of the imported enum
   order_status has neither a row nor `default` ``. Add the row `| returned  | skip        |` under
   `cancelled`, and rulec passes; yuen then says the table `changed after accounts looked at this
   link on 2026-10-04`, and shows the row that is new. A rule that passes its check is not yet a
   rule someone has looked at.
4. **Break the border between a workflow and a rule.** In `ordering/rules/urgency.rule`, rename the
   output `carrier` to `courier`, in `outputs` and in the headers of the table and the examples.
   rulec passes. dandori, which reads the rule's outputs from rulec in the same process, says
   `` `urgency.outputs` has no field `carrier` (urgent, courier) `` on the two lines of
   `ordering/ship_order.flow` that read it.
5. **Generate.** Open `ordering/ship_order.flow` and press *generate*: what Temporal runs, in
   TypeScript, eight files. Pick Step Functions beside it, and nothing is written: the flow waits
   for an event sent to the workflow and cleans up in `on cancel`, which Step Functions cannot do,
   and its tasks name no connection to call the warehouse through. Every language that generates
   does it here: a rule its code in twelve languages, a calendar in five, the book its SQL and its
   clients, the map its Context Mapper CML, the requirements ReqIF or W3C PROV.
6. **Open the approver's page** of a rule, a calendar, the book or the flow. It is laid out for a
   whole window, so it opens in a tab of its own; the Markdown each writes for a pull request is
   below the link.

*A small shop, in Japanese* in the list is the same project with Japanese names, and the same steps
work there.

## What is not here

- **geas.** geas holds a claim by running the code it names, and a page starts no process. A
  `.geas` file here is read and its syntax checked, and then geas says it cannot start the program.
- **Running.** `dandori run`, `chobo run`, the scenarios, `rulec verify` and `replay` need
  processes, servers and files of records. They are the [command](https://github.com/i2y/ritsu).
- **The network.** `source fetch` and `outdated` ask e-Gov, the eCFR and the URLs a source names.
- **A Rust workspace in a map.** sakai asks Cargo for the crates of a map's `code rust "…"`, and a
  page cannot run Cargo.
- **Writing into the project.** `rulec fmt` and `yuen review` change the files they are given. Here
  nothing changes the files but you.
