//! Definition of [`JsonNode`], an owned tree representation of a JSON document.
use serde_json::{Map, Number, Value};

/// The way a node is reached from its parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edge {
    Root,
    Key(String),
    Index {
        index: usize,
        /// Length of the parent array, needed for negative indices and slices.
        len: usize,
    },
}

/// The value of a [`JsonNode`].
///
/// Objects and arrays keep their contents in [`JsonNode::children`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeValue {
    Object,
    Array,
    String(String),
    Number(Number),
    Bool(bool),
    Null,
}

/// A node of a JSON document tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonNode {
    pub edge: Edge,
    pub value: NodeValue,
    /// Array elements are kept in document order.
    pub children: Vec<Self>,
}

impl JsonNode {
    /// Convert a [`serde_json::Value`] into a tree rooted at an [`Edge::Root`].
    ///
    /// # Examples
    /// ```rust
    /// # use rsonpath_tree_automaton::tree::{Edge, JsonNode};
    /// let tree = JsonNode::from_json(&serde_json::json!([10, 20]));
    ///
    /// assert_eq!(tree.children[1].edge, Edge::Index { index: 1, len: 2 });
    /// ```
    #[inline]
    #[must_use]
    pub fn from_json(value: &Value) -> Self {
        Self::build(Edge::Root, value)
    }

    fn build(edge: Edge, value: &Value) -> Self {
        let (node_value, children) = match value {
            Value::Null => (NodeValue::Null, vec![]),
            Value::Bool(b) => (NodeValue::Bool(*b), vec![]),
            Value::Number(n) => (NodeValue::Number(n.clone()), vec![]),
            Value::String(s) => (NodeValue::String(s.clone()), vec![]),
            Value::Array(items) => {
                let len = items.len();
                let elements = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| Self::build(Edge::Index { index, len }, item))
                    .collect();
                (NodeValue::Array, elements)
            }
            Value::Object(members) => {
                let members = members
                    .iter()
                    .map(|(key, member)| Self::build(Edge::Key(key.clone()), member))
                    .collect();
                (NodeValue::Object, members)
            }
        };
        Self {
            edge,
            value: node_value,
            children,
        }
    }

    #[inline]
    #[must_use]
    pub fn to_json(&self) -> Value {
        match &self.value {
            NodeValue::Null => Value::Null,
            NodeValue::Bool(b) => Value::Bool(*b),
            NodeValue::Number(n) => Value::Number(n.clone()),
            NodeValue::String(s) => Value::String(s.clone()),
            NodeValue::Array => Value::Array(self.children.iter().map(Self::to_json).collect()),
            NodeValue::Object => {
                let mut members = Map::new();
                for child in &self.children {
                    if let Edge::Key(key) = &child.edge {
                        members.insert(key.clone(), child.to_json());
                    }
                }
                Value::Object(members)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip() {
        let value = json!({"a": [1, "x", null, {"b": true}], "c": {}});
        assert_eq!(
            JsonNode::from_json(&value).to_json(),
            value,
            "conversion must round-trip"
        );
    }
}
