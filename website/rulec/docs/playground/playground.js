// The playground: the checker itself, compiled to wasm32 (src/wasm.rs, §15.48).
//
// Nothing is sent anywhere. The table the reader types is handed to the module in this
// page and the answers come back from it, so the page works offline and a rule nobody is
// allowed to paste into a web form is safe to paste here.
//
// One convention crosses the boundary: every buffer begins with its own length as a
// little-endian u32. `put` writes one, `take` reads one and frees it.

const TEXT = {
  en: {
    booting: "loading the checker…",
    checked: (ms, e, w) =>
      `rulec ${VERSION} · ${ms} ms · ${e} error${e === 1 ? "" : "s"}, ${w} warning${w === 1 ? "" : "s"}`,
    passed: (ms) => `rulec ${VERSION} · ${ms} ms · passes`,
    refused: "Nothing is generated for a rule that does not pass — the findings are on the check tab.",
    generated: (n, ms) => `${n} files · ${ms} ms`,
    broken: "the checker stopped on this input and was reloaded; please report the table that did it",
    file: "file",
    board: "The approver's page is a board that wants the whole window, so it opens in a tab of its own. Edit the table and the link below is the new one.",
    open: "open the approver's page →",
  },
  ja: {
    booting: "検査器を読み込んでいます…",
    checked: (ms, e, w) => `rulec ${VERSION} ・ ${ms} ms ・ エラー ${e} 件、警告 ${w} 件`,
    passed: (ms) => `rulec ${VERSION} ・ ${ms} ms ・ 通ります`,
    refused: "検査を通らない規則からは何も生成しません。指摘は「検査」のタブにあります。",
    generated: (n, ms) => `${n} ファイル ・ ${ms} ms`,
    broken: "この入力で検査器が止まったので読み直しました。その表を報告してください",
    file: "ファイル",
    board: "承認者向けのページはウィンドウいっぱいを使うので、別のタブで開きます。表を直すと、下のリンクはその新しいほうになります。",
    open: "承認者向けのページを開く →",
  },
};

// The table on the front page's opening diagram (website/tools/overview.rule and
// overview-ja.rule). A test holds these two to those files: a playground that shows a
// different table from the picture would be a second source of the same example.
const FULL = {
  en: `rule Fee(fee) v1

enum Zone(zone) = Domestic(domestic) | Overseas(overseas)

inputs
  Destination(dest) : Zone
  Weight(weight)    : mass[g]  range >=1g <=10kg

outputs
  Fee(fee) : money[USD, incl_tax]  round up(1USD)

table FeeTable(fee_table)
policy unique
| Destination | Weight     | -> Fee(fee) : money[USD, incl_tax] |
| Domestic    | <=2kg      | 8USD                               |
| Domestic    | >2kg <=5kg | 10USD                              |
| Domestic    | >5kg       | 13USD                              |
| Overseas    | <=2kg      | 12USD                              |
| Overseas    | >5kg       | 20USD                              |
| Overseas    | >2kg <=5kg | 15USD                              |
`,
  ja: `rule 運賃(fee) v1

enum あて先区分(zone) = 近畿圏(kinki) | 遠隔地(remote)

inputs
  あて先(dest)  : あて先区分
  重量(weight)  : mass[g]  range >=1g <=10kg

outputs
  運賃(fee) : money[円, incl_tax]  round up(10円)

table 運賃表(fee_table)
policy unique
| あて先 | 重量       | -> 運賃(fee) : money[円, incl_tax] |
| 近畿圏 | <=2kg      | 800円                              |
| 近畿圏 | >2kg <=5kg | 1000円                             |
| 近畿圏 | >5kg       | 1300円                             |
| 遠隔地 | <=2kg      | 1200円                             |
| 遠隔地 | >5kg       | 2000円                             |
| 遠隔地 | >2kg <=5kg | 1500円                             |
`,
};

