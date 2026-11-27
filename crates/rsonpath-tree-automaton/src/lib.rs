//! JSONPath evaluation with bottom-up unranked tree automata.
//!
//! A query is compiled into an [`Automaton`](automaton::Automaton) that decides,
//! for a single marked node of a [`JsonNode`] tree, whether that node is selected.
//! An [`Engine`] uses the automaton to find all selected nodes.
//!
//! # Examples
//! ```rust
//! use rsonpath_tree_automaton::{engine::NaiveEngine, query, tree::JsonNode};
//!
//! let document = serde_json::json!({ "store": { "book": [ { "title": "Dune" } ] } });
//! let tree = JsonNode::from_json(&document);
//!
//! let results = query("$..title", &tree, &NaiveEngine).unwrap();
//!
//! assert_eq!(results.len(), 1);
//! assert_eq!(results[0].to_json(), serde_json::json!("Dune"));
//! ```
#![forbid(unsafe_code)]
// Panic-free lints (disabled for tests).
#![cfg_attr(not(test), warn(clippy::panic, clippy::panic_in_result_fn, clippy::unwrap_used))]

pub mod automaton;
pub mod compiler;
pub mod engine;
mod filter;
pub mod tree;

use engine::Engine;
use thiserror::Error;
use tree::JsonNode;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] rsonpath_syntax::error::ParseError),
    #[error(transparent)]
    Compile(#[from] compiler::CompileError),
}

/// Parse and compile `query`, then select matching nodes of `root` with `engine`.
#[inline]
pub fn query<'a>(query: &str, root: &'a JsonNode, engine: &impl Engine) -> Result<Vec<&'a JsonNode>, Error> {
    let ast = rsonpath_syntax::parse(query)?;
    let automaton = compiler::compile(&ast)?;
    Ok(engine.select(&automaton, root))
}
