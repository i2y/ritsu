package harness

// Runs a workflow that dandori generated for Temporal's Go SDK on a real Temporal server against
// scripted answers, and writes what it did in the shape the reference interpreter prints for
// Temporal: every call with its arguments and the answer it got, and how the workflow ended. With
// each run, it also writes what the query dandori.status says of the cases at the end (`cases`),
// and the search attribute DandoriCases (`shown`). The workers, the starts, the callbacks' answers
// and the query go through the generated package (worker.go and client.go). The Go twin of
// ../../temporal-python/run.py, which says the same of the Python build.
//
//	<bin> <key> <runs.json> <results.json> [<histories dir>]
//	<bin> --replay <key> <histories dir> <results.json>
//	<bin> --serve <key> <runs.json> <steps.json> <server address>
//
// With DANDORI_ACTIVITIES_BY set to a command (a JSON list), the workers here run only the
// workflows, and the activities are the other language's: the command, with <steps.json> and the
// server's address added, serves them (run.py --serve, or ../../temporal/run.mjs --serve) until
// its input closes, and then writes the calls each run made to <steps.json>. With --serve, this
// runner is that command for another: it serves the activities of the generated code, with the
// stand-ins, on the server at the address.
//
// Once the runs are over, the history of each is replayed with the same code, which must not find
// it nondeterministic; with a histories directory, the histories are also written there as JSON,
// one file a run: <workflow id>.json, and for each run that went on from it after a
// Continue-As-New, <workflow id>.<n>.json (the second is 2). With --replay, it only replays the
// histories of a directory with the code, and writes for each file the error, or null.
//
// runs.json: { "workflow": type, "own": [ { "name", "method", "callback" } ], "rules": [activity],
//              "children": [ { "type", "queue" } ], "queues": [queue], "http": [...], "aws": [...],
//              "callbacks": [the function of each task that hands on a callback's id],
//              "runs": [ { "id": workflow id, "input": {...},
//                          "answers": [ {"ok": value} | {"error": kind} | {"cancel": true} ],
//                          "events": { "<the index of an answer>": the event it is for } } ] }
//
// A task the user writes is found by its "name", which the generated OwnTasksBy hands over; the
// "callbacks" are read by the build, which patches the copies (../build).
//
// Every run goes at once, each as the workflow with its own id; a stand-in finds its run by the id
// of the workflow that called it (a child workflow's id starts with its parent's). The tasks that
// say `lambda`, `http` or `aws` run the generated code, with a Transport that writes down what it
// would send (transport.go). The tasks the user writes, the rules and the child workflows are
// stand-ins that answer from the scenario. A callback's answer comes as the update the workflow
// waits for, sent before the task that hands the id on returns; a callback that the scenario times
// out gets none. A call that the scenario times out gets no answer either: its stand-in keeps the
// activity busy until the server times it out. A call during which the scenario cancels the
// workflow asks the server to cancel it, and keeps the activity busy until the server tells it that
// it is cancelled; a callback's answer that is a cancellation is the request to cancel. An event
// (a task that says `event`) is sent with the generated client's Send once the query
// dandori.status says the workflow waits for it; an event the scenario times out is not sent, and
// is written down when the next call comes, or when the run is over. While the workflow waits for
// one event, another it has must be refused.
//
// The server keeps real time, so the copy of the generated code that runs here waits far less (the
// build patches it as run.py patches its copy): every duration of the workflow's timers (ddSeconds)
// is at most 10 ms, and an activity or a child workflow gets Settings.Timeout seconds before it
// times out, but for one that hands on a callback's id. An event is waited for 5 seconds at most,
// long enough for the runner to see the wait and send it. The rounds of `for … in parallel` run one
// at a time, so that the calls come in the order the reference interpreter makes them. A loop at the
// top of the flow goes on in a new run (Continue-As-New) at every round but the first of a run,
// since the history counts as long from one event on (ddContinueAt).
//
// A workflow task that panics fails the workflow here (worker.FailWorkflow), where Temporal's
// default would retry the task until the workflow times out: a fault of the generated code then
// shows as the run's end, with the panic's message, instead of a runner that never ends.

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"slices"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	enumspb "go.temporal.io/api/enums/v1"
	historypb "go.temporal.io/api/history/v1"
	"go.temporal.io/api/serviceerror"
	"go.temporal.io/api/temporalproto"
	"go.temporal.io/sdk/activity"
	"go.temporal.io/sdk/client"
	"go.temporal.io/sdk/converter"
	"go.temporal.io/sdk/temporal"
	"go.temporal.io/sdk/worker"
	"go.temporal.io/sdk/workflow"
	"google.golang.org/protobuf/proto"
)

type spec struct {
	Workflow string           `json:"workflow"`
	Own      []ownTask        `json:"own"`
	Rules    []string         `json:"rules"`
	Children []map[string]any `json:"children"`
	Queues   []string         `json:"queues"`
	HTTP     []httpTask       `json:"http"`
	AWS      []awsTask        `json:"aws"`
	Runs     []scenario       `json:"runs"`
}

