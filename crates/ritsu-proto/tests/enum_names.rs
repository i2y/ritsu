//! How the values of a `.proto` enum read: one reading, which rulec's `import proto`, sekisho's
//! `use proto` and the check across the languages hold a contract to (rulec's DESIGN §15.59 has
//! the table this follows, a row a test).

use ritsu_proto::validate::{names, How};

/// Each value read (its names, the first one first), and the name of the value read as not set.
fn read(enum_name: &str, values: &[(&str, i64)]) -> (Vec<Vec<String>>, Option<String>) {
    let n = names(enum_name, values.iter().map(|(v, k)| (*v, *k)));
    (n.values.iter().map(|v| v.names.iter().map(|x| x.read.clone()).collect()).collect(), n.unset.map(|u| u.written().to_string()))
}

fn one(names: &[&str]) -> Vec<Vec<String>> {
    names.iter().map(|n| vec![n.to_string()]).collect()
}

#[test]
fn the_prefix_of_bufs_form_is_dropped_and_the_rest_read_in_lower_case() {
    let (values, unset) = read("Category", &[("CATEGORY_UNSPECIFIED", 0), ("CATEGORY_TRAVEL", 1), ("CATEGORY_AIR_TRAVEL", 2)]);
    assert_eq!(values, one(&["travel", "air_travel"]));
    assert_eq!(unset.as_deref(), Some("CATEGORY_UNSPECIFIED"));
    let (values, _) = read("HTTPMethod", &[("HTTP_METHOD_UNSPECIFIED", 0), ("HTTP_METHOD_GET", 1)]);
    assert_eq!(values, one(&["get"]));
}

#[test]
fn a_prefix_in_other_cases_or_with_its_underscores_elsewhere_is_dropped_too() {
    let (values, unset) = read("Category", &[("Category_Unspecified", 0), ("Category_Food", 1), ("category_drink", 2)]);
    assert_eq!(values, one(&["food", "drink"]));
    assert_eq!(unset.as_deref(), Some("Category_Unspecified"));
    let (values, unset) = read("MemberTier", &[("MEMBERTIER_UNSPECIFIED", 0), ("MEMBERTIER_GOLD", 1), ("Member_Tier_Basic", 2)]);
    assert_eq!(values, one(&["gold", "basic"]));
    assert_eq!(unset.as_deref(), Some("MEMBERTIER_UNSPECIFIED"));
}

#[test]
fn a_prefix_ending_where_a_capital_begins_a_word_is_dropped() {
    let (values, _) = read("MemberTier", &[("MemberTierGold", 1), ("MEMBERTIERBasic", 2)]);
    assert_eq!(values, one(&["gold", "basic"]));
    // `Bar` begins a word after `Status`, as `Gold` does after `MemberTier`
    let (values, _) = read("Status", &[("StatusBarHidden", 1)]);
    assert_eq!(values, one(&["barhidden"]));
}

#[test]
fn an_enums_name_that_runs_on_into_a_word_is_no_prefix() {
    let (values, unset) = read(
        "Status",
        &[("STATUS_UNSPECIFIED", 0), ("STATUSBAR_HIDDEN", 1), ("statusbar_open", 2), ("Statusbar_Closed", 3), ("STATUS_SHOWN", 4)],
    );
    assert_eq!(values, one(&["statusbar_hidden", "statusbar_open", "statusbar_closed", "shown"]));
    assert_eq!(unset.as_deref(), Some("STATUS_UNSPECIFIED"));
    let n = names("Status", [("STATUSBAR_HIDDEN", 1)]);
    assert_eq!(n.values[0].names[0].how, How::RunsOn);
}

#[test]
fn a_name_with_no_prefix_is_read_whole() {
    let (values, unset) = read("Category", &[("TRAVEL", 1), ("Purchase", 2), ("air_travel", 3)]);
    assert_eq!(values, one(&["travel", "purchase", "air_travel"]));
    assert_eq!(unset, None);
}

