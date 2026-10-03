// The checks of the Go that dandori writes beside the runs of the scenarios: what its default
// Transport sends (--wire, the Go twin of ../wire/check.py), how it runs an agent (--agents, the
// twin of ../agents/check.py), how it reads a rule's answer from the rule's Connect service
// (--connect-read, the twin of ../connect/read.py) and Jev's (--jev, the twin of ../jev/check.py).
// Each takes the same cases as its twins and writes its results in the same shape.
package checks

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"reflect"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/config"
	"github.com/aws/aws-sdk-go-v2/credentials"
	"github.com/aws/aws-sdk-go-v2/service/sns"
	"github.com/aws/aws-sdk-go-v2/service/sqs"

	"temporalgo/harness"
)

const (
	region  = "ap-northeast-1"
	account = "123456789012"
)

func readJSON(file string, v any) error {
	b, err := os.ReadFile(file)
	if err != nil {
		return err
	}
	d := json.NewDecoder(bytes.NewReader(b))
	d.UseNumber()
	if err := d.Decode(v); err != nil {
		return err
	}
	return numbers(v)
}

// numbers turns the json.Numbers a decoder kept into float64s, as the SDK's converter reads them.
func numbers(v any) error {
	p, ok := v.(*any)
	if !ok {
		return nil
	}
	*p = floats(*p)
	return nil
}

func floats(v any) any {
	switch x := v.(type) {
	case json.Number:
		f, _ := x.Float64()
		return f
	case map[string]any:
		for k, y := range x {
			x[k] = floats(y)
		}
	case []any:
		for i, y := range x {
			x[i] = floats(y)
		}
	}
	return v
}

func writeJSON(file string, v any) error {
	var b bytes.Buffer
	e := json.NewEncoder(&b)
	e.SetEscapeHTML(false)
	e.SetIndent("", "  ")
	if err := e.Encode(v); err != nil {
		return err
	}
	return os.WriteFile(file, b.Bytes(), 0o644)
}

func pairs(text string) []any {
	out := []any{}
	q, err := url.ParseQuery(text)
	if err != nil {
		return out
	}
	// ParseQuery keeps no order; the pairs are compared as a set
	for k, vs := range q {
		for _, v := range vs {
			out = append(out, []any{k, v})
		}
	}
	return out
}

// outcome is an Outcome as the TypeScript and the Python write it down.
func outcome(o harness.Outcome, err error) any {
	if err != nil {
		return map[string]any{"thrown": err.Error()}
	}
	if o.Error != "" {
		return map[string]any{"error": o.Error, "message": o.Message}
	}
	return map[string]any{"ok": o.OK}
}