// The picture's missing row is the last one, so the page opens on the table without it:
// the first thing the reader sees is the gap, named with the input that falls through it.
// The other samples, one shape each: a rule decided in stages, a bigger one where two
// columns meet again further down, and one that walks a sequence the caller passes. They
// are corpus rules, copied in rather than fetched so the page stays three files, and
// `tests/website.rs` holds each one to its file — a sample that drifted from the corpus
// would be a rule nothing checks.
const SAMPLES = {
  "en:multi": `rule parcel_rate v1
description "A parcel tariff in pounds and inches, written in English. A sketch, not a transcription: the amounts are made up"

# Nothing else in the corpus is priced in USD by weight, and nothing at all reached oz, lb
# or in — an unexercised unit is an unchecked unit (§15.9).

enum size_class = envelope | small | large
enum zone = domestic | canada | overseas

group north_america = domestic, canada

inputs
  weight    : mass[lb]    range >=1lb <=70lb
  girth     : length[in]  range >=1in <=130in
  dest      : zone
  signature : bool

outputs
  fee : money[USD, incl_tax]  round up(1USD)

# One table decides the class and the next one prices it: what the first produces is a
# column of the second.
table size_of
policy first
| girth  | -> size : size_class |
| <=22in | envelope             |
| <=60in | small                |
| -      | large                |

table base_rate
policy unique
| dest          | size     | weight  | -> base : money[USD, incl_tax] |
| north_america | envelope | -       | 6USD                           |
| north_america | small    | <=160oz | 12USD                          |
| north_america | small    | >160oz  | 18USD                          |
| north_america | large    | <=160oz | 22USD                          |
| north_america | large    | >160oz  | 30USD                          |
| overseas      | envelope | -       | 16USD                          |
| overseas      | small    | -       | 38USD                          |
| overseas      | large    | -       | 60USD                          |

# A fuel surcharge is a percentage of the base, which is what the rounding on the output is
# there to settle: 12USD at 5% is 12.60USD, and up(1USD) makes that 13USD.
table fuel_rate
policy unique
| dest          | -> fuel : rate[step 1%] |
| north_america | 5%                      |
| overseas      | 12%                     |

table signature_fee
policy unique
| signature | -> extra : money[USD, incl_tax] |
| true      | 4USD                            |
| false     | 0USD                            |

result fee = base + base × fuel + extra

examples
| weight | girth | dest     | signature | -> fee |
| 5lb    | 10in  | domestic | false     | 7USD   |
| 5lb    | 40in  | canada   | false     | 13USD  |
| 20lb   | 40in  | domestic | true      | 23USD  |
| 5lb    | 10in  | overseas | false     | 18USD  |
`,
  "ja:multi": `rule 送料(shipping_fee) v4
description "設計文書 §1.2 のスケッチを規則にしたもの。真偽定義を列に使う正常系と、率の出力、default の印、policy first の要確認 1 対が出てくる"

import std/都道府県

enum 会員区分(member_kind) = 一般(basic) default | ゴールド(gold) default | プラチナ(platinum)
group 遠隔地(remote) = 北海道, 沖縄県

inputs
  届け先(dest)    : 都道府県
  重量(weight)    : mass[g]        range >=1g <=40kg
  注文金額(total) : money[円, incl_tax]  range >=0円 <=1000万円
  会員(member)    : 会員区分

outputs
  送料(fee) : money[円, incl_tax]  round up(10円)

# 真偽の定義。条件は入力ひとつを見るだけなので、展開すれば区画の足し合わせになる（§5.3、§6.2）
define 大口(bulk) : bool = 注文金額 >= 3万円

table 基本送料(base_fee)
policy unique
| 届け先      | 重量    | -> 基本送料(base) : money[円, incl_tax] |
| 遠隔地      | <=2000g | 1200円                                  |
| 遠隔地      | >2000g  | 1800円                                  |
| not: 遠隔地 | <=2000g | 800円                                   |
| not: 遠隔地 | >2000g  | 1100円                                  |

table 負担判定(payer)
policy first
| 大口 | 会員     | -> 負担率(pay_rate) : rate |
| true | -        | 0%                         |
| -    | プラチナ | 50%                        |
| -    | -        | 100%                       |

result 送料 = 基本送料 × 負担率

examples
| 届け先 | 重量  | 注文金額 | 会員     | -> 送料 |
| 沖縄県 | 2500g | 40000円  | 一般     | 0円     |
| 東京都 | 1999g | 12000円  | プラチナ | 400円   |
| 北海道 | 500g  | 5000円   | 一般     | 1200円  |
`,
  "en:big": `rule Claude利用料(claude_usage_fee) v1
description "Claude API の期間合計の利用料。モデルとトークンの種類で百万トークンあたりの単価が決まり、バッチと米国内推論の倍率を掛け、トークン数を掛けて金額が出る。出典: platform.claude.com/docs/en/about-claude/pricing（2026-09-19 に転記）"

# モデルは閉じた集合。新しいモデルを足すと、単価表に行が無い限り E033 で止まる。
enum モデル(model) = Fable5_1(fable_5_1) | Fable5(fable_5) | Opus5(opus_5) | Opus4_8(opus_4_8) | Opus4_7(opus_4_7) | Opus4_6(opus_4_6) | Opus4_5(opus_4_5) | Sonnet5(sonnet_5) | Sonnet4_6(sonnet_4_6) | Sonnet4_5(sonnet_4_5) | Haiku4_5(haiku_4_5)
enum 種類(kind) = 入力(input) | 出力(output) | キャッシュ書込5分(cache_write_5m) | キャッシュ書込1時間(cache_write_1h) | キャッシュ読出(cache_read)

group Opus帯(opus_tier)      = Opus5, Opus4_8, Opus4_7, Opus4_6, Opus4_5
group Sonnet旧帯(sonnet_old) = Sonnet4_6, Sonnet4_5

inputs
  モデル(model)      : モデル
  種類(kind)         : 種類
  バッチ(batch)      : bool
  米国内推論(us_geo) : bool
  トークン数(tokens) : number  range >=0 <=100億

outputs
  利用料(fee) : money[USDc, excl_tax]  round half_up(1USDc)

# 単価は百万トークンあたり。ページの表をそのまま写している。
table 単価表(unit_price)
policy unique
| モデル     | 種類                | -> 単価(price) : money[USDc, excl_tax] |
| Fable5_1   | 入力                | 10USD                                  |
| Fable5_1   | 出力                | 50USD                                  |
| Fable5_1   | キャッシュ書込5分   | 1250USDc                               |
| Fable5_1   | キャッシュ書込1時間 | 20USD                                  |
| Fable5_1   | キャッシュ読出      | 25USDc                                 |
| Fable5     | 入力                | 10USD                                  |
| Fable5     | 出力                | 50USD                                  |
| Fable5     | キャッシュ書込5分   | 1250USDc                               |
| Fable5     | キャッシュ書込1時間 | 20USD                                  |
| Fable5     | キャッシュ読出      | 1USD                                   |
| Opus帯     | 入力                | 5USD                                   |
| Opus帯     | 出力                | 25USD                                  |
| Opus帯     | キャッシュ書込5分   | 625USDc                                |
| Opus帯     | キャッシュ書込1時間 | 10USD                                  |
| Opus帯     | キャッシュ読出      | 50USDc                                 |
| Sonnet5    | 入力                | 2USD                                   |
| Sonnet5    | 出力                | 10USD                                  |
| Sonnet5    | キャッシュ書込5分   | 250USDc                                |
| Sonnet5    | キャッシュ書込1時間 | 4USD                                   |
| Sonnet5    | キャッシュ読出      | 20USDc                                 |
| Sonnet旧帯 | 入力                | 3USD                                   |
| Sonnet旧帯 | 出力                | 15USD                                  |
| Sonnet旧帯 | キャッシュ書込5分   | 375USDc                                |
| Sonnet旧帯 | キャッシュ書込1時間 | 6USD                                   |
| Sonnet旧帯 | キャッシュ読出      | 30USDc                                 |
| Haiku4_5   | 入力                | 1USD                                   |
| Haiku4_5   | 出力                | 5USD                                   |
| Haiku4_5   | キャッシュ書込5分   | 125USDc                                |
| Haiku4_5   | キャッシュ書込1時間 | 2USD                                   |
| Haiku4_5   | キャッシュ読出      | 10USDc                                 |

# バッチは入力・出力・キャッシュのすべてに半額で効く（ページの「multipliers stack」）。
table バッチ割引(batch_discount)
policy unique
| バッチ | -> バッチ率(batch_rate) : rate[step 10%] |
| true   | 50%                                      |
| false  | 100%                                     |

# inference_geo を us にすると 4.6 以降のモデルで全区分に 1.1 倍。
table 地域倍率(geo_multiplier)
policy unique
| 米国内推論 | -> 地域率(geo_rate) : rate[step 10%] |
| true       | 110%                                 |
| false      | 100%                                 |

define 適用単価(applied) : money[USDc, excl_tax] = 単価 × バッチ率 × 地域率
define 利用料(fee) : money[USDc, excl_tax] = トークン数 × 適用単価 ÷ 100万

examples
| モデル   | 種類           | バッチ | 米国内推論 | トークン数 | -> 利用料 |
| Opus5    | 入力           | false  | false      | 100万      | 5USD      |
| Sonnet5  | 出力           | true   | false      | 100万      | 5USD      |
| Fable5_1 | キャッシュ読出 | false  | true       | 100万      | 28USDc    |
| Haiku4_5 | 入力           | false  | false      | 3700       | 0USDc     |
| Opus5    | 出力           | true   | true       | 15000      | 21USDc    |
`,
  "ja:big": `rule クーポン割引(coupon_discount) v1
description "クーポン1枚の適用可否と割引額。順序と反復は呼び出し側。出典: 楽天/Yahoo の公開ヘルプから再構成"
# ※ 出力セルに名前を書ける（§3.2 で採用済み）。式は書けないので、率引きは定義を一枚挟む

enum 種別(kind)  = 率引き(rate) | 額引き(amount) | 送料無料(free_ship)
enum 適用範囲(scope) = 店内全商品(shop) default | 対象商品(item)

inputs
  元価(list)        : money[円,incl_tax]  range >=0円 <=1000万円   # 率引きの基準
  現在残額(running) : money[円,incl_tax]  range >=0円 <=1000万円   # 額引きの基準（率引き適用後）
  クーポン種別(kind) : 種別
  クーポン範囲(scope): 適用範囲
  割引率(rate)      : rate[step 1%]
  割引額面(face)    : money[円,incl_tax]  range >=0円 <=100万円
  上限額(cap)       : money[円,incl_tax]  range >=0円 <=100万円
  最低購入額(min)   : money[円,incl_tax]  range >=0円 <=100万円
  同商品適用済(used): bool
  期限内(alive)     : bool

outputs
  割引額(discount) : money[円,incl_tax]  round down(1円)

# [導出] 実表が要求した形。どちらも「入力 − 入力」の線形結合
derive 最低差(min_gap) : money[円,incl_tax] = 現在残額 − 最低購入額  range >=-100万円 <=1000万円
derive 支払差(pay_gap) : money[円,incl_tax] = 現在残額 − 割引額面    range >=-100万円 <=1000万円

table 適用可否(applicable)
policy first
| 期限内 | 同商品適用済 | クーポン範囲 | クーポン種別 | 最低差 | 支払差 | -> 可否(ok) : bool |
| false  | -            | -            | -            | -      | -      | false              |
| -      | true         | 対象商品     | -            | -      | -      | false              |
| -      | -            | -            | -            | <0円   | -      | false              |
| -      | -            | -            | 額引き       | -      | <=0円  | false              |
| -      | -            | -            | -            | -      | -      | true               |

# 率引きは元価基準、額引きは割引後（現在残額）基準 — 楽天の原文どおり
define 率割引(rate_off) : money[円,incl_tax] = 元価 × 割引率

table 素の割引(raw_discount)
policy unique
| 可否  | クーポン種別 | -> 素割引(raw) : money[円,incl_tax] |
| false | -            | 0円                                 |
| true  | 率引き       | 率割引                              |
| true  | 額引き       | 割引額面                            |
| true  | 送料無料     | 0円                                 |

result 割引額 = down(min(素割引, 上限額), 1円)

examples
| 元価    | 現在残額 | クーポン種別 | クーポン範囲 | 割引率 | 割引額面 | 上限額 | 最低購入額 | 同商品適用済 | 期限内 | -> 割引額 |
| 10000円 | 10000円  | 率引き       | 店内全商品   | 10%    | 0円      | 1000円 | 5000円     | false        | true   | 1000円    |
| 10000円 | 9000円   | 額引き       | 店内全商品   | 0%     | 500円    | 500円  | 5000円     | false        | true   | 500円     |
| 10000円 | 400円    | 額引き       | 店内全商品   | 0%     | 500円    | 500円  | 0円        | false        | true   | 0円       |
`,
  "en:walk": `rule shipment_surcharge v1
description "A surcharge decided from the total weight of a shipment's cartons. A sketch, not a transcription: the amounts are made up"

# Every rule that walks a sequence was written in Japanese, so the elements/sum shape had
# never been read in English — its diagnostics and its generated code were exercised in one
# language only. Writing this one found a brand the generators never defined (DESIGN).

enum service = ground | express

inputs
  svc         : service
  residential : bool

# One carton. The caller passes as many as the shipment has.
elements cartons
  weight : mass[lb]  range >=1lb <=150lb

# What is left after the walk is the total. The range is the universe completeness is
# checked against, and it is also the door the caller is turned away at.
sum total over cartons of weight  range >=0lb <=2000lb

outputs
  surcharge : money[USD, incl_tax]  round up(1USD)

table surcharge_table
policy unique
| total   | svc     | residential | -> surcharge(surcharge) : money[USD, incl_tax] |
| <=150lb | ground  | false       | 0USD                                           |
| <=150lb | ground  | true        | 4USD                                           |
| <=150lb | express | -           | 12USD                                          |
| >150lb  | ground  | -           | 35USD                                          |
| >150lb  | express | -           | 60USD                                          |

examples
| cartons | svc     | residential | -> surcharge |
| two     | ground  | false       | 0USD         |
| two     | ground  | true        | 4USD         |
| two     | express | false       | 12USD        |
| heavy   | ground  | false       | 35USD        |
| heavy   | express | true        | 60USD        |
| empty   | ground  | false       | 0USD         |

sequence two
| weight |
| 40lb   |
| 25lb   |

sequence heavy
| weight |
| 90lb   |
| 70lb   |

sequence empty
| weight |
`,
  "ja:walk": `rule 買物かごの送料(cart_shipping) v1
description "明細の金額を合計して送料を決める。並びを合計する例"

enum 会員区分(tier) = 一般(regular) | 優待(premium)

inputs
  区分(tier) : 会員区分

# 一件ぶんの明細。呼び出し側は何件でも渡す
elements 明細(lines)
  金額(amount) : money[円]  range >=0円 <=100000円

# 歩いたあとに残るのは合計だけ。範囲は完全性の全体集合であり、走っている途中の合計が
# ここを出た時点で入口が断る境目でもある
sum 合計(total) over 明細 of 金額  range >=0円 <=1000000円

outputs
  送料(fee) : money[円]  round up(1円)

table 送料表(fee_table)
policy unique
| 合計             | 区分 | -> 送料(fee) : money[円] |
| >=5000円         | -    | 0円                      |
| >=3000円 <5000円 | 優待 | 0円                      |
| >=3000円 <5000円 | 一般 | 250円                    |
| <3000円          | -    | 500円                    |

examples
| 明細     | 区分 | -> 送料 |
| 二点     | 一般 | 0円     |
| 二点     | 優待 | 0円     |
| 少額     | 一般 | 500円   |
| 中くらい | 一般 | 250円   |
| 中くらい | 優待 | 0円     |
| 空       | 一般 | 500円   |

sequence 二点(two)
| 金額   |
| 3000円 |
| 2500円 |

sequence 少額(small)
| 金額   |
| 1200円 |

sequence 中くらい(middle)
| 金額   |
| 2000円 |
| 1500円 |

sequence 空(empty)
| 金額 |
`,
};

