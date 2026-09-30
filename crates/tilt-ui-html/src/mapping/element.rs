use tilt_ui_core::ElementKind;

/// Maps a built-in template tag to its corresponding element kind.
pub fn map_element(tag: &str) -> Option<ElementKind> {
    match tag {
        "avatar" => Some(ElementKind::Avatar),
        "badge" => Some(ElementKind::Badge),
        "body" => Some(ElementKind::Body),
        "button" => Some(ElementKind::Button),
        "checkbox" => Some(ElementKind::Checkbox),
        "select" | "choice-box" => Some(ElementKind::ChoiceBox),
        "colorpicker" | "color-picker" => Some(ElementKind::ColorPicker),
        "context-menu" => Some(ElementKind::ContextMenu),
        "date-picker" => Some(ElementKind::DatePicker),
        "dialog" => Some(ElementKind::Dialog),
        "div" => Some(ElementKind::Div),
        "divider" => Some(ElementKind::Divider),
        "fieldset" | "field-set" => Some(ElementKind::FieldSet),
        "form" => Some(ElementKind::Form),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "headline" => Some(ElementKind::Headline),
        "a" | "hyperlink" => Some(ElementKind::HyperLink),
        "img" | "image" => Some(ElementKind::Image),
        "input" => Some(ElementKind::Input),
        "label" => Some(ElementKind::Label),
        "list-box" => Some(ElementKind::ListBox),
        "option" => Some(ElementKind::Option),
        "p" | "paragraph" => Some(ElementKind::Paragraph),
        "icon" => Some(ElementKind::Icon),
        "progressbar" | "progress-bar" => Some(ElementKind::ProgressBar),
        "spinner" => Some(ElementKind::Spinner),
        "toast" => Some(ElementKind::Toast),
        "radio" | "radio-button" => Some(ElementKind::RadioButton),
        "router-outlet" => Some(ElementKind::RouterOutlet),
        "scroll" | "scrollbar" => Some(ElementKind::Scrollbar),
        "slider" => Some(ElementKind::Slider),
        "switch" | "switch-button" => Some(ElementKind::SwitchButton),
        "table" => Some(ElementKind::Table),
        "table-cell" | "td" | "th" => Some(ElementKind::TableCell),
        "textarea" => Some(ElementKind::TextArea),
        "toggle" | "toggle-button" => Some(ElementKind::ToggleButton),
        "tooltip" => Some(ElementKind::ToolTip),
        _ => None,
    }
}

/// Returns whether a tag is reserved for a built-in TiltUI element.
pub fn is_builtin_element_tag(tag: &str) -> bool {
    map_element(tag).is_some()
}
