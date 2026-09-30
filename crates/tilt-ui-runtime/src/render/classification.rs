use tilt_ui_core::ElementKind;

/// Describes the minimal Bevy UI primitive required by a built-in element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElementRenderKind {
    /// A layout-only element represented by a Bevy UI node.
    Container,
    /// An interactive semantic element represented by a node and common control marker.
    Control,
    /// An image element represented by Bevy's native image node.
    Image,
}

/// Classifies every built-in TiltUI element by its initial render primitive.
pub(crate) fn element_render_kind(kind: ElementKind) -> ElementRenderKind {
    match kind {
        ElementKind::Badge
        | ElementKind::ContextMenu
        | ElementKind::Body
        | ElementKind::Div
        | ElementKind::Dialog
        | ElementKind::Divider
        | ElementKind::FieldSet
        | ElementKind::Form
        | ElementKind::Headline
        | ElementKind::Label
        | ElementKind::Paragraph
        | ElementKind::ProgressBar
        | ElementKind::Spinner
        | ElementKind::Toast
        | ElementKind::RouterOutlet
        | ElementKind::Table
        | ElementKind::TableCell
        | ElementKind::ToolTip => ElementRenderKind::Container,
        ElementKind::Button
        | ElementKind::Checkbox
        | ElementKind::ChoiceBox
        | ElementKind::Option
        | ElementKind::HyperLink
        | ElementKind::ColorPicker
        | ElementKind::DatePicker
        | ElementKind::Input
        | ElementKind::ListBox
        | ElementKind::RadioButton
        | ElementKind::Scrollbar
        | ElementKind::Slider
        | ElementKind::SwitchButton
        | ElementKind::TextArea
        | ElementKind::ToggleButton => ElementRenderKind::Control,
        ElementKind::Avatar | ElementKind::Image | ElementKind::Icon => ElementRenderKind::Image,
    }
}
