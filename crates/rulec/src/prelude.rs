//! The built-in `std/` namespace (§2.2, §15.182): the first-level divisions of thirteen countries.
//!
//! Their values appear on the public surface of the generated code, so the generated names are
//! frozen here: `PREFECTURES` since the first release (open question 9), the other twelve
//! countries' divisions since §15.182. The prefectures' romanization is Hepburn, not Kunrei
//! (`Hyogo`, `Oita`), with long vowels dropped, to match the English spellings local governments
//! use.
//!
//! The other countries' names come from the Unicode CLDR 48.2 subdivision names (the English and
//! the country's language; Unicode License V3, see THIRD_PARTY_NOTICES), held against ISO 3166-2
//! and each government's list. The table is written into this file; nothing is read at run time.

use crate::ast::{BindValue, Cell, Expr, Item, Lit, OutCell, RuleFile, Table};
use crate::diag::{Diag, Span};

pub const PREFECTURES: &[(&str, &str)] = &[
    ("北海道", "Hokkaido"),
    ("青森県", "Aomori"),
    ("岩手県", "Iwate"),
    ("宮城県", "Miyagi"),
    ("秋田県", "Akita"),
    ("山形県", "Yamagata"),
    ("福島県", "Fukushima"),
    ("茨城県", "Ibaraki"),
    ("栃木県", "Tochigi"),
    ("群馬県", "Gunma"),
    ("埼玉県", "Saitama"),
    ("千葉県", "Chiba"),
    ("東京都", "Tokyo"),
    ("神奈川県", "Kanagawa"),
    ("新潟県", "Niigata"),
    ("富山県", "Toyama"),
    ("石川県", "Ishikawa"),
    ("福井県", "Fukui"),
    ("山梨県", "Yamanashi"),
    ("長野県", "Nagano"),
    ("岐阜県", "Gifu"),
    ("静岡県", "Shizuoka"),
    ("愛知県", "Aichi"),
    ("三重県", "Mie"),
    ("滋賀県", "Shiga"),
    ("京都府", "Kyoto"),
    ("大阪府", "Osaka"),
    ("兵庫県", "Hyogo"),
    ("奈良県", "Nara"),
    ("和歌山県", "Wakayama"),
    ("鳥取県", "Tottori"),
    ("島根県", "Shimane"),
    ("岡山県", "Okayama"),
    ("広島県", "Hiroshima"),
    ("山口県", "Yamaguchi"),
    ("徳島県", "Tokushima"),
    ("香川県", "Kagawa"),
    ("愛媛県", "Ehime"),
    ("高知県", "Kochi"),
    ("福岡県", "Fukuoka"),
    ("佐賀県", "Saga"),
    ("長崎県", "Nagasaki"),
    ("熊本県", "Kumamoto"),
    ("大分県", "Oita"),
    ("宮崎県", "Miyazaki"),
    ("鹿児島県", "Kagoshima"),
    ("沖縄県", "Okinawa"),
];

/// One first-level division of a country (§15.182).
pub struct Division {
    /// Its English name in ASCII, from CLDR: the value a rule names it by and the wire carries
    /// (for the prefectures under `std/都道府県`, the Japanese name is the value instead).
    pub name: &'static str,
    /// The member the generated code names it by, in every target. Frozen.
    pub member: &'static str,
    /// Its ISO 3166-2 code after the country's prefix (`CA` of `US-CA`), when the code can be
    /// written as a name; empty when it starts with a digit (`JP-13`, `KR-11`, `IT-21`, `FR-20R`).
    pub code: &'static str,
    /// The other spellings a rule may name it by: the name in the country's language (with `_`
    /// for spaces and hyphens, which a name cannot hold), official and short forms, and the
    /// English forms CLDR or the government list also use. Each is the same value.
    pub spellings: &'static [&'static str],
}

/// One country's first-level divisions, as `import std/<country>/<kind>` brings them in.
pub struct Namespace {
    /// `std/us/states`: ISO 3166-1's two letters in lower case, and the kind of division.
    pub path: &'static str,
    /// The country, as ISO 3166-1 writes it.
    pub country: &'static str,
    /// The enum's name in a rule (`us_state`). The country is in it because two countries share
    /// a kind (five have `states`), and a rule may import both.
    pub ty: &'static str,
    /// The enum's name in the generated code. Frozen.
    pub generated: &'static str,
    pub divisions: &'static [Division],
}

/// Every namespace under `std/`, in the order the documents list them. Japan comes last because
/// it is also `std/都道府県`, the one namespace there was before §15.182.
pub const NAMESPACES: &[Namespace] = &[
    Namespace { path: "std/us/states", country: "US", ty: "us_state", generated: "UsState", divisions: US_STATES },
    Namespace { path: "std/gb/nations", country: "GB", ty: "gb_nation", generated: "GbNation", divisions: GB_NATIONS },
    Namespace { path: "std/cn/provinces", country: "CN", ty: "cn_province", generated: "CnProvince", divisions: CN_PROVINCES },
    Namespace { path: "std/tw/divisions", country: "TW", ty: "tw_division", generated: "TwDivision", divisions: TW_DIVISIONS },
    Namespace { path: "std/kr/provinces", country: "KR", ty: "kr_province", generated: "KrProvince", divisions: KR_PROVINCES },
    Namespace { path: "std/in/states", country: "IN", ty: "in_state", generated: "InState", divisions: IN_STATES },
    Namespace { path: "std/fr/regions", country: "FR", ty: "fr_region", generated: "FrRegion", divisions: FR_REGIONS },
    Namespace { path: "std/es/communities", country: "ES", ty: "es_community", generated: "EsCommunity", divisions: ES_COMMUNITIES },
    Namespace { path: "std/it/regions", country: "IT", ty: "it_region", generated: "ItRegion", divisions: IT_REGIONS },
    Namespace { path: "std/de/states", country: "DE", ty: "de_state", generated: "DeState", divisions: DE_STATES },
    Namespace { path: "std/au/states", country: "AU", ty: "au_state", generated: "AuState", divisions: AU_STATES },
    Namespace { path: "std/br/states", country: "BR", ty: "br_state", generated: "BrState", divisions: BR_STATES },
    Namespace { path: "std/jp/prefectures", country: "JP", ty: "jp_prefecture", generated: "Prefecture", divisions: JP_PREFECTURES },
];

/// The Japanese name of the prefectures' namespace, and of its enum: `import std/都道府県`.
pub const JAPANESE_PATH: &str = "std/都道府県";
pub const JAPANESE_TYPE: &str = "都道府県";

/// One `import std/…` line, read: which namespace, the enum's name in the rule, and how its
/// values are spelled.
#[derive(Clone, Copy)]
pub struct Import {
    pub ns: &'static Namespace,
    pub ty: &'static str,
    /// The values are the Japanese names (`std/都道府県`, as they were before §15.182); every
    /// other import, `std/jp/prefectures` included, spells them in English. Either way the
    /// division is the same, and so is the generated code.
    pub japanese: bool,
}

