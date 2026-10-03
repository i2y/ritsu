package harness

// A Transport for the Go dandori writes, in place of Lambda, HTTP, the AWS APIs and the agents:
// every call goes to `run.take(call)` as Step Functions would send it (an agent's, as the
// generated code hands it over), which writes it down and gives the scenario's answer. The Go
// twin of ../../transport.py: the calls are written down in the shapes it writes them in.
//
// spec "http": [ { method, url, errors: { <error>: <status> } } ]  (url with {placeholders}; the
//   Jev tasks all send to one URL, and an error takes its status from the task that declares it)
// spec "aws":  [ { api: "<service>:<action>", errors: { <error>: <exception> }, keyParam } ]
//
// A callback task's submit hands on `callback_id` (in the Lambda payload, or in the SQS
// message); the call is written down without it, and `run.answerLater(id, answer)` is called with
// the answer the scenario gives the callback.
//
// An agent answers {"answer": <the scenario's value>}, as the model would under the schema; its
// failure is an error, as the default transport's refusal is. A Jev task's call is an HTTP
// request, whose body is the scenario's answer (Jev's response); `typesafe`, which says it is
// one, is not written down.
//
// A call that the scenario times out, or cancels the workflow during, goes to `run.hold(answer)`:
// the runner keeps the call from answering until the server times it out, or cancels the
// workflow and keeps the call until the server cancels it.

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"strings"
)

// taker is what the stand-in transport needs of the runner: run.py's Run.
type taker interface {
	// take writes the call down with the next answer of the run that makes it, and gives the answer.
	take(ctx context.Context, call map[string]any) (map[string]any, error)
	// answerLater answers the callback the id names with the scenario's answer.
	answerLater(ctx context.Context, callbackID string, ans map[string]any) error
	// hold keeps a call that times out, or during which the workflow is cancelled, from answering.
	hold(ctx context.Context, ans map[string]any) error
}

type httpTask struct {
	Method string         `json:"method"`
	URL    string         `json:"url"`
	Errors map[string]any `json:"errors"`
	re     *regexp.Regexp
}

type awsTask struct {
	API    string         `json:"api"`
	Errors map[string]any `json:"errors"`
}

type standIn struct {
	run  taker
	http []httpTask
	aws  []awsTask
}

var placeholder = regexp.MustCompile(`\{[^}]*\}`)

func newStandIn(sp spec, run taker) *standIn {
	s := &standIn{run: run, aws: sp.AWS}
	for _, t := range sp.HTTP {
		parts := placeholder.Split(t.URL, -1)
		for i, p := range parts {
			parts[i] = regexp.QuoteMeta(p)
		}
		t.re = regexp.MustCompile("^" + strings.Join(parts, "[^/?]*") + "$")
		s.http = append(s.http, t)
	}
	return s
}

func errorName(kind string) string {
	if kind == "failure" {
		return "Dandori.Test.Failure"
	}
	return kind
}

// held says whether the runner keeps the call from answering: a timeout, or a cancellation.
func held(ans map[string]any) bool {
	return ans["cancel"] == true || timesOut(ans)
}

func timesOut(ans map[string]any) bool {
	_, ok := ans["ok"]
	return !ok && ans["error"] == "timeout"
}

// without is a copy of the map without the key.
func without(m map[string]any, key string) map[string]any {
	out := make(map[string]any, len(m))
	for k, v := range m {
		if k != key {
			out[k] = v
		}
	}
	return out
}

func (s *standIn) Lambda(ctx context.Context, fn string, payload map[string]any) (Outcome, error) {
	callbackID := payload["callback_id"]
	ans, err := s.run.take(ctx, map[string]any{"lambda": fn, "payload": without(payload, "callback_id")})
	if err != nil {
		return Outcome{}, err
	}
	if callbackID != nil {
		if err := s.run.answerLater(ctx, text(callbackID), ans); err != nil {
			return Outcome{}, err
		}
		return Outcome{OK: nil}, nil
	}
	if held(ans) {
		return Outcome{}, s.run.hold(ctx, ans)
	}
	if v, ok := ans["ok"]; ok {
		return Outcome{OK: v}, nil
	}
	return Outcome{Error: errorName(kind(ans)), Message: "scripted"}, nil
}