type ownTask struct {
	Name     string `json:"name"`
	Method   string `json:"method"`
	Callback bool   `json:"callback"`
}

type scenario struct {
	ID      string            `json:"id"`
	Input   any               `json:"input"`
	Answers []map[string]any  `json:"answers"`
	Events  map[string]string `json:"events"`
}

func readSpec(path string) (spec, error) {
	var sp spec
	b, err := os.ReadFile(path)
	if err != nil {
		return sp, err
	}
	if err := json.Unmarshal(b, &sp); err != nil {
		return sp, fmt.Errorf("reading %s: %w", path, err)
	}
	return sp, nil
}

// childTypes are the types of the child workflows the stand-ins take the place of, and the task
// queues they run on.
func (sp spec) childTypes() (types []string, queues []string) {
	for _, c := range sp.Children {
		if t, ok := c["type"].(string); ok {
			types = append(types, t)
		}
		if q, ok := c["queue"].(string); ok && q != "" {
			queues = append(queues, q)
		}
	}
	return types, queues
}

// queuesOf are the task queue of the workflow and, after it in order, the others, once each.
func queuesOf(first string, others []string) []string {
	rest := []string{}
	for _, q := range others {
		if q != first && !slices.Contains(rest, q) {
			rest = append(rest, q)
		}
	}
	sort.Strings(rest)
	return append([]string{first}, rest...)
}

// runState is one run: the answers it gets, the next one, the calls it made, and the events it waits for.
type runState struct {
	id      string
	answers []map[string]any
	events  map[string]string
	next    int
	steps   []any
	done    bool
}

// runner holds the runs by their workflows' ids; mu guards them, since activities, the event
// senders and the runs' ends come on goroutines of their own.
type runner struct {
	flow            Flow
	spec            spec
	lateFor         time.Duration
	client          client.Client
	own             map[string]ownTask
	mu              sync.Mutex
	runs            map[string]*runState
	refusalsChecked bool
}

func newRunner(flow Flow, sp spec, s Settings, c client.Client) *runner {
	r := &runner{
		flow: flow, spec: sp, client: c,
		// how long a stand-in that the scenario times out keeps its activity busy: until the server has timed it out
		lateFor: time.Duration(s.Timeout+5) * time.Second,
		own:     map[string]ownTask{},
		runs:    map[string]*runState{},
	}
	for _, t := range sp.Own {
		r.own[t.Name] = t
	}
	return r
}

// current is the run of the activity being served: its workflow's id, or its parent's.
func (r *runner) current(ctx context.Context) (*runState, error) {
	id := activity.GetInfo(ctx).WorkflowExecution.ID
	r.mu.Lock()
	defer r.mu.Unlock()
	run, ok := r.runs[strings.SplitN(id, "/", 2)[0]]
	if !ok {
		return nil, temporal.NewNonRetryableApplicationError("no run for the workflow "+id, "Dandori.Test.NoRun", nil)
	}
	return run, nil
}

// settleEvents writes down the events the workflow let run out of time before the call that
// takes the next answer. The caller holds mu.
func (r *runner) settleEvents(run *runState) {
	for {
		name, ok := run.events[strconv.Itoa(run.next)]
		if !ok || run.next >= len(run.answers) || !timesOut(run.answers[run.next]) {
			return
		}
		run.steps = append(run.steps, map[string]any{"call": map[string]any{"event": name}, "answer": recorded(run.answers[run.next])})
		run.next++
	}
}

// takeFor gives the run its next answer, and writes the call down with it at once, so that the
// calls are written down in the order they took their answers.
func (r *runner) takeFor(run *runState, label string, call map[string]any) (map[string]any, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.settleEvents(run)
	if run.next >= len(run.answers) {
		return nil, temporal.NewNonRetryableApplicationError(fmt.Sprintf("no answer for call %d (%s)", run.next+1, label), "Dandori.Test.NoAnswer", nil)
	}
	ans := run.answers[run.next]
	run.next++
	run.steps = append(run.steps, map[string]any{"call": frozen(call), "answer": recorded(ans)})
	return ans, nil
}

// frozen is a copy of a value as JSON has it, taken when the call is made: a local activity gets
// the workflow's own values, which the workflow could change afterwards.
func frozen(v any) any {
	b, err := json.Marshal(v)
	if err != nil {
		return fmt.Sprintf("(not JSON: %v)", err)
	}
	var out any
	_ = json.Unmarshal(b, &out)
	return out
}

func recorded(ans map[string]any) map[string]any {
	if ans["cancel"] == true {
		return map[string]any{"cancel": true}
	}
	if v, ok := ans["ok"]; ok {
		return map[string]any{"ok": v}
	}
	return map[string]any{"error": ans["error"], "as": ans["error"]}
}

