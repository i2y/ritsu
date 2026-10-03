"""The airline domain with the rulec-generated tools: same database, same tasks, the policy
with one added section that names the tools."""
import os

from tau2.domains.airline.data_model import FlightDB
from tau2.domains.airline.environment import get_tasks, get_tasks_split
from tau2.domains.airline.utils import AIRLINE_DB_PATH, AIRLINE_POLICY_PATH
from tau2.environment.environment import Environment

from tau2_rulec.tools import RulecAirlineTools

DOMAIN = "airline_rulec"
POLICY_EXTRA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "policy_rulec.md")


def get_environment(db=None, solo_mode: bool = False) -> Environment:
    if solo_mode:
        raise ValueError("Airline domain does not support solo mode")
    if db is None:
        db = FlightDB.load(AIRLINE_DB_PATH)
    with open(AIRLINE_POLICY_PATH, encoding="utf-8") as fp:
        policy = fp.read()
    with open(POLICY_EXTRA, encoding="utf-8") as fp:
        policy = policy.rstrip("\n") + "\n\n" + fp.read()
    return Environment(domain_name=DOMAIN, policy=policy, tools=RulecAirlineTools(db))


def register(registry) -> None:
    registry.register_domain(get_environment, DOMAIN)
    registry.register_tasks(get_tasks, DOMAIN)
    for name in ("register_task_split", "register_task_splits", "register_tasks_split"):
        if hasattr(registry, name):
            getattr(registry, name)(get_tasks_split, DOMAIN)
            break
