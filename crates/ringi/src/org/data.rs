//! The files the data of an organization and of masters is kept in (DESIGN 4.6, 8.2): a CSV file
//! with a line of column names, or, for a version of an organization, one JSON file of the same
//! shape (`organization.json`, an object whose `teams`, `people` and `members` are arrays of
//! objects with the CSV's columns as keys). Both are read through `ritsu_base::fs`, so that the
//! playground's files in memory read as the disk's do.
//!
//! A row keeps the line it starts on, so that a mistake in the data is said at its line, in the
//! file as a person opens it: the path is the one the file names (`data "../org"`) joined to the
//! directory of the file that names it, written the way the run writes its paths ([`fold`],
//! ritsu's DESIGN 4.15): from the directory the run is in, absolute only in a run whose first path
//! was.

use ritsu_base::json::Json;
use ritsu_base::text::Text;
use std::path::{Path, PathBuf};

/// One row of a table: the line it starts on (from 1), and its cells in the order of the columns.
#[derive(Clone, Debug)]
pub struct Row {
    pub line: usize,
    pub cells: Vec<String>,
}

/// A table of data: a CSV file, or one array of a JSON file.
#[derive(Clone, Debug)]
pub struct Table {
    /// The file, as a diagnostic names it.
    pub file: String,
    /// Its text, for the line a diagnostic shows.
    pub src: String,
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}

impl Table {
    pub fn col(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }

    /// The cell of the column `name` in `row`; empty when the table has no such column.
    pub fn get<'r>(&self, row: &'r Row, name: &str) -> &'r str {
        self.col(name).and_then(|i| row.cells.get(i)).map(|s| s.as_str()).unwrap_or("")
    }
}

/// What is wrong with a file of data as a whole: the line, and what to say.
#[derive(Clone, Debug)]
pub struct Unread {
    pub line: usize,
    pub message: Text,
}

/// `p`, a file or a directory of data, as the run writes its paths (ritsu's DESIGN 4.15): `.` and
/// `..` folded as the system folds them when it opens the path (`examples/shared/../org` is
/// `examples/org` where `shared` is no symbolic link), from the directory the run is in, and
/// absolute when the run's first path was: what a diagnostic names a file of data and its rows by,
/// and the path it is read at.
pub fn fold(p: &Path) -> PathBuf {
    PathBuf::from(ritsu_base::paths::show(p))
}

/// The path written in the file at `file` (`data "../org"`), from the directory that file is in,
/// folded: what the data is read at and named by.
pub fn beside(file: &str, written: &str) -> PathBuf {
    let dir = Path::new(file).parent().unwrap_or(Path::new(""));
    if ritsu_base::paths::is_absolute(written) {
        return fold(Path::new(written));
    }
    fold(&dir.join(written))
}

/// The CSV file at `path`, read: Err for a file that cannot be read, or whose text is no CSV (a
/// quote that is not closed).
pub fn read_csv(path: &Path) -> Result<Table, Unread> {
    ritsu_base::tally::count("ringi.data", path);
    let file = path.display().to_string();
    let src = ritsu_base::fs::read_to_string(path).map_err(|e| {
        let e = e.to_string();
        Unread { line: 0, message: tr!("`{file}` を読めません: {e}", "Cannot read `{file}`: {e}") }
    })?;
    csv(&file, &src)
}

/// The most bytes a row of a CSV file may be (the server's `maxRowBytes`): a row longer than it
/// does not read, as the server's reader stops at it.
const ROW_BYTES: usize = 1 << 20;

/// The most values a row of a CSV file may have (the server's `maxRowValues`).
const ROW_VALUES: usize = 1000;

