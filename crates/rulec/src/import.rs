//! `rulec import csv`: a first draft of a `.rule` from a spreadsheet's CSV export (§1.4,
//! §15.41).
//!
//! The draft is a **starting point, not a rule**: every column's type, every range and every
//! rounding here is a guess from the values seen, and each guess carries a `# 推定` comment
//! so that `rulec doc` shows it to its readers and `rulec check` names what is missing. The
//! importer never decides an amount, a rounding or a threshold; it lays the values out in the
//! shape the language wants and stops.

/// One field, classified.
#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Empty,
    /// A number with its unit as written after it (`1200円` → ("1200", "円")).
    Num(String, String),
    Date(String),
    Word(String),
}

fn classify(raw: &str) -> Cell {
    let s = raw.trim();
    if s.is_empty() {
        return Cell::Empty;
    }
    // A date: YYYY-MM-DD or YYYY/MM/DD.
    let parts: Vec<&str> = s.split(['-', '/']).collect();
    if parts.len() == 3 && parts[0].len() == 4 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        return Cell::Date(format!("{}-{:0>2}-{:0>2}", parts[0], parts[1], parts[2]));
    }
    // A number: an optional sign, digits with thousands separators, an optional fraction,
    // then whatever unit follows.
    let body = s.replace(',', "");
    let mut chars = body.char_indices().peekable();
    let mut end = 0;
    if let Some((_, '-')) = chars.peek().copied() {
        chars.next();
        end = 1;
    }
    let mut digits = 0;
    let mut dot = false;
    for (i, c) in chars {
        if c.is_ascii_digit() {
            digits += 1;
            end = i + 1;
        } else if c == '.' && !dot && digits > 0 {
            dot = true;
            end = i + 1;
        } else {
            break;
        }
    }
    if digits > 0 && !body[..end].ends_with('.') {
        let unit = body[end..].trim().replace('％', "%");
        // Unicode files ℃, ㎡ and their kin as symbols rather than letters, so an
        // alphanumeric test alone turned a whole column of `120㎡` into an enum of strings.
        // They are folded to one spelling in `num_type`; this is what lets them reach it.
        let compat = |c: char| matches!(c, '℃' | '℉' | '㎡' | '㎢' | '㎠' | '㎥' | '㎤' | '㎖' | '°');
        if unit.chars().all(|c| c.is_alphanumeric() || compat(c) || c == '%' || c == '_') {
            return Cell::Num(body[..end].to_string(), unit);
        }
    }
    Cell::Word(s.to_string())
}

/// RFC 4180, near enough: quotes, doubled quotes inside them, commas, CRLF, a BOM.
pub(crate) fn parse_csv(src: &str) -> Vec<Vec<String>> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.into_iter().filter(|r| r.iter().any(|f| !f.trim().is_empty())).collect()
}

/// What a column holds, decided from every cell in it.
enum Column {
    Num { unit: String, lo: String, hi: String, decimal: bool },
    Date,
    Enum(Vec<String>),
}

fn column_kind(cells: &[Cell]) -> Column {
    let filled: Vec<&Cell> = cells.iter().filter(|c| **c != Cell::Empty).collect();
    let nums: Vec<(&String, &String)> = filled.iter().filter_map(|c| if let Cell::Num(v, u) = c { Some((v, u)) } else { None }).collect();
    if !nums.is_empty() && nums.len() == filled.len() && nums.iter().all(|(_, u)| *u == nums[0].1) {
        let cmp = |a: &str, b: &str| a.parse::<f64>().unwrap_or(0.0).partial_cmp(&b.parse::<f64>().unwrap_or(0.0)).unwrap();
        let mut vs: Vec<&str> = nums.iter().map(|(v, _)| v.as_str()).collect();
        vs.sort_by(|a, b| cmp(a, b));
        return Column::Num {
            unit: nums[0].1.clone(),
            lo: vs[0].to_string(),
            hi: vs[vs.len() - 1].to_string(),
            decimal: vs.iter().any(|v| v.contains('.')),
        };
    }
    if !filled.is_empty() && filled.iter().all(|c| matches!(c, Cell::Date(_))) {
        return Column::Date;
    }
    let mut vals: Vec<String> = Vec::new();
    for c in &filled {
        let w = match c {
            Cell::Num(v, u) => format!("{v}{u}"),
            Cell::Date(d) => d.clone(),
            Cell::Word(w) => w.clone(),
            Cell::Empty => continue,
        };
        if !vals.contains(&w) {
            vals.push(w);
        }
    }
    Column::Enum(vals)
}

/// The type a numeric column is declared with, from its unit, and the note that goes with
/// it when the unit had to be guessed at.
/// A workbook states a unit in its number format (`#,##0"㎡"`), and writes it the way a
/// reader expects: the squared metre as one character, the litre as ℓ, the degree with a
/// ring. The language keeps one spelling each, so they are folded here — at the boundary,
/// where the workbook's own habits stop. Both the declared type and the values written into
/// the draft go through this, or the draft names a type it then cannot parse a bound for.
fn canonical_unit(u: &str) -> &str {
    match u {
        "㎡" | "m²" => "m2",
        "㎢" | "km²" => "km2",
        "㎠" | "cm²" => "cm2",
        "㎥" | "m³" => "m3",
        "㎤" | "cm³" => "cm3",
        "ℓ" | "l" => "L",
        "㎖" | "ml" => "mL",
        "°C" => "℃",
        "°F" => "℉",
        "db" => "dB",
        u => u,
    }
}

fn num_type(unit: &str, decimal: bool) -> (String, Option<String>) {
    let unit = canonical_unit(unit);
    match unit {
        // the currency spelled as the sheet spells it: 円 and JPY are the same to rulec
        "円" | "JPY" => (format!("money[{unit}, incl_tax]"), Some(tr!("推定: 税込か税抜かは出典で確かめること", "guess: whether tax is included has to come from the source"))),
        "%" => (
            if decimal { "rate[step 0.1%]" } else { "rate[step 1%]" }.into(),
            Some(tr!("推定: 刻みは見た値から", "guess: the step is taken from the values seen")),
        ),
        "g" | "kg" | "mg" | "t" | "lb" | "oz" => (format!("mass[{unit}]"), None),
        "cm" | "mm" | "m" | "km" | "in" | "ft" | "yd" | "mi" => (format!("length[{unit}]"), None),
        "mm2" | "cm2" | "m2" | "a" | "ha" | "km2" | "坪" | "in2" | "ft2" | "yd2" | "mi2" | "ac" => {
            (format!("area[{unit}]"), None)
        }
        "mm3" | "cm3" | "m3" | "mL" | "L" | "kL" => (format!("volume[{unit}]"), None),
        "ms" | "s" | "min" | "h" | "d" | "w" => (format!("duration[{unit}]"), None),
        "℃" | "℉" => (format!("temperature[{unit}]"), None),
        "dB" => (format!("sound[{unit}]"), None),
        "" => ("number".into(), Some(tr!("推定: 単位が無いので number にした", "guess: no unit was written, so it is a number"))),
        u if u.len() == 3 && u.chars().all(|c| c.is_ascii_uppercase()) => (format!("money[{u}]"), None),
        u => ("number".into(), Some(tr!("推定: 単位 {u} は rulec に無いので number にした", "guess: the unit {u} is not one rulec knows, so it is a number"))),
    }
}

/// Several notes on one line, the `guess:` prefix said once.
fn join_notes(notes: &[String]) -> String {
    let prefix = tr!("推定: ", "guess: ");
    notes
        .iter()
        .enumerate()
        .map(|(i, n)| if i > 0 { n.strip_prefix(&prefix).unwrap_or(n).to_string() } else { n.clone() })
        .collect::<Vec<_>>()
        .join(&tr!("。", "; "))
}

