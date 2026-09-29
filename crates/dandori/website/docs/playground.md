# Try it in the browser

This is dandori itself, compiled to wasm32 and running in this page: the same checker, the same
builds for every platform, the same page `dandori doc` draws. **Nothing is sent anywhere.** The
flow you type stays in the browser.

<div class="pg" data-lang="en">
  <div class="pg-bar">
    <select class="pg-preset" aria-label="The flow to open"></select>
    <span class="pg-path"></span>
    <button class="pg-revert" type="button" hidden></button>
    <span class="pg-status"></span>
  </div>
  <textarea class="pg-src" spellcheck="false" autocapitalize="off" autocorrect="off" aria-label="The flow"></textarea>
  <div class="pg-tabs">
    <button data-view="check" class="on" type="button">check</button>
    <button data-view="build" type="button">build</button>
    <button data-view="doc" type="button">draw</button>
    <button data-view="rules" type="button">rules</button>
    <select class="pg-target" aria-label="The platform" hidden></select>
    <select class="pg-picker" aria-label="The file" hidden></select>
  </div>
  <div class="pg-out"></div>
</div>

<script src="playground/playground.js" defer></script>

A flow here reads what the examples read, from the paths they give: their rules, the descriptions of
their APIs, and fulfillment's child flow. A page cannot run rulec, so it carries what rulec printed
for the examples' rules, and what `rulec doc` renders for them, recorded from the repository. The
tests hold that record to what rulec prints now, and what this page answers to what the command
answers ([How it is checked](assurance.md)).

## What to try

**It opens on a first draft of the hotel booking**, the one [What it checks](checks.md) starts
from. It calls the same rules and the same Stripe API as the example, and check finds four errors
in it.

1. **Read the findings.** The last one says the workflow can end while the PaymentIntent is still
   `processing`, or back in `requires_payment_method`, and gives the run that gets there: after
   the capture, the bank can still decline the payment (`settle` happens on Stripe's side). Click
   where a finding is, such as `tests/fixtures/hotel_naive.flow:95:1`, to go to that line.
2. **Put one right.** At the end of line 91, after `fail CardDeclined "The card was declined"`, add
   `leaving pi`: the workflow hands the PaymentIntent over as it is, and the finding on line 91 goes
   away. The other three stay.
3. **Draw it.** *draw* opens the page `dandori doc` writes for whoever reviews the workflow. It is
   drawn even with errors, and the run of each error lights up on the flow.
4. **Read the rules.** *rules* shows the rules the flow calls, as the repository has them: `hold`,
   which decides how much to hold on the card and whether the front desk looks first, and
   `payment_intent`, the PaymentIntent's state machine. *open the rule's page* opens what
   `rulec doc` renders for whoever approves the rule, and a case can be tried on it, as on rulec's
   playground. The page `dandori doc` writes opens it too, from the list of rules on its left.
5. **Open a version of an example.**
   [*Hotel booking · for Temporal*](#flow=examples/hotel/temporal/hotel.flow&view=build) passes
   check, and *build* shows what Temporal runs: the workflow, the activities, the client and the
   worker, in TypeScript. Pick another platform beside it. For Step Functions nothing is built, and
   it says why: this version waits for Stripe's webhook as an event sent to the workflow and
   releases the hold in `on cancel`, which Step Functions cannot do, and it names no EventBridge
   connection to call Stripe through and no Lambda function for the rule.
   [*Hotel booking · for AWS*](#flow=examples/hotel/aws/hotel.flow&view=build) is the version
   written for it.
6. **Break something on purpose.** In *Hotel booking · for Temporal*, take out the arm
   `canceled => fail PaymentCanceled …`, and check finds a `match` with no arm for `canceled`, with
   the run in which confirming the payment cancels it. Take `key` off `capture_intent`, which is
   retried, or `leaving pi` off `fail SettlementUnclear`, and read what comes back.

## What is not here

- **Running.** `dandori run`, the scenarios, and the platforms themselves. What the builds do on
  every scenario is run by the tests.
- **Changing a rule.** The rules are the examples' own, and a flow here can use only those; *rules*
  shows them, but they cannot be changed here. To write a rule, use
  [rulec's playground](https://i2y.github.io/rulec/playground/).
- **Calls.** Nothing is called: not an API, an agent or Jev. *build* shows the code that would call
  them.
- **Other files.** Only the flow in the box can be edited. A child flow and the API descriptions
  are read as the repository has them.
