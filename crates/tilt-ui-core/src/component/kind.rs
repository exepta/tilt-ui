/// Identifies whether a discovered component is a page or reusable component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ComponentKind {
    /// A routable page component discovered below `src-ui/pages`.
    Page,
    /// A reusable component discovered below `src-ui/components`.
    Component,
}
