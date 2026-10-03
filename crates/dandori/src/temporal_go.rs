//! The workflow as Go, for Temporal's Go SDK. It means what the TypeScript and the Python for
//! Temporal mean (see `temporal.rs` and `temporal_py.rs`), and it puts the same names on the wire —
//! the workflow type, the activities, the update and the signal that bring a callback's answer,
//! the query, the ids of child workflows and callbacks — so that a worker in any of the three
//! languages can serve the others.
//!
//! One package, named after the workflow (or after the `.flow` file when the workflow's name is
//! not ASCII: Go's import paths are), with no `go.mod` of its own:
//! - `doc.go`: what the package is, and the modules it is written against
//! - `types.go`: the records and enums as Go types for the user's code, and a check for each
//!   (`Is<Type>`), which the workflow runs on the JSON values it carries
//! - `values.go`: how the code reads and writes those JSON values
//! - `activities.go`: the tasks; the ones that say `lambda`, `http`, `aws`, `agent` or `jev` are
//!   written there, the others (`OwnTasks`) are the user's; `Activities(own, transport)` gives them
//! - `io.go` (and `io_aws.go`, `io_openai.go`, `io_claude.go` when the flow needs them): how those
//!   tasks reach Lambda, HTTP, the AWS APIs and the agents (a `Transport`)
//! - `rules.go`: the rules as activities, around the Go rulec generates
//! - `runtime.go`: what the workflow code shares — retries, error kinds, keys, callbacks, events
//! - `workflow.go`: the workflow
//! - `client.go`, `worker.go`: starting it, answering its callbacks, asking where it is; its worker
//!
//! Values travel as JSON values (`map[string]any`, `[]any`, `float64`, `string`, `bool`, `nil`),
//! as in the Python: a struct would lose a field that is not there against one that is null, the
//! fields it does not know, and an answer of another shape, which the workflow must see to refuse.

use crate::diag::Diag;
use crate::model::*;
use crate::render::{self, ident};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The versions of the modules the generated code is written against.
pub const TEMPORAL_SDK: &str = "go.temporal.io/sdk v1.49.0";
const AWS_SDK: &str = "github.com/aws/aws-sdk-go-v2 (config v1.33.6, service/lambda v1.110.0, service/sns v1.47.2, service/sqs v1.52.1)";
const OPENAI_CLIENT: &str = "github.com/openai/openai-go/v3 v3.68.0";
const ANTHROPIC_SDK: &str = "github.com/anthropics/anthropic-sdk-go v1.78.0";

/// The services of the AWS APIs whose clients the default Transport knows, by the name Step
/// Functions gives each: the package of the AWS SDK for Go v2 has the same name.
const AWS_SERVICES: &[&str] = &["bedrockruntime", "dynamodb", "ecs", "eventbridge", "kinesis", "lambda", "s3", "secretsmanager", "sesv2", "sfn", "sns", "sqs", "ssm"];

/// What a name of the flow cannot be in the generated Go, with an `_` after it when it is: the
/// keywords, the predeclared names, and the names the generated code uses.
const GO_RESERVED: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range",
    "return", "select", "struct", "switch", "type", "var", "any", "bool", "byte", "comparable", "complex64", "complex128", "error", "float32", "float64", "int", "int8", "int16",
    "int32", "int64", "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr", "true", "false", "iota", "nil", "append", "cap", "clear", "close", "complex",
    "copy", "delete", "imag", "len", "make", "max", "min", "new", "panic", "print", "println", "real", "recover", "ctx", "input", "resume", "workflow", "time", "errors", "err", "out",
    "WorkflowType", "TaskQueue", "Events", "Workflow",
];

/// The exported names every package has, which a record's, an enum's or a method's name gives way to.
const GO_EXPORTED: &[&str] = &[
    "WorkflowType", "TaskQueue", "Events", "Workflow", "RegisterWorkflow", "OwnTasks", "OwnTasksBy", "Activities", "RegisterActivities", "BuildID", "WorkerOptions", "NewWorker",
    "History", "ReplayFailure", "Replay", "Start", "CallbackAnswer", "Answer", "Send", "Where", "Status", "Histories", "Outcome", "HTTPRequest", "HTTPResponse", "AgentCall",
    "Transport", "TransportOptions", "NewTransport", "DefaultTransport", "AgentHTTPError", "AgentStopped", "JevURL", "ClaudeMaxTokens", "WorkflowInput", "WorkflowOutput",
    "IsWorkflowInput", "Decode", "TIMESTAMP", "Service",
];

/// A local name of the flow (a variable) as a Go identifier.
pub fn go_name(name: &str) -> String {
    let s = ident(name);
    if GO_RESERVED.contains(&s.as_str()) || s.starts_with("dd") {
        format!("{s}_")
    } else {
        s
    }
}

/// A name as Go exports it: its parts, each with its first letter in upper case (`hold_amount.Room`
/// is `HoldAmountRoom`), and an `X` before a name whose first letter has no upper case (Japanese).
pub fn exported(name: &str) -> String {
    let mut out = String::new();
    for part in name.split(|c: char| !c.is_alphanumeric()) {
        let mut cs = part.chars();
        if let Some(first) = cs.next() {
            out.extend(first.to_uppercase());
            out.push_str(cs.as_str());
        }
    }
    match out.chars().next() {
        Some(c) if c.is_uppercase() => out,
        _ => format!("X{out}"),
    }
}

/// The package (and the directory) the build writes: the workflow's name when it is an ASCII
/// identifier, else the `.flow` file's name when that is, else `workflow`. Go wants an import path
/// in ASCII, so a package named `宿泊` could not be imported.
pub fn package(m: &Model) -> String {
    fn ascii_ident(s: &str) -> Option<String> {
        let s: String = s.chars().map(|c| if c == '.' || c == '-' { '_' } else { c }).collect::<String>().to_lowercase();
        let ok = !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
        (ok && !GO_RESERVED.contains(&s.as_str()) && s != "main").then_some(s)
    }
    let stem = m.source_file.strip_suffix(".flow").unwrap_or(&m.source_file);
    ascii_ident(&m.name).or_else(|| ascii_ident(stem)).unwrap_or_else(|| "workflow".into())
}

/// A Go string literal (JSON's is one).
fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// A JSON value written into the code, read once when the package loads.
fn json_value(v: &Value) -> String {
    format!("ddJSON({})", q(&v.to_string()))
}

/// The function of the workflow's code that calls a task's activity, which the test runner finds
/// by this name.
pub fn task_function(task: &str) -> String {
    format!("ddTask_{}", ident(task))
}

/// The Go names of what the package exports for the flow: the records' and enums' types, and the
/// methods of `OwnTasks`, each made unique.
struct Names {
    types: BTreeMap<String, String>,
    methods: BTreeMap<String, String>,
}

impl Names {
    fn new(m: &Model) -> Names {
        let (enums, recs) = crate::temporal::used_types(m);
        let mut taken: BTreeSet<String> = GO_EXPORTED.iter().map(|s| s.to_string()).collect();
        let mut types = BTreeMap::new();
        let unique = |base: String, taken: &mut BTreeSet<String>| {
            let mut n = base;
            while taken.contains(&n) {
                n.push('_');
            }
            taken.insert(n.clone());
            n
        };
        for e in &enums {
            let name = &m.enums[*e].name;
            let n = unique(exported(name), &mut taken);
            taken.insert(format!("{n}Values"));
            types.insert(name.clone(), n);
        }
        for r in &recs {
            let name = &m.records[*r].name;
            let n = unique(exported(name), &mut taken);
            taken.insert(format!("Is{n}"));
            types.insert(name.clone(), n);
        }
        let mut methods = BTreeMap::new();
        let mut own_taken = BTreeSet::new();
        for t in &m.tasks {
            if matches!(t.via(Platform::Temporal), Some(Via::Own)) {
                let n = unique(exported(&t.name), &mut own_taken);
                methods.insert(t.name.clone(), n);
            }
        }
        Names { types, methods }
    }

    fn ty(&self, name: &str) -> String {
        self.types.get(name).cloned().unwrap_or_else(|| exported(name))
    }
}

/// The Go type of `t`, for the structs of types.go.
fn go_type(m: &Model, n: &Names, t: &Ty) -> String {
    match t {
        Ty::Int | Ty::Num(_) => "int64".into(),
        Ty::Str | Ty::Timestamp => "string".into(),
        Ty::Bool => "bool".into(),
        Ty::Enum(e) => n.ty(&m.enums[*e].name),
        Ty::Record(r) => n.ty(&m.records[*r].name),
        Ty::List(t) => format!("[]{}", go_type(m, n, t)),
        Ty::Opt(inner) => match &**inner {
            Ty::List(_) | Ty::Json | Ty::Opt(_) => go_type(m, n, inner),
            other => format!("*{}", go_type(m, n, other)),
        },
        Ty::Json => "any".into(),
    }
}

/// A check that `x` is a well-formed JSON value of `t`, as a Go expression.
fn go_check(m: &Model, n: &Names, x: &str, t: &Ty, rg: Option<Range>, depth: usize) -> String {
    match t {
        Ty::Int | Ty::Num(_) => match rg {
            Some(r) => format!("(ddIsInt({x}) && {})", r.tests(|op, k| format!("ddNum({x}) {op} {k}")).join(" && ")),
            None => format!("ddIsInt({x})"),
        },
        Ty::Str => format!("ddIsStr({x})"),
        Ty::Timestamp => format!("ddIsTimestamp({x})"),
        Ty::Bool => format!("ddIsBool({x})"),
        Ty::Enum(e) => format!("ddIsOneOf({x}, {}Values...)", n.ty(&m.enums[*e].name)),
        Ty::Record(r) => format!("Is{}({x})", n.ty(&m.records[*r].name)),
        Ty::List(inner) => {
            let v = format!("dd_v{depth}");
            format!("ddEvery({x}, func({v} any) bool {{ return {} }})", go_check(m, n, &v, inner, rg, depth + 1))
        }
        Ty::Opt(inner) => format!("({x} == nil || {})", go_check(m, n, x, inner, rg, depth)),
        Ty::Json => "true".into(),
    }
}

/// The struct's fields, each by an exported name with the JSON name in its tag, in the columns
/// gofmt puts them in.
fn struct_fields(m: &Model, n: &Names, fields: &[(String, Ty)], optional_json: bool) -> String {
    let mut rows = Vec::new();
    let mut taken = BTreeSet::new();
    for (f, ft) in fields {
        let mut name = exported(f);
        while taken.contains(&name) {
            name.push('_');
        }
        taken.insert(name.clone());
        let omit = if matches!(ft, Ty::Opt(_)) || (optional_json && *ft == Ty::Json) { ",omitempty" } else { "" };
        rows.push((name, go_type(m, n, ft), format!("`json:\"{f}{omit}\"`")));
    }
    let width = |s: &String| s.chars().count();
    let names = rows.iter().map(|r| width(&r.0)).max().unwrap_or(0);
    let types = rows.iter().map(|r| width(&r.1)).max().unwrap_or(0);
    let mut out = String::new();
    for (name, ty, tag) in rows {
        out.push_str(&format!("\t{name}{} {ty}{} {tag}\n", " ".repeat(names - width(&name)), " ".repeat(types - width(&ty))));
    }
    out
}

/// The code with the keys and values of a composite literal that are written one to a line in
/// columns, as gofmt puts them: a run of such lines, at one indentation, is a section whose
/// values line up one space after its longest key, but a key that is much longer or shorter than
/// the ones before it (gofmt's ratio of 2.5 to their geometric mean, for keys of more than 40
/// bytes) starts a section of its own.
fn gofmt_columns(text: &str) -> String {
    /// A line that is one key and its value, whole: the indentation, the key, and the value.
    fn entry(line: &str) -> Option<(&str, &str, &str)> {
        let body = line.trim_start_matches('\t');
        let indent = &line[..line.len() - body.len()];
        if indent.is_empty() || !body.ends_with(',') {
            return None;
        }
        // the key: a string literal or an identifier, then a colon
        let key_end = if let Some(rest) = body.strip_prefix('"') {
            let mut escaped = false;
            let mut end = None;
            for (i, c) in rest.char_indices() {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    end = Some(i + 2);
                    break;
                }
            }
            end?
        } else {
            let n = body.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(body.len());
            if n == 0 || body.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return None;
            }
            n
        };
        let value = body[key_end..].strip_prefix(": ")?.trim_start_matches(' ');
        // the value is whole on the line: its brackets close, outside its strings
        let mut depth = 0i32;
        let mut in_string = false;
        let mut in_raw = false;
        let mut escaped = false;
        for c in value.chars() {
            if in_raw {
                in_raw = c != '`';
            } else if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
            } else {
                match c {
                    '"' => in_string = true,
                    '`' => in_raw = true,
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    _ => {}
                }
                if depth < 0 {
                    return None;
                }
            }
        }
        (depth == 0 && !in_string && !in_raw).then_some((indent, &body[..key_end], value))
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let Some((indent, _, _)) = entry(lines[i]) else {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        };
        // the run of entries at this indentation, cut into sections
        let mut j = i;
        let mut sections: Vec<Vec<usize>> = vec![];
        let (mut lnsum, mut count, mut prev) = (0.0f64, 0usize, 0usize);
        while j < lines.len() {
            let Some((ind, key, _)) = entry(lines[j]) else { break };
            if ind != indent {
                break;
            }
            let size = key.len();
            let together = prev > 0 && (count == 0 || (prev <= 40 && size <= 40) || {
                let ratio = size as f64 / (lnsum / count as f64).exp();
                !(2.5 * ratio <= 1.0 || 2.5 <= ratio)
            });
            if together {
                sections.last_mut().unwrap().push(j);
            } else {
                sections.push(vec![j]);
            }
            lnsum += (size as f64).ln();
            count += 1;
            prev = size;
            j += 1;
        }
        for section in sections {
            let width = section.iter().map(|k| entry(lines[*k]).unwrap().1.chars().count()).max().unwrap_or(0);
            for k in section {
                let (ind, key, value) = entry(lines[k]).unwrap();
                out.push(format!("{ind}{key}:{} {value}", " ".repeat(width - key.chars().count())));
            }
        }
        i = j;
    }
    out.join("\n")
}

/// A check function for a JSON object with these fields: a field that is not there reads as nil,
/// which is a value of a `T?` and of a `json`.
fn check_function(m: &Model, n: &Names, name: &str, fields: &[(String, Ty)], ranges: &BTreeMap<String, Range>) -> String {
    let conds: Vec<String> = fields.iter().filter(|(_, ft)| *ft != Ty::Json).map(|(f, ft)| go_check(m, n, &format!("m[{}]", q(f)), ft, ranges.get(f).copied(), 0)).collect();
    if conds.is_empty() {
        format!("func {name}(v any) bool {{\n\t_, ok := v.(map[string]any)\n\treturn ok\n}}\n")
    } else {
        format!("func {name}(v any) bool {{\n\tm, ok := v.(map[string]any)\n\treturn ok &&\n\t\t{}\n}}\n", conds.join(" &&\n\t\t"))
    }
}

fn types_file(m: &Model, n: &Names, pkg: &str, header: &str) -> String {
    let (enums, recs) = crate::temporal::used_types(m);
    let mut t = header.to_string();
    t.push_str(&format!(
        "// The records and enums of {} v{}, as Go types for the code you write, and a check for each. The\n// workflow carries JSON values and checks them with the Is functions; Decode reads one into these types.\n\npackage {pkg}\n\nimport \"regexp\"\n\n",
        m.name, m.version
    ));
    t.push_str(&format!("// TIMESTAMP is what a timestamp looks like: RFC 3339, in UTC.\nvar TIMESTAMP = regexp.MustCompile(`{}`)\n", render::TIMESTAMP_RE.replace("\\\\", "\\")));
    for e in &enums {
        let en = &m.enums[*e];
        let tn = n.ty(&en.name);
        let vals: Vec<String> = en.values.iter().map(|v| q(v)).collect();
        t.push_str(&format!("\n// {tn} is the enum {}: one of {tn}Values.\ntype {tn} string\n\n// {tn}Values are the values of {tn}.\nvar {tn}Values = []string{{{}}}\n", en.name, vals.join(", ")));
    }
    for r in &recs {
        let rd = &m.records[*r];
        let tn = n.ty(&rd.name);
        t.push_str(&format!("\n// {tn} is the record {}.\ntype {tn} struct {{\n{}}}\n", rd.name, struct_fields(m, n, &rd.fields, true)));
        t.push_str(&format!("\n// Is{tn} checks that v is a {tn} as JSON carries it.\n{}", check_function(m, n, &format!("Is{tn}"), &rd.fields, &rd.ranges)));
    }
    // an input that may be absent (`T?`, `json`) may be left out of the execution's input
    t.push_str(&format!("\n// WorkflowInput is the input of a run.\ntype WorkflowInput struct {{\n{}}}\n", struct_fields(m, n, &m.inputs, true)));
    t.push_str(&format!("\n// IsWorkflowInput checks that v is a WorkflowInput as JSON carries it.\n{}", check_function(m, n, "IsWorkflowInput", &m.inputs, &m.input_ranges)));
    t.push_str(&format!("\n// WorkflowOutput is what a run ends with.\ntype WorkflowOutput struct {{\n{}}}\n", struct_fields(m, n, &m.outputs, false)));
    t
}

/// values.go: how the code reads and writes JSON values.
const VALUES: &str = r##"// Values as the workflow and its tasks carry them: JSON values (map[string]any, []any, float64,
// string, bool, nil), as the Temporal SDK's JSON converter reads them. A number the workflow
// writes itself may be an int, and a local activity gets its arguments as the workflow has them,
// without JSON between: what reads a number here takes either.

package {{PKG}}

import (
	"bytes"
	"encoding/json"
	"math"
)

// Decode reads a JSON value (the arguments of one of your tasks, a record in them) into a Go
// value, such as one of the types of types.go.
func Decode(v any, out any) error {
	b, err := json.Marshal(v)
	if err != nil {
		return err
	}
	return json.Unmarshal(b, out)
}

// ddGet is the value at a path of keys in v, or nil where a key is not there.
func ddGet(v any, keys ...string) any {
	for _, k := range keys {
		m, ok := v.(map[string]any)
		if !ok {
			return nil
		}
		v = m[k]
	}
	return v
}

// ddJSONText is a value as JSON text, as Step Functions and the TypeScript code write it: no
// spaces, no escapes for <, > and &, and text as it is.
func ddJSONText(v any) string {
	var b bytes.Buffer
	e := json.NewEncoder(&b)
	e.SetEscapeHTML(false)
	if err := e.Encode(v); err != nil {
		return "null"
	}
	return string(bytes.TrimRight(b.Bytes(), "\n"))
}

