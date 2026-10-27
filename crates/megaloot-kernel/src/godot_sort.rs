//! Godot 4.4.1 unstable introsort. Shared by item-catalog and inventory ordering.
//! Adapted from core/templates/sort_array.h; see third_party/RNG-NOTICE.md.

pub fn source_sort<T: Copy>(items: &mut [T], less: impl Fn(&T, &T) -> bool) {
    fn heap_adjust<T: Copy, F: Fn(&T, &T) -> bool>(
        a: &mut [T],
        first: usize,
        mut hole: usize,
        len: usize,
        value: T,
        less: &F,
    ) {
        let top = hole;
        let mut child = 2 * hole + 2;
        while child < len {
            if less(&a[first + child], &a[first + child - 1]) {
                child -= 1;
            }
            a[first + hole] = a[first + child];
            hole = child;
            child = 2 * (child + 1);
        }
        if child == len {
            a[first + hole] = a[first + child - 1];
            hole = child - 1;
        }
        while hole > top {
            let parent = (hole - 1) / 2;
            if !less(&a[first + parent], &value) {
                break;
            }
            a[first + hole] = a[first + parent];
            hole = parent;
        }
        a[first + hole] = value;
    }
    fn heap_sort<T: Copy, F: Fn(&T, &T) -> bool>(a: &mut [T], first: usize, last: usize, less: &F) {
        let len = last - first;
        if len < 2 {
            return;
        }
        for parent in (0..=(len - 2) / 2).rev() {
            heap_adjust(a, first, parent, len, a[first + parent], less);
        }
        for end in ((first + 1)..last).rev() {
            let value = a[end];
            a[end] = a[first];
            heap_adjust(a, first, 0, end - first, value, less);
        }
    }
    fn intro<T: Copy, F: Fn(&T, &T) -> bool>(
        a: &mut [T],
        first: usize,
        mut last: usize,
        mut depth: u32,
        less: &F,
    ) {
        while last - first > 16 {
            if depth == 0 {
                heap_sort(a, first, last, less);
                return;
            }
            depth -= 1;
            let (x, y, z) = (a[first], a[first + (last - first) / 2], a[last - 1]);
            let pivot = if less(&x, &y) {
                if less(&y, &z) {
                    y
                } else if less(&x, &z) {
                    z
                } else {
                    x
                }
            } else if less(&x, &z) {
                x
            } else if less(&y, &z) {
                z
            } else {
                y
            };
            let (mut left, mut right) = (first, last);
            let cut = loop {
                while less(&a[left], &pivot) {
                    left += 1;
                }
                right -= 1;
                while less(&pivot, &a[right]) {
                    right -= 1;
                }
                if left >= right {
                    break left;
                }
                a.swap(left, right);
                left += 1;
            };
            intro(a, cut, last, depth, less);
            last = cut;
        }
    }
    if items.is_empty() {
        return;
    }
    intro(items, 0, items.len(), items.len().ilog2() * 2, &less);
    for index in 1..items.len() {
        let value = items[index];
        let mut hole = index;
        while hole > 0 && less(&value, &items[hole - 1]) {
            items[hole] = items[hole - 1];
            hole -= 1;
        }
        items[hole] = value;
    }
}
