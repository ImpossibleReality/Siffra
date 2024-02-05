#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn merge(&self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl<'l> From<pest::Span<'l>> for Span {
    fn from(span: pest::Span) -> Span {
        Span {
            start: span.start(),
            end: span.end(),
        }
    }
}
