//! Definition of the bottom-up [`Automaton`] deciding whether a single marked node
//! is selected by a query.
//!
//! A query with `n` steps has positions `0..=n`. Position `k` means that steps
//! `k+1..=n` remain to be matched between a node and the marked node.
//! The run accepts when the root reaches position 0.
use crate::tree::{Edge, JsonNode};
use rsonpath_syntax::{Index, Slice, Step};
use std::ops::{BitAnd, BitOr, Shr};

/// A set of query positions, stored as a bitmask.
/// This limits queries to [`MAX_STEPS`](crate::compiler::MAX_STEPS) steps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PosSet(u64);

impl PosSet {
    pub const EMPTY: Self = Self(0);

    #[inline]
    #[must_use]
    pub const fn single(position: usize) -> Self {
        Self(1 << position)
    }

    #[inline]
    #[must_use]
    pub const fn contains(self, position: usize) -> bool {
        self.0 & (1 << position) != 0
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for PosSet {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitAnd for PosSet {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl Shr<u32> for PosSet {
    type Output = Self;

    #[inline]
    fn shr(self, rhs: u32) -> Self {
        Self(self.0 >> rhs)
    }
}

/// The steps (numbered from 1) whose selectors a node satisfies through its [`Edge`].
pub type Letter = PosSet;

/// A compiled JSONPath selector, tested against a node's [`Edge`].
#[derive(Debug, Clone)]
pub enum SelectorTest {
    Name(String),
    Wildcard,
    Index(Index),
    Slice(Slice),
}

impl SelectorTest {
    fn matches(&self, edge: &Edge) -> bool {
        match (self, edge) {
            (Self::Wildcard, Edge::Key(_) | Edge::Index { .. }) => true,
            (Self::Name(name), Edge::Key(key)) => name == key,
            (Self::Index(selector), Edge::Index { index, len }) => index_matches(*selector, *index, *len),
            (Self::Slice(slice), Edge::Index { index, len }) => slice_matches(slice, *index, *len),
            _ => false,
        }
    }
}

fn index_matches(selector: Index, index: usize, len: usize) -> bool {
    let (index, len) = (index as u64, len as u64);
    match selector {
        Index::FromStart(i) => i.as_u64() == index,
        Index::FromEnd(i) => i.as_u64() <= len && len - i.as_u64() == index,
    }
}

/// Bounds as in RFC 9535, section 2.3.4.2.
fn slice_matches(slice: &Slice, index: usize, len: usize) -> bool {
    let (index, len) = (index as i64, len as i64);
    let normalize = |i: Index| match i {
        Index::FromStart(i) => i.as_u64() as i64,
        Index::FromEnd(i) => len - i.as_u64() as i64,
    };
    match slice.step() {
        Step::Forward(step) if step.as_u64() == 0 => false,
        Step::Forward(step) => {
            let lower = normalize(slice.start()).clamp(0, len);
            let upper = slice.end().map_or(len, normalize).clamp(0, len);
            lower <= index && index < upper && (index - lower) % step.as_u64() as i64 == 0
        }
        Step::Backward(step) => {
            let upper = normalize(slice.start()).clamp(-1, len - 1);
            let lower = slice.end().map_or(-1, normalize).clamp(-1, len - 1);
            lower < index && index <= upper && (upper - index) % step.as_u64() as i64 == 0
        }
    }
}

/// State of a node in a run with a fixed marked node.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct State {
    /// Positions from which this node reaches the marked node.
    pub matched: PosSet,
    /// Positions produced by a descendant step at this node or below.
    /// They are passed up through ancestors regardless of their edges.
    pub descendant_reach: PosSet,
    /// Positions this node contributes to its parent's `matched`.
    pub offered: PosSet,
}

#[derive(Debug, Clone)]
pub struct Automaton {
    pub(crate) step_count: usize,
    /// Selectors of step `k` at index `k`; index 0 is unused.
    pub(crate) selectors: Vec<Vec<SelectorTest>>,
    /// Positions `k` such that step `k+1` is a child step.
    pub(crate) child_steps: PosSet,
    /// Positions `k` such that step `k+1` is a descendant step.
    pub(crate) descendant_steps: PosSet,
}

impl Automaton {
    /// The letter does not depend on which node is marked, so engines can compute it
    /// once per node and reuse it across runs.
    #[inline]
    #[must_use]
    pub fn letter(&self, node: &JsonNode) -> Letter {
        (1..=self.step_count)
            .filter(|&step| self.selectors[step].iter().any(|s| s.matches(&node.edge)))
            .fold(PosSet::EMPTY, |letter, step| letter | PosSet::single(step))
    }

    #[inline]
    #[must_use]
    pub fn transition(&self, letter: Letter, marked: bool, children: &[State]) -> State {
        let mut matched = if marked {
            PosSet::single(self.step_count)
        } else {
            PosSet::EMPTY
        };
        let mut descendant_reach = PosSet::EMPTY;
        for child in children {
            matched = matched | child.offered;
            descendant_reach = descendant_reach | child.descendant_reach;
        }

        // Step k+1 is taken at this node: its selector matches and k+1 is already matched.
        let steps_taken_here = (letter & matched) >> 1;
        descendant_reach = descendant_reach | (steps_taken_here & self.descendant_steps);
        let offered = (steps_taken_here & self.child_steps) | descendant_reach;

        State {
            matched,
            descendant_reach,
            offered,
        }
    }

    #[inline]
    #[must_use]
    pub fn accepts(&self, root: State) -> bool {
        root.matched.contains(0)
    }
}
