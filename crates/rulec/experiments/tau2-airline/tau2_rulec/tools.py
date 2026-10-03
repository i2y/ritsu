"""The airline tools, plus the three decisions the policy states as tables.

The three functions are the Python `rulec gen` wrote from tests/corpus/預け荷物料金.rule,
予約取消可否.rule and 補償証明書.rule. Each tool returns the answer and the rows of the table
that decided it, so a transcript shows where a decision came from.
"""
import json
import os
import sys

GEN = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "gen", "python")
if GEN not in sys.path:
    sys.path.insert(0, GEN)

import baggage_fee as _bf  # noqa: E402
import cancel_verdict as _cv  # noqa: E402
import compensation as _cp  # noqa: E402
from tau2.domains.airline.tools import AirlineTools  # noqa: E402
from tau2.environment.toolkit import ToolType, is_tool  # noqa: E402

# Each generated module declares its own enum classes, and each function checks its inputs
# against its own, so the vocabulary is mapped per module.
MEMBERSHIP_BF = {m.value: m for m in _bf.Membership}
CABIN_BF = {c.value: c for c in _bf.Cabin}
CABIN_CV = {c.value: c for c in _cv.Cabin}
MEMBERSHIP_CP = {m.value: m for m in _cp.Membership}
CABIN_CP = {c.value: c for c in _cp.Cabin}
REASON = {
    "change_of_plan": _cv.Reason.CHANGEOFPLAN,
    "health": _cv.Reason.HEALTH,
    "weather": _cv.Reason.WEATHER,
    "other": _cv.Reason.OTHER,
}
COMPLAINT = {"cancelled": _cp.Complaint.CANCELLED, "delayed": _cp.Complaint.DELAYED, "other": _cp.Complaint.OTHER}
VERDICT = {_cv.Verdict.ALLOWED: "allowed", _cv.Verdict.REFUSED: "refused", _cv.Verdict.TRANSFER: "transfer_to_human"}


def _pick(table: dict, value, what: str):
    try:
        return table[value]
    except KeyError:
        raise ValueError(f"{what} must be one of {', '.join(repr(k) for k in table)}, not {value!r}")


def _rows(trace) -> list[str]:
    return [f"{f.table} row {f.row}" for f in trace]


class RulecAirlineTools(AirlineTools):
    """All the airline tools, and three that decide the policy's tables."""

    @is_tool(ToolType.READ)
    def baggage_fee(self, membership: str, cabin: str, passengers: int, bags: int) -> str:
        """
        Compute the checked baggage fee for a reservation from the policy's allowance table. Use it instead of working the allowance out yourself.

        Args:
            membership: The booking user's membership level: 'regular', 'silver' or 'gold'.
            cabin: The cabin class of the reservation: 'basic_economy', 'economy' or 'business'.
            passengers: The number of passengers on the reservation, 1 to 5.
            bags: The total number of checked bags for the whole reservation, 0 to 50.

        Returns:
            A JSON string with the fee in dollars and the policy rows that decided it.

        Raises:
            ValueError: If an argument is outside the policy's vocabulary or range.
        """
        m = _pick(MEMBERSHIP_BF, membership, "membership")
        c = _pick(CABIN_BF, cabin, "cabin")
        try:
            out, trace = _bf.baggage_fee_traced(m, c, int(passengers), int(bags))
        except _bf.RuleInputError as e:
            raise ValueError(str(e))
        return json.dumps({"fee_usd": int(out), "decided_by": _rows(trace)})

    @is_tool(ToolType.READ)
    def cancellation_verdict(
        self, flown: bool, within_24h: bool, airline_cancelled: bool, cabin: str, insured: bool, reason: str
    ) -> str:
        """
        Decide from the policy whether a reservation can be cancelled. Use it instead of applying the cancellation rules yourself.

        Args:
            flown: Whether any flight segment of the reservation has already been flown.
            within_24h: Whether the reservation was booked within the last 24 hours.
            airline_cancelled: Whether the airline cancelled a flight of the reservation (from the flight status, not from the user's words).
            cabin: The cabin class of the reservation: 'basic_economy', 'economy' or 'business'.
            insured: Whether the reservation has travel insurance.
            reason: The user's reason for cancelling: 'change_of_plan', 'health', 'weather' or 'other'.

        Returns:
            A JSON string with the verdict ('allowed', 'refused' or 'transfer_to_human') and the policy row that decided it.

        Raises:
            ValueError: If an argument is outside the policy's vocabulary.
        """
        c = _pick(CABIN_CV, cabin, "cabin")
        r = _pick(REASON, reason, "reason")
        out, trace = _cv.cancel_verdict_traced(bool(flown), bool(within_24h), bool(airline_cancelled), c, bool(insured), r)
        return json.dumps({"verdict": VERDICT[out], "decided_by": _rows(trace)})

    @is_tool(ToolType.READ)
    def compensation_amount(
        self, membership: str, cabin: str, insured: bool, complaint: str, wants_change_or_cancel: bool, passengers: int
    ) -> str:
        """
        Compute the compensation certificate amount the policy allows for a complaint, in dollars. 0 means the policy allows none. Use it instead of applying the compensation rules yourself.

        Args:
            membership: The user's membership level: 'regular', 'silver' or 'gold'.
            cabin: The cabin class of the reservation: 'basic_economy', 'economy' or 'business'.
            insured: Whether the reservation has travel insurance.
            complaint: What the user complains about: 'cancelled' for a cancelled flight, 'delayed' for a delayed flight, or 'other'.
            wants_change_or_cancel: For a delayed flight, whether the user wants to change or cancel the reservation.
            passengers: The number of passengers on the reservation, 1 to 5.

        Returns:
            A JSON string with the amount in dollars and the policy rows that decided it.

        Raises:
            ValueError: If an argument is outside the policy's vocabulary or range.
        """
        m = _pick(MEMBERSHIP_CP, membership, "membership")
        c = _pick(CABIN_CP, cabin, "cabin")
        k = _pick(COMPLAINT, complaint, "complaint")
        try:
            out, trace = _cp.compensation_traced(m, c, bool(insured), k, bool(wants_change_or_cancel), int(passengers))
        except _cp.RuleInputError as e:
            raise ValueError(str(e))
        return json.dumps({"amount_usd": int(out), "decided_by": _rows(trace)})
