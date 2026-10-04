# GUI targets: the screen

A GUI target turns each `when` into an action on an app and observes the screen once the app has
settled: its accessibility tree, which is what a screen reader is told. Claims say what the screen
contains. They never name a widget class, a CSS selector, an element id or a pixel, so a control a
claim cannot reach by its role and name is one a person using a screen reader cannot reach
either, and that failure says something true about the app.

Three drivers stand behind one contract: a page in Chrome (a `serve` target), a pixie app
(`pixie`), and a program of your own (`driver`).

## The screen

The screen is a tree of nodes. A node has a role, a name, a value, states and children.

- The role is a WAI-ARIA 1.2 role name (`button`, `textbox`, `heading`, `checkbox`, `dialog`,
  `listitem`, …), or `text` for static text. Each driver maps its own words onto these, so one
  claim reads the same against a pixie app and a web page.
- The name is the accessible name; the value is what a screen reader reads after it (a field's
  contents, a slider's number); the states are `disabled`, `checked` and `unchecked` (a node that is
  not `disabled` is `enabled`).
- A container that only lays things out reports nothing and hands its children up; a node named by
  its own text drops the text children that repeat its name or its value; empty text reports
  nothing.

In a report, a screen is one node a line, two spaces a level:

```text
      the screen:
        heading "Greeter"
        text "Today is Saturday, August 29, 2026."
        form
          text "Your name"
          textbox "Your name"
          text "Greet"
        status
        list "Greeted"
        text "Your fortune: A pleasant surprise is waiting for you."
```

(That page made its Greet button a `<div>` with a click handler: it looks like a button and a
mouse can click it, but to a screen reader it is the text "Greet", and the claim that clicks the
button named Greet is refused, E035.)

## The actions

| Action | What it does |
|---|---|
| `open()`, `open("/path")` | the app's first screen; in a browser, that path of the service |
| `click("<name>")`, `click("<name>", nth: n)` | presses the n-th (by default the first) button, link, checkbox, radio, switch, tab, menu item or option of that name |
| `input("<text>")`, `input("<text>", field: n)`, `input("<text>", into: "<name>")` | makes the text what the n-th text field (`textbox`, `searchbox`, `spinbutton`), or the field of that name, holds |
| `submit()`, `submit(field: n)`, `submit(into: "<name>")` | Enter in that field |
| `press("<key>")` | a key or a chord: `enter`, `escape`, `tab`, `backspace`, `up`, `cmd-s`, `ctrl-a`, `shift-tab` |
| `advance(<ms>)` | moves the app's clock forward |

Each claim gets a fresh app, as it gets a fresh service. An action the app cannot take (no control
of that name, a field that is not there) ends the claim with E035, and the message gives the screen
it was on and what on it the action could reach:

```text
      = nothing on it can be clicked
```

A `combobox` (a `<select>`) is not a text field. Choosing an option of a native `<select>` is not an
action yet; reach it with `press`.

## A page in Chrome (live)

A `serve` target can be driven in a headless Chrome: `open("/path")` loads
`http://127.0.0.1:<port>/path`, and the other actions act on that page. `get` and `post` still
reach the same service in the same claim.

- Chrome is found through `GEAS_CHROME` (then the only one looked at), the macOS application, or
  `google-chrome`, `chromium` or `chromium-browser` on PATH; without it, the claims that need it end
  in E034.
- One Chrome per run (one per worker under `--jobs`), headless, with a profile under `.geas/` that
  is removed when geas exits, also on Ctrl-C.
- Each claim gets a browser context of its own: cookies and storage do not pass from one claim to
  the next.
- Settling is by virtual time. The page's clock is paused; after each action geas grants one second
  of the page's own time and waits until that second has run, so timers due within it fire and a
  fetch holds the clock until it returns. A status set 300 ms after a click is there at the next
  check, without waiting in real time. `advance(ms)` grants that much more. A page whose second
  does not run out within 10 s of real time is E036, and so is a page that does not load.
- `click` finds the node by role and name, scrolls it into view, and presses the mouse at the centre
  of its box, so whatever a person's click would land on, an overlay included, is what gets it.
  `input` selects what the field holds and types the text; an empty text clears it. `submit` and
  `press` send key events; `press` takes a letter, a digit, `enter`, `escape`, `tab`, `backspace`,
  `delete`, `space`, the arrows, `home`, `end`, `pageup`, `pagedown`, `f1` to `f12`, and chords of
  them with `cmd`, `ctrl`, `alt` and `shift` (any other key is E013).
- A page that pins no `locale`, `tz` or `clock` sees the machine's, so pin them for screens that
  should read the same everywhere; `seed` makes `Math.random` the same in every run. On a
  `port auto` service, the page's own address is written `{port}` in names and values.
