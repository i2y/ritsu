#!/bin/sh
# Rebuild the mutant files from the corpus. Material for confirming that a single seeded error
# produces the decided code and wording (§13).
#
# Lines are identified by content alone. Depending on whitespace would make the substitution miss
# silently the moment `rulec fmt` changes a column width, turning a mutant into an "unbroken file".
#
# The files it writes are committed, like the diagrams and the playground's wasm, and like them
# they are a build product: `変異はコーパスから作り直せる` in tests/m0.rs re-runs this into a
# temporary directory and holds the committed copies to it, so a corpus rule that moves on
# leaves a failing test rather than a mutant that is quietly no longer that rule with one
# seeded error. `$1` is where to write, for that test.
set -eu
cd "$(dirname "$0")/.."
C=tests/corpus
M=${1:-tests/mutants}
mkdir -p "$M"
y="$C/ゆうパック運賃.rule"
k="$C/クーポン割引.rule"

# --- Syntax
awk '/<=60cm/ { sub(/<=60cm/, "..60cm") } { print }'                  "$y" > "$M/m_e010.rule"
awk '/\| S80 /  { sub(/S80/, "   ") } { print }'                       "$y" > "$M/m_e008.rule"
awk '/あて先\(dest\)/ { sub(/\(dest\)/, "      ") } { print }'          "$y" > "$M/m_e011.rule"

# --- Table checks
awk '{ sub(/, 山梨県/, ""); print }'                                 "$y" > "$M/m_e101.rule"
# The same defect in English, which is the one the README shows: a reader who meets the
# message first should be able to read it (§15.110).
awk '/38USD/ { next } { print }'                        "$C/parcel_rate.rule" > "$M/m_e101en.rule"
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

# Two columns above, cut from the same input at different thresholds (§15.114). `小口` is 2kg
# and under, `重量物` is over 10kg, so a row naming both is dead however live each half looks
# on its own — and the row below it, which leaves that combination out, is right to.
awk '{ print } /^\| *小口 *\| *- *\|/ { print "| 小口 | 重量物 | 700円 |" }' "$C/二つの区分.rule" > "$M/m_e102c.rule"
# The other side of the same sieve: a combination that **does** happen, left uncovered. If the
# narrowing above ever prunes too hard, this stops reporting and the generated code walks into
# its own `unreachable!`.
awk '/^\| *大口 *\| *通常 *\| *200円/ { next } { print }' "$C/二つの区分.rule" > "$M/m_e101e.rule"

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
# A boundary transcribed onto the other side of itself. Both rows that share the boundary are
# moved, so the table stays complete and has no overlap — E101 and E105 have nothing to say,
# the number 18 is still used so W120 has nothing to say, and the copy's "Under 18" is the
# only thing left that knows (§15.124).
awk '/ <18 / { sub(/<18 /, "<=18") } / >=18 <=20 / { sub(/>=18 <=20/, ">18 <=20 ") } { print }' \
  "$C/uk_minimum_wage.rule" > "$M/m_e119.rule"
# The same mistake where the copy says which side in a heading over the column (`円以上` and
# `円未満`) instead of in the cell, which is the shape a Japanese premium table takes.
awk '/\| <93000円/ { sub(/<93000円/, "<=93000円") } /\| >=93000円 </ { sub(/>=93000円/, ">93000円") } { print }' \
  "$C/厚生年金保険料.rule" > "$M/m_e119col.rule"

# --- A projection into the caller's object (§15.125). The contract stays in the corpus, so
# the mutants reach it by the same relative path the corpus rule uses.
p="$C/注文の送料.rule"
# `count` gives a number and the input that takes it is a bool
awk '/from any 注文.lines/ { sub(/any 注文.lines where chilled = true/, "count 注文.lines") } { print }' \
  "$p" > "$M/m_e120.rule"
