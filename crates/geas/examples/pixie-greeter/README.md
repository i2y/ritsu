# pixie-greeter

Claims ([greeter.geas](greeter.geas)) on a desktop app: the greeter of pixie, a GUI language whose
apps compile to native binaries. The app has a field for a name, a button that greets it, and a
second field for a note. A pixie app can replay a script of actions headless (`PIXIE_SCRIPT`) and
print its accessibility tree after each, so geas collects each claim's actions on the app, runs them
as one script, and hands each `when` the screen after its action. No window opens.

## The app

The app is not in this repository: it is a 57 MB binary, built with pixie. In a clone of pixie's
repository, as pixie's README says, install pixie's runtime once and build the greeter example
(`examples/greeter/greeter.pix` there):

```console
$ cargo run -q -p pixie-cli -- install-runtime
$ cargo run -q -p pixie-cli -- build examples/greeter/greeter.pix
```

pixie builds every app into one shared directory, so the app is `~/.cache/pixie/target/debug/greeter`.
Put it beside the claims file as `greeter`, which git ignores:

```console
$ ln -s ~/.cache/pixie/target/debug/greeter examples/pixie-greeter/greeter
$ geas check examples/pixie-greeter/greeter.geas
```

`pixie "greeter"` starts the file `greeter` in the claims file's directory, or else a program of that
name on PATH; without either, each claim ends in E030. Run by hand without `PIXIE_SCRIPT`, the app
opens a window; geas always gives it a script.

## What to look for

All four claims hold. They show what a replayed driver can and cannot do:

- `open()` alone is the first screen, and `does not contain text containing "Hello"` holds there.
- `input("Ada")` types into the first text field; pixie reaches fields by position, so the note is
  `input("milk", field: 2)`, and `into:` is refused before anything runs (E013).
- The greeter binds nothing to a key, so Enter is sent to the field with `submit()`; `press("enter")`
  would be refused by the app.
- Within a claim, no other target's `when` may come between two actions on the app (E012): the app
  runs only while its script does.

geas's tests run these claims, and those of `tests/pixie/`, when `GEAS_PIXIE_GREETER` names a built
greeter; without it they print `SKIP:`.
