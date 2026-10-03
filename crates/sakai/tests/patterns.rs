//! The patterns, held to each other (PLAN B.6).

mod common;

use common::{check_dir, codes, variant};

#[test]
fn a_shared_kernel_both_sides_write_alike_passes_and_one_side_does_not() {
    assert_eq!(codes(&check_dir(variant("基本", &[]).path())), Vec::<&str>::new());
    let dir = variant("基本", &[("ctx/在庫.ctx", "\nshared kernel with 受注\n  proto \"../proto/shop/common/v1/money.proto\"\n", "\n")]);
    let os = check_dir(dir.path());
    assert!(codes(&os).contains(&"E307"));
}

#[test]
fn copies_kept_on_both_sides_pass_and_a_byte_apart_do_not() {
    assert_eq!(codes(&check_dir(variant("パターン", &[]).path())), Vec::<&str>::new());
    let dir = variant("パターン", &[("proto/contracts/kernel/units.proto", "日数の単位", "日数の単位。")]);
    assert_eq!(codes(&check_dir(dir.path())), ["E308"]);
}

#[test]
fn customer_and_supplier_is_told_from_the_side_that_wrote_it() {
    let dir = variant("パターン", &[("ctx/見積.ctx", "upstream 料金 customer\n", "upstream 料金 conformist\n")]);
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["E303"]);
    let d = &os[0].diags[0];
    assert_eq!(d.file, "ctx/料金.ctx");
    assert!(d.notes.iter().any(|n| n.en.contains("`upstream 料金 conformist`")), "{:?}", d.notes);
}

#[test]
fn the_upstream_direction_going_round_is_a_warning() {
    let dir = common::mutant("W301_互いに上流");
    let os = check_dir(dir.path());
    assert_eq!(codes(&os), ["W301"]);
    assert!(os[0].summary.is_some(), "a warning lets the map pass");
}
