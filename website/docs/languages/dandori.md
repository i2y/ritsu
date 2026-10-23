# dandori

`.flow` — the skeleton of a workflow.

**dandori** says what a workflow calls, in what order, with which retries and timeouts, and what each call does to a case outside — an order, a payment, a booking. `dandori check` shows that on every path each case is left untouched or finished, never halfway, and the same file becomes Temporal, Step Functions, Argo Workflows, Lambda durable functions or pydantic-graph.

dandori has a site of its own: **[the dandori site](../dandori/)**.
