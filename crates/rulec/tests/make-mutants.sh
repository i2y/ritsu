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
awk '{ sub(/, 山梨県/, ""); print }'                                 "$y" > "$M/m_e101.rule"
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

# --- Labels and tables that share an output (DESIGN-draft §2)
s="$C/印紙税の本則と軽減.rule"
# Two rows of one table with the same label
awk '/^r4 / { sub(/^r4 /, "r3 ") } { print }'                          "$s" > "$M/m_e034.rule"
# `overrides` naming a table that does not exist
awk '/^overrides 本則$/ { sub(/本則/, "本則の表") } { print }'            "$s" > "$M/m_e035.rule"
# A precedence over a row the rows of this table never meet
awk '/^overrides 本則$/ { sub(/本則/, "本則, 本則:記載なし") } { print }'  "$s" > "$M/m_w117.rule"
# A clause without its `when` line
awk '!/^  when 注文金額/ { print }'                                      "$C/送料のただし書.rule" > "$M/m_e046.rule"

# --- Sources (DESIGN-draft §3). The copies live beside the corpus, so the mutants sit there too.
# A cited fragment with its pin line removed
awk '!/^  第91条 sha256:/ { print }'                                     "$s" > "$M/m_e037.rule"
# A pin whose digest is not the copy's
awk '/^  第91条 sha256:/ { sub(/sha256:[0-9a-f]+/, "sha256:0000000000000000") } { print }' "$s" > "$M/m_e038.rule"
# A citation of a fragment that has no copy
awk '/^table 軽減/ { sub(/@措置法 第91条/, "@措置法 第92条") } { print }'  "$s" > "$M/m_e039.rule"
# A pin no citation uses
awk '{ print } /^  第91条 sha256:/ { print "  第92条 sha256:0000000000000000" }' "$s" > "$M/m_w119.rule"

# --- A rule applied by another (DESIGN-draft §5). The callee stays in the corpus, so the
# mutants reach it by a relative path; the digest in the heading is the corpus caller's own.
a="$C/非常勤退職手当.rule"
callee='"退職手当.rule"'
via='"../corpus/退職手当.rule"'
# A pinned digest the callee no longer has
awk -v c="$callee" -v v="$via" '/^apply / { sub(c, v); sub(/sha256:[0-9a-f]+/, "sha256:0000000000000000") } { print }' "$a" > "$M/m_e040.rule"
# A callee input left unbound
awk -v c="$callee" -v v="$via" '/^apply / { sub(c, v) } !/^  基本給 = / { print }' "$a" > "$M/m_e041.rule"
# A value of this rule's enum that stands for no value of the callee's
awk -v c="$callee" -v v="$via" '/^apply / { sub(c, v) } { sub(/, 辞職 -> 自己都合/, ""); print }' "$a" > "$M/m_e042.rule"
# A range that reaches outside the callee's
awk -v c="$callee" -v v="$via" '/^apply / { sub(c, v) } { sub(/range >=1 <=3/, "range >=0 <=3"); print }' "$a" > "$M/m_e043.rule"
# A callee that does not pass check (the E101 mutant, made above)
h=$(shasum -a 256 "$M/m_e101.rule" | cut -c1-16)
awk -v c="$callee" -v h="$h" '/^apply / { sub(c, "\"m_e101.rule\""); sub(/sha256:[0-9a-f]+/, "sha256:" h) } { print }' "$a" > "$M/m_e044.rule"
# A clause of this rule that takes precedence over the whole applied table
awk -v c="$callee" -v v="$via" '/^apply / { sub(c, v) } { print } /^  手当 -> 非常勤手当$/ { print ""; print "clause 特例(special) -> 退職手当:支給月数"; print "  when always"; print "  then 5"; print "  overrides 退職手当:支給表" }' "$a" > "$M/m_w118.rule"

ls "$M" | wc -l | tr -d ' ' | xargs echo "変異ファイル:"
