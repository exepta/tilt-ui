use std::collections::BTreeMap;

use tilt_ui_css::{
    ComputedStyle, Edges, Length, Specificity, StyleDeclaration, ValueContext,
    parse_declaration_value, resolve_value,
};

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
    important: bool,
    origin: StyleOrigin,
    specificity: Specificity,
    source_order: usize,
    declaration_order: usize,
}

pub(crate) struct Cascade {
    style: ComputedStyle,
    winners: Vec<Option<CascadeKey>>,
    variables: BTreeMap<String, (String, Option<CascadeKey>)>,
    deferred: Vec<(String, String, CascadeKey)>,
}

impl Default for Cascade {
    fn default() -> Self {
        Self {
            style: ComputedStyle::default(),
            winners: vec![None; 93],
            variables: BTreeMap::new(),
            deferred: Vec::new(),
        }
    }
}

impl Cascade {
    pub(crate) fn with_variables(inherited: BTreeMap<String, String>) -> Self {
        Self {
            variables: inherited
                .into_iter()
                .map(|(name, value)| (name, (value, None)))
                .collect(),
            ..Self::default()
        }
    }

    pub(crate) fn variables(&self) -> BTreeMap<String, String> {
        self.variables
            .iter()
            .map(|(name, (value, _))| (name.clone(), value.clone()))
            .collect()
    }

    pub(crate) fn has_deferred(&self) -> bool {
        !self.deferred.is_empty()
    }

    pub(crate) fn resolve_deferred(&mut self, mut context: impl FnMut(&str) -> ValueContext) {
        let variables = self.variables();
        for (property, source, key) in std::mem::take(&mut self.deferred) {
            let Ok(resolved) = resolve_value(&source, &variables, context(&property)) else {
                continue;
            };
            if let Ok(declaration) = parse_declaration_value(&property, &resolved) {
                self.apply(&declaration, key);
            }
        }
    }

