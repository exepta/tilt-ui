//! Shared interaction behavior for native TiltUI controls.

mod activation;
pub(crate) mod context_menu;
mod cursor;
pub(crate) use cursor::CssCursor;
pub use cursor::{UiCursor, set_ui_cursor};
pub(crate) mod drag;
mod editable;
#[cfg(feature = "file-dialog")]
mod file;
mod marker;
mod options;
mod plugin;
mod popup;
mod range;
mod resize;
mod selection;
pub(crate) mod text_selection;

pub use crate::widgets::controls::{
    TiltButton, TiltCheckbox, TiltRadioButton, TiltSwitchButton, TiltToggleButton,
};
pub use activation::ControlActivated;
pub use context_menu::ContextMenu;
pub(crate) use editable::{AnimateInputText, EditableHistory};
pub use marker::{ControlTabIndex, TiltControl};
pub use options::{OptionSelectionChanged, set_option_selected};
pub use plugin::{TiltControlSystems, TiltUiControlRuntimePlugin};
pub use selection::{
    ControlChecked, ControlCheckedChanged, ControlPart, ControlPartKind, FieldSetSelection,
    set_control_checked,
};
pub use text_selection::{ActiveStaticTextSelection, SelectableStaticText};
