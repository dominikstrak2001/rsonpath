//! Evaluation of filter expressions, directly on the AST.
use crate::automaton::{index_matches, slice_matches};
use crate::tree::{Edge, JsonNode, NodeValue};
use rsonpath_syntax::{
    num::JsonNumber, Comparable, ComparisonExpr, ComparisonOp, JsonPathQuery, Literal, LogicalExpr, Segment, Selector,
    Selectors, SingularJsonPathQuery, SingularSegment, TestExpr,
};

pub(crate) fn evaluate(expr: &LogicalExpr, node: &JsonNode, root: &JsonNode) -> bool {
    match expr {
        LogicalExpr::Or(lhs, rhs) => evaluate(lhs, node, root) || evaluate(rhs, node, root),
        LogicalExpr::And(lhs, rhs) => evaluate(lhs, node, root) && evaluate(rhs, node, root),
        LogicalExpr::Not(inner) => !evaluate(inner, node, root),
        LogicalExpr::Test(TestExpr::Relative(query)) => any_match(query, node, root),
        LogicalExpr::Test(TestExpr::Absolute(query)) => any_match(query, root, root),
        LogicalExpr::Comparison(comparison) => compare(comparison, node, root),
    }
}

fn any_match(query: &JsonPathQuery, start: &JsonNode, root: &JsonNode) -> bool {
    let mut current = vec![start];
    for segment in query.segments() {
        let mut next = vec![];
        for node in current {
            match segment {
                Segment::Child(selectors) => select_children(node, selectors, root, &mut next),
                Segment::Descendant(selectors) => select_descendants(node, selectors, root, &mut next),
            }
        }
        if next.is_empty() {
            return false;
        }
        current = next;
    }
    true
}

fn select_children<'a>(node: &'a JsonNode, selectors: &Selectors, root: &JsonNode, out: &mut Vec<&'a JsonNode>) {
    for child in &node.children {
        if selectors.iter().any(|selector| selector_matches(selector, child, root)) {
            out.push(child);
        }
    }
}

fn select_descendants<'a>(node: &'a JsonNode, selectors: &Selectors, root: &JsonNode, out: &mut Vec<&'a JsonNode>) {
    select_children(node, selectors, root, out);
    for child in &node.children {
        select_descendants(child, selectors, root, out);
    }
}

fn selector_matches(selector: &Selector, node: &JsonNode, root: &JsonNode) -> bool {
    match (selector, &node.edge) {
        // `node` is always some node's child here, so a wildcard always matches.
        (Selector::Wildcard, _) => true,
        (Selector::Name(name), Edge::Key(key)) => name.unquoted() == key,
        (Selector::Index(selector), Edge::Index { index, len }) => index_matches(*selector, *index, *len),
        (Selector::Slice(slice), Edge::Index { index, len }) => slice_matches(slice, *index, *len),
        (Selector::Filter(expr), _) => evaluate(expr, node, root),
        _ => false,
    }
}

fn compare(comparison: &ComparisonExpr, node: &JsonNode, root: &JsonNode) -> bool {
    let lhs = operand(comparison.lhs(), node, root);
    let rhs = operand(comparison.rhs(), node, root);
    match comparison.op() {
        ComparisonOp::EqualTo => equals(&lhs, &rhs),
        ComparisonOp::NotEqualTo => !equals(&lhs, &rhs),
        ComparisonOp::LessThan => less_than(&lhs, &rhs),
        ComparisonOp::GreaterThan => less_than(&rhs, &lhs),
        ComparisonOp::LesserOrEqualTo => less_than(&lhs, &rhs) || equals(&lhs, &rhs),
        ComparisonOp::GreaterOrEqualTo => less_than(&rhs, &lhs) || equals(&lhs, &rhs),
    }
}

/// An evaluated side of a comparison: a literal, the node the side's singular query
/// found, or `Nothing` if it found none.
enum Operand<'a> {
    Nothing,
    Node(&'a JsonNode),
    Literal(&'a Literal),
}

fn operand<'a>(side: &'a Comparable, node: &'a JsonNode, root: &'a JsonNode) -> Operand<'a> {
    match side {
        Comparable::Literal(literal) => Operand::Literal(literal),
        Comparable::RelativeSingularQuery(query) => singular(query, node).map_or(Operand::Nothing, Operand::Node),
        Comparable::AbsoluteSingularQuery(query) => singular(query, root).map_or(Operand::Nothing, Operand::Node),
    }
}

