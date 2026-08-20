//! 過去データの記録（§10.2）と、その検証。
//!
//! 本番ログからこの形への抽出（ETL）は利用者の仕事と割り切り、rulec が引き受けるのは
//! 型と範囲の検証だけである。1 件 1 行の JSONL に固定する。
//!
//! ```text
//! {"ts":"2025-08-14T09:12:33+09:00","tag":"order:1234567",
//!  "in":{"届け先":"鹿児島県","重量":800,"注文金額":4200,"会員":"一般"},
//!  "observed":{"送料":800}}
//! ```
//!
//! **fixtures はリポジトリに入れない。** 注文金額を含むので、CI にはアーティファクトか
//! 保護ストレージで渡す（§10.2）。

use crate::ast::RuleFile;
use crate::eval::Val;
use crate::json::Json;
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// 一件の記録。`in` は評価器に渡せる形まで解いてある。
pub struct Record {
    pub line: usize,
    pub tag: String,
    pub ts: String,
    pub input: BTreeMap<String, Val>,
    /// 観測された出力。宣言順ではなく名前で引く（記録の側の順序に頼らない）。
    pub observed: BTreeMap<String, Val>,
    /// 既定値で補った欄。空でなければこの記録は「補完系」（§10.3）。
    pub filled: Vec<String>,
}

pub struct Problem {
    pub line: usize,
    pub tag: String,
    pub what: String,
    pub hint: String,
}

pub struct Load {
    pub records: Vec<Record>,
    pub problems: Vec<Problem>,
    /// 欄が欠けていたので丸ごと外した件数（§10.3 のモード 1）。
    pub dropped: usize,
}

impl Load {
    /// 実測系（補完のない記録）の件数。見出しの一致率はここからだけ計算する。
    pub fn measured(&self) -> usize {
        self.records.iter().filter(|r| r.filled.is_empty()).count()
    }
    pub fn filled(&self) -> usize {
        self.records.len() - self.measured()
    }
}

/// JSON の値を、宣言された型の `Val` に直す。ワイヤは正準単位の整数（§10.1）。
pub fn to_val(j: &Json, ty: &Ty, c: &Checked, name: &str) -> Result<Val, String> {
    let inner = match ty {
        Ty::Opt(t) => {
            if *j == Json::Null {
                return Ok(Val::Enum("無し".into()));
            }
            (**t).clone()
        }
        other => other.clone(),
    };
    match (&inner, j) {
        (Ty::Enum(en), Json::Str(s)) => {
            let vs = c.enums.get(en).cloned().unwrap_or_default();
            if vs.iter().any(|v| v == s) {
                Ok(Val::Enum(s.clone()))
            } else {
                Err(format!("`{s}` は列挙 {en} の値ではありません"))
            }
        }
        (Ty::Bool, Json::Bool(b)) => Ok(Val::Bool(*b)),
        (Ty::Str, Json::Str(s)) => Ok(Val::Str(s.clone())),
        (Ty::Date, Json::Str(s)) => {
            let p: Vec<&str> = s.split('-').collect();
            let ok = p.len() == 3 && p[0].len() == 4 && p[1].len() == 2 && p[2].len() == 2;
            let n: Option<Vec<i64>> = p.iter().map(|x| x.parse::<i64>().ok()).collect();
            match (ok, n) {
                (true, Some(v)) if (1..=12).contains(&v[1]) && (1..=31).contains(&v[2]) => {
                    Ok(Val::Date(v[0] as i32, v[1] as u32, v[2] as u32))
                }
                _ => Err(format!("`{s}` は YYYY-MM-DD の日付ではありません")),
            }
        }
        (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate, Json::Int(n)) => {
            let v = Rat::int(*n);
            if let Some((lo, hi)) = c.ranges.get(name) {
                if lo.is_some_and(|l| v.cmp_to(l) == std::cmp::Ordering::Less)
                    || hi.is_some_and(|h| v.cmp_to(h) == std::cmp::Ordering::Greater)
                {
                    let show = |b: &Option<Rat>| b.map(|x| x.to_string()).unwrap_or_else(|| "…".into());
                    return Err(format!(
                        "{n} は宣言範囲 {}..{} の外です",
                        show(&lo.map(|x| x)),
                        show(&hi.map(|x| x))
                    ));
                }
            }
            Ok(Val::Num(v))
        }
        (_, got) => Err(format!("{} を期待しましたが {} でした", ty_word(&inner), got.kind())),
    }
}

