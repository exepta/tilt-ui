/// Identifies a component inside one generated component manifest.
///
/// IDs are assigned deterministically at build time and are local to the
/// consuming application's discovered component set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u32);