const gap = (src) => src.trimEnd().split("\n").slice(0, -1).join("\n") + "\n";

// The module and the stylesheet sit beside this script, so they are found from its own
// URL rather than the page's: the same three files are loaded by a page in each language.
const HERE = document.currentScript.src;
const WASM = new URL("rulec.wasm", HERE).href;
document.head.append(
  Object.assign(document.createElement("link"), {
    rel: "stylesheet",
    href: new URL("playground.css", HERE).href,
  })
);

let VERSION = "";
let mod = null; // the compiled module, kept so a trapped instance can be replaced
let inst = null;

async function boot() {
  if (!mod) mod = await WebAssembly.compileStreaming(fetch(WASM)).catch(async () =>
    WebAssembly.compile(await (await fetch(WASM)).arrayBuffer())
  );
  inst = await WebAssembly.instantiate(mod, {});
  VERSION = call("rulec_version");
  return inst;
}

function put(str) {
  const bytes = new TextEncoder().encode(str);
  const ptr = inst.exports.rulec_alloc(bytes.length);
  new Uint8Array(inst.exports.memory.buffer, ptr + 4, bytes.length).set(bytes);
  return ptr;
}

function take(ptr) {
  // Read the views after the call: allocating may have grown the memory, which detaches
  // every view taken before it.
  const len = new DataView(inst.exports.memory.buffer).getUint32(ptr, true);
  const bytes = new Uint8Array(inst.exports.memory.buffer, ptr + 4, len).slice();
  inst.exports.rulec_free(ptr);
  return new TextDecoder().decode(bytes);
}