// Wire sends every case through the default Transport of the package to stand-ins on this
// machine: an HTTP task's request to a server here (in place of its scheme and host), Lambda to the
// same server, any other AWS API to moto through it (AWS_ENDPOINT_URL).
func Wire(f harness.Flow, casesFile, outFile, moto string) error {
	var cases []map[string]any
	var raw any
	if err := readJSON(casesFile, &raw); err != nil {
		return err
	}
	for _, c := range raw.([]any) {
		cases = append(cases, c.(map[string]any))
	}
	var mu sync.Mutex
	var now map[string]any
	received := map[string]any{}
	motoURL, err := url.Parse(moto)
	if err != nil {
		return err
	}
	lambdaPath := regexp.MustCompile(`^/2015-03-31/functions/([^/]+)/invocations$`)
	front := httptest(func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		mu.Lock()
		c := now
		mu.Unlock()
		switch {
		case strings.HasPrefix(r.URL.Path, "/http/"):
			// /http/<host><path>: an HTTP task's request
			kind := r.Header.Get("Content-Type")
			text := string(body)
			var parsed any
			switch {
			case text == "":
				parsed = nil
			case strings.Contains(kind, "json"):
				_ = json.Unmarshal(body, &parsed)
			case strings.Contains(kind, "x-www-form-urlencoded"):
				parsed = pairs(text)
			default:
				parsed = text
			}
			headers := map[string]any{}
			for k, v := range r.Header {
				headers[strings.ToLower(k)] = strings.Join(v, ", ")
			}
			rest := strings.TrimPrefix(r.URL.EscapedPath(), "/http/")
			path := ""
			if i := strings.Index(rest, "/"); i >= 0 {
				path = rest[i:]
			}
			rawQuery := ""
			if r.URL.RawQuery != "" {
				rawQuery = "?" + r.URL.RawQuery
			}
			mu.Lock()
			received = map[string]any{"method": r.Method, "path": path, "query": pairs(r.URL.RawQuery), "headers": headers, "body": parsed, "raw": text, "rawQuery": rawQuery}
			mu.Unlock()
			reply := c["reply"].(map[string]any)
			var out []byte
			if s, ok := reply["body"].(string); ok {
				w.Header().Set("Content-Type", "text/plain; charset=utf-8")
				out = []byte(s)
			} else {
				w.Header().Set("Content-Type", "application/json")
				out, _ = json.Marshal(reply["body"])
			}
			w.WriteHeader(int(reply["status"].(float64)))
			_, _ = w.Write(out)
		case lambdaPath.MatchString(r.URL.Path) && r.Method == "POST":
			name, _ := url.PathUnescape(lambdaPath.FindStringSubmatch(r.URL.Path)[1])
			var payload any
			_ = json.Unmarshal(body, &payload)
			var invocation any
			if t := r.Header.Get("X-Amz-Invocation-Type"); t != "" {
				invocation = t
			}
			mu.Lock()
			received = map[string]any{"fn": name, "invocationType": invocation, "payload": payload, "raw": string(body)}
			mu.Unlock()
			reply := c["reply"].(map[string]any)
			var out []byte
			if ok, has := reply["ok"]; has {
				out, _ = json.Marshal(ok)
			} else {
				out, _ = json.Marshal(map[string]any{"errorType": reply["error"], "errorMessage": reply["message"]})
				w.Header().Set("X-Amz-Function-Error", "Unhandled")
			}
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(200)
			_, _ = w.Write(out)
		default:
			// any other AWS API: moto's
			req, _ := http.NewRequest(r.Method, motoURL.String()+r.URL.RequestURI(), bytes.NewReader(body))
			for k, v := range r.Header {
				if !strings.EqualFold(k, "Host") {
					req.Header[k] = v
				}
			}
			res, err := http.DefaultClient.Do(req)
			if err != nil {
				w.WriteHeader(502)
				return
			}
			defer res.Body.Close()
			out, _ := io.ReadAll(res.Body)
			for k, v := range res.Header {
				switch strings.ToLower(k) {
				case "content-length", "transfer-encoding", "connection", "server", "date":
				default:
					w.Header()[k] = v
				}
			}
			w.WriteHeader(res.StatusCode)
			_, _ = w.Write(out)
		}
	})
	defer front.Close()
	// the AWS SDK's clients go to the server here, with credentials of no one
	os.Setenv("AWS_ENDPOINT_URL", front.URL)
	os.Setenv("AWS_REGION", region)
	os.Setenv("AWS_ACCESS_KEY_ID", "wire")
	os.Setenv("AWS_SECRET_ACCESS_KEY", "wire")
	t := f.NewTransport(func(string) map[string]string { return map[string]string{"X-Dandori-Check": "wire"} }, nil)
	ctx := context.Background()
	cfg, err := config.LoadDefaultConfig(ctx, config.WithRegion(region), config.WithCredentialsProvider(credentials.NewStaticCredentialsProvider("wire", "wire", "")), config.WithBaseEndpoint(moto))
	if err != nil {
		return err
	}
	snsc := sns.NewFromConfig(cfg)
	sqsc := sqs.NewFromConfig(cfg)
	var results []any
	for _, c := range cases {
		mu.Lock()
		now = c
		received = nil
		mu.Unlock()
		var returned any
		var messages any
		switch c["kind"] {
		case "http":
			req := c["request"].(map[string]any)
			u, err := url.Parse(req["url"].(string))
			if err != nil {
				return err
			}
			target := front.URL + "/http/" + u.Host + u.EscapedPath()
			if u.RawQuery != "" {
				target += "?" + u.RawQuery
			}
			var h harness.HTTPRequest
			b, _ := json.Marshal(req)
			if err := json.Unmarshal(b, &h); err != nil {
				return err
			}
			h.URL = target
			h.Body = req["body"]
			h.Query = req["query"]
			res, err := t.HTTP(ctx, h)
			if err != nil {
				returned = map[string]any{"thrown": err.Error()}
			} else {
				returned = map[string]any{"status": res.Status, "body": res.Body}
			}
		case "lambda":
			returned = outcome(t.Lambda(ctx, c["fn"].(string), c["payload"].(map[string]any)))
		default:
			// moto as the case wants it: the topics and queues that exist, and a queue that listens
			if _, err := http.Post(moto+"/moto-api/reset", "application/json", nil); err != nil {
				return err
			}
			for _, n := range list(c["topics"]) {
				if _, err := snsc.CreateTopic(ctx, &sns.CreateTopicInput{Name: aws.String(n.(string))}); err != nil {
					return err
				}
			}
			for _, n := range list(c["queues"]) {
				if _, err := sqsc.CreateQueue(ctx, &sqs.CreateQueueInput{QueueName: aws.String(n.(string))}); err != nil {
					return err
				}
			}
			listen, _ := c["listen"].(map[string]any)
			if listen != nil {
				if topic, ok := listen["topic"].(string); ok && topic != "" {
					if _, err := sqsc.CreateQueue(ctx, &sqs.CreateQueueInput{QueueName: aws.String(listen["queue"].(string))}); err != nil {
						return err
					}
					_, err := snsc.Subscribe(ctx, &sns.SubscribeInput{
						TopicArn:   aws.String(fmt.Sprintf("arn:aws:sns:%s:%s:%s", region, account, topic)),
						Protocol:   aws.String("sqs"),
						Endpoint:   aws.String(fmt.Sprintf("arn:aws:sqs:%s:%s:%s", region, account, listen["queue"])),
						Attributes: map[string]string{"RawMessageDelivery": "true"},
					})
					if err != nil {
						return err
					}
				}
			}
			returned = outcome(t.AWS(ctx, c["service"].(string), c["action"].(string), c["input"].(map[string]any)))
			if listen != nil {
				q, err := sqsc.GetQueueUrl(ctx, &sqs.GetQueueUrlInput{QueueName: aws.String(listen["queue"].(string))})
				if err != nil {
					return err
				}
				got, err := sqsc.ReceiveMessage(ctx, &sqs.ReceiveMessageInput{QueueUrl: q.QueueUrl, MaxNumberOfMessages: 10})
				if err != nil {
					return err
				}
				var bodies []any
				for _, m := range got.Messages {
					bodies = append(bodies, aws.ToString(m.Body))
				}
				if bodies == nil {
					bodies = []any{}
				}
				messages = bodies
			}
		}
		mu.Lock()
		results = append(results, map[string]any{"received": received, "returned": returned, "messages": messages})
		mu.Unlock()
	}
	return writeJSON(outFile, results)
}

