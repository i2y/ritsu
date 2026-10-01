package checks

import (
	"fmt"
	"os"

	"temporalgo/harness"
)

// Run runs the check the arguments name, and says whether they named one:
//
//	<bin> --wire <key> <cases.json> <results.json> <moto's address>
//	<bin> --agents <key> <cases.json> <results.json>
//	<bin> --connect-read <key> <cases.json> <results.json>
//	<bin> --jev <key> <cases.json> <results.json>
//
// readRule and jev are the package's own readings of a rule's answer and of Jev's, by its key.
func Run(flows map[string]harness.Flow, readRule map[string]func(wire, body any) (any, error), jev map[string]func(body, spec any) (any, error), args []string) bool {
	if len(args) < 1 {
		return false
	}
	need := map[string]int{"--wire": 5, "--agents": 4, "--connect-read": 4, "--jev": 4}
	n, ok := need[args[0]]
	if !ok {
		return false
	}
	if len(args) != n {
		fail(fmt.Errorf("%s takes %d arguments", args[0], n-1))
	}
	f, ok := flows[args[1]]
	if !ok {
		fail(fmt.Errorf("no package %q in this runner", args[1]))
	}
	var err error
	switch args[0] {
	case "--wire":
		err = Wire(f, args[2], args[3], args[4])
	case "--agents":
		err = Agents(f, args[2], args[3])
	case "--connect-read":
		err = ReadRule(readRule[args[1]], args[2], args[3])
	case "--jev":
		err = Jev(f, jev[args[1]], args[2], args[3])
	}
	if err != nil {
		fail(err)
	}
	return true
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}
