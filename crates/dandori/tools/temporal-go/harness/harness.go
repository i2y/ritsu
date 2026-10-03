// Package harness runs workflows that dandori generated for Temporal's Go SDK on a real Temporal
// server — the dev server of the Temporal CLI, which the SDK's testsuite package starts — against
// scripted answers, and writes what they did in the shapes the Python runner writes: the Go twin
// of ../temporal-python/run.py, children.py and versions.py, and of ../transport.py.
//
// One binary, which ../build makes, holds many generated packages, each a copy patched as run.py
// patches its copy. It reaches each package through a Flow, which an adapter in the binary's
// main.go makes from the package: every package has its own Transport, CallbackAnswer, Where and
// History, which are other types to Go than another package's, so the adapter converts them to
// the ones here, which have the same fields.
//
//	<bin> <key> <runs.json> <results.json> [<histories dir>]       (run.go)
//	<bin> --replay <key> <histories dir> <results.json>            (run.go)
//	<bin> --serve <key> <runs.json> <steps.json> <server address>  (run.go)
//	<bin> --children <parent key> <child key> <runs.json> <results.json>  (children.go)
//	<bin> --children-serve <child key> <server address>            (children.go)
//	<bin> --versions <key A> <key B> <results.json>                (versions.go)
//	<bin> --fetch
//
// A key is the name the build's manifest gives a flow. With --fetch, the binary starts a dev
// server and stops it, so that the Temporal CLI is downloaded once, before the runners that run
// at once start theirs. The modes are in the table `modes` below; a new one is an entry there.
// The binary's main.go hands the checks of ../checks their modes first (--wire, --agents,
// --connect-read, --jev), which need a package's readings that only an added file reaches.
package harness

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/exec"
	"sort"
	"strconv"
	"strings"
	"time"

	historypb "go.temporal.io/api/history/v1"
	"go.temporal.io/sdk/client"
	"go.temporal.io/sdk/log"
	"go.temporal.io/sdk/temporal"
	"go.temporal.io/sdk/testsuite"
	"go.temporal.io/sdk/worker"
)

// Flow is one generated package as the binary's main.go hands it over: its names, and its
// functions with the types of this package in place of the package's own.
type Flow struct {
	WorkflowType string
	TaskQueue    string
	Events       []string
	BuildID      string

	RegisterWorkflow   func(r worker.WorkflowRegistry)
	RegisterActivities func(r worker.ActivityRegistry, own OwnFunc, t Transport)
	WorkerOptions      func(deployment string) worker.Options
	NewWorker          func(c client.Client, own OwnFunc, t Transport, deployment string) worker.Worker
	Replay             func(histories []History) []ReplayFailure

	Start     func(ctx context.Context, c client.Client, id string, input any, searchAttributes bool) (client.WorkflowRun, error)
	Answer    func(ctx context.Context, c client.Client, callbackID string, a CallbackAnswer) error
	Send      func(ctx context.Context, c client.Client, workflowID, event string, a CallbackAnswer) error
	Status    func(ctx context.Context, c client.Client, id string) (Where, error)
	Histories func(ctx context.Context, c client.Client, query string) ([]History, error)

	// NewTransport is the package's default Transport (its NewTransport), with TransportOptions'
	// Headers and HTTPClient, and the rest of its options zero: its HTTP, and the OpenAI and
	// Anthropic SDKs' clients, go through the client; the AWS SDK reads AWS_ENDPOINT_URL.
	NewTransport func(headers func(url string) map[string]string, httpClient *http.Client) Transport
}

// Settings are what the build wrote into the copies of the generated code, which the runner
// has to know too.
type Settings struct {
	// How long an activity or a child workflow gets before it times out, in seconds: the
	// DANDORI_TEMPORAL_TIMEOUT of the build, or 20 (run.py's TIMEOUT).
	Timeout int
}

// OwnFunc stands in for every task the user writes: the generated OwnTasksBy calls it with
// the task's name as the .flow writes it.
type OwnFunc func(ctx context.Context, task string, args map[string]any) (any, error)

// The types below have the fields of the generated package's types of the same names, in the
// same order, so that the adapter converts one to the other.

// Outcome is what Lambda or an AWS API answered: OK, or the name the other side gives its error.
type Outcome struct {
	OK      any
	Error   string
	Message string
}

// HTTPRequest is a request as Step Functions' HTTP Task sends it.
type HTTPRequest struct {
	Method   string            `json:"http"`
	URL      string            `json:"url"`
	Headers  map[string]string `json:"headers,omitempty"`
	Body     any               `json:"body,omitempty"`
	Query    any               `json:"query,omitempty"`
	Form     bool              `json:"form,omitempty"`
	TypeSafe bool              `json:"typesafe,omitempty"`
}

