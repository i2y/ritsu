// The host of handles: it takes what people typed, and the generated code of the module says the
// handle kept for each, or why there is none. Written in a directory of its own in the module of the
// package `ritsu gen` writes (go/), with `--module example.com/handles`.
package main

import (
	"fmt"
	"strings"

	"example.com/handles/glue/handles"
	"example.com/handles/glue/kaname"
)

func main() {
	for _, typed := range []string{" Grace Hopper", "9", "Root", "ünïcode"} {
		switch r := handles.Handle(typed).(type) {
		case kaname.Ok[string, []string]:
			fmt.Printf("%q -> %s\n", typed, r.Value)
		case kaname.Err[string, []string]:
			fmt.Printf("%q -> refused: %s\n", typed, strings.Join(r.Error, "; "))
		}
	}
	for _, typed := range []string{"Ada Lovelace#42", "ada#0"} {
		switch r := handles.TagFn(typed).(type) {
		case kaname.Ok[handles.Tag, []string]:
			fmt.Printf("%q -> %s #%d\n", typed, r.Value.Handle, r.Value.Number)
		case kaname.Err[handles.Tag, []string]:
			fmt.Printf("%q -> refused: %s\n", typed, strings.Join(r.Error, "; "))
		}
	}
}