func scripted(kind string) error {
	return temporal.NewNonRetryableApplicationError("scripted", errorName(kind), nil)
}

// result is what an activity returns for a value: none goes on the wire as JSON null, as the
// generated code sends it, not as the SDK's binary/null, which the TypeScript SDK reads as
// undefined.
func result(v any) any {
	if v == nil {
		return json.RawMessage("null")
	}
	return v
}

// holdOn heartbeats until the server says the activity is over (cancelled, or timed out; the
// activity's deadline ends it too), at most r.lateFor.
func (r *runner) holdOn(ctx context.Context) {
	until := time.Now().Add(r.lateFor)
	for time.Now().Before(until) {
		activity.RecordHeartbeat(ctx)
		select {
		case <-ctx.Done():
			return
		case <-time.After(100 * time.Millisecond):
		}
	}
}

// late is a call the scenario times out: the activity is kept busy past its timeout, so that the
// server times it out. (The SDK does not send what an activity returns after its deadline.)
func (r *runner) late(ctx context.Context) error {
	r.holdOn(ctx)
	return temporal.NewNonRetryableApplicationError("the server should have timed this out", "Dandori.Test.Late", nil)
}

// cancelling is a call during which the scenario cancels the workflow: the server is asked, and the
// activity is kept until it is cancelled.
func (r *runner) cancelling(ctx context.Context, run *runState) error {
	if err := r.client.CancelWorkflow(ctx, run.id, ""); err != nil {
		return err
	}
	r.holdOn(ctx)
	return temporal.NewNonRetryableApplicationError("the workflow was cancelled", "Dandori.Test.Cancelled", nil)
}

// reply is the answer of a stand-in: its value, the scripted error, no answer in time, or none
// before the workflow is cancelled.
func (r *runner) reply(ctx context.Context, run *runState, ans map[string]any) (any, error) {
	if ans["cancel"] == true {
		return nil, r.cancelling(ctx, run)
	}
	if v, ok := ans["ok"]; ok {
		return result(v), nil
	}
	if kind(ans) == "timeout" {
		return nil, r.late(ctx)
	}
	return nil, scripted(kind(ans))
}

func callbackAnswer(ans map[string]any) CallbackAnswer {
	if v, ok := ans["ok"]; ok {
		return CallbackAnswer{OK: v}
	}
	return CallbackAnswer{Error: errorName(kind(ans)), Message: "scripted"}
}

// refused says whether an update failed in the workflow: its validator refused it, or its handler
// failed (the SDK gives the failure as an ApplicationError).
func refused(err error) bool {
	var app *temporal.ApplicationError
	return errors.As(err, &app)
}

// mustRefuse sends an answer that the workflow must refuse: a second answer to a callback, and an
// answer to one the workflow never waits for.
func (r *runner) mustRefuse(ctx context.Context, callbackID string, a CallbackAnswer, why string) error {
	err := r.flow.Answer(ctx, r.client, callbackID, a)
	if err == nil {
		return fmt.Errorf("the workflow took %s", why)
	}
	if refused(err) {
		return nil
	}
	return err
}

// answerLater answers a callback with the generated client's Answer (the update dandori.answer),
// before the task that hands the id on returns: then the answer is in the history before the
// workflow waits for it, and its short wait cannot run out first. A timeout gets no answer.
func (r *runner) answerLater(ctx context.Context, callbackID string, ans map[string]any) error {
	if timesOut(ans) {
		return nil
	}
	if ans["cancel"] == true {
		run, err := r.current(ctx)
		if err != nil {
			return err
		}
		return r.client.CancelWorkflow(ctx, run.id, "")
	}
	a := callbackAnswer(ans)
	if err := r.flow.Answer(ctx, r.client, callbackID, a); err != nil {
		return fmt.Errorf("answering the callback %s: %w", callbackID, err)
	}
	if err := r.mustRefuse(ctx, callbackID, a, "a second answer to a callback"); err != nil {
		return err
	}
	r.mu.Lock()
	first := !r.refusalsChecked
	r.refusalsChecked = true
	r.mu.Unlock()
	if first {
		var id []any
		if err := json.Unmarshal([]byte(callbackID), &id); err != nil || len(id) == 0 {
			return fmt.Errorf("the callback id %s is not [workflow id, ...]", callbackID)
		}
		other, _ := json.Marshal([]any{id[0], "0"})
		return r.mustRefuse(ctx, string(other), a, "an answer to a callback it does not wait for")
	}
	return nil
}

// The stand-in transport's Run: every call is written down with the answer it takes; a call it
// times out gets none.

func (r *runner) take(ctx context.Context, call map[string]any) (map[string]any, error) {
	run, err := r.current(ctx)
	if err != nil {
		return nil, err
	}
	label, _ := json.Marshal(call)
	return r.takeFor(run, firstRunes(string(label), 80), call)
}

