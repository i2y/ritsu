#!/bin/sh
# go-arch-lint v1.19.0 (PLAN 0.4), built with -trimpath into tools/go/bin. The build cache, the
# module cache and GOPATH go under tools/go/cache unless GOCACHE, GOMODCACHE or GOPATH say
# otherwise; none of it is in git.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
export GOBIN="$here/bin"
export GOCACHE="${GOCACHE:-$here/cache/build}"
export GOMODCACHE="${GOMODCACHE:-$here/cache/mod}"
export GOPATH="${GOPATH:-$here/cache/gopath}"
export GOTOOLCHAIN=local
export GOFLAGS=-modcacherw

go install -trimpath github.com/fe3dback/go-arch-lint@v1.19.0
"$GOBIN/go-arch-lint" version 2>/dev/null || true
