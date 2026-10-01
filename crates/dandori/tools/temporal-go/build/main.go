// Builds one binary that runs many workflows dandori generated for Temporal's Go SDK (a Go
// package each) with the runner of ../harness: a binary a flow would cost far more memory and
// time. Run it in tools/temporal-go:
//
//	go run ./build <out binary> <manifest.json>
//
// manifest.json, the flows of the binary:
//
//	[ { "key": the name the binary knows the flow by, "dir": the generated package's directory,
//	    "patch": "full" | "continue" | "none",
//	    "callbacks": [the function of each task that hands on a callback's id] } ]
//
// It makes a module of its own in a temporary directory (go.mod, go.sum and harness/ of this one),
// copies each package's Go files to flows/<the key made safe for an import path>/, patching them,
// writes main.go, where an adapter converts each package's types to the harness's, and builds
// the binary (GOWORK=off, -mod=readonly: what the generated packages import must be required by
// go.mod, and summed in go.sum). Then it runs `<out binary> --fetch` once, which downloads the
// Temporal CLI for the dev servers, so that the runners that start at once do not race to download
// it. On a failure it says why and exits with 1; when go build fails, it prints the build's errors
// and keeps the temporary directory they point into.
//
// The patches make the copy wait far less than the generated code does, as
// ../../temporal-python/run.py patches its copy (see there why each is what it is):
//
//   - "full": in workflow.go, the rounds of `for … in parallel` run one at a time (ddAtATime(1));
//     in every file, an activity or a child workflow gets TIMEOUT seconds before it times out
//     (DANDORI_TEMPORAL_TIMEOUT when the binary is built, or 20; the binary knows it too), but for
//     the task that hands on a callback's id (the line after `func <callback>(`); in runtime.go,
//     every duration of the workflow's timers is at most 10 ms (ddSeconds), an event is waited for
//     5 seconds at most, and every history is long enough to go on in a new run (ddContinueAt);
//     and the rules are stand-ins, so rules.go (when there is one) bundles none of rulec's code.
//   - "continue": only every history long enough to go on in a new run (as versions.py patches).
//   - "none": nothing (the children's flows, which run as they are).
//
// A patch whose place is not found in runtime.go is an error, so that a change in the generator shows.
package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"text/template"
)

type entry struct {
	Key       string   `json:"key"`
	Dir       string   `json:"dir"`
	Patch     string   `json:"patch"`
	Callbacks []string `json:"callbacks"`
}