// ddText is a value put into a string or a URL: text as it is, nothing for none, anything else as JSON.
func ddText(v any) string {
	switch x := v.(type) {
	case string:
		return x
	case nil:
		return ""
	}
	return ddJSONText(v)
}

// ddJSON is a value from JSON text that dandori wrote into the code.
func ddJSON(s string) any {
	var v any
	if err := json.Unmarshal([]byte(s), &v); err != nil {
		panic(err)
	}
	return v
}

// ddWire is a value as it leaves the workflow or an activity: none as JSON's null, which every
// SDK reads as none. The Go SDK writes nil as a binary/null payload, which the TypeScript SDK
// reads as undefined, and an undefined field is left out of what it sends on.
func ddWire(v any) any {
	if v == nil {
		return json.RawMessage("null")
	}
	return v
}

func ddObject(v any) map[string]any {
	m, _ := v.(map[string]any)
	return m
}

func ddList(v any) []any {
	l, _ := v.([]any)
	return l
}

func ddStr(v any) string {
	s, _ := v.(string)
	return s
}

// ddNumber is v as a float64, and whether it is a number (a bool is not one, as in JSON).
func ddNumber(v any) (float64, bool) {
	switch x := v.(type) {
	case float64:
		return x, true
	case float32:
		return float64(x), true
	case int:
		return float64(x), true
	case int64:
		return float64(x), true
	case int32:
		return float64(x), true
	case json.Number:
		f, err := x.Float64()
		return f, err == nil
	}
	return 0, false
}

func ddNum(v any) float64 {
	f, _ := ddNumber(v)
	return f
}

func ddInt(v any) int {
	return int(ddNum(v))
}

func ddInt64(v any) int64 {
	return int64(ddNum(v))
}

// ddIsInt is whether v is an integer, as JSON carries one: a number with nothing after the point.
func ddIsInt(v any) bool {
	f, ok := ddNumber(v)
	return ok && !math.IsInf(f, 0) && f == math.Trunc(f)
}

func ddIsStr(v any) bool {
	_, ok := v.(string)
	return ok
}

func ddIsBool(v any) bool {
	_, ok := v.(bool)
	return ok
}

func ddIsTimestamp(v any) bool {
	s, ok := v.(string)
	return ok && TIMESTAMP.MatchString(s)
}

// ddIsOneOf is whether v is a string that is one of values.
func ddIsOneOf(v any, values ...string) bool {
	s, ok := v.(string)
	if !ok {
		return false
	}
	for _, x := range values {
		if x == s {
			return true
		}
	}
	return false
}

// ddEvery is whether v is a list whose every item passes check.
func ddEvery(v any, check func(any) bool) bool {
	l, ok := v.([]any)
	if !ok {
		return false
	}
	for _, x := range l {
		if !check(x) {
			return false
		}
	}
	return true
}

func ddHas(names []string, name string) bool {
	for _, n := range names {
		if n == name {
			return true
		}
	}
	return false
}

// ddWith is the object v with key set to x, v itself left as it is.
func ddWith(v any, key string, x any) map[string]any {
	out := map[string]any{}
	for k, y := range ddObject(v) {
		out[k] = y
	}
	out[key] = x
	return out
}

// ddFill is a message as protobuf reads it, with the zero values filled in that its JSON leaves
// out: zeros["f"] by key, and the same for the messages under zeros["m"] and the lists of
// messages under zeros["l"].
func ddFill(v any, zeros any) any {
	m, ok := v.(map[string]any)
	if !ok {
		return v
	}
	z := ddObject(zeros)
	out := make(map[string]any, len(m))
	for k, x := range m {
		out[k] = x
	}
	for k, x := range ddObject(z["f"]) {
		if out[k] == nil {
			out[k] = x
		}
	}
	for k, d := range ddObject(z["m"]) {
		if _, ok := out[k].(map[string]any); ok {
			out[k] = ddFill(out[k], d)
		}
	}
	for k, d := range ddObject(z["l"]) {
		if l, ok := out[k].([]any); ok {
			filled := make([]any, len(l))
			for i, x := range l {
				filled[i] = ddFill(x, d)
			}
			out[k] = filled
		}
	}
	return out
}
"##;

/// runtime.go: what the workflow code shares.
const RUNTIME: &str = r##"// What the generated workflow code shares. It runs inside the workflow, so it uses nothing but the
// workflow's own context (go.temporal.io/sdk/workflow): no clock, no goroutine or channel of Go's
// own, and nothing that depends on the order of a map.

package {{PKG}}

import (
	"errors"
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"
	"time"

	"go.temporal.io/sdk/converter"
	"go.temporal.io/sdk/temporal"
	"go.temporal.io/sdk/workflow"
)

// ddTaskError is a task that failed after its retries, with the kind of error the .flow names it by.
type ddTaskError struct {
	Kind    string
	Message string
}

func (e *ddTaskError) Error() string { return e.Kind + ": " + e.Message }

// ddCallbackTimeout is a callback or an event that got no answer in time.
type ddCallbackTimeout struct{}

func (*ddCallbackTimeout) Error() string { return "no answer in time" }

// ddCallbackError is a callback's answer or an event that came with an error.
type ddCallbackError struct {
	Kind    string
	Message string
}

func (e *ddCallbackError) Error() string { return e.Message }

// ddRetrier is a retrier, as the state machine's Retry has it: the kinds it takes ("*" is every kind).
type ddRetrier struct {
	On      []string
	Max     int
	Every   float64
	Backoff float64
}

// ddNoRetry: Temporal tries an activity once; the workflow retries, as the retry of each task says.
var ddNoRetry = &temporal.RetryPolicy{MaximumAttempts: 1}

// ddCause is the error an activity's or a child workflow's error carries, or err itself. A local
// activity's error comes as an activity's does.
func ddCause(err error) error {
	switch err.(type) {
	case *temporal.ActivityError, *temporal.ChildWorkflowExecutionError:
		if c := errors.Unwrap(err); c != nil {
			return c
		}
	}
	return err
}

// ddKindOf is the kind of an error from an activity, a child workflow, a callback or an event: a
// declared error, "timeout", or "failure".
func ddKindOf(err error, declared []string) string {
	var timeout *ddCallbackTimeout
	if errors.As(err, &timeout) {
		return "timeout"
	}
	var came *ddCallbackError
	if errors.As(err, &came) {
		if ddHas(declared, came.Kind) {
			return came.Kind
		}
		return "failure"
	}
	switch c := ddCause(err).(type) {
	case *temporal.TimeoutError:
		return "timeout"
	case *temporal.ApplicationError:
		if c.Type() != "" && ddHas(declared, c.Type()) {
			return c.Type()
		}
	}
	return "failure"
}

// ddMessageOf is the error's message as the other side sent it.
func ddMessageOf(err error) string {
	var came *ddCallbackError
	if errors.As(err, &came) {
		return came.Message
	}
	switch c := ddCause(err).(type) {
	case *temporal.ApplicationError:
		return c.Message()
	case *temporal.TimeoutError:
		return c.Message()
	default:
		return c.Error()
	}
}

// ddCancelled is whether the error is the workflow's cancellation, as a wait or a call in the workflow meets it.
func ddCancelled(err error) bool {
	return err != nil && temporal.IsCanceledError(err)
}

// ddAttempt calls once, and again as the retriers say, waiting between as Step Functions would. A
// cancellation is no task's error: it goes on as it is.
func ddAttempt(ctx workflow.Context, call func(ctx workflow.Context) (any, error), retriers []ddRetrier, declared []string) (any, error) {
	counts := make([]int, len(retriers))
	for {
		r, err := call(ctx)
		if err == nil {
			return r, nil
		}
		if ddCancelled(err) {
			return nil, err
		}
		kind := ddKindOf(err, declared)
		again := false
		for i, rt := range retriers {
			if ddHas(rt.On, kind) || ddHas(rt.On, "*") {
				if counts[i] < rt.Max {
					if err := workflow.Sleep(ctx, ddSeconds(rt.Every*math.Pow(rt.Backoff, float64(counts[i])))); err != nil {
						return nil, err
					}
					counts[i]++
					again = true
				}
				break
			}
		}
		if !again {
			return nil, &ddTaskError{Kind: kind, Message: ddMessageOf(err)}
		}
	}
}

// ddKind is the kind of the task's error that reached a call, or "" for any other error (a cancellation).
func ddKind(err error) string {
	var te *ddTaskError
	if errors.As(err, &te) {
		return te.Kind
	}
	return ""
}

// ddSeconds is a duration of the workflow's timers: every wait of the workflow goes through here.
func ddSeconds(n float64) time.Duration {
	return time.Duration(n * float64(time.Second))
}

// ddWaitUntil waits until a moment given as an RFC 3339 string in UTC, by the workflow's clock.
func ddWaitUntil(ctx workflow.Context, at any) error {
	t, err := time.Parse(time.RFC3339Nano, ddStr(at))
	if err != nil {
		return nil
	}
	if left := t.Sub(workflow.Now(ctx)).Seconds(); left > 0 {
		return workflow.Sleep(ctx, ddSeconds(left))
	}
	return nil
}

func ddPlaces(site int, rounds []int) []string {
	out := []string{strconv.Itoa(site)}
	for _, r := range rounds {
		out = append(out, strconv.Itoa(r))
	}
	return out
}

// ddKey is the idempotency key of a call: the workflow, the call's place, and the round of each loop around it.
func ddKey(ctx workflow.Context, site int, rounds []int) string {
	return workflow.GetInfo(ctx).WorkflowExecution.ID + "/" + strings.Join(ddPlaces(site, rounds), "/")
}

// ddChildID is the workflow id of a child workflow: this workflow's, the call's place, the rounds, and the try.
func ddChildID(ctx workflow.Context, site int, rounds []int, n int) string {
	return ddKey(ctx, site, rounds) + "/" + strconv.Itoa(n)
}

// ddRun is what a run keeps beside its variables: where it is, its cases, and the callbacks and
// events it waits for. A worker runs many runs at once, so none of this is a package variable.
type ddRun struct {
	at      any
	names   []string
	cases   func() map[string]any
	awaited map[string]bool
	events  map[string]map[string]any
	answers map[string]map[string]any
	waiting map[string]bool
}

// ddNew is a run's own state, with the query dandori.status, which answers from the start, for a
// run that fails at its input too: no case has started.
func ddNew(ctx workflow.Context, cases ...string) *ddRun {
	r := &ddRun{names: cases, awaited: map[string]bool{}, events: map[string]map[string]any{}, answers: map[string]map[string]any{}, waiting: map[string]bool{}}
	none := map[string]any{}
	for _, c := range cases {
		none[c] = nil
	}
	r.cases = func() map[string]any { return none }
	_ = workflow.SetQueryHandler(ctx, "dandori.status", func() (map[string]any, error) {
		events := []string{}
		for e := range r.awaited {
			events = append(events, e)
		}
		sort.Strings(events)
		return map[string]any{"at": r.at, "cases": r.cases(), "events": events}, nil
	})
	return r
}

// report makes the query read the cases' states with read.
func (r *ddRun) report(read func() map[string]any) {
	r.cases = read
}

// listenForAnswers takes the answers of callbacks as they come: by the update dandori.answer, which
// tells the one who answers whether the workflow took it (it refuses an answer for a callback it
// does not wait for, an id it never handed on or one it gave up on, and a second answer), and by
// the signal dandori.callback, whose answers for callbacks it does not wait for it drops.
func (r *ddRun) listenForAnswers(ctx workflow.Context) {
	signals := workflow.GetSignalChannel(ctx, "dandori.callback")
	workflow.Go(ctx, func(ctx workflow.Context) {
		for {
			var a map[string]any
			if !signals.Receive(ctx, &a) {
				return
			}
			if id, _ := a["callback_id"].(string); r.waiting[id] {
				r.answers[id] = a
			}
		}
	})
	_ = workflow.SetUpdateHandlerWithOptions(ctx, "dandori.answer", func(ctx workflow.Context, a map[string]any) error {
		id, _ := a["callback_id"].(string)
		r.answers[id] = a
		return nil
	}, workflow.UpdateHandlerOptions{Validator: func(ctx workflow.Context, a map[string]any) error {
		id, _ := a["callback_id"].(string)
		if !r.waiting[id] {
			return fmt.Errorf("no callback waits for an answer with the id %v", a["callback_id"])
		}
		if _, ok := r.answers[id]; ok {
			return fmt.Errorf("the callback %s has its answer already", id)
		}
		return nil
	}})
}

// callbackID is the id a callback task hands on: which workflow to answer, and which call it answers.
func (r *ddRun) callbackID(ctx workflow.Context, site int, rounds []int, n int) string {
	id := ddJSONText([]any{workflow.GetInfo(ctx).WorkflowExecution.ID, strings.Join(append(ddPlaces(site, rounds), strconv.Itoa(n)), "/")})
	r.waiting[id] = true
	return id
}

// awaitCallback waits for the answer of the callback with this id, at most limit seconds; the
// workflow then waits for it no more.
func (r *ddRun) awaitCallback(ctx workflow.Context, id string, limit float64) (any, error) {
	ok, err := workflow.AwaitWithTimeout(ctx, ddSeconds(limit), func() bool { _, ok := r.answers[id]; return ok })
	delete(r.waiting, id)
	if err != nil {
		return nil, err
	}
	if !ok {
		return nil, &ddCallbackTimeout{}
	}
	a := r.answers[id]
	delete(r.answers, id)
	return ddCame(a)
}

// ddCame is the value of a callback's answer or an event, or the error it names.
func ddCame(a map[string]any) (any, error) {
	if kind, ok := a["error"].(string); ok {
		message, _ := a["message"].(string)
		return nil, &ddCallbackError{Kind: kind, Message: message}
	}
	return a["ok"], nil
}

// listenForEvents takes the events the workflow waits for as they come, by the update
// dandori.event (client.go: Send). It refuses an event it does not wait for now, and a second one:
// the one who sends it learns so, and can send it again when it waits.
func (r *ddRun) listenForEvents(ctx workflow.Context) {
	_ = workflow.SetUpdateHandlerWithOptions(ctx, "dandori.event", func(ctx workflow.Context, a map[string]any) error {
		name, _ := a["event"].(string)
		r.events[name] = a
		return nil
	}, workflow.UpdateHandlerOptions{Validator: func(ctx workflow.Context, a map[string]any) error {
		name, _ := a["event"].(string)
		if !r.awaited[name] {
			return fmt.Errorf("the workflow does not wait for the event %v now", a["event"])
		}
		if _, ok := r.events[name]; ok {
			return fmt.Errorf("the event %s has come already", name)
		}
		return nil
	}})
}

// awaitEvent waits for the event with this name, at most limit seconds; the workflow then waits for it no more.
func (r *ddRun) awaitEvent(ctx workflow.Context, name string, limit float64) (any, error) {
	r.awaited[name] = true
	ok, err := workflow.AwaitWithTimeout(ctx, ddSeconds(limit), func() bool { _, ok := r.events[name]; return ok })
	delete(r.awaited, name)
	if err != nil {
		return nil, err
	}
	if !ok {
		return nil, &ddCallbackTimeout{}
	}
	a := r.events[name]
	delete(r.events, name)
	return ddCame(a)
}

// ddCases is the search attribute that lists the cases' states, as "<case>=<state>". The workflow
// keeps it up to date only when it was started so (client.go: Start(…, true), which puts
// dandori.cases in the memo), since it must be registered on the namespace first.
var ddCases = temporal.NewSearchAttributeKeyKeywordList("DandoriCases")

// shown: a case moved; show the cases' states in the search attribute, when the workflow was started so.
func (r *ddRun) shown(ctx workflow.Context) {
	memo := workflow.GetInfo(ctx).Memo
	if memo == nil {
		return
	}
	p, ok := memo.GetFields()["dandori.cases"]
	if !ok {
		return
	}
	var on bool
	if err := converter.GetDefaultDataConverter().FromPayload(p, &on); err != nil || !on {
		return
	}
	now := []string{}
	cases := r.cases()
	for _, n := range r.names {
		if s := cases[n]; s != nil {
			now = append(now, n+"="+ddText(s))
		}
	}
	_ = workflow.UpsertTypedSearchAttributes(ctx, ddCases.ValueSet(now))
}

// ddContinueAt is how many events a run's history has before a loop at the top of the flow goes on
// in a new run (Continue-As-New), at the start of a round. The server suggests it sooner when the
// history grows large.
const ddContinueAt = 10000

// ddHistoryIsLong is whether a loop at the top of the flow should go on in a new run: the history is long, or the server suggests it.
func ddHistoryIsLong(ctx workflow.Context) bool {
	info := workflow.GetInfo(ctx)
	return info.GetContinueAsNewSuggested() || info.GetCurrentHistoryLength() >= ddContinueAt
}

// ddCarried is what a run that goes on at the at-th loop at the top of the flow was handed (the
// "round", the list as "items", or what the loop yielded as "out"), or else what fresh makes.
func ddCarried(resume any, at int, what string, fresh func() any) any {
	if resume != nil && ddInt(ddGet(resume, "at")) == at {
		return ddGet(resume, what)
	}
	return fresh()
}

// ddAtATime is how many rounds of `for … in parallel` run at a time: all, when the .flow does not say (0).
func ddAtATime(k int) int {
	return k
}

// ddRounds runs a round for every item, k at a time (0: all at once). Every round runs to its end;
// then what they yield comes back in the list's order, or a cancellation, or else the first
// failure by place in the list, is returned.
func ddRounds(ctx workflow.Context, items []any, k int, round func(ctx workflow.Context, item any, index int) (any, error)) ([]any, error) {
	n := len(items)
	out := make([]any, n)
	failed := make([]error, n)
	lanes := n
	if k > 0 && k < n {
		lanes = k
	}
	next := 0
	wg := workflow.NewWaitGroup(ctx)
	for l := 0; l < lanes; l++ {
		wg.Add(1)
		workflow.Go(ctx, func(ctx workflow.Context) {
			defer wg.Done()
			for {
				i := next
				next++
				if i >= n {
					return
				}
				v, err := round(ctx, items[i], i)
				if err != nil {
					failed[i] = err
				} else {
					out[i] = v
				}
			}
		})
	}
	wg.Wait(ctx)
	for _, err := range failed {
		if ddCancelled(err) {
			return nil, err
		}
	}
	for _, err := range failed {
		if err != nil {
			return nil, err
		}
	}
	return out, nil
}