#[test]
fn zero_is_not_set_only_when_it_is_read_unspecified() {
    let (values, unset) = read("Category", &[("UNSPECIFIED", 0), ("TRAVEL", 1)]);
    assert_eq!((values, unset.as_deref()), (one(&["travel"]), Some("UNSPECIFIED")));
    // a 0 with another name is a value: proto3 sends it for a field left out
    let (values, unset) = read("Category", &[("UNKNOWN", 0), ("TRAVEL", 1)]);
    assert_eq!((values, unset), (one(&["unknown", "travel"]), None));
    let (values, unset) = read("Tier", &[("TIER_NONE", 0), ("TIER_ONE", 1)]);
    assert_eq!((values, unset), (one(&["none", "one"]), None));
    let (values, unset) = read("Status", &[("STATUS_ACTIVE", 0), ("STATUS_CLOSED", 1)]);
    assert_eq!((values, unset), (one(&["active", "closed"]), None));
}

#[test]
fn a_rest_that_begins_with_a_digit_keeps_the_prefix() {
    let (values, _) = read("Size", &[("SIZE_UNSPECIFIED", 0), ("SIZE_60", 1), ("SIZE_LARGE", 2)]);
    assert_eq!(values, one(&["size_60", "large"]));
    assert_eq!(names("Size", [("SIZE_60", 1)]).values[0].names[0].how, How::Digit);
}

#[test]
fn two_numbers_are_never_read_as_one_name() {
    // without its prefix, `COLOR_RED` would be `RED`'s name
    let (values, _) = read("Color", &[("COLOR_UNSPECIFIED", 0), ("COLOR_RED", 1), ("RED", 2)]);
    assert_eq!(values, one(&["color_red", "red"]));
    // and so in lower case
    let n = names("Color", [("COLOR_RED", 1), ("red", 2)]);
    assert_eq!(n.values.iter().map(|v| v.read()).collect::<Vec<_>>(), ["color_red", "red"]);
    assert_eq!(n.values[0].names[0].how, How::Kept("red".into()));
    // two names that differ only in case are read as written
    let n = names("Color", [("COLOR_RED", 1), ("Color_Red", 2)]);
    assert_eq!(n.values.iter().map(|v| v.read()).collect::<Vec<_>>(), ["COLOR_RED", "Color_Red"]);
    assert_eq!(n.values[1].names[0].how, How::AsWritten);
}

#[test]
fn the_names_of_one_number_are_one_value() {
    let (values, unset) = read(
        "Category",
        &[("CATEGORY_UNSPECIFIED", 0), ("CATEGORY_TRAVEL", 1), ("CATEGORY_TRIP", 1), ("CATEGORY_PURCHASE", 2)],
    );
    assert_eq!(values, vec![vec!["travel".to_string(), "trip".to_string()], vec!["purchase".to_string()]]);
    assert_eq!(unset.as_deref(), Some("CATEGORY_UNSPECIFIED"));
    let n = names("Category", [("CATEGORY_TRAVEL", 1), ("CATEGORY_TRIP", 1)]);
    assert_eq!((n.values[0].read(), n.values[0].written(), n.values[0].is("trip")), ("travel", "CATEGORY_TRAVEL", true));
    // an alias that adds or drops the prefix reads the same: one name
    let (values, _) = read("Category", &[("CATEGORY_TRAVEL", 1), ("TRAVEL", 1)]);
    assert_eq!(values, one(&["travel"]));
}

#[test]
fn a_nested_enum_drops_its_own_name() {
    let (values, unset) = read("PayRequest.Category", &[("CATEGORY_UNSPECIFIED", 0), ("CATEGORY_TRAVEL", 1)]);
    assert_eq!((values, unset.as_deref()), (one(&["travel"]), Some("CATEGORY_UNSPECIFIED")));
}

#[test]
fn the_rules_on_a_field_let_through_whole_values() {
    let src = "syntax = \"proto3\";\nimport \"buf/validate/validate.proto\";\nenum Category {\n  option allow_alias = true;\n  CATEGORY_UNSPECIFIED = 0;\n  CATEGORY_TRAVEL = 1;\n  CATEGORY_TRIP = 1;\n  CATEGORY_PURCHASE = 2;\n}\nmessage Pay {\n  Category kind = 1 [(buf.validate.field).enum = {in: [1]}];\n}\n";
    let f = ritsu_proto::read("pay.proto", src).expect("the file reads");
    let field = &f.messages[0].fields[0];
    let got = ritsu_proto::validate::enum_names(&f.enums[0], &field.rules.enum_values);
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].number, got[0].read(), got[0].is("trip")), (1, "travel", true));
}