- A JavaScript dialog is accepted.

```geas
target web {
  serve "python3 server.py {port}"
  port auto
}

locale "en-US"
tz "Asia/Tokyo"
clock "2026-08-29T09:00:00+09:00"
seed 7

claim "the first screen asks for a name" {
  when web.open("/")
  then screen contains heading "Greeter"
  and  screen contains text "Today is Saturday, August 29, 2026."
  and  screen contains textbox "Your name"
  and  screen contains button "Greet" disabled
}

claim "the service keeps the names the page greets" {
  when web.open("/")
  when web.input("Ada")
  when web.submit()
  when web.get("/api/greetings")
  then body json ".names[0]" is "Ada"
}
```

## A pixie app (replayed)

`pixie "<app>"` runs a built pixie app: a file of that name in the spec's directory, else a program
on PATH. A pixie app replays a script of steps headless (`PIXIE_SCRIPT`) and prints its
accessibility tree after each, so geas collects a claim's actions on the app, runs them as one
script with the tree read after each action, and hands each `when` its screen.

- Within one claim, no other target's `when` may come between two actions on a pixie app (E012):
  the app is not running between them. Before the first and after the last is fine.
- The app starts once per claim, so `open()` after another action on it is E013, and so is a path.
- Text fields are reached by position only: `input("Ada", field: 2)`; `into:` is E013.
- `click("x", nth: n)`, `input`, `submit`, `press("k")` and `advance(ms)` become pixie's steps
  `click`, `input`, `submit`, `key` and `advance`. A key the app binds nothing to is refused; to
  send Enter to a field, use `submit()`.
- When a step names something the app does not have, the app exits with 101 and prints nothing
  about it. geas then runs the script's beginnings until the first that fails, to find the refused
  action and the screen just before it: E035, with what that screen had to click.
- Each action has 5 s; an app that does not finish, or prints trees geas cannot read, is E037.
- pixie prints names and values without escaping them, so a name holding `", ` or `]` can be read
  two ways; geas takes the shorter reading.

pixie's roles become ARIA's: `label` is `text`, `textInput` is `textbox`, `image` is `img`,
`listItem` is `listitem`, and a checkbox's or a switch's value `true` or `false` becomes the state
`checked` or `unchecked`.

## Any other GUI: a driver of your own (live)

`driver "<command>"` starts the command once per claim, in the spec's directory with the target's
pins, and speaks JSON lines on its stdin and stdout:

1. geas sends `{"geas":1,"pins":{…}}`, the pins that are set; the driver answers `{"ok":true}`, or
   `{"error":"…"}` for a pin it cannot keep (E011).
2. Then one action a line: `{"do":"open"}` or `{"do":"open","path":"/"}`,
   `{"do":"click","name":"greet","nth":1}`, `{"do":"input","text":"Ada","field":1}` (or
   `"into":"<name>","nth":1`), `{"do":"submit","field":1}`, `{"do":"press","key":"enter"}`,
   `{"do":"advance","ms":500}`. Each is answered by one line: `{"screen":<node>}`, or
   `{"error":"…","screen":<node>}` when the app refused the action (E035).
3. Last, `{"do":"close"}`; the driver exits within 5 s, or what is left of its process group is
   killed.

A node is `{"role":…,"name":…,"value":…,"states":[…],"children":[…]}` without the members that are
empty; a root without a role is the screen. An answer that does not come within 5 s, or is not of
that shape, is E037, with the last lines of the driver's stderr.

A whole driver, for a pretend app with one counter, as `counter_driver.py`:

```python
import json
import sys

count = 0


def screen():
    return {"children": [
        {"role": "heading", "name": "Counter"},
        {"role": "text", "name": "Count: %d" % count},
        {"role": "button", "name": "Add"},
    ]}


def answer(message):
    print(json.dumps(message), flush=True)


for line in sys.stdin:
    message = json.loads(line)
    if "geas" in message:
        answer({"ok": True})
    elif message["do"] == "close":
        break
    elif message["do"] == "open":
        answer({"screen": screen()})
    elif message["do"] == "click" and message["name"] == "Add":
        count += 1
        answer({"screen": screen()})
    else:
        answer({"error": "the counter cannot do that", "screen": screen()})
```

and claims on it:

```geas
target counter {
  driver "python3 counter_driver.py"
}

claim "counts the clicks" {
  when counter.open()
  then screen contains text "Count: 0"
  when counter.click("Add")
  when counter.click("Add")
  then screen contains text "Count: 2"
  and  screen contains exactly 1 button
}
```

This is how a GUI geas does not drive itself is reached: an accessibility API of a desktop (macOS's
needs a person to grant the permission in System Settings, which geas's tests cannot), an Android
emulator, a terminal UI.
