//! `rulec import csv`: a first draft of a `.rule` from a spreadsheet's CSV export (§1.4,
//! §15.41).
//!
//! The draft is a **starting point, not a rule**: every column's type, every range and every
//! rounding here is a guess from the values seen, and each guess carries a `# 推定` comment
//! so that `rulec doc` shows it to its readers and `rulec check` names what is missing. The
//! importer never decides an amount, a rounding or a threshold; it lays the values out in the
//! shape the language wants and stops.
//!
//! The names are the importer's to decide, though, so that `rulec check` reads them back without
//! a word (§15.259, §15.266). Whether a word can stand as a name is the lexer's to say. Whether the
//! generated code can take it as an identifier — each language spells names its own way, and some
//! cannot take a name at all — is the check's: the draft is checked as it is written, and a name a
//! W121 or an E034 marks takes its next spelling, until the check says neither. No rule of a
//! target language is written down here.

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

/// How a word stands in the draft as a name, before `check` has been asked about it (§15.259).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Form {
    /// The word is the name and the identifier: `weight`, `Zone`.
    Plain,
    /// The same, for a word of the importer's own rather than the sheet's: the table's `decision`,
    /// a column's enum `Zone_values`.
    Own,
    /// A name that is not ASCII, with the alias of its kind beside it: `重量(c2)`.
    NonAscii,
    /// One of the language's own words (E009), with `_` after it and its spelling as the alias, which
    /// is what the generated code calls it: `count_(count)`.
    Keyword,
    /// The alias of its kind in its place, because the lexer does not read the word as a name:
    /// `Weight (kg)`, `re-quote`, `2FA` or `1200円` would be read as more than one word or as none.
    Unread,
    /// The alias of its kind in its place, because the word is already the name of another of its
    /// kind, which keeps it: a second column headed `zone`.
    Taken,
}

/// A word of the sheet as the draft declares it, and what the draft refers to it by (§15.259,
/// §15.266).
#[derive(Clone, Debug)]
struct Named {
    /// The word as the sheet wrote it, or the importer's own word.
    word: String,
    /// The alias of its kind: the column's (`c2`, `o1`), the value's (`v1`), the enum's (`c2_kind`),
    /// the rule's (`imported`) or the table's (`t`).
    kind: String,
    form: Form,
    /// Which of its spellings it is: the first is its form's, and each one after is the one it takes
    /// when `check` says the generated code cannot take the one before (W121, E034).
    at: usize,
    /// What its declaration writes, and what a table's head and its cells write for it: the places
    /// that *refer* to a declaration rather than make one. The two have to agree, or the draft refers
    /// to a column it never declared.
    name: String,
    /// The alias its declaration writes beside the name: what the generated code calls it.
    alias: Option<String>,
    /// The identifiers it had before, each tried once.
    had: Vec<String>,
}

impl Named {
    /// A word of the sheet, or one of the importer's own (`own`), with the alias of its kind.
    ///
    /// Whether the word can stand as a name is the lexer's to say (`lex::is_bare_word`): a word it
    /// does not read as a name would stop `check` reading the draft back (E057, E058, E063), so the
    /// alias of its kind names it and the word is kept in a `# 推定` note. A name cannot be one of the
    /// language's own words (E009) either: a column headed `count` or `source` would not parse. A
    /// reserved word keeps its spelling as the alias, which is what the generated code calls the
    /// column, and the name gains the `_` that makes it a name.
    fn new(word: &str, kind: &str, own: bool) -> Named {
        let form = if !crate::lex::is_bare_word(word) {
            Form::Unread
        } else if !ascii_ok(word) {
            Form::NonAscii
        } else if crate::kw::RESERVED.contains(&word) {
            Form::Keyword
        } else if own {
            Form::Own
        } else {
            Form::Plain
        };
        let mut n = Named { word: word.to_string(), kind: kind.to_string(), form, at: 0, name: String::new(), alias: None, had: Vec::new() };
        (n.name, n.alias) = n.spelling(0);
        n
    }

