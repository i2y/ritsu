//! The copies of the sources (DESIGN 1.4): where a law's articles are kept beside the `.req`,
//! the file each article is copied into, and the text of a copy.
//!
//! The places and the names are rulec's and koyomi's, so that the three tools keep the same
//! copy of the same article at the same path: `sources/law/<id>@<asof>/<element>.xml`, the
//! element as e-Gov addresses it (`第143条第2項` → `MainProvision-Article_143-Paragraph_2`,
//! `別表第一` → `AppdxTable_1`, the supplementary provisions as rulec's §15.71 writes them),
//! and for the eCFR `sources/law/29-CFR-1910@<asof>/1910.157.xml`. That is ritsu-base's now
//! (`ritsu_base::sources`), written once for the three; what the forms are, said in a
//! diagnostic, is yuen's.

use crate::ast::LawDb;
use ritsu_base::text::Text;

pub use ritsu_base::sources::{article_lines, copy_dir, copy_path, fragment_file, quote_lines, readable, revision, root_element, xml_text};

/// What the forms of an article are, for a diagnostic.
pub fn fragment_shapes(db: LawDb) -> Text {
    match db {
        LawDb::Egov => tr!(
            "書けるのは `第20条`、`第20条の2`、`第20条第2項`、`第20条第2項第3号`、`別表第一`、`附則第3条`、`附則（令和七年三月三一日法律第一三号）第3条` の形です（漢数字でもよい。丸括弧を含むものは `\"…\"` で囲む）。",
            "The forms are `第20条`, `第20条の2`, `第20条第2項`, `第20条第2項第3号`, `別表第一`, `附則第3条` and `附則（令和七年三月三一日法律第一三号）第3条` (in kanji numerals too; quote one with parentheses)."
        ),
        LawDb::Ecfr => tr!(
            "書けるのは `\"§1910.157\"` か `\"1910.157\"` の形です（section の単位）。",
            "The forms are `\"§1910.157\"` and `\"1910.157\"` (a section)."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_articles_are_copied_into() {
        let e = |s: &str| fragment_file(LawDb::Egov, s);
        assert_eq!(e("第140条").as_deref(), Some("MainProvision-Article_140.xml"));
        assert_eq!(e("第百四十三条第二項").as_deref(), Some("MainProvision-Article_143-Paragraph_2.xml"));
        assert_eq!(e("第20条の2第3項第4号").as_deref(), Some("MainProvision-Article_20_2-Paragraph_3-Item_4.xml"));
        assert_eq!(e("別表第一").as_deref(), Some("AppdxTable_1.xml"));
        assert_eq!(e("附則第3条").as_deref(), Some("SupplProvision-Article_3.xml"));
        assert_eq!(e("附則").as_deref(), Some("SupplProvision.xml"));
        assert_eq!(e("附則（令和七年三月三一日法律第一三号）第3条第2項").as_deref(), Some("SupplProvision_令和七年三月三一日法律第一三号-Article_3-Paragraph_2.xml"));
        assert_eq!(e("140条"), None);
        assert_eq!(fragment_file(LawDb::Ecfr, "§1910.157").as_deref(), Some("1910.157.xml"));
        assert_eq!(fragment_file(LawDb::Ecfr, "1910"), None);
        assert_eq!(copy_dir("29 CFR 1910", "2026-01-01"), "sources/law/29-CFR-1910@2026-01-01");
        assert_eq!(root_element(LawDb::Egov, "MainProvision-Article_143-Paragraph_2.xml"), "Paragraph");
        assert_eq!(root_element(LawDb::Egov, "AppdxTable_1.xml"), "AppdxTable");
        assert_eq!(root_element(LawDb::Egov, "SupplProvision.xml"), "SupplProvision");
        assert_eq!(root_element(LawDb::Egov, "MainProvision-Article_140.xml"), "Article");
    }
}
