//! Module that provides helpful math functions.

use bitvec::prelude::BitSlice;

pub mod ray;
pub mod block;




pub fn bslice_to_usize(slice: &BitSlice) -> usize {
    let mut acc = 0;
    for b in slice {
        acc = acc << 1;
        if *b {
            acc += 1;
        }
    }
    acc
}