    /// Its spelling `at`, as the name and the alias beside it. After the form's own come, for a word
    /// of the sheet, the word with its leading capitals in lower case, as W121 asks of a name that
    /// starts with one (`Zone(zone)`, `URLPath(urlPath)`), and that with `_` after it, the identifier
    /// the generated code writes for a word some language keeps (`type(type_)`); and then the alias of
    /// its kind, numbered from the second on (`c1`, `c1_2`, `c1_3` …), as rulec numbers an identifier
    /// it makes itself (§15.260). Which of them the generated code can take is not known here: `check`
    /// says it of the draft they are written in.
    fn spelling(&self, at: usize) -> (String, Option<String>) {
        let kind = |n: usize| if n <= 1 { self.kind.clone() } else { format!("{}_{n}", self.kind) };
        let w = &self.word;
        let (name, alias) = match self.form {
            Form::Unread | Form::Taken => (kind(at + 1), None),
            Form::NonAscii => (w.clone(), Some(kind(at + 1))),
            Form::Own => (w.clone(), (at > 0).then(|| kind(at))),
            Form::Plain => {
                let low = crate::codegen::lower_lead(w);
                let alias = match at {
                    0 => None,
                    1 => Some(low),
                    2 => Some(format!("{low}_")),
                    n => Some(kind(n - 2)),
                };
                (w.clone(), alias)
            }
            Form::Keyword => {
                let alias = match at {
                    0 => Some(w.clone()),
                    1 => None,
                    n => Some(kind(n - 1)),
                };
                (format!("{w}_"), alias)
            }
        };
        // An alias that is the name is no alias: the name is the identifier then.
        let alias = alias.filter(|a| *a != name);
        (name, alias)
    }

    /// What the generated code calls it: the alias, else the name.
    fn ident(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }

    /// What its declaration writes: `重量(c2)`, `count_(count)`, `Zone(zone)`, `weight`, or a name of
    /// its own.
    fn decl(&self) -> String {
        match &self.alias {
            Some(a) => format!("{}({a})", self.name),
            None => self.name.clone(),
        }
    }

    /// Whether its identifier is one the importer chose rather than the sheet's word as written: the
    /// one that gives way, when `check` says two meet.
    fn made(&self) -> bool {
        self.at > 0 || !matches!(self.form, Form::Plain | Form::Keyword)
    }

    /// Whether it is called otherwise in the generated code than its word, as written, would have it,
    /// because `check` said the generated code cannot take that: what its note says.
    fn respelled(&self) -> bool {
        self.at > 0 && matches!(self.form, Form::Plain | Form::Own | Form::Keyword)
    }

    /// On to its next spelling that is not an identifier it had, nor a name or an alias another of its
    /// kind has. There is always one: the alias of its kind takes a number no other has.
    fn advance(&mut self, others: &[(String, Option<String>)]) {
        self.had.push(self.ident().to_string());
        loop {
            self.at += 1;
            let (name, alias) = self.spelling(self.at);
            let ident = alias.clone().unwrap_or_else(|| name.clone());
            if !self.had.contains(&ident) && !clashes(&name, alias.as_deref(), others) {
                (self.name, self.alias) = (name, alias);
                return;
            }
        }
    }
}

/// Whether a spelling takes a name or an alias another of its kind has, as written. Two names of one
/// identifier stop the check (E034); a name that is another's alias (`a(x)` beside `x(y)`) passes it,
/// but leaves a reader to tell which `x` is meant.
fn clashes(name: &str, alias: Option<&str>, others: &[(String, Option<String>)]) -> bool {
    others.iter().any(|(n, a)| n == name || a.as_deref() == Some(name) || alias.is_some_and(|al| al == n || a.as_deref() == Some(al)))
}

/// The names and the aliases of all but the `i`th.
fn others_of(all: &[Named], i: usize) -> Vec<(String, Option<String>)> {
    all.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, o)| (o.name.clone(), o.alias.clone())).collect()
}

/// The words of one kind — the columns, the values of one column, the enums — named together, each
/// with the alias of its kind. The sheet's words keep their names where they can; what gives way is
/// the importer's own: the second of two columns headed alike (`Taken`), and a name or an alias made
/// of the alias of its kind that a word of the sheet already is — a sheet may well head a column
/// `c2`, or hold a value `v1` — which takes the next number (`c2_2`).
fn named_all(words: &[(String, String)], own: bool) -> Vec<Named> {
    let mut out: Vec<Named> = words.iter().map(|(w, k)| Named::new(w, k, own)).collect();
    // The words' own names, compared as written: a name the importer made gives way below.
    let own_name = |n: &Named| !matches!(n.form, Form::Unread | Form::Taken);
    for i in 0..out.len() {
        if own_name(&out[i]) && out[..i].iter().any(|o| own_name(o) && o.name == out[i].name) {
            out[i].form = Form::Taken;
            (out[i].name, out[i].alias) = out[i].spelling(0);
        }
    }
    for i in 0..out.len() {
        let others = others_of(&out, i);
        if out[i].made() && clashes(&out[i].name, out[i].alias.as_deref(), &others) {
            out[i].advance(&others);
        }
    }
    out
}