func (s *standIn) HTTP(ctx context.Context, req HTTPRequest) (HTTPResponse, error) {
	call, err := asMap(req)
	if err != nil {
		return HTTPResponse{}, err
	}
	delete(call, "form")
	delete(call, "typesafe")
	ans, err := s.run.take(ctx, call)
	if err != nil {
		return HTTPResponse{}, err
	}
	if held(ans) {
		return HTTPResponse{}, s.run.hold(ctx, ans)
	}
	if v, ok := ans["ok"]; ok {
		return HTTPResponse{Status: 200, Body: v}, nil
	}
	// the status of the error, from the task that sends this request and declares it
	var sent []httpTask
	for _, t := range s.http {
		if t.Method == req.Method && t.re.MatchString(req.URL) {
			sent = append(sent, t)
		}
	}
	var task *httpTask
	for i := range sent {
		if _, ok := sent[i].Errors[kind(ans)]; ok {
			task = &sent[i]
			break
		}
	}
	if task == nil && len(sent) > 0 {
		task = &sent[0]
	}
	status := 500
	if task != nil {
		if n, ok := task.Errors[kind(ans)].(float64); ok {
			status = int(n)
		}
	}
	return HTTPResponse{Status: status, Body: "scripted"}, nil
}

func (s *standIn) AWS(ctx context.Context, service, action string, input map[string]any) (Outcome, error) {
	api := service + ":" + action
	args := input
	var callbackID any
	if body, ok := input["MessageBody"].(map[string]any); ok && api == "sqs:sendMessage" {
		if id, ok := body["callback_id"]; ok {
			callbackID = id
			args = without(input, "MessageBody")
			args["MessageBody"] = without(body, "callback_id")
		}
	}
	ans, err := s.run.take(ctx, map[string]any{"aws": api, "args": args})
	if err != nil {
		return Outcome{}, err
	}
	if callbackID != nil {
		if err := s.run.answerLater(ctx, text(callbackID), ans); err != nil {
			return Outcome{}, err
		}
		return Outcome{OK: map[string]any{"MessageId": "message-1"}}, nil
	}
	if held(ans) {
		return Outcome{}, s.run.hold(ctx, ans)
	}
	if v, ok := ans["ok"]; ok {
		return Outcome{OK: v}, nil
	}
	name := errorName(kind(ans))
	for _, t := range s.aws {
		if t.API == api {
			if e, ok := t.Errors[kind(ans)].(string); ok {
				name = e
			}
			break
		}
	}
	return Outcome{Error: name, Message: "scripted"}, nil
}

func (s *standIn) Agent(ctx context.Context, call AgentCall) (any, error) {
	c, err := asMap(call)
	if err != nil {
		return nil, err
	}
	ans, err := s.run.take(ctx, c)
	if err != nil {
		return nil, err
	}
	if held(ans) {
		return nil, s.run.hold(ctx, ans)
	}
	if v, ok := ans["ok"]; ok {
		return map[string]any{"answer": v}, nil
	}
	return nil, errors.New("scripted")
}

// kind is the error a scenario's answer names.
func kind(ans map[string]any) string {
	k, _ := ans["error"].(string)
	return k
}

// asMap is a value's JSON as a map: a request as its struct's JSON writes it.
func asMap(v any) (map[string]any, error) {
	b, err := json.Marshal(v)
	if err != nil {
		return nil, fmt.Errorf("writing down %T: %w", v, err)
	}
	var m map[string]any
	if err := json.Unmarshal(b, &m); err != nil {
		return nil, err
	}
	return m, nil
}

// text is a value put into a string: text as it is, nothing for none, anything else as JSON.
func text(v any) string {
	switch t := v.(type) {
	case string:
		return t
	case nil:
		return ""
	}
	b, _ := json.Marshal(v)
	return string(b)
}
