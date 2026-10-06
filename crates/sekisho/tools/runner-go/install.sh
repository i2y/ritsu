#!/bin/sh
# The modules tests/go.rs builds the generated Go with: those go.mod requires (cedar-go, the AWS
# SDK's client of Verified Permissions), fetched into the module cache to the sums of go.sum. The
# tests then build with no network (GOPROXY=off). The cache is Go's own (GOMODCACHE); nothing of it
# is in git.
set -eu

cd "$(dirname "$0")"
GOTOOLCHAIN=local GOFLAGS=-modcacherw go mod download
GOTOOLCHAIN=local go list -m all
