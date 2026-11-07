# Holds the code of the anticorruption layers in py/ to what the map says (`sakai vectors`).
#
#   sakai vectors examples/shop/shop.ctx | python3 examples/shop/check_layers.py
#
# sakai does not read the code of a layer; it gives the lines, and this reads them: each line is a value
# of an upstream enum, the number it has on the wire, and what the map makes of it. Only the standard
# library is used, and the code is imported from py/ as the services import it (the modules the
# protobuf compiler writes are stand-ins there).
import importlib
import json
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "py"))

# The layers of the example, by context and upstream enum: the module, the function that reads the
# number of the value, and what it should answer for a line of the map.
def ship(line):
    # delivery.ctx maps the values to the decision `ship` or `wait`
    return {"ship": True, "wait": False}[line["to"]["value"]]


def handling(line):
    # billing_need.rule decides what billing does with each status; the map says what rulec answers
    return line["answers"]["handling"]


LAYERS = {
    ("Delivery", "PackingStatus"): ("delivery.acl.inventory.packing", "may_ship", ship),
    ("Billing", "OrderStatus"): ("billing.acl.ordering.status", "handling", handling),
}


def main():
    held, failed, left_out = 0, [], 0
    for text in sys.stdin:
        if not text.strip():
            continue
        line = json.loads(text)
        key = (line["context"], line["enum"])
        if key not in LAYERS:
            left_out += 1
            continue
        # a value the map says nothing of (`unmapped`) is left to the code's tests: there is nothing to hold it to
        if line.get("unmapped"):
            left_out += 1
            continue
        module, function, expect = LAYERS[key]
        read = getattr(importlib.import_module(module), function)
        where = f'{line["context"]} {line["enum"]} {line["value"]} ({line["number"]})'
        if "refuse" in line:
            # a refused value is an error in the code
            try:
                got = read(line["number"])
            except ValueError:
                held += 1
            else:
                failed.append(f"{where}: the map refuses it, and the code answers {got!r}")
            continue
        want = expect(line)
        got = read(line["number"])
        if got == want:
            held += 1
        else:
            failed.append(f"{where}: the map says {want!r}, and the code answers {got!r}")
    for f in failed:
        print(f)
    print(f"{held} held, {len(failed)} failed, {left_out} left out (unmapped, or a layer this file does not know)")
    return 1 if failed else 0


sys.exit(main())
