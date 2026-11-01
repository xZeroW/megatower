//! `stats_scroll_container.gd` source ordering pass.

/// One `sort()` pass from the source. `Some(value)` represents a
/// `StatLabelContiner`; other children keep their relative position and are
/// never compared. Larger target values move toward the front one neighbour
/// at a time, matching the forward original loop.
pub fn sort_once<T>(children: &mut [(Option<f32>, T)]) {
    for index in 0..children.len().saturating_sub(1) {
        let (Some(current), Some(next)) = (children[index].0, children[index + 1].0) else {
            continue;
        };
        if current < next {
            children.swap(index, index + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_performs_a_single_forward_adjacent_pass() {
        let mut labels = [(Some(1.0), 'a'), (Some(3.0), 'b'), (Some(2.0), 'c')];
        sort_once(&mut labels);
        assert_eq!(
            labels,
            [(Some(3.0), 'b'), (Some(2.0), 'c'), (Some(1.0), 'a')]
        );
    }

    #[test]
    fn non_stat_children_are_not_moved_or_compared() {
        let mut children = [(Some(1.0), 'a'), (None, 'x'), (Some(3.0), 'b')];
        sort_once(&mut children);
        assert_eq!(children, [(Some(1.0), 'a'), (None, 'x'), (Some(3.0), 'b')]);
    }
}