fn ty_word(ty: &Ty) -> String {
    match ty {
        Ty::Enum(e) => format!("列挙 {e} の値（文字列）"),
        Ty::Bool => "真偽".into(),
        Ty::Date => "日付（YYYY-MM-DD の文字列）".into(),
        Ty::Str => "文字列".into(),
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => "正準単位の整数".into(),
        _ => format!("{ty}"),
    }
}

/// 再生マニフェスト（§10.3）。fixtures の隣に置く小さな JSON で、
/// **既定値と対象欄しか持たないので機微を含まず、リポジトリに入る**。
///
/// ```json
/// {"rulec":"replay/1","rule":"送料","fills":{"会員":"一般"}}
/// ```
///
/// 既定値を規則そのものに焼き込まないのは、規則が純関数で、補完は**特定の
/// 再生実験の判断**だからである。「会員は一般で埋める」と「ゴールドで埋めて
/// 影響の上限を見る」を同じ規則に対して別々に走らせるのは正当な使い方で、
/// 規則に一つ焼くとそれができない。意味に関与しない注記が規則のハッシュを変えて
/// `gen --check` に無駄な再生成を要求する害もある。
///
/// 正直さの担保は「根拠がどこに書かれたか」ではなく「**数字の出る場所に根拠が
/// 現れるか**」に置く。レポートは使った既定値と欄ごとの補完件数を必ず刻む。
#[derive(Default)]
pub struct Manifest {
    /// 欄 → 既定値。ここに無い欄が欠けている記録は、補完せず丸ごと外す。
    pub fills: BTreeMap<String, Val>,
    /// 刻印に出すための、書かれたままの表記。
    pub shown: BTreeMap<String, String>,
}

impl Manifest {
    /// `会員=一般` の形を読む（感度分析の一時上書き。§10.3）。
    pub fn add(&mut self, spec: &str, f: &RuleFile, c: &Checked) -> Result<(), String> {
        let (name, text) = spec.split_once('=').ok_or_else(|| {
            format!("`{spec}` は `欄=値` の形ではありません")
        })?;
        let (name, text) = (name.trim(), text.trim());
        if !f.inputs.iter().any(|i| i.name.text == name) {
            return Err(format!("`{name}` は規則の入力ではありません"));
        }
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        // 数量・金額・率は整数、それ以外は文字列として読む。
        let j = match &ty {
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => text
                .parse::<i128>()
                .map(Json::Int)
                .map_err(|_| format!("`{name}` は正準単位の整数で書いてください"))?,
            Ty::Bool => match text {
                "真" | "true" => Json::Bool(true),
                "偽" | "false" => Json::Bool(false),
                _ => return Err(format!("`{name}` は 真 か 偽 です")),
            },
            _ => Json::Str(text.into()),
        };
        let v = to_val(&j, &ty, c, name).map_err(|e| format!("`{name}`: {e}"))?;
        self.shown.insert(name.into(), text.into());
        self.fills.insert(name.into(), v);
        Ok(())
    }

    /// マニフェストの JSON を読む。
    pub fn load(src: &str, f: &RuleFile, c: &Checked) -> Result<Manifest, String> {
        let j = crate::json::parse(src.trim()).map_err(|e| format!("マニフェストが読めません: {e}"))?;
        if let Some(r) = j.get("rule").and_then(|x| x.as_str()) {
            if r != f.name.text {
                return Err(format!(
                    "マニフェストは規則 `{r}` のものです（いま見ているのは `{}`）",
                    f.name.text
                ));
            }
        }
        let mut m = Manifest::default();
        let Some(fills) = j.get("fills").and_then(|x| x.as_obj()) else {
            return Ok(m);
        };
        for (name, v) in fills {
            if !f.inputs.iter().any(|i| i.name.text == *name) {
                return Err(format!("`{name}` は規則の入力ではありません"));
            }
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            let val = to_val(v, &ty, c, name).map_err(|e| format!("既定値 `{name}`: {e}"))?;
            m.shown.insert(name.clone(), format!("{v}"));
            m.fills.insert(name.clone(), val);
        }
        Ok(m)
    }
}