/// The text of a CSV file, read: the first line that is not blank names the columns, and each line
/// after it that is not blank is a row. A field may be written in double quotes (a `"` in it
/// written twice), and may then hold commas and line breaks; a field written without quotes has
/// the spaces around it taken off. A quote inside a field not written in quotes, a character
/// after a field's closing quote, a row longer than `ROW_BYTES` or of more than `ROW_VALUES`
/// values, the file does not read (as the server's CSV reader refuses them).
pub fn csv(file: &str, src: &str) -> Result<Table, Unread> {
    let text = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut records: Vec<(usize, Vec<String>)> = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut line = 1usize;
    let mut start = 1usize;
    let mut any = false;
    let mut bytes = 0usize;
    let mut commas = 0usize;
    let mut chars = text.chars().peekable();
    let end_field = |fields: &mut Vec<String>, field: &mut String, quoted: &mut bool| {
        let f = std::mem::take(field);
        fields.push(if *quoted { f } else { f.trim().to_string() });
        *quoted = false;
    };
    // a row too long, or of too many values, the file does not read, said at the line it starts on
    let too_large = |start: usize, values: bool| -> Unread {
        if values {
            Unread { line: start, message: tr!("この行の値は{ROW_VALUES}個を超えます。一つの行の値は{ROW_VALUES}個までです", "The row has more than {ROW_VALUES} values, the most a row may have") }
        } else {
            Unread { line: start, message: tr!("この行は{ROW_BYTES}バイトを超えます。一つの行は{ROW_BYTES}バイトまでです", "The row is longer than {ROW_BYTES} bytes, the most a row may be") }
        }
    };
    while let Some(c) = chars.next() {
        bytes += c.len_utf8();
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    bytes += 1;
                    field.push('"');
                }
                '"' => in_quotes = false,
                '\n' => {
                    line += 1;
                    field.push('\n');
                }
                '\r' => {}
                c => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.trim().is_empty() && !quoted => {
                field.clear();
                in_quotes = true;
                quoted = true;
                any = true;
            }
            '"' if !quoted => {
                return Err(Unread { line: start, message: tr!("引用符で囲んでいない値の中に `\"` があります", "a quote `\"` is inside a cell not written in quotes") });
            }
            '"' => {
                return Err(Unread { line: start, message: tr!("引用符 `\"` が閉じていないか、閉じたあとに文字が続いています", "a quote `\"` is not closed, or more follows it in its cell") });
            }
            ',' => {
                end_field(&mut fields, &mut field, &mut quoted);
                commas += 1;
                any = true;
            }
            '\r' => {}
            '\n' => {
                bytes -= 1;
                if any || !field.trim().is_empty() {
                    if bytes > ROW_BYTES || commas >= ROW_VALUES {
                        return Err(too_large(start, commas >= ROW_VALUES));
                    }
                    end_field(&mut fields, &mut field, &mut quoted);
                    records.push((start, std::mem::take(&mut fields)));
                }
                field.clear();
                any = false;
                bytes = 0;
                commas = 0;
                line += 1;
                start = line;
            }
            c if quoted => {
                if !c.is_whitespace() {
                    return Err(Unread { line: start, message: tr!("引用符 `\"` が閉じていないか、閉じたあとに文字が続いています", "a quote `\"` is not closed, or more follows it in its cell") });
                }
            }
            c => {
                if !c.is_whitespace() {
                    any = true;
                }
                field.push(c);
            }
        }
    }
    if in_quotes {
        return Err(Unread { line: start, message: tr!("引用符 `\"` が閉じていません", "A quote `\"` is not closed") });
    }
    if any || !field.trim().is_empty() {
        if bytes > ROW_BYTES || commas >= ROW_VALUES {
            return Err(too_large(start, commas >= ROW_VALUES));
        }
        end_field(&mut fields, &mut field, &mut quoted);
        records.push((start, fields));
    }
    let mut it = records.into_iter();
    let Some((_, columns)) = it.next() else {
        return Ok(Table { file: file.to_string(), src: src.to_string(), columns: Vec::new(), rows: Vec::new() });
    };
    let rows = it.map(|(line, cells)| Row { line, cells }).collect();
    Ok(Table { file: file.to_string(), src: src.to_string(), columns, rows })
}