func (r *runner) hold(ctx context.Context, ans map[string]any) error {
	if ans["cancel"] == true {
		run, err := r.current(ctx)
		if err != nil {
			return err
		}
		return r.cancelling(ctx, run)
	}
	return r.late(ctx)
}

func firstRunes(s string, n int) string {
	runes := []rune(s)
	if len(runes) > n {
		return string(runes[:n])
	}
	return s
}

// ownTask stands in for every task the user writes, as OwnTasksBy calls it.
func (r *runner) ownTask(ctx context.Context, task string, args map[string]any) (any, error) {
	t, ok := r.own[task]
	if !ok {
		return nil, temporal.NewNonRetryableApplicationError("no stand-in for the task "+task, "Dandori.Test.NoStandIn", nil)
	}
	run, err := r.current(ctx)
	if err != nil {
		return nil, err
	}
	label := t.Method
	if label == "" {
		label = t.Name
	}
	if t.Callback {
		ans, err := r.takeFor(run, label, map[string]any{"activity": t.Name, "args": without(args, "callback_id")})
		if err != nil {
			return nil, err
		}
		if err := r.answerLater(ctx, text(args["callback_id"]), ans); err != nil {
			return nil, err
		}
		return result(nil), nil
	}
	ans, err := r.takeFor(run, label, map[string]any{"activity": t.Name, "args": args})
	if err != nil {
		return nil, err
	}
	return r.reply(ctx, run, ans)
}

// rule stands in for a rule whose code goes with the workflow, by its activity's name.
func (r *runner) rule(name string) func(ctx context.Context, args map[string]any) (any, error) {
	return func(ctx context.Context, args map[string]any) (any, error) {
		run, err := r.current(ctx)
		if err != nil {
			return nil, err
		}
		ans, err := r.takeFor(run, name, map[string]any{"activity": name, "args": args})
		if err != nil {
			return nil, err
		}
		return r.reply(ctx, run, ans)
	}
}

// testChild is the activity the child workflows' stand-ins ask for the scenario's answer.
func (r *runner) testChild(ctx context.Context, a map[string]any) (any, error) {
	run, err := r.current(ctx)
	if err != nil {
		return nil, err
	}
	typ, _ := a["type"].(string)
	ans, err := r.takeFor(run, typ, map[string]any{"child_workflow": typ, "args": a["args"]})
	if err != nil {
		return nil, err
	}
	return r.reply(ctx, run, ans)
}

// childStandIn is a child workflow's stand-in, which asks an activity for the scenario's answer.
func childStandIn(typ string) func(ctx workflow.Context, input any) (any, error) {
	return func(ctx workflow.Context, input any) (any, error) {
		ctx = workflow.WithActivityOptions(ctx, workflow.ActivityOptions{
			StartToCloseTimeout: 60 * time.Second,
			RetryPolicy:         &temporal.RetryPolicy{MaximumAttempts: 1},
		})
		var out any
		err := workflow.ExecuteActivity(ctx, "dd_test_child", map[string]any{"type": typ, "args": input}).Get(ctx, &out)
		if err != nil {
			var failed *temporal.ActivityError
			if errors.As(err, &failed) {
				if app, ok := failed.Unwrap().(*temporal.ApplicationError); ok {
					return nil, temporal.NewNonRetryableApplicationError(app.Message(), app.Type(), nil)
				}
			}
			return nil, err
		}
		return out, nil
	}
}

// registerStandIns registers the rules' stand-ins and the activity of the child workflows' stand-ins.
func (r *runner) registerStandIns(w worker.ActivityRegistry) {
	for _, name := range r.spec.Rules {
		w.RegisterActivityWithOptions(r.rule(name), activity.RegisterOptions{Name: name})
	}
	w.RegisterActivityWithOptions(r.testChild, activity.RegisterOptions{Name: "dd_test_child"})
}

// heartbeatsAtOnce: heartbeats go out at once, so that a held activity hears soon that it is over.
func heartbeatsAtOnce(o *worker.Options) {
	o.MaxConcurrentActivityExecutionSize = 200
	o.DefaultHeartbeatThrottleInterval = 100 * time.Millisecond
	o.MaxHeartbeatThrottleInterval = 100 * time.Millisecond
}

