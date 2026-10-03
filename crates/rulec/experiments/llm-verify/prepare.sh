#!/bin/sh
# Write the contexts the conditions read: the rule itself, its approver rendering, its customer
# rendering, and the boundary vectors. The official page text (ctx/ゆうパック運賃.page.txt) was
# taken from 日本郵便's 基本運賃表（東京） on 2026-09-19 and is kept as fetched.
set -e
cd "$(dirname "$0")"
RULEC=${RULEC:-../../target/release/rulec}
CORPUS=${CORPUS:-../../tests/corpus}
for r in ゆうパック運賃 会員特典; do
  cp "$CORPUS/$r.rule" "ctx/$r.rule"
  $RULEC vectors "$CORPUS/$r.rule" > "ctx/$r.vectors.jsonl"
  $RULEC doc "$CORPUS/$r.rule" --lang ja > "ctx/$r.doc.md"
  $RULEC doc "$CORPUS/$r.rule" --lang ja --audience customer > "ctx/$r.article.txt"
done
ls -la ctx
