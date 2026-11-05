// A calendar, asked in the page: what `Calendar::is_open` and `Calendar::reasons` answer, from the
// calendar's own lines. tests/doc.rs holds it to them over every day of the examples.
function tryCalendar(D) {
  var rows = new Map(D.holidays.map(function (h) {
    return [h[0], h];
  }));
  // Day 0 is 1970-01-01, as in date.rs.
  function civil(z) {
    z += 719468;
    var era = Math.floor(z / 146097);
    var doe = z - era * 146097;
    var yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
    var doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
    var mp = Math.floor((5 * doy + 2) / 153);
    var m = mp < 10 ? mp + 3 : mp - 9;
    return [yoe + era * 400 + (m <= 2 ? 1 : 0), m, doy - Math.floor((153 * mp + 2) / 5) + 1];
  }
  function dayOf(y, m, d) {
    y -= m <= 2 ? 1 : 0;
    var era = Math.floor(y / 400);
    var yoe = y - era * 400;
    var doy = Math.floor((153 * (m > 2 ? m - 3 : m + 9) + 2) / 5) + d - 1;
    return era * 146097 + yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy - 719468;
  }
  // Monday is 0.
  function weekday(z) {
    return (((z + 3) % 7) + 7) % 7;
  }
  function iso(z) {
    var c = civil(z);
    return String(c[0]).padStart(4, "0") + "-" + String(c[1]).padStart(2, "0") + "-" + String(c[2]).padStart(2, "0");
  }
  function parse(s) {
    var m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s || "");
    if (!m) return null;
    var z = dayOf(+m[1], +m[2], +m[3]);
    return iso(z) === s ? z : null;
  }
  function closedBy(z) {
    var out = [];
    var c = civil(z);
    var x = c[1] * 100 + c[2];
    if (D.weekly[weekday(z)]) out.push({ kind: "weekly", text: D.weekdays[weekday(z)] });
    var h = rows.get(z);
    if (h) out.push({ kind: "table", text: h[2], table: D.tables[h[1]] });
    D.every.forEach(function (e) {
      var from = e[0] * 100 + e[1];
      var to = e[2] * 100 + e[3];
      if (from <= to ? from <= x && x <= to : x >= from || x <= to) out.push({ kind: "every", text: e[4] });
    });
    D.days.forEach(function (s) {
      if (s[0] <= z && z <= s[1]) out.push({ kind: "day", text: s[2] });
    });
    return out;
  }
  function answer(z) {
    if (z < D.from || z > D.to) return { known: false, open: false, reasons: [], opened: null };
    var rules = closedBy(z);
    var line = D.opens.filter(function (o) {
      return o[0] <= z && z <= o[1];
    })[0];
    if (line) return { known: true, open: true, reasons: [], opened: rules.length ? line[2] : null };
    return { known: true, open: rules.length === 0, reasons: rules, opened: null };
  }
  return { answer: answer, parse: parse, iso: iso, weekday: weekday, dayOf: dayOf };
}
