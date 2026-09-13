#!/bin/sh
# Rebuild the mutant files from the corpus. Material for confirming that a single seeded error
# produces the decided code and wording (§13).
#
# Lines are identified by content alone. Depending on whitespace would make the substitution miss
# silently the moment `rulec fmt` changes a column width, turning a mutant into an "unbroken file".
set -eu
cd "$(dirname "$0")/.."
C=tests/corpus
M=tests/mutants
mkdir -p "$M"
y="$C/ゆうパック運賃.rule"
k="$C/クーポン割引.rule"

# --- Syntax
awk '/<=60cm/ { sub(/<=60cm/, "..60cm") } { print }'                  "$y" > "$M/m_e010.rule"
awk '/\| S80 /  { sub(/S80/, "   ") } { print }'                       "$y" > "$M/m_e008.rule"
awk '/あて先\(dest\)/ { sub(/\(dest\)/, "      ") } { print }'          "$y" > "$M/m_e011.rule"

# --- Table checks
awk '{ sub(/ ・ 山梨県/, ""); print }'                                 "$y" > "$M/m_e101.rule"
awk '{ print } /^\| *- *\| *S170 /{ print "| <=60cm | S60 |" }'        "$y" > "$M/m_e102.rule"
awk '/<=60cm/ { sub(/<=60cm/, "<=1200円") } { print }'                 "$y" > "$M/m_e103.rule"
awk '/運賃\(fee\)/ { sub(/  round up\(10円\)/, "") } { print }'    "$y" > "$M/m_e104.rule"
awk '/中国四国/ && /S60/ && /1150円/ { sub(/中国四国/, "近距離圏") } { print }' "$y" > "$M/m_e105.rule"
awk '/1450円/ { sub(/1450円/, "1451円") } { print }'                   "$y" > "$M/m_e106.rule"
awk '/沖縄県/ && /100cm/ { sub(/2160円/, "2170円") } { print }'         "$y" > "$M/m_e107.rule"
awk '/重量\(weight\)/ { sub(/  contract_only/, "") } { print }'              "$y" > "$M/m_w111.rule"

# --- Types, rounding, and derivations
awk '/元価\(list\)/ { sub(/<=1000万円/, "<=100000000000000000円") } { print }' "$k" > "$M/m_e108.rule"
awk '/最低差\(min_gap\)/ { sub(/>=-100万円/, ">=0円") } { print }'      "$k" > "$M/m_e112.rule"
awk '/^result 割引額/ { print "define 得か(is_good) : bool = 素割引 >= 上限額"; print "" } { print }' "$k" > "$M/m_e113.rule"
awk '/割引額\(discount\)/ { sub(/  round down\(1円\)/, "") }
     /^result 割引額/     { sub(/down\(min\(素割引, 上限額\), 1円\)/, "min(素割引, 上限額)") }
     { print }'                                                        "$k" > "$M/m_e104b.rule"
awk '/<0円/ { sub(/false/, "true") } { print }'                             "$k" > "$M/m_w105.rule"
# Once the upstream table no longer emits true, the downstream row that names it becomes
# unreachable (a variant of E102)
awk '/table 適用可否/,/^$/ { if (/\| *true *\|$/) sub(/true *\|$/, "false |") } { print }' "$k" > "$M/m_e102b.rule"

# Open a hole at a date boundary
# Remove a single day from a closed-interval tiling. Without day-count representation this is a
# false positive.
awk '{ sub(/>=2026-04-01 <=2026-06-30/, ">=2026-04-02 <=2026-06-30"); print }' "$C/期間区分.rule" > "$M/m_e101d.rule"

# Drop an example's output column. It is the only wedge that breaks the blind spot of three-way
# agreement, so a missing one is an error (§1.2).
awk '{ sub(/ 素割引 \|$/, "|"); print }'                               "$C/クーポン一枚.rule" > "$M/m_e111.rule"

ls "$M" | wc -l | tr -d ' ' | xargs echo "変異ファイル:"
