"""Read JSON values as protobuf's JSON mapping (protojson), as a client of the service in another
language reads them: the messages that a workflow that implements a service takes and answers, read
with the messages of a descriptor set that protoc wrote. A field the message does not have is
refused. For each value, what protobuf makes of it, written back as protobuf's JSON (the zero values
of the fields without presence left out), or why protobuf refused it.

usage: check.py <descriptors.pb> <values.json> <results.json>

values.json:  [ { "message": <the message's full name>, "json": <a value> } ]
results.json: [ { "ok": <the message, as protobuf's JSON> } | { "error": <why it was refused> } ]

It needs protobuf for Python (tools/temporal-python/.venv has it).
"""

import json
import sys

from google.protobuf import descriptor_pb2, descriptor_pool, json_format, message_factory


def main() -> None:
    files = descriptor_pb2.FileDescriptorSet()
    with open(sys.argv[1], "rb") as f:
        files.ParseFromString(f.read())
    pool = descriptor_pool.DescriptorPool()
    for file in files.file:
        pool.Add(file)
    with open(sys.argv[2], encoding="utf-8") as f:
        values = json.load(f)
    out = []
    for v in values:
        cls = message_factory.GetMessageClass(pool.FindMessageTypeByName(v["message"]))
        try:
            message = json_format.Parse(json.dumps(v["json"]), cls(), ignore_unknown_fields=False)
            out.append({"ok": json_format.MessageToDict(message)})
        except Exception as e:  # noqa: BLE001 - why protobuf refused it is what is told
            out.append({"error": f"{type(e).__name__}: {e}"})
    with open(sys.argv[3], "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False)


if __name__ == "__main__":
    main()
