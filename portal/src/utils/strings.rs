use std::ops::RangeBounds;

/// Returns the same string, with all the characters outside the range stripped out.
pub fn cut_string<R: RangeBounds<usize>>(mut s: String, range: R) -> String {
    let start_index = match range.start_bound() {
        std::ops::Bound::Included(i) => *i,
        std::ops::Bound::Excluded(i) => *i + 1,
        std::ops::Bound::Unbounded => 0,
    };

    let end_index = match range.end_bound() {
        std::ops::Bound::Included(i) => *i + 1,
        std::ops::Bound::Excluded(i) => *i,
        std::ops::Bound::Unbounded => s.len(),
    };

    if !s.is_char_boundary(start_index) || !s.is_char_boundary(end_index) {
        panic!("The specified range does not split the string across UTF-8 char boundaries");
    }

    unsafe {
        // SAFETY: We previously ensured the range splits the bytes across char boundaries,
        // thus ensuring the slice in the range is, on its own, valid UTF-8.
        let vec = s.as_mut_vec();
        vec.copy_within(range, 0);
        vec.truncate(end_index - start_index);
    }

    s
}
