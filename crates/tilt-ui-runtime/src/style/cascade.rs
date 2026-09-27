use tilt_ui_css::{ComputedStyle, Specificity, StyleDeclaration};

use super::state::CascadedStyle;

/// Identifies the cascade layer that supplied a stylesheet declaration.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StyleOrigin {
    /// Built-in TiltUI defaults with the lowest precedence.
    DefaultTheme,
    /// Selected named theme, replacing the global selection inside a provider.
    NamedTheme,
    /// Stylesheets linked from the entry document.
    Document,
    /// Additional provider CSS, ordered from outer to inner scopes.
    Provider(usize),
    /// Component-authored stylesheet declarations.
    Author,
    /// An element's `style` attribute or `[style]` binding.
    Inline,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CascadeKey {
    origin: StyleOrigin,
    specificity: Specificity,
    source_order: usize,
    declaration_order: usize,
}

pub(crate) struct Cascade {
    style: ComputedStyle,
    winners: Vec<Option<CascadeKey>>,
}

impl Default for Cascade {
    fn default() -> Self {
        Self {
            style: ComputedStyle::default(),
            winners: vec![None; 49],
        }
    }
}

impl Cascade {
    pub(crate) fn apply(&mut self, declaration: &StyleDeclaration, key: CascadeKey) {
        macro_rules! set {
            ($slot:expr, $field:ident, $value:expr) => {
                if self.winners[$slot].is_none_or(|old| key >= old) {
                    self.winners[$slot] = Some(key);
                    self.style.$field = Some($value);
                }
            };
        }
        match declaration {
            StyleDeclaration::Display(value) => set!(0, display, *value),
            StyleDeclaration::Overflow(x, y) => {
                set!(46, overflow_x, *x);
                set!(47, overflow_y, *y);
            }
            StyleDeclaration::OverflowX(value) => set!(46, overflow_x, *value),
            StyleDeclaration::OverflowY(value) => set!(47, overflow_y, *value),
            StyleDeclaration::Position(value) => set!(1, position, *value),
            StyleDeclaration::Width(value) => set!(2, width, *value),
            StyleDeclaration::Height(value) => set!(3, height, *value),
            StyleDeclaration::MinWidth(value) => set!(4, min_width, *value),
            StyleDeclaration::MinHeight(value) => set!(5, min_height, *value),
            StyleDeclaration::MaxWidth(value) => set!(6, max_width, *value),
            StyleDeclaration::MaxHeight(value) => set!(7, max_height, *value),
            StyleDeclaration::Top(value) => set!(8, top, *value),
            StyleDeclaration::Right(value) => set!(9, right, *value),
            StyleDeclaration::Bottom(value) => set!(10, bottom, *value),
            StyleDeclaration::Left(value) => set!(11, left, *value),
            StyleDeclaration::Margin(value) => set!(12, margin, *value),
            StyleDeclaration::MarginTop(value) => {
                let mut edges = self.style.margin.unwrap_or(tilt_ui_css::Edges::all(*value));
                edges.top = *value;
                set!(12, margin, edges)
            }
            StyleDeclaration::MarginRight(value) => {
                let mut edges = self.style.margin.unwrap_or(tilt_ui_css::Edges::all(*value));
                edges.right = *value;
                set!(12, margin, edges)
            }
            StyleDeclaration::MarginBottom(value) => {
                let mut edges = self.style.margin.unwrap_or(tilt_ui_css::Edges::all(*value));
                edges.bottom = *value;
                set!(12, margin, edges)
            }
            StyleDeclaration::MarginLeft(value) => {
                let mut edges = self.style.margin.unwrap_or(tilt_ui_css::Edges::all(*value));
                edges.left = *value;
                set!(12, margin, edges)
            }
            StyleDeclaration::Padding(value) => set!(13, padding, *value),
            StyleDeclaration::PaddingTop(value) => {
                let mut edges = self
                    .style
                    .padding
                    .unwrap_or(tilt_ui_css::Edges::all(*value));
                edges.top = *value;
                set!(13, padding, edges)
            }
            StyleDeclaration::PaddingRight(value) => {
                let mut edges = self
                    .style
                    .padding
                    .unwrap_or(tilt_ui_css::Edges::all(*value));
                edges.right = *value;
                set!(13, padding, edges)
            }
            StyleDeclaration::PaddingBottom(value) => {
                let mut edges = self
                    .style
                    .padding
                    .unwrap_or(tilt_ui_css::Edges::all(*value));
                edges.bottom = *value;
                set!(13, padding, edges)
            }
            StyleDeclaration::PaddingLeft(value) => {
                let mut edges = self
                    .style
                    .padding
                    .unwrap_or(tilt_ui_css::Edges::all(*value));
                edges.left = *value;
                set!(13, padding, edges)
            }
            StyleDeclaration::Gap(value) => {
                set!(14, row_gap, *value);
                set!(15, column_gap, *value)
            }
            StyleDeclaration::RowGap(value) => set!(14, row_gap, *value),
            StyleDeclaration::ColumnGap(value) => set!(15, column_gap, *value),
            StyleDeclaration::FlexDirection(value) => set!(16, flex_direction, *value),
            StyleDeclaration::FlexWrap(value) => set!(17, flex_wrap, *value),
            StyleDeclaration::JustifyContent(value) => set!(18, justify_content, *value),
            StyleDeclaration::AlignItems(value) => set!(19, align_items, *value),
            StyleDeclaration::AlignSelf(value) => set!(20, align_self, *value),
            StyleDeclaration::FlexGrow(value) => set!(21, flex_grow, *value),
            StyleDeclaration::FlexShrink(value) => set!(22, flex_shrink, *value),
            StyleDeclaration::BackgroundColor(value) => set!(23, background_color, *value),
            StyleDeclaration::BackgroundImage(value) => {
                if self.winners[48].is_none_or(|old| key >= old) {
                    self.winners[48] = Some(key);
                    self.style.background_image = value.clone();
                }
            }
            StyleDeclaration::Color(value) => set!(24, color, *value),
            StyleDeclaration::Opacity(value) => set!(25, opacity, *value),
            StyleDeclaration::Transform(value) => set!(32, transform, *value),
            StyleDeclaration::Animation(value) => set!(33, animation, value.clone()),
            StyleDeclaration::AnimationName(value) => set!(34, animation_name, value.clone()),
            StyleDeclaration::AnimationDuration(value) => {
                set!(35, animation_duration, value.clone())
            }
            StyleDeclaration::AnimationDelay(value) => set!(36, animation_delay, value.clone()),
            StyleDeclaration::AnimationTimingFunction(value) => {
                set!(37, animation_timing_function, value.clone())
            }
            StyleDeclaration::AnimationIterationCount(value) => {
                set!(38, animation_iteration_count, value.clone())
            }
            StyleDeclaration::AnimationDirection(value) => {
                set!(39, animation_direction, value.clone())
            }
            StyleDeclaration::Transition(value) => set!(40, transition, value.clone()),
            StyleDeclaration::TransitionProperty(value) => {
                set!(41, transition_property, value.clone())
            }
            StyleDeclaration::TransitionDuration(value) => {
                set!(42, transition_duration, value.clone())
            }
            StyleDeclaration::TransitionDelay(value) => {
                set!(43, transition_delay, value.clone())
            }
            StyleDeclaration::TransitionTimingFunction(value) => {
                set!(44, transition_timing_function, value.clone())
            }
            StyleDeclaration::BorderWidth(value) => set!(26, border_width, *value),
            StyleDeclaration::BorderColor(value) => set!(27, border_color, *value),
            StyleDeclaration::BorderRadius(value) => set!(28, border_radius, *value),
            StyleDeclaration::FontSize(value) => set!(29, font_size, *value),
            StyleDeclaration::FontFamily(value) => set!(45, font_family, *value),
            StyleDeclaration::FontWeight(value) => set!(30, font_weight, *value),
            StyleDeclaration::TextAlign(value) => set!(31, text_align, *value),
        }
    }

    pub(crate) fn finish(self) -> CascadedStyle {
        CascadedStyle(self.style)
    }
}

pub(crate) fn key(
    origin: StyleOrigin,
    specificity: Specificity,
    source_order: usize,
    declaration_order: usize,
) -> CascadeKey {
    CascadeKey {
        origin,
        specificity,
        source_order,
        declaration_order,
    }
}