// ddFail is a deliberate end of the workflow as failed.
func ddFail(kind string, cause any) error {
	return temporal.NewNonRetryableApplicationError(ddText(cause), kind, nil)
}

// ddEnd is how the workflow ends: a task's error that nothing handled ends it with the error's
// kind, and none ends it with JSON's null.
func ddEnd(out any, err error) (any, error) {
	var te *ddTaskError
	if errors.As(err, &te) {
		return nil, ddFail(te.Kind, te.Message)
	}
	if err != nil {
		return nil, err
	}
	return ddWire(out), nil
}
"##;

/// What the default Transport needs, by what the flow calls: Lambda, the AWS APIs (by service and
/// action), OpenAI's agents through OpenAI's client, Claude's.
struct Needs {
    lambda: bool,
    aws: BTreeSet<(String, String)>,
    openai: bool,
    claude: bool,
}

fn needs(m: &Model) -> Needs {
    let mut n = Needs { lambda: false, aws: BTreeSet::new(), openai: false, claude: false };
    for t in &m.tasks {
        if t.is_child(Platform::Temporal) || t.event {
            continue;
        }
        match t.via(Platform::Temporal) {
            Some(Via::Lambda(_)) => n.lambda = true,
            Some(Via::Aws { service, action }) => {
                n.aws.insert((service.to_string(), action.to_string()));
            }
            Some(Via::Agent { provider: Provider::OpenAi, url: None, .. }) => n.openai = true,
            Some(Via::Agent { provider: Provider::Claude, .. }) => n.claude = true,
            _ => {}
        }
    }
    n
}

/// io.go: the Transport and what the tasks dandori writes share; its options only those the flow uses.
fn io_file(n: &Needs, pkg: &str, header: &str) -> String {
    let mut sdks = vec!["net/http for HTTP".to_string()];
    if n.lambda || !n.aws.is_empty() {
        sdks.push("the AWS SDK for Go v2 for Lambda and the AWS APIs".into());
    }
    if n.openai {
        sdks.push("OpenAI's Go client (openai-go, which reads OPENAI_API_KEY) for OpenAI's agents".into());
    }
    if n.claude {
        sdks.push("Anthropic's Go SDK (anthropic-sdk-go, which reads ANTHROPIC_API_KEY) for Claude's".into());
    }
    let mut t = header.to_string();
    t.push_str("// How the tasks that say `lambda`, `http`, `aws`, `agent` or `jev` reach the other side. They go\n");
    t.push_str("// through a Transport, so that the credentials, the clients and a test's stand-in are yours to\n");
    t.push_str(&format!("// set; the default one uses {}. Jev is called over HTTP with TypeSafe's API key (TYPESAFE_API_KEY).\n", sdks.join(", ")));
    t.push_str(&format!("\npackage {pkg}\n\nimport (\n"));
    for i in ["context", "encoding/json", "errors", "fmt", "io", "math", "net/http", "os", "regexp", "sort", "strconv", "strings", "sync", "time"] {
        t.push_str(&format!("\t{}\n", q(i)));
    }
    t.push('\n');
    if n.lambda || !n.aws.is_empty() {
        t.push_str("\t\"github.com/aws/aws-sdk-go-v2/aws\"\n");
    }
    if n.claude {
        t.push_str("\tanthropicoption \"github.com/anthropics/anthropic-sdk-go/option\"\n");
    }
    if n.openai {
        t.push_str("\topenaioption \"github.com/openai/openai-go/v3/option\"\n");
    }
    t.push_str("\t\"go.temporal.io/sdk/activity\"\n\t\"go.temporal.io/sdk/temporal\"\n)\n\n");
    t.push_str("// TransportOptions are what the default Transport is made with.\ntype TransportOptions struct {\n");
    t.push_str("\t// Headers to add to an HTTP request, such as the credentials the other side wants (a server of Open Responses too).\n\tHeaders func(url string) map[string]string\n");
    t.push_str("\t// HTTPClient sends the HTTP requests, the agents' too; http.DefaultClient when it is nil.\n\tHTTPClient *http.Client\n");
    t.push_str("\t// TypeSafeAPIKey is TypeSafe's API key for the Jev tasks, in place of TYPESAFE_API_KEY.\n\tTypeSafeAPIKey string\n");
    if n.lambda || !n.aws.is_empty() {
        t.push_str("\t// AWS is the configuration of the AWS SDK's clients, such as the region; without it, what\n\t// config.LoadDefaultConfig finds.\n\tAWS *aws.Config\n");
    }
    if n.openai {
        t.push_str("\t// OpenAI are options of OpenAI's client for the agents, such as option.WithAPIKey or option.WithBaseURL.\n\t// The client does not retry by itself unless they say so: the workflow retries, as `retry` says.\n\tOpenAI []openaioption.RequestOption\n");
    }
    if n.claude {
        t.push_str("\t// Claude are options of Anthropic's client for the Claude agents, such as option.WithAPIKey or\n\t// option.WithBaseURL. The client does not retry by itself unless they say so: the workflow retries,\n\t// as `retry` says.\n\tClaude []anthropicoption.RequestOption\n");
    }
    t.push_str("}\n");
    t.push_str(&IO.replace("{{CLAUDE_MAX_TOKENS}}", &render::CLAUDE_MAX_TOKENS.to_string()).replace("{{BEAT_SECONDS}}", &crate::temporal::BEAT_SECONDS.to_string()));
    t
}

const IO: &str = r##"
// Outcome is what Lambda or an AWS API answered: the value OK, or, when Error is not "", the error
// by the name the other side gives it.
type Outcome struct {
	OK      any
	Error   string
	Message string
}

// HTTPRequest is an HTTP request as Step Functions' HTTP Task sends it. Form asks for a URL-encoded
// body, and TypeSafe says it is a call of Jev, to which the default Transport adds TypeSafe's API key.
type HTTPRequest struct {
	Method   string            `json:"http"`
	URL      string            `json:"url"`
	Headers  map[string]string `json:"headers,omitempty"`
	Body     any               `json:"body,omitempty"`
	Query    any               `json:"query,omitempty"`
	Form     bool              `json:"form,omitempty"`
	TypeSafe bool              `json:"typesafe,omitempty"`
}

// HTTPResponse is the status of an HTTP answer and its body, parsed when it is JSON.
type HTTPResponse struct {
	Status int
	Body   any
}

// JevURL is where every target sends a Jev task's request: TypeSafe's API.
const JevURL = "https://api.typesafe.ai/v1/systemone"

// AgentCall is a call of an agent: the model is told Instructions, reads Input as JSON text, and
// answers in Schema, the JSON Schema of {"answer": …} in the strict form of OpenAI's Structured
// Outputs, which Claude's take too. Provider is "openai" or "claude". An OpenAI agent's URL is a
// server of Open Responses other than OpenAI's: its base URL, to which /responses is added. Effort
// says how hard the model reasons: the Responses API's reasoning.effort, Claude's output_config.effort.
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

// ClaudeMaxTokens is the most a Claude agent's answer may take, thinking included, as every target asks for.
const ClaudeMaxTokens = {{CLAUDE_MAX_TOKENS}}

// AgentHTTPError is an agent's server that answered with an error status.
type AgentHTTPError struct {
	Status int
	Body   any
}

func (e *AgentHTTPError) Error() string {
	body, ok := e.Body.(string)
	if !ok {
		body = ddJSONText(e.Body)
	}
	return fmt.Sprintf("the agent's server answered %d: %s", e.Status, body)
}

// AgentStopped is an agent that did not end with an answer: it refused, or stopped at the limit.
type AgentStopped struct {
	Reason string
}

func (e *AgentStopped) Error() string {
	return fmt.Sprintf("the model stopped with %s, not with an answer", e.Reason)
}

// Transport is how the tasks dandori writes reach the other side.
type Transport interface {
	// Lambda invokes a Lambda function; an error the function raises comes back by its type.
	Lambda(ctx context.Context, fn string, payload map[string]any) (Outcome, error)
	// HTTP sends an HTTP request.
	HTTP(ctx context.Context, req HTTPRequest) (HTTPResponse, error)
	// AWS calls an AWS API, named as Step Functions names it (sns, publish); an exception comes back by its name.
	AWS(ctx context.Context, service, action string, input map[string]any) (Outcome, error)
	// Agent runs an agent once; the answer is the JSON it gave, as Schema says. A refusal is an error.
	Agent(ctx context.Context, call AgentCall) (any, error)
}

// DefaultTransport is the Transport NewTransport makes.
type DefaultTransport struct {
	o       TransportOptions
	mu      sync.Mutex
	clients map[string]any
}

// NewTransport is the default Transport, made with o.
func NewTransport(o TransportOptions) Transport {
	return &DefaultTransport{o: o, clients: map[string]any{}}
}

// client is what build makes for key, made once.
func (t *DefaultTransport) client(key string, build func() (any, error)) (any, error) {
	t.mu.Lock()
	defer t.mu.Unlock()
	if c, ok := t.clients[key]; ok {
		return c, nil
	}
	c, err := build()
	if err != nil {
		return nil, err
	}
	t.clients[key] = c
	return c, nil
}

// The parts of the default Transport that a build writes only when the flow needs them: io_aws.go,
// io_openai.go and io_claude.go set these when the package loads.
var (
	ddLambdaCall  func(ctx context.Context, t *DefaultTransport, fn string, payload map[string]any) (Outcome, error)
	ddAWSCall     func(ctx context.Context, t *DefaultTransport, service, action string, input map[string]any) (Outcome, error)
	ddOpenAIAgent func(ctx context.Context, t *DefaultTransport, call AgentCall) (any, error)
	ddClaudeAgent func(ctx context.Context, t *DefaultTransport, call AgentCall) (any, error)
)

func (t *DefaultTransport) Lambda(ctx context.Context, fn string, payload map[string]any) (Outcome, error) {
	if ddLambdaCall == nil {
		return Outcome{}, errors.New("the flow calls no Lambda function, so this build has no Lambda client: pass a Transport whose Lambda calls it")
	}
	return ddLambdaCall(ctx, t, fn, payload)
}

func (t *DefaultTransport) AWS(ctx context.Context, service, action string, input map[string]any) (Outcome, error) {
	if ddAWSCall == nil {
		return Outcome{}, fmt.Errorf("the default Transport has no client for %s:%s; pass a Transport whose AWS calls it", service, action)
	}
	return ddAWSCall(ctx, t, service, action, input)
}

func (t *DefaultTransport) Agent(ctx context.Context, call AgentCall) (any, error) {
	if call.URL != "" {
		return t.openResponses(ctx, call)
	}
	if call.Provider == "claude" {
		if ddClaudeAgent == nil {
			return nil, errors.New("the flow has no Claude agent, so this build has no client for Claude: pass a Transport whose Agent calls it")
		}
		return ddClaudeAgent(ctx, t, call)
	}
	if ddOpenAIAgent == nil {
		return nil, errors.New("the flow has no agent of OpenAI's, so this build has no client for OpenAI: pass a Transport whose Agent calls it")
	}
	return ddOpenAIAgent(ctx, t, call)
}

// ddEscape is text as JavaScript's encodeURIComponent writes it into a URL.
func ddEscape(s string) string {
	var b strings.Builder
	for _, c := range []byte(s) {
		if 'a' <= c && c <= 'z' || 'A' <= c && c <= 'Z' || '0' <= c && c <= '9' || strings.IndexByte("-_.!~*'()", c) >= 0 {
			b.WriteByte(c)
		} else {
			fmt.Fprintf(&b, "%%%02X", c)
		}
	}
	return b.String()
}

// ddEncode is a=1&b[c]=2, the way a URL-encoded body nests.
func ddEncode(v any, prefix string) string {
	var keys []string
	values := map[string]any{}
	switch x := v.(type) {
	case map[string]any:
		for k, y := range x {
			keys = append(keys, k)
			values[k] = y
		}
		sort.Strings(keys)
	case []any:
		for i, y := range x {
			k := strconv.Itoa(i)
			keys = append(keys, k)
			values[k] = y
		}
	}
	var parts []string
	for _, k := range keys {
		key := k
		if prefix != "" {
			key = prefix + "[" + k + "]"
		}
		switch y := values[k].(type) {
		case map[string]any, []any:
			if p := ddEncode(y, key); p != "" {
				parts = append(parts, p)
			}
		default:
			parts = append(parts, ddEscape(key)+"="+ddEscape(ddText(y)))
		}
	}
	return strings.Join(parts, "&")
}

func (t *DefaultTransport) HTTP(ctx context.Context, req HTTPRequest) (HTTPResponse, error) {
	u := req.URL
	if req.Query != nil {
		sep := "?"
		if strings.Contains(u, "?") {
			sep = "&"
		}
		u += sep + ddEncode(req.Query, "")
	}
	headers := map[string]string{}
	for k, v := range req.Headers {
		headers[k] = v
	}
	if t.o.Headers != nil {
		for k, v := range t.o.Headers(req.URL) {
			headers[k] = v
		}
	}
	if req.TypeSafe && !ddHasHeader(headers, "Authorization") {
		key := t.o.TypeSafeAPIKey
		if key == "" {
			key = os.Getenv("TYPESAFE_API_KEY")
		}
		if key == "" {
			return HTTPResponse{}, errors.New("no API key for Jev: set TYPESAFE_API_KEY, or give the transport TransportOptions.TypeSafeAPIKey")
		}
		headers["Authorization"] = "Bearer " + key
	}
	var body io.Reader
	if req.Body != nil {
		if req.Form {
			body = strings.NewReader(ddEncode(req.Body, ""))
		} else {
			body = strings.NewReader(ddJSONText(req.Body))
			if !ddHasHeader(headers, "Content-Type") {
				headers["Content-Type"] = "application/json"
			}
		}
	}
	r, err := http.NewRequestWithContext(ctx, req.Method, u, body)
	if err != nil {
		return HTTPResponse{}, err
	}
	for k, v := range headers {
		r.Header.Set(k, v)
	}
	c := t.o.HTTPClient
	if c == nil {
		c = http.DefaultClient
	}
	res, err := c.Do(r)
	if err != nil {
		return HTTPResponse{}, err
	}
	defer res.Body.Close()
	raw, err := io.ReadAll(res.Body)
	if err != nil {
		return HTTPResponse{}, err
	}
	var parsed any = string(raw)
	if strings.Contains(res.Header.Get("Content-Type"), "json") {
		var v any
		if json.Unmarshal(raw, &v) == nil {
			parsed = v
		}
	}
	return HTTPResponse{Status: res.StatusCode, Body: parsed}, nil
}

func ddHasHeader(headers map[string]string, name string) bool {
	for k := range headers {
		if strings.EqualFold(k, name) {
			return true
		}
	}
	return false
}

// openResponses runs an agent on a server of Open Responses: what Step Functions sends it, over
// HTTP, and no SDK, which may send what the specification does not have.
func (t *DefaultTransport) openResponses(ctx context.Context, call AgentCall) (any, error) {
	body := map[string]any{
		"model":        call.Model,
		"instructions": call.Instructions,
		"input":        ddJSONText(call.Input),
		"text":         map[string]any{"format": map[string]any{"type": "json_schema", "name": "answer", "strict": true, "schema": call.Schema}},
	}
	if call.Effort != "" {
		body["reasoning"] = map[string]any{"effort": call.Effort}
	}
	res, err := t.HTTP(ctx, HTTPRequest{Method: "POST", URL: strings.TrimRight(call.URL, "/") + "/responses", Body: body})
	if err != nil {
		return nil, err
	}
	if res.Status < 200 || res.Status >= 300 {
		return nil, &AgentHTTPError{Status: res.Status, Body: res.Body}
	}
	return ddResponsesAnswer(res.Body)
}

// ddResponsesAnswer is the answer of the Responses API as Step Functions reads it: the first
// output_text of the messages, as JSON. A refusal, or an answer without text, stops the agent.
func ddResponsesAnswer(out any) (any, error) {
	var content []map[string]any
	for _, o := range ddList(ddGet(out, "output")) {
		if ddGet(o, "type") != "message" {
			continue
		}
		for _, c := range ddList(ddGet(o, "content")) {
			if m, ok := c.(map[string]any); ok {
				content = append(content, m)
			}
		}
	}
	for _, c := range content {
		if text, ok := c["text"].(string); ok && c["type"] == "output_text" {
			var v any
			err := json.Unmarshal([]byte(text), &v)
			return v, err
		}
	}
	reason := "no answer"
	if s, ok := ddGet(out, "status").(string); ok {
		reason = s
	}
	for _, c := range content {
		if c["type"] == "refusal" {
			reason = "refusal"
		}
	}
	return nil, &AgentStopped{Reason: reason}
}

// ddBeating is a task that heartbeats while it runs, every {{BEAT_SECONDS}} seconds, so that Temporal notices a
// worker that went away; what it answers goes out as JSON, none as null.
func ddBeating(run func(ctx context.Context, args map[string]any) (any, error)) func(context.Context, map[string]any) (any, error) {
	return func(ctx context.Context, args map[string]any) (any, error) {
		done := make(chan struct{})
		defer close(done)
		go func() {
			tick := time.NewTicker({{BEAT_SECONDS}} * time.Second)
			defer tick.Stop()
			for {
				select {
				case <-done:
					return
				case <-tick.C:
					activity.RecordHeartbeat(ctx)
				}
			}
		}()
		v, err := run(ctx, args)
		if err != nil {
			return nil, err
		}
		return ddWire(v), nil
	}
}

// ddFailure is a declared error, as the workflow reads it: an ApplicationError of the error's
// type, which Temporal does not retry.
func ddFailure(kind, message string) error {
	return temporal.NewNonRetryableApplicationError(message, kind, nil)
}

// ddBadArgument is a rule's argument that is not one of its values.
func ddBadArgument(name string, v any) error {
	return fmt.Errorf("the rule's %s cannot be %s", name, ddJSONText(v))
}

// ddValue is the value of an answer, or the declared error it names (names: the other side's name -> the error).
func ddValue(o Outcome, err error, names map[string]string) (any, error) {
	if err != nil {
		return nil, err
	}
	if o.Error == "" {
		return o.OK, nil
	}
	if kind, ok := names[o.Error]; ok {
		return nil, ddFailure(kind, o.Message)
	}
	return nil, ddFailure("Dandori.Failure."+o.Error, o.Message)
}