/// One name the draft declares, by what it names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Rule,
    Column(usize),
    /// The enum of a column of words.
    Enum(usize),
    /// A value of that enum.
    Value(usize, usize),
    Table,
}

/// What the draft calls everything it declares.
struct Names {
    rule: Named,
    columns: Vec<Named>,
    /// The enum of each column of words, with its values in the order the column holds them.
    enums: Vec<Option<(Named, Vec<Named>)>>,
    table: Named,
}

/// The name of a column's enum: after the header as written, or after the column's name where the
/// header cannot be it.
fn enum_word(column: &Named) -> String {
    let base = if matches!(column.form, Form::Unread | Form::Taken) { column.name.clone() } else { column.word.clone() };
    tr!("{}の値", "{}_values", base)
}

impl Names {
    fn new(s: &Sheet, rule: &str) -> Names {
        let columns = named_all(&(0..s.header.len()).map(|k| (s.header[k].trim().to_string(), s.alias(k))).collect::<Vec<_>>(), false);
        let mut enums: Vec<Option<(Named, Vec<Named>)>> = (0..s.header.len()).map(|_| None).collect();
        let kinds: Vec<usize> = (0..s.header.len()).filter(|k| matches!(s.cols[*k], Column::Enum(_))).collect();
        let types = named_all(&kinds.iter().map(|k| (enum_word(&columns[*k]), format!("{}_kind", s.alias(*k)))).collect::<Vec<_>>(), true);
        for (k, e) in kinds.iter().zip(types) {
            let Column::Enum(vals) = &s.cols[*k] else { unreachable!("a column of words") };
            let members = named_all(&vals.iter().enumerate().map(|(i, v)| (v.clone(), format!("v{}", i + 1))).collect::<Vec<_>>(), false);
            enums[*k] = Some((e, members));
        }
        Names { rule: Named::new(rule, "imported", false), columns, enums, table: Named::new(&tr!("表", "decision"), "t", true) }
    }

    fn get(&self, s: Slot) -> Option<&Named> {
        match s {
            Slot::Rule => Some(&self.rule),
            Slot::Column(k) => self.columns.get(k),
            Slot::Enum(k) => self.enums.get(k)?.as_ref().map(|(e, _)| e),
            Slot::Value(k, i) => self.enums.get(k)?.as_ref()?.1.get(i),
            Slot::Table => Some(&self.table),
        }
    }

    /// The others of its kind, which a new spelling of it is held apart from.
    fn others(&self, s: Slot) -> Vec<(String, Option<String>)> {
        match s {
            Slot::Rule | Slot::Table => Vec::new(),
            Slot::Column(k) => others_of(&self.columns, k),
            Slot::Enum(k) => self.enums.iter().enumerate().filter(|(j, _)| *j != k).filter_map(|(_, e)| e.as_ref()).map(|(e, _)| (e.name.clone(), e.alias.clone())).collect(),
            Slot::Value(k, i) => self.enums[k].as_ref().map(|(_, vs)| others_of(vs, i)).unwrap_or_default(),
        }
    }

    /// The name at `s` on to its next spelling. A column named by the alias of its kind names its enum
    /// too, which follows it.
    fn advance(&mut self, s: Slot) {
        let others = self.others(s);
        match s {
            Slot::Rule => self.rule.advance(&others),
            Slot::Table => self.table.advance(&others),
            Slot::Column(k) => {
                self.columns[k].advance(&others);
                let word = enum_word(&self.columns[k]);
                if let Some((e, _)) = &mut self.enums[k] {
                    if e.word != word {
                        e.word = word;
                        (e.name, e.alias) = e.spelling(e.at);
                    }
                }
            }
            Slot::Enum(k) => {
                if let Some((e, _)) = &mut self.enums[k] {
                    e.advance(&others);
                }
            }
            Slot::Value(k, i) => {
                if let Some((_, vs)) = &mut self.enums[k] {
                    vs[i].advance(&others);
                }
            }
        }
    }

