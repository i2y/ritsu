# tally-go

The same tally as [tally-node](../tally-node), written in Go ([main.go](main.go)), with the same
five claims ([tally.geas](tally.geas)). The claims say nothing about the language: only the target's
command changed, `serve "./tally {port}"`.

`geas map` needs the program built with coverage, and the service has to stop on SIGTERM by
returning from `main`, which is when a Go program built with `-cover` writes what it ran; this one
does it with `signal.NotifyContext` and `Server.Shutdown`. Build it in its directory (the binary
is ignored by git):

```console
$ cd examples/tally-go
$ go build -cover -coverpkg=./... -trimpath -o tally .
$ cd ../..
$ geas check examples/tally-go/tally.geas
$ geas map examples/tally-go/tally.geas --root examples/tally-go
```

`map` sets `GOCOVERDIR` for the service and reads the counters with `go tool covdata`, and prints
`map: 1 file · 43 lines of code, 40 run by some claim`. Built with `-cover`, the program prints
`warning: GOCOVERDIR not set, no coverage data emitted` on stderr whenever it runs outside `map`,
which a claim on a command's `stderr` would see: that build is for `map`. A Go service left to Go's
default handling of SIGTERM writes no counters, and `map` says so (E066).

[tests/changes/tally-go](../../tests/changes/tally-go) holds an agent's change, and
[tests/golden/en/languages/tally-go](../../tests/golden/en/languages/tally-go) what the tests expect
of `map` and `geas affected` on it: the `/health` case it adds, lines 48 and 49, is code no claim
runs.