// ddStatus is the body of a 2xx answer, or the declared error its status names.
func ddStatus(r HTTPResponse, err error, names map[string]string) (any, error) {
	if err != nil {
		return nil, err
	}
	if r.Status >= 200 && r.Status < 300 {
		return r.Body, nil
	}
	message, ok := r.Body.(string)
	if !ok {
		message = ddJSONText(r.Body)
	}
	if kind, ok := names[strconv.Itoa(r.Status)]; ok {
		return nil, ddFailure(kind, message)
	}
	return nil, ddFailure("Dandori.HttpStatus."+strconv.Itoa(r.Status), message)
}

// ddFold is a Claude agent's answer with every enum value in it spelled as schema spells it.
// Claude's structured outputs do not keep an enum value's case, so a value that differs from one
// only in case is taken as that one; what does not fit stays, for the answer's check to find.
func ddFold(v any, schema any) any {
	s, ok := schema.(map[string]any)
	if !ok {
		return v
	}
	if enum, ok := s["enum"].([]any); ok {
		text, isText := v.(string)
		if !isText {
			return v
		}
		for _, e := range enum {
			if e == text {
				return v
			}
		}
		for _, e := range enum {
			if es, ok := e.(string); ok && strings.ToLower(es) == strings.ToLower(text) {
				return es
			}
		}
		return v
	}
	if anyOf, ok := s["anyOf"].([]any); ok {
		for _, x := range anyOf {
			if ddGet(x, "type") != "null" {
				return ddFold(v, x)
			}
		}
		return v
	}
	switch s["type"] {
	case "array":
		if l, ok := v.([]any); ok {
			out := make([]any, len(l))
			for i, x := range l {
				out[i] = ddFold(x, s["items"])
			}
			return out
		}
	case "object":
		if m, ok := v.(map[string]any); ok {
			out := map[string]any{}
			for k, x := range m {
				out[k] = x
			}
			for k, p := range ddObject(s["properties"]) {
				if x, ok := out[k]; ok {
					out[k] = ddFold(x, p)
				}
			}
			return out
		}
	}
	return v
}

// ddAgentAnswer is what an agent answered: the value under "answer". An answer without it fails the call.
func ddAgentAnswer(out any, err error) (any, error) {
	if err != nil {
		return nil, err
	}
	if m, ok := out.(map[string]any); ok {
		if v, ok := m["answer"]; ok {
			return v, nil
		}
	}
	return nil, errors.New(`the agent's answer has no "answer"`)
}

// ddWhole is a number as protobuf's JSON writes a 64-bit one: its decimal string.
var ddWhole = regexp.MustCompile(`^-?[0-9]{1,16}$`)

// ddPick is an enum's value by its table, or as it is when the table has not got it.
func ddPick(values any, v any) any {
	if s, ok := v.(string); ok {
		if x, ok := ddObject(values)[s]; ok {
			return x
		}
	}
	return v
}

// ddConnectRule calls a rule at its Connect service: it POSTs the arguments as protobuf's JSON
// writes them (a number as its decimal string, an enum's value by the .proto's name, a none not at
// all), and reads the answer as the rule's own record: the zero values its JSON leaves out put back,
// a number from its decimal string (up to 16 digits, and not above 2^53 - 1), an enum's value by the
// rule's name for it. What is not one of those is left as it is, and so is an answer that is not an
// object: the workflow's check of the answer refuses them. A status other than 2xx fails the call.
// wire is {"url", "request": [field], "response": [field], "zeros"}, where a field is {"name": the
// rule's name for it, "json": its key in protobuf's JSON, "kind": "bool", "int", "str" or "enum",
// "values": for an enum, the rule's name of each value -> the .proto's for the request, and the other
// way round for the response}.
func ddConnectRule(ctx context.Context, t Transport, wire any, args map[string]any) (any, error) {
	body := map[string]any{}
	for _, f := range ddList(ddGet(wire, "request")) {
		v := args[ddStr(ddGet(f, "name"))]
		if v == nil {
			continue
		}
		switch ddGet(f, "kind") {
		case "int":
			if ddIsInt(v) {
				v = strconv.FormatInt(ddInt64(v), 10)
			}
		case "enum":
			v = ddPick(ddGet(f, "values"), v)
		}
		body[ddStr(ddGet(f, "json"))] = v
	}
	got, err := t.HTTP(ctx, HTTPRequest{Method: "POST", URL: ddStr(ddGet(wire, "url")), Headers: map[string]string{"Connect-Protocol-Version": "1"}, Body: body})
	answer, err := ddStatus(got, err, nil)
	if err != nil {
		return nil, err
	}
	answer = ddFill(answer, ddGet(wire, "zeros"))
	m, ok := answer.(map[string]any)
	if !ok {
		return answer, nil
	}
	out := map[string]any{}
	for _, f := range ddList(ddGet(wire, "response")) {
		x := m[ddStr(ddGet(f, "json"))]
		switch ddGet(f, "kind") {
		case "int":
			if s, ok := x.(string); ok && ddWhole.MatchString(s) {
				if n, err := strconv.ParseInt(s, 10, 64); err == nil && math.Abs(float64(n)) <= 9007199254740991 {
					x = n
				}
			}
		case "enum":
			x = ddPick(ddGet(f, "values"), x)
		}
		out[ddStr(ddGet(f, "name"))] = x
	}
	return out, nil
}

// ddUnit is x as a number from 0 to 1 (a bool is not a number here, as in JSON).
func ddUnit(x any) (float64, bool) {
	f, ok := ddNumber(x)
	return f, ok && f >= 0 && f <= 1
}

// ddJevOne is Jev's answer to one question: the value it takes and how sure Jev is of it, from 0
// to 1, and whether it is there and of its kind. A choice and a score say how sure ("confidence");
// a score's place goes to the nearest level, a half going up; a noul's answer is yes when the
// probability of yes is over one half, and Jev is as sure as the probability of the answer taken.
func ddJevOne(q map[string]any, a map[string]any) (any, float64, bool) {
	if q["kind"] == "noul" {
		p, ok := ddUnit(a["noul"])
		if !ok {
			return nil, 0, false
		}
		if p > 0.5 {
			return true, p, true
		}
		return false, 1 - p, true
	}
	c, ok := ddUnit(a["confidence"])
	if !ok {
		return nil, 0, false
	}
	values := ddList(q["values"])
	if q["kind"] == "choice" {
		if v, ok := a["choice"].(string); ok {
			for _, x := range values {
				if x == v {
					return v, c, true
				}
			}
		}
		return nil, 0, false
	}
	s, ok := ddNumber(a["score"])
	if !ok {
		return nil, 0, false
	}
	level := int(math.Floor(s + 0.5))
	if level >= 0 && level < len(values) {
		return values[level], c, true
	}
	return nil, 0, false
}

// ddJev is a Jev task's answer, read from the body of Jev's response as every target reads it: the
// value of the task's type, nil where an answer is not there or not of its kind (the workflow's
// check of the answer refuses it), and how sure Jev is as a count of a rate's steps, rounded down
// after a billionth that takes up how a decimal falls between binary fractions. When every answer
// is there and one is less sure than the task asks, the call fails with the task's error. task is
// {"questions", "read": [{"id", "field", "kind", "values"}], "confidences": [{"field", "question",
// "per"}], "floor": {"at", "error", "cause"}}.
func ddJev(body any, task any) (any, error) {
	answers := ddObject(ddGet(body, "answers"))
	read := ddList(ddGet(task, "read"))
	type taken struct {
		v    any
		sure float64
		ok   bool
	}
	got := make([]taken, len(read))
	for i, q := range read {
		if a, ok := answers[ddStr(ddGet(q, "id"))].(map[string]any); ok {
			v, sure, ok := ddJevOne(ddObject(q), a)
			got[i] = taken{v, sure, ok}
		}
	}
	if floor := ddObject(ddGet(task, "floor")); floor != nil {
		every, low := true, false
		for _, g := range got {
			if !g.ok {
				every = false
			} else if g.sure < ddNum(floor["at"]) {
				low = true
			}
		}
		if every && low {
			return nil, ddFailure(ddStr(floor["error"]), ddStr(floor["cause"]))
		}
	}
	if len(read) == 1 && ddGet(read[0], "field") == nil {
		return got[0].v, nil
	}
	out := map[string]any{}
	for i, q := range read {
		field := ddStr(ddGet(q, "field"))
		if field == "" {
			field = ddStr(ddGet(q, "id"))
		}
		out[field] = got[i].v
	}
	for _, c := range ddList(ddGet(task, "confidences")) {
		var v any
		if i := ddInt(ddGet(c, "question")); i < len(got) && got[i].ok {
			v = math.Floor(got[i].sure*ddNum(ddGet(c, "per")) + 1e-9)
		}
		out[ddStr(ddGet(c, "field"))] = v
	}
	return out, nil
}
"##;