// HTTPResponse is the status of an HTTP answer, and its body (parsed when it is JSON).
type HTTPResponse struct {
	Status int
	Body   any
}

// AgentCall is one call of an agent, as the generated code hands it to the Transport.
type AgentCall struct {
	Agent        string         `json:"agent"`
	Provider     string         `json:"provider"`
	Model        string         `json:"model"`
	Instructions string         `json:"instructions"`
	Input        map[string]any `json:"input"`
	Schema       map[string]any `json:"schema"`
	URL          string         `json:"url,omitempty"`
	Effort       string         `json:"effort,omitempty"`
}

// Transport is how the generated tasks that say lambda, http, aws, agent or jev reach the other side.
type Transport interface {
	Lambda(ctx context.Context, fn string, payload map[string]any) (Outcome, error)
	HTTP(ctx context.Context, req HTTPRequest) (HTTPResponse, error)
	AWS(ctx context.Context, service, action string, input map[string]any) (Outcome, error)
	Agent(ctx context.Context, call AgentCall) (any, error)
}

// History is the history of one run of a workflow.
type History struct {
	WorkflowID string
	History    *historypb.History
}

// ReplayFailure is a workflow whose history the code finds nondeterministic, with why.
type ReplayFailure struct {
	WorkflowID string
	Error      string
}

// CallbackAnswer is the answer to a callback or an event: OK, or the error it names ("" = OK).
type CallbackAnswer struct {
	OK      any
	Error   string
	Message string
}

// Where is what the query dandori.status says: the line the workflow is at, each case's state,
// and the events it waits for now.
type Where struct {
	At     *int           `json:"at"`
	Cases  map[string]any `json:"cases"`
	Events []string       `json:"events"`
}

// Main runs the mode the arguments name with the flows of the binary, and exits with 1 on an error.
func Main(flows map[string]Flow, s Settings) {
	if err := dispatch(flows, s, os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

// mode is one way to run the binary: `<bin> <name> <args>…`. run gets the arguments after the
// name, as many as usage names (the ones in brackets may be left out).
type mode struct {
	usage string
	min   int
	max   int
	run   func(flows map[string]Flow, s Settings, args []string) error
}

// modes by their names; "" is the mode without one, run.py's main mode.
var modes = map[string]mode{
	"": {"<key> <runs.json> <results.json> [<histories dir>]", 3, 4, func(flows map[string]Flow, s Settings, args []string) error {
		f, err := flowOf(flows, args[0])
		if err != nil {
			return err
		}
		sp, err := readSpec(args[1])
		if err != nil {
			return err
		}
		histories := ""
		if len(args) == 4 {
			histories = args[3]
		}
		results, err := runAll(f, sp, s, histories)
		if err != nil {
			return err
		}
		return writeJSON(args[2], results, true)
	}},
	"--replay": {"<key> <histories dir> <results.json>", 3, 3, func(flows map[string]Flow, s Settings, args []string) error {
		f, err := flowOf(flows, args[0])
		if err != nil {
			return err
		}
		return replayDir(f, args[1], args[2])
	}},
	"--serve": {"<key> <runs.json> <steps.json> <server address>", 4, 4, func(flows map[string]Flow, s Settings, args []string) error {
		f, err := flowOf(flows, args[0])
		if err != nil {
			return err
		}
		sp, err := readSpec(args[1])
		if err != nil {
			return err
		}
		return serve(f, sp, s, args[2], args[3])
	}},
	"--children": {"<parent key> <child key> <runs.json> <results.json>", 4, 4, func(flows map[string]Flow, s Settings, args []string) error {
		parent, err := flowOf(flows, args[0])
		if err != nil {
			return err
		}
		child, err := flowOf(flows, args[1])
		if err != nil {
			return err
		}
		return children(parent, child, args[2], args[3])
	}},
	"--children-serve": {"<child key> <server address>", 2, 2, func(flows map[string]Flow, s Settings, args []string) error {
		child, err := flowOf(flows, args[0])
		if err != nil {
			return err
		}
		return serveChild(child, args[1])
	}},
	"--versions": {"<key A> <key B> <results.json>", 3, 3, func(flows map[string]Flow, s Settings, args []string) error {
		a, err := flowOf(flows, args[0])
		if err != nil {
			return err
		}
		b, err := flowOf(flows, args[1])
		if err != nil {
			return err
		}
		return versions(a, b, args[2])
	}},
	"--fetch": {"", 0, 0, func(flows map[string]Flow, s Settings, args []string) error {
		server, err := startServer(serverOptions{})
		if err != nil {
			return err
		}
		return server.Stop()
	}},
}

func usage() string {
	names := make([]string, 0, len(modes))
	for name := range modes {
		names = append(names, name)
	}
	sort.Strings(names)
	var b strings.Builder
	b.WriteString("usage:")
	for _, name := range names {
		line := strings.TrimSpace(name + " " + modes[name].usage)
		b.WriteString("\n  <bin> " + line)
	}
	return b.String()
}

func dispatch(flows map[string]Flow, s Settings, args []string) error {
	name := ""
	if len(args) > 0 && strings.HasPrefix(args[0], "--") {
		name, args = args[0], args[1:]
	}
	m, ok := modes[name]
	if !ok || len(args) < m.min || len(args) > m.max {
		return fmt.Errorf("%s", usage())
	}
	return m.run(flows, s, args)
}

// flowOf is the flow the build's manifest gave the key.
func flowOf(flows map[string]Flow, key string) (Flow, error) {
	f, ok := flows[key]
	if !ok {
		keys := make([]string, 0, len(flows))
		for k := range flows {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		return Flow{}, fmt.Errorf("no flow %q in this binary; it has %s", key, strings.Join(keys, ", "))
	}
	return f, nil
}

// quietLogger is the workers' and the clients' log: errors only, to stderr. The results go to files.
func quietLogger() log.Logger {
	return log.NewStructuredLogger(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelError})))
}

