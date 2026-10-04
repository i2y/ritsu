//! What is on the shelves.

pub fn take(count: u32) -> bool {
    stock_ledger::on_hand() >= count
}