    /// What `check` said of the draft's names, as the names each finding marks, takes effect: the name
    /// of a finding that gives way takes its next spelling. A finding that marks one name is taken
    /// first, since what that name takes may part it from the others too; of two, the one whose
    /// identifier the importer chose gives way, else the later — unless one of the two has taken
    /// another spelling this round already, which may have parted them. False when there was nothing
    /// to give way.
    fn respell(&mut self, found: &[Vec<Slot>]) -> bool {
        let mut chosen: Vec<Slot> = Vec::new();
        for f in found.iter().filter(|f| f.len() == 1).chain(found.iter().filter(|f| f.len() > 1)) {
            if f.iter().any(|s| chosen.contains(s)) {
                continue;
            }
            let pick = f.iter().rev().find(|s| self.get(**s).is_some_and(Named::made)).or(f.last());
            if let Some(s) = pick {
                chosen.push(*s);
            }
        }
        for s in &chosen {
            self.advance(*s);
        }
        !chosen.is_empty()
    }
}

/// A word of the sheet as a comment or a string of the draft writes it: a line break or another
/// control character would end the line there, and what follows would be read as a line of the
/// rule.
fn one_line(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// The sheet, read: what the draft writes whatever it calls the names.
struct Sheet {
    header: Vec<String>,
    data: Vec<Vec<Cell>>,
    cols: Vec<Column>,
    /// How many of the columns are inputs: the rest are outputs.
    n_in: usize,
    source: String,
}

impl Sheet {
    /// The alias of the column's kind: `c1`, `c2` … for an input, `o1` … for an output.
    fn alias(&self, k: usize) -> String {
        if k < self.n_in { format!("c{}", k + 1) } else { format!("o{}", k + 1 - self.n_in) }
    }
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
    // An output is what a row answers: one left empty has no value to copy.
    for (i, r) in data.iter().enumerate() {
        if let Some(k) = (n_in..ncol).find(|k| r[*k] == Cell::Empty) {
            return Err(tr!("{} 行目の出力の列 {} が空です", "row {}: the output column {} is empty", i + 2, header[k].trim()));
        }
    }
    let cols: Vec<Column> = (0..ncol).map(|k| column_kind(&data.iter().map(|r| r[k].clone()).collect::<Vec<_>>())).collect();
    let sheet = Sheet { header: header.clone(), data, cols, n_in, source: source.to_string() };

    // The draft as the words are written, then checked as `rulec check` checks it: a name a W121 or
    // an E034 marks takes its next spelling, and the draft is checked again, until the check says
    // neither (§15.266). What the generated code of each language can take is the check's to say.
    // Each round moves a name on, and the spelling a name ends at is the alias of its kind, numbered,
    // which every language takes; the bound is never reached.
    let mut names = Names::new(&sheet, name);
    let mut text = render(&sheet, &names);
    for _ in 0..64 {
        let found = said(&text, &sheet, &names);
        if found.is_empty() || !names.respell(&found) {
            break;
        }
        text = render(&sheet, &names);
    }
    Ok(text)
}

/// What the draft is checked as: no file is read for it, so its name says only what it is.
const DRAFT: &str = "draft.rule";

/// What `rulec check` says of the draft's names: every W121 and E034, as the names each marks, in the
/// order it marks them (§15.266). A finding marks the name it is about where that name is declared,
/// and the later of two last.
fn said(text: &str, s: &Sheet, names: &Names) -> Vec<Vec<Slot>> {
    let report = crate::report(text, DRAFT);
    let mut found: Vec<Vec<Slot>> = Vec::new();
    let places = places(text, s, names);
    for d in report.diags.iter().filter(|d| matches!(d.code, "W121" | "E034")) {
        let mut slots: Vec<Slot> = Vec::new();
        for m in d.marks.iter().filter(|m| m.span.file.is_none()) {
            if let Some((_, slot)) = places.iter().find(|(at, _)| *at == (m.span.line, m.span.col)) {
                if !slots.contains(slot) {
                    slots.push(*slot);
                }
            }
        }
        if !slots.is_empty() {
            found.push(slots);
        }
    }
    found
}

/// Where the draft declares each name, as (line, column): the rule's, each column's on its line and
/// an output's in the table's head too, each enum's and each of its values', and the table's.
fn places(text: &str, s: &Sheet, names: &Names) -> Vec<((usize, usize), Slot)> {
    let Some(f) = crate::parse::parse(text, DRAFT).file else { return Vec::new() };
    let at = |n: &crate::ast::Name| (n.span.line, n.span.col);
    let mut out = vec![(at(&f.name), Slot::Rule)];
    out.extend(f.inputs.iter().enumerate().map(|(k, i)| (at(&i.name), Slot::Column(k))));
    out.extend(f.outputs.iter().enumerate().map(|(j, o)| (at(&o.name), Slot::Column(s.n_in + j))));
    let kinds: Vec<usize> = (0..names.columns.len()).filter(|k| names.enums[*k].is_some()).collect();
    for (e, k) in f.enums.iter().zip(kinds) {
        out.push((at(&e.name), Slot::Enum(k)));
        out.extend(e.values.iter().enumerate().map(|(i, v)| (at(v), Slot::Value(k, i))));
    }
    for it in &f.items {
        if let crate::ast::Item::Table(t) = it {
            out.extend(t.name.iter().map(|n| (at(n), Slot::Table)));
            out.extend(t.outputs.iter().enumerate().map(|(j, oc)| (at(&oc.name), Slot::Column(s.n_in + j))));
        }
    }
    out
}

/// The note of a name the generated code calls otherwise than its word, as written, would have it
/// (§15.266): `what` names the word the way the note says it (`the header "Zone"`), `whose` what is
/// named (`the column`).
fn respelled_note(n: &Named, what_ja: &str, what_en: &str, whose_en: &str) -> Option<String> {
    if !n.respelled() {
        return None;
    }
    let guess = tr!("推定", "guess");
    let (w, i) = (one_line(&n.word), n.ident());
    Some(tr!(
        "{guess}: {what_ja}「{w}」は、生成先の言語によってはそのままでは使えないか、ほかの名前と同じ識別子になるので、生成コードでは仮に {i} と名付けた",
        "{guess}: the code generated for some languages cannot take {what_en} \"{w}\" as it is, or makes it one identifier with another name, so the generated code names {whose_en} {i} for now"
    ))
}

/// The draft, as the names are now spelled.
fn render(s: &Sheet, n: &Names) -> String {
    let (header, data, cols, n_in) = (&s.header, &s.data, &s.cols, s.n_in);
    let ncol = header.len();
    let columns = &n.columns;
    let guess = tr!("推定", "guess");
    // The note a column's name carries: the header as the sheet wrote it, where the column is named
    // otherwise.
    let header_note = |k: usize| {
        let c = &columns[k];
        let (w, name) = (one_line(&c.word), &c.name);
        match c.form {
            Form::Unread if w.is_empty() => Some(tr!("{guess}: 見出しが空なので、仮に {name} と名付けた", "{guess}: the header is empty, so the column is named {name} for now")),
            Form::Unread => Some(tr!("{guess}: 見出しの「{w}」は名前として読めないので、仮に {name} と名付けた", "{guess}: the header \"{w}\" is not read as a name, so the column is named {name} for now")),
            Form::Taken => Some(tr!("{guess}: 見出しの「{w}」はほかの列の見出しと同じなので、仮に {name} と名付けた", "{guess}: the header \"{w}\" is another column's too, so the column is named {name} for now")),
            _ => respelled_note(c, "見出しの", "the header", "the column"),
        }
    };

    let mut o = String::new();
    o.push_str(&format!("rule {} v1", n.rule.decl()));
    let rule_note = match n.rule.form {
        Form::Unread => {
            let (w, name) = (one_line(&n.rule.word), &n.rule.name);
            Some(tr!("{guess}: 規則の名前「{w}」は名前として読めないので、仮に {name} と名付けた", "{guess}: the rule's name \"{w}\" is not read as a name, so the rule is named {name} for now"))
        }
        _ => respelled_note(&n.rule, "規則の名前", "the rule's name", "the rule"),
    };
    if let Some(note) = rule_note {
        o.push_str(&format!("  # {note}"));
    }
    o.push('\n');
    // The file's name inside a string: a `"` would end the string there.
    let source_said = one_line(&s.source).replace('"', "'");
    o.push_str(&tr!(
        "description \"{source_said} から rulec import が起こした下書き。「{guess}」と書いた行は全部、人が確かめること\"\n",
        "description \"A draft that rulec import made from {source_said}. Every line marked {guess} is for a person to confirm\"\n"
    ));

    // Enums, one per column of words: each value by its name, or by `v1`, `v2` … where the lexer
    // does not read it as a name.
    let mut enums = String::new();
    for k in 0..ncol {
        let Some((e, members)) = &n.enums[k] else { continue };
        let mut notes = vec![tr!("{guess}: この列に現れた値をそのまま列挙にした。値が足りなければ足し、別名は付け直すこと", "{guess}: the values seen in this column, as an enum; add what is missing, and rename the aliases")];
        notes.extend(respelled_note(e, "列挙の名前", "the enum's name", "the enum"));
        let unread: Vec<(String, String)> = members.iter().filter(|m| m.form == Form::Unread).map(|m| (m.name.clone(), one_line(&m.word))).collect();
        if !unread.is_empty() {
            let ja: Vec<String> = unread.iter().map(|(n, w)| format!("{n} は「{w}」")).collect();
            let en: Vec<String> = unread.iter().map(|(n, w)| format!("{n} is \"{w}\"")).collect();
            let (ja, en) = (ja.join("、"), en.join(", "));
            notes.push(tr!("{guess}: 名前として読めない値には仮の名前を付けた（{ja}）", "{guess}: a value not read as a name has a name for now ({en})"));
        }
        let respelled: Vec<(String, String)> = members.iter().filter(|m| m.respelled()).map(|m| (one_line(&m.word), m.ident().to_string())).collect();
        if !respelled.is_empty() {
            let ja: Vec<String> = respelled.iter().map(|(w, i)| format!("{w} は {i}")).collect();
            let en: Vec<String> = respelled.iter().map(|(w, i)| format!("{w} is {i}")).collect();
            let (ja, en) = (ja.join("、"), en.join(", "));
            notes.push(tr!(
                "{guess}: 生成先の言語によってはそのままでは使えないか、ほかの値と同じ識別子になる値には、生成コードでの仮の名前を付けた（{ja}）",
                "{guess}: a value the code generated for some languages cannot take as it is, or makes one identifier with another value, has another name in the generated code for now ({en})"
            ));
        }
        let decls: Vec<String> = members.iter().map(Named::decl).collect();
        enums.push_str(&format!("enum {} = {}  # {}\n", e.decl(), decls.join(" | "), join_notes(&notes)));
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
            Column::Enum(_) => (n.enums[k].as_ref().map(|(e, _)| e.name.clone()).unwrap_or_default(), String::new()),
        }
    };
    o.push_str("\ninputs\n");
    for k in 0..n_in {
        let (ty, extra) = decl(k);
        let mut line = format!("  {} : {ty}", columns[k].decl());
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
        let mut line = format!("  {} : {ty}", columns[k].decl());
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
    let source_line = one_line(&s.source);
    let mut source_note = tr!("出典: {source_line}（{guess}: 出典の文書名と日付に書き換えること）", "source: {source_line} ({guess}: replace with the document's name and date)");
    if let Some(note) = respelled_note(&n.table, "表の名前", "the table's name", "the table") {
        source_note.push_str(&tr!("。{note}", "; {note}"));
    }
    o.push_str(&format!("\ntable {}  # {source_note}\n", n.table.decl()));
    o.push_str("policy unique\n");
    let mut head: Vec<String> = (0..n_in).map(|k| columns[k].name.clone()).collect();
    for k in n_in..ncol {
        let (ty, _) = decl(k);
        head.push(format!("{}{} : {ty}", if k == n_in { "-> " } else { "" }, columns[k].decl()));
    }
    o.push_str(&format!("| {} |\n", head.join(" | ")));
    for r in data {
        let mut cells: Vec<String> = Vec::new();
        for (k, c) in r.iter().enumerate() {
            let text = match (c, &cols[k]) {
                // An empty output was refused before the draft was written.
                (Cell::Empty, _) => "-".to_string(),
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
                        Cell::Empty => unreachable!("an empty cell is `-`"),
                    };
                    match (&cols[k], &n.enums[k]) {
                        (Column::Enum(vals), Some((_, members))) => vals.iter().position(|v| *v == seen).map(|p| members[p].name.clone()).unwrap_or(seen),
                        _ => seen,
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
    crate::fmt::format(&o)
}