impl Import {
    /// The value a division is, in this import's spelling.
    pub fn value(&self, d: &Division) -> &'static str {
        if self.japanese { d.spellings[0] } else { d.name }
    }

    /// The enum's values, in order.
    pub fn values(&self) -> Vec<String> {
        self.ns.divisions.iter().map(|d| self.value(d).to_string()).collect()
    }

    /// Every way to write each division, the value itself first.
    fn all(d: &Division) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(d.name).chain((!d.code.is_empty()).then_some(d.code)).chain(d.spellings.iter().copied())
    }

    /// The division a word names, in any of its spellings.
    pub fn division(&self, w: &str) -> Option<&'static Division> {
        self.ns.divisions.iter().find(|d| Self::all(d).any(|s| s == w))
    }

    /// The value a word stands for, when it is another spelling of one (`CA` → `California`).
    pub fn resolve(&self, w: &str) -> Option<&'static str> {
        self.division(w).map(|d| self.value(d))
    }

    /// `import std/us/states` as the rule writes it.
    pub fn path(&self) -> &'static str {
        if self.japanese { JAPANESE_PATH } else { self.ns.path }
    }
}

/// The namespace an `import` names. `std/都道府県` stays what it was: the prefectures, spelled in
/// Japanese, as the enum `都道府県`.
pub fn lookup(path: &str) -> Option<Import> {
    let jp = NAMESPACES.last().expect("Japan is in the table");
    if path.ends_with(JAPANESE_TYPE) {
        return Some(Import { ns: jp, ty: JAPANESE_TYPE, japanese: true });
    }
    NAMESPACES.iter().find(|n| n.path == path).map(|ns| Import { ns, ty: ns.ty, japanese: false })
}

/// The imports of a file that name a namespace, in the order written.
pub fn imports(f: &RuleFile) -> Vec<Import> {
    f.imports.iter().filter_map(|(p, _)| lookup(p)).collect()
}

/// The namespace an enum of a file is, when it is one that was imported.
pub fn import_of(f: &RuleFile, ty: &str) -> Option<Import> {
    imports(f).into_iter().find(|i| i.ty == ty)
}

/// Every path an `import std/…` takes, for the notes that list them.
pub fn paths() -> Vec<&'static str> {
    NAMESPACES.iter().map(|n| n.path).chain(std::iter::once(JAPANESE_PATH)).collect()
}

/// Edit distance, by characters, without regard to case (the hints only).
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The path closest to one that names nothing (`std/us/state`), for E013's hint.
pub fn nearest_path(path: &str) -> Option<&'static str> {
    paths().into_iter().map(|p| (distance(path, p), p)).filter(|(d, _)| *d <= 3).min_by_key(|(d, _)| *d).map(|(_, p)| p)
}

/// What to tell the writer of a word that is no value here (E012), when a namespace knows it:
/// the namespace that has it, when the rule does not import that one (`Bavaria` in a rule that
/// imports `std/us/states` only), or the closest spelling of an imported one (`Calfornia`).
pub fn hint(w: &str, imported: &[Import]) -> Option<String> {
    if imported.iter().any(|i| i.division(w).is_some()) {
        return None;
    }
    let owners: Vec<&Namespace> = NAMESPACES.iter().filter(|n| n.divisions.iter().any(|d| Import::all(d).any(|s| s == w))).collect();
    if !owners.is_empty() && !imported.is_empty() {
        let list = owners.iter().map(|n| format!("`{}`", n.path)).collect::<Vec<_>>().join(", ");
        let ours = imported.iter().map(|i| format!("`{}`", i.path())).collect::<Vec<_>>().join(", ");
        return Some(tr!(
            "`{w}` は {list} の区分です。この規則が取り込んでいるのは {ours} です。別の国の区分は、その列の列挙の値にはなりません。",
            "`{w}` is a division of {list}. This rule imports {ours}. A division of another country is not a value of the column's enum."
        ));
    }
    let mut best: Option<(usize, &str, &str)> = None;
    for i in imported {
        for d in i.ns.divisions {
            for s in Import::all(d) {
                let k = distance(w, s);
                let limit = if w.chars().count() <= 4 { 1 } else { 2 };
                if k <= limit && best.is_none_or(|(b, _, _)| k < b) {
                    best = Some((k, s, i.value(d)));
                }
            }
        }
    }
    best.map(|(_, s, v)| {
        if s == v {
            tr!("`{v}` のことですか。", "Did you mean `{v}`?")
        } else {
            tr!("`{s}`（= `{v}`）のことですか。", "Did you mean `{s}` (= `{v}`)?")
        }
    })
}

/// Writes every other spelling of an imported division as the value it stands for (`CA` →
/// `California`, `Bayern` → `Bavaria`, `東京都` → `Tokyo` under `std/jp/prefectures`), so that
/// everything after the parser — the checks, the vectors, the certificate, the generators — sees
/// one value for one division. Runs after `apply::expand`, so a callee's cells are spelled this
/// rule's way too. A word that is a value of some enum here, or a declared name, is left alone:
/// a rule's own `enum region = CA | NY` keeps its `CA`. A word two imported countries both
/// spell (`WA` is Washington and Western Australia) is E012: the rule writes the name instead.
pub fn normalize(f: &mut RuleFile, path: &str) -> Vec<Diag> {
    let imported = imports(f);
    if imported.is_empty() {
        return Vec::new();
    }
    let mut keep: std::collections::HashSet<String> = std::collections::HashSet::new();
    for e in &f.enums {
        keep.extend(e.values.iter().map(|v| v.text.clone()));
    }
    for i in &imported {
        keep.extend(i.values());
    }
    keep.extend(f.inputs.iter().map(|i| i.name.text.clone()));
    keep.extend(f.outputs.iter().map(|o| o.name.text.clone()));
    keep.extend(f.groups.iter().map(|g| g.name.text.clone()));
    if let Some(e) = &f.elements {
        keep.extend(e.fields.iter().map(|i| i.name.text.clone()));
    }
    for it in &f.items {
        match it {
            Item::Derived(d) => keep.insert(d.name.text.clone()),
            Item::Define(d) => keep.insert(d.name.text.clone()),
            Item::Agg(d) => keep.insert(d.name.text.clone()),
            Item::Table(t) => {
                keep.extend(t.outputs.iter().map(|o| o.name.text.clone()));
                false
            }
        };
    }
    let mut n = Norm { imported, keep, diags: Vec::new(), path: path.to_string() };
    for g in &mut f.groups {
        for m in &mut g.members {
            n.word(&mut m.text, &m.span);
        }
    }
    for it in &mut f.items {
        match it {
            Item::Table(t) => n.table(t),
            Item::Derived(d) => n.expr(&mut d.expr),
            Item::Define(d) => n.expr(&mut d.expr),
            Item::Agg(d) => {
                if let Some(v) = &mut d.value {
                    n.word(&mut v.text, &v.span);
                }
            }
        }
    }
    if let Some(r) = &mut f.result {
        n.expr(&mut r.expr);
    }
    for t in &mut f.examples {
        n.table(t);
    }
    for s in &mut f.sequences {
        let sp = s.span.clone();
        for r in &mut s.rows {
            n.row(&mut r.cells, &mut r.outs, &r.cell_spans, &r.out_spans, &sp);
        }
    }
    if let Some(fold) = &mut f.fold {
        for (v, arm, _) in &mut fold.arms {
            n.word(&mut v.text, &v.span);
            match arm {
                crate::ast::Arm::Stop(Some(e)) => n.expr(e),
                crate::ast::Arm::Take { expr, .. } => n.expr(expr),
                crate::ast::Arm::KeepMax { expr, key } => {
                    n.expr(expr);
                    n.expr(key);
                }
                _ => {}
            }
        }
        if let Some(e) = &mut fold.empty {
            n.expr(e);
        }
        if let Some(e) = &mut fold.exhausted {
            n.expr(e);
        }
    }
    if let Some(m) = &mut f.machine {
        if let Some((v, sp)) = &mut m.initial {
            let sp = sp.clone();
            n.word(&mut v.text, &sp);
        }
        for v in &mut m.finals {
            n.word(&mut v.text, &v.span);
        }
        for nv in &mut m.nevers {
            for v in nv.states.iter_mut().chain(nv.after.iter_mut()) {
                n.word(&mut v.text, &v.span);
            }
        }
        for o in &mut m.onces {
            let sp = o.cell_span.clone();
            n.cell(&mut o.cell, &sp);
        }
    }
    for s in &mut f.scenarios {
        n.table(&mut s.table);
    }
    for a in &mut f.applies {
        for b in &mut a.bindings {
            let sp = b.span.clone();
            match &mut b.value {
                BindValue::Name(w) | BindValue::Lit(Lit::Word(w)) => n.word(w, &sp),
                _ => {}
            }
            for (from, _) in &mut b.map {
                n.word(from, &sp);
            }
        }
    }
    n.diags
}

