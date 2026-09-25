/// Describes the semantic form of an attribute declared on a template node.
///
/// Static values, property bindings, and event bindings retain distinct
/// representations while expressions remain unevaluated source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateAttribute {
    /// Stores a literal attribute value, such as `class="menu"`.
    Static { name: String, value: String },

    /// Stores a property expression, such as `[disabled]="loading"`.
    PropertyBinding { name: String, expression: String },

    /// Stores an event expression, such as `(click)="start_game()"`.
    EventBinding { name: String, expression: String },
}
