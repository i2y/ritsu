#!/usr/bin/env python3
"""ask.py: put a rule's boundary vectors to a model and keep every answer.

One JSON line per vector: the inputs, the expected outputs, the rows that fired, and k
samples (raw text, parsed outputs, usage, latency). adapter.py later answers `rulec verify`
from this file, so the model is asked once and the comparison can be re-run for free.

  ask.py --rule ゆうパック運賃 --vectors ctx/ゆうパック運賃.vectors.jsonl \
         --context ctx/ゆうパック運賃.rule --label rule --model claude-opus-5 \
         --effort low --samples 3 --out runs/yupack.rule.opus5.jsonl
"""
import argparse, json, re, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor

PRICE = {  # USD per million tokens: input, output. cache read 0.1x, cache write 1.25x
    "claude-opus-5": (5.0, 25.0), "claude-sonnet-5": (2.0, 10.0), "claude-haiku-4-5": (1.0, 5.0),
    "claude-opus-4-8": (5.0, 25.0), "claude-sonnet-4-6": (3.0, 15.0),
}
EFFORT_OK = ("claude-opus-5", "claude-sonnet-5", "claude-opus-4-8", "claude-opus-4-7", "claude-opus-4-6", "claude-sonnet-4-6")


def canon(d):
    return json.dumps(d, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def weight_str(g):
    return f"{g // 1000}kg" if g % 1000 == 0 else f"{g}g"


# ---- the two rules: how a customer would ask, and how to read the answer ----------------

def q_yupack(i):
    return (f"東京都から{i['あて先']}へゆうパックで荷物を送ります。"
            f"荷物の縦・横・高さの合計は {i['三辺合計']}cm、重さは {weight_str(i['重量'])} です。"
            "ゆうパックの基本運賃（税込）はいくらですか。金額を整数の円で、数字だけ答えてください。")


def p_yupack(text):
    yen = re.findall(r"(\d[\d,]*)\s*円", text)
    nums = yen or re.findall(r"\d[\d,]*", text)
    if not nums:
        return None
    return {"運賃": int(nums[-1].replace(",", ""))}


def q_perk(i):
    return (f"会員区分「{i['会員区分']}」のお客様が、商品合計 {i['商品合計']}円 の注文に"
            f"値引 {i['値引']}円 を使いました。荷物の重量は {weight_str(i['重量'])} です。"
            "請求額と付与されるポイントを求め、"
            '{"請求額": 整数, "付与点": 整数} という JSON だけを答えてください。')


def p_perk(text):
    for m in re.finditer(r"\{[^{}]*\}", text):
        try:
            d = json.loads(m.group(0))
        except json.JSONDecodeError:
            continue
        out = {}
        for k in ("請求額", "付与点"):
            v = d.get(k)
            if isinstance(v, str):
                v = re.sub(r"[^\d-]", "", v)
            try:
                out[k] = int(v)
            except (TypeError, ValueError):
                return None
        return out
    return None


RULES = {
    "ゆうパック運賃": (q_yupack, p_yupack, "あなたは日本郵便のカスタマーサポート担当です。"),
    "会員特典": (q_perk, p_perk, "あなたはある通販サイトのカスタマーサポート担当です。"),
}


def system_text(role, ctx):
    s = role + "質問には最終的な答えだけを、指示された形式で書きます。\n"
    if ctx is None:
        return s + "資料はありません。知っている範囲で答えてください。"
    return s + "次の資料だけを根拠に答えてください。\n\n<資料>\n" + ctx + "\n</資料>"


# ---- backends ------------------------------------------------------------------------

class SDK:
    def __init__(self, model, effort, max_tokens):
        import anthropic
        self.anthropic = anthropic
        self.c = anthropic.Anthropic()
        self.model, self.effort, self.max_tokens = model, effort, max_tokens

    def ask(self, system, question):
        kw = dict(model=self.model, max_tokens=self.max_tokens,
                  system=[{"type": "text", "text": system, "cache_control": {"type": "ephemeral"}}],
                  messages=[{"role": "user", "content": question}])
        if self.effort and self.model in EFFORT_OK:
            kw["output_config"] = {"effort": self.effort}
        for attempt in range(6):
            t = time.time()
            try:
                r = self.c.messages.create(**kw)
            except self.anthropic.RateLimitError as e:
                wait = int(e.response.headers.get("retry-after", "20"))
                time.sleep(min(wait, 60) * (attempt + 1))
                continue
            except self.anthropic.APIStatusError as e:
                if e.status_code >= 500 and attempt < 5:
                    time.sleep(5 * (attempt + 1))
                    continue
                return dict(text="", error=f"{e.status_code}: {e.message}", ms=int((time.time() - t) * 1000), usage={})
            except self.anthropic.APIConnectionError as e:
                time.sleep(5 * (attempt + 1))
                continue
            u = r.usage
            return dict(
                text="".join(b.text for b in r.content if b.type == "text"),
                stop=r.stop_reason, ms=int((time.time() - t) * 1000),
                usage=dict(input=u.input_tokens, cache_write=u.cache_creation_input_tokens or 0,
                           cache_read=u.cache_read_input_tokens or 0, output=u.output_tokens))
        return dict(text="", error="gave up after retries", ms=0, usage={})


class Ollama:
    """A local model through Ollama's /api/chat. `think` turns a reasoning model's thinking on
    or off, which is the local analogue of effort. The system prompt is sent as the system
    message; Ollama caches the prompt prefix on its own."""

    def __init__(self, model, think, max_tokens, temperature=0.7, host="http://localhost:11434"):
        self.model, self.think, self.max_tokens, self.temperature, self.host = model, think, max_tokens, temperature, host

    def ask(self, system, question):
        import urllib.request
        body = json.dumps({
            "model": self.model, "stream": False, "think": self.think,
            "options": {"temperature": self.temperature, "num_predict": self.max_tokens},
            "messages": [{"role": "system", "content": system}, {"role": "user", "content": question}],
        }).encode("utf-8")
        req = urllib.request.Request(self.host + "/api/chat", data=body, headers={"Content-Type": "application/json"})
        t = time.time()
        for attempt in range(4):
            try:
                with urllib.request.urlopen(req, timeout=1800) as r:
                    d = json.loads(r.read().decode("utf-8"))
                break
            except Exception as e:  # a busy server, a model still loading
                if attempt == 3:
                    return dict(text="", error=str(e)[:300], ms=int((time.time() - t) * 1000), usage={})
                time.sleep(5 * (attempt + 1))
        m = d.get("message", {})
        return dict(text=m.get("content", ""), thinking=(m.get("thinking") or "")[:2000], stop=d.get("done_reason"),
                    ms=int((time.time() - t) * 1000),
                    usage=dict(input=d.get("prompt_eval_count", 0), cache_write=0, cache_read=0, output=d.get("eval_count", 0)))


class CLI:
    """`claude -p` with the user's login. Pilot use only: the harness adds ~9k tokens of its
    own to every call, and neither effort nor sampling can be set."""

    def __init__(self, model):
        self.model = model

    def ask(self, system, question):
        cmd = ["claude", "-p", question, "--system-prompt", system, "--tools", "",
               "--no-session-persistence", "--model", self.model, "--output-format", "json"]
        t = time.time()
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
        try:
            d = json.loads(p.stdout)
        except json.JSONDecodeError:
            return dict(text="", error=(p.stderr or p.stdout)[:300], ms=int((time.time() - t) * 1000), usage={})
        u = d.get("usage", {})
        return dict(text=d.get("result", ""), stop=d.get("stop_reason"), ms=d.get("duration_api_ms"),
                    cost=d.get("total_cost_usd"),
                    usage=dict(input=u.get("input_tokens", 0), cache_write=u.get("cache_creation_input_tokens", 0),
                               cache_read=u.get("cache_read_input_tokens", 0), output=u.get("output_tokens", 0)))


# ---- main -----------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rule", required=True, choices=sorted(RULES))
    ap.add_argument("--vectors", required=True)
    ap.add_argument("--context", default="none", help="a file whose text is the only material, or 'none'")
    ap.add_argument("--label", required=True, help="a name for the condition: none, page, rule, doc, article")
    ap.add_argument("--model", default="claude-opus-5")
    ap.add_argument("--backend", default="sdk", choices=("sdk", "cli", "ollama"))
    ap.add_argument("--effort", default="low", help="sdk: the effort level; ollama: 'think' or 'nothink'")
    ap.add_argument("--samples", type=int, default=3)
    ap.add_argument("--concurrency", type=int, default=6)
    ap.add_argument("--max-tokens", type=int, default=2000)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--only-why", default="", help="regex over the vector's `why`")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    q, parse, role = RULES[a.rule]
    ctx = None if a.context == "none" else open(a.context, encoding="utf-8").read()
    system = system_text(role, ctx)

    vectors = [json.loads(l) for l in open(a.vectors, encoding="utf-8") if l.strip().startswith("{")]
    if a.only_why:
        vectors = [v for v in vectors if re.search(a.only_why, v.get("why", ""))]
    if a.limit:
        vectors = vectors[:a.limit]

    done = set()
    try:
        for l in open(a.out, encoding="utf-8"):
            d = json.loads(l)
            if "key" in d:
                done.add(d["key"])
    except FileNotFoundError:
        pass
    todo = [v for v in vectors if canon(v["in"]) not in done]
    print(f"{a.rule} / {a.label} / {a.model} / effort={a.effort} / k={a.samples}: "
          f"{len(vectors)} vectors, {len(todo)} to ask, system prompt {len(system)} chars", file=sys.stderr)

    if a.backend == "sdk":
        be = SDK(a.model, a.effort, a.max_tokens)
    elif a.backend == "ollama":
        be = Ollama(a.model, think=(a.effort == "think"), max_tokens=a.max_tokens)
    else:
        be = CLI(a.model)
    out = open(a.out, "a", encoding="utf-8")
    if not done:
        out.write(json.dumps({"meta": dict(rule=a.rule, label=a.label, model=a.model, backend=a.backend,
                                            effort=a.effort, samples=a.samples, context=a.context,
                                            started=time.strftime("%Y-%m-%dT%H:%M:%S"))}, ensure_ascii=False) + "\n")
        out.flush()

    def one(v):
        question = q(v["in"])
        samples = []
        for _ in range(a.samples):
            r = be.ask(system, question)
            r["parsed"] = parse(r.get("text", "")) if r.get("text") else None
            samples.append(r)
        return dict(key=canon(v["in"]), **{k: v[k] for k in ("in", "out", "trace", "why") if k in v},
                    question=question, samples=samples)

    lock = __import__("threading").Lock()
    n = 0
    if todo:  # warm the cache once, so the parallel calls read it instead of all writing it
        first = one(todo[0])
        out.write(json.dumps(first, ensure_ascii=False) + "\n"); out.flush(); n += 1
        todo = todo[1:]
    with ThreadPoolExecutor(max_workers=a.concurrency) as ex:
        for rec in ex.map(one, todo):
            with lock:
                out.write(json.dumps(rec, ensure_ascii=False) + "\n"); out.flush(); n += 1
                if n % 20 == 0:
                    print(f"  {n} done", file=sys.stderr)
    out.close()
    summarize(a.out, a.model)


def summarize(path, model):
    recs = [json.loads(l) for l in open(path, encoding="utf-8")]
    recs = [r for r in recs if "key" in r]
    k = max(len(r["samples"]) for r in recs)
    per_sample = [0] * k
    agree_ok = agree_wrong = disagree = unanswered = 0
    usage = dict(input=0, cache_write=0, cache_read=0, output=0)
    cost_cli = 0.0
    for r in recs:
        exp = canon(r["out"])
        parsed = [s.get("parsed") for s in r["samples"]]
        for i, p in enumerate(parsed):
            if p is not None and canon(p) == exp:
                per_sample[i] += 1
        if any(p is None for p in parsed):
            unanswered += 1
        elif all(canon(p) == canon(parsed[0]) for p in parsed):
            if canon(parsed[0]) == exp:
                agree_ok += 1
            else:
                agree_wrong += 1
        else:
            disagree += 1
        for s in r["samples"]:
            for key in usage:
                usage[key] += (s.get("usage") or {}).get(key, 0) or 0
            cost_cli += s.get("cost") or 0
    n = len(recs)
    print(f"vectors {n}; per-sample correct: {per_sample}; "
          f"all-agree correct {agree_ok}, all-agree wrong {agree_wrong}, disagree {disagree}, unanswered {unanswered}",
          file=sys.stderr)
    pin, pout = PRICE.get(model, (0, 0))
    est = (usage["input"] * pin + usage["cache_write"] * pin * 1.25 + usage["cache_read"] * pin * 0.1 + usage["output"] * pout) / 1e6
    print(f"tokens {usage}; estimated cost ${est:.2f}" + (f" (cli-reported ${cost_cli:.2f})" if cost_cli else ""), file=sys.stderr)


if __name__ == "__main__":
    main()
