/// Identifies a node inside a compiled template.
///
/// `NodeId` is local to a template and references centrally stored nodes
/// without relying on runtime ECS entities or globally unique IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);