/// io_aws.go: Lambda and the AWS APIs the flow calls, through the AWS SDK for Go v2.
fn io_aws_file(n: &Needs, pkg: &str, header: &str) -> String {
    let known: Vec<&(String, String)> = n.aws.iter().filter(|(s, _)| AWS_SERVICES.contains(&s.as_str())).collect();
    let api = !n.aws.is_empty();
    let mut t = header.to_string();
    t.push_str("// Lambda and the AWS APIs, through the AWS SDK for Go v2: the clients of the services the flow calls.\n");
    t.push_str(&format!("\npackage {pkg}\n\nimport (\n\t\"context\"\n"));
    if n.lambda || !known.is_empty() {
        t.push_str("\t\"encoding/json\"\n");
    }
    if api {
        t.push_str("\t\"errors\"\n\t\"fmt\"\n\t\"reflect\"\n");
    }
    t.push_str("\n\t\"github.com/aws/aws-sdk-go-v2/aws\"\n\t\"github.com/aws/aws-sdk-go-v2/config\"\n");
    let mut services: BTreeSet<&str> = known.iter().map(|(s, _)| s.as_str()).collect();
    if n.lambda {
        services.insert("lambda");
    }
    for s in &services {
        t.push_str(&format!("\t\"github.com/aws/aws-sdk-go-v2/service/{s}\"\n"));
    }
    if api {
        t.push_str("\t\"github.com/aws/smithy-go\"\n");
    }
    t.push_str(")\n\nfunc init() {\n");
    if n.lambda {
        t.push_str("\tddLambdaCall = ddAWSLambda\n");
    }
    if api {
        t.push_str("\tddAWSCall = ddAWSAPI\n");
    }
    t.push_str("}\n\n");
    t.push_str("// awsConfig is the configuration of the AWS SDK's clients: TransportOptions.AWS, or what the SDK's\n// default chain finds, loaded once.\nfunc (t *DefaultTransport) awsConfig(ctx context.Context) (aws.Config, error) {\n\tc, err := t.client(\"aws\", func() (any, error) {\n\t\tif t.o.AWS != nil {\n\t\t\treturn *t.o.AWS, nil\n\t\t}\n\t\treturn config.LoadDefaultConfig(ctx)\n\t})\n\tif err != nil {\n\t\treturn aws.Config{}, err\n\t}\n\treturn c.(aws.Config), nil\n}\n");
    if n.lambda {
        t.push_str(r##"
// ddAWSLambda invokes a Lambda function; an error the function raises comes back by its type.
func ddAWSLambda(ctx context.Context, t *DefaultTransport, fn string, payload map[string]any) (Outcome, error) {
	cfg, err := t.awsConfig(ctx)
	if err != nil {
		return Outcome{}, err
	}
	out, err := lambda.NewFromConfig(cfg).Invoke(ctx, &lambda.InvokeInput{FunctionName: aws.String(fn), Payload: []byte(ddJSONText(payload))})
	if err != nil {
		return Outcome{}, err
	}
	var body any
	if len(out.Payload) > 0 {
		if err := json.Unmarshal(out.Payload, &body); err != nil {
			return Outcome{}, err
		}
	}
	if out.FunctionError != nil {
		name, ok := ddGet(body, "errorType").(string)
		if !ok {
			name = *out.FunctionError
		}
		message, _ := ddGet(body, "errorMessage").(string)
		return Outcome{Error: name, Message: message}, nil
	}
	return Outcome{OK: body}, nil
}
"##);
    }
    if api {
        t.push_str("\n// ddAWSAPI calls an AWS API, named as Step Functions names it (sns, publish): the input's JSON\n// goes into the SDK's input struct, whose fields take the API's names (QueueUrl) as they are, and\n// the answer comes back as JSON, without the SDK's metadata.\nfunc ddAWSAPI(ctx context.Context, t *DefaultTransport, service, action string, input map[string]any) (Outcome, error) {\n");
        if !known.is_empty() {
            t.push_str("\tcfg, err := t.awsConfig(ctx)\n\tif err != nil {\n\t\treturn Outcome{}, err\n\t}\n");
            t.push_str("\targs := input\n\t// SQS takes the message as text; Step Functions writes a JSON body out the same way\n\tif service == \"sqs\" && action == \"sendMessage\" {\n\t\tswitch b := input[\"MessageBody\"].(type) {\n\t\tcase map[string]any, []any:\n\t\t\targs = ddWith(input, \"MessageBody\", ddJSONText(b))\n\t\t}\n\t}\n");
            t.push_str("\tvar out any\n\tswitch service + \":\" + action {\n");
            for (s, a) in &known {
                let method = {
                    let mut cs = a.chars();
                    cs.next().map(|f| f.to_uppercase().chain(cs).collect::<String>()).unwrap_or_default()
                };
                t.push_str(&format!(
                    "\tcase {}:\n\t\tvar in {s}.{method}Input\n\t\tif err := ddAWSIn(args, &in); err != nil {{\n\t\t\treturn Outcome{{}}, err\n\t\t}}\n\t\to, err := {s}.NewFromConfig(cfg).{method}(ctx, &in)\n\t\tif err != nil {{\n\t\t\treturn ddAWSError(err)\n\t\t}}\n\t\tout = o\n",
                    q(&format!("{s}:{a}"))
                ));
            }
            t.push_str("\tdefault:\n\t\treturn Outcome{}, fmt.Errorf(\"the default Transport has no client for %s:%s; pass a Transport whose AWS calls it\", service, action)\n\t}\n");
            t.push_str("\tv, err := ddAWSOut(out)\n\tif err != nil {\n\t\treturn Outcome{}, err\n\t}\n\treturn Outcome{OK: v}, nil\n}\n");
        } else {
            t.push_str("\treturn Outcome{}, fmt.Errorf(\"the default Transport has no client for %s:%s; pass a Transport whose AWS calls it\", service, action)\n}\n");
        }
        if !known.is_empty() {
            t.push_str(r##"
// ddAWSIn is a call's input, from its JSON value.
func ddAWSIn(input map[string]any, in any) error {
	b, err := json.Marshal(input)
	if err != nil {
		return err
	}
	return json.Unmarshal(b, in)
}

// ddAWSOut is the API's answer as JSON, without the SDK's metadata and the members it has not got.
func ddAWSOut(out any) (any, error) {
	b, err := json.Marshal(out)
	if err != nil {
		return nil, err
	}
	var v any
	if err := json.Unmarshal(b, &v); err != nil {
		return nil, err
	}
	if m, ok := v.(map[string]any); ok {
		delete(m, "ResultMetadata")
	}
	return ddPrune(v), nil
}

// ddPrune is a JSON value without the members that are null: the SDK's struct has every member, a
// nil one for each the answer has not got.
func ddPrune(v any) any {
	switch x := v.(type) {
	case map[string]any:
		out := map[string]any{}
		for k, y := range x {
			if y != nil {
				out[k] = ddPrune(y)
			}
		}
		return out
	case []any:
		out := make([]any, len(x))
		for i, y := range x {
			out[i] = ddPrune(y)
		}
		return out
	}
	return v
}
"##);
        }
        t.push_str(r##"
// ddAWSError is an exception of the API's, by the name the API's model gives it (NotFoundException),
// as the SDK for JavaScript names it; the code on the wire can be another (NotFound).
func ddAWSError(err error) (Outcome, error) {
	var ae smithy.APIError
	if !errors.As(err, &ae) {
		return Outcome{}, err
	}
	name := ae.ErrorCode()
	if _, generic := ae.(*smithy.GenericAPIError); !generic {
		t := reflect.TypeOf(ae)
		for t.Kind() == reflect.Pointer {
			t = t.Elem()
		}
		name = t.Name()
	}
	return Outcome{Error: name, Message: ae.ErrorMessage()}, nil
}
"##);
    }
    t
}

const IO_OPENAI: &str = r##"// OpenAI's agents, through OpenAI's Go client and the Responses API. Go has no Agents SDK of
// OpenAI's; the agents here take no tools and answer once, which one request of the Responses API
// does, with what Step Functions sends.

package {{PKG}}

import (
	"context"
	"encoding/json"

	"github.com/openai/openai-go/v3"
	"github.com/openai/openai-go/v3/option"
	"github.com/openai/openai-go/v3/responses"
	"github.com/openai/openai-go/v3/shared"
)

func init() {
	ddOpenAIAgent = ddOpenAI
}

// ddOpenAI runs an agent of OpenAI's: the model, the instructions, the input as JSON text, and the
// answer's JSON Schema. The client does not retry by itself unless TransportOptions.OpenAI says so:
// the workflow retries, as `retry` says.
func ddOpenAI(ctx context.Context, t *DefaultTransport, call AgentCall) (any, error) {
	c, err := t.client("openai", func() (any, error) {
		opts := []option.RequestOption{option.WithMaxRetries(0)}
		if t.o.HTTPClient != nil {
			opts = append(opts, option.WithHTTPClient(t.o.HTTPClient))
		}
		c := openai.NewClient(append(opts, t.o.OpenAI...)...)
		return &c, nil
	})
	if err != nil {
		return nil, err
	}
	params := responses.ResponseNewParams{
		Model:        call.Model,
		Instructions: openai.String(call.Instructions),
		Input:        responses.ResponseNewParamsInputUnion{OfString: openai.String(ddJSONText(call.Input))},
		Text: responses.ResponseTextConfigParam{
			Format: responses.ResponseFormatTextConfigUnionParam{
				OfJSONSchema: &responses.ResponseFormatTextJSONSchemaConfigParam{Name: "answer", Schema: call.Schema, Strict: openai.Bool(true)},
			},
		},
	}
	if call.Effort != "" {
		params.Reasoning = shared.ReasoningParam{Effort: shared.ReasoningEffort(call.Effort)}
	}
	res, err := c.(*openai.Client).Responses.New(ctx, params)
	if err != nil {
		return nil, err
	}
	var out any
	if err := json.Unmarshal([]byte(res.RawJSON()), &out); err != nil {
		return nil, err
	}
	return ddResponsesAnswer(out)
}
"##;

const IO_CLAUDE: &str = r##"// Claude's agents, through Anthropic's Go SDK and the Messages API.

package {{PKG}}

import (
	"context"
	"encoding/json"
	"strings"

	"github.com/anthropics/anthropic-sdk-go"
	"github.com/anthropics/anthropic-sdk-go/option"
)

func init() {
	ddClaudeAgent = ddClaude
}

// ddClaude runs a Claude agent with what Step Functions sends: the instructions as the system
// prompt, the input as the user's message, both as text (the SDK writes them as lists of text
// blocks, so the two are set as they go), the most the answer may take, and its JSON Schema. The
// client does not retry by itself unless TransportOptions.Claude says so: the workflow retries, as
// `retry` says.
func ddClaude(ctx context.Context, t *DefaultTransport, call AgentCall) (any, error) {
	c, err := t.client("claude", func() (any, error) {
		opts := []option.RequestOption{option.WithMaxRetries(0)}
		if t.o.HTTPClient != nil {
			opts = append(opts, option.WithHTTPClient(t.o.HTTPClient))
		}
		c := anthropic.NewClient(append(opts, t.o.Claude...)...)
		return &c, nil
	})
	if err != nil {
		return nil, err
	}
	config := anthropic.OutputConfigParam{Format: anthropic.JSONOutputFormatParam{Schema: call.Schema}}
	if call.Effort != "" {
		config.Effort = anthropic.OutputConfigEffort(call.Effort)
	}
	text := ddJSONText(call.Input)
	message, err := c.(*anthropic.Client).Messages.New(ctx, anthropic.MessageNewParams{
		Model:        anthropic.Model(call.Model),
		MaxTokens:    ClaudeMaxTokens,
		Messages:     []anthropic.MessageParam{anthropic.NewUserMessage(anthropic.NewTextBlock(text))},
		OutputConfig: config,
	}, option.WithJSONSet("system", call.Instructions), option.WithJSONSet("messages", []map[string]any{{"role": "user", "content": text}}))
	if err != nil {
		return nil, err
	}
	if message.StopReason != "end_turn" {
		return nil, &AgentStopped{Reason: string(message.StopReason)}
	}
	var b strings.Builder
	for _, block := range message.Content {
		if block.Type == "text" {
			b.WriteString(block.Text)
		}
	}
	var out any
	err = json.Unmarshal([]byte(b.String()), &out)
	return out, err
}
"##;

/// What a task's code gets, as a comment: its arguments, the key, a callback's id, and the answer.
fn task_doc(m: &Model, n: &Names, task: &TaskDef) -> String {
    let mut params: Vec<String> = task.params.iter().map(|(p, pt)| format!("{}: {}", q(p), go_type(m, n, pt))).collect();
    if task.key {
        params.push("\"idempotency_key\": string".into());
    }
    if task.callback {
        params.push("\"callback_id\": string".into());
    }
    let answer = if task.callback {
        "nothing; the answer comes to the callback".to_string()
    } else {
        match &task.result {
            Some(t) => format!("a {} as JSON", go_type(m, n, t)),
            None => "anything; the workflow does not read it".into(),
        }
    };
    format!("args {{{}}}. Answers {answer}.", params.join(", "))
}

/// The body of the activity that runs a task dandori writes, as lines, inside a function of
/// (ctx, args) that answers (any, error); `t` is the Transport.
fn task_body(m: &Model, task: &TaskDef) -> Vec<String> {
    let p = Platform::Temporal;
    let names = |pairs: Vec<(String, String)>| -> String {
        if pairs.is_empty() {
            "nil".into()
        } else {
            format!("map[string]string{{{}}}", pairs.iter().map(|(k, v)| format!("{}: {}", q(k), q(v))).collect::<Vec<_>>().join(", "))
        }
    };
    let statuses = || names(task.errors.iter().filter_map(|e| e.status.map(|s| (s.to_string(), e.name.clone()))).collect());
    let returned = |call: String| -> Vec<String> {
        if task.callback {
            vec![format!("if _, err := {call}; err != nil {{"), "\treturn nil, err".into(), "}".into(), "return nil, nil".into()]
        } else {
            vec![format!("return {call}")]
        }
    };
    match task.via(p) {
        Some(Via::Lambda(f)) => {
            let mut out = vec![format!("o, err := t.Lambda(ctx, {}, args)", q(f))];
            out.extend(returned(format!("ddValue(o, err, {})", names(task.errors.iter().map(|e| (e.name.clone(), e.name.clone())).collect()))));
            out
        }
        Some(Via::Http { method, url, form }) => {
            let used = crate::lower::placeholders(url);
            let mut url_parts: Vec<String> = Vec::new();
            let mut rest = url;
            while let Some(i) = rest.find('{') {
                let j = match rest[i..].find('}') {
                    Some(j) => i + j,
                    None => break,
                };
                if i > 0 {
                    url_parts.push(q(&rest[..i]));
                }
                url_parts.push(format!("ddText(args[{}])", q(&rest[i + 1..j])));
                rest = &rest[j + 1..];
            }
            if !rest.is_empty() || url_parts.is_empty() {
                url_parts.push(q(rest));
            }
            let mut req = vec![format!("Method: {}", q(method)), format!("URL: {}", url_parts.join(" + "))];
            let mut headers = Vec::new();
            if form {
                headers.push("\"Content-Type\": \"application/x-www-form-urlencoded\"".to_string());
            }
            if task.connect.is_some() {
                headers.push("\"Connect-Protocol-Version\": \"1\"".to_string());
            }
            if task.key {
                headers.push("\"Idempotency-Key\": ddStr(args[\"idempotency_key\"])".to_string());
            }
            if !headers.is_empty() {
                req.push(format!("Headers: map[string]string{{{}}}", headers.join(", ")));
            }
            let rest_args: Vec<String> = task.params.iter().filter(|(p, _)| !used.contains(p)).map(|(p, _)| format!("{}: args[{}]", q(p), q(p))).collect();
            if !rest_args.is_empty() {
                let place = if method == "GET" || method == "DELETE" { "Query" } else { "Body" };
                req.push(format!("{place}: map[string]any{{{}}}", rest_args.join(", ")));
            }
            if form {
                req.push("Form: true".into());
            }
            let mut out = vec![format!("res, err := t.HTTP(ctx, HTTPRequest{{{}}})", req.join(", "))];
            match &task.connect {
                Some(zeros) => {
                    out.push(format!("v, err := ddStatus(res, err, {})", statuses()));
                    out.push("if err != nil {".into());
                    out.push("\treturn nil, err".into());
                    out.push("}".into());
                    out.push(format!("return ddFill(v, {}), nil", json_value(zeros)));
                }
                None => out.extend(returned(format!("ddStatus(res, err, {})", statuses()))),
            }
            out
        }
        Some(Via::Aws { service, action }) => {
            let mut input: Vec<String> = task
                .params
                .iter()
                .map(|(p, _)| {
                    if task.callback && p == "MessageBody" {
                        format!("{}: ddWith(args[{}], \"callback_id\", args[\"callback_id\"])", q(p), q(p))
                    } else {
                        format!("{}: args[{}]", q(p), q(p))
                    }
                })
                .collect();
            if let (true, Some(kp)) = (task.key, &task.key_param) {
                input.push(format!("{}: args[\"idempotency_key\"]", q(kp)));
            }
            let mut out = vec![format!("o, err := t.AWS(ctx, {}, {}, map[string]any{{{}}})", q(service), q(action), input.join(", "))];
            out.extend(returned(format!("ddValue(o, err, {})", names(task.errors.iter().map(|e| (e.exception.clone().unwrap_or_else(|| e.name.clone()), e.name.clone())).collect()))));
            out
        }
        Some(Via::Agent { provider, instructions, model, url, effort }) => {
            let input: Vec<String> = task.params.iter().map(|(p, _)| format!("{}: args[{}]", q(p), q(p))).collect();
            let mut call = vec![
                format!("Agent: {}", q(&task.name)),
                format!("Provider: {}", q(provider.name())),
                format!("Model: {}", q(model)),
                format!("Instructions: {}", q(instructions)),
                format!("Input: map[string]any{{{}}}", input.join(", ")),
                format!("Schema: ddObject(ddSchemas[{}])", q(&task.name)),
            ];
            if let Some(u) = url {
                call.push(format!("URL: {}", q(u)));
            }
            if let Some(e) = effort {
                call.push(format!("Effort: {}", q(e)));
            }
            let mut out = vec!["out, err := t.Agent(ctx, AgentCall{".to_string()];
            out.extend(call.iter().map(|c| format!("\t{c},")));
            out.push("})".into());
            // Claude may answer an enum's value in another case: it is taken as the value
            if provider == Provider::Claude && !render::enums_of(m, task.result.as_ref().unwrap_or(&Ty::Json)).is_empty() {
                out.push(format!("return ddAgentAnswer(ddFold(out, ddSchemas[{}]), err)", q(&task.name)));
            } else {
                out.push("return ddAgentAnswer(out, err)".into());
            }
            out
        }
        Some(Via::Jev(j)) => {
            let state: Vec<String> = task.params.iter().map(|(p, _)| format!("{}: args[{}]", q(p), q(p))).collect();
            vec![
                format!(
                    "res, err := t.HTTP(ctx, HTTPRequest{{Method: \"POST\", URL: JevURL, Body: map[string]any{{\"state\": map[string]any{{{}}}, \"model\": {}, \"questions\": ddGet(ddJevs[{}], \"questions\")}}, TypeSafe: true}})",
                    state.join(", "),
                    q(&j.model),
                    q(&task.name)
                ),
                format!("body, err := ddStatus(res, err, {})", statuses()),
                "if err != nil {".into(),
                "\treturn nil, err".into(),
                "}".into(),
                format!("return ddJev(body, ddJevs[{}])", q(&task.name)),
            ]
        }
        _ => vec!["return nil, errors.New(\"not written\")".into()],
    }
}

/// activities.go: every task the workflow calls as an activity, the ones the user writes, and the
/// code for the others.
fn activities_file(m: &Model, n: &Names, pkg: &str, header: &str) -> String {
    let p = Platform::Temporal;
    let tasks: Vec<&TaskDef> = m.tasks.iter().filter(|t| !t.is_child(p) && !t.event).collect();
    let own: Vec<&TaskDef> = tasks.iter().filter(|t| matches!(t.via(p), Some(Via::Own))).cloned().collect();
    let connected = crate::temporal::connect_rules(m);
    let mut a = header.to_string();
    a.push_str("// The tasks the workflow calls.\n//\n");
    a.push_str("// - A task that says `lambda`, `http` or `aws` is written here: it sends what Step Functions\n");
    a.push_str("//   would send, through a Transport (io.go), where the credentials and the clients are yours to set.\n");
    a.push_str("// - So is a task that says `agent`: the model gets the arguments as JSON text, as from Step\n");
    a.push_str("//   Functions, and answers {\"answer\": …} in the JSON Schema below; the Transport runs the agent.\n");
    a.push_str("//   A Claude agent's enum values are taken without regard to case (ddFold).\n");
    a.push_str("// - So is a task that says `jev`: TypeSafe's Jev reads the arguments as its state and answers the\n");
    a.push_str("//   questions below, sent over HTTP (JevURL) with TypeSafe's key, and ddJev reads the answer.\n");
    if !connected.is_empty() {
        a.push_str("// - So is a rule that says `connect` under `use rule`: it is called at its Connect service, over HTTP\n");
        a.push_str("//   through the Transport, and the answer is read as the rule's record (ddConnectRule). Its name is the\n");
        a.push_str("//   rule's activity, `rule_<rule>`, the same as when the rule's code goes with the workflow.\n");
    }
    a.push_str("// - The others are yours to write (OwnTasks). Each gets its arguments as JSON values (Decode reads\n");
    a.push_str("//   them into the types of types.go) and answers a value the SDK writes as JSON. A declared error is\n");
    a.push_str("//   returned as temporal.NewNonRetryableApplicationError(\"...\", \"<error>\", nil).\n");
    a.push_str("// - The workflow retries by itself, as the `retry` of each task says; the platform does not.\n");
    a.push_str("// - A task with `key` gets `idempotency_key`: pass it on to the other side as it is.\n");
    a.push_str("// - A `callback` task gets `callback_id` and returns once it has handed the id on. The answer goes\n");
    a.push_str("//   to the workflow the id names, with client.go's Answer (the update `dandori.answer`, or the\n");
    a.push_str("//   signal `dandori.callback`): {\"callback_id\", \"ok\"} or {\"callback_id\", \"error\", \"message\"}.\n");
    a.push_str("// - A task that says `event` is not here: nothing is called, and its value is sent to the workflow\n");
    a.push_str("//   (client.go: Send).\n");
    a.push_str(&format!("\npackage {pkg}\n\nimport \"context\"\n\n"));
    a.push_str("// OwnTasks are the tasks you write: the ones that say neither `lambda`, `http`, `aws`, `agent` nor `jev`.\ntype OwnTasks interface {\n");
    for task in &own {
        let method = &n.methods[&task.name];
        a.push_str(&format!("\t// {method} runs the task {}: {}\n\t{method}(ctx context.Context, args map[string]any) (any, error)\n", task.name, task_doc(m, n, task)));
    }
    a.push_str("}\n\n");
    a.push_str("// OwnTasksBy is an OwnTasks whose every method calls f with the task's name as the .flow writes it.\n");
    a.push_str("func OwnTasksBy(f func(ctx context.Context, task string, args map[string]any) (any, error)) OwnTasks {\n\treturn ddOwnTasksBy{f}\n}\n\n");
    a.push_str("type ddOwnTasksBy struct {\n\tf func(ctx context.Context, task string, args map[string]any) (any, error)\n}\n");
    for task in &own {
        let method = &n.methods[&task.name];
        a.push_str(&format!("\nfunc (o ddOwnTasksBy) {method}(ctx context.Context, args map[string]any) (any, error) {{\n\treturn o.f(ctx, {}, args)\n}}\n", q(&task.name)));
    }
    let agents: Vec<&&TaskDef> = tasks.iter().filter(|t| matches!(t.via(p), Some(Via::Agent { .. }))).collect();
    if !agents.is_empty() {
        a.push_str("\n// ddSchemas are what each agent answers in: the JSON Schema its provider's structured outputs hold the model to.\nvar ddSchemas = map[string]any{\n");
        for task in agents {
            let schema = render::agent_schema(m, task).expect("the checker gives an agent an answer with a schema");
            a.push_str(&format!("\t{}: {},\n", q(&task.name), json_value(&schema)));
        }
        a.push_str("}\n");
    }
    let jevs: Vec<&&TaskDef> = tasks.iter().filter(|t| matches!(t.via(p), Some(Via::Jev(_)))).collect();
    if !jevs.is_empty() {
        a.push_str("\n// ddJevs are what each Jev task asks, and how ddJev reads the answer.\nvar ddJevs = map[string]any{\n");
        for task in jevs {
            let spec = render::jev_spec(task, task.jev().expect("a Jev task"));
            a.push_str(&format!("\t{}: {},\n", q(&task.name), json_value(&spec)));
        }
        a.push_str("}\n");
    }
    if !connected.is_empty() {
        a.push_str("\n// ddRuleWires are where each rule is, and how a call is written and its answer read: `connect` under `use rule`.\nvar ddRuleWires = map[string]any{\n");
        for r in &connected {
            let ru = &m.rules[*r];
            let wire = render::rule_wire_spec(ru.connect.as_ref().expect("a rule at its service"));
            a.push_str(&format!("\t{}: {},\n", q(&ru.name), json_value(&wire)));
        }
        a.push_str("}\n");
    }
    a.push_str("\n// Activities are your tasks and the ones dandori writes, by the names the workflow calls them by.\n// A nil Transport is the default one, NewTransport(TransportOptions{}).\n");
    a.push_str("func Activities(own OwnTasks, t Transport) map[string]func(context.Context, map[string]any) (any, error) {\n\tif t == nil {\n\t\tt = NewTransport(TransportOptions{})\n\t}\n");
    if tasks.is_empty() && connected.is_empty() {
        a.push_str("\treturn map[string]func(context.Context, map[string]any) (any, error){}\n}\n");
        return a;
    }
    a.push_str("\treturn map[string]func(context.Context, map[string]any) (any, error){\n");
    for task in &tasks {
        let name = ident(&task.name);
        a.push_str(&format!("\t\t// {}\n", task_doc(m, n, task)));
        a.push_str(&format!("\t\t{}: ddBeating(func(ctx context.Context, args map[string]any) (any, error) {{\n", q(&name)));
        match task.via(p) {
            Some(Via::Own) => a.push_str(&format!("\t\t\treturn own.{}(ctx, args)\n", n.methods[&task.name])),
            _ => {
                for l in task_body(m, task) {
                    a.push_str(&format!("\t\t\t{l}\n"));
                }
            }
        }
        a.push_str("\t\t}),\n");
    }
    // the rules at their services are activities too, but not ones to heartbeat: a rule is done in no time
    for r in &connected {
        let ru = &m.rules[*r];
        a.push_str(&format!(
            "\t\t// The rule {} v{}, called at its Connect service. Answers the record of its outputs.\n\t\t{}: func(ctx context.Context, args map[string]any) (any, error) {{\n\t\t\tv, err := ddConnectRule(ctx, t, ddRuleWires[{}], args)\n\t\t\tif err != nil {{\n\t\t\t\treturn nil, err\n\t\t\t}}\n\t\t\treturn ddWire(v), nil\n\t\t}},\n",
            ru.info.rule,
            ru.info.version,
            q(&render::rule_activity(&ru.name)),
            q(&ru.name)
        ));
    }
    a.push_str("\t}\n}\n");
    if a.contains("errors.New(") {
        a = a.replacen("import \"context\"\n", "import (\n\t\"context\"\n\t\"errors\"\n)\n", 1);
    }
    a
}

/// rules.go: every rule the flow calls whose code goes with the workflow, around the Go rulec
/// generates, as activities.
fn rules_file(m: &Model, pkg: &str, header: &str, called: &BTreeSet<usize>) -> String {
    let mut t = header.to_string();
    t.push_str("// The rules the workflow calls, as activities around the Go rulec generates. `rulec gen <rule>\n");
    t.push_str("// --out <this package's directory>/rulec` writes each package these imports read, as a module of its\n");
    t.push_str("// own (rulec/go/<package>), which your go.mod requires and replaces with that directory:\n//\n");
    let mut packages = BTreeSet::new();
    for r in called {
        packages.insert(m.rules[*r].info.api["go"]["package"].as_str().unwrap_or("rule").to_string());
    }
    for p in &packages {
        t.push_str(&format!("//\trequire {p} v0.0.0\n//\treplace {p} => ./<this package's directory>/rulec/go/{p}\n"));
    }
    t.push_str(&format!("\npackage {pkg}\n\nimport (\n\t\"context\"\n\n"));
    for p in &packages {
        t.push_str(&format!("\t{}\n", q(p)));
    }
    t.push_str(")\n\n// ddRules are the rules whose code goes with the workflow, by the names the workflow calls them by.\nvar ddRules = map[string]any{\n");
    for r in called {
        let ru = &m.rules[*r];
        t.push_str(&format!("\t{}: ddRule_{},\n", q(&render::rule_activity(&ru.name)), ident(&ru.name)));
    }
    t.push_str("}\n");
    for r in called {
        let ru = &m.rules[*r];
        let go = &ru.info.api["go"];
        let pkgname = go["package"].as_str().unwrap_or("rule");
        let enums: Vec<&str> = go["enums"].as_array().map(|a| a.iter().filter_map(|e| e["alias"].as_str()).collect()).unwrap_or_default();
        let is_enum = |ty: &str| enums.contains(&ty);
        let mut body = Vec::new();
        let mut fields = Vec::new();
        for (i, f) in go["input_fields"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let name = f["name"].as_str().unwrap_or("");
            let alias = f["alias"].as_str().unwrap_or("");
            let ty = f["type"].as_str().unwrap_or("");
            let get = format!("args[{}]", q(name));
            let v = if is_enum(ty) {
                let x = format!("dd_in{i}");
                body.push(format!("{x}, ok := {pkgname}.Parse{ty}(ddStr({get}))"));
                body.push("if !ok {".into());
                body.push(format!("\treturn nil, ddBadArgument({}, {get})", q(name)));
                body.push("}".into());
                x
            } else if ty == "bool" {
                format!("{get} == true")
            } else if ty == "string" {
                format!("ddStr({get})")
            } else if ty == "int64" {
                format!("ddInt64({get})")
            } else {
                // a number with a unit is an int64 of a type of its own
                format!("{pkgname}.{ty}(ddInt64({get}))")
            };
            fields.push(format!("{alias}: {v}"));
        }
        let outputs = go["output_fields"].as_array().cloned().unwrap_or_default();
        let mut outs = Vec::new();
        for o in &outputs {
            let name = o["name"].as_str().unwrap_or("");
            let alias = o["alias"].as_str().unwrap_or("");
            let ty = o["type"].as_str().unwrap_or("");
            // with one output, the rule's function answers that value itself, not a record of outputs
            let x = if outputs.len() == 1 { "out".to_string() } else { format!("out.{alias}") };
            let v = if is_enum(ty) {
                format!("{x}.String()")
            } else if ty == "bool" || ty == "string" {
                x
            } else {
                format!("int64({x})")
            };
            outs.push(format!("{}: {v}", q(name)));
        }
        t.push_str(&format!("\n// ddRule_{} is the rule {} v{}.\nfunc ddRule_{}(ctx context.Context, args map[string]any) (any, error) {{\n", ident(&ru.name), ru.info.rule, ru.info.version, ident(&ru.name)));
        for l in &body {
            t.push_str(&format!("\t{l}\n"));
        }
        t.push_str(&format!(
            "\tout, err := {pkgname}.{}({pkgname}.{}{{{}}})\n\tif err != nil {{\n\t\treturn nil, err\n\t}}\n\treturn map[string]any{{{}}}, nil\n}}\n",
            go["func"].as_str().unwrap_or("Rule"),
            go["input_type"].as_str().unwrap_or("Input"),
            fields.join(", "),
            outs.join(", ")
        ));
    }
    t
}

/// client.go: starting the workflow, answering its callbacks, and asking where it is.
fn client_file(m: &Model, n: &Names, pkg: &str, header: &str) -> String {
    let ty = crate::temporal::workflow_type(m);
    let mut c = header.to_string();
    c.push_str(&format!("// Starting {} v{}, answering its callbacks, sending it events, and asking where it is.\n\npackage {pkg}\n\n", m.name, m.version));
    c.push_str("import (\n\t\"context\"\n\t\"encoding/json\"\n\t\"fmt\"\n\n\tenumspb \"go.temporal.io/api/enums/v1\"\n\thistorypb \"go.temporal.io/api/history/v1\"\n\t\"go.temporal.io/api/workflowservice/v1\"\n\t\"go.temporal.io/sdk/client\"\n)\n\n");
    c.push_str(&format!("// WorkflowType is the workflow's type. The .flow's version is in it, so that a new version is a new workflow.\nconst WorkflowType = {}\n\n", q(&ty)));
    c.push_str("// TaskQueue is the task queue of the workflow and its activities (the tasks that say `queue` go to theirs).\nconst TaskQueue = WorkflowType\n\n");
    let events: Vec<String> = m.tasks.iter().filter(|t| t.event).map(|t| q(&t.name)).collect();
    c.push_str(&format!("// Events are the events the workflow waits for (the tasks that say `event`), by name.\nvar Events = []string{{{}}}\n\n", events.join(", ")));
    c.push_str("// Start starts the workflow as id, with input, a WorkflowInput or its JSON value. The idempotency\n// keys of its calls are made from the id, so an id is used once: a second start with it is refused,\n// even after the first run has ended. With searchAttributes, the workflow keeps the search attribute\n// DandoriCases (a keyword list) up to date with the cases' states, as \"<case>=<state>\"; register it on\n// the namespace first.\n");
    c.push_str("func Start(ctx context.Context, c client.Client, id string, input any, searchAttributes bool) (client.WorkflowRun, error) {\n\to := client.StartWorkflowOptions{ID: id, TaskQueue: TaskQueue, WorkflowIDReusePolicy: enumspb.WORKFLOW_ID_REUSE_POLICY_REJECT_DUPLICATE, WorkflowExecutionErrorWhenAlreadyStarted: true}\n\tif searchAttributes {\n\t\to.Memo = map[string]any{\"dandori.cases\": true}\n\t}\n\treturn c.ExecuteWorkflow(ctx, o, WorkflowType, input)\n}\n\n");
    c.push_str("// CallbackAnswer is a callback's answer, or an event's value: OK, or the error Error names, with Message.\ntype CallbackAnswer struct {\n\tOK      any\n\tError   string\n\tMessage string\n}\n\n");
    c.push_str("func (a CallbackAnswer) fields() map[string]any {\n\tif a.Error != \"\" {\n\t\treturn map[string]any{\"error\": a.Error, \"message\": a.Message}\n\t}\n\treturn map[string]any{\"ok\": a.OK}\n}\n\n");
    c.push_str("// Answer answers a callback, by the id its task handed on. The workflow says whether it took the\n// answer: it refuses one for a callback it does not wait for, and a second one.\nfunc Answer(ctx context.Context, c client.Client, callbackID string, a CallbackAnswer) error {\n\tvar id []any\n\tif err := json.Unmarshal([]byte(callbackID), &id); err != nil || len(id) != 2 {\n\t\treturn fmt.Errorf(\"%q is not the id of a callback\", callbackID)\n\t}\n\tworkflowID, _ := id[0].(string)\n\targs := a.fields()\n\targs[\"callback_id\"] = callbackID\n\th, err := c.UpdateWorkflow(ctx, client.UpdateWorkflowOptions{WorkflowID: workflowID, UpdateName: \"dandori.answer\", Args: []any{args}, WaitForStage: client.WorkflowUpdateStageCompleted})\n\tif err != nil {\n\t\treturn err\n\t}\n\treturn h.Get(ctx, nil)\n}\n\n");
    c.push_str("// Send sends the workflow an event, by its id and the event's name. The workflow refuses an event\n// it does not wait for now, and a second one: send it again when it waits for it (Status: Events).\nfunc Send(ctx context.Context, c client.Client, workflowID, event string, a CallbackAnswer) error {\n\tif !ddHas(Events, event) {\n\t\treturn fmt.Errorf(\"the workflow has no event %s\", event)\n\t}\n\targs := a.fields()\n\targs[\"event\"] = event\n\th, err := c.UpdateWorkflow(ctx, client.UpdateWorkflowOptions{WorkflowID: workflowID, UpdateName: \"dandori.event\", Args: []any{args}, WaitForStage: client.WorkflowUpdateStageCompleted})\n\tif err != nil {\n\t\treturn err\n\t}\n\treturn h.Get(ctx, nil)\n}\n\n");
    c.push_str("// Where is where a run is: the line of the call or the wait it is at, each case's state (nil before\n// it starts), and the events it waits for now.\ntype Where struct {\n\tAt     *int           `json:\"at\"`\n\tCases  map[string]any `json:\"cases\"`\n\tEvents []string       `json:\"events\"`\n}\n\n");
    c.push_str("// Status is where the run id is (the query dandori.status).\nfunc Status(ctx context.Context, c client.Client, id string) (Where, error) {\n\tvar w Where\n\tv, err := c.QueryWorkflow(ctx, id, \"\", \"dandori.status\")\n\tif err != nil {\n\t\treturn w, err\n\t}\n\treturn w, v.Get(&w)\n}\n\n");
    c.push_str("// History is the history of a run.\ntype History struct {\n\tWorkflowID string\n\tHistory    *historypb.History\n}\n\n");
    c.push_str("// Histories are the histories of the runs of this workflow that query finds, by default (\"\") the\n// ones going on: replay them with new code (worker.go: Replay) before it takes them over.\nfunc Histories(ctx context.Context, c client.Client, query string) ([]History, error) {\n\tif query == \"\" {\n\t\tquery = fmt.Sprintf(\"WorkflowType = '%s' AND ExecutionStatus = 'Running'\", WorkflowType)\n\t}\n\tvar out []History\n\tvar token []byte\n\tfor {\n\t\tlisted, err := c.ListWorkflow(ctx, &workflowservice.ListWorkflowExecutionsRequest{Query: query, NextPageToken: token})\n\t\tif err != nil {\n\t\t\treturn nil, err\n\t\t}\n\t\tfor _, e := range listed.Executions {\n\t\t\th := &historypb.History{}\n\t\t\tevents := c.GetWorkflowHistory(ctx, e.Execution.WorkflowId, e.Execution.RunId, false, enumspb.HISTORY_EVENT_FILTER_TYPE_ALL_EVENT)\n\t\t\tfor events.HasNext() {\n\t\t\t\tev, err := events.Next()\n\t\t\t\tif err != nil {\n\t\t\t\t\treturn nil, err\n\t\t\t\t}\n\t\t\t\th.Events = append(h.Events, ev)\n\t\t\t}\n\t\t\tout = append(out, History{WorkflowID: e.Execution.WorkflowId, History: h})\n\t\t}\n\t\ttoken = listed.NextPageToken\n\t\tif len(token) == 0 {\n\t\t\treturn out, nil\n\t\t}\n\t}\n}\n");
    if let Some(s) = &m.service {
        c.push_str(&service_client_go(m, n, s));
    }
    c
}

/// client.go, for a workflow that implements a service: the service's name, a type for each
/// message by the name the `.proto` gives it, and a function for each method, which calls the
/// client's own Start, Send, Answer or Status. The names are those of client.ts, as Go exports them.
fn service_client_go(m: &Model, n: &Names, s: &ServiceUse) -> String {
    use crate::temporal::{client_name, Named};
    const FUNCTIONS: &[&str] = &["Start", "Answer", "Send", "Status", "Histories"];
    let mut c = String::new();
    c.push_str(&format!("\n// Service is the service the workflow implements, which `{}` describes. Its methods are the functions below.\nconst Service = {}\n", s.file, q(&s.name)));
    let mut written: Vec<String> = Vec::new();
    let others: Vec<String> = GO_EXPORTED.iter().map(|x| x.to_string()).chain(n.types.values().cloned()).chain(n.types.values().map(|t| format!("Is{t}"))).chain(n.types.values().map(|t| format!("{t}Values"))).collect();
    let alias = |c: &mut String, written: &mut Vec<String>, message: &str, ty: &str, same: bool, what: String| -> String {
        let name = exported(message.rsplit('.').next().unwrap_or(message));
        let taken: Vec<&str> = others.iter().map(|x| x.as_str()).chain(FUNCTIONS.iter().copied()).chain(written.iter().map(|x| x.as_str())).collect();
        match client_name(name, &taken, same, "RPC") {
            Named::Same => ty.to_string(),
            Named::New(t) => {
                c.push_str(&format!("\n// {t} is {what}: `{message}`.\ntype {t} = {ty}\n"));
                written.push(t.clone());
                t
            }
        }
    };
    let task_type = |task: &str| m.tasks.iter().find(|t| t.name == task).and_then(|t| t.result.as_ref()).map(|t| go_type(m, n, t)).unwrap_or_else(|| "any".into());
    for mt in &s.methods {
        let label = s.label(mt);
        let base = exported(&mt.name);
        let taken: Vec<String> = others.iter().cloned().chain(written.iter().cloned()).collect();
        let named = |same: bool| match client_name(base.clone(), FUNCTIONS, same, "RPC") {
            Named::New(x) if taken.contains(&x) => Named::New(format!("{x}RPC")),
            other => other,
        };
        let (f, sig, call, what) = match mt.marks.first() {
            Some(Mark::Start { .. }) => {
                let f = named(true);
                let req = alias(&mut c, &mut written, &mt.request, "WorkflowInput", false, format!("the request of {label}, the workflow's input"));
                alias(&mut c, &mut written, &mt.response, "WorkflowOutput", false, format!("the response of {label}, the workflow's output"));
                (
                    f,
                    format!("(ctx context.Context, c client.Client, id string, input {req}, searchAttributes bool) (client.WorkflowRun, error)"),
                    "\treturn Start(ctx, c, id, input, searchAttributes)\n".to_string(),
                    "starts the workflow as id, as Start does".to_string(),
                )
            }
            Some(Mark::Event { task }) => {
                let f = named(false);
                let req = alias(&mut c, &mut written, &mt.request, &task_type(task), false, format!("the request of {label}, the value of the event of `{task}`"));
                (
                    f,
                    format!("(ctx context.Context, c client.Client, workflowID string, value {req}) error"),
                    format!("\treturn Send(ctx, c, workflowID, {}, CallbackAnswer{{OK: value}})\n", q(task)),
                    format!("sends the workflow workflowID the event of `{task}`, as Send does"),
                )
            }
            Some(Mark::Answer { task }) => {
                let f = named(false);
                let req = alias(&mut c, &mut written, &mt.request, &task_type(task), false, format!("the request of {label}, the answer of the callback of `{task}`"));
                (
                    f,
                    format!("(ctx context.Context, c client.Client, callbackID string, value {req}) error"),
                    "\treturn Answer(ctx, c, callbackID, CallbackAnswer{OK: value})\n".to_string(),
                    format!("answers the callback of `{task}`, by the id its task handed on, as Answer does"),
                )
            }
            Some(Mark::Status) => {
                let f = named(true);
                let res = alias(&mut c, &mut written, &mt.response, "Where", true, format!("the response of {label}, where a run is"));
                (f, format!("(ctx context.Context, c client.Client, id string) ({res}, error)"), "\treturn Status(ctx, c, id)\n".to_string(), "is where the run id is, as Status says".to_string())
            }
            None => continue,
        };
        if let Named::New(name) = f {
            c.push_str(&format!("\n// {name} ({label}) {what}.\nfunc {name}{sig} {{\n{call}}}\n"));
            written.push(name);
        }
    }
    c
}

/// worker.go: the worker that runs the workflow, its tasks and its rules.
fn worker_file(m: &Model, pkg: &str, header: &str, rules: bool, build: &str) -> String {
    let mut w = header.to_string();
    w.push_str(&format!("// The worker of {} v{}: the workflow, the tasks dandori writes and the ones you write, and the rules.\n\npackage {pkg}\n\n", m.name, m.version));
    w.push_str("import (\n\t\"go.temporal.io/sdk/activity\"\n\t\"go.temporal.io/sdk/client\"\n\t\"go.temporal.io/sdk/worker\"\n\t\"go.temporal.io/sdk/workflow\"\n)\n\n");
    w.push_str(&format!("// BuildID is a hash of the code dandori wrote: it changes whenever the code does. The build id of\n// Worker Deployment Versioning.\nconst BuildID = {}\n\n", q(build)));
    w.push_str("// RegisterWorkflow registers the workflow, as WorkflowType.\nfunc RegisterWorkflow(r worker.WorkflowRegistry) {\n\tr.RegisterWorkflowWithOptions(Workflow, workflow.RegisterOptions{Name: WorkflowType})\n}\n\n");
    w.push_str("// RegisterActivities registers your tasks, the ones dandori writes, and the rules, each by the name the\n// workflow calls it by. A nil Transport is the default one.\nfunc RegisterActivities(r worker.ActivityRegistry, own OwnTasks, t Transport) {\n\tfor name, f := range Activities(own, t) {\n\t\tr.RegisterActivityWithOptions(f, activity.RegisterOptions{Name: name})\n\t}\n");
    if rules {
        w.push_str("\tfor name, f := range ddRules {\n\t\tr.RegisterActivityWithOptions(f, activity.RegisterOptions{Name: name})\n\t}\n");
    }
    w.push_str("}\n\n");
    w.push_str("// WorkerOptions are the worker's options. With deployment, the worker is a version of that deployment\n// (Worker Deployment Versioning), and a run stays on the build it started on (PINNED), so the code\n// dandori writes anew never replays an old run.\nfunc WorkerOptions(deployment string) worker.Options {\n\tif deployment == \"\" {\n\t\treturn worker.Options{}\n\t}\n\treturn worker.Options{DeploymentOptions: worker.DeploymentOptions{\n\t\tUseVersioning:             true,\n\t\tVersion:                   worker.WorkerDeploymentVersion{DeploymentName: deployment, BuildID: BuildID},\n\t\tDefaultVersioningBehavior: workflow.VersioningBehaviorPinned,\n\t}}\n}\n\n");
    w.push_str("// NewWorker is the worker on TaskQueue, with the workflow and the activities registered (\"\": no\n// deployment).\nfunc NewWorker(c client.Client, own OwnTasks, t Transport, deployment string) worker.Worker {\n\tw := worker.New(c, TaskQueue, WorkerOptions(deployment))\n\tRegisterWorkflow(w)\n\tRegisterActivities(w, own, t)\n\treturn w\n}\n\n");
    w.push_str("// ReplayFailure is a run that the code finds nondeterministic, with why.\ntype ReplayFailure struct {\n\tWorkflowID string\n\tError      string\n}\n\n");
    w.push_str("// Replay replays histories (client.go: Histories) with this code, and tells the runs it finds\n// nondeterministic, with why: before this code takes over runs that are going on, none may be.\nfunc Replay(histories []History) []ReplayFailure {\n\tvar failed []ReplayFailure\n\tfor _, h := range histories {\n\t\tr := worker.NewWorkflowReplayer()\n\t\tRegisterWorkflow(r)\n\t\t// the run's own id: the code makes keys and the ids of callbacks and child workflows from it\n\t\tif err := r.ReplayWorkflowHistoryWithOptions(nil, h.History, worker.ReplayWorkflowHistoryOptions{OriginalExecution: workflow.Execution{ID: h.WorkflowID}}); err != nil {\n\t\t\tfailed = append(failed, ReplayFailure{WorkflowID: h.WorkflowID, Error: err.Error()})\n\t\t}\n\t}\n\treturn failed\n}\n");
    w
}

/// doc.go: what the package is, and what it is written against.
fn doc_file(m: &Model, nd: &Needs, pkg: &str, header: &str, called: &BTreeSet<usize>) -> String {
    let mut modules = vec![TEMPORAL_SDK.to_string()];
    if nd.lambda || !nd.aws.is_empty() {
        modules.push(AWS_SDK.to_string());
    }
    if nd.openai {
        modules.push(OPENAI_CLIENT.to_string());
    }
    if nd.claude {
        modules.push(ANTHROPIC_SDK.to_string());
    }
    let mut d = header.to_string();
    d.push_str(&format!("// Package {pkg} is the workflow {} v{} of {} for Temporal's Go SDK{}.\n", m.name, m.version, m.source_file, if m.description.is_empty() { String::new() } else { format!(": {}", m.description) }));
    d.push_str("//\n// Workflow is the workflow (workflow.go), and NewWorker its worker, with the tasks you write\n// (OwnTasks); Start, Answer, Send and Status are its client (client.go).\n//\n");
    d.push_str(&format!("// It is written against {}; add them to your module with go get.\n", modules.join(", ")));
    if !called.is_empty() {
        d.push_str("// The rules it calls are rulec's Go: see rules.go.\n");
    }
    d.push_str(&format!("package {pkg}\n"));
    d
}

pub fn build(m: &Model) -> Result<Vec<(String, String)>, Vec<Diag>> {
    if let Some(d) = crate::check::history_limit(m, Platform::Temporal) {
        return Err(vec![d]);
    }
    let pkg = package(m);
    let header = format!("// Code generated by dandori from {}. DO NOT EDIT.\n\n", m.source_file);
    let names = Names::new(m);
    let nd = needs(m);
    // the rules whose code goes with the workflow are in rules.go; the others are called at their services, from activities.go
    let called: BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } if m.rules[*r].connect.is_none() => Some(*r), _ => None }).collect();
    let constant = |text: &str| format!("{header}{}", text.replace("{{PKG}}", &pkg));
    let mut g = Gen { m, names: &names, out: String::new(), loops: vec![], can: None, time: false };
    let wf = g.workflow(&header, &pkg);
    let mut files = vec![
        (format!("{pkg}/doc.go"), doc_file(m, &nd, &pkg, &header, &called)),
        (format!("{pkg}/types.go"), types_file(m, &names, &pkg, &header)),
        (format!("{pkg}/values.go"), constant(VALUES)),
        (format!("{pkg}/activities.go"), activities_file(m, &names, &pkg, &header)),
        (format!("{pkg}/io.go"), io_file(&nd, &pkg, &header)),
    ];
    if nd.lambda || !nd.aws.is_empty() {
        files.push((format!("{pkg}/io_aws.go"), io_aws_file(&nd, &pkg, &header)));
    }
    if nd.openai {
        files.push((format!("{pkg}/io_openai.go"), constant(IO_OPENAI)));
    }
    if nd.claude {
        files.push((format!("{pkg}/io_claude.go"), constant(IO_CLAUDE)));
    }
    files.push((format!("{pkg}/runtime.go"), constant(RUNTIME)));
    files.push((format!("{pkg}/workflow.go"), wf));
    if !called.is_empty() {
        files.push((format!("{pkg}/rules.go"), rules_file(m, &pkg, &header, &called)));
    }
    files.push((format!("{pkg}/client.go"), client_file(m, &names, &pkg, &header)));
    let mut files: Vec<(String, String)> = files.into_iter().map(|(name, text)| (name, gofmt_columns(&text))).collect();
    let id = crate::temporal::build_id(&files);
    files.push((format!("{pkg}/worker.go"), gofmt_columns(&worker_file(m, &pkg, &header, !called.is_empty(), &id))));
    Ok(files)
}