// serve serves only the activities, on the server at the address, until the input closes; then
// it writes the calls of each run.
func serve(flow Flow, sp spec, s Settings, stepsFile, address string) error {
	c, err := dial(address)
	if err != nil {
		return err
	}
	defer c.Close()
	r := newRunner(flow, sp, s, c)
	for _, sc := range sp.Runs {
		r.runs[sc.ID] = &runState{id: sc.ID, answers: sc.Answers, events: map[string]string{}, steps: []any{}}
	}
	transport := newStandIn(sp, r)
	var workers []worker.Worker
	defer func() {
		for _, w := range workers {
			w.Stop()
		}
	}()
	for _, q := range queuesOf(flow.TaskQueue, sp.Queues) {
		o := flow.WorkerOptions("")
		heartbeatsAtOnce(&o)
		// the Go SDK polls for workflow tasks with no workflow registered, and would take the
		// other runner's workflow tasks and queries
		o.DisableWorkflowWorker = true
		w := worker.New(c, q, o)
		flow.RegisterActivities(w, r.ownTask, transport)
		r.registerStandIns(w)
		if err := w.Start(); err != nil {
			return fmt.Errorf("starting the worker on %s: %w", q, err)
		}
		workers = append(workers, w)
	}
	if _, err := os.Stdout.WriteString("serving\n"); err != nil {
		return err
	}
	if _, err := io.Copy(io.Discard, os.Stdin); err != nil {
		return err
	}
	for _, w := range workers {
		w.Stop()
	}
	workers = nil
	r.mu.Lock()
	steps := map[string]any{}
	for id, run := range r.runs {
		steps[id] = run.steps
	}
	r.mu.Unlock()
	return writeJSON(stepsFile, steps, false)
}

// sendEvents sends the run's events, each once the workflow waits for it (the query's "events"):
// the value, the error, or instead a request to cancel the workflow. Before, every other event must
// be refused, since the workflow does not wait for it. The query may tell of a wait that is just
// over, or of a run that is closing to go on in a new one (Continue-As-New): an event that the
// workflow refuses so, or that finds the run closed, is sent again when it waits.
func (r *runner) sendEvents(ctx context.Context, run *runState) error {
	waitsFor := func(name string) bool {
		where, err := r.flow.Status(ctx, r.client, run.id)
		if err != nil {
			return false // the workflow has not answered a query yet
		}
		return slices.Contains(where.Events, name)
	}
	// taken says whether the workflow took the event: not when it refused it, nor when the run it
	// reached closed as it came
	taken := func(name string, a CallbackAnswer) (bool, error) {
		err := r.flow.Send(ctx, r.client, run.id, name, a)
		if err == nil {
			return true, nil
		}
		var timedOut *client.WorkflowUpdateServiceTimeoutOrCanceledError
		var notFound *serviceerror.NotFound
		if refused(err) || errors.As(err, &timedOut) || errors.As(err, &notFound) {
			return false, nil
		}
		return false, fmt.Errorf("sending the event %s to %s: %w", name, run.id, err)
	}
	for {
		r.mu.Lock()
		if run.done {
			r.mu.Unlock()
			return nil
		}
		name, has := run.events[strconv.Itoa(run.next)]
		var ans map[string]any
		if run.next < len(run.answers) {
			ans = run.answers[run.next]
		}
		r.mu.Unlock()
		if has && ans != nil && !timesOut(ans) && waitsFor(name) {
			// the answer is the event's before it goes: once the workflow takes it, the next call may come at once
			r.mu.Lock()
			run.next++
			run.steps = append(run.steps, map[string]any{"call": map[string]any{"event": name}, "answer": recorded(ans)})
			r.mu.Unlock()
			if ans["cancel"] == true {
				if err := r.client.CancelWorkflow(ctx, run.id, ""); err != nil {
					return err
				}
				continue
			}
			a := callbackAnswer(ans)
			for _, other := range r.flow.Events {
				if other == name {
					continue
				}
				took, err := taken(other, a)
				if err != nil {
					return err
				}
				if took {
					return fmt.Errorf("the workflow took the event %s while it waited for %s", other, name)
				}
			}
			took, err := taken(name, a)
			if err != nil {
				return err
			}
			if !took {
				// not taken: the workflow still waits for it, and nothing else has come
				r.mu.Lock()
				run.next--
				run.steps = run.steps[:len(run.steps)-1]
				r.mu.Unlock()
			}
			continue
		}
		time.Sleep(20 * time.Millisecond)
	}
}

// failure is how a run that did not succeed ended: the error's type (or its text, for an error
// without one) and its message as the workflow sent it, null when it is empty.
func failure(err error) map[string]any {
	cause := err
	var ended *temporal.WorkflowExecutionError
	if errors.As(err, &ended) && ended.Unwrap() != nil {
		cause = ended.Unwrap()
	}
	var name, message string
	switch c := cause.(type) {
	case *temporal.ApplicationError:
		name, message = c.Type(), c.Message()
		if name == "" {
			name = c.Message()
		}
	case *temporal.TimeoutError:
		name, message = c.TimeoutType().String(), c.Message()
	default:
		name, message = c.Error(), c.Error()
	}
	var m any
	if message != "" {
		m = message
	}
	return map[string]any{"error": name, "cause": m}
}

// ended is what one run came to.
type ended struct {
	end       map[string]any
	cases     any
	shown     any
	asked     []any
	histories []*historypb.History
}