func list(v any) []any {
	l, _ := v.([]any)
	return l
}

// server is an HTTP server on 127.0.0.1, on a port of its own.
type server struct {
	URL string
	s   *http.Server
}

func (s *server) Close() {
	_ = s.s.Close()
}

func httptest(handle http.HandlerFunc) *server {
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		panic(err)
	}
	s := &http.Server{Handler: handle}
	go func() { _ = s.Serve(l) }()
	return &server{URL: "http://" + l.Addr().String(), s: s}
}

// standIn is a stand-in of an agent's API that answers every request with reply and writes each down.
type standIn struct {
	*server
	mu    sync.Mutex
	asked []map[string]any
}

func newStandIn(status int, reply any) *standIn {
	si := &standIn{}
	si.server = httptest(func(w http.ResponseWriter, r *http.Request) {
		raw, _ := io.ReadAll(r.Body)
		var body any
		if len(raw) > 0 {
			_ = json.Unmarshal(raw, &body)
		}
		si.mu.Lock()
		si.asked = append(si.asked, map[string]any{"method": r.Method, "path": r.URL.Path, "version": nilIfEmpty(r.Header.Get("anthropic-version")), "body": body})
		si.mu.Unlock()
		out, _ := json.Marshal(reply)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(status)
		_, _ = w.Write(out)
	})
	return si
}

