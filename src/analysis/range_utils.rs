//! Range utility functions for LSP operations.
//!
//! Provides efficient range comparison, deduplication, and position checking.

use rustc_hash::FxHashSet;
use tower_lsp::lsp_types::{Position, Range};

use crate::constants::RANGE_SIZE_LINE_WEIGHT;

/// A hashable key for range deduplication.
///
/// Converts an LSP `Range` into a tuple of u32 values that can be
/// efficiently hashed and compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RangeKey {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
}

impl From<Range> for RangeKey {
    fn from(range: Range) -> Self {
        Self {
            start_line: range.start.line,
            start_char: range.start.character,
            end_line: range.end.line,
            end_char: range.end.character,
        }
    }
}

impl RangeKey {
    /// Create a new RangeKey from an LSP Range.
    #[inline]
    pub fn new(range: Range) -> Self {
        Self::from(range)
    }

    /// Convert back to an LSP Range.
    #[inline]
    pub fn to_range(self) -> Range {
        Range::new(
            Position::new(self.start_line, self.start_char),
            Position::new(self.end_line, self.end_char),
        )
    }
}

/// Helper for deduplicating ranges efficiently.
///
/// Uses FxHashSet with RangeKey for fast O(1) lookups.
#[derive(Debug, Default)]
pub struct RangeDeduplicator {
    seen: FxHashSet<RangeKey>,
}

impl RangeDeduplicator {
    /// Create a new deduplicator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new deduplicator with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            seen: FxHashSet::with_capacity_and_hasher(capacity, Default::default()),
        }
    }

    /// Try to insert a range. Returns true if the range was new, false if already seen.
    #[inline]
    pub fn insert(&mut self, range: Range) -> bool {
        self.seen.insert(RangeKey::from(range))
    }

    /// Check if a range has already been seen.
    #[inline]
    pub fn contains(&self, range: Range) -> bool {
        self.seen.contains(&RangeKey::from(range))
    }

    /// Clear all seen ranges.
    pub fn clear(&mut self) {
        self.seen.clear();
    }

    /// Number of unique ranges seen.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Check if no ranges have been seen.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

/// Check if a position is contained within a range.
///
/// The range is treated as half-open: start is inclusive, end is exclusive.
#[inline]
pub fn contains_position(range: Range, pos: Position) -> bool {
    if pos.line < range.start.line || pos.line > range.end.line {
        return false;
    }
    if pos.line == range.start.line && pos.character < range.start.character {
        return false;
    }

    if pos.line == range.end.line && pos.character >= range.end.character {
        return false;
    }

    true
}

/// Check if an inner range is fully contained within an outer range.
#[inline]
pub fn range_contains_range(outer: Range, inner: Range) -> bool {
    if inner.start.line < outer.start.line {
        return false;
    }
    if inner.start.line == outer.start.line && inner.start.character < outer.start.character {
        return false;
    }
    if inner.end.line > outer.end.line {
        return false;
    }
    if inner.end.line == outer.end.line && inner.end.character > outer.end.character {
        return false;
    }

    true
}

/// Check if two ranges overlap.
///
/// Touching ranges (one ends exactly where the other starts) are not considered overlapping.
#[inline]
pub fn ranges_overlap(a: Range, b: Range) -> bool {
    // No overlap if one ends before the other starts
    if a.end.line < b.start.line
        || (a.end.line == b.start.line && a.end.character <= b.start.character)
    {
        return false;
    }
    if b.end.line < a.start.line
        || (b.end.line == a.start.line && b.end.character <= a.start.character)
    {
        return false;
    }

    true
}

/// Calculate a size metric for a range.
///
/// Useful for finding the most specific (smallest) range containing a position.
/// Multi-line ranges are weighted more heavily than single-line ranges.
#[inline]
pub fn range_size(range: Range) -> u64 {
    let lines = (range.end.line - range.start.line) as u64;
    let chars = if range.end.line == range.start.line {
        (range.end.character - range.start.character) as u64
    } else {
        range.end.character as u64
    };
    lines * RANGE_SIZE_LINE_WEIGHT + chars
}

/// Convert a Position to a 1D point for interval tree operations.
///
/// Uses 32-bit line number in upper bits, 32-bit character in lower bits.
#[inline]
pub fn position_to_point(pos: Position) -> u64 {
    ((pos.line as u64) << 32) | (pos.character as u64)
}

/// Convert an LSP Range to an interval for interval tree operations.
///
/// Returns a half-open range [start, end) suitable for interval trees.
#[inline]
pub fn range_to_interval(range: Range) -> std::ops::Range<u64> {
    position_to_point(range.start)..position_to_point(range.end)
}