func main() {
	if len(os.Args) != 3 {
		fmt.Fprintln(os.Stderr, "usage: go run ./build <out binary> <manifest.json>")
		os.Exit(2)
	}
	if err := build(os.Args[1], os.Args[2]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func build(out, manifestFile string) error {
	out, err := filepath.Abs(out)
	if err != nil {
		return err
	}
	b, err := os.ReadFile(manifestFile)
	if err != nil {
		return err
	}
	var entries []entry
	if err := json.Unmarshal(b, &entries); err != nil {
		return fmt.Errorf("reading %s: %w", manifestFile, err)
	}
	if len(entries) == 0 {
		return fmt.Errorf("%s names no flow", manifestFile)
	}
	timeout := 20
	if v := os.Getenv("DANDORI_TEMPORAL_TIMEOUT"); v != "" {
		if timeout, err = strconv.Atoi(v); err != nil {
			return fmt.Errorf("DANDORI_TEMPORAL_TIMEOUT is not a number of seconds: %w", err)
		}
	}
	work, err := os.MkdirTemp("", "dandori-temporal-go-")
	if err != nil {
		return err
	}
	// the files are kept when go build fails, for its errors to be read where they point
	keep := false
	defer func() {
		if keep {
			fmt.Fprintf(os.Stderr, "the files of the build are kept in %s\n", work)
		} else {
			_ = os.RemoveAll(work)
		}
	}()

	// the module: this one's go.mod, go.sum, harness and checks
	for _, f := range []string{"go.mod", "go.sum"} {
		if err := copyFile(f, filepath.Join(work, f)); err != nil {
			return fmt.Errorf("%w (run the build in tools/temporal-go)", err)
		}
	}
	for _, pkg := range []string{"harness", "checks"} {
		files, err := goFiles(pkg)
		if err != nil {
			return err
		}
		for name, text := range files {
			if err := write(filepath.Join(work, pkg, name), text); err != nil {
				return err
			}
		}
	}

	var flows []flowRef
	seen := map[string]bool{}
	for i, e := range entries {
		if seen[e.Key] {
			return fmt.Errorf("the manifest names %q twice", e.Key)
		}
		seen[e.Key] = true
		dir := fmt.Sprintf("f%d_%s", i, safe(e.Key))
		files, err := goFiles(e.Dir)
		if err != nil {
			return fmt.Errorf("%s: %w", e.Key, err)
		}
		if err := patch(files, e, timeout); err != nil {
			return fmt.Errorf("%s (%s): %w", e.Key, e.Dir, err)
		}
		// the package's own readings of a rule's answer and of Jev's, for the checks to call
		m := packageClause.FindStringSubmatch(files["io.go"])
		if m == nil {
			return fmt.Errorf("%s: io.go has no package clause", e.Key)
		}
		files["dd_checks.go"] = strings.ReplaceAll(checksHooks, "{{PKG}}", m[1])
		for name, text := range files {
			if err := write(filepath.Join(work, "flows", dir, name), text); err != nil {
				return err
			}
		}
		flows = append(flows, flowRef{Key: e.Key, Alias: fmt.Sprintf("f%d", i), Import: "temporalgo/flows/" + dir})
	}
	var main strings.Builder
	if err := mainTemplate.Execute(&main, map[string]any{"Flows": flows, "Timeout": timeout}); err != nil {
		return err
	}
	if err := write(filepath.Join(work, "main.go"), main.String()); err != nil {
		return err
	}

	cmd := exec.Command("go", "build", "-o", out, ".")
	cmd.Dir = work
	cmd.Env = append(os.Environ(), "GOWORK=off", "GOFLAGS=-mod=readonly")
	if output, err := cmd.CombinedOutput(); err != nil {
		keep = true
		return fmt.Errorf("go build failed (%v):\n%s", err, output)
	}
	fetch := exec.Command(out, "--fetch")
	fetch.Stdout = os.Stderr
	fetch.Stderr = os.Stderr
	if err := fetch.Run(); err != nil {
		return fmt.Errorf("%s --fetch: %w", out, err)
	}
	return nil
}

type flowRef struct {
	Key    string
	Alias  string
	Import string
}

// safe is a key made into a directory name that an import path can hold: ASCII letters and
// digits, and _ for the rest (the index before it keeps two keys apart).
func safe(key string) string {
	var b strings.Builder
	for _, r := range strings.ToLower(key) {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') {
			b.WriteRune(r)
		} else {
			b.WriteRune('_')
		}
	}
	s := b.String()
	if len(s) > 60 {
		s = s[:60]
	}
	return s
}

// goFiles are the Go files of a package's directory (not its tests, nor its subdirectories, where
// rulec's modules go), by name.
func goFiles(dir string) (map[string]string, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	files := map[string]string{}
	for _, e := range entries {
		name := e.Name()
		if e.IsDir() || !strings.HasSuffix(name, ".go") || strings.HasSuffix(name, "_test.go") {
			continue
		}
		b, err := os.ReadFile(filepath.Join(dir, name))
		if err != nil {
			return nil, err
		}
		files[name] = string(b)
	}
	if len(files) == 0 {
		return nil, fmt.Errorf("no Go files in %s", dir)
	}
	return files, nil
}

var (
	packageClause    = regexp.MustCompile(`(?m)^package\s+(\w+)`)
	atATime          = regexp.MustCompile(`ddAtATime\(\d+\)`)
	startToClose     = regexp.MustCompile(`StartToCloseTimeout: \d+ \* time\.Second`)
	executionTimeout = regexp.MustCompile(`WorkflowExecutionTimeout: \d+ \* time\.Second`)
	importsMath      = regexp.MustCompile(`(?m)^\s*(import\s+)?"math"\s*$`)
)

const (
	// every wait of the workflow goes through ddSeconds
	secondsMarker = "return time.Duration(n * float64(time.Second))"
	secondsShort  = "return time.Duration(math.Min(n, 0.01) * float64(time.Second))"
	continueAt    = "const ddContinueAt = 10000"
	continueSoon  = "const ddContinueAt = 1"
	// an event's wait is not a timer to shorten to nothing: the runner has to see it, and send the event
	eventWait  = "workflow.AwaitWithTimeout(ctx, ddSeconds(limit), func() bool { _, ok := r.events[name]; return ok })"
	eventShort = "workflow.AwaitWithTimeout(ctx, time.Duration(math.Min(limit, 5)*float64(time.Second)), func() bool { _, ok := r.events[name]; return ok })"
)

func patch(files map[string]string, e entry, timeout int) error {
	switch e.Patch {
	case "none":
		return nil
	case "continue":
		rt, ok := files["runtime.go"]
		if !ok {
			return errors.New("no runtime.go")
		}
		if !strings.Contains(rt, continueAt) {
			return errors.New("runtime.go has no ddContinueAt to lower")
		}
		files["runtime.go"] = strings.Replace(rt, continueAt, continueSoon, 1)
		return nil
	case "full":
	default:
		return fmt.Errorf("no patch %q (full, continue or none)", e.Patch)
	}
	wf, ok := files["workflow.go"]
	if !ok {
		return errors.New("no workflow.go")
	}
	files["workflow.go"] = atATime.ReplaceAllString(wf, "ddAtATime(1)")
	short := fmt.Sprintf("StartToCloseTimeout: %d * time.Second", timeout)
	for name, text := range files {
		lines := strings.Split(text, "\n")
		for i, line := range lines {
			// a task that hands on a callback's id keeps its timeout: the line after its `func`
			keep := false
			for _, c := range e.Callbacks {
				if i > 0 && strings.HasPrefix(lines[i-1], "func "+c+"(") {
					keep = true
				}
			}
			if !keep {
				lines[i] = startToClose.ReplaceAllString(line, short)
			}
		}
		text = strings.Join(lines, "\n")
		files[name] = executionTimeout.ReplaceAllString(text, fmt.Sprintf("WorkflowExecutionTimeout: %d * time.Second", timeout))
	}
	rt, ok := files["runtime.go"]
	if !ok {
		return errors.New("no runtime.go")
	}
	for _, p := range []struct{ marker, by, why string }{
		{secondsMarker, secondsShort, "runtime.go has no ddSeconds to shorten"},
		{continueAt, continueSoon, "runtime.go has no ddContinueAt to lower"},
		{eventWait, eventShort, "runtime.go has no event's wait to shorten"},
	} {
		if !strings.Contains(rt, p.marker) {
			return errors.New(p.why)
		}
		rt = strings.Replace(rt, p.marker, p.by, 1)
	}
	files["runtime.go"] = withMath(rt)
	// the rules are stand-ins here, so the build needs none of rulec's packages
	if rules, ok := files["rules.go"]; ok {
		m := packageClause.FindStringSubmatch(rules)
		if m == nil {
			return errors.New("rules.go has no package clause")
		}
		files["rules.go"] = "package " + m[1] + "\n\nvar ddRules = map[string]any{}\n"
	}
	return nil
}

// checksHooks is a file the build adds to each package: its own readings of a rule's answer from
// the rule's Connect service and of Jev's answer, which the checks call (--connect-read, --jev).
const checksHooks = `// Written by tools/temporal-go/build, for the checks.

package {{PKG}}

import (
	"context"
	"errors"
)

// DDReadRule reads body as the answer of a rule's Connect service, as wire says.
func DDReadRule(wire, body any) (any, error) {
	return ddConnectRule(context.Background(), ddBody{body}, wire, map[string]any{})
}

// DDJev reads body as Jev's answer, as spec says.
func DDJev(body, spec any) (any, error) {
	return ddJev(body, spec)
}

// ddBody is a Transport whose every HTTP request is answered with the body.
type ddBody struct{ body any }

func (t ddBody) Lambda(context.Context, string, map[string]any) (Outcome, error) {
	return Outcome{}, errors.New("not here")
}

func (t ddBody) HTTP(context.Context, HTTPRequest) (HTTPResponse, error) {
	return HTTPResponse{Status: 200, Body: t.body}, nil
}

func (t ddBody) AWS(context.Context, string, string, map[string]any) (Outcome, error) {
	return Outcome{}, errors.New("not here")
}

func (t ddBody) Agent(context.Context, AgentCall) (any, error) {
	return nil, errors.New("not here")
}
`

// withMath is a file that imports "math", which the shortened waits use.
func withMath(text string) string {
	if importsMath.MatchString(text) {
		return text
	}
	loc := packageClause.FindStringIndex(text)
	if loc == nil {
		return text
	}
	return text[:loc[1]] + "\n\nimport \"math\"" + text[loc[1]:]
}

func copyFile(from, to string) error {
	b, err := os.ReadFile(from)
	if err != nil {
		return err
	}
	return write(to, string(b))
}

func write(path, text string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, []byte(text), 0o644)
}

