# Try it in the browser

This is the checker itself, compiled to wasm32 and running in this page: the same
`report`, the same diagnostics, the same generator. **Nothing is sent anywhere.** The
table you type stays in the browser, so a tariff you are not allowed to paste into a web
form is safe to paste here.

<div class="pg" data-lang="en">
  <div class="pg-bar">
    <button data-preset="gap" type="button">the table with a row missing</button>
    <button data-preset="full" type="button">the whole table</button>
    <select class="pg-picker" hidden></select>
    <span class="pg-status"></span>
  </div>
  <textarea class="pg-src" spellcheck="false" autocapitalize="off" autocorrect="off"></textarea>
  <div class="pg-tabs">
    <button data-view="check" class="on" type="button">check</button>
    <button data-view="gen" type="button">generated code</button>
    <button data-view="doc" type="button">the approver's page</button>
  </div>
  <div class="pg-out"></div>
</div>

<script src="playground/playground.js" defer></script>

## What to try

**It opens on a table with its last row missing**, which is the table on the
[front page](index.md)'s first picture. `check` does not say "incomplete": it says which
input falls through, and gives the shape of the row that closes it.

1. **Read the finding.** `An input that matches no row: Destination = Overseas, Weight = 2001g`.
   No data and no old implementation were needed to find that.
2. **Close the gap.** Press *the whole table* to put the row back —
   `| Overseas | >2kg <=5kg | 15USD |` — and the finding goes away. The row the hint prints
   is a *shape*, with an amount copied from the first row and a weight of exactly 2001g: it
   closes the one point the witness names, because the tool does not invent an amount and
   does not guess where the band ends.
3. **Open *generated code*.** Everything `rulec gen` writes for this table: Python,
   TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy, each with its runner, the rule as
   an MCP server, and the test vectors built from the table's own boundaries.
4. **Open *the approver's page*.** What `rulec doc --format html` renders for whoever signs
   the table off. It is a board laid out for a whole window, so it opens in a tab of its
   own rather than in a box on this page — and it is not a picture of the answer: the page
   runs the generated JavaScript, so a case typed into it is decided by the same code.
5. **Break something on purpose.** Change `<=2kg` to `<=6kg` and watch the overlap come
   back with the input that matches both rows; take the `round up(1USD)` off the output and
   read what the rounding diagnostic asks.

## What is not here

`verify` (against an implementation that runs today), `replay` and `diff` (against past
records) need a process and files, so they are not in this page — they are the
[command](install.md). The grammar is in [Write a table (.rule)](tour.md), and what each
finding means is in [What it proves](checks.md).
