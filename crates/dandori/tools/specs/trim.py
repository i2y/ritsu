# Copies of published API descriptions, cut down to what the examples call, so that the
# repository holds a few kilobytes of each and not the whole: an OpenAPI document keeps the
# operations named and the schemas named, and every other schema they refer to is left out (a
# `$ref` to it becomes an empty schema marked x-dandori-left-out, which reads as any value); a
# Smithy model keeps the operations named and every shape they reach, and its service shape
# without the rules that pick an endpoint. The copy says where it came from.
#
#   python3 tools/specs/trim.py openapi <spec.json> <out.json> "<source>" --op "POST /v1/…" … --schema <name> …
#   python3 tools/specs/trim.py smithy <model.json> <out.json> "<source>" --op <Operation> …

import json
import sys


def openapi(doc, out, source, ops, keep):
    def cut(x):
        if isinstance(x, dict):
            r = x.get("$ref")
            if isinstance(r, str) and r.startswith("#/components/schemas/") and r.split("/")[-1] not in keep:
                return {"x-dandori-left-out": r}
            return {k: cut(v) for k, v in x.items()}
        if isinstance(x, list):
            return [cut(v) for v in x]
        return x

    paths = {}
    for op in ops:
        method, path = op.split(" ", 1)
        paths.setdefault(path, {})[method.lower()] = cut(doc["paths"][path][method.lower()])
    info = dict(doc["info"])
    info["description"] = f"{source}. Only the operations {', '.join(ops)} and the schemas {', '.join(sorted(keep))} are kept; every other schema they refer to is left out (x-dandori-left-out) and reads as any value. Made by tools/specs/trim.py."
    kept = {"openapi": doc["openapi"], "info": info, "servers": doc.get("servers", []), "paths": paths, "components": {"schemas": {k: cut(doc["components"]["schemas"][k]) for k in sorted(keep)}}}
    # on one line, as Stripe writes its own: the operations' bodies alone are a hundred kilobytes
    with open(out, "w", encoding="utf-8") as f:
        json.dump(kept, f, ensure_ascii=False, separators=(",", ":"))
        f.write("\n")


def smithy(model, out, source, ops):
    shapes = model["shapes"]
    service_id = next(k for k, v in shapes.items() if v["type"] == "service")
    wanted = [k for k, v in shapes.items() if v["type"] == "operation" and k.split("#")[1] in ops]
    seen, todo = set(), list(wanted)
    while todo:
        s = todo.pop()
        if s in seen or s not in shapes:
            continue
        seen.add(s)

        def walk(x):
            if isinstance(x, dict):
                for k, v in x.items():
                    if k == "target" and isinstance(v, str):
                        todo.append(v)
                    else:
                        walk(v)
            elif isinstance(x, list):
                for v in x:
                    walk(v)

        walk(shapes[s])
    service = dict(shapes[service_id])
    service["operations"] = [{"target": w} for w in sorted(wanted)]
    service["traits"] = {k: v for k, v in service.get("traits", {}).items() if not k.startswith("smithy.rules#")}
    for e in service.get("errors", []):
        seen.add(e["target"])
    kept = {
        "smithy": model["smithy"],
        "metadata": {"dandori": f"{source}. Only the operations {', '.join(ops)} and the shapes they reach are kept, and the service without its endpoint rules. Made by tools/specs/trim.py."},
        "shapes": {service_id: service, **{k: shapes[k] for k in sorted(seen)}},
    }
    with open(out, "w", encoding="utf-8") as f:
        json.dump(kept, f, ensure_ascii=False, indent=1)
        f.write("\n")


if __name__ == "__main__":
    kind, src, out, source = sys.argv[1:5]
    rest = sys.argv[5:]
    ops = [rest[i + 1] for i, a in enumerate(rest) if a == "--op"]
    keep = {rest[i + 1] for i, a in enumerate(rest) if a == "--schema"}
    with open(src, encoding="utf-8") as f:
        doc = json.load(f)
    if kind == "openapi":
        openapi(doc, out, source, ops, keep)
    else:
        smithy(doc, out, source, ops)