# A field the contract does not have — what a renamed field looks like
awk '{ sub(/注文.shipping.zone/, "注文.shipping.region"); print }'  "$p" > "$M/m_e121.rule"
# The contract declared and nothing projected from it
awk '{ sub(/  from .*$/, ""); print }'                              "$p" > "$M/m_w122.rule"
# The contract lets through 50 lines and the input takes 40: an order it validated is refused (§15.132)
awk '{ sub(/range >=1 <=50  from count/, "range >=1 <=40  from count"); print }' "$p" > "$M/m_e122.rule"
# The input widened to 80 lines and a row written for more than 60, which the contract never sends
awk '/range >=1 <=50  from count/ { sub(/range >=1 <=50/, "range >=1 <=80") }
     /^\| >10 / { print "| >10 <=60 | 200円                           |"; print "| >60      | 300円                           |"; next }
     { print }' "$p" > "$M/m_w123.rule"

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

# --- Five more codes that had no mutant at all (§15.92). The syntax errors keep their
# minimal example in the ledger — a misplaced character has no amount attached to it — but
# these five change what a real table answers, or what it is allowed to say.
k="$C/期間区分.rule"
# A name that is one of the language's own words: the line-oriented parser reads the
# declaration as the start of a section.
awk '{ sub(/^  注文日\(order_date\) : date/, "  range(order_date) : date"); print }'  "$k" > "$M/m_e009.rule"
# Subtracting one date from another. §2.1 has said since the beginning that a date has no
# arithmetic, and until §15.84 nothing enforced it.
awk '{ print } /^  注文日\(order_date\)/ { print ""; print "derive 差(gap) : date = 注文日 - 注文日  range >=2026-01-01 <=2026-12-31" }' "$k" > "$M/m_e048.rule"
# A word after the range that belongs to no modifier. It used to be dropped in silence.
awk '{ sub(/range >=1cm <=170cm/, "range >=1cm <=170cm incl_tax"); print }'         "$y" > "$M/m_e047.rule"
# A rate off the step its own column declares: it has no runtime representation.
awk '{ sub(/18\.3%/, "18.35%"); print }'                          "$C/厚生年金保険料.rule" > "$M/m_e114.rule"
# `policy first` on a table whose rows do not overlap: the order is claimed to matter and
# does not, so `unique` would prove more.
awk '{ sub(/^policy unique/, "policy first"); print }'                    "$C/送料.rule" > "$M/m_w110.rule"
# `result` naming an output other than the first: `result` is sugar for the first one.
awk '{ sub(/^result 送料 =/, "result 大口 ="); print }'                     "$C/送料.rule" > "$M/m_e015.rule"

# --- The last of the seedable codes (§15.92). What is left after these keeps its minimal
# example in the ledger and nothing more: E109 needs `--budget` (it is about scale, not about
# what a rule says), E110 needs a column's type *and* its cells changed, and W114 needs two
# derived values sharing an input — a shape no transcription in the corpus has.
# An example that does not say which sequence it walks
awk '/^\| 運賃行   \| -> 運賃 \|$/ { print "| -> 運賃 |"; next } /^\| 近い一件 \|/ { print "| 800円   |"; next } /^\| 空       \|/ { print "| 0円     |"; next } { print }' "$f" > "$M/m_e025.rule"
# A `sequence` whose header does not name the element's fields
awk '{ sub(/^\| 行ゾーン \| 閾値   \| 行運賃 \|$/, "| 行ゾーン | 閾値 |"); print }'    "$f" > "$M/m_e026.rule"
# A verdict no element can land on
awk '{ sub(/^\| 遠隔地   \| <=1000円 \| スキップ                    \|/, "| 遠隔地   | <=1000円 | 打ち切り                    |"); print }' "$f" > "$M/m_w115.rule"
# `overrides` pointing at a table that defines a different output
awk '{ sub(/^  overrides 通常$/, "  overrides 運賃表"); print }'      "$C/送料のただし書.rule" > "$M/m_e036.rule"
# A clause takes precedence over a table that decides two outputs at once
awk '/^\| 金 \| / && /150%/ { print; print ""; print "clause 特例(special) -> 送料"; print "  when 帯 金"; print "  then 0円"; print "  overrides 送料表"; next } { print }' "$C/会員特典.rule" > "$M/m_e045.rule"

