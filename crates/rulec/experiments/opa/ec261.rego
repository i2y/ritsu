# Article 7 of Regulation (EC) No 261/2004 in Rego: one rule per row of the three tables,
# rounding through rulec.round. Transcribed from tests/corpus/ec261.rule by hand.
package rulec.ec261

import data.rulec.round

_band(d, e) := "short" if d <= 1500
_band(d, e) := "medium" if { d > 1500; e == true }
_band(d, e) := "medium" if { d > 1500; d <= 3500; e == false }
_band(d, e) := "long" if { d > 3500; e == false }

_base("short") := 250
_base("medium") := 400
_base("long") := 600

_factor(d, e, t) := 1 if { d <= 1500; t <= 2 }
_factor(d, e, t) := 2 if { d <= 1500; t > 2 }
_factor(d, e, t) := 1 if { d > 1500; e == true; t <= 3 }
_factor(d, e, t) := 2 if { d > 1500; e == true; t > 3 }
_factor(d, e, t) := 1 if { d > 1500; d <= 3500; e == false; t <= 3 }
_factor(d, e, t) := 2 if { d > 1500; d <= 3500; e == false; t > 3 }
_factor(d, e, t) := 1 if { d > 3500; e == false; t <= 4 }
_factor(d, e, t) := 2 if { d > 3500; e == false; t > 4 }

# The entry guard. Out of range returns an error object rather than no value: Rego has no
# exceptions, and an undefined answer is indistinguishable from a denial at the caller.
result(d, e, t) := {"error": sprintf("distance が範囲の外です: %v", [d])} if {
	d < 1
} else := {"error": sprintf("distance が範囲の外です: %v", [d])} if {
	d > 20000
} else := {"error": sprintf("delay が範囲の外です: %v", [t])} if {
	t < 0
} else := {"error": sprintf("delay が範囲の外です: %v", [t])} if {
	t > 48
} else := {"compensation": v} if {
	raw := _base(_band(d, e)) * _factor(d, e, t) # held in halves of a EUR
	v := round.apply("down", raw, 2) / 2 # round down(1EUR), then divide the scale out
}

# The same arithmetic with the division transcribed literally, the way the generated SQL
# writes it. Rego's `/` is exact rational and does not truncate, so this is wrong — and the
# rule's own vectors do not catch it.
naive(d, e, t) := (((raw / 2) * 2) / 2) if {
	raw := _base(_band(d, e)) * _factor(d, e, t)
}

failures contains {"in": v.in, "want": v.out.compensation, "got": got} if {
	some v in input.vectors
	got := result(v.in.distance, v.in.intra_eu, v.in.delay).compensation
	got != v.out.compensation
}

answered contains i if {
	some i, v in input.vectors
	result(v.in.distance, v.in.intra_eu, v.in.delay).compensation
}