    pub(crate) fn apply(&mut self, declaration: &StyleDeclaration, key: CascadeKey) {
        match declaration {
            StyleDeclaration::Important(inner) => {
                self.apply(
                    inner,
                    CascadeKey {
                        important: true,
                        ..key
                    },
                );
                return;
            }
            StyleDeclaration::CustomProperty(name, value) => {
                if self
                    .variables
                    .get(name)
                    .is_none_or(|(_, old)| old.is_none_or(|old| key >= old))
                {
                    self.variables
                        .insert(name.clone(), (value.clone(), Some(key)));
                }
                return;
            }
            StyleDeclaration::Deferred(property, value) => {
                self.deferred.push((property.clone(), value.clone(), key));
                return;
            }
            _ => {}
        }
        macro_rules! set {
            ($slot:expr, $field:ident, $value:expr) => {
                if self.winners[$slot].is_none_or(|old| key >= old) {
                    self.winners[$slot] = Some(key);
                    self.style.$field = Some($value);
                }
            };
        }
        macro_rules! set_edge {
            ($slot:expr, $field:ident, $side:ident, $value:expr) => {
                if self.winners[$slot].is_none_or(|old| key >= old) {
                    self.winners[$slot] = Some(key);
                    self.style
                        .$field
                        .get_or_insert(Edges::all(Length::Px(0.0)))
                        .$side = $value;
                }
            };
        }
        macro_rules! set_color_edge {
            ($slot:expr, $side:ident, $value:expr) => {
                if self.winners[$slot].is_none_or(|old| key >= old) {
                    self.winners[$slot] = Some(key);
                    self.style
                        .border_color_edges
                        .get_or_insert(Edges::all(tilt_ui_css::CssColor::transparent()))
                        .$side = $value;
                }
            };
        }
        match declaration {
            StyleDeclaration::Important(_)
            | StyleDeclaration::CustomProperty(_, _)
            | StyleDeclaration::Deferred(_, _) => unreachable!(),
            StyleDeclaration::Display(value) => set!(0, display, *value),
            StyleDeclaration::BoxSizing(value) => set!(61, box_sizing, *value),
            StyleDeclaration::PointerEvents(value) => set!(77, pointer_events, *value),
            StyleDeclaration::Cursor(value) => set!(78, cursor, *value),
            StyleDeclaration::ZIndex(value) => set!(79, z_index, *value),
            StyleDeclaration::ScrollWidth(value) => set!(80, scroll_width, *value),
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
            StyleDeclaration::Margin(value) => {
                set_edge!(49, margin, top, value.top);
                set_edge!(50, margin, right, value.right);
                set_edge!(51, margin, bottom, value.bottom);
                set_edge!(52, margin, left, value.left);
            }
            StyleDeclaration::MarginTop(value) => set_edge!(49, margin, top, *value),
            StyleDeclaration::MarginRight(value) => set_edge!(50, margin, right, *value),
            StyleDeclaration::MarginBottom(value) => set_edge!(51, margin, bottom, *value),
            StyleDeclaration::MarginLeft(value) => set_edge!(52, margin, left, *value),
            StyleDeclaration::Padding(value) => {
                set_edge!(53, padding, top, value.top);
                set_edge!(54, padding, right, value.right);
                set_edge!(55, padding, bottom, value.bottom);
                set_edge!(56, padding, left, value.left);
            }
            StyleDeclaration::PaddingTop(value) => set_edge!(53, padding, top, *value),
            StyleDeclaration::PaddingRight(value) => set_edge!(54, padding, right, *value),
            StyleDeclaration::PaddingBottom(value) => set_edge!(55, padding, bottom, *value),
            StyleDeclaration::PaddingLeft(value) => set_edge!(56, padding, left, *value),
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
            StyleDeclaration::FlexBasis(value) => set!(62, flex_basis, *value),
            StyleDeclaration::Flex(grow, shrink, basis) => {
                self.apply(&StyleDeclaration::FlexGrow(*grow), key);
                self.apply(&StyleDeclaration::FlexShrink(*shrink), key);
                self.apply(&StyleDeclaration::FlexBasis(*basis), key);
            }
            StyleDeclaration::FlexFlow(direction, wrap) => {
                self.apply(&StyleDeclaration::FlexDirection(*direction), key);
                self.apply(&StyleDeclaration::FlexWrap(*wrap), key);
            }
            StyleDeclaration::GridTemplateRows(value) => {
                set!(63, grid_template_rows, value.clone())
            }
            StyleDeclaration::GridTemplateColumns(value) => {
                set!(64, grid_template_columns, value.clone())
            }
            StyleDeclaration::GridAutoRows(value) => set!(65, grid_auto_rows, value.clone()),
            StyleDeclaration::GridAutoColumns(value) => set!(66, grid_auto_columns, value.clone()),
            StyleDeclaration::GridAutoFlow(value) => set!(67, grid_auto_flow, *value),
            StyleDeclaration::GridRow(value) => set!(68, grid_row, *value),
            StyleDeclaration::GridColumn(value) => set!(69, grid_column, *value),
            StyleDeclaration::BackgroundColor(value) => set!(23, background_color, *value),
            StyleDeclaration::BackgroundImage(value) => {
                if self.winners[48].is_none_or(|old| key >= old) {
                    self.winners[48] = Some(key);
                    self.style.background_image = value.clone();
                }
            }
            StyleDeclaration::BackgroundSize(value) => set!(86, background_size, *value),
            StyleDeclaration::BackgroundPosition(value) => set!(87, background_position, *value),
            StyleDeclaration::BackgroundAttachment(value) => {
                set!(88, background_attachment, *value)
            }
            StyleDeclaration::BackgroundFilter(value) => {
                set!(89, background_filter, value.clone())
            }
            StyleDeclaration::BackdropFilter(value) => {
                set!(90, backdrop_filter, value.clone())
            }
            StyleDeclaration::AnimatedEffects(value) => {
                set!(91, animated_effects, value.clone())
            }
            StyleDeclaration::EffectQuality(value) => {
                set!(92, effect_quality, *value)
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
            StyleDeclaration::BorderWidth(value) => {
                set_edge!(57, border_width, top, value.top);
                set_edge!(58, border_width, right, value.right);
                set_edge!(59, border_width, bottom, value.bottom);
                set_edge!(60, border_width, left, value.left);
            }
            StyleDeclaration::BorderTopWidth(value) => set_edge!(57, border_width, top, *value),
            StyleDeclaration::BorderRightWidth(value) => set_edge!(58, border_width, right, *value),
            StyleDeclaration::BorderBottomWidth(value) => {
                set_edge!(59, border_width, bottom, *value)
            }
            StyleDeclaration::BorderLeftWidth(value) => set_edge!(60, border_width, left, *value),
            StyleDeclaration::Border(edge) => {
                self.apply(&StyleDeclaration::BorderWidth(Edges::all(edge.width)), key);
                self.apply(&StyleDeclaration::BorderColor(edge.color), key);
            }
            StyleDeclaration::BorderTop(edge) => {
                self.apply(&StyleDeclaration::BorderTopWidth(edge.width), key);
                self.apply(&StyleDeclaration::BorderTopColor(edge.color), key);
            }
            StyleDeclaration::BorderRight(edge) => {
                self.apply(&StyleDeclaration::BorderRightWidth(edge.width), key);
                self.apply(&StyleDeclaration::BorderRightColor(edge.color), key);
            }
            StyleDeclaration::BorderBottom(edge) => {
                self.apply(&StyleDeclaration::BorderBottomWidth(edge.width), key);
                self.apply(&StyleDeclaration::BorderBottomColor(edge.color), key);
            }
            StyleDeclaration::BorderLeft(edge) => {
                self.apply(&StyleDeclaration::BorderLeftWidth(edge.width), key);
                self.apply(&StyleDeclaration::BorderLeftColor(edge.color), key);
            }
            StyleDeclaration::BorderColor(value) => {
                set!(27, border_color, *value);
                set_color_edge!(70, top, *value);
                set_color_edge!(71, right, *value);
                set_color_edge!(72, bottom, *value);
                set_color_edge!(73, left, *value);
            }
            StyleDeclaration::BorderTopColor(value) => set_color_edge!(70, top, *value),
            StyleDeclaration::BorderRightColor(value) => set_color_edge!(71, right, *value),
            StyleDeclaration::BorderBottomColor(value) => set_color_edge!(72, bottom, *value),
            StyleDeclaration::BorderLeftColor(value) => set_color_edge!(73, left, *value),
            StyleDeclaration::BorderRadius(value) => set!(28, border_radius, *value),
            StyleDeclaration::BoxShadow(value) => set!(81, box_shadow, value.clone()),
            StyleDeclaration::TextShadow(value) => {
                if self.winners[82].is_none_or(|old| key >= old) {
                    self.winners[82] = Some(key);
                    self.style.text_shadow = *value;
                }
            }
            StyleDeclaration::Outline(edge) => {
                self.apply(&StyleDeclaration::OutlineWidth(edge.width), key);
                self.apply(&StyleDeclaration::OutlineColor(edge.color), key);
            }
            StyleDeclaration::OutlineWidth(value) => set!(83, outline_width, *value),
            StyleDeclaration::OutlineColor(value) => set!(84, outline_color, *value),
            StyleDeclaration::OutlineOffset(value) => set!(85, outline_offset, *value),
            StyleDeclaration::FontSize(value) => set!(29, font_size, *value),
            StyleDeclaration::LineHeight(value) => set!(74, line_height, *value),
            StyleDeclaration::FontFamily(value) => set!(45, font_family, value.clone()),
            StyleDeclaration::FontWeight(value) => set!(30, font_weight, *value),
            StyleDeclaration::TextAlign(value) => set!(31, text_align, *value),
            StyleDeclaration::TextWrap(value) => set!(75, text_wrap, *value),
            StyleDeclaration::TextTransform(value) => set!(76, text_transform, *value),
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
        important: false,
        origin,
        specificity,
        source_order,
        declaration_order,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tilt_ui_css::parse_stylesheet;

    #[test]
    fn border_shorthand_respects_independent_side_priority() {
        let declarations = parse_stylesheet(
            "* { border: 1px solid #111111; border-left: 3px solid #222222 !important; border-color: #333333; border-top-width: 4px; }",
        ).unwrap().rules.remove(0).declarations;
        let mut cascade = Cascade::default();
        for (index, declaration) in declarations.iter().enumerate() {
            cascade.apply(
                declaration,
                key(StyleOrigin::Author, Specificity(0), 0, index),
            );
        }
        let style = cascade.finish().0;
        let widths = style.border_width.unwrap();
        let colors = style.border_color_edges.unwrap();
        assert_eq!(widths.top, Length::Px(4.0));
        assert_eq!(widths.left, Length::Px(3.0));
        assert_eq!(
            colors.left,
            tilt_ui_css::CssColor::rgba(34.0 / 255.0, 34.0 / 255.0, 34.0 / 255.0, 1.0)
        );
        assert_eq!(
            colors.right,
            tilt_ui_css::CssColor::rgba(51.0 / 255.0, 51.0 / 255.0, 51.0 / 255.0, 1.0)
        );
    }

    #[test]
    fn important_priority_and_independent_edge_winners() {
        let declarations = parse_stylesheet(
            "* { color: #111111 !important; margin-left: 7px !important; margin: 2px; }",
        )
        .unwrap()
        .rules
        .remove(0)
        .declarations;
        let mut cascade = Cascade::default();
        for (index, declaration) in declarations.iter().enumerate() {
            cascade.apply(
                declaration,
                key(StyleOrigin::Author, Specificity(0), 0, index),
            );
        }
        let inline = parse_stylesheet("* { color: #ffffff; margin: 3px; }")
            .unwrap()
            .rules
            .remove(0)
            .declarations;
        for (index, declaration) in inline.iter().enumerate() {
            cascade.apply(
                declaration,
                key(StyleOrigin::Inline, Specificity(0), 0, index),
            );
        }
        let style = cascade.finish().0;
        assert_eq!(
            style.color,
            Some(tilt_ui_css::CssColor::rgba(
                17.0 / 255.0,
                17.0 / 255.0,
                17.0 / 255.0,
                1.0
            ))
        );
        assert_eq!(
            style.margin.unwrap(),
            Edges {
                top: Length::Px(3.0),
                right: Length::Px(3.0),
                bottom: Length::Px(3.0),
                left: Length::Px(7.0)
            }
        );
    }

    #[test]
    fn deferred_values_use_cascaded_variables_and_priority() {
        let declarations = parse_stylesheet(
            "* { --size: 10px; width: var(--size); --size: 12px !important; width: 20px; }",
        )
        .unwrap()
        .rules
        .remove(0)
        .declarations;
        let mut cascade = Cascade::default();
        for (index, declaration) in declarations.iter().enumerate() {
            cascade.apply(
                declaration,
                key(StyleOrigin::Author, Specificity(0), 0, index),
            );
        }
        cascade.resolve_deferred(|_| ValueContext::default());
        assert_eq!(cascade.finish().0.width, Some(Length::Px(20.0)));
    }
}