func nilIfEmpty(s string) any {
	if s == "" {
		return nil
	}
	return s
}

// to sends every request of the client to the server at base, as it is but for the scheme and the host.
func to(base string) *http.Client {
	u, _ := url.Parse(base)
	return &http.Client{Transport: roundTrip(func(r *http.Request) (*http.Response, error) {
		r = r.Clone(r.Context())
		r.URL.Scheme = u.Scheme
		r.URL.Host = u.Host
		r.Host = u.Host
		return http.DefaultTransport.RoundTrip(r)
	})}
}

type roundTrip func(*http.Request) (*http.Response, error)

func (f roundTrip) RoundTrip(r *http.Request) (*http.Response, error) {
	return f(r)
}

// errorName is the Go type an agent's call failed with, as the test compares it: AgentStopped,
// AgentHTTPError, or the SDK's own error (Error).
func errorName(err error) string {
	t := reflect.TypeOf(err)
	for t.Kind() == reflect.Pointer {
		t = t.Elem()
	}
	for _, name := range []string{"AgentStopped", "AgentHTTPError"} {
		var target error = err
		for target != nil {
			tt := reflect.TypeOf(target)
			for tt.Kind() == reflect.Pointer {
				tt = tt.Elem()
			}
			if tt.Name() == name {
				return name
			}
			target = errors.Unwrap(target)
		}
	}
	return t.Name()
}

func message(model, text, stop string) map[string]any {
	content := []any{map[string]any{"type": "text", "text": text}}
	var details any
	if stop == "refusal" {
		content = []any{}
		details = map[string]any{"type": "refusal", "category": nil, "explanation": nil}
	}
	return map[string]any{"id": "msg_test", "type": "message", "role": "assistant", "model": model, "content": content, "stop_reason": stop, "stop_sequence": nil, "stop_details": details, "usage": map[string]any{"input_tokens": 10, "output_tokens": 10}}
}

func response(model string, content map[string]any) map[string]any {
	return map[string]any{
		"id": "resp_test", "object": "response", "status": "completed", "model": model, "created_at": 1, "output": []any{
			map[string]any{"id": "msg_test", "type": "message", "role": "assistant", "status": "completed", "content": []any{content}},
		},
	}
}

