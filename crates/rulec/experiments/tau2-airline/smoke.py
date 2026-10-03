"""Stand the environment up without a model and call the three tools."""
import json
from tau2.registry import registry

env = registry.get_env_constructor("airline_rulec")()
tools = env.get_tools()
names = [getattr(t, "name", None) or t.get("name") for t in tools]
print("domain:", env.domain_name, "| tools:", len(tools))
print("new:", [n for n in names if n in ("baggage_fee", "cancellation_verdict", "compensation_amount")])
t = [t for t in tools if (getattr(t, "name", None) or t.get("name")) == "baggage_fee"][0]
dump = t.model_dump() if hasattr(t, "model_dump") else t
print(json.dumps(dump, ensure_ascii=False, default=str)[:1200])
print(env.tools.use_tool("baggage_fee", membership="gold", cabin="business", passengers=2, bags=9))
print(env.tools.use_tool("cancellation_verdict", flown=False, within_24h=False, airline_cancelled=False, cabin="economy", insured=True, reason="weather"))
print(env.tools.use_tool("compensation_amount", membership="regular", cabin="economy", insured=False, complaint="cancelled", wants_change_or_cancel=False, passengers=2))
try:
    print(env.tools.use_tool("cancellation_verdict", flown=False, within_24h=False, airline_cancelled=False, cabin="economy", insured=True, reason="sick"))
except Exception as e:
    print("refused:", e)
print("policy tail:", env.policy[-300:].replace("\n", " ")[:300])