// runsOf are the histories of a workflow's runs, from the first: a run that went on in a new one
// (Continue-As-New) is followed by that one.
func runsOf(ctx context.Context, c client.Client, workflowID, firstRunID string) ([]*historypb.History, error) {
	var out []*historypb.History
	for runID := firstRunID; runID != ""; {
		h := &historypb.History{}
		it := c.GetWorkflowHistory(ctx, workflowID, runID, false, enumspb.HISTORY_EVENT_FILTER_TYPE_ALL_EVENT)
		for it.HasNext() {
			e, err := it.Next()
			if err != nil {
				return nil, fmt.Errorf("the history of %s (%s): %w", workflowID, runID, err)
			}
			h.Events = append(h.Events, e)
		}
		if len(h.Events) == 0 {
			return nil, fmt.Errorf("the history of %s (%s) is empty", workflowID, runID)
		}
		out = append(out, h)
		runID = h.Events[len(h.Events)-1].GetWorkflowExecutionContinuedAsNewEventAttributes().GetNewExecutionRunId()
	}
	return out, nil
}

// shownOf is the search attribute DandoriCases of a workflow, or nil when it has none.
func shownOf(ctx context.Context, c client.Client, id string) (any, error) {
	d, err := c.DescribeWorkflowExecution(ctx, id, "")
	if err != nil {
		return nil, err
	}
	p, ok := d.GetWorkflowExecutionInfo().GetSearchAttributes().GetIndexedFields()[casesKey.GetName()]
	if !ok || p == nil {
		return nil, nil
	}
	var list []string
	if err := converter.GetDefaultDataConverter().FromPayload(p, &list); err != nil {
		return nil, fmt.Errorf("reading DandoriCases of %s: %w", id, err)
	}
	if list == nil {
		return nil, nil
	}
	return list, nil
}

// listedText is the cases' states as the search attribute lists them, sorted, as JSON (null for none).
func listedText(cases map[string]any) string {
	var items []string
	for name, state := range cases {
		if state != nil {
			items = append(items, name+"="+text(state))
		}
	}
	sort.Strings(items)
	b, _ := json.Marshal(items)
	return string(b)
}

// sortedText is the search attribute's list, sorted, as JSON (null for none).
func sortedText(shown any) string {
	list, _ := shown.([]string)
	if list != nil {
		list = slices.Clone(list)
		sort.Strings(list)
	}
	b, _ := json.Marshal(list)
	return string(b)
}

func (r *runner) one(ctx context.Context, sc scenario) (ended, error) {
	events := sc.Events
	if events == nil {
		events = map[string]string{}
	}
	run := &runState{id: sc.ID, answers: sc.Answers, events: events, steps: []any{}}
	r.mu.Lock()
	r.runs[sc.ID] = run
	r.mu.Unlock()
	handle, err := r.flow.Start(ctx, r.client, sc.ID, sc.Input, true)
	if err != nil {
		return ended{}, fmt.Errorf("starting %s: %w", sc.ID, err)
	}
	// the first run's id: once Get has followed the runs that went on in a new one, GetRunID is the last's
	firstRunID := handle.GetRunID()
	var sending chan error
	if len(events) > 0 {
		sending = make(chan error, 1)
		go func() { sending <- r.sendEvents(ctx, run) }()
	}
	var out any
	err = handle.Get(ctx, &out)
	var end map[string]any
	var failed *temporal.WorkflowExecutionError
	if err == nil {
		end = map[string]any{"succeed": out}
	} else if errors.As(err, &failed) {
		if _, ok := failed.Unwrap().(*temporal.CanceledError); ok {
			end = map[string]any{"cancel": nil}
		} else {
			end = map[string]any{"fail": failure(err)}
		}
	}
	r.mu.Lock()
	run.done = true
	r.mu.Unlock()
	if sending != nil {
		if serr := <-sending; serr != nil {
			return ended{}, serr
		}
	}
	if end == nil {
		return ended{}, fmt.Errorf("the result of %s: %w", sc.ID, err)
	}
	r.mu.Lock()
	r.settleEvents(run)
	r.mu.Unlock()
	// what the query and the search attribute say of the cases once the run is over; a query
	// the workflow does not answer is a difference to show, and the other runs go on to their end.
	// The search attribute is what the server keeps of the cases as each of them moves, and the query
	// is what the workflow says from its variables: at the end they say the same. When the query says
	// something else, it is asked again, up to three times, and what it said is kept in `asked`, for
	// the test to show.
	var cases any
	var shown any
	asked := []any{}
	for attempt := 1; attempt <= 4; attempt++ {
		where, err := r.flow.Status(ctx, r.client, sc.ID)
		if err != nil {
			cases = map[string]any{"the query failed": err.Error()}
			break
		}
		cases = where.Cases
		if shown, err = shownOf(ctx, r.client, sc.ID); err != nil {
			return ended{}, err
		}
		if listedText(where.Cases) == sortedText(shown) || attempt == 4 {
			break
		}
		asked = append(asked, where.Cases)
		time.Sleep(time.Duration(250*attempt) * time.Millisecond)
	}
	if shown == nil {
		if shown, err = shownOf(ctx, r.client, sc.ID); err != nil {
			return ended{}, err
		}
	}
	histories, err := runsOf(ctx, r.client, sc.ID, firstRunID)
	if err != nil {
		return ended{}, err
	}
	return ended{end: end, cases: cases, shown: shown, asked: asked, histories: histories}, nil
}

