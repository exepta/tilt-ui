//! Bevy-specific integration for TiltUI.
//!
//! Parsing and semantic data models remain in the Bevy-independent TiltUI
//! crates. This crate provides the Bevy asset boundary around those models.

/// Native Bevy asset integration for TiltUI component source files.
pub mod assets;

/// Mutable paths for component assets and UI file discovery.
pub mod configuration;

#[cfg(feature = "fluent")]
/// Optional Fluent translation catalogs and locale selection.
pub mod localization;

#[cfg(feature = "component")]
/// Runtime component instantiation for parsed TiltUI templates.
pub mod component;

#[cfg(feature = "component")]
/// Creation-time Bevy UI materialization for TiltUI semantic entities.
pub mod render;

#[cfg(feature = "component")]
/// Shared interaction behavior for native TiltUI controls.
pub mod control;

#[cfg(feature = "component")]
/// Element-specific runtime definitions for built-in TiltUI widgets.
pub mod widgets;

#[cfg(feature = "component")]
/// Component-scoped CSS matching, cascade resolution, and Bevy UI application.
pub mod style;

#[cfg(feature = "component")]
/// Layout-neutral template providers and named CSS themes.
pub mod provider;

#[cfg(feature = "component")]
/// Shared CSS overflow input and scrollbar rendering.
pub mod scroll;

#[cfg(feature = "component")]
pub(crate) mod theme;

#[cfg(feature = "component")]
pub use provider::{
    ProviderChildPolicy, ProviderContext, ProviderEffect, ProviderRules, ProviderScope,
    ThemeProvider, UiProvider, UiProviderAppExt, UiProviderRegistry, UiThemeAppExt, UiThemes,
    register_ui_theme, remove_ui_theme, switch_ui_theme,
};
#[cfg(feature = "component")]
pub use theme::{reset_default_theme, set_default_theme_css};

pub use assets::*;
#[cfg(feature = "component")]
pub use component::ComponentUpdateRegistration;
#[cfg(feature = "component")]
pub use component::*;
pub use configuration::UiRuntimeConfiguration;
#[cfg(feature = "component")]
pub use control::{
    ContextMenu, ControlActivated, ControlChecked, ControlCheckedChanged, ControlPart,
    ControlPartKind, ControlTabIndex, FieldSetSelection, OptionSelectionChanged,
    SelectableStaticText, TiltButton, TiltCheckbox, TiltControl, TiltControlSystems,
    TiltRadioButton, TiltSwitchButton, TiltToggleButton, TiltUiControlRuntimePlugin, UiCursor,
    set_control_checked, set_option_selected, set_ui_cursor,
};
#[cfg(feature = "fluent")]
pub use localization::{
    UiFluentArgs, UiFluentAsset, UiFluentAssetLoader, UiFluentConfig, UiFluentError,
    UiFluentPlugin, UiFluentValue, UiLang, UiLocalization,
};
#[cfg(feature = "tilt-icons")]
pub use render::set_icon_size;
#[cfg(feature = "component")]
pub use style::{
    ActiveAnimations, CascadedStyle, RuntimeComputedStyle, TiltUiMediaEnvironment,
    TiltUiStyleRuntimePlugin,
};
#[cfg(feature = "css")]
pub use tilt_ui_css;
#[cfg(feature = "component")]
pub use widgets::advanced::color_picker::{
    ColorPickerChanged, ColorPickerState, set_color_picker_open, set_color_value,
};
#[cfg(feature = "component")]
pub use widgets::advanced::date_picker::{
    DatePickerChanged, DatePickerRangeChanged, DatePickerState, IsoDate, set_date_picker_open,
    set_date_range, set_date_value,
};
#[cfg(feature = "component")]
pub use widgets::advanced::dialog::{
    DialogClosed, DialogConfig, DialogKind, DialogLayout, DialogRenderer, DialogResult,
    DialogSpawned, DialogState, ShowDialog, close_dialog, open_dialog, spawn_dialog,
};
#[cfg(feature = "component")]
pub use widgets::advanced::hyperlink::{LinkActivated, LinkTarget, set_link_href};
#[cfg(feature = "component")]
pub use widgets::advanced::progress_bar::set_progress_value;
#[cfg(feature = "component")]
pub use widgets::advanced::spinner::TiltSpinner;
#[cfg(feature = "component")]
pub use widgets::advanced::toast::{
    ShowToast, ToastCloseReason, ToastClosed, ToastConfig, ToastKind, ToastPlacement, ToastSpawned,
    ToastStackSettings, ToastState, close_toast, show_toast, spawn_toast,
};
#[cfg(feature = "component")]
pub use widgets::advanced::tooltip::{TooltipSettings, TooltipVariant};
#[cfg(feature = "component")]
pub use widgets::content::avatar::{AvatarFallback, set_avatar_source};
#[cfg(feature = "component")]
pub use widgets::content::badge::{BadgeValue, set_badge_value};
#[cfg(feature = "component")]
pub use widgets::content::divider::DividerAxis;
#[cfg(feature = "component")]
pub use widgets::content::headline::HeadlineLevel;
#[cfg(feature = "tilt-icons")]
pub use widgets::content::image::icon_image;
#[cfg(feature = "component")]
pub use widgets::content::image::set_image_source;
#[cfg(feature = "component")]
pub use widgets::content::image::{ImageMetadata, ImagePreviewInput};
#[cfg(feature = "component")]
pub use widgets::controls::button::FileUploadButton;
#[cfg(feature = "component")]
pub use widgets::controls::button::{LoadingButton, set_button_loading};
#[cfg(feature = "component")]
pub use widgets::controls::choice_box::{ChoiceBoxParts, set_choice_open};
#[cfg(feature = "component")]
pub use widgets::controls::input::{FileInputOptions, FileInputSelected, FileInputSelection};
#[cfg(feature = "component")]
pub use widgets::controls::list_box::ListBoxMode;
#[cfg(feature = "component")]
pub use widgets::controls::option::OptionData;
#[cfg(feature = "component")]
pub use widgets::controls::slider::{SliderSettings, set_slider_value};
#[cfg(feature = "component")]
pub use widgets::state::{
    EditableText, EditableTextChanged, EditableTextCommitted, EditableTextOptions, NumericParts,
    NumericRange, NumericValueChanged, RangeOrientation, SliderChanged, SliderCommitted,
    UiMotionSettings, WidgetLayoutOverride, set_editable_readonly, set_editable_text,
    set_numeric_value, set_slider_values, set_text_area_size,
};
#[cfg(feature = "component")]
pub use widgets::structure::form::{
    FormButton, FormData, FormFile, FormSettings, FormSubmitted, FormValidationFailed, FormValue,
};
#[cfg(feature = "component")]
pub use widgets::structure::{
    table::TableInfo,
    table_cell::{TableCellInfo, TableSection},
};
