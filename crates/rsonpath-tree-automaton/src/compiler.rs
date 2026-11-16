//! Compilation of a [`JsonPathQuery`] into an [`Automaton`].
use crate::automaton::{Automaton, PosSet, SelectorTest};
use rsonpath_syntax::{JsonPathQuery, Segment, Selector};
use thiserror::Error;

/// Positions `0..=MAX_STEPS` must fit in a [`PosSet`] bitmask.
pub const MAX_STEPS: usize = 63;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("query has {0} steps, at most {MAX_STEPS} are supported")]
    TooManySteps(usize),
    #[error("filter selectors are not supported yet")]
    FilterNotSupported,
}

#[inline]
pub fn compile(query: &JsonPathQuery) -> Result<Automaton, CompileError> {
    let segments = query.segments();
    if segments.len() > MAX_STEPS {
        return Err(CompileError::TooManySteps(segments.len()));
    }

    let mut selectors = vec![vec![]];
    let mut child_steps = PosSet::EMPTY;
    let mut descendant_steps = PosSet::EMPTY;

    for (k, segment) in segments.iter().enumerate() {
        match segment {
            Segment::Child(_) => child_steps = child_steps | PosSet::single(k),
            Segment::Descendant(_) => descendant_steps = descendant_steps | PosSet::single(k),
        }
        let tests = segment
            .selectors()
            .iter()
            .map(compile_selector)
            .collect::<Result<Vec<_>, _>>()?;
        selectors.push(tests);
    }

    Ok(Automaton {
        step_count: segments.len(),
        selectors,
        child_steps,
        descendant_steps,
    })
}

fn compile_selector(selector: &Selector) -> Result<SelectorTest, CompileError> {
    match selector {
        Selector::Name(name) => Ok(SelectorTest::Name(name.unquoted().to_owned())),
        Selector::Wildcard => Ok(SelectorTest::Wildcard),
        Selector::Index(index) => Ok(SelectorTest::Index(*index)),
        Selector::Slice(slice) => Ok(SelectorTest::Slice(slice.clone())),
        Selector::Filter(_) => Err(CompileError::FilterNotSupported),
    }
}