/// The retriers of a call, as the list ddAttempt takes: the ASL's, by the `.flow`'s kinds.
fn retriers(m: &Model, callee: &Callee) -> String {
    let v: Value = match callee {
        Callee::Rule(_) => crate::asl::rule_retriers(),
        Callee::Task(t) => match &m.tasks[*t].retry {
            Some(r) => crate::asl::retriers(m, callee, &m.tasks[*t], r),
            None => serde_json::json!([]),
        },
    };
    let mut out = Vec::new();
    for r in v.as_array().unwrap() {
        let kinds: Vec<String> = r["ErrorEquals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| {
                let n = x.as_str().unwrap();
                if n == "States.ALL" {
                    "*".to_string()
                } else if n == "States.Timeout" {
                    "timeout".to_string()
                } else {
                    match callee {
                        Callee::Task(t) => m.tasks[*t]
                            .errors
                            .iter()
                            .find(|e| render::asl_error(m, callee, &HErr::Declared(e.name.clone())).iter().any(|a| a == n))
                            .map(|e| e.name.clone())
                            .unwrap_or_else(|| n.to_string()),
                        Callee::Rule(_) => n.to_string(),
                    }
                }
            })
            .collect();
        out.push(format!(
            "{{On: []string{{{}}}, Max: {}, Every: {}, Backoff: {}}}",
            kinds.iter().map(|k| q(k)).collect::<Vec<_>>().join(", "),
            r["MaxAttempts"].as_u64().unwrap_or(3),
            r["IntervalSeconds"].as_u64().unwrap_or(1),
            r["BackoffRate"].as_f64().unwrap_or(2.0)
        ));
    }
    if out.is_empty() {
        "nil".into()
    } else {
        format!("[]ddRetrier{{{}}}", out.join(", "))
    }
}

