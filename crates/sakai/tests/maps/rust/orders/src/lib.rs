//! Orders reserve what the shelves hold.

pub fn reserve(count: u32) -> bool {
    stock::take(count)
}
