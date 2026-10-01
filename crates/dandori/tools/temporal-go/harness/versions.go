package harness

// Worker Deployment Versioning on a real Temporal server (the dev server): two builds of one
// workflow, A and B, whose workers are versions of one deployment through the generated
// NewWorker. A run that starts on A and waits while B becomes the current version must end on A's
// code, the rounds it runs in a new run after a Continue-As-New too; a run that starts after must
// run on B's. The Go twin of ../../temporal-python/versions.py. The build gives both copies the
// patch "continue": every history is long enough to go on in a new run.
//
//	<bin> --versions <key A> <key B> <results.json>
//
// results.json: { "buildIds": [A's, B's], "notified": { "<run>": [the texts it notified] },
//                 "runs": { "<run>": how many runs it took } }

import (
	"context"
	"fmt"
	"sync"

	workflowservice "go.temporal.io/api/workflowservice/v1"
	"go.temporal.io/sdk/activity"
	"go.temporal.io/sdk/client"
	"go.temporal.io/sdk/worker"
)

const deployment = "dandori-versions"

func versions(a, b Flow, outFile string) error {
	ctx := context.Background()
	// no workflow kept in the workers' cache (the cache is the process's, and set before a
	// worker starts): a run goes where the server sends it, not where it was
	worker.SetStickyWorkflowCacheSize(0)
	server, err := startServer(serverOptions{timerShift: true})
	if err != nil {
		return err
	}
	defer func() { _ = server.Stop() }()
	c := server.Client()

	// what each run did: the ids of its callbacks, and the texts it notified
	var mu sync.Mutex
	callbacks := map[string][]string{}
	notified := map[string][]string{}
	// the tasks of tests/versions/approvals.flow
	own := func(ctx context.Context, task string, args map[string]any) (any, error) {
		id := activity.GetInfo(ctx).WorkflowExecution.ID
		mu.Lock()
		defer mu.Unlock()
		switch task {
		case "承認を求める":
			callbacks[id] = append(callbacks[id], text(args["callback_id"]))
		case "通知する":
			notified[id] = append(notified[id], text(args["本文"]))
		default:
			return nil, fmt.Errorf("tests/versions/approvals.flow has no task %s", task)
		}
		return result(nil), nil
	}
	waiting := func(run string, n int) func() (bool, error) {
		return func() (bool, error) {
			mu.Lock()
			defer mu.Unlock()
			return len(callbacks[run]) == n, nil
		}
	}
	callback := func(run string, i int) string {
		mu.Lock()
		defer mu.Unlock()
		return callbacks[run][i]
	}
	// current makes the build the current version of the deployment, once the server has seen its worker
	current := func(f Flow) error {
		return until(f.BuildID+" can be the current version", func() (bool, error) {
			_, err := c.WorkflowService().SetWorkerDeploymentCurrentVersion(ctx, &workflowservice.SetWorkerDeploymentCurrentVersionRequest{
				Namespace: "default", DeploymentName: deployment, BuildId: f.BuildID,
			})
			return err == nil, nil // an error: the server has not seen the worker's pollers yet
		})
	}
	// runsOfCount is how many runs a workflow took: the first, and each that went on from the one before (Continue-As-New)
	runsOfCount := func(id, firstRunID string) (int, error) {
		h, err := runsOf(ctx, c, id, firstRunID)
		return len(h), err
	}

	inputs := map[string]any{"申込": map[string]any{"id": "申込-1"}}
	workerA := a.NewWorker(c, own, nil, deployment)
	if err := workerA.Start(); err != nil {
		return fmt.Errorf("starting A's worker: %w", err)
	}
	defer workerA.Stop()
	if err := current(a); err != nil {
		return err
	}
	first, err := a.Start(ctx, c, "run-a", inputs, false)
	if err != nil {
		return err
	}
	// the first run's id: once Get has followed the runs that went on in a new one, GetRunID is the last's
	firstA := first.GetRunID()
	if err := until("the first run waits for its approval", waiting("run-a", 1)); err != nil {
		return err
	}
	workerB := b.NewWorker(c, own, nil, deployment)
	if err := workerB.Start(); err != nil {
		return fmt.Errorf("starting B's worker: %w", err)
	}
	defer workerB.Stop()
	if err := current(b); err != nil {
		return err
	}
	second, err := b.Start(ctx, c, "run-b", inputs, false)
	if err != nil {
		return err
	}
	firstB := second.GetRunID()
	if err := until("the second run waits for its approval", waiting("run-b", 1)); err != nil {
		return err
	}
	// the first run, pinned to A, goes on with A's code, in its next run too; the second, started on B, with B's
	if err := a.Answer(ctx, c, callback("run-a", 0), CallbackAnswer{OK: map[string]any{"承認者": "a"}}); err != nil {
		return err
	}
	if err := b.Answer(ctx, c, callback("run-b", 0), CallbackAnswer{OK: map[string]any{"承認者": "b"}}); err != nil {
		return err
	}
	if err := until("the first run waits for its second approval", waiting("run-a", 2)); err != nil {
		return err
	}
	if err := until("the second run waits for its second approval", waiting("run-b", 2)); err != nil {
		return err
	}
	if err := a.Answer(ctx, c, callback("run-a", 1), CallbackAnswer{OK: map[string]any{"承認者": "a"}}); err != nil {
		return err
	}
	if err := b.Answer(ctx, c, callback("run-b", 1), CallbackAnswer{OK: map[string]any{"承認者": "b"}}); err != nil {
		return err
	}
	for _, run := range []client.WorkflowRun{first, second} {
		if err := run.Get(ctx, nil); err != nil {
			return fmt.Errorf("the run %s: %w", run.GetID(), err)
		}
	}
	runA, err := runsOfCount("run-a", firstA)
	if err != nil {
		return err
	}
	runB, err := runsOfCount("run-b", firstB)
	if err != nil {
		return err
	}
	mu.Lock()
	out := map[string]any{"buildIds": []string{a.BuildID, b.BuildID}, "notified": notified, "runs": map[string]int{"run-a": runA, "run-b": runB}}
	err = writeJSON(outFile, out, true)
	mu.Unlock()
	return err
}
