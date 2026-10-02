// A tally kept in memory, over HTTP: `./tally <port>`. It stops on SIGTERM,
// returning from main, which is also when a build with -cover writes what ran.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"sync"
	"syscall"
)

type tally struct {
	mu    sync.Mutex
	total int
}

func send(w http.ResponseWriter, status int, value any) {
	body, _ := json.Marshal(value)
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	w.Write(body)
}

func (t *tally) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	t.mu.Lock()
	defer t.mu.Unlock()
	switch {
	case r.Method == http.MethodGet && r.URL.Path == "/total":
		send(w, 200, map[string]int{"total": t.total})
	case r.Method == http.MethodPost && r.URL.Path == "/add":
		raw, _ := io.ReadAll(r.Body)
		text := strings.TrimSpace(string(raw))
		n, err := strconv.Atoi(text)
		if err != nil {
			send(w, 400, map[string]string{"error": "not a whole number: " + text})
			return
		}
		t.total += n
		send(w, 200, map[string]int{"total": t.total})
	case r.Method == http.MethodGet && r.URL.Path == "/health":
		send(w, 200, map[string]bool{"ok": true})
	case r.Method == http.MethodPost && r.URL.Path == "/reset":
		t.total = 0
		send(w, 200, map[string]int{"total": t.total})
	default:
		send(w, 404, map[string]string{"error": "not found"})
	}
}

func main() {
	port := "8125"
	if len(os.Args) > 1 {
		port = os.Args[1]
	}
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGTERM, os.Interrupt)
	defer stop()
	server := &http.Server{Addr: "127.0.0.1:" + port, Handler: &tally{}}
	go func() {
		if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
	}()
	<-ctx.Done()
	server.Shutdown(context.Background())
}
