#![allow(dead_code)]

// Include TRAVERSAL_TABLE generated by the build.rs script
include!(concat!(env!("OUT_DIR"), "/table_inc.rs"));

/// Represents the split axes of the BVH2 node(s) that a BVH4 node was created
/// from.
///
/// * `Full` is four nodes from three splits: top, left, and right.
/// * `Left` is three nodes from two splits: top and left.
/// * `Right` is three nodes from two splits: top and right.
/// * `TopOnly` is two nodes from one split (in other words, the BVH4 node is
///   identical to the single BVH2 node that it was created from).
///
/// The left node of a split is the node whose coordinate on the top split-axis
/// is lower.  For example, if the top split is on the x axis, then `left.x <= right.x`.
///
/// The values representing each axis are x = 0, y = 1, and z = 2.
#[derive(Debug, Copy, Clone)]
pub enum SplitAxes {
    Full((u8, u8, u8)), // top, left, right
    Left((u8, u8)),     // top, left
    Right((u8, u8)),    // top, right
    TopOnly(u8),        // top
}

/// Calculates the traversal code for a BVH4 node based on the splits and
/// topology of the BVH2 node(s) it was created from.
#[inline(always)]
pub fn calc_traversal_code(split: SplitAxes) -> u8 {
    match split {
        SplitAxes::Full((top, left, right)) => top + (left * 3) + (right * 9),
        SplitAxes::Left((top, left)) => top + (left * 3) + 27,
        SplitAxes::Right((top, right)) => top + (right * 3) + (27 + 9),
        SplitAxes::TopOnly(top) => top + (27 + 9 + 9),
    }
}
