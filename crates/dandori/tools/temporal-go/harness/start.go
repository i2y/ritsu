package harness

import (
	"bytes"
	"context"
	"encoding/json"
	"reflect"

	enumspb "go.temporal.io/api/enums/v1"
	"go.temporal.io/sdk/client"
)

// StartOf makes a Flow's Start of a package's Start, which takes the package's WorkflowInput (the
// input a run's JSON carries is given as that type). An input the type carries as it is goes
// through the package's Start. One it cannot carry (a required input left out, a value of another
// type, as the runs that try the input at the door have) starts as the package's Start starts a
// run, with the JSON value as it is, so that the workflow turns it away as it would turn away what
// another client sent.
func StartOf[I any](start func(ctx context.Context, c client.Client, id string, input I, searchAttributes bool) (client.WorkflowRun, error), workflowType, taskQueue string) func(ctx context.Context, c client.Client, id string, input any, searchAttributes bool) (client.WorkflowRun, error) {
	return func(ctx context.Context, c client.Client, id string, input any, searchAttributes bool) (client.WorkflowRun, error) {
		if typed, ok := carried[I](input); ok {
			return start(ctx, c, id, typed, searchAttributes)
		}
		o := client.StartWorkflowOptions{ID: id, TaskQueue: taskQueue, WorkflowIDReusePolicy: enumspb.WORKFLOW_ID_REUSE_POLICY_REJECT_DUPLICATE, WorkflowExecutionErrorWhenAlreadyStarted: true}
		if searchAttributes {
			o.Memo = map[string]any{"dandori.cases": true}
		}
		return c.ExecuteWorkflow(ctx, o, workflowType, input)
	}
}

// carried is v as an I, when the I goes to JSON as v does: nothing left out, added or changed on
// the way.
func carried[I any](v any) (I, bool) {
	var out I
	b, err := json.Marshal(v)
	if err != nil {
		return out, false
	}
	d := json.NewDecoder(bytes.NewReader(b))
	d.DisallowUnknownFields()
	if d.Decode(&out) != nil {
		return out, false
	}
	back, err := json.Marshal(out)
	if err != nil {
		return out, false
	}
	var was, is any
	if json.Unmarshal(b, &was) != nil || json.Unmarshal(back, &is) != nil || !reflect.DeepEqual(was, is) {
		return out, false
	}
	return out, true
}
