# Runs the agent call of the default Transport that dandori writes for Python (io.py) with
# OpenAI's Agents SDK, and a scripted model in place of OpenAI's, so that nothing leaves the
# machine. The Python twin of check.mjs: the same cases, and the results in the same shape.
#
#   .venv/bin/python check.py <io.py> <cases.json> <results.json>
#
# The venv:
#   uv venv --python 3.13 tools/agents/.venv
#   uv pip install --python tools/agents/.venv/bin/python -r tools/agents/requirements.txt

from __future__ import annotations

import asyncio
import importlib.util
import json
import sys
from typing import Any

from agents import Model, ModelProvider, RunConfig
from agents.testing import ScriptedModel, assistant_message
from openai.types.responses import ResponseOutputMessage, ResponseOutputRefusal


class Provider(ModelProvider):
    """Gives the scripted model for any name, and writes the names down."""

    def __init__(self, model: Model) -> None:
        self.model = model
        self.names: list[str | None] = []

    def get_model(self, model_name: str | None) -> Model:
        self.names.append(model_name)
        return self.model


def load(path: str) -> Any:
    spec = importlib.util.spec_from_file_location("dandori_io", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


async def main() -> None:
    io_file, cases_file, out_file = sys.argv[1:4]
    io = load(io_file)
    with open(cases_file, encoding="utf-8") as f:
        cases = json.load(f)
    results = []
    for c in cases:
        if "refusal" in c:
            refusal = ResponseOutputRefusal(type="refusal", refusal=c["refusal"])
            message: Any = ResponseOutputMessage(id="scripted-message", type="message", role="assistant", status="completed", content=[refusal])
        else:
            message = assistant_message(c["text"])
        model = ScriptedModel([[message]])
        provider = Provider(model)
        transport = io.transport(agents=RunConfig(model_provider=provider, tracing_disabled=True))
        out: dict[str, Any] = {}
        try:
            out["answer"] = await transport.agent(c["call"])
        except Exception as e:  # noqa: BLE001 - the error's name is what is compared
            out["error"] = type(e).__name__
        if model.calls:
            call = model.calls[0]
            schema = call.output_schema
            out["asked"] = {
                "models": provider.names,
                "calls": len(model.calls),
                "instructions": call.system_instructions,
                "input": call.input,
                "outputType": None if schema is None else {"type": "json_schema", "name": schema.name(), "strict": schema.is_strict_json_schema(), "schema": schema.json_schema()},
                "modelSettings": {k: v for k, v in call.model_settings.to_json_dict().items() if v is not None},
                "tools": [t.name for t in call.tools],
            }
        results.append(out)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")


if __name__ == "__main__":
    asyncio.run(main())