# --- The syntax errors (§15.93). Their minimal example lives in the ledger, but the ledger's
# rule is four lines long: what it cannot show is that the diagnostic lands on the right line
# of a sixty-line table, and that a lexer error says where it started rather than reporting
# the rest of the file. Several of these cascade on purpose, and the cascade is pinned too.
s="$C/送料.rule"
# A string that is not closed
awk '{ if (/^description /) sub(/"$/, ""); print }'                        "$s" > "$M/m_e001.rule"
# A character that can start no identifier. The lines after it are read as headless.
awk '{ sub(/^policy unique$/, "policy unique €"); print }'                 "$s" > "$M/m_e002.rule"
# The file does not begin with `rule`
awk 'NR==1 { sub(/^rule /, "ruel ") } { print }'                           "$s" > "$M/m_e003.rule"
# A line with no word at its head
awk '{ print } /^outputs$/ { print "| 1 |" }'                              "$s" > "$M/m_e004.rule"
# A word that cannot stand where it is written
awk 'END { print "overrides 運賃表" } { print }'                            "$s" > "$M/m_e005.rule"
# A declaration with no `=`
awk '{ sub(/= 一般\(basic\)/, "一般(basic)"); print }'                      "$s" > "$M/m_e006.rule"
# A policy that is not one
awk '{ sub(/^policy unique$/, "policy 適当"); print }'                      "$s" > "$M/m_e007.rule"
# An import of something that is not there
awk '{ print } /^rule /{ print ""; print "import std/無い" }'               "$s" > "$M/m_e013.rule"
# A `constraint` that is not a relation at all
awk '/^table / && !done { print "constraint 重量"; print ""; done=1 } { print }'          "$s" > "$M/m_e017.rule"
# A `constraint` with a table output on one side; it relates inputs
awk '/^table / && !done { print "constraint 重量 <= 基本送料"; print ""; done=1 } { print }' "$s" > "$M/m_e018.rule"
# An example outside what the constraint says can happen
awk '/^table / && !done { print "constraint 商品合計 <= 値引"; print ""; done=1 } { print }' "$C/会員特典.rule" > "$M/m_e019.rule"
# An `elements` line with no name
awk '{ sub(/^elements 運賃行\(freight_rows\)$/, "elements"); print }'       "$f" > "$M/m_e020.rule"

# A column of a type the region IR cannot hold. It takes two edits — the type and the cells
# that read it — because either alone is a different error; §11 calls E110 the internal
# breakwater, and what is confirmed here is that it fires before anything is skipped.
awk '{ sub(/^  キャビン\(cabin\)                : キャビン/, "  キャビン(cabin) : string"); gsub(/\| basic_economy/, "| \"basic_economy\""); gsub(/\| economy/, "| \"economy\""); gsub(/\| business/, "| \"business\""); print }' "$C/予約取消可否.rule" > "$M/m_e110.rule"

# A share with nothing to bound it: the line saying the running total stays within the
# whole is taken out, and `allocate` is no longer an allocation (§15.102).
awk '!/^constraint ここまでの定価 <= 定価合計$/ { print }' "$C/比例配分.rule" > "$M/m_e117.rule"

# A call with one argument too few. Until §15.102 the arity of a call was not checked at
# all: this passed, and the generator was the first thing to run out of cases.
awk '{ sub(/down\(min\(素割引, 上限額\), 1円\)/, "down(min(素割引), 1円)"); print }' "$C/クーポン割引.rule" > "$M/m_e118.rule"

# An alias that the target language will not take. `type` is a Rust keyword, and the
# generated Rust reads `pub fn … (type: i64, …)` — measured, it does not compile (§15.103).
awk '{ sub(/^  区分\(kind\)/, "  区分(type)"); print }' "$C/ポイント付与.rule" > "$M/m_w121.rule"

ls "$M" | wc -l | tr -d ' ' | xargs echo "変異ファイル:"