// Agents runs every case's agent call through the default Transport, against a stand-in of the API
// it goes to (the Responses API for OpenAI's agents and for a server of Open Responses, at the
// call's path; the Messages API for Claude's), or, for a case that says "live", the server the
// call names. It writes down the answer or the error, and what the stand-in was asked.
func Agents(f harness.Flow, casesFile, outFile string) error {
	var raw any
	if err := readJSON(casesFile, &raw); err != nil {
		return err
	}
	os.Setenv("OPENAI_API_KEY", "test")
	os.Setenv("ANTHROPIC_API_KEY", "test")
	ctx := context.Background()
	var results []any
	for _, x := range raw.([]any) {
		c := x.(map[string]any)
		var call harness.AgentCall
		b, _ := json.Marshal(c["call"])
		if err := json.Unmarshal(b, &call); err != nil {
			return err
		}
		out := map[string]any{}
		if live, _ := c["live"].(bool); live {
			ctx, cancel := context.WithTimeout(ctx, 5*time.Minute)
			v, err := f.NewTransport(nil, nil).Agent(ctx, call)
			cancel()
			if err != nil {
				out["error"] = errorName(err)
				out["message"] = err.Error()
			} else {
				out["answer"] = v
			}
			results = append(results, out)
			continue
		}
		status := 200
		var reply any
		claude := call.Provider == "claude" && call.URL == ""
		switch {
		case c["status"] != nil:
			status = int(c["status"].(float64))
			if claude {
				reply = map[string]any{"type": "error", "error": map[string]any{"type": "api_error", "message": "scripted"}}
			} else {
				reply = map[string]any{"error": map[string]any{"message": "scripted", "type": "server_error"}}
			}
		case c["refusal"] != nil:
			if claude {
				reply = message(call.Model, c["refusal"].(string), "refusal")
			} else {
				reply = response(call.Model, map[string]any{"type": "refusal", "refusal": c["refusal"]})
			}
		default:
			if claude {
				reply = message(call.Model, c["text"].(string), "end_turn")
			} else {
				reply = response(call.Model, map[string]any{"type": "output_text", "text": c["text"], "annotations": []any{}})
			}
		}
		si := newStandIn(status, reply)
		// everything the Transport sends goes to the stand-in: an OpenAI agent's to OpenAI's path
		// (/v1/responses), a Claude agent's to Claude's (/v1/messages), one on a server of Open
		// Responses to its own
		v, err := f.NewTransport(nil, to(si.URL)).Agent(ctx, call)
		si.Close()
		if err != nil {
			out["error"] = errorName(err)
		} else {
			out["answer"] = v
		}
		si.mu.Lock()
		asked := []any{}
		for _, a := range si.asked {
			switch {
			case claude:
				asked = append(asked, a)
			default:
				asked = append(asked, map[string]any{"method": a["method"], "path": a["path"], "body": a["body"]})
			}
		}
		si.mu.Unlock()
		out["asked"] = asked
		results = append(results, out)
	}
	return writeJSON(outFile, results)
}

// ReadRule reads every case's body as the answer of a rule's Connect service, with the wire of the
// case: read is the package's own reading (ddConnectRule, through a Transport that answers the body).
func ReadRule(read func(wire, body any) (any, error), casesFile, outFile string) error {
	var raw any
	if err := readJSON(casesFile, &raw); err != nil {
		return err
	}
	var results []any
	for _, x := range raw.([]any) {
		c := x.(map[string]any)
		v, err := read(c["wire"], c["body"])
		if err != nil {
			results = append(results, map[string]any{"thrown": err.Error()})
			continue
		}
		results = append(results, v)
	}
	return writeJSON(outFile, results)
}

// Jev sends every case's request to TypeSafe's API through the default Transport, which adds the
// key from TYPESAFE_API_KEY, and reads the answer as the task's spec says (read: the package's ddJev).
func Jev(f harness.Flow, read func(body, spec any) (any, error), casesFile, outFile string) error {
	var raw any
	if err := readJSON(casesFile, &raw); err != nil {
		return err
	}
	t := f.NewTransport(nil, nil)
	var results []any
	for _, x := range raw.([]any) {
		c := x.(map[string]any)
		var req harness.HTTPRequest
		b, _ := json.Marshal(c["request"])
		if err := json.Unmarshal(b, &req); err != nil {
			return err
		}
		req.Body = c["request"].(map[string]any)["body"]
		req.TypeSafe = true
		started := time.Now()
		ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
		res, err := t.HTTP(ctx, req)
		cancel()
		if err != nil {
			return fmt.Errorf("the call of Jev failed: %w", err)
		}
		out := map[string]any{"status": res.Status, "body": res.Body, "ms": time.Since(started).Milliseconds()}
		if res.Status == 200 {
			v, err := read(res.Body, c["spec"])
			if err != nil {
				kind := err.Error()
				var typed interface{ Type() string }
				if errors.As(err, &typed) {
					kind = typed.Type()
				}
				out["error"] = map[string]any{"kind": kind, "message": err.Error()}
			} else {
				out["value"] = v
			}
		}
		results = append(results, out)
	}
	return writeJSON(outFile, results)
}