struct Gen<'a> {
    m: &'a Model,
    /// the Go names of the records and enums
    names: &'a Names,
    out: String,
    /// the counters of the loops around the current statement
    loops: Vec<usize>,
    /// the loop at the top of the flow that the next statement is, which goes on in a new run
    /// once the history is long, by its place among such loops (the first is 1)
    can: Option<usize>,
    /// whether workflow.go uses the package time
    time: bool,
}

impl<'a> Gen<'a> {
    fn line(&mut self, depth: usize, s: &str) {
        if !s.is_empty() {
            for _ in 0..depth {
                self.out.push('\t');
            }
            self.out.push_str(s);
        }
        self.out.push('\n');
    }

    fn var(&self, name: &str) -> String {
        go_name(name)
    }

    fn expr(&self, e: &TExpr) -> String {
        match e {
            TExpr::Str(s) => q(s),
            TExpr::Int(n) => n.to_string(),
            TExpr::Bool(b) => b.to_string(),
            TExpr::Enum(v, _) => q(v),
            TExpr::None(_) => "nil".into(),
            TExpr::Var { name, fields, .. } => {
                if fields.is_empty() {
                    self.var(name)
                } else {
                    // a field that is not there, or of a case that has not started, reads as nil
                    format!("ddGet({}, {})", self.var(name), fields.iter().map(|f| q(f)).collect::<Vec<_>>().join(", "))
                }
            }
            TExpr::Record { fields, .. } => format!("map[string]any{{{}}}", fields.iter().map(|(f, x)| format!("{}: {}", q(f), self.expr(x))).collect::<Vec<_>>().join(", ")),
            TExpr::List { items, .. } => format!("[]any{{{}}}", items.iter().map(|x| self.expr(x)).collect::<Vec<_>>().join(", ")),
            TExpr::Interp(parts) => {
                let out: Vec<String> = parts
                    .iter()
                    .map(|p| match p {
                        IPart::Lit(s) => q(s),
                        IPart::Hole(x) => format!("ddText({})", self.expr(x)),
                    })
                    .collect();
                if out.is_empty() {
                    "\"\"".into()
                } else {
                    format!("({})", out.join(" + "))
                }
            }
        }
    }

    fn rounds_expr(&self) -> String {
        if self.loops.is_empty() {
            return "nil".into();
        }
        let rounds: Vec<String> = self.loops.iter().map(|l| format!("dd_loop_{l}")).collect();
        format!("[]int{{{}}}", rounds.join(", "))
    }

    fn workflow(&mut self, header: &str, pkg: &str) -> String {
        let m = self.m;
        let mut body = String::new();
        std::mem::swap(&mut self.out, &mut body);
        // the activities: one function a task, so each has its own timeout and queue; Temporal does
        // not retry, the workflow does; the worker of the workflow heartbeats from the tasks it serves
        for t in &m.tasks {
            if t.is_child(Platform::Temporal) || t.event {
                continue;
            }
            self.time = true;
            let timeout = crate::temporal::activity_timeout(t);
            let rest = match &t.queue {
                Some(qn) => format!("TaskQueue: {}", q(qn)),
                None => format!("HeartbeatTimeout: {} * time.Second", crate::temporal::HEARTBEAT_SECONDS),
            };
            self.out.push_str(&format!(
                "\n// {} calls the activity of the task {}.\nfunc {}(ctx workflow.Context, args map[string]any) (any, error) {{\n\tctx = workflow.WithActivityOptions(ctx, workflow.ActivityOptions{{StartToCloseTimeout: {timeout} * time.Second, {rest}, RetryPolicy: ddNoRetry}})\n\tvar r any\n\terr := workflow.ExecuteActivity(ctx, {}, args).Get(ctx, &r)\n\treturn r, err\n}}\n",
                task_function(&t.name),
                t.name,
                task_function(&t.name),
                q(&ident(&t.name))
            ));
        }
        let called: BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
        // every rule is an activity the workflow calls by name, whether its code goes with the workflow or it is called at its service
        if called.iter().any(|r| !m.rules[*r].local) {
            self.time = true;
            self.out.push_str(&format!(
                "\n// ddRuleActivity calls a rule's activity, by its name.\nfunc ddRuleActivity(ctx workflow.Context, name string, args map[string]any) (any, error) {{\n\tctx = workflow.WithActivityOptions(ctx, workflow.ActivityOptions{{StartToCloseTimeout: {} * time.Second, RetryPolicy: ddNoRetry}})\n\tvar r any\n\terr := workflow.ExecuteActivity(ctx, name, args).Get(ctx, &r)\n\treturn r, err\n}}\n",
                crate::temporal::RULE_SECONDS
            ));
        }
        if called.iter().any(|r| m.rules[*r].local) {
            self.time = true;
            self.out.push_str(&format!(
                "\n// ddLocalRuleActivity calls a rule that says `local`: in the worker that runs the workflow; the\n// history keeps each answer as a marker.\nfunc ddLocalRuleActivity(ctx workflow.Context, name string, args map[string]any) (any, error) {{\n\tctx = workflow.WithLocalActivityOptions(ctx, workflow.LocalActivityOptions{{StartToCloseTimeout: {} * time.Second, RetryPolicy: ddNoRetry}})\n\tvar r any\n\terr := workflow.ExecuteLocalActivity(ctx, name, args).Get(ctx, &r)\n\treturn r, err\n}}\n",
                crate::temporal::RULE_SECONDS
            ));
        }
        let resume = if self.resumes() { ", resume any" } else { "" };
        let cases: Vec<String> = m.cases.iter().map(|c| q(&c.name)).collect();
        self.out.push_str(&format!(
            "\n// Workflow runs {} v{}. RegisterWorkflow registers it as WorkflowType.\nfunc Workflow(ctx workflow.Context, input any{resume}) (any, error) {{\n\tdd := ddNew(ctx{})\n",
            m.name,
            m.version,
            cases.iter().map(|c| format!(", {c}")).collect::<String>()
        ));
        if m.tasks.iter().any(|t| t.callback) {
            self.out.push_str("\tdd.listenForAnswers(ctx)\n");
        }
        if m.tasks.iter().any(|t| t.event) {
            self.out.push_str("\tdd.listenForEvents(ctx)\n");
        }
        self.out.push_str(&format!("\treturn ddEnd(ddFlow(ctx, dd, input{}))\n}}\n", if self.resumes() { ", resume" } else { "" }));
        self.out.push_str(&format!("\nfunc ddFlow(ctx workflow.Context, dd *ddRun, input any{}) (any, error) {{\n", if self.resumes() { ", dd_resume any" } else { "" }));
        self.body();
        self.out.push_str("}\n");
        std::mem::swap(&mut self.out, &mut body);
        let mut w = header.to_string();
        w.push_str(&format!("// {} v{}{}\n\npackage {pkg}\n\n", m.name, m.version, if m.description.is_empty() { String::new() } else { format!(": {}", m.description) }));
        if self.time {
            w.push_str("import (\n\t\"time\"\n\n\t\"go.temporal.io/sdk/workflow\"\n)\n");
        } else {
            w.push_str("import \"go.temporal.io/sdk/workflow\"\n");
        }
        w.push_str(&body);
        w
    }

    /// The body of ddFlow: the input's check, the variables, the flow, `on failure` and `on cancel`.
    fn body(&mut self) {
        let m = self.m;
        let d = 1;
        if let Some(s) = &m.service {
            // the input is the request of the service's method that starts a run, read as protobuf reads it
            self.line(d, &format!("input = ddFill(input, {})", json_value(&s.input_zeros)));
        }
        self.line(d, "if !IsWorkflowInput(input) {");
        self.line(d + 1, "return nil, ddFail(\"Dandori.BadInput\", \"the execution's input does not have the declared shape\")");
        self.line(d, "}");
        let locals = crate::asl::parallel_locals(m);
        for (v, _) in &m.vars {
            if m.inputs.iter().any(|(i, _)| i == v) {
                self.line(d, &format!("var {} any = ddGet(input, {})", self.var(v), q(v)));
                self.line(d, &format!("_ = {}", self.var(v)));
            } else if !locals.contains(v) {
                self.line(d, &format!("var {} any", self.var(v)));
                self.line(d, &format!("_ = {}", self.var(v)));
            }
        }
        if self.resumes() {
            let carried: Vec<&String> = m.vars.iter().map(|(v, _)| v).filter(|v| !m.inputs.iter().any(|(i, _)| i == *v) && !locals.contains(*v)).collect();
            self.line(d, "// a run that goes on from an earlier one (Continue-As-New) takes up its variables, and starts at its loop");
            self.line(d, "dd_from := 0");
            self.line(d, "if dd_resume != nil {");
            self.line(d + 1, "dd_from = ddInt(ddGet(dd_resume, \"at\"))");
            for v in &carried {
                self.line(d + 1, &format!("{} = ddGet(dd_resume, \"vars\", {})", self.var(v), q(v)));
            }
            self.line(d, "}");
            let vars: Vec<String> = carried.iter().map(|v| format!("{}: {}", q(v), self.var(v))).collect();
            self.line(d, "dd_vars := func() map[string]any {");
            self.line(d + 1, &format!("return map[string]any{{{}}}", vars.join(", ")));
            self.line(d, "}");
        }
        // what the query dandori.status and the search attribute say of the cases
        let cases: Vec<String> = m.cases.iter().map(|c| format!("{}: ddGet({}, {})", q(&c.name), self.var(&c.name), q(&c.state_field))).collect();
        if !cases.is_empty() {
            self.line(d, "dd.report(func() map[string]any {");
            self.line(d + 1, &format!("return map[string]any{{{}}}", cases.join(", ")));
            self.line(d, "})");
        }
        // the flow's end without outputs (the checker sees that a flow with outputs ends with `succeed`),
        // which a workflow that implements a service answers as an object
        let end = self.no_output();
        let flow = |g: &mut Gen, d: usize| {
            let terminated = g.flow(d);
            if !terminated {
                g.line(d, &end);
            }
        };
        match (&m.on_cancel, &m.on_failure) {
            (None, None) => flow(self, d),
            (None, Some(block)) => {
                self.failing(d, block, &flow);
            }
            (Some(cancel), failure) => {
                // a cancellation reaches past `on failure` to `on cancel`
                self.line(d, "out, err := func() (any, error) {");
                match failure {
                    Some(block) => self.failing(d + 1, block, &flow),
                    None => flow(self, d + 1),
                }
                self.line(d, "}()");
                self.line(d, "if ddCancelled(err) {");
                self.line(d + 1, "// on cancel: out of the cancellation's reach; then the workflow ends as cancelled");
                self.line(d + 1, "ctx, _ := workflow.NewDisconnectedContext(ctx)");
                self.line(d + 1, "_ = ctx");
                if !self.block(cancel, d + 1) {
                    self.line(d + 1, "return nil, err");
                }
                self.line(d, "}");
                self.line(d, "return out, err");
            }
        }
    }

    /// The flow, and `on failure`, which runs for a task's error that nothing handled, and not for
    /// `fail` or a failed check.
    fn failing(&mut self, d: usize, block: &[TStmt], flow: &dyn Fn(&mut Gen, usize)) {
        self.line(d, "out, err := func() (any, error) {");
        flow(self, d + 1);
        self.line(d, "}()");
        self.line(d, "if ddKind(err) != \"\" {");
        self.line(d + 1, "// on failure");
        if !self.block(block, d + 1) {
            self.line(d + 1, "return nil, err");
        }
        self.line(d, "}");
        self.line(d, "return out, err");
    }

    /// How the flow returns when it ends without outputs: none, but `{}` for a workflow that
    /// implements a service, whose response protobuf's JSON reads from an object.
    fn no_output(&self) -> String {
        if self.m.service.is_some() { "return map[string]any{}, nil".into() } else { "return nil, nil".into() }
    }

    /// The statements of a block, up to the first that ends it; whether it ends the block as Go
    /// sees it (a return), so that nothing after it is written.
    fn block(&mut self, ss: &[TStmt], d: usize) -> bool {
        for s in ss {
            let ended = self.stmt(s, d);
            if ended {
                return true;
            }
            if matches!(s.kind, TK::Break) {
                return false;
            }
        }
        false
    }

