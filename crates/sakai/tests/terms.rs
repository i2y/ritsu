//! The glossaries and the same word (PLAN B.9).

mod common;

use common::{check_dir, codes, variant};

#[test]
fn the_cancel_of_ordering_is_mapped_by_billing_and_passes() {
    assert_eq!(codes(&check_dir(variant("基本", &[]).path())), Vec::<&str>::new());
}

#[test]
fn without_the_mapping_the_same_word_crosses_unmapped() {
    let dir = variant(
        "基本",
        &[("ctx/請求.ctx", "  enum OrderStatus -> enum BillingStatus\n    ORDER_STATUS_RECEIVED  -> BILLING_STATUS_WAIT\n    ORDER_STATUS_PAID      -> BILLING_STATUS_BILL\n    ORDER_STATUS_SHIPPED   -> BILLING_STATUS_BILL\n    ORDER_STATUS_CANCELLED -> BILLING_STATUS_SKIP\n", "")],
    );
    let os = check_dir(dir.path());
    // E404 too: the layer refers to OrderStatus, now with no mapping.
    assert_eq!(codes(&os), ["E406", "E404"]);
    let d = &os[0].diags[0];
    assert!(d.notes.iter().any(|n| n.en.contains("`term キャンセル -> <a term of 請求>`")), "{:?}", d.notes);
}

#[test]
fn a_value_mapped_to_the_same_name_and_a_conformist_with_the_same_word() {
    let os = check_dir(common::mutant("E407_同じ名前の値に読み替えた").path());
    assert_eq!(codes(&os), ["E407"]);
    let os = check_dir(common::mutant("E406_順応者の同じ語").path());
    assert_eq!(codes(&os), ["E406"]);
    let notes: Vec<&str> = os[0].diags[0].notes.iter().map(|n| n.en.as_str()).collect();
    assert!(notes.iter().any(|n| n.contains("make `upstream 在庫` an anticorruption layer")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("`引当 as 在庫.引当`")), "{notes:?}");
}

#[test]
fn a_term_mapping_maps_the_word_too() {
    let dir = variant(
        "基本",
        &[
            ("ctx/請求.ctx", "  キャンセル \"請求を確定したあとに請求を取り消し、返金すること\"\n", "  キャンセル \"請求を確定したあとに請求を取り消し、返金すること\"\n  受注の取消 \"出荷の前に、客の申し出で注文が取り消されたこと\"\n"),
            ("ctx/請求.ctx", "    ORDER_STATUS_CANCELLED -> BILLING_STATUS_SKIP\n", "    ORDER_STATUS_CANCELLED -> BILLING_STATUS_SKIP\n  term キャンセル -> 受注の取消\n"),
        ],
    );
    assert_eq!(codes(&check_dir(dir.path())), Vec::<&str>::new());
}

#[test]
fn a_term_taken_with_as_from_a_context_there_is_a_relationship_with() {
    let dir = variant("基本", &[("ctx/請求.ctx", "  キャンセル \"請求を確定したあとに請求を取り消し、返金すること\"\n", "  キャンセル \"請求を確定したあとに請求を取り消し、返金すること\"\n  注文 as 受注.注文\n")]);
    assert_eq!(codes(&check_dir(dir.path())), Vec::<&str>::new());
}

#[test]
fn what_crosses_through_a_shared_kernel_is_not_held_to_the_words() {
    // 受注 publishes the kernel's Money and calls it 金額; 在庫 has a 金額 of its own. Money
    // crosses into 在庫 through the shared kernel only: the kernel is the two teams' model
    // together, and its words are theirs to agree on (DESIGN 1.6).
    let dir = variant(
        "基本",
        &[
            ("ctx/受注.ctx", "  open host service OrderService
", "  open host service OrderService

published language shop.common.v1
  proto \"../proto/shop/common/v1/money.proto\"
"),
            ("ctx/受注.ctx", "    means enum OrderStatus value ORDER_STATUS_CANCELLED\n", "    means enum OrderStatus value ORDER_STATUS_CANCELLED\n  金額 \"税込みの円\"\n    means message Money\n"),
            ("ctx/在庫.ctx", "    means enum PackingStatus\n", "    means enum PackingStatus\n  金額 \"梱包の進みを答える応答\"\n    means message GetPackingResponse\n"),
        ],
    );
    assert_eq!(codes(&check_dir(dir.path())), Vec::<&str>::new());
    // Through the conformist's reference, the same word is told.
    let dir = variant(
        "基本",
        &[
            ("ctx/受注.ctx", "    means enum OrderStatus value ORDER_STATUS_CANCELLED\n", "    means enum OrderStatus value ORDER_STATUS_CANCELLED\n  金額 \"税込みの円\"\n    means message Order field amount_jpy\n"),
            ("ctx/在庫.ctx", "    means enum PackingStatus\n", "    means enum PackingStatus\n  金額 \"棚の品の仕入れ値\"\n    means message ReserveResponse field price\n"),
        ],
    );
    assert_eq!(codes(&check_dir(dir.path())), ["E406"]);
}
