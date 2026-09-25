use bevy::{
    color::Color,
    text::{FontSize, FontWeight},
    ui::{
        AlignItems, AlignSelf, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent,
        PositionType, UiRect, UiTransform, Val, Val2,
    },
};
use tilt_ui_css::{
    AlignItems as CssAlignItems, AlignSelf as CssAlignSelf, BorderRadius as CssBorderRadius,
    CssColor, Display as CssDisplay, Edges, FlexDirection as CssFlexDirection,
    FlexWrap as CssFlexWrap, FontWeight as CssFontWeight, JustifyContent as CssJustifyContent,
    Length, Position,
};

pub(crate) fn val(value: Length) -> Val {
    match value {
        Length::Auto => Val::Auto,
        Length::Px(value) => Val::Px(value),
        Length::Percent(value) => Val::Percent(value),
        Length::Vw(value) => Val::Vw(value),
        Length::Vh(value) => Val::Vh(value),
    }
}
pub(crate) fn color(value: CssColor) -> Color {
    Color::srgba(value.red, value.green, value.blue, value.alpha)
}
pub(crate) fn rect(value: Edges<Length>) -> UiRect {
    UiRect {
        top: val(value.top),
        right: val(value.right),
        bottom: val(value.bottom),
        left: val(value.left),
    }
}
pub(crate) fn radius(value: CssBorderRadius) -> BorderRadius {
    BorderRadius::new(
        val(value.top_left),
        val(value.top_right),
        val(value.bottom_right),
        val(value.bottom_left),
    )
}
pub(crate) fn display(value: CssDisplay) -> Display {
    match value {
        CssDisplay::Block => Display::Block,
        CssDisplay::Flex => Display::Flex,
        CssDisplay::Grid => Display::Grid,
        CssDisplay::None => Display::None,
    }
}
pub(crate) fn position(value: Position) -> PositionType {
    match value {
        Position::Absolute => PositionType::Absolute,
        Position::Static | Position::Relative => PositionType::Relative,
    }
}
pub(crate) fn flex_direction(value: CssFlexDirection) -> FlexDirection {
    match value {
        CssFlexDirection::Row => FlexDirection::Row,
        CssFlexDirection::Column => FlexDirection::Column,
        CssFlexDirection::RowReverse => FlexDirection::RowReverse,
        CssFlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
    }
}
pub(crate) fn flex_wrap(value: CssFlexWrap) -> FlexWrap {
    match value {
        CssFlexWrap::NoWrap => FlexWrap::NoWrap,
        CssFlexWrap::Wrap => FlexWrap::Wrap,
        CssFlexWrap::WrapReverse => FlexWrap::WrapReverse,
    }
}
pub(crate) fn justify_content(value: CssJustifyContent) -> JustifyContent {
    match value {
        CssJustifyContent::Start => JustifyContent::Start,
        CssJustifyContent::End => JustifyContent::End,
        CssJustifyContent::Center => JustifyContent::Center,
        CssJustifyContent::SpaceBetween => JustifyContent::SpaceBetween,
        CssJustifyContent::SpaceAround => JustifyContent::SpaceAround,
        CssJustifyContent::SpaceEvenly => JustifyContent::SpaceEvenly,
    }
}
pub(crate) fn align_items(value: CssAlignItems) -> AlignItems {
    match value {
        CssAlignItems::Start => AlignItems::Start,
        CssAlignItems::End => AlignItems::End,
        CssAlignItems::Center => AlignItems::Center,
        CssAlignItems::Stretch => AlignItems::Stretch,
    }
}
pub(crate) fn align_self(value: CssAlignSelf) -> AlignSelf {
    match value {
        CssAlignSelf::Auto => AlignSelf::Auto,
        CssAlignSelf::Start => AlignSelf::Start,
        CssAlignSelf::End => AlignSelf::End,
        CssAlignSelf::Center => AlignSelf::Center,
        CssAlignSelf::Stretch => AlignSelf::Stretch,
    }
}
pub(crate) fn font_size(value: Length, inherited: Option<FontSize>) -> Option<FontSize> {
    Some(match value {
        Length::Px(value) => FontSize::Px(value),
        Length::Vw(value) => FontSize::Vw(value),
        Length::Vh(value) => FontSize::Vh(value),
        Length::Percent(value) => inherited? * (value / 100.0),
        Length::Auto => return None,
    })
}
pub(crate) fn font_weight(value: CssFontWeight) -> FontWeight {
    FontWeight(match value {
        CssFontWeight::Normal => 400,
        CssFontWeight::Bold => 700,
        CssFontWeight::Number(value) => value,
    })
}

pub(crate) fn transform(value: tilt_ui_css::CssTransform) -> UiTransform {
    UiTransform {
        translation: Val2::new(val(value.translate_x), val(value.translate_y)),
        scale: bevy::math::Vec2::new(value.scale_x, value.scale_y),
        rotation: bevy::math::Rot2::radians(value.rotation),
    }
}
