pub(crate) fn node_parent(index: usize) -> Option<usize> {
    if index == 0 {
        None
    } else {
        Some(((index + 1) >> 1) - 1)
    }
}

pub(crate) fn first_child(index: usize) -> usize {
    (index << 1) + 1
}
