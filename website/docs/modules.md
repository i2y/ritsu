# Modules

A module is a directory with a `ritsu.mod`: a project that other projects require, from a directory
beside them or from an OCI registry, and whose files they read by `@<name>/…` paths. The modules
published from ritsu's repository are on [the list of modules](modules/list/), with their versions,
the pages of their files and what changed between versions.

## Writing a module

## Requiring a module

## Fetching what a module requires

## Pushing a module to a registry

## Signatures

## What raising a version changes

## Libraries

A module is published for npm, PyPI and Go with `ritsu gen --library`:
[Publishing libraries](libraries.md).