/// The arrays of a JSON file of data, by their keys, each as a table whose columns are the ones
/// `keys` gives for it, then any other key its objects use, in the order first written. A key an
/// object leaves out is an empty value, as `null` is. A value is a string or `null`; of a key
/// `bools` names, `true` and `false` too, read as `yes` and `no`. Any other value (a number, an
/// array, an object, a truth value elsewhere) does not read, as the schema of the data says
/// (DESIGN 8.2): a server that holds the data to the schema refuses it, and the check says so first.
pub fn read_json(path: &Path, keys: &[(&str, &[&str])], bools: &[&str]) -> Result<Vec<Table>, Vec<Unread>> {
    ritsu_base::tally::count("ringi.data", path);
    let file = path.display().to_string();
    let src = ritsu_base::fs::read_to_string(path).map_err(|e| {
        let e = e.to_string();
        vec![Unread { line: 0, message: tr!("`{file}` を読めません: {e}", "Cannot read `{file}`: {e}") }]
    })?;
    // a byte order mark at the start is not read, as in a CSV file
    let text = src.strip_prefix('\u{feff}').unwrap_or(&src);
    let json = ritsu_base::json::parse(text).map_err(|e| {
        let line = text[..e.at.min(text.len())].bytes().filter(|&b| b == b'\n').count() + 1;
        vec![Unread { line, message: Text::new(format!("`{file}` は JSON として読めません（{}）", e.message.ja), format!("`{file}` does not read as JSON ({})", e.message.en)) }]
    })?;
    let Some(obj) = json.as_obj() else {
        return Err(vec![Unread { line: 1, message: tr!("`{file}` の中身は、JSON のオブジェクトにしてください", "The JSON of `{file}` is to be an object") }]);
    };
    let (lines, key_lines) = element_lines(text);
    // a key of the top object the file is not read by, as the server refuses it (one written
    // otherwise in only case, "People", would leave out what it holds), or one it writes twice —
    // the reader of the JSON refuses the second first: the keys are `keys`, in this case
    let known: Vec<&str> = keys.iter().map(|(k, _)| *k).collect();
    let mut errs: Vec<Unread> = Vec::new();
    for (k, _) in obj {
        if !known.contains(&k.as_str()) {
            let all = known.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ");
            let line = key_lines.iter().find(|(x, _)| x == k).map(|(_, l)| *l).unwrap_or(1);
            errs.push(Unread { line, message: tr!("`{k}` は、このファイルのキーではありません（読むキーは {all}）", "`{k}` is not a key of this file (the keys read are {all})") });
        }
    }
    let mut out = Vec::new();
    for (key, columns) in keys {
        let Some(arr) = json.get(key) else {
            errs.push(Unread { line: 1, message: tr!("`{file}` に `{key}` の配列がありません", "`{file}` has no array `{key}`") });
            continue;
        };
        let Some(arr) = arr.as_arr() else {
            errs.push(Unread { line: 1, message: tr!("`{file}` の `{key}` が配列ではありません", "`{key}` in `{file}` is not an array") });
            continue;
        };
        let cols: Vec<String> = columns.iter().map(|c| c.to_string()).collect();
        let starts = lines.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default();
        // a key of an object the table does not read, as the server refuses it too (a key an
        // object writes twice, the reader of the JSON refuses first), said at the object's line
        for (i, el) in arr.iter().enumerate() {
            let line = starts.get(i).copied().unwrap_or(1);
            let Some(el_obj) = el.as_obj() else { continue };
            for (k, _) in el_obj {
                if !columns.contains(&k.as_str()) {
                    let all = columns.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ");
                    errs.push(Unread { line, message: tr!("`{k}` は `{key}` のキーではありません（読むキーは {all}）", "`{k}` is not a key of `{key}` (the keys read are {all})") });
                }
            }
        }
        let mut rows = Vec::new();
        let mut bad = false;
        for (i, el) in arr.iter().enumerate() {
            let line = starts.get(i).copied().unwrap_or(1);
            let Some(obj) = el.as_obj() else {
                let n = i + 1;
                errs.push(Unread { line, message: tr!("`{key}` の {n} 番目がオブジェクトではありません", "The element {n} of `{key}` is not an object") });
                break;
            };
            let mut cells = Vec::with_capacity(cols.len());
            for c in &cols {
                cells.push(match obj.iter().find(|(k, _)| k == c).map(|(_, v)| v) {
                    None | Some(Json::Null) => String::new(),
                    Some(Json::Str(s)) => s.clone(),
                    Some(Json::Bool(b)) if bools.contains(&c.as_str()) => if *b { "yes" } else { "no" }.to_string(),
                    Some(other) => {
                        let (n, v) = (i + 1, other.compact());
                        let message = if bools.contains(&c.as_str()) {
                            tr!("`{key}` の {n} 番目の `{c}` は、真偽値か文字列か null にしてください。`{v}` ではありません", "The `{c}` of the element {n} of `{key}` is to be a truth value, a string or null, not `{v}`")
                        } else {
                            tr!("`{key}` の {n} 番目の `{c}` は、文字列か null にしてください。`{v}` ではありません", "The `{c}` of the element {n} of `{key}` is to be a string or null, not `{v}`")
                        };
                        errs.push(Unread { line, message });
                        bad = true;
                        break;
                    }
                });
            }
            if bad {
                break;
            }
            rows.push(Row { line, cells });
        }
        // a key an object leaves out is an empty value; a key no column names is said by the
        // reader of the table, as a CSV's unknown column is
        out.push(Table { file: file.clone(), src: src.clone(), columns: cols, rows });
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    Ok(out)
}

/// The line each object of each array of the top object starts on, by the array's key.
type ArrayLines = Vec<(String, Vec<usize>)>;
/// The line each key of the top object ends on.
type KeyLines = Vec<(String, usize)>;

/// The line each object of each array of the top object starts on, by the array's key, and the
/// line each key of the top object ends on: what a diagnostic of a row of a JSON file, and of a
/// key it does not know, names.
fn element_lines(src: &str) -> (ArrayLines, KeyLines) {
    let mut out: Vec<(String, Vec<usize>)> = Vec::new();
    let mut keys: Vec<(String, usize)> = Vec::new();
    let mut depth = 0usize;
    let mut line = 1usize;
    let mut in_str = false;
    let mut esc = false;
    let mut cur = String::new();
    let mut last_str = String::new();
    let mut last_str_line = 1usize;
    let mut key: Option<String> = None;
    for c in src.chars() {
        if c == '\n' {
            line += 1;
        }
        if in_str {
            if esc {
                esc = false;
                cur.push(c);
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
                last_str = std::mem::take(&mut cur);
                last_str_line = line;
            } else {
                cur.push(c);
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            ':' if depth == 1 => {
                key = Some(last_str.clone());
                keys.push((last_str.clone(), last_str_line));
            }
            '{' | '[' => {
                if c == '{' && depth == 2
                    && let Some(k) = &key
                {
                    match out.iter_mut().find(|(x, _)| x == k) {
                        Some((_, v)) => v.push(line),
                        None => out.push((k.clone(), vec![line])),
                    }
                }
                depth += 1;
            }
            '}' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    (out, keys)
}

/// What is wrong with the columns of a table: a column it needs and does not have, or one it does
/// not know (`primay`). `known` are the columns it may have besides those it needs.
pub fn columns(t: &Table, needed: &[&str], known: &[&str]) -> Vec<Unread> {
    let mut out = Vec::new();
    let missing: Vec<&str> = needed.iter().copied().filter(|c| t.col(c).is_none()).collect();
    if !missing.is_empty() {
        let m = missing.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ");
        let all = needed.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ");
        out.push(Unread { line: 1, message: tr!("列 {m} がありません（要る列は {all}）", "The column {m} is missing (the columns needed are {all})") });
    }
    let mut seen: Vec<&str> = Vec::new();
    for c in &t.columns {
        if seen.contains(&c.as_str()) {
            out.push(Unread { line: 1, message: tr!("列 `{c}` が二度あります", "The column `{c}` is there twice") });
        }
        seen.push(c);
        if !needed.contains(&c.as_str()) && !known.contains(&c.as_str()) {
            let all = needed.iter().chain(known).map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ");
            out.push(Unread { line: 1, message: tr!("列 `{c}` は、このファイルの列ではありません（読む列は {all}）", "`{c}` is not a column of this file (the columns read are {all})") });
        }
    }
    for r in &t.rows {
        if r.cells.len() != t.columns.len() {
            let (n, m) = (r.cells.len(), t.columns.len());
            out.push(Unread { line: r.line, message: tr!("この行の値は {n} 個で、列は {m} 個です", "The row has {n} values, and there are {m} columns") });
        }
    }
    out
}