// casesKey is the search attribute that lists the cases' states, as "<case>=<state>".
var casesKey = temporal.NewSearchAttributeKeyKeywordList("DandoriCases")

type serverOptions struct {
	// the server fires a timer up to a second late unless told to shift its timers less
	timerShift bool
	// DandoriCases registered on the namespace
	searchAttributes bool
}

// startServer starts a dev server, whose log goes to stderr: the runner's stdout is read by the
// runner that started it (--serve). With every flow's runner starting a dev server at once, a
// server can be later than the SDK waits for it (a minute here; run.py's SDK waits less);
// nothing has been started on it, so it is stopped and started again, up to twice.
func startServer(o serverOptions) (*testsuite.DevServer, error) {
	options := testsuite.DevServerOptions{
		ClientOptions: &client.Options{Logger: quietLogger()},
		LogLevel:      "error",
		Stdout:        os.Stderr,
		Stderr:        os.Stderr,
	}
	if o.timerShift {
		options.ExtraArgs = []string{"--dynamic-config-value", `history.timerProcessorMaxTimeShift="10ms"`}
	}
	if o.searchAttributes {
		options.SearchAttributes = temporal.NewSearchAttributes(casesKey.ValueSet([]string{}))
	}
	for attempt := 1; ; attempt++ {
		server, err := testsuite.StartDevServer(context.Background(), options)
		if err == nil {
			return server, nil
		}
		if attempt == 3 || !strings.Contains(err.Error(), "failed connecting after timeout") {
			return nil, fmt.Errorf("starting the dev server: %w", err)
		}
		// the SDK leaves the server's process running when it gives up waiting for it
		stopStrayServers()
		fmt.Fprintf(os.Stderr, "the dev server did not start in time; starting it again (%d)\n", attempt)
		time.Sleep(time.Duration(attempt) * time.Second)
	}
}

// stopStrayServers kills the dev servers this process started and lost: its child processes
// that run `server start-dev`.
func stopStrayServers() {
	out, err := exec.Command("pgrep", "-P", strconv.Itoa(os.Getpid())).Output()
	if err != nil {
		return
	}
	for _, field := range strings.Fields(string(out)) {
		pid, err := strconv.Atoi(field)
		if err != nil {
			continue
		}
		command, err := exec.Command("ps", "-o", "command=", "-p", field).Output()
		if err == nil && strings.Contains(string(command), "start-dev") {
			if p, err := os.FindProcess(pid); err == nil {
				_ = p.Kill()
				_, _ = p.Wait()
			}
		}
	}
}

// dial connects to a server another runner started.
func dial(address string) (client.Client, error) {
	c, err := client.Dial(client.Options{HostPort: address, Logger: quietLogger()})
	if err != nil {
		return nil, fmt.Errorf("connecting to %s: %w", address, err)
	}
	return c, nil
}

// writeJSON writes a value as JSON, with the text as it is (no \u escapes for what is not
// ASCII, nor for <, > and &), indented as run.py indents its results when `indent`.
func writeJSON(path string, v any, indent bool) error {
	var b bytes.Buffer
	enc := json.NewEncoder(&b)
	enc.SetEscapeHTML(false)
	if indent {
		enc.SetIndent("", "  ")
	}
	if err := enc.Encode(v); err != nil {
		return fmt.Errorf("writing %s: %w", path, err)
	}
	return os.WriteFile(path, b.Bytes(), 0o644)
}

// until asks `ok` every 100 ms, 300 times at most, as versions.py does.
func until(what string, ok func() (bool, error)) error {
	for range 300 {
		got, err := ok()
		if err != nil {
			return err
		}
		if got {
			return nil
		}
		time.Sleep(100 * time.Millisecond)
	}
	return fmt.Errorf("gave up waiting until %s", what)
}
