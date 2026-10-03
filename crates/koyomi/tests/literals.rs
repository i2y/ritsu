//! The string literals of the five targets are ritsu's (`ritsu_emit::lit::json`, PLAN C.10):
//! the same bytes serde_json wrote when koyomi wrote them with it, so a generated file has not
//! changed for the move.

#[test]
fn a_literal_is_what_serde_json_writes() {
    let mut s: String = (0u32..0x80).filter_map(char::from_u32).collect();
    s.push_str("本「」\u{2028}\u{feff}");
    assert_eq!(ritsu_emit::lit::json(&s), serde_json::to_string(&s).unwrap());
}
