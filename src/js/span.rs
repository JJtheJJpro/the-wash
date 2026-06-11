//! Source-location bookkeeping.

/// A half-open byte range `[start, end)` within the original source string.
///
/// Both values are `u32` — files larger than 4 GiB are not supported, which
/// is perfectly fine for a browser's JS engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
}

impl Span {
    /// A zero-length sentinel used for synthetic / missing nodes.
    pub const DUMMY: Self = Self { start: 0, end: 0 };

    /// Construct a span from raw byte offsets.
    #[inline]
    pub fn new(start: u32, end: u32) -> Self {
        debug_assert!(start <= end, "span start must be ≤ end");
        Self { start, end }
    }

    /// Extend this span to also cover `other`.
    #[inline]
    pub fn merge(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Length of the span in bytes.
    #[inline]
    pub fn len(self) -> u32 {
        self.end - self.start
    }

    /// `true` if the span covers zero bytes.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Return the source slice covered by this span.
    ///
    /// # Panics
    /// Panics in debug mode if the offsets are out of range.
    #[inline]
    pub fn src<'src>(&self, source: &'src str) -> &'src str {
        &source[self.start as usize..self.end as usize]
    }
}

impl core::fmt::Display for Span {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}