fn singular<'a>(query: &SingularJsonPathQuery, start: &'a JsonNode) -> Option<&'a JsonNode> {
    let mut current = start;
    for segment in query.segments() {
        current = current.children.iter().find(|child| match (segment, &child.edge) {
            (SingularSegment::Name(name), Edge::Key(key)) => name.unquoted() == key,
            (SingularSegment::Index(selector), Edge::Index { index, len }) => index_matches(*selector, *index, *len),
            _ => false,
        })?;
    }
    Some(current)
}

/// Scalar view for comparisons. RFC 9535 compares numbers as numbers, so integers and
/// floats are unified through `f64`.
enum Atom<'a> {
    Number(f64),
    String(&'a str),
    Bool(bool),
    Null,
}

fn node_atom(node: &JsonNode) -> Option<Atom<'_>> {
    match &node.value {
        NodeValue::String(s) => Some(Atom::String(s)),
        NodeValue::Number(n) => Some(Atom::Number(n.as_f64().unwrap_or(f64::NAN))),
        NodeValue::Bool(b) => Some(Atom::Bool(*b)),
        NodeValue::Null => Some(Atom::Null),
        NodeValue::Object | NodeValue::Array => None,
    }
}

fn literal_atom(literal: &Literal) -> Atom<'_> {
    match literal {
        Literal::String(s) => Atom::String(s.unquoted()),
        Literal::Number(JsonNumber::Int(i)) => Atom::Number(i.as_i64() as f64),
        Literal::Number(JsonNumber::Float(f)) => Atom::Number(f.as_f64()),
        Literal::Bool(b) => Atom::Bool(*b),
        Literal::Null => Atom::Null,
    }
}

fn atom_of<'a>(value: &Operand<'a>) -> Option<Atom<'a>> {
    match value {
        Operand::Nothing => None,
        Operand::Node(node) => node_atom(node),
        Operand::Literal(literal) => Some(literal_atom(literal)),
    }
}

fn equals(lhs: &Operand<'_>, rhs: &Operand<'_>) -> bool {
    match (lhs, rhs) {
        (Operand::Nothing, Operand::Nothing) => true,
        (Operand::Nothing, _) | (_, Operand::Nothing) => false,
        (Operand::Node(a), Operand::Node(b)) => deep_equals(a, b),
        _ => match (atom_of(lhs), atom_of(rhs)) {
            (Some(a), Some(b)) => atoms_equal(&a, &b),
            _ => false,
        },
    }
}

fn atoms_equal(lhs: &Atom<'_>, rhs: &Atom<'_>) -> bool {
    match (lhs, rhs) {
        (Atom::Number(a), Atom::Number(b)) => a == b,
        (Atom::String(a), Atom::String(b)) => a == b,
        (Atom::Bool(a), Atom::Bool(b)) => a == b,
        (Atom::Null, Atom::Null) => true,
        _ => false,
    }
}

fn less_than(lhs: &Operand<'_>, rhs: &Operand<'_>) -> bool {
    match (atom_of(lhs), atom_of(rhs)) {
        (Some(Atom::Number(a)), Some(Atom::Number(b))) => a < b,
        (Some(Atom::String(a)), Some(Atom::String(b))) => a < b,
        _ => false,
    }
}

/// Positional comparison of object members is enough because `serde_json`'s map keeps
/// them sorted by key.
fn deep_equals(a: &JsonNode, b: &JsonNode) -> bool {
    match (&a.value, &b.value) {
        (NodeValue::Object, NodeValue::Object) => {
            a.children.len() == b.children.len()
                && a.children
                    .iter()
                    .zip(&b.children)
                    .all(|(x, y)| x.edge == y.edge && deep_equals(x, y))
        }
        (NodeValue::Array, NodeValue::Array) => {
            a.children.len() == b.children.len() && a.children.iter().zip(&b.children).all(|(x, y)| deep_equals(x, y))
        }
        _ => match (node_atom(a), node_atom(b)) {
            (Some(x), Some(y)) => atoms_equal(&x, &y),
            _ => false,
        },
    }
}
