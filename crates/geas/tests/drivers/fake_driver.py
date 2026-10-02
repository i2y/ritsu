"""A fake GUI driver for geas's tests. It speaks the driver protocol (DESIGN §8.5)
for an app described in the JSON file named on its command line:

    {
      "start": "<screen>",
      "screens": {"<screen>": <node>, ...},
      "on": {"<screen> <action>": "<screen>", ...},
      "refuse_pins": ["clock", ...],
      "garble": "<name>", "hang": "<name>", "die": "<name>"
    }

A screen is a node as the protocol sends it; `{1}`, `{2}` in a name or a value
stand for what was typed into the first and the second text field. An action is
`open`, `click:<name>`, `submit`, `press:<key>` or `advance`; one with no entry in
`on` leaves the screen as it is. Clicking the button named by `garble`, `hang` or
`die` answers with a line that is not JSON, answers nothing, or exits.
"""

import json
import sys

FIELDS = ("textbox", "searchbox", "spinbutton")
CLICKABLE = ("button", "link", "checkbox", "radio", "switch", "tab", "menuitem", "option")


def render(node, typed):
    out = dict(node)
    for key in ("name", "value"):
        if key in out:
            for n, text in typed.items():
                out[key] = out[key].replace("{%d}" % n, text)
            out[key] = out[key].replace("{1}", "").replace("{2}", "")
    if "children" in node:
        out["children"] = [render(c, typed) for c in node["children"]]
    return out


def walk(node):
    yield node
    for c in node.get("children", []):
        yield from walk(c)


def say(answer):
    sys.stdout.write(json.dumps(answer) + "\n")
    sys.stdout.flush()


def main():
    with open(sys.argv[1]) as f:
        app = json.load(f)
    screen = app["start"]
    typed = {}
    for line in sys.stdin:
        msg = json.loads(line)
        if "geas" in msg:
            refused = [p for p in app.get("refuse_pins", []) if p in msg["pins"]]
            say({"error": "this app cannot pin " + ", ".join(refused)} if refused else {"ok": True})
            continue
        do = msg["do"]
        if do == "close":
            return
        shown = render(app["screens"][screen], typed)
        if do in ("input", "submit"):
            fields = [n for n in walk(shown) if n.get("role") in FIELDS]
            if "into" in msg:
                named = [i for i, n in enumerate(fields) if n.get("name") == msg["into"]]
                place = named[msg["nth"] - 1] + 1 if len(named) >= msg["nth"] else None
            else:
                place = msg["field"] if msg["field"] <= len(fields) else None
            if place is None:
                say({"error": "no such text field", "screen": shown})
                continue
            if do == "input":
                typed[place] = msg["text"]
                say({"screen": render(app["screens"][screen], typed)})
                continue
            action = "submit"
        elif do == "click":
            name = msg["name"]
            if name == app.get("garble"):
                print("this line is not JSON", flush=True)
                continue
            if name == app.get("hang"):
                continue
            if name == app.get("die"):
                sys.stderr.write("the fake app fell over\n")
                sys.exit(3)
            found = [n for n in walk(shown) if n.get("role") in CLICKABLE and n.get("name") == name]
            if len(found) < msg["nth"]:
                say({"error": 'nothing to click named "%s"' % name, "screen": shown})
                continue
            action = "click:" + name
        elif do == "press":
            action = "press:" + msg["key"]
        else:
            action = do
        screen = app.get("on", {}).get(screen + " " + action, screen)
        say({"screen": render(app["screens"][screen], typed)})


main()