/// JSONL を読んで検証する。**壊れた記録は捨てずに報告する。**
/// 黙って落とすと、分母が縮んだ分だけ一致率が上がって見える（§10.3）。
pub fn load(src: &str, f: &RuleFile, c: &Checked, m: &Manifest) -> Load {
    let mut out = Load { records: Vec::new(), problems: Vec::new(), dropped: 0 };
    for (li, raw) in src.lines().enumerate() {
        let line = li + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let j = match crate::json::parse(raw) {
            Ok(j) => j,
            Err(e) => {
                out.problems.push(Problem {
                    line,
                    tag: String::new(),
                    what: format!("JSON として読めません: {e}"),
                    hint: "1 件 1 行の JSON Lines です（§10.2）。".into(),
                });
                continue;
            }
        };
        let tag = j.get("tag").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let ts = j.get("ts").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let mut bad = |what: String, hint: &str| {
            out.problems.push(Problem { line, tag: tag.clone(), what, hint: hint.into() });
        };

        let Some(ins) = j.get("in").and_then(|x| x.as_obj()) else {
            bad("`in` がありません".into(), "入力は `in` の下に、規則の和名で置きます。");
            continue;
        };
        let Some(obs) = j.get("observed").and_then(|x| x.as_obj()) else {
            bad("`observed` がありません".into(), "そのとき実際に出た値を `observed` に置きます。");
            continue;
        };

        // 規則が知らない欄は誤りとして言う。黙って捨てると、綴りの間違いが
        // 「既定値で補完された」に化けて一致率だけが動く。
        let known: Vec<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).collect();
        let mut extra: Vec<&String> = ins.keys().filter(|k| !known.contains(&k.as_str())).collect();
        extra.sort();
        if !extra.is_empty() {
            bad(
                format!("`in` に規則が知らない欄があります: {}", extra.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
                "規則の入力の和名と綴りを合わせてください。",
            );
            continue;
        }

        let mut input: BTreeMap<String, Val> = BTreeMap::new();
        let mut filled: Vec<String> = Vec::new();
        let mut broken = false;
        for i in &f.inputs {
            let name = &i.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            match ins.get(name) {
                Some(v) => match to_val(v, &ty, c, name) {
                    Ok(v) => {
                        input.insert(name.clone(), v);
                    }
                    Err(e) => {
                        bad(format!("`in.{name}`: {e}"), "型か範囲が宣言と食い違っています。");
                        broken = true;
                    }
                },
                // §10.3: 欄ごとに二つのモードしかない。既定値が宣言されていれば
                // 補って「補完系」の札を付け、無ければ**その記録を丸ごと外す**。
                // 逆推定はしない。欄の欠けは鍵の不在で、optional の `無し` は
                // `null` である（「無いと分かっている」は補完の対象ではない）。
                None => match m.fills.get(name) {
                    Some(v) => {
                        input.insert(name.clone(), v.clone());
                        filled.push(name.clone());
                    }
                    None => {
                        broken = true;
                        out.dropped += 1;
                        break;
                    }
                },
            }
        }
        if broken {
            continue;
        }

        let mut observed: BTreeMap<String, Val> = BTreeMap::new();
        for o in &f.outputs {
            let name = &o.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            match obs.get(name) {
                Some(v) => match to_val(v, &ty, c, name) {
                    Ok(v) => {
                        observed.insert(name.clone(), v);
                    }
                    Err(e) => {
                        bad(format!("`observed.{name}`: {e}"), "そのとき出た値を、正準単位で書いてください。");
                        broken = true;
                    }
                },
                None => {
                    bad(
                        format!("`observed.{name}` がありません"),
                        "出力は全部要ります。片方だけ比べると、比べなかった側の食い違いが緑になります。",
                    );
                    broken = true;
                }
            }
        }
        if broken {
            continue;
        }
        out.records.push(Record { line, tag, ts, input, observed, filled });
    }
    out
}

/// `rulec fixtures lint` の報告。
pub fn render_lint(l: &Load, path: &str) -> String {
    let mut o = format!(
        "{path}: 記録 {} 件（実測系 {}、補完系 {}）\n",
        l.records.len(),
        l.measured(),
        l.filled()
    );
    if l.dropped > 0 {
        o.push_str(&format!("欄が欠けていたので外した記録: {} 件\n", l.dropped));
    }
    if l.problems.is_empty() {
        o.push_str("形式の問題はありません。\n");
        return o;
    }
    o.push_str(&format!("\n問題 {} 件:\n", l.problems.len()));
    // 同じ形の問題は括る。1 万行の抽出で同じ綴り間違いが 1 万回並ぶと読めない。
    let mut by: BTreeMap<&str, Vec<&Problem>> = BTreeMap::new();
    for p in &l.problems {
        by.entry(p.what.as_str()).or_default().push(p);
    }
    for (what, ps) in &by {
        let ex = ps[0];
        let where_ = if ex.tag.is_empty() {
            format!("{} 行目", ex.line)
        } else {
            format!("{} 行目 ({})", ex.line, ex.tag)
        };
        o.push_str(&format!("  {what}\n    {} 件。例: {where_}\n    {}\n", ps.len(), ex.hint));
    }
    o
}
