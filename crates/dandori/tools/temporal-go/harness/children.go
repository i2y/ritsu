package harness

// A workflow and the .flow it runs as its child (`flow "<path>"`), both as dandori writes them, on
// one Temporal server (the dev server): the parent's worker and the child's, each on its own task
// queue, made by their generated NewWorker. The Go twin of ../../temporal-python/children.py:
// nothing stands in for the child, and with DANDORI_CHILD_BY (a JSON command, to which the
// server's address is added) the other language's runner serves the child until its input closes.
//
//	<bin> --children <parent key> <child key> <runs.json> <results.json>
//	<bin> --children-serve <child key> <server address>
//
// runs.json: [ { "id", "input" } ]
// results.json: [ { "end": { "succeed": the outputs } | { "fail": { "error", "cause" } } } ]

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"slices"
	"strings"

	"go.temporal.io/sdk/client"
	"go.temporal.io/sdk/temporal"
	"go.temporal.io/sdk/worker"
)

// noTasks: the flows here call nothing but each other, so no task is the user's.
func noTasks(_ context.Context, task string, _ map[string]any) (any, error) {
	return nil, temporal.NewNonRetryableApplicationError("no task is the user's here: "+task, "Dandori.Test.NoStandIn", nil)
}

// buildWorker is a build's worker, on its own task queue, as its generated NewWorker makes it.
func buildWorker(c client.Client, f Flow) worker.Worker {
	return f.NewWorker(c, noTasks, nil, "")
}

// serveChild serves the child on a server another runner started, until this process's input closes.
func serveChild(child Flow, address string) error {
	c, err := dial(address)
	if err != nil {
		return err
	}
	defer c.Close()
	w := buildWorker(c, child)
	if err := w.Start(); err != nil {
		return fmt.Errorf("starting the child's worker: %w", err)
	}
	defer w.Stop()
	if _, err := os.Stdout.WriteString("serving\n"); err != nil {
		return err
	}
	_, err = io.Copy(io.Discard, os.Stdin)
	return err
}

func children(parent, child Flow, runsFile, outFile string) error {
	b, err := os.ReadFile(runsFile)
	if err != nil {
		return err
	}
	var runs []struct {
		ID    string `json:"id"`
		Input any    `json:"input"`
	}
	if err := json.Unmarshal(b, &runs); err != nil {
		return fmt.Errorf("reading %s: %w", runsFile, err)
	}
	var childBy []string
	if v, ok := os.LookupEnv("DANDORI_CHILD_BY"); ok {
		if err := json.Unmarshal([]byte(v), &childBy); err != nil {
			return fmt.Errorf("DANDORI_CHILD_BY is not a JSON list of strings: %w", err)
		}
	}
	server, err := startServer(serverOptions{})
	if err != nil {
		return err
	}
	defer func() { _ = server.Stop() }()
	c := server.Client()
	ctx := context.Background()
	var elsewhere *exec.Cmd
	var elsewhereIn io.WriteCloser
	defer func() {
		if elsewhere != nil && elsewhere.ProcessState == nil {
			_ = elsewhere.Process.Kill()
			_ = elsewhere.Wait()
		}
	}()
	if childBy != nil {
		// the other language's runner serves the child; its workers poll before the first run starts
		elsewhere = exec.Command(childBy[0], append(slices.Clone(childBy[1:]), server.FrontendHostPort())...)
		elsewhere.Stderr = os.Stderr
		if elsewhereIn, err = elsewhere.StdinPipe(); err != nil {
			return err
		}
		out, err := elsewhere.StdoutPipe()
		if err != nil {
			return err
		}
		if err := elsewhere.Start(); err != nil {
			return fmt.Errorf("starting the child's runner: %w", err)
		}
		lines := bufio.NewReader(out)
		for {
			line, err := lines.ReadString('\n')
			if strings.Contains(line, "serving") {
				break
			}
			if err != nil {
				_ = elsewhere.Wait()
				return fmt.Errorf("the child's runner exited with %d", elsewhere.ProcessState.ExitCode())
			}
		}
		go func() { _, _ = io.Copy(io.Discard, lines) }()
	}
	workers := []worker.Worker{buildWorker(c, parent)}
	if childBy == nil {
		workers = append(workers, buildWorker(c, child))
	}
	stop := func() {
		for i := len(workers) - 1; i >= 0; i-- {
			workers[i].Stop()
		}
		workers = nil
	}
	defer stop()
	for _, w := range workers {
		if err := w.Start(); err != nil {
			return fmt.Errorf("starting a worker: %w", err)
		}
	}
	results := []any{}
	for _, r := range runs {
		handle, err := parent.Start(ctx, c, r.ID, r.Input, false)
		if err != nil {
			return fmt.Errorf("starting %s: %w", r.ID, err)
		}
		var out any
		var end map[string]any
		var failed *temporal.WorkflowExecutionError
		if err := handle.Get(ctx, &out); err == nil {
			end = map[string]any{"succeed": out}
		} else if errors.As(err, &failed) {
			end = map[string]any{"fail": failure(err)}
		} else {
			return fmt.Errorf("the result of %s: %w", r.ID, err)
		}
		results = append(results, map[string]any{"end": end})
	}
	stop()
	if elsewhere != nil {
		_ = elsewhereIn.Close()
		if err := elsewhere.Wait(); err != nil {
			return fmt.Errorf("the child's runner exited with %d", elsewhere.ProcessState.ExitCode())
		}
	}
	return writeJSON(outFile, results, true)
}
