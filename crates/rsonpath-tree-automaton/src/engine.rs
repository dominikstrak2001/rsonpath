//! Engines evaluating an [`Automaton`] over a [`JsonNode`] tree.
use crate::automaton::{Automaton, Letter, State};
use crate::tree::JsonNode;

pub trait Engine {
    /// Result order is unspecified.
    fn select<'a>(&self, automaton: &Automaton, root: &'a JsonNode) -> Vec<&'a JsonNode>;
}

/// One full bottom-up run per node, `O(n^2)`. Reference implementation for testing
/// faster engines.
#[derive(Debug, Default, Clone, Copy)]
pub struct NaiveEngine;

impl Engine for NaiveEngine {
    #[inline]
    fn select<'a>(&self, automaton: &Automaton, root: &'a JsonNode) -> Vec<&'a JsonNode> {
        let mut nodes = vec![];
        let mut letters = vec![];
        compute_letters(automaton, root, root, &mut nodes, &mut letters);

        let mut results = vec![];
        for (mark, node) in nodes.into_iter().enumerate() {
            let mut next_id = 0;
            let root_state = run_marked(automaton, root, &letters, mark, &mut next_id);
            if automaton.accepts(root_state) {
                results.push(node);
            }
        }
        results
    }
}

/// Node ids are pre-order positions; `run_marked` must visit nodes in the same order.
fn compute_letters<'a>(
    automaton: &Automaton,
    node: &'a JsonNode,
    root: &'a JsonNode,
    nodes: &mut Vec<&'a JsonNode>,
    letters: &mut Vec<Letter>,
) {
    nodes.push(node);
    letters.push(automaton.letter(node, root));
    for child in &node.children {
        compute_letters(automaton, child, root, nodes, letters);
    }
}

fn run_marked(automaton: &Automaton, node: &JsonNode, letters: &[Letter], mark: usize, next_id: &mut usize) -> State {
    let id = *next_id;
    *next_id += 1;
    let child_states: Vec<State> = node
        .children
        .iter()
        .map(|child| run_marked(automaton, child, letters, mark, next_id))
        .collect();
    automaton.transition(letters[id], id == mark, &child_states)
}