    /// Whether a loop at the top of the flow goes on in a new run once the history is long.
    fn resumes(&self) -> bool {
        self.m.flow.iter().any(crate::check::continues_as_new)
    }

    /// The flow's statements. A loop at the top of the flow goes on in a new run once the history
    /// is long (Continue-As-New); a run that goes on so starts at that loop, the `dd_from`-th
    /// such loop, and skips what comes before it. Whether the flow ends with a return.
    fn flow(&mut self, d: usize) -> bool {
        let ss = &self.m.flow;
        if !self.resumes() {
            return self.block(ss, d);
        }
        let total = ss.iter().filter(|s| crate::check::continues_as_new(s)).count();
        let mut k = 0;
        // the statements under the `if` that is open run when dd_from is at most this
        let mut open: Option<usize> = None;
        for s in ss {
            let can = crate::check::continues_as_new(s);
            let guard = if can { Some(k + 1) } else if k < total { Some(k) } else { None };
            if open != guard {
                if open.is_some() {
                    self.line(d, "}");
                }
                match guard {
                    Some(0) => self.line(d, "if dd_resume == nil {"),
                    Some(g) => self.line(d, &format!("if dd_from <= {g} {{")),
                    None => {}
                }
                open = guard;
            }
            if can {
                k += 1;
                self.can = Some(k);
            }
            let ended = self.stmt(s, if open.is_some() { d + 1 } else { d });
            if ended || matches!(s.kind, TK::Break) {
                if open.is_some() {
                    self.line(d, "}");
                    // the return is under an if: the flow goes on past it as Go sees it
                    return false;
                }
                return ended;
            }
        }
        if open.is_some() {
            self.line(d, "}");
        }
        false
    }

    /// At the start of a round of the `k`-th loop at the top of the flow but its first in this
    /// run: once the history is long, go on in a new run, with the variables, the round, and
    /// `more` (the loop's list, and what it has yielded).
    fn continue_as_new(&mut self, d: usize, site: usize, k: usize, more: &str) {
        self.line(d, &format!("if dd_loop_{site} > dd_first_{site} && ddHistoryIsLong(ctx) {{"));
        self.line(d + 1, &format!("return nil, workflow.NewContinueAsNewError(ctx, WorkflowType, input, map[string]any{{\"at\": {k}, \"round\": dd_loop_{site}, \"vars\": dd_vars(){more}}})"));
        self.line(d, "}");
    }

    /// A statement; whether it ends the block as Go sees it.
    fn stmt(&mut self, s: &TStmt, d: usize) -> bool {
        match &s.kind {
            TK::Pass => false,
            TK::Break => {
                self.line(d, "break");
                false
            }
            TK::Wait { seconds } => {
                self.line(d, &format!("dd.at = {}", s.line));
                self.line(d, &format!("if err := workflow.Sleep(ctx, ddSeconds({seconds})); err != nil {{"));
                self.line(d + 1, "return nil, err");
                self.line(d, "}");
                false
            }
            TK::WaitUntil { at } => {
                let x = self.expr(at);
                self.line(d, &format!("dd.at = {}", s.line));
                self.line(d, &format!("if err := ddWaitUntil(ctx, {x}); err != nil {{"));
                self.line(d + 1, "return nil, err");
                self.line(d, "}");
                false
            }
            TK::Assign { name, expr } => {
                let x = self.expr(expr);
                self.line(d, &format!("{} = {x}", self.var(name)));
                false
            }
            TK::Succeed { fields } => {
                if fields.is_empty() {
                    let end = self.no_output();
                    self.line(d, &end);
                } else {
                    let parts: Vec<String> = fields.iter().map(|(f, e)| format!("{}: {}", q(f), self.expr(e))).collect();
                    self.line(d, &format!("return map[string]any{{{}}}, nil", parts.join(", ")));
                }
                true
            }
            TK::Fail { error, cause, .. } => {
                let c = cause.as_ref().map(|c| self.expr(c)).unwrap_or_else(|| "nil".into());
                self.line(d, &format!("return nil, ddFail({}, {c})", q(error)));
                true
            }
            TK::Repeat { times, body } => {
                let site = s.site;
                self.line(d, "{");
                match self.can.take() {
                    Some(k) => {
                        self.line(d + 1, &format!("// line {}: a round starts in a new run once the history is long", s.line));
                        self.line(d + 1, &format!("dd_first_{site} := ddInt(ddCarried(dd_resume, {k}, \"round\", func() any {{ return 0 }}))"));
                        self.line(d + 1, &format!("for dd_loop_{site} := dd_first_{site}; dd_loop_{site} < {times}; dd_loop_{site}++ {{"));
                        self.continue_as_new(d + 2, site, k, "");
                    }
                    None => self.line(d + 1, &format!("for dd_loop_{site} := 0; dd_loop_{site} < {times}; dd_loop_{site}++ {{")),
                }
                self.loops.push(site);
                self.block(body, d + 2);
                self.loops.pop();
                self.line(d + 1, "}");
                self.line(d, "}");
                false
            }
            TK::For { var, list, max, parallel, body, result, locals } => {
                let site = s.site;
                let can = self.can.take();
                let items = format!("dd_items_{site}");
                self.line(d, "{");
                self.line(d + 1, &format!("// line {}: for {var} in {}", s.line, list.show()));
                let lx = self.expr(list);
                match can {
                    // a run that goes on in the loop takes up the list the loop began with
                    Some(k) => {
                        self.line(d + 1, &format!("{items} := ddList(ddCarried(dd_resume, {k}, \"items\", func() any {{"));
                        self.line(d + 2, &format!("return {lx}"));
                        self.line(d + 1, "}))");
                    }
                    None => self.line(d + 1, &format!("{items} := ddList({lx})")),
                }
                self.line(d + 1, &format!("if len({items}) > {max} {{"));
                self.line(d + 2, &format!("return nil, ddFail(\"Dandori.TooManyItems\", {})", q(&format!("line {}: the list has more than {max} items", s.line))));
                self.line(d + 1, "}");
                match parallel {
                    None => {
                        match (result, can) {
                            (Some(_), Some(k)) => self.line(d + 1, &format!("dd_out_{site} := append([]any{{}}, ddList(ddCarried(dd_resume, {k}, \"out\", func() any {{ return []any{{}} }}))...)")),
                            (Some(_), None) => self.line(d + 1, &format!("dd_out_{site} := []any{{}}")),
                            (None, _) => {}
                        }
                        match can {
                            Some(k) => {
                                self.line(d + 1, "// a round starts in a new run once the history is long");
                                self.line(d + 1, &format!("dd_first_{site} := ddInt(ddCarried(dd_resume, {k}, \"round\", func() any {{ return 0 }}))"));
                                self.line(d + 1, &format!("for dd_loop_{site} := dd_first_{site}; dd_loop_{site} < len({items}); dd_loop_{site}++ {{"));
                                let more = if result.is_some() { format!(", \"items\": {items}, \"out\": dd_out_{site}") } else { format!(", \"items\": {items}") };
                                self.continue_as_new(d + 2, site, k, &more);
                            }
                            None => self.line(d + 1, &format!("for dd_loop_{site} := 0; dd_loop_{site} < len({items}); dd_loop_{site}++ {{")),
                        }
                        self.line(d + 2, &format!("{} = {items}[dd_loop_{site}]", self.var(var)));
                        self.loops.push(site);
                        let ended = self.block(body, d + 2);
                        let breaks = body.last().map(|x| matches!(x.kind, TK::Break)).unwrap_or(false);
                        if let (Some((_, y)), false, false) = (result, ended, breaks) {
                            let yv = self.expr(y);
                            self.line(d + 2, &format!("dd_out_{site} = append(dd_out_{site}, {yv})"));
                        }
                        self.loops.pop();
                        self.line(d + 1, "}");
                        if let Some((r, _)) = result {
                            self.line(d + 1, &format!("{} = dd_out_{site}", self.var(r)));
                        }
                    }
                    Some(k) => {
                        self.line(d + 1, &format!("dd_res_{site}, err := ddRounds(ctx, {items}, ddAtATime({k}), func(ctx workflow.Context, dd_item any, dd_loop_{site} int) (any, error) {{"));
                        self.line(d + 2, &format!("var {} any = dd_item", self.var(var)));
                        self.line(d + 2, &format!("_ = {}", self.var(var)));
                        for l in locals {
                            if l != var {
                                self.line(d + 2, &format!("var {} any", self.var(l)));
                                self.line(d + 2, &format!("_ = {}", self.var(l)));
                            }
                        }
                        self.loops.push(site);
                        let ended = self.block(body, d + 2);
                        self.loops.pop();
                        if !ended {
                            let y = result.as_ref().map(|(_, y)| self.expr(y)).unwrap_or_else(|| "nil".into());
                            self.line(d + 2, &format!("return {y}, nil"));
                        }
                        self.line(d + 1, "})");
                        self.line(d + 1, "if err != nil {");
                        self.line(d + 2, "return nil, err");
                        self.line(d + 1, "}");
                        match result {
                            Some((r, _)) => self.line(d + 1, &format!("{} = dd_res_{site}", self.var(r))),
                            None => self.line(d + 1, &format!("_ = dd_res_{site}")),
                        }
                    }
                }
                self.line(d, "}");
                false
            }
            TK::Match { expr, arms } => {
                let x = self.expr(expr);
                self.line(d, "{");
                self.line(d + 1, &format!("dd_v := {x}"));
                let mut all_end = true;
                for (i, a) in arms.iter().enumerate() {
                    let mut parts = Vec::new();
                    if a.none {
                        parts.push("dd_v == nil".to_string());
                    }
                    if a.some.is_some() {
                        parts.push("dd_v != nil".to_string());
                    }
                    if !a.values.is_empty() {
                        if expr.ty().inner() == &Ty::Bool {
                            for v in &a.values {
                                parts.push(format!("dd_v == {}", if v == "true" { "true" } else { "false" }));
                            }
                        } else {
                            parts.push(format!("ddIsOneOf(dd_v, {})", a.values.iter().map(|v| q(v)).collect::<Vec<_>>().join(", ")));
                        }
                    }
                    let kw = if i == 0 { "if" } else { "} else if" };
                    self.line(d + 1, &format!("{kw} {} {{", parts.join(" || ")));
                    if let Some(v) = &a.some {
                        self.line(d + 2, &format!("{} = dd_v", self.var(v)));
                    }
                    if !self.block(&a.body, d + 2) {
                        all_end = false;
                    }
                }
                self.line(d + 1, "} else {");
                self.line(d + 2, &format!("return nil, ddFail(\"Dandori.UnexpectedValue\", {})", q(&format!("line {}: {} took a value that no arm names", s.line, expr.show()))));
                self.line(d + 1, "}");
                self.line(d, "}");
                all_end
            }
            TK::Call { target, callee, args, handlers } => self.call(s, target.as_ref(), callee, args, handlers, d),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn call(&mut self, s: &TStmt, target: Option<&Target>, callee: &Callee, args: &[(String, TExpr)], handlers: &[THandler], d: usize) -> bool {
        let m = self.m;
        let site = s.site;
        let p = Platform::Temporal;
        let (cname, declared, result_ty) = match callee {
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                (task.name.clone(), task.errors.iter().map(|e| q(&e.name)).collect::<Vec<_>>(), task.result.clone())
            }
            Callee::Rule(r) => (m.rules[*r].name.clone(), vec![], Some(Ty::Record(m.rules[*r].outputs))),
        };
        let mut parts: Vec<String> = args.iter().map(|(a, e)| format!("{}: {}", q(a), self.expr(e))).collect();
        if let Callee::Task(t) = callee {
            if m.tasks[*t].key {
                parts.push(format!("\"idempotency_key\": ddKey(ctx, {site}, {})", self.rounds_expr()));
            }
        }
        let args_map = format!("map[string]any{{{}}}", parts.join(", "));
        let retriers = retriers(m, callee);
        let declared = if declared.is_empty() { "nil".to_string() } else { format!("[]string{{{}}}", declared.join(", ")) };
        self.line(d, "{");
        self.line(d + 1, &format!("// line {}: {cname}", s.line));
        self.line(d + 1, &format!("dd.at = {}", s.line));
        // the call, as the lines of a function of ctx that answers (any, error)
        let mut invocation: Vec<String> = Vec::new();
        let mut counted = false;
        match callee {
            Callee::Rule(r) => {
                let call = if m.rules[*r].local { "ddLocalRuleActivity" } else { "ddRuleActivity" };
                invocation.push(format!("return {call}(ctx, {}, {args_map})", q(&render::rule_activity(&m.rules[*r].name))));
            }
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                match task.via(p) {
                    Some(Via::Workflow(ty)) => {
                        counted = true;
                        let mut opts = vec![format!("WorkflowID: ddChildID(ctx, {site}, {}, dd_n)", self.rounds_expr())];
                        if let Some(qn) = &task.queue {
                            opts.push(format!("TaskQueue: {}", q(qn)));
                        }
                        if let Some(t) = task.timeout {
                            self.time = true;
                            opts.push(format!("WorkflowExecutionTimeout: {t} * time.Second"));
                        }
                        invocation.push("dd_n++".into());
                        invocation.push(format!("ctx = workflow.WithChildOptions(ctx, workflow.ChildWorkflowOptions{{{}}})", opts.join(", ")));
                        invocation.push("var r any".into());
                        invocation.push(format!("err := workflow.ExecuteChildWorkflow(ctx, {}, {args_map}).Get(ctx, &r)", q(ty)));
                        invocation.push("return r, err".into());
                    }
                    // nothing is called: the workflow waits for the event, as long as a callback
                    Some(Via::Event) => invocation.push(format!("return dd.awaitEvent(ctx, {}, {})", q(&task.name), task.timeout.unwrap_or(86_400))),
                    _ if task.callback => {
                        counted = true;
                        let mut with_id = parts.clone();
                        with_id.push("\"callback_id\": dd_id".into());
                        invocation.push("dd_n++".into());
                        invocation.push(format!("dd_id := dd.callbackID(ctx, {site}, {}, dd_n)", self.rounds_expr()));
                        invocation.push(format!("if _, err := {}(ctx, map[string]any{{{}}}); err != nil {{", task_function(&task.name), with_id.join(", ")));
                        invocation.push("\treturn nil, err".into());
                        invocation.push("}".into());
                        invocation.push(format!("return dd.awaitCallback(ctx, dd_id, {})", task.timeout.unwrap_or(86_400)));
                    }
                    _ => invocation.push(format!("return {}(ctx, {args_map})", task_function(&task.name))),
                }
            }
        }
        if counted {
            self.line(d + 1, "dd_n := 0");
        }
        let var = match target {
            Some(Target::Let(v)) => Some(self.var(v)),
            Some(Target::Case(c)) => Some(self.var(&m.cases[*c].name)),
            None => None,
        };
        let reads = var.is_some() && result_ty.is_some();
        self.line(d + 1, &format!("{}, dd_e := ddAttempt(ctx, func(ctx workflow.Context) (any, error) {{", if reads { "dd_r" } else { "_" }));
        for l in &invocation {
            self.line(d + 2, l);
        }
        self.line(d + 1, &format!("}}, {retriers}, {declared})"));
        // the value of an event or a callback that a method of the service sends, read as protobuf reads it
        let filled = match callee {
            Callee::Task(t) => m.tasks[*t].answer_zeros.as_ref(),
            Callee::Rule(_) => None,
        };
        let answer = |g: &mut Gen, d: usize| {
            // the answer: its declared type, and for a case the states it may carry
            if let (Some(v), Some(ty)) = (&var, &result_ty) {
                if let Some(z) = filled {
                    g.line(d, &format!("dd_r = ddFill(dd_r, {})", json_value(z)));
                }
                g.line(d, &format!("if !{} {{", go_check(m, g.names, "dd_r", ty, m.answer_range(callee), 0)));
                g.line(d + 1, &format!("return nil, ddFail(\"Dandori.BadResponse\", {})", q(&format!("line {}: the answer from {} does not have the declared shape", s.line, cname))));
                g.line(d, "}");
                if let (Some(Target::Case(c)), Some((_, allowed))) = (target, m.monitors.get(&site)) {
                    let field = &m.cases[*c].state_field;
                    g.line(d, &format!("if !ddIsOneOf(ddGet(dd_r, {}), {}) {{", q(field), allowed.iter().map(|v| q(v)).collect::<Vec<_>>().join(", ")));
                    g.line(
                        d + 1,
                        &format!(
                            "return nil, ddFail(\"Dandori.UnexpectedState\", {})",
                            q(&format!("line {}: {} answered with a state the machine does not lead to here (expected one of {})", s.line, cname, allowed.join(", ")))
                        ),
                    );
                    g.line(d, "}");
                }
                g.line(d, &format!("{v} = dd_r"));
                if let Some(Target::Case(_)) = target {
                    g.line(d, "dd.shown(ctx)");
                }
            }
        };
        let mut ended = false;
        if handlers.is_empty() {
            self.line(d + 1, "if dd_e != nil {");
            self.line(d + 2, "return nil, dd_e");
            self.line(d + 1, "}");
            answer(self, d + 1);
        } else {
            self.line(d + 1, "if dd_e != nil {");
            self.line(d + 2, "dd_k := ddKind(dd_e)");
            let mut every_ends = true;
            for (i, h) in handlers.iter().enumerate() {
                let conds: Vec<String> = h
                    .errors
                    .iter()
                    .map(|x| match x {
                        HErr::Failure => "dd_k != \"\"".to_string(),
                        HErr::Timeout => "dd_k == \"timeout\"".to_string(),
                        HErr::Declared(n) => format!("dd_k == {}", q(n)),
                    })
                    .collect();
                let kw = if i == 0 { "if" } else { "} else if" };
                self.line(d + 2, &format!("{kw} {} {{", conds.join(" || ")));
                if !self.block(&h.body, d + 3) {
                    every_ends = false;
                }
            }
            self.line(d + 2, "} else {");
            self.line(d + 3, "return nil, dd_e");
            self.line(d + 2, "}");
            if reads {
                self.line(d + 1, "} else {");
                answer(self, d + 2);
                self.line(d + 1, "}");
            } else {
                self.line(d + 1, "}");
                // with no answer to read, the call ends the block only when every handler and the call's success do
                let _ = every_ends;
            }
            ended = false;
        }
        self.line(d, "}");
        ended
    }
}
