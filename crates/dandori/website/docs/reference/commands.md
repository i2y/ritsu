# Commands

```text
dandori check <file.flow>...
dandori build <file.flow> --target asl|temporal|temporal-python|durable|argo|pydantic-graph [--out <dir>]
dandori scenarios <file.flow> [--out <dir>]
dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|durable|argo|pydantic-graph]
```

| Command | What it does |
|---|---|
| `check` | Checks one or more `.flow` files: types, every arm, every state a case can be left in, retries. Prints the diagnostics, each with the run that gets there, and `ok` for a file that passes. |
| `build` | Writes the code for one platform into `--out` (`out/` by default), and prints each file's path. Also refuses what the platform cannot do (E050), and a workflow whose one run can outgrow the platform (E040). [Build for a platform](../platforms.md) |
| `scenarios` | Writes scenarios (an input and the answers the calls get) that together take every arm, every handler and every way a case can move: one file each into `--out`, or all of them as JSON on standard output. |
| `run` | Plays one scenario through the reference interpreter, and prints every call as the target would make it, every wait, and how the run ends. |

## Flags and environment

| | |
|---|---|
| `--target <target>` | the platform to build for, or the one whose calls `run` prints (`reference` by default) |
| `--out <dir>` | where `build` writes (`out/` by default), and where `scenarios` writes |
| `--scenario <file.json>` | the scenario `run` plays |
| `--format json` | the diagnostics of `check` as JSON, for tools |
| `--lang ja` or `--lang en` | the language of the messages; else `DANDORI_LANG`, else English |
| `DANDORI_RULEC` | the rulec binary; else `rulec` on the PATH |

## Exit codes

| Code | Means |
|---|---|
| 0 | no errors (warnings only, or nothing) |
| 1 | errors found |
| 2 | bad arguments, or a file that cannot be read |