// mainTemplate is the binary's main.go: for each package, an adapter that implements the
// package's Transport with the harness's (and the harness's with the package's default one), and
// converts its other types to the harness's, which have the same fields; and the harness's Main
// with every flow by its key.
var mainTemplate = template.Must(template.New("main").Parse(`// Code written by tools/temporal-go/build. DO NOT EDIT.
// The flows of this binary, each through an adapter that converts its package's types to the harness's.
package main

import (
	"context"
	"net/http"
	"os"

	"go.temporal.io/sdk/client"
	"go.temporal.io/sdk/worker"

	"temporalgo/checks"
	"temporalgo/harness"
{{range .Flows}}	{{.Alias}} "{{.Import}}"
{{end}})
{{range .Flows}}
// {{.Alias}}: {{printf "%q" .Key}}

type {{.Alias}}Transport struct{ t harness.Transport }

func (a {{.Alias}}Transport) Lambda(ctx context.Context, fn string, payload map[string]any) ({{.Alias}}.Outcome, error) {
	o, err := a.t.Lambda(ctx, fn, payload)
	return {{.Alias}}.Outcome(o), err
}

func (a {{.Alias}}Transport) HTTP(ctx context.Context, req {{.Alias}}.HTTPRequest) ({{.Alias}}.HTTPResponse, error) {
	r, err := a.t.HTTP(ctx, harness.HTTPRequest(req))
	return {{.Alias}}.HTTPResponse(r), err
}

func (a {{.Alias}}Transport) AWS(ctx context.Context, service, action string, input map[string]any) ({{.Alias}}.Outcome, error) {
	o, err := a.t.AWS(ctx, service, action, input)
	return {{.Alias}}.Outcome(o), err
}

func (a {{.Alias}}Transport) Agent(ctx context.Context, call {{.Alias}}.AgentCall) (any, error) {
	return a.t.Agent(ctx, harness.AgentCall(call))
}

func {{.Alias}}TransportOf(t harness.Transport) {{.Alias}}.Transport {
	if t == nil {
		return nil
	}
	return {{.Alias}}Transport{t}
}

// {{.Alias}}Default is the package's own Transport as the harness's.
type {{.Alias}}Default struct{ t {{.Alias}}.Transport }

func (a {{.Alias}}Default) Lambda(ctx context.Context, fn string, payload map[string]any) (harness.Outcome, error) {
	o, err := a.t.Lambda(ctx, fn, payload)
	return harness.Outcome(o), err
}

func (a {{.Alias}}Default) HTTP(ctx context.Context, req harness.HTTPRequest) (harness.HTTPResponse, error) {
	r, err := a.t.HTTP(ctx, {{.Alias}}.HTTPRequest(req))
	return harness.HTTPResponse(r), err
}

func (a {{.Alias}}Default) AWS(ctx context.Context, service, action string, input map[string]any) (harness.Outcome, error) {
	o, err := a.t.AWS(ctx, service, action, input)
	return harness.Outcome(o), err
}

func (a {{.Alias}}Default) Agent(ctx context.Context, call harness.AgentCall) (any, error) {
	return a.t.Agent(ctx, {{.Alias}}.AgentCall(call))
}

func {{.Alias}}Flow() harness.Flow {
	return harness.Flow{
		WorkflowType:     {{.Alias}}.WorkflowType,
		TaskQueue:        {{.Alias}}.TaskQueue,
		Events:           {{.Alias}}.Events,
		BuildID:          {{.Alias}}.BuildID,
		RegisterWorkflow: {{.Alias}}.RegisterWorkflow,
		RegisterActivities: func(r worker.ActivityRegistry, own harness.OwnFunc, t harness.Transport) {
			{{.Alias}}.RegisterActivities(r, {{.Alias}}.OwnTasksBy(own), {{.Alias}}TransportOf(t))
		},
		WorkerOptions: {{.Alias}}.WorkerOptions,
		NewWorker: func(c client.Client, own harness.OwnFunc, t harness.Transport, deployment string) worker.Worker {
			return {{.Alias}}.NewWorker(c, {{.Alias}}.OwnTasksBy(own), {{.Alias}}TransportOf(t), deployment)
		},
		Replay: func(histories []harness.History) []harness.ReplayFailure {
			in := make([]{{.Alias}}.History, len(histories))
			for i, h := range histories {
				in[i] = {{.Alias}}.History(h)
			}
			failed := {{.Alias}}.Replay(in)
			out := make([]harness.ReplayFailure, len(failed))
			for i, f := range failed {
				out[i] = harness.ReplayFailure(f)
			}
			return out
		},
		Start: {{.Alias}}.Start,
		Answer: func(ctx context.Context, c client.Client, callbackID string, a harness.CallbackAnswer) error {
			return {{.Alias}}.Answer(ctx, c, callbackID, {{.Alias}}.CallbackAnswer(a))
		},
		Send: func(ctx context.Context, c client.Client, workflowID, event string, a harness.CallbackAnswer) error {
			return {{.Alias}}.Send(ctx, c, workflowID, event, {{.Alias}}.CallbackAnswer(a))
		},
		Status: func(ctx context.Context, c client.Client, id string) (harness.Where, error) {
			w, err := {{.Alias}}.Status(ctx, c, id)
			return harness.Where(w), err
		},
		Histories: func(ctx context.Context, c client.Client, query string) ([]harness.History, error) {
			hs, err := {{.Alias}}.Histories(ctx, c, query)
			out := make([]harness.History, len(hs))
			for i, h := range hs {
				out[i] = harness.History(h)
			}
			return out, err
		},
		NewTransport: func(headers func(url string) map[string]string, httpClient *http.Client) harness.Transport {
			return {{.Alias}}Default{ {{- .Alias}}.NewTransport({{.Alias}}.TransportOptions{Headers: headers, HTTPClient: httpClient})}
		},
	}
}
{{end}}
func main() {
	flows := map[string]harness.Flow{
{{range .Flows}}		{{printf "%q" .Key}}: {{.Alias}}Flow(),
{{end}}	}
	readRule := map[string]func(wire, body any) (any, error){
{{range .Flows}}		{{printf "%q" .Key}}: {{.Alias}}.DDReadRule,
{{end}}	}
	jev := map[string]func(body, spec any) (any, error){
{{range .Flows}}		{{printf "%q" .Key}}: {{.Alias}}.DDJev,
{{end}}	}
	if checks.Run(flows, readRule, jev, os.Args[1:]) {
		return
	}
	harness.Main(flows, harness.Settings{Timeout: {{.Timeout}}})
}
`))