// activitiesBy starts the other language's runner, which serves the activities; done closes its
// input, waits for it, and reads the calls of each run.
func activitiesBy(by []string, address string) (done func() (map[string][]any, error), stop func(), err error) {
	dir, err := os.MkdirTemp("", "dandori-steps-")
	if err != nil {
		return nil, nil, err
	}
	stepsFile := filepath.Join(dir, "steps.json")
	cmd := exec.Command(by[0], append(slices.Clone(by[1:]), stepsFile, address)...)
	cmd.Stderr = os.Stderr
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, nil, err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, nil, err
	}
	if err := cmd.Start(); err != nil {
		return nil, nil, fmt.Errorf("starting the activities' runner: %w", err)
	}
	stop = func() {
		if cmd.ProcessState == nil {
			_ = cmd.Process.Kill()
			_ = cmd.Wait()
		}
		_ = os.RemoveAll(dir)
	}
	// its workers poll before the first run starts, so that no activity waits for them
	lines := bufio.NewReader(stdout)
	for {
		line, err := lines.ReadString('\n')
		if strings.Contains(line, "serving") {
			break
		}
		if err != nil {
			stop()
			return nil, nil, errors.New("the activities' runner ended before it served")
		}
	}
	// the rest of its output is read to its end, so that it never waits to write
	drained := make(chan struct{})
	go func() {
		_, _ = io.Copy(io.Discard, lines)
		close(drained)
	}()
	done = func() (map[string][]any, error) {
		_ = stdin.Close()
		<-drained
		if err := cmd.Wait(); err != nil {
			return nil, fmt.Errorf("the activities' runner: %w", err)
		}
		b, err := os.ReadFile(stepsFile)
		if err != nil {
			return nil, err
		}
		var steps map[string][]any
		if err := json.Unmarshal(b, &steps); err != nil {
			return nil, fmt.Errorf("reading the activities' runner's steps: %w", err)
		}
		return steps, nil
	}
	return done, stop, nil
}

// namedHistory is a history with the file it is written to.
type namedHistory struct {
	file    string
	history History
}

