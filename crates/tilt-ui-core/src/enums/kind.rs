#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementKind {
    // Structure
    Body,
    Div,
    Form,
    Dialog,
    FieldSet,
    Table,
    TableCell,
    RouterOutlet,

    // Content
    Paragraph,
    Label,
    Headline,
    Image,
    Icon,
    Avatar,
    Badge,
    Divider,

    // Controls
    Button,
    Checkbox,
    RadioButton,
    ToggleButton,
    SwitchButton,
    Input,
    TextArea,
    ChoiceBox,
    Option,
    ListBox,
    Slider,

    // Advanced
    ColorPicker,
    ContextMenu,
    DatePicker,
    ProgressBar,
    Spinner,
    Toast,
    Scrollbar,
    ToolTip,
    HyperLink,
}

impl ElementKind {
    /// Returns the canonical template tag name for this built-in element kind.
    pub const fn tag_name(self) -> &'static str {
        match self {
            Self::Avatar => "avatar",
            Self::Badge => "badge",
            Self::Body => "body",
            Self::Button => "button",
            Self::Checkbox => "checkbox",
            Self::ChoiceBox => "choice-box",
            Self::Option => "option",
            Self::ColorPicker => "color-picker",
            Self::ContextMenu => "context-menu",
            Self::DatePicker => "date-picker",
            Self::Dialog => "dialog",
            Self::Div => "div",
            Self::Divider => "divider",
            Self::FieldSet => "field-set",
            Self::Form => "form",
            Self::Headline => "headline",
            Self::HyperLink => "hyperlink",
            Self::Image => "img",
            Self::Icon => "icon",
            Self::Label => "label",
            Self::Input => "input",
            Self::ListBox => "list-box",
            Self::Paragraph => "p",
            Self::ProgressBar => "progress-bar",
            Self::Spinner => "spinner",
            Self::Toast => "toast",
            Self::RadioButton => "radio-button",
            Self::RouterOutlet => "router-outlet",
            Self::Scrollbar => "scrollbar",
            Self::Slider => "slider",
            Self::SwitchButton => "switch-button",
            Self::Table => "table",
            Self::TableCell => "table-cell",
            Self::TextArea => "textarea",
            Self::ToggleButton => "toggle-button",
            Self::ToolTip => "tooltip",
        }
    }
}
