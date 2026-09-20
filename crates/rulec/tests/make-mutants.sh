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

# --- Labels and tables that share an output (§15.66)
s="$C/印紙税の本則と軽減.rule"
# Two rows of one table with the same label
awk '/^r4 / { sub(/^r4 /, "r3 ") } { print }'                          "$s" > "$M/m_e034.rule"
# `overrides` naming a table that does not exist
awk '/^overrides 本則$/ { sub(/本則/, "本則の表") } { print }'            "$s" > "$M/m_e035.rule"
# A precedence over a row the rows of this table never meet
awk '/^overrides 本則$/ { sub(/本則/, "本則, 本則:記載なし") } { print }'  "$s" > "$M/m_w117.rule"
# A clause without its `when` line
awk '!/^  when 注文金額/ { print }'                                      "$C/送料のただし書.rule" > "$M/m_e046.rule"

# --- Sources (§15.68). The copies live beside the corpus, so the mutants sit there too.
# A cited fragment with its pin line removed
awk '!/^  第91条 sha256:/ { print }'                                     "$s" > "$M/m_e037.rule"
# A pin whose digest is not the copy's
awk '/^  第91条 sha256:/ { sub(/sha256:[0-9a-f]+/, "sha256:0000000000000000") } { print }' "$s" > "$M/m_e038.rule"
# A citation of a fragment that has no copy
awk '/^table 軽減/ { sub(/@措置法 第91条/, "@措置法 第92条") } { print }'  "$s" > "$M/m_e039.rule"
# A pin no citation uses
awk '{ print } /^  第91条 sha256:/ { print "  第92条 sha256:0000000000000000" }' "$s" > "$M/m_w119.rule"

# --- A rule applied by another (§15.69). The callee stays in the corpus, so the
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

# --- A value that meets a declared type. Every one of these produced no diagnostic at all
# until §15.86: the corpus is made of correct rules, so a position nothing checks and a
# position that checks out look the same. One seed per position that was found unchecked.
f="$C/全国運賃.rule"
n="$C/納入先照合.rule"
# The expected value of an example, in the wrong unit. It was compared by not comparing it.
awk '/^\| 近い一件 / { sub(/800円/, "800g") } { print }'                 "$f" > "$M/m_e103ex.rule"
# The grid an output is rounded to, in the wrong unit. It fell back on a grid of one yen.
awk '/^  運賃\(fee\)/ { sub(/round up\(1円\)/, "round up(1g)") } { print }' "$f" > "$M/m_e103round.rule"
# The answer a fold gives for an empty sequence, in the wrong unit. 銭 was read as 円.
awk '/^  empty/ { sub(/0円/, "999銭") } { print }'                      "$f" > "$M/m_e103fold.rule"
# The range of an element's field. Its diagnostics were dropped on the floor.
awk '/^  閾値\(threshold\)/ { sub(/<=100万円/, "<=100万g") } { print }'   "$f" > "$M/m_e103elem.rule"
# A group member that is a value of no enum. It was dropped in silence, leaving the group
# one value smaller and the value it meant to hold falling through to the catch-all row.
y="$C/ゆうパック運賃.rule"
awk '/^group 近畿圏/ { sub(/大阪府/, "大阪") } { print }'                   "$y" > "$M/m_e012group.rule"
# A step, in the wrong unit. It was taken as 1, so the runtime value counted whole units.
awk '/^  料率\(rate\)/ { sub(/step 0.1%/, "step 0.1g") } { print }'      "$C/厚生年金保険料.rule" > "$M/m_e103step.rule"

# --- The walk and the count. Neither shape was in the mutants at all, so the diagnostics
# that hold a `fold` and a `count` together (§15.56, §15.58) were exercised only by their own
# minimal examples in the ledger, never by a rule a business would write.
# `over` names a sequence that is not declared
awk '{ sub(/^fold 採用 over 運賃行/, "fold 採用 over 無い並び"); print }'  "$f" > "$M/m_e021.rule"
# No answer for an empty sequence
awk '!/^  empty/ { print }'                                            "$f" > "$M/m_e022.rule"
# No answer for a walk that reached the end
awk '!/^  exhausted/ { print }'                                        "$f" > "$M/m_e023.rule"
# A verdict the table produces, with no arm
awk '!/^  スキップ/ { print }'                                          "$f" > "$M/m_e024.rule"
# An example that names a sequence nothing declares
awk '{ sub(/^\| 近い一件 \|/, "| 無い並び |"); print }'                   "$f" > "$M/m_e027.rule"
# A count with no `over`
awk '{ sub(/^count 一致数\(hits\) over 候補 /, "count 一致数(hits) "); print }' "$n" > "$M/m_e028.rule"
# A count whose `where` names a column an element does not have
awk '{ sub(/where 照合結果 = 一致/, "where 会社名一致 = 一致"); print }'      "$n" > "$M/m_e029.rule"
# A count with no range: the universe of the completeness check, and the cap on the sequence
awk '{ sub(/  range >=0 <=50/, ""); print }'                            "$n" > "$M/m_e030.rule"
# A rule that both folds and counts the same walk
awk '{ print } /^count 一致数/ { print ""; print "fold 照合結果 over 候補"; print "  一致    -> next"; print "  不一致  -> next"; print "  empty     -> false"; print "  exhausted -> false" }' "$n" > "$M/m_e031.rule"

# --- The expression and the result line.
# An expression in an output cell
awk '{ sub(/\| 1200円  \|$/, "| 600円 × 2 |"); print }'                 "$C/送料.rule" > "$M/m_e014.rule"
# A second `result` line
awk '{ print } /^result /{ print "result 送料 = 送料" }'                 "$C/送料.rule" > "$M/m_e016.rule"
# A divisor that is not a constant. Until §15.88 this **panicked** `rulec check`: the
# interval of the derived value was computed before the diagnostic, and a divisor whose
# range contains zero asserted its way out of the process.
awk '{ sub(/税込金額 ÷ 100円/, "税込金額 ÷ 税込金額"); print }'            "$C/ポイント付与.rule" > "$M/m_e115.rule"

ls "$M" | wc -l | tr -d ' ' | xargs echo "変異ファイル:"
