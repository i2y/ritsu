permit (principal, action, resource)
when {
  decimal("1.23").lessThan(decimal("2.5")) && decimal("1.0").lessThanOrEqual(decimal("1.0")) &&
  decimal("3.0").greaterThan(decimal("2.0")) && decimal("3.0").greaterThanOrEqual(decimal("3.0")) &&
  ip("10.0.0.1").isInRange(ip("10.0.0.0/8")) && ip("127.0.0.1").isLoopback() &&
  ip("::1").isIpv6() && ip("10.0.0.1").isIpv4() && ip("224.0.0.1").isMulticast() &&
  datetime("2026-10-06T07:00:00Z") < datetime("2026-10-06T08:00:00Z") &&
  datetime("2026-10-06").offset(duration("1h")) > datetime("2026-10-06") &&
  datetime("2026-10-06T10:00:00Z").durationSince(datetime("2026-10-06")) == duration("10h") &&
  datetime("2026-10-06T10:00:00Z").toDate() == datetime("2026-10-06") &&
  datetime("2026-10-06T10:00:00Z").toTime() == duration("10h") &&
  duration("1d2h3m4s5ms").toMilliseconds() > 0 && duration("90s").toSeconds() == 90 &&
  duration("90m").toMinutes() == 90 && duration("2h").toHours() == 2 && duration("3d").toDays() == 3 &&
  context.when < datetime("2027-01-01") && unknown("pending")
};