func runAll(flow Flow, sp spec, s Settings, historiesDir string) ([]any, error) {
	ctx := context.Background()
	server, err := startServer(serverOptions{timerShift: true, searchAttributes: true})
	if err != nil {
		return nil, err
	}
	defer func() { _ = server.Stop() }()
	r := newRunner(flow, sp, s, server.Client())
	transport := newStandIn(sp, r)
	var by []string
	if v := os.Getenv("DANDORI_ACTIVITIES_BY"); v != "" {
		if err := json.Unmarshal([]byte(v), &by); err != nil {
			return nil, fmt.Errorf("DANDORI_ACTIVITIES_BY is not a JSON list of strings: %w", err)
		}
	}
	childTypes, childQueues := sp.childTypes()
	queues := queuesOf(flow.TaskQueue, sp.Queues)
	var served func() (map[string][]any, error)
	if by != nil {
		// the other language's runner serves the activities; a worker here serves the workflow and
		// the child workflows' stand-ins only
		done, stop, err := activitiesBy(by, server.FrontendHostPort())
		if err != nil {
			return nil, err
		}
		defer stop()
		served = done
		queues = queuesOf(flow.TaskQueue, childQueues)
	}
	// every worker runs until the runs are done
	var workers []worker.Worker
	stopWorkers := func() {
		for _, w := range workers {
			w.Stop()
		}
		workers = nil
	}
	defer stopWorkers()
	for _, q := range queues {
		o := flow.WorkerOptions("")
		o.WorkflowPanicPolicy = worker.FailWorkflow
		if by == nil {
			heartbeatsAtOnce(&o)
		} else {
			// the Go SDK polls for activity tasks with no activity registered, and asks for the
			// workflow's activities itself (eager execution): neither, when they are the other runner's
			o.LocalActivityWorkerOnly = true
		}
		w := worker.New(r.client, q, o)
		flow.RegisterWorkflow(w)
		for _, t := range childTypes {
			w.RegisterWorkflowWithOptions(childStandIn(t), workflow.RegisterOptions{Name: t})
		}
		if by == nil {
			flow.RegisterActivities(w, r.ownTask, transport)
			r.registerStandIns(w)
		}
		if err := w.Start(); err != nil {
			return nil, fmt.Errorf("starting the worker on %s: %w", q, err)
		}
		workers = append(workers, w)
	}
	ends := make([]ended, len(sp.Runs))
	errs := make([]error, len(sp.Runs))
	var wg sync.WaitGroup
	for i, sc := range sp.Runs {
		wg.Add(1)
		go func() {
			defer wg.Done()
			ends[i], errs[i] = r.one(ctx, sc)
		}()
	}
	wg.Wait()
	if err := errors.Join(errs...); err != nil {
		return nil, err
	}
	if served != nil {
		// the calls each run made, as the other language's runner saw them
		steps, err := served()
		if err != nil {
			return nil, err
		}
		for _, sc := range sp.Runs {
			got, ok := steps[sc.ID]
			if !ok {
				return nil, fmt.Errorf("the activities' runner wrote no steps for %s", sc.ID)
			}
			r.runs[sc.ID].steps = got
		}
	}
	var histories []namedHistory
	results := []any{}
	for i, sc := range sp.Runs {
		e := ends[i]
		for n, h := range e.histories {
			file := sc.ID + ".json"
			if n > 0 {
				file = fmt.Sprintf("%s.%d.json", sc.ID, n+1)
			}
			histories = append(histories, namedHistory{file: file, history: History{WorkflowID: sc.ID, History: h}})
		}
		results = append(results, map[string]any{"steps": r.runs[sc.ID].steps, "end": e.end, "cases": e.cases, "shown": e.shown, "asked": e.asked})
	}
	// the generated client finds every run by the workflow's type, the ones that went on in a new
	// run too (the server lists them a moment late)
	for tries := 0; ; tries++ {
		found, err := flow.Histories(ctx, r.client, fmt.Sprintf("WorkflowType = '%s'", flow.WorkflowType))
		if err != nil {
			return nil, fmt.Errorf("the client's histories: %w", err)
		}
		if len(found) == len(histories) {
			break
		}
		if tries == 50 {
			return nil, fmt.Errorf("the client's histories found %d run(s) of %d", len(found), len(histories))
		}
		time.Sleep(100 * time.Millisecond)
	}
	stopWorkers()
	// the same code, replaying what it did, must find it deterministic
	all := make([]History, len(histories))
	for i, h := range histories {
		all[i] = h.history
	}
	if failed := flow.Replay(all); len(failed) > 0 {
		return nil, fmt.Errorf("replaying the history of %s with the same code: %s", failed[0].WorkflowID, failed[0].Error)
	}
	if historiesDir != "" {
		if err := os.MkdirAll(historiesDir, 0o755); err != nil {
			return nil, err
		}
		for _, h := range histories {
			if err := writeHistory(filepath.Join(historiesDir, h.file), h.history.History); err != nil {
				return nil, err
			}
		}
		children := sp.Children
		if children == nil {
			children = []map[string]any{}
		}
		if err := writeJSON(filepath.Join(historiesDir, "spec.json"), map[string]any{"children": children}, false); err != nil {
			return nil, err
		}
	}
	return results, nil
}

// writeHistory writes a history as JSON that client.HistoryFromJSON reads back as it was (which
// is checked), in the form the Python and TypeScript runners write theirs.
func writeHistory(path string, h *historypb.History) error {
	b, err := temporalproto.CustomJSONMarshalOptions{Indent: "  "}.Marshal(h)
	if err != nil {
		return fmt.Errorf("writing %s: %w", path, err)
	}
	back, err := client.HistoryFromJSON(bytes.NewReader(b), client.HistoryJSONOptions{})
	if err != nil {
		return fmt.Errorf("reading back %s: %w", path, err)
	}
	if !proto.Equal(back, h) {
		return fmt.Errorf("%s does not read back as the history it was written from", path)
	}
	return os.WriteFile(path, append(b, '\n'), 0o644)
}

var historyFile = regexp.MustCompile(`(\.\d+)?\.json$`)

// workflowIDOf is the workflow id of a history's file: <workflow id>.json, or <workflow id>.<n>.json for a run that went on from it.
func workflowIDOf(file string) string {
	return historyFile.ReplaceAllString(file, "")
}

// replayDir replays the histories of a directory with the generated package's Replay, and writes
// for each file the error, or null.
func replayDir(flow Flow, dir, outFile string) error {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return err
	}
	var files []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), ".json") && e.Name() != "spec.json" {
			files = append(files, e.Name())
		}
	}
	sort.Strings(files)
	var histories []History
	for _, name := range files {
		f, err := os.Open(filepath.Join(dir, name))
		if err != nil {
			return err
		}
		h, err := client.HistoryFromJSON(f, client.HistoryJSONOptions{})
		_ = f.Close()
		if err != nil {
			return fmt.Errorf("reading %s: %w", name, err)
		}
		histories = append(histories, History{WorkflowID: workflowIDOf(name), History: h})
	}
	why := map[string]string{}
	for _, f := range flow.Replay(histories) {
		why[f.WorkflowID] = f.Error
	}
	results := []any{}
	for _, name := range files {
		var e any
		if w, ok := why[workflowIDOf(name)]; ok {
			e = w
		}
		results = append(results, map[string]any{"file": name, "error": e})
	}
	return writeJSON(outFile, results, true)
}
