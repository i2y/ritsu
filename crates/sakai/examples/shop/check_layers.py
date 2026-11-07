# Holds the code of the anticorruption layers in py/ to what the map says (`sakai vectors`).
#
#   sakai vectors examples/shop/shop.ctx | python3 examples/shop/check_layers.py
#
# sakai does not read the code of a layer; it gives the lines, and this reads them: each line is a value
# of an upstream enum, the number it has on the wire, and what the map makes of it (where it goes, that it
# is refused, or that the map says nothing of it: the code must raise on the last two, and on a number the
# enum does not have yet). Only the standard
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
    seen = {}
    for text in sys.stdin:
        if not text.strip():
            continue
        line = json.loads(text)
        key = (line["context"], line["enum"])
        if key not in LAYERS:
            left_out += 1
            continue
        module, function, expect = LAYERS[key]
        read = getattr(importlib.import_module(module), function)
        where = f'{line["context"]} {line["enum"]} {line["value"]} ({line["number"]})'
        seen[key] = max(seen.get(key, 0), line["number"])
        if "refuse" in line or line.get("unmapped"):
            # a value the map refuses, or says nothing of (`unmapped`: 0, or a value the upstream adds later), is an
            # error in the code: it does not guess what the value means
            try:
                got = read(line["number"])
            except ValueError:
                held += 1
            else:
                failed.append(f"{where}: the map {'refuses' if 'refuse' in line else 'says nothing of'} it, and the code answers {got!r}")
            continue
        want = expect(line)
        try:
            got = read(line["number"])
        except ValueError as e:
            failed.append(f"{where}: the map says {want!r}, and the code raises ({e})")
            continue
        if got == want:
            held += 1
        else:
            failed.append(f"{where}: the map says {want!r}, and the code answers {got!r}")
    # a number the upstream's enum does not have yet: the first one past the last value the map lists
    for key, last in seen.items():
        module, function, _ = LAYERS[key]
        read = getattr(importlib.import_module(module), function)
        where = f"{key[0]} {key[1]} {last + 1} (a value the proto does not have yet)"
        try:
            got = read(last + 1)
        except ValueError:
            held += 1
        else:
            failed.append(f"{where}: the map says nothing of it, and the code answers {got!r}")
    for f in failed:
        print(f)
    print(f"{held} held, {len(failed)} failed, {left_out} left out (a layer this file does not know)")
    return 1 if failed else 0


sys.exit(main())