// One call: hand over the source, read the answer, release both buffers.
function call(fn, src, ja) {
  if (src === undefined) return take(inst.exports[fn]());
  const ptr = put(src);
  try {
    return take(inst.exports[fn](ptr, ja ? 1 : 0));
  } finally {
    inst.exports.rulec_free(ptr);
  }
}

function start(root) {
  const lang = root.dataset.lang === "ja" ? "ja" : "en";
  const t = TEXT[lang];
  const src = root.querySelector(".pg-src");
  const status = root.querySelector(".pg-status");
  const out = root.querySelector(".pg-out");
  const picker = root.querySelector(".pg-picker");
  let view = "check";
  let timer = null;
  let files = [];

  src.value = gap(FULL[lang]);
  status.textContent = t.booting;

  const show = (node) => {
    out.replaceChildren(node);
  };
  const pre = (text) => {
    const el = document.createElement("pre");
    el.className = "pg-pre";
    el.textContent = text;
    return el;
  };

  // The address of the approver's page as it stands. A link, not `window.open`: a scripted
  // pop-up is blocked often enough to be unreliable, and a link the reader clicks is a
  // plain navigation — which also means a middle click or a cmd click does what it should.
  //
  // What the link points at is a one-line host document holding the page in a sandboxed
  // frame: the generated JavaScript runs with scripts and no same-origin, exactly as it did
  // in the pane, so nothing on this site is ever in its reach.
  let docUrl = "";
  function boardUrl(html) {
    if (docUrl) URL.revokeObjectURL(docUrl);
    const host =
      '<!doctype html><meta charset="utf-8"><title>rulec</title>' +
      "<style>html,body{margin:0;height:100%;background:#fff}iframe{display:block;border:0;width:100%;height:100%}</style>" +
      '<iframe sandbox="allow-scripts" srcdoc="' + html.replace(/&/g, "&amp;").replace(/"/g, "&quot;") + '"></iframe>';
    docUrl = URL.createObjectURL(new Blob([host], { type: "text/html" }));
    return docUrl;
  }

  function run() {
    if (!inst) return;
    const started = performance.now();
    let answer;
    try {
      answer = JSON.parse(call(view === "check" ? "rulec_check" : view === "gen" ? "rulec_gen" : "rulec_doc", src.value, lang === "ja"));
    } catch (e) {
      // A trap leaves the instance unusable; the module is still good, so take a new one.
      status.textContent = t.broken;
      inst = null;
      boot().then(run);
      return;
    }
    const ms = Math.round(performance.now() - started);
    picker.hidden = true;
    if (view === "check") {
      status.textContent = answer.errors + answer.warnings === 0 ? t.passed(ms) : t.checked(ms, answer.errors, answer.warnings);
      show(pre(answer.text));
      return;
    }
    if (!answer.ok) {
      status.textContent = t.refused;
      show(pre(answer.text));
      return;
    }
    if (view === "doc") {
      // The approver's page is a board laid out for a window, and this pane is a box inside
      // a documentation page — so it is not shown here. It is held, and opened full-screen
      // in a tab of its own when the reader asks for it. Rendering still happens on every
      // edit, so the tab that opens is always the current table's.
      status.textContent = `${ms} ms`;
      const box = document.createElement("div");
      box.className = "pg-board";
      const p = document.createElement("p");
      p.textContent = t.board;
      const a = document.createElement("a");
      a.className = "pg-open";
      a.href = boardUrl(answer.html);
      a.target = "_blank";
      a.rel = "noopener";
      a.textContent = t.open;
      box.append(p, a);
      show(box);
      return;
    }
    files = answer.files;
    picker.replaceChildren(
      ...files.map((f, i) => {
        const o = document.createElement("option");
        o.value = String(i);
        o.textContent = f.path;
        return o;
      })
    );
    picker.hidden = false;
    const keep = files.findIndex((f) => f.path === picker.dataset.path);
    picker.value = String(keep < 0 ? 0 : keep);
    status.textContent = t.generated(files.length, ms);
    show(pre(files[picker.value].body));
  }

  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(run, 300);
  };

  src.addEventListener("input", later);
  picker.addEventListener("change", () => {
    picker.dataset.path = files[picker.value].path;
    show(pre(files[picker.value].body));
  });
  for (const b of root.querySelectorAll("[data-view]")) {
    b.addEventListener("click", () => {
      view = b.dataset.view;
      for (const o of root.querySelectorAll("[data-view]")) o.classList.toggle("on", o === b);
      run();
    });
  }
  for (const b of root.querySelectorAll("[data-preset]")) {
    b.addEventListener("click", () => {
      const k = b.dataset.preset;
      src.value = k === "gap" ? gap(FULL[lang]) : k === "full" ? FULL[lang] : SAMPLES[lang + ":" + k];
      run();
    });
  }

  boot().then(run);
}

for (const root of document.querySelectorAll(".pg")) start(root);