/// Expand a range by N lines in each direction.
///
/// This is useful for incremental analysis to capture nearby declarations
/// that may be affected by an edit.
#[inline]
pub fn expand_range(range: Range, lines: u32) -> Range {
    Range {
        start: Position {
            line: range.start.line.saturating_sub(lines),
            character: 0,
        },
        end: Position {
            line: range.end.line.saturating_add(lines),
            character: u32::MAX,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    }

    #[test]
    fn test_range_key_roundtrip() {
        let range = make_range(5, 10, 15, 20);
        let key = RangeKey::from(range);
        assert_eq!(key.to_range(), range);
    }

    #[test]
    fn test_range_deduplicator() {
        let mut dedup = RangeDeduplicator::new();
        let range1 = make_range(1, 0, 1, 10);
        let range2 = make_range(2, 0, 2, 10);

        assert!(dedup.insert(range1));
        assert!(!dedup.insert(range1)); // duplicate
        assert!(dedup.insert(range2));
        assert_eq!(dedup.len(), 2);
    }

    #[test]
    fn test_contains_position_basic() {
        let range = make_range(5, 10, 5, 20);

        // Inside
        assert!(contains_position(range, Position::new(5, 15)));

        // At start
        assert!(contains_position(range, Position::new(5, 10)));

        // At end (exclusive)
        assert!(!contains_position(range, Position::new(5, 20)));

        // Before
        assert!(!contains_position(range, Position::new(5, 5)));

        // After
        assert!(!contains_position(range, Position::new(5, 25)));
    }

    #[test]
    fn test_contains_position_multiline() {
        let range = make_range(5, 10, 7, 20);

        // First line
        assert!(contains_position(range, Position::new(5, 15)));

        // Middle line
        assert!(contains_position(range, Position::new(6, 0)));

        // Last line
        assert!(contains_position(range, Position::new(7, 10)));

        // Past end
        assert!(!contains_position(range, Position::new(7, 20)));
    }

    #[test]
    fn test_ranges_overlap() {
        let a = make_range(5, 10, 5, 20);
        let b = make_range(5, 15, 5, 25);

        assert!(ranges_overlap(a, b));
        assert!(ranges_overlap(b, a));
    }

    #[test]
    fn test_ranges_no_overlap_touching() {
        let a = make_range(5, 10, 5, 20);
        let b = make_range(5, 20, 5, 30);

        // Touching ranges don't overlap
        assert!(!ranges_overlap(a, b));
        assert!(!ranges_overlap(b, a));
    }

    #[test]
    fn test_ranges_no_overlap_separate() {
        let a = make_range(5, 10, 5, 20);
        let b = make_range(6, 0, 6, 10);

        assert!(!ranges_overlap(a, b));
        assert!(!ranges_overlap(b, a));
    }

    #[test]
    fn test_range_size_single_line() {
        let range = make_range(5, 10, 5, 20);
        assert_eq!(range_size(range), 10);
    }

    #[test]
    fn test_range_size_multi_line() {
        let range = make_range(5, 10, 8, 20);
        let size = range_size(range);
        // lines = 3, chars = end.character = 20
        assert_eq!(size, 3 * RANGE_SIZE_LINE_WEIGHT + 20);
    }

    #[test]
    fn test_position_to_point() {
        let pos = Position::new(100, 50);
        let point = position_to_point(pos);

        // Verify line is in upper 32 bits, character in lower 32 bits
        assert_eq!((point >> 32) as u32, 100);
        assert_eq!(point as u32, 50);
    }

    #[test]
    fn test_range_to_interval() {
        let range = make_range(5, 10, 7, 20);
        let interval = range_to_interval(range);

        assert_eq!(interval.start, position_to_point(range.start));
        assert_eq!(interval.end, position_to_point(range.end));
    }

    #[test]
    fn test_range_contains_range() {
        let outer = make_range(5, 10, 10, 20);
        let inner = make_range(6, 0, 9, 30);

        assert!(range_contains_range(outer, inner));
        assert!(!range_contains_range(inner, outer));
    }

    #[test]
    fn test_expand_range() {
        let range = make_range(10, 5, 15, 10);
        let expanded = expand_range(range, 3);

        // Start line should be 10 - 3 = 7, character 0
        assert_eq!(expanded.start.line, 7);
        assert_eq!(expanded.start.character, 0);

        // End line should be 15 + 3 = 18, character MAX
        assert_eq!(expanded.end.line, 18);
        assert_eq!(expanded.end.character, u32::MAX);
    }

    #[test]
    fn test_expand_range_saturating() {
        // Test that expansion near line 0 doesn't underflow
        let range = make_range(1, 5, 3, 10);
        let expanded = expand_range(range, 5);

        assert_eq!(expanded.start.line, 0); // saturating_sub prevents underflow
        assert_eq!(expanded.start.character, 0);
    }
}