struct Norm {
    imported: Vec<Import>,
    keep: std::collections::HashSet<String>,
    diags: Vec<Diag>,
    path: String,
}

impl Norm {
    fn word(&mut self, w: &mut String, sp: &Span) {
        if self.keep.contains(w.as_str()) {
            return;
        }
        let mut found: Vec<(&'static str, Import)> = Vec::new();
        for i in &self.imported {
            if let Some(v) = i.resolve(w) {
                if !found.iter().any(|(x, j)| *x == v && j.ty == i.ty) {
                    found.push((v, *i));
                }
            }
        }
        match found.as_slice() {
            [] => {}
            [(v, _)] => *w = v.to_string(),
            many => {
                let list = many.iter().map(|(v, i)| format!("`{v}`（{}）", i.ty)).collect::<Vec<_>>().join(tr!("、", ", ").as_str());
                let list_en = many.iter().map(|(v, i)| format!("`{v}` ({})", i.ty)).collect::<Vec<_>>().join(", ");
                let at = format!("{}:{}", self.path, sp.line);
                self.diags.push(
                    Diag::error("E012", tr!("`{w}` は取り込んだ二つの区分のどちらにも読めます", "`{w}` reads as a division of two imports"))
                        .at(at)
                        .mark(sp.clone(), tr!("{list} のどれか", "one of {list_en}"))
                        .note(tr!(
                            "符号や現地の綴りが二つの国で同じです。どの区分か分かるように、値の名前（{list}）で書いてください。",
                            "Two countries spell a division this way (a code, or a local name). Write the value's name ({list_en}), so that it says which one."
                        )),
                );
            }
        }
    }

    fn lit(&mut self, l: &mut Lit, sp: &Span) {
        if let Lit::Word(w) = l {
            self.word(w, sp);
        }
    }

    fn cell(&mut self, c: &mut Cell, sp: &Span) {
        match c {
            Cell::Lit(l) => self.lit(l, sp),
            Cell::Set(ls) | Cell::Not(ls) => {
                for l in ls {
                    self.lit(l, sp);
                }
            }
            _ => {}
        }
    }

    fn row(&mut self, cells: &mut [Cell], outs: &mut [OutCell], cell_spans: &[Span], out_spans: &[Span], fallback: &Span) {
        for (k, c) in cells.iter_mut().enumerate() {
            let sp = cell_spans.get(k).cloned().unwrap_or_else(|| fallback.clone());
            self.cell(c, &sp);
        }
        for (k, o) in outs.iter_mut().enumerate() {
            let sp = out_spans.get(k).cloned().unwrap_or_else(|| fallback.clone());
            match o {
                OutCell::Lit(l) => self.lit(l, &sp),
                OutCell::Name(w) => self.word(w, &sp),
            }
        }
    }

    fn table(&mut self, t: &mut Table) {
        for r in &mut t.rows {
            let sp = r.span.clone();
            self.row(&mut r.cells, &mut r.outs, &r.cell_spans, &r.out_spans, &sp);
        }
    }

    fn expr(&mut self, e: &mut Expr) {
        match e {
            Expr::Name(w, sp) => {
                let sp = sp.clone();
                self.word(w, &sp)
            }
            Expr::Lit(l, sp) => {
                let sp = sp.clone();
                self.lit(l, &sp)
            }
            Expr::Bin(a, _, b, _) => {
                self.expr(a);
                self.expr(b);
            }
            Expr::Call(_, args, _) => {
                for a in args {
                    self.expr(a);
                }
            }
        }
    }
}

const US_STATES: &[Division] = &[
    Division { name: "Alaska", member: "Alaska", code: "AK", spellings: &[] },
    Division { name: "Alabama", member: "Alabama", code: "AL", spellings: &[] },
    Division { name: "Arkansas", member: "Arkansas", code: "AR", spellings: &[] },
    Division { name: "American_Samoa", member: "AmericanSamoa", code: "AS", spellings: &[] },
    Division { name: "Arizona", member: "Arizona", code: "AZ", spellings: &[] },
    Division { name: "California", member: "California", code: "CA", spellings: &[] },
    Division { name: "Colorado", member: "Colorado", code: "CO", spellings: &[] },
    Division { name: "Connecticut", member: "Connecticut", code: "CT", spellings: &[] },
    Division { name: "Washington_DC", member: "WashingtonDC", code: "DC", spellings: &["District_of_Columbia"] },
    Division { name: "Delaware", member: "Delaware", code: "DE", spellings: &[] },
    Division { name: "Florida", member: "Florida", code: "FL", spellings: &[] },
    Division { name: "Georgia", member: "Georgia", code: "GA", spellings: &[] },
    Division { name: "Guam", member: "Guam", code: "GU", spellings: &[] },
    Division { name: "Hawaii", member: "Hawaii", code: "HI", spellings: &[] },
    Division { name: "Iowa", member: "Iowa", code: "IA", spellings: &[] },
    Division { name: "Idaho", member: "Idaho", code: "ID", spellings: &[] },
    Division { name: "Illinois", member: "Illinois", code: "IL", spellings: &[] },
    Division { name: "Indiana", member: "Indiana", code: "IN", spellings: &[] },
    Division { name: "Kansas", member: "Kansas", code: "KS", spellings: &[] },
    Division { name: "Kentucky", member: "Kentucky", code: "KY", spellings: &[] },
    Division { name: "Louisiana", member: "Louisiana", code: "LA", spellings: &[] },
    Division { name: "Massachusetts", member: "Massachusetts", code: "MA", spellings: &[] },
    Division { name: "Maryland", member: "Maryland", code: "MD", spellings: &[] },
    Division { name: "Maine", member: "Maine", code: "ME", spellings: &[] },
    Division { name: "Michigan", member: "Michigan", code: "MI", spellings: &[] },
    Division { name: "Minnesota", member: "Minnesota", code: "MN", spellings: &[] },
    Division { name: "Missouri", member: "Missouri", code: "MO", spellings: &[] },
    Division { name: "Northern_Mariana_Islands", member: "NorthernMarianaIslands", code: "MP", spellings: &[] },
    Division { name: "Mississippi", member: "Mississippi", code: "MS", spellings: &[] },
    Division { name: "Montana", member: "Montana", code: "MT", spellings: &[] },
    Division { name: "North_Carolina", member: "NorthCarolina", code: "NC", spellings: &[] },
    Division { name: "North_Dakota", member: "NorthDakota", code: "ND", spellings: &[] },
    Division { name: "Nebraska", member: "Nebraska", code: "NE", spellings: &[] },
    Division { name: "New_Hampshire", member: "NewHampshire", code: "NH", spellings: &[] },
    Division { name: "New_Jersey", member: "NewJersey", code: "NJ", spellings: &[] },
    Division { name: "New_Mexico", member: "NewMexico", code: "NM", spellings: &[] },
    Division { name: "Nevada", member: "Nevada", code: "NV", spellings: &[] },
    Division { name: "New_York", member: "NewYork", code: "NY", spellings: &[] },
    Division { name: "Ohio", member: "Ohio", code: "OH", spellings: &[] },
    Division { name: "Oklahoma", member: "Oklahoma", code: "OK", spellings: &[] },
    Division { name: "Oregon", member: "Oregon", code: "OR", spellings: &[] },
    Division { name: "Pennsylvania", member: "Pennsylvania", code: "PA", spellings: &[] },
    Division { name: "Puerto_Rico", member: "PuertoRico", code: "PR", spellings: &[] },
    Division { name: "Rhode_Island", member: "RhodeIsland", code: "RI", spellings: &[] },
    Division { name: "South_Carolina", member: "SouthCarolina", code: "SC", spellings: &[] },
    Division { name: "South_Dakota", member: "SouthDakota", code: "SD", spellings: &[] },
    Division { name: "Tennessee", member: "Tennessee", code: "TN", spellings: &[] },
    Division { name: "Texas", member: "Texas", code: "TX", spellings: &[] },
    Division { name: "Utah", member: "Utah", code: "UT", spellings: &[] },
    Division { name: "Virginia", member: "Virginia", code: "VA", spellings: &[] },
    Division { name: "US_Virgin_Islands", member: "USVirginIslands", code: "VI", spellings: &[] },
    Division { name: "Vermont", member: "Vermont", code: "VT", spellings: &[] },
    Division { name: "Washington", member: "Washington", code: "WA", spellings: &[] },
    Division { name: "Wisconsin", member: "Wisconsin", code: "WI", spellings: &[] },
    Division { name: "West_Virginia", member: "WestVirginia", code: "WV", spellings: &[] },
    Division { name: "Wyoming", member: "Wyoming", code: "WY", spellings: &[] },
];

const GB_NATIONS: &[Division] = &[
    Division { name: "England", member: "England", code: "ENG", spellings: &[] },
    Division { name: "Northern_Ireland", member: "NorthernIreland", code: "NIR", spellings: &[] },
    Division { name: "Scotland", member: "Scotland", code: "SCT", spellings: &[] },
    Division { name: "Wales", member: "Wales", code: "WLS", spellings: &["Cymru"] },
];

const CN_PROVINCES: &[Division] = &[
    Division { name: "Anhui", member: "Anhui", code: "AH", spellings: &["安徽省", "安徽"] },
    Division { name: "Beijing", member: "Beijing", code: "BJ", spellings: &["北京市", "北京"] },
    Division { name: "Chongqing", member: "Chongqing", code: "CQ", spellings: &["重庆市", "重庆"] },
    Division { name: "Fujian", member: "Fujian", code: "FJ", spellings: &["福建省", "福建"] },
    Division { name: "Guangdong", member: "Guangdong", code: "GD", spellings: &["广东省", "广东"] },
    Division { name: "Gansu", member: "Gansu", code: "GS", spellings: &["甘肃省", "甘肃"] },
    Division { name: "Guangxi", member: "Guangxi", code: "GX", spellings: &["广西壮族自治区", "广西"] },
    Division { name: "Guizhou", member: "Guizhou", code: "GZ", spellings: &["贵州省", "贵州"] },
    Division { name: "Henan", member: "Henan", code: "HA", spellings: &["河南省", "河南"] },
    Division { name: "Hubei", member: "Hubei", code: "HB", spellings: &["湖北省", "湖北"] },
    Division { name: "Hebei", member: "Hebei", code: "HE", spellings: &["河北省", "河北"] },
    Division { name: "Hainan", member: "Hainan", code: "HI", spellings: &["海南省", "海南"] },
    Division { name: "Hong_Kong", member: "HongKong", code: "HK", spellings: &["香港", "香港特别行政区", "香港特別行政區"] },
    Division { name: "Heilongjiang", member: "Heilongjiang", code: "HL", spellings: &["黑龙江省", "黑龙江"] },
    Division { name: "Hunan", member: "Hunan", code: "HN", spellings: &["湖南省", "湖南"] },
    Division { name: "Jilin", member: "Jilin", code: "JL", spellings: &["吉林省", "吉林"] },
    Division { name: "Jiangsu", member: "Jiangsu", code: "JS", spellings: &["江苏省", "江苏"] },
    Division { name: "Jiangxi", member: "Jiangxi", code: "JX", spellings: &["江西省", "江西"] },
    Division { name: "Liaoning", member: "Liaoning", code: "LN", spellings: &["辽宁省", "辽宁"] },
    Division { name: "Macau", member: "Macau", code: "MO", spellings: &["澳門", "澳门", "澳门特别行政区", "澳門特別行政區", "Macao"] },
    Division { name: "Inner_Mongolia", member: "InnerMongolia", code: "NM", spellings: &["内蒙古自治区", "内蒙古", "Nei_Mongol"] },
    Division { name: "Ningxia", member: "Ningxia", code: "NX", spellings: &["宁夏回族自治区", "宁夏"] },
    Division { name: "Qinghai", member: "Qinghai", code: "QH", spellings: &["青海省", "青海"] },
    Division { name: "Sichuan", member: "Sichuan", code: "SC", spellings: &["四川省", "四川"] },
    Division { name: "Shandong", member: "Shandong", code: "SD", spellings: &["山东省", "山东"] },
    Division { name: "Shanghai", member: "Shanghai", code: "SH", spellings: &["上海市", "上海"] },
    Division { name: "Shaanxi", member: "Shaanxi", code: "SN", spellings: &["陕西省", "陕西"] },
    Division { name: "Shanxi", member: "Shanxi", code: "SX", spellings: &["山西省", "山西"] },
    Division { name: "Tianjin", member: "Tianjin", code: "TJ", spellings: &["天津市", "天津"] },
    Division { name: "Xinjiang", member: "Xinjiang", code: "XJ", spellings: &["新疆维吾尔自治区", "新疆"] },
    Division { name: "Tibet", member: "Tibet", code: "XZ", spellings: &["西藏自治区", "西藏", "Xizang"] },
    Division { name: "Yunnan", member: "Yunnan", code: "YN", spellings: &["云南省", "云南"] },
    Division { name: "Zhejiang", member: "Zhejiang", code: "ZJ", spellings: &["浙江省", "浙江"] },
];

const TW_DIVISIONS: &[Division] = &[
    Division { name: "Changhua", member: "Changhua", code: "CHA", spellings: &["彰化縣"] },
    Division { name: "Chiayi_City", member: "ChiayiCity", code: "CYI", spellings: &["嘉義市"] },
    Division { name: "Chiayi_County", member: "ChiayiCounty", code: "CYQ", spellings: &["嘉義縣"] },
    Division { name: "Hsinchu_County", member: "HsinchuCounty", code: "HSQ", spellings: &["新竹縣"] },
    Division { name: "Hsinchu_City", member: "HsinchuCity", code: "HSZ", spellings: &["新竹市"] },
    Division { name: "Hualien", member: "Hualien", code: "HUA", spellings: &["花蓮縣"] },
    Division { name: "Yilan", member: "Yilan", code: "ILA", spellings: &["宜蘭縣"] },
    Division { name: "Keelung", member: "Keelung", code: "KEE", spellings: &["基隆市"] },
    Division { name: "Kaohsiung", member: "Kaohsiung", code: "KHH", spellings: &["高雄市"] },
    Division { name: "Kinmen", member: "Kinmen", code: "KIN", spellings: &["金門縣"] },
    Division { name: "Lienchiang", member: "Lienchiang", code: "LIE", spellings: &["連江縣"] },
    Division { name: "Miaoli", member: "Miaoli", code: "MIA", spellings: &["苗栗縣"] },
    Division { name: "Nantou", member: "Nantou", code: "NAN", spellings: &["南投縣"] },
    Division { name: "New_Taipei", member: "NewTaipei", code: "NWT", spellings: &["新北市"] },
    Division { name: "Penghu", member: "Penghu", code: "PEN", spellings: &["澎湖縣"] },
    Division { name: "Pingtung", member: "Pingtung", code: "PIF", spellings: &["屏東縣"] },
    Division { name: "Taoyuan", member: "Taoyuan", code: "TAO", spellings: &["桃園市"] },
    Division { name: "Tainan", member: "Tainan", code: "TNN", spellings: &["臺南市", "台南市"] },
    Division { name: "Taipei", member: "Taipei", code: "TPE", spellings: &["臺北市", "台北市"] },
    Division { name: "Taitung", member: "Taitung", code: "TTT", spellings: &["臺東縣", "台東縣"] },
    Division { name: "Taichung", member: "Taichung", code: "TXG", spellings: &["臺中市", "台中市"] },
    Division { name: "Yunlin", member: "Yunlin", code: "YUN", spellings: &["雲林縣"] },
];

const KR_PROVINCES: &[Division] = &[
    Division { name: "Seoul", member: "Seoul", code: "", spellings: &["서울특별시", "서울"] },
    Division { name: "Busan", member: "Busan", code: "", spellings: &["부산광역시", "부산"] },
    Division { name: "Daegu", member: "Daegu", code: "", spellings: &["대구광역시", "대구"] },
    Division { name: "Incheon", member: "Incheon", code: "", spellings: &["인천광역시", "인천"] },
    Division { name: "Gwangju", member: "Gwangju", code: "", spellings: &["광주광역시", "광주", "Gwangju_City"] },
    Division { name: "Daejeon", member: "Daejeon", code: "", spellings: &["대전광역시", "대전"] },
    Division { name: "Ulsan", member: "Ulsan", code: "", spellings: &["울산광역시", "울산"] },
    Division { name: "Gyeonggi", member: "Gyeonggi", code: "", spellings: &["경기도", "경기"] },
    Division { name: "Gangwon", member: "Gangwon", code: "", spellings: &["강원도", "강원", "강원특별자치도"] },
    Division { name: "North_Chungcheong", member: "NorthChungcheong", code: "", spellings: &["충청북도", "충북"] },
    Division { name: "South_Chungcheong", member: "SouthChungcheong", code: "", spellings: &["충청남도", "충남"] },
    Division { name: "North_Jeolla", member: "NorthJeolla", code: "", spellings: &["전라북도", "전북", "전북특별자치도"] },
    Division { name: "South_Jeolla", member: "SouthJeolla", code: "", spellings: &["전라남도", "전남"] },
    Division { name: "North_Gyeongsang", member: "NorthGyeongsang", code: "", spellings: &["경상북도", "경북"] },
    Division { name: "South_Gyeongsang", member: "SouthGyeongsang", code: "", spellings: &["경상남도", "경남"] },
    Division { name: "Jeju", member: "Jeju", code: "", spellings: &["제주특별자치도", "제주"] },
    Division { name: "Sejong", member: "Sejong", code: "", spellings: &["세종특별자치시", "세종"] },
];

const IN_STATES: &[Division] = &[
    Division { name: "Andaman_and_Nicobar_Islands", member: "AndamanAndNicobarIslands", code: "AN", spellings: &["अण्डमान_और_निकोबार_द्वीपसमूह"] },
    Division { name: "Andhra_Pradesh", member: "AndhraPradesh", code: "AP", spellings: &["आन्ध्र_प्रदेश"] },
    Division { name: "Arunachal_Pradesh", member: "ArunachalPradesh", code: "AR", spellings: &["अरुणाचल_प्रदेश"] },
    Division { name: "Assam", member: "Assam", code: "AS", spellings: &["असम"] },
    Division { name: "Bihar", member: "Bihar", code: "BR", spellings: &["बिहार"] },
    Division { name: "Chhattisgarh", member: "Chhattisgarh", code: "CG", spellings: &["छत्तीसगढ़"] },
    Division { name: "Chandigarh", member: "Chandigarh", code: "CH", spellings: &["चण्डीगढ़"] },
    Division { name: "Dadra_and_Nagar_Haveli_and_Daman_and_Diu", member: "DadraAndNagarHaveliAndDamanAndDiu", code: "DH", spellings: &["दादरा_और_नगर_हवेली_और_दमन_और_दीव"] },
    Division { name: "Delhi", member: "Delhi", code: "DL", spellings: &["दिल्ली"] },
    Division { name: "Goa", member: "Goa", code: "GA", spellings: &["गोआ"] },
    Division { name: "Gujarat", member: "Gujarat", code: "GJ", spellings: &["गुजरात"] },
    Division { name: "Himachal_Pradesh", member: "HimachalPradesh", code: "HP", spellings: &["हिमाचल_प्रदेश"] },
    Division { name: "Haryana", member: "Haryana", code: "HR", spellings: &["हरियाणा"] },
    Division { name: "Jharkhand", member: "Jharkhand", code: "JH", spellings: &["झारखण्ड"] },
    Division { name: "Jammu_and_Kashmir", member: "JammuAndKashmir", code: "JK", spellings: &["जम्मू_और_कश्मीर"] },
    Division { name: "Karnataka", member: "Karnataka", code: "KA", spellings: &["कर्नाटक"] },
    Division { name: "Kerala", member: "Kerala", code: "KL", spellings: &["केरल"] },
    Division { name: "Ladakh", member: "Ladakh", code: "LA", spellings: &["लद्दाख़"] },
    Division { name: "Lakshadweep", member: "Lakshadweep", code: "LD", spellings: &["लक्षद्वीप"] },
    Division { name: "Maharashtra", member: "Maharashtra", code: "MH", spellings: &["महाराष्ट्र"] },
    Division { name: "Meghalaya", member: "Meghalaya", code: "ML", spellings: &["मेघालय"] },
    Division { name: "Manipur", member: "Manipur", code: "MN", spellings: &["मणिपुर"] },
    Division { name: "Madhya_Pradesh", member: "MadhyaPradesh", code: "MP", spellings: &["मध्य_प्रदेश"] },
    Division { name: "Mizoram", member: "Mizoram", code: "MZ", spellings: &["मिज़ोरम"] },
    Division { name: "Nagaland", member: "Nagaland", code: "NL", spellings: &["नागालैण्ड"] },
    Division { name: "Odisha", member: "Odisha", code: "OD", spellings: &["ओडिशा"] },
    Division { name: "Punjab", member: "Punjab", code: "PB", spellings: &["पंजाब"] },
    Division { name: "Puducherry", member: "Puducherry", code: "PY", spellings: &["पुदुच्चेरी"] },
    Division { name: "Rajasthan", member: "Rajasthan", code: "RJ", spellings: &["राजस्थान"] },
    Division { name: "Sikkim", member: "Sikkim", code: "SK", spellings: &["सिक्किम"] },
    Division { name: "Tamil_Nadu", member: "TamilNadu", code: "TN", spellings: &["तमिल_नाडु"] },
    Division { name: "Tripura", member: "Tripura", code: "TR", spellings: &["त्रिपुरा"] },
    Division { name: "Telangana", member: "Telangana", code: "TS", spellings: &["तेलंगाना"] },
    Division { name: "Uttarakhand", member: "Uttarakhand", code: "UK", spellings: &["उत्तराखण्ड"] },
    Division { name: "Uttar_Pradesh", member: "UttarPradesh", code: "UP", spellings: &["उत्तर_प्रदेश"] },
    Division { name: "West_Bengal", member: "WestBengal", code: "WB", spellings: &["पश्चिम_बंगाल"] },
];

const FR_REGIONS: &[Division] = &[
    Division { name: "Corse", member: "Corse", code: "", spellings: &[] },
    Division { name: "Guadeloupe", member: "Guadeloupe", code: "", spellings: &[] },
    Division { name: "Martinique", member: "Martinique", code: "", spellings: &[] },
    Division { name: "Guyane", member: "Guyane", code: "", spellings: &["Guyane_française", "French_Guiana", "Guyane_francaise"] },
    Division { name: "La_Reunion", member: "LaReunion", code: "", spellings: &["La_Réunion"] },
    Division { name: "Mayotte", member: "Mayotte", code: "", spellings: &[] },
    Division { name: "Auvergne_Rhone_Alpes", member: "AuvergneRhoneAlpes", code: "ARA", spellings: &["Auvergne_Rhône_Alpes"] },
    Division { name: "Burgundy_Franche_Comte", member: "BurgundyFrancheComte", code: "BFC", spellings: &["Bourgogne_Franche_Comté", "Bourgogne_Franche_Comte"] },
    Division { name: "Brittany", member: "Brittany", code: "BRE", spellings: &["Bretagne"] },
    Division { name: "Centre_Val_de_Loire", member: "CentreValDeLoire", code: "CVL", spellings: &[] },
    Division { name: "Grand_Est", member: "GrandEst", code: "GES", spellings: &[] },
    Division { name: "Hauts_de_France", member: "HautsDeFrance", code: "HDF", spellings: &[] },
    Division { name: "Ile_de_France", member: "IleDeFrance", code: "IDF", spellings: &["Île_de_France"] },
    Division { name: "Nouvelle_Aquitaine", member: "NouvelleAquitaine", code: "NAQ", spellings: &[] },
    Division { name: "Normandie", member: "Normandie", code: "NOR", spellings: &[] },
    Division { name: "Occitanie", member: "Occitanie", code: "OCC", spellings: &[] },
    Division { name: "Provence_Alpes_Cote_d_Azur", member: "ProvenceAlpesCoteDAzur", code: "PAC", spellings: &["Provence_Alpes_Côte_d’Azur", "Provence_Alpes_Côte_d'Azur"] },
    Division { name: "Pays_de_la_Loire", member: "PaysDeLaLoire", code: "PDL", spellings: &[] },
];

const ES_COMMUNITIES: &[Division] = &[
    Division { name: "Andalusia", member: "Andalusia", code: "AN", spellings: &["Andalucía", "Andalucia"] },
    Division { name: "Aragon", member: "Aragon", code: "AR", spellings: &["Aragón"] },
    Division { name: "Asturias", member: "Asturias", code: "AS", spellings: &["Principado_de_Asturias"] },
    Division { name: "Cantabria", member: "Cantabria", code: "CB", spellings: &[] },
    Division { name: "Ceuta", member: "Ceuta", code: "CE", spellings: &[] },
    Division { name: "Castile_and_Leon", member: "CastileAndLeon", code: "CL", spellings: &["Castilla_y_León", "Castilla_y_Leon"] },
    Division { name: "Castile_La_Mancha", member: "CastileLaMancha", code: "CM", spellings: &["Castilla_La_Mancha"] },
    Division { name: "Canary_Islands", member: "CanaryIslands", code: "CN", spellings: &["Canarias"] },
    Division { name: "Catalonia", member: "Catalonia", code: "CT", spellings: &["Cataluña", "Catalunya", "Cataluna"] },
    Division { name: "Extremadura", member: "Extremadura", code: "EX", spellings: &[] },
    Division { name: "Galicia", member: "Galicia", code: "GA", spellings: &[] },
    Division { name: "Balearic_Islands", member: "BalearicIslands", code: "IB", spellings: &["Islas_Baleares", "Illes_Balears"] },
    Division { name: "Murcia", member: "Murcia", code: "MC", spellings: &["Región_de_Murcia", "Murcia_Region", "Region_de_Murcia"] },
    Division { name: "Madrid", member: "Madrid", code: "MD", spellings: &["Comunidad_de_Madrid", "Madrid_Autonomous_Community"] },
    Division { name: "Melilla", member: "Melilla", code: "ML", spellings: &[] },
    Division { name: "Navarra", member: "Navarra", code: "NC", spellings: &["Nafarroako_Foru_Erkidegoa", "Navarra_Chartered_Community", "Navarre"] },
    Division { name: "Basque_Country", member: "BasqueCountry", code: "PV", spellings: &["País_Vasco", "Euskal_Autonomia_Erkidegoa", "Euskadi", "Pais_Vasco"] },
    Division { name: "La_Rioja", member: "LaRioja", code: "RI", spellings: &[] },
    Division { name: "Valencian_Community", member: "ValencianCommunity", code: "VC", spellings: &["Comunidad_Valenciana", "País_Valencià", "Comunitat_Valenciana", "Pais_Valencia"] },
];

const IT_REGIONS: &[Division] = &[
    Division { name: "Piedmont", member: "Piedmont", code: "", spellings: &["Piemonte"] },
    Division { name: "Aosta_Valley", member: "AostaValley", code: "", spellings: &["Valle_d’Aosta", "Valle_d'Aosta", "Vallée_d’Aoste", "Vallée_d'Aoste", "Valle_d_Aosta", "Vallee_d_Aoste"] },
    Division { name: "Lombardy", member: "Lombardy", code: "", spellings: &["Lombardia"] },
    Division { name: "Trentino_South_Tyrol", member: "TrentinoSouthTyrol", code: "", spellings: &["Trentino_Alto_Adige", "Trentino_Südtirol", "Trentino_Sudtirol"] },
    Division { name: "Veneto", member: "Veneto", code: "", spellings: &[] },
    Division { name: "Friuli_Venezia_Giulia", member: "FriuliVeneziaGiulia", code: "", spellings: &[] },
    Division { name: "Liguria", member: "Liguria", code: "", spellings: &[] },
    Division { name: "Emilia_Romagna", member: "EmiliaRomagna", code: "", spellings: &[] },
    Division { name: "Tuscany", member: "Tuscany", code: "", spellings: &["Toscana"] },
    Division { name: "Umbria", member: "Umbria", code: "", spellings: &[] },
    Division { name: "Marche", member: "Marche", code: "", spellings: &[] },
    Division { name: "Lazio", member: "Lazio", code: "", spellings: &[] },
    Division { name: "Abruzzo", member: "Abruzzo", code: "", spellings: &[] },
    Division { name: "Molise", member: "Molise", code: "", spellings: &[] },
    Division { name: "Campania", member: "Campania", code: "", spellings: &[] },
    Division { name: "Apulia", member: "Apulia", code: "", spellings: &["Puglia"] },
    Division { name: "Basilicata", member: "Basilicata", code: "", spellings: &[] },
    Division { name: "Calabria", member: "Calabria", code: "", spellings: &[] },
    Division { name: "Sicily", member: "Sicily", code: "", spellings: &["Regione_Siciliana", "Sicilia"] },
    Division { name: "Sardinia", member: "Sardinia", code: "", spellings: &["Sardegna"] },
];

const DE_STATES: &[Division] = &[
    Division { name: "Brandenburg", member: "Brandenburg", code: "BB", spellings: &[] },
    Division { name: "Berlin", member: "Berlin", code: "BE", spellings: &[] },
    Division { name: "Baden_Wurttemberg", member: "BadenWurttemberg", code: "BW", spellings: &["Baden_Württemberg", "Baden_Wuerttemberg"] },
    Division { name: "Bavaria", member: "Bavaria", code: "BY", spellings: &["Bayern"] },
    Division { name: "Bremen", member: "Bremen", code: "HB", spellings: &[] },
    Division { name: "Hesse", member: "Hesse", code: "HE", spellings: &["Hessen"] },
    Division { name: "Hamburg", member: "Hamburg", code: "HH", spellings: &[] },
    Division { name: "Mecklenburg_Vorpommern", member: "MecklenburgVorpommern", code: "MV", spellings: &[] },
    Division { name: "Lower_Saxony", member: "LowerSaxony", code: "NI", spellings: &["Niedersachsen"] },
    Division { name: "North_Rhine_Westphalia", member: "NorthRhineWestphalia", code: "NW", spellings: &["Nordrhein_Westfalen"] },
    Division { name: "Rhineland_Palatinate", member: "RhinelandPalatinate", code: "RP", spellings: &["Rheinland_Pfalz"] },
    Division { name: "Schleswig_Holstein", member: "SchleswigHolstein", code: "SH", spellings: &[] },
    Division { name: "Saarland", member: "Saarland", code: "SL", spellings: &[] },
    Division { name: "Saxony", member: "Saxony", code: "SN", spellings: &["Sachsen"] },
    Division { name: "Saxony_Anhalt", member: "SaxonyAnhalt", code: "ST", spellings: &["Sachsen_Anhalt"] },
    Division { name: "Thuringia", member: "Thuringia", code: "TH", spellings: &["Thüringen", "Thueringen", "Thuringen"] },
];

const AU_STATES: &[Division] = &[
    Division { name: "Australian_Capital_Territory", member: "AustralianCapitalTerritory", code: "ACT", spellings: &[] },
    Division { name: "New_South_Wales", member: "NewSouthWales", code: "NSW", spellings: &[] },
    Division { name: "Northern_Territory", member: "NorthernTerritory", code: "NT", spellings: &[] },
    Division { name: "Queensland", member: "Queensland", code: "QLD", spellings: &[] },
    Division { name: "South_Australia", member: "SouthAustralia", code: "SA", spellings: &[] },
    Division { name: "Tasmania", member: "Tasmania", code: "TAS", spellings: &[] },
    Division { name: "Victoria", member: "Victoria", code: "VIC", spellings: &[] },
    Division { name: "Western_Australia", member: "WesternAustralia", code: "WA", spellings: &[] },
];

const BR_STATES: &[Division] = &[
    Division { name: "Acre", member: "Acre", code: "AC", spellings: &[] },
    Division { name: "Alagoas", member: "Alagoas", code: "AL", spellings: &[] },
    Division { name: "Amazonas", member: "Amazonas", code: "AM", spellings: &[] },
    Division { name: "Amapa", member: "Amapa", code: "AP", spellings: &["Amapá"] },
    Division { name: "Bahia", member: "Bahia", code: "BA", spellings: &[] },
    Division { name: "Ceara", member: "Ceara", code: "CE", spellings: &["Ceará"] },
    Division { name: "Federal_District", member: "FederalDistrict", code: "DF", spellings: &["Distrito_Federal"] },
    Division { name: "Espirito_Santo", member: "EspiritoSanto", code: "ES", spellings: &["Espírito_Santo"] },
    Division { name: "Goias", member: "Goias", code: "GO", spellings: &["Goiás"] },
    Division { name: "Maranhao", member: "Maranhao", code: "MA", spellings: &["Maranhão"] },
    Division { name: "Minas_Gerais", member: "MinasGerais", code: "MG", spellings: &[] },
    Division { name: "Mato_Grosso_do_Sul", member: "MatoGrossoDoSul", code: "MS", spellings: &[] },
    Division { name: "Mato_Grosso", member: "MatoGrosso", code: "MT", spellings: &[] },
    Division { name: "Para", member: "Para", code: "PA", spellings: &["Pará"] },
    Division { name: "Paraiba", member: "Paraiba", code: "PB", spellings: &["Paraíba"] },
    Division { name: "Pernambuco", member: "Pernambuco", code: "PE", spellings: &[] },
    Division { name: "Piaui", member: "Piaui", code: "PI", spellings: &["Piauí"] },
    Division { name: "Parana", member: "Parana", code: "PR", spellings: &["Paraná"] },
    Division { name: "Rio_de_Janeiro", member: "RioDeJaneiro", code: "RJ", spellings: &[] },
    Division { name: "Rio_Grande_do_Norte", member: "RioGrandeDoNorte", code: "RN", spellings: &[] },
    Division { name: "Rondonia", member: "Rondonia", code: "RO", spellings: &["Rondônia"] },
    Division { name: "Roraima", member: "Roraima", code: "RR", spellings: &[] },
    Division { name: "Rio_Grande_do_Sul", member: "RioGrandeDoSul", code: "RS", spellings: &[] },
    Division { name: "Santa_Catarina", member: "SantaCatarina", code: "SC", spellings: &[] },
    Division { name: "Sergipe", member: "Sergipe", code: "SE", spellings: &[] },
    Division { name: "Sao_Paulo", member: "SaoPaulo", code: "SP", spellings: &["São_Paulo"] },
    Division { name: "Tocantins", member: "Tocantins", code: "TO", spellings: &[] },
];

const JP_PREFECTURES: &[Division] = &[
    Division { name: "Hokkaido", member: "Hokkaido", code: "", spellings: &["北海道"] },
    Division { name: "Aomori", member: "Aomori", code: "", spellings: &["青森県"] },
    Division { name: "Iwate", member: "Iwate", code: "", spellings: &["岩手県"] },
    Division { name: "Miyagi", member: "Miyagi", code: "", spellings: &["宮城県"] },
    Division { name: "Akita", member: "Akita", code: "", spellings: &["秋田県"] },
    Division { name: "Yamagata", member: "Yamagata", code: "", spellings: &["山形県"] },
    Division { name: "Fukushima", member: "Fukushima", code: "", spellings: &["福島県"] },
    Division { name: "Ibaraki", member: "Ibaraki", code: "", spellings: &["茨城県"] },
    Division { name: "Tochigi", member: "Tochigi", code: "", spellings: &["栃木県"] },
    Division { name: "Gunma", member: "Gunma", code: "", spellings: &["群馬県"] },
    Division { name: "Saitama", member: "Saitama", code: "", spellings: &["埼玉県"] },
    Division { name: "Chiba", member: "Chiba", code: "", spellings: &["千葉県"] },
    Division { name: "Tokyo", member: "Tokyo", code: "", spellings: &["東京都"] },
    Division { name: "Kanagawa", member: "Kanagawa", code: "", spellings: &["神奈川県"] },
    Division { name: "Niigata", member: "Niigata", code: "", spellings: &["新潟県"] },
    Division { name: "Toyama", member: "Toyama", code: "", spellings: &["富山県"] },
    Division { name: "Ishikawa", member: "Ishikawa", code: "", spellings: &["石川県"] },
    Division { name: "Fukui", member: "Fukui", code: "", spellings: &["福井県"] },
    Division { name: "Yamanashi", member: "Yamanashi", code: "", spellings: &["山梨県"] },
    Division { name: "Nagano", member: "Nagano", code: "", spellings: &["長野県"] },
    Division { name: "Gifu", member: "Gifu", code: "", spellings: &["岐阜県"] },
    Division { name: "Shizuoka", member: "Shizuoka", code: "", spellings: &["静岡県"] },
    Division { name: "Aichi", member: "Aichi", code: "", spellings: &["愛知県"] },
    Division { name: "Mie", member: "Mie", code: "", spellings: &["三重県"] },
    Division { name: "Shiga", member: "Shiga", code: "", spellings: &["滋賀県"] },
    Division { name: "Kyoto", member: "Kyoto", code: "", spellings: &["京都府"] },
    Division { name: "Osaka", member: "Osaka", code: "", spellings: &["大阪府"] },
    Division { name: "Hyogo", member: "Hyogo", code: "", spellings: &["兵庫県"] },
    Division { name: "Nara", member: "Nara", code: "", spellings: &["奈良県"] },
    Division { name: "Wakayama", member: "Wakayama", code: "", spellings: &["和歌山県"] },
    Division { name: "Tottori", member: "Tottori", code: "", spellings: &["鳥取県"] },
    Division { name: "Shimane", member: "Shimane", code: "", spellings: &["島根県"] },
    Division { name: "Okayama", member: "Okayama", code: "", spellings: &["岡山県"] },
    Division { name: "Hiroshima", member: "Hiroshima", code: "", spellings: &["広島県"] },
    Division { name: "Yamaguchi", member: "Yamaguchi", code: "", spellings: &["山口県"] },
    Division { name: "Tokushima", member: "Tokushima", code: "", spellings: &["徳島県"] },
    Division { name: "Kagawa", member: "Kagawa", code: "", spellings: &["香川県"] },
    Division { name: "Ehime", member: "Ehime", code: "", spellings: &["愛媛県"] },
    Division { name: "Kochi", member: "Kochi", code: "", spellings: &["高知県"] },
    Division { name: "Fukuoka", member: "Fukuoka", code: "", spellings: &["福岡県"] },
    Division { name: "Saga", member: "Saga", code: "", spellings: &["佐賀県"] },
    Division { name: "Nagasaki", member: "Nagasaki", code: "", spellings: &["長崎県"] },
    Division { name: "Kumamoto", member: "Kumamoto", code: "", spellings: &["熊本県"] },
    Division { name: "Oita", member: "Oita", code: "", spellings: &["大分県"] },
    Division { name: "Miyazaki", member: "Miyazaki", code: "", spellings: &["宮崎県"] },
    Division { name: "Kagoshima", member: "Kagoshima", code: "", spellings: &["鹿児島県"] },
    Division { name: "Okinawa", member: "Okinawa", code: "", spellings: &["沖縄県"] },
];