fn ascii_ok(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// How a word of the sheet is declared, and what the draft refers to it by (§15.259).
struct Named {
    /// What its declaration writes: `重量(c2)`, `count_(count)`, `weight`, or a name of its own.
    decl: String,
    /// What a table's head and its cells write for it — the places that *refer* to a declaration
    /// rather than make one. It has to agree with the declaration, or the draft refers to a column
    /// it never declared.
    name: String,
    /// The word as the sheet wrote it, when the lexer does not read it as a name.
    unread: Option<String>,
}

/// A word of the sheet as a declared name, with `alias` beside it where one is needed.
///
/// Whether the word can stand as a name is the lexer's to say (`lex::is_bare_word`): `Weight (kg)`,
/// `re-quote`, `2FA` or `1200円` would be read as more than one word or as none, and `check` could
/// not read the draft back (E057, E058, E063). Such a word is named by `alias` itself — the
/// column's (`c2`) or the value's (`v1`) — and the word is kept in a `# 推定` note.
///
/// A name cannot be one of the language's own words (E009) either: the draft used to call its own
/// table `table` in English, so `rulec import csv` produced a draft that would not parse — and a
/// column headed `count` or `source` did the same. A reserved word keeps its spelling as the alias,
/// which is what the generated code calls the column, and the name gains the `_` that makes it a
/// name.
fn named(text: &str, alias: &str) -> Named {
    if !crate::lex::is_bare_word(text) {
        return Named { decl: alias.to_string(), name: alias.to_string(), unread: Some(text.to_string()) };
    }
    if !ascii_ok(text) {
        return Named { decl: format!("{text}({alias})"), name: text.to_string(), unread: None };
    }
    if crate::kw::RESERVED.contains(&text) {
        return Named { decl: format!("{text}_({text})"), name: format!("{text}_"), unread: None };
    }
    Named { decl: text.to_string(), name: text.to_string(), unread: None }
}

/// The words of one kind — the columns, or the values of one column — named together, each with
/// its alias. A name made of an alias is set apart from the names of the others by `_`: a sheet may
/// well head a column `c2`, or hold a value `v1`.
fn named_all(words: &[(String, String)]) -> Vec<Named> {
    let mut out: Vec<Named> = words.iter().map(|(w, a)| named(w, a)).collect();
    for i in 0..out.len() {
        if out[i].unread.is_none() {
            continue;
        }
        while out.iter().enumerate().any(|(j, o)| j != i && o.name == out[i].name) {
            out[i].name.push('_');
        }
        out[i].decl = out[i].name.clone();
    }
    out
}

/// A word of the sheet as a comment or a string of the draft writes it: a line break or another
/// control character would end the line there, and what follows would be read as a line of the
/// rule.
fn one_line(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// The draft, from a CSV. `outputs` is how many of the trailing columns are outputs.
pub fn draft(csv: &str, name: &str, source: &str, outputs: usize) -> Result<String, String> {
    draft_rows(parse_csv(csv), name, source, outputs)
}

/// The same draft from a grid that came from somewhere else — a sheet of an `.xlsx`
/// (§15.50). Everything below the reading of the file is shared: one importer, whichever
/// door the table came through.
pub fn draft_rows(
    rows: Vec<Vec<String>>,
    name: &str,
    source: &str,
    outputs: usize,
) -> Result<String, String> {
    let Some(header) = rows.first() else {
        return Err(tr!("表が空です", "the table is empty"));
    };
    let ncol = header.len();
    if ncol < 2 {
        return Err(tr!("列が二つ以上要ります（入力と出力）", "at least two columns are needed (inputs and an output)"));
    }
    if outputs == 0 || outputs >= ncol {
        return Err(tr!("出力の列数 {outputs} は 1 以上 {} 以下です", "the number of output columns, {outputs}, has to be between 1 and {}", ncol - 1));
    }
    let data: Vec<Vec<Cell>> = rows[1..]
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if r.len() != ncol {
                return Err(tr!("{} 行目の列数が見出しと違います（{} と {ncol}）", "row {} has {} columns where the header has {ncol}", i + 2, r.len()));
            }
            Ok(r.iter().map(|c| classify(c)).collect())
        })
        .collect::<Result<_, _>>()?;
    if data.is_empty() {
        return Err(tr!("見出しの下に行がありません", "there are no rows under the header"));
    }
    let n_in = ncol - outputs;
    let cols: Vec<Column> = (0..ncol).map(|k| column_kind(&data.iter().map(|r| r[k].clone()).collect::<Vec<_>>())).collect();
    let alias = |k: usize| if k < n_in { format!("c{}", k + 1) } else { format!("o{}", k + 1 - n_in) };
    let guess = tr!("推定", "guess");
    // Every column by the name its header gives it, or by its alias where the lexer does not read
    // the header as a name (§15.259).
    let columns = named_all(&(0..ncol).map(|k| (header[k].trim().to_string(), alias(k))).collect::<Vec<_>>());
    // The note a column named by its alias carries: the header as the sheet wrote it.
    let header_note = |k: usize| {
        columns[k].unread.as_ref().map(|w| {
            let (w, n) = (one_line(w), &columns[k].name);
            if w.is_empty() {
                tr!("{guess}: 見出しが空なので、仮に {n} と名付けた", "{guess}: the header is empty, so the column is named {n} for now")
            } else {
                tr!("{guess}: 見出しの「{w}」は名前として読めないので、仮に {n} と名付けた", "{guess}: the header \"{w}\" is not read as a name, so the column is named {n} for now")
            }
        })
    };

    let mut o = String::new();
    let rule = named(name, "imported");
    o.push_str(&format!("rule {} v1", rule.decl));
    if let Some(w) = &rule.unread {
        let (w, n) = (one_line(w), &rule.name);
        o.push_str(&format!("  # {}", tr!("{guess}: 規則の名前「{w}」は名前として読めないので、仮に {n} と名付けた", "{guess}: the rule's name \"{w}\" is not read as a name, so the rule is named {n} for now")));
    }
    o.push('\n');
    // The file's name inside a string: a `"` would end the string there.
    let source_said = one_line(source).replace('"', "'");
    o.push_str(&tr!(
        "description \"{source_said} から rulec import が起こした下書き。「{guess}」と書いた行は全部、人が確かめること\"\n",
        "description \"A draft that rulec import made from {source_said}. Every line marked {guess} is for a person to confirm\"\n"
    ));

    // Enums, one per column of words: each value by its name, or by `v1`, `v2` … where the lexer
    // does not read it as a name.
    let mut enum_of: Vec<Option<String>> = vec![None; ncol];
    let mut values_of: Vec<Option<(Vec<String>, Vec<Named>)>> = (0..ncol).map(|_| None).collect();
    let mut enums = String::new();
    for (k, col) in cols.iter().enumerate() {
        if let Column::Enum(vals) = col {
            // named after the header as written, or after the column's name where it is not a name
            let base = if columns[k].unread.is_some() { columns[k].name.clone() } else { header[k].trim().to_string() };
            let e = named(&tr!("{}の値", "{}_values", base), &format!("{}_kind", alias(k)));
            let members = named_all(&vals.iter().enumerate().map(|(i, v)| (v.clone(), format!("v{}", i + 1))).collect::<Vec<_>>());
            let mut notes = vec![tr!("{guess}: この列に現れた値をそのまま列挙にした。値が足りなければ足し、別名は付け直すこと", "{guess}: the values seen in this column, as an enum; add what is missing, and rename the aliases")];
            let unread: Vec<(String, String)> = members.iter().filter_map(|m| m.unread.as_ref().map(|w| (m.name.clone(), one_line(w)))).collect();
            if !unread.is_empty() {
                let ja: Vec<String> = unread.iter().map(|(n, w)| format!("{n} は「{w}」")).collect();
                let en: Vec<String> = unread.iter().map(|(n, w)| format!("{n} is \"{w}\"")).collect();
                let (ja, en) = (ja.join("、"), en.join(", "));
                notes.push(tr!("{guess}: 名前として読めない値には仮の名前を付けた（{ja}）", "{guess}: a value not read as a name has a name for now ({en})"));
            }
            let decls: Vec<&str> = members.iter().map(|m| m.decl.as_str()).collect();
            enums.push_str(&format!("enum {} = {}  # {}\n", e.decl, decls.join(" | "), join_notes(&notes)));
            enum_of[k] = Some(e.name);
            values_of[k] = Some((vals.clone(), members));
        }
    }
    if !enums.is_empty() {
        o.push('\n');
        o.push_str(&enums);
    }

    // Declarations.
    let decl = |k: usize| -> (String, String) {
        match &cols[k] {
            Column::Num { unit, lo, hi, decimal } => {
                let (ty, note) = num_type(unit, *decimal);
                let u = if ty.starts_with("number") { String::new() } else if ty.starts_with("rate") { "%".into() } else { canonical_unit(unit).to_string() };
                (ty, format!("{lo}{u}|{hi}{u}|{}", note.unwrap_or_default()))
            }
            Column::Date => ("date".into(), String::new()),
            Column::Enum(_) => (enum_of[k].clone().unwrap_or_default(), String::new()),
        }
    };
    o.push_str("\ninputs\n");
    for k in 0..n_in {
        let (ty, extra) = decl(k);
        let mut line = format!("  {} : {ty}", columns[k].decl);
        let mut notes: Vec<String> = header_note(k).into_iter().collect();
        if let Column::Num { .. } = cols[k] {
            let parts: Vec<&str> = extra.split('|').collect();
            line.push_str(&format!("  range >={} <={}", parts[0], parts[1]));
            notes.push(tr!("{guess}: 範囲は見た値の最小と最大。実際の範囲は出典で決めること", "{guess}: the range is the smallest and largest value seen; the real range comes from the source"));
            if !parts[2].is_empty() {
                notes.push(parts[2].to_string());
            }
        }
        if let Column::Date = cols[k] {
            let dates: Vec<String> = data.iter().filter_map(|r| if let Cell::Date(d) = &r[k] { Some(d.clone()) } else { None }).collect();
            let (lo, hi) = (dates.iter().min().cloned().unwrap_or_default(), dates.iter().max().cloned().unwrap_or_default());
            line.push_str(&format!("  range >={lo} <={hi}"));
            notes.push(tr!("{guess}: 範囲は見た日付の最初と最後", "{guess}: the range is the first and last date seen"));
        }
        if !notes.is_empty() {
            line.push_str(&format!("  # {}", join_notes(&notes)));
        }
        o.push_str(&line);
        o.push('\n');
    }
    o.push_str("\noutputs\n");
    for k in n_in..ncol {
        let (ty, extra) = decl(k);
        let mut line = format!("  {} : {ty}", columns[k].decl);
        let mut notes: Vec<String> = header_note(k).into_iter().collect();
        if let Column::Num { unit, decimal, .. } = &cols[k] {
            // The grid a rate output can be rounded to is its step; anything coarser would
            // put the values seen off the grid (E106).
            let grid = if ty.starts_with("number") {
                "1".to_string()
            } else if ty.starts_with("rate") {
                if *decimal { "0.1%" } else { "1%" }.to_string()
            } else {
                format!("1{unit}")
            };
            line.push_str(&format!("  round down({grid})"));
            notes.push(tr!("{guess}: 丸めの向きと刻みは出典で決めること。無ければ仮置きだと書き残すこと", "{guess}: the rounding's direction and grid come from the source; if it has none, write down that this is a placeholder"));
            let note = extra.split('|').nth(2).unwrap_or("");
            if !note.is_empty() {
                notes.push(note.to_string());
            }
        }
        if !notes.is_empty() {
            line.push_str(&format!("  # {}", join_notes(&notes)));
        }
        o.push_str(&line);
        o.push('\n');
    }

    // The table: one row per line of the file, every cell an equality.
    let source_line = one_line(source);
    o.push_str(&format!(
        "\ntable {}  # {}\n",
        named(&tr!("表", "decision"), "t").decl,
        tr!("出典: {source_line}（{guess}: 出典の文書名と日付に書き換えること）", "source: {source_line} ({guess}: replace with the document's name and date)")
    ));
    o.push_str("policy unique\n");
    let mut head: Vec<String> = (0..n_in).map(|k| columns[k].name.clone()).collect();
    for k in n_in..ncol {
        let (ty, _) = decl(k);
        head.push(format!("{}{} : {ty}", if k == n_in { "-> " } else { "" }, columns[k].decl));
    }
    o.push_str(&format!("| {} |\n", head.join(" | ")));
    for (i, r) in data.iter().enumerate() {
        let mut cells: Vec<String> = Vec::new();
        for (k, c) in r.iter().enumerate() {
            let text = match (c, &cols[k]) {
                (Cell::Empty, _) if k < n_in => "-".to_string(),
                (Cell::Empty, _) => {
                    return Err(tr!("{} 行目の出力の列 {} が空です", "row {}: the output column {} is empty", i + 2, header[k].trim()));
                }
                (Cell::Num(v, u), Column::Num { .. }) => {
                    let (ty, _) = num_type(u, v.contains('.'));
                    let u = canonical_unit(u);
                    if ty.starts_with("number") { v.clone() } else if ty.starts_with("rate") { format!("{v}%") } else { format!("{v}{u}") }
                }
                (Cell::Date(d), Column::Date) => d.clone(),
                // A cell of a column of words names the value, by the name its enum declares: a
                // number or a date among words is one of the values too.
                (c, _) => {
                    let seen = match c {
                        Cell::Num(v, u) => format!("{v}{u}"),
                        Cell::Date(d) => d.clone(),
                        Cell::Word(w) => w.clone(),
                        Cell::Empty => unreachable!("an empty cell is `-` or refused above"),
                    };
                    match &values_of[k] {
                        Some((vals, members)) => vals.iter().position(|v| *v == seen).map(|p| members[p].name.clone()).unwrap_or(seen),
                        None => seen,
                    }
                }
            };
            cells.push(text);
        }
        o.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    if cols[..n_in].iter().any(|c| matches!(c, Column::Num { .. })) {
        o.push_str(&tr!(
            "# {guess}: 数値の列のセルは、書いてあった値との等値として転記した。閾値の意味なら `<=60cm` のような比較に書き換えること\n",
            "# {guess}: a cell in a numeric column was copied as equality with the value written; if it means a threshold, rewrite it as a comparison such as `<=60cm`\n"
        ));
    }
    // Written the way `rulec fmt` would leave it: a draft is read before anything else is
    // done to it, and a table whose bars wander is harder to hold against the sheet.
    Ok(crate::fmt::format(&o))
}
