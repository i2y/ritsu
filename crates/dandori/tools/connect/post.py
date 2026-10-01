# Sends each request of a file to a rule's Connect service, as dandori's generated code does (a POST
# with a JSON body, Content-Type: application/json and Connect-Protocol-Version: 1), with the
# standard library alone, and writes down what came back: the status, the headers (their names in
# lower case) and the body, parsed when it is JSON. The test that sends rulec's services what dandori
# sends them (tests/protos.rs) runs it with the Python of tools/connect/.venv.
#
#   python post.py <requests.json> <answers.json>
#
# requests.json: { "url": "<the service's URL and the path>", "bodies": [ <a JSON body>, ... ] }
# answers.json:  [ { "status": 200, "headers": { ... }, "body": <the answer> }, ... ]

import json
import sys
import urllib.error
import urllib.request


def send(url: str, body: object) -> dict:
    data = json.dumps(body, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    req = urllib.request.Request(url, data=data, method="POST", headers={"Content-Type": "application/json", "Connect-Protocol-Version": "1"})
    try:
        res = urllib.request.urlopen(req, timeout=30)
    except urllib.error.HTTPError as e:
        res = e
    text = res.read().decode("utf-8")
    headers = {k.lower(): v for k, v in res.headers.items()}
    try:
        parsed = json.loads(text)
    except ValueError:
        parsed = text
    return {"status": res.status, "headers": headers, "body": parsed}


def main() -> None:
    with open(sys.argv[1], encoding="utf-8") as f:
        spec = json.load(f)
    answers = [send(spec["url"], body) for body in spec["bodies"]]
    with open(sys.argv[2], "w", encoding="utf-8") as f:
        json.dump(answers, f, ensure_ascii=False)


if __name__ == "__main__":
    main()
