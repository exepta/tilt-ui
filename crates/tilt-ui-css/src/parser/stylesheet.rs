use crate::{
    AnimationName, Keyframe, KeyframesRule, Length, MediaComparison, MediaCondition,
    MediaOrientation, MediaQuery, MediaRule, MediaType, Specificity, StyleDeclaration, StyleRule,
    StyleSheet,
};
use cssparser::{
    AtRuleParser, DeclarationParser, ParseErrorKind, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};

use super::{StyleParseError, declaration::parse_declaration};

/// Parses a TiltUI component stylesheet into its typed core representation.
///
/// The parser operates on source text and does not access files, assets, or
/// runtime style state.
pub fn parse_stylesheet(source: &str) -> Result<StyleSheet, StyleParseError> {
    let mut input = ParserInput::new(source);
    let mut input = Parser::new(&mut input);
    let mut parser = StyleRuleParser { source_order: 0 };
    let mut rules = Vec::new();
    let mut media_rules = Vec::new();
    let mut keyframes = Vec::new();

    for rule in StyleSheetParser::new(&mut input, &mut parser) {
        match rule.map_err(style_error_from_rule)? {
            SheetEntry::Rule(rule) => rules.push(rule),
            SheetEntry::Media(rule) => media_rules.push(rule),
            SheetEntry::Keyframes(rule) => keyframes.push(rule),
        }
    }

    Ok(StyleSheet {
        rules,
        media_rules,
        keyframes,
    })
}

struct StyleRuleParser {
    source_order: usize,
}

enum SheetEntry {
    Rule(StyleRule),
    Media(MediaRule),
    Keyframes(KeyframesRule),
}

enum AtPrelude {
    Media(Vec<(MediaType, MediaCondition)>),
    Keyframes(AnimationName),
}

impl<'i> AtRuleParser<'i> for StyleRuleParser {
    type Prelude = AtPrelude;
    type AtRule = SheetEntry;
    type Error = StyleParseError;

    fn parse_prelude<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        if name.eq_ignore_ascii_case("media") {
            return parse_media_list(input)
                .map(AtPrelude::Media)
                .map_err(|error| input.new_custom_error(error));
        }
        if name.eq_ignore_ascii_case("keyframes") {
            let name = input.expect_ident()?.to_string();
            input.expect_exhausted()?;
            return Ok(AtPrelude::Keyframes(AnimationName(name)));
        }
        Err(input.new_custom_error(StyleParseError::CssSyntax(format!(
            "unsupported at-rule @{name}"
        ))))
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, cssparser::ParseError<'i, Self::Error>> {
        match prelude {
            AtPrelude::Media(queries) => {
                let mut nested = NestedRuleParser {
                    source_order: &mut self.source_order,
                };
                let mut rules = Vec::new();
                for rule in RuleBodyParser::new(input, &mut nested) {
                    rules.push(rule.map_err(|(error, _)| error)?);
                }
                Ok(SheetEntry::Media(MediaRule {
                    queries: queries
                        .into_iter()
                        .map(|(media_type, condition)| MediaQuery {
                            media_type,
                            condition,
                        })
                        .collect(),
                    rules,
                }))
            }
            AtPrelude::Keyframes(name) => Ok(SheetEntry::Keyframes(parse_keyframes(name, input)?)),
        }
    }
}

impl<'i> QualifiedRuleParser<'i> for StyleRuleParser {
    type Prelude = crate::ParsedSelectorList;
    type QualifiedRule = SheetEntry;
    type Error = StyleParseError;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        crate::selector::parse_selector_list(input).map_err(|error| {
            input.new_custom_error(StyleParseError::InvalidSelector(format!("{error:?}")))
        })
    }

    fn parse_block<'t>(
        &mut self,
        selectors: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        parse_style_rule(selectors, &mut self.source_order, input).map(SheetEntry::Rule)
    }
}

struct NestedRuleParser<'a> {
    source_order: &'a mut usize,
}

impl<'i> AtRuleParser<'i> for NestedRuleParser<'_> {
    type Prelude = ();
    type AtRule = StyleRule;
    type Error = StyleParseError;
}

impl<'i> QualifiedRuleParser<'i> for NestedRuleParser<'_> {
    type Prelude = crate::ParsedSelectorList;
    type QualifiedRule = StyleRule;
    type Error = StyleParseError;
    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        crate::selector::parse_selector_list(input).map_err(|error| {
            input.new_custom_error(StyleParseError::InvalidSelector(format!("{error:?}")))
        })
    }
    fn parse_block<'t>(
        &mut self,
        selectors: Self::Prelude,
        _: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        parse_style_rule(selectors, self.source_order, input)
    }
}

fn parse_style_rule<'i, 't>(
    selectors: crate::ParsedSelectorList,
    source_order: &mut usize,
    input: &mut Parser<'i, 't>,
) -> Result<StyleRule, cssparser::ParseError<'i, StyleParseError>> {
    let mut declaration_parser = DeclarationParserAdapter;
    let mut declarations = Vec::new();
    for declaration in RuleBodyParser::new(input, &mut declaration_parser) {
        declarations.push(declaration.map_err(|(error, _)| error)?);
    }
    let specificity = selectors
        .slice()
        .iter()
        .map(|selector| Specificity(selector.specificity()))
        .collect();
    let order = *source_order;
    *source_order += 1;
    Ok(StyleRule {
        selectors,
        specificity,
        declarations,
        source_order: order,
    })
}

fn parse_media_list(
    input: &mut Parser<'_, '_>,
) -> Result<Vec<(MediaType, MediaCondition)>, StyleParseError> {
    input
        .parse_comma_separated(parse_media_query)
        .map_err(|_| StyleParseError::CssSyntax("invalid @media query".into()))
}

fn parse_media_query<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<(MediaType, MediaCondition), cssparser::ParseError<'i, StyleParseError>> {
    let negated = input
        .try_parse(|input| input.expect_ident_matching("not"))
        .is_ok();
    let media_type = input
        .try_parse(|input| input.expect_ident().map(|name| name.to_string()))
        .ok()
        .map_or(MediaType::All, |name| {
            match name.to_ascii_lowercase().as_str() {
                "all" => MediaType::All,
                "screen" => MediaType::Screen,
                _ => MediaType::Unsupported,
            }
        });
    let mut condition = None;
    while !input.is_exhausted() {
        let combine_with_and = if condition.is_some() {
            if input
                .try_parse(|input| input.expect_ident_matching("and"))
                .is_ok()
            {
                true
            } else if input
                .try_parse(|input| input.expect_ident_matching("or"))
                .is_ok()
            {
                false
            } else {
                return Err(input.new_custom_error(StyleParseError::CssSyntax(
                    "expected `and` or `or` between media features".into(),
                )));
            }
        } else {
            input
                .try_parse(|input| input.expect_ident_matching("and"))
                .ok();
            true
        };
        let token = input.next()?.clone();
        if !matches!(token, cssparser::Token::ParenthesisBlock) {
            return Err(input.new_custom_error(StyleParseError::CssSyntax(
                "expected parenthesized media feature".into(),
            )));
        }
        let next = input.parse_nested_block(parse_media_feature)?;
        condition = Some(match condition {
            None => next,
            Some(previous) if combine_with_and => MediaCondition::And(vec![previous, next]),
            Some(previous) => MediaCondition::Or(vec![previous, next]),
        });
    }
    let condition = condition.unwrap_or(MediaCondition::True);
    Ok((
        media_type,
        if negated {
            MediaCondition::Not(Box::new(condition))
        } else {
            condition
        },
    ))
}

fn parse_media_feature<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<MediaCondition, cssparser::ParseError<'i, StyleParseError>> {
    let first = input.next()?.clone();
    match first {
        cssparser::Token::Ident(name) if name.eq_ignore_ascii_case("orientation") => {
            input.expect_colon()?;
            match input.expect_ident()?.to_ascii_lowercase().as_str() {
                "portrait" => Ok(MediaCondition::Orientation(MediaOrientation::Portrait)),
                "landscape" => Ok(MediaCondition::Orientation(MediaOrientation::Landscape)),
                _ => Err(input.new_custom_error(StyleParseError::CssSyntax(
                    "unsupported orientation".into(),
                ))),
            }
        }
        cssparser::Token::Ident(name) => parse_named_media_feature(input, &name),
        token => {
            let lower = parse_media_length_token(token, input)?;
            let operator = parse_media_operator(input)?;
            let dimension = input.expect_ident()?.to_string();
            let second_operator = parse_media_operator(input)?;
            let upper = parse_media_length(input)?;
            let lower_condition = dimension_condition(&dimension, invert_operator(operator), lower)
                .map_err(|error| input.new_custom_error(error))?;
            let upper_condition = dimension_condition(&dimension, second_operator, upper)
                .map_err(|error| input.new_custom_error(error))?;
            Ok(MediaCondition::And(vec![lower_condition, upper_condition]))
        }
    }
}

fn parse_named_media_feature<'i, 't>(
    input: &mut Parser<'i, 't>,
    name: &str,
) -> Result<MediaCondition, cssparser::ParseError<'i, StyleParseError>> {
    let (dimension, comparison) = match name.to_ascii_lowercase().as_str() {
        "min-width" => ("width", MediaComparisonKind::GreaterOrEqual),
        "max-width" => ("width", MediaComparisonKind::LessOrEqual),
        "min-height" => ("height", MediaComparisonKind::GreaterOrEqual),
        "max-height" => ("height", MediaComparisonKind::LessOrEqual),
        "width" | "height" => {
            let operator = if input.try_parse(|input| input.expect_colon()).is_ok() {
                MediaComparisonKind::Equal
            } else {
                parse_media_operator(input)?
            };
            let length = parse_media_length(input)?;
            return dimension_condition(name, operator, length)
                .map_err(|error| input.new_custom_error(error));
        }
        _ => return Ok(MediaCondition::False),
    };
    input.expect_colon()?;
    dimension_condition(dimension, comparison, parse_media_length(input)?)
        .map_err(|error| input.new_custom_error(error))
}

#[derive(Clone, Copy)]
enum MediaComparisonKind {
    Equal,
    GreaterOrEqual,
    GreaterThan,
    LessOrEqual,
    LessThan,
}

fn parse_media_operator<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<MediaComparisonKind, cssparser::ParseError<'i, StyleParseError>> {
    match input.next()?.clone() {
        cssparser::Token::Delim('>') => {
            if input.try_parse(|input| input.expect_delim('=')).is_ok() {
                Ok(MediaComparisonKind::GreaterOrEqual)
            } else {
                Ok(MediaComparisonKind::GreaterThan)
            }
        }
        cssparser::Token::Delim('<') => {
            if input.try_parse(|input| input.expect_delim('=')).is_ok() {
                Ok(MediaComparisonKind::LessOrEqual)
            } else {
                Ok(MediaComparisonKind::LessThan)
            }
        }
        _ => Err(input.new_custom_error(StyleParseError::CssSyntax(
            "expected media comparison operator".into(),
        ))),
    }
}

fn invert_operator(operator: MediaComparisonKind) -> MediaComparisonKind {
    match operator {
        MediaComparisonKind::GreaterOrEqual => MediaComparisonKind::LessOrEqual,
        MediaComparisonKind::GreaterThan => MediaComparisonKind::LessThan,
        MediaComparisonKind::LessOrEqual => MediaComparisonKind::GreaterOrEqual,
        MediaComparisonKind::LessThan => MediaComparisonKind::GreaterThan,
        MediaComparisonKind::Equal => MediaComparisonKind::Equal,
    }
}

fn dimension_condition(
    dimension: &str,
    operator: MediaComparisonKind,
    length: Length,
) -> Result<MediaCondition, StyleParseError> {
    let comparison = match operator {
        MediaComparisonKind::Equal => MediaComparison::Equal(length),
        MediaComparisonKind::GreaterOrEqual => MediaComparison::GreaterOrEqual(length),
        MediaComparisonKind::GreaterThan => MediaComparison::GreaterThan(length),
        MediaComparisonKind::LessOrEqual => MediaComparison::LessOrEqual(length),
        MediaComparisonKind::LessThan => MediaComparison::LessThan(length),
    };
    match dimension.to_ascii_lowercase().as_str() {
        "width" => Ok(MediaCondition::Width(comparison)),
        "height" => Ok(MediaCondition::Height(comparison)),
        _ => Err(StyleParseError::CssSyntax(
            "expected width or height".into(),
        )),
    }
}

fn parse_media_length<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<Length, cssparser::ParseError<'i, StyleParseError>> {
    let token = input.next()?.clone();
    parse_media_length_token(token, input)
}

fn parse_media_length_token<'i, 't>(
    token: cssparser::Token<'i>,
    input: &mut Parser<'i, 't>,
) -> Result<Length, cssparser::ParseError<'i, StyleParseError>> {
    match token {
        cssparser::Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => {
            Ok(Length::Px(value))
        }
        cssparser::Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("vw") => {
            Ok(Length::Vw(value))
        }
        cssparser::Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("vh") => {
            Ok(Length::Vh(value))
        }
        _ => Err(input.new_custom_error(StyleParseError::InvalidLength(
            "media queries require px, vw, or vh".into(),
        ))),
    }
}

fn parse_keyframes<'i, 't>(
    name: AnimationName,
    input: &mut Parser<'i, 't>,
) -> Result<KeyframesRule, cssparser::ParseError<'i, StyleParseError>> {
    let mut parser = KeyframeParser { source_order: 0 };
    let mut frames = Vec::new();
    for frame in RuleBodyParser::new(input, &mut parser) {
        let (offsets, declarations, order) = frame.map_err(|(error, _)| error)?;
        frames.extend(offsets.into_iter().map(|offset| Keyframe {
            offset,
            declarations: declarations.clone(),
            source_order: order,
        }));
    }
    frames.sort_by(|left, right| {
        left.offset
            .total_cmp(&right.offset)
            .then(left.source_order.cmp(&right.source_order))
    });
    Ok(KeyframesRule { name, frames })
}

struct KeyframeParser {
    source_order: usize,
}
impl<'i> AtRuleParser<'i> for KeyframeParser {
    type Prelude = ();
    type AtRule = (Vec<f32>, Vec<StyleDeclaration>, usize);
    type Error = StyleParseError;
}
impl<'i> QualifiedRuleParser<'i> for KeyframeParser {
    type Prelude = Vec<f32>;
    type QualifiedRule = (Vec<f32>, Vec<StyleDeclaration>, usize);
    type Error = StyleParseError;
    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Vec<f32>, cssparser::ParseError<'i, StyleParseError>> {
        input.parse_comma_separated(|input| match input.next()?.clone() {
            cssparser::Token::Ident(value) if value.eq_ignore_ascii_case("from") => Ok(0.0),
            cssparser::Token::Ident(value) if value.eq_ignore_ascii_case("to") => Ok(1.0),
            cssparser::Token::Percentage { unit_value, .. }
                if (0.0..=1.0).contains(&unit_value) =>
            {
                Ok(unit_value)
            }
            _ => Err(input.new_custom_error(StyleParseError::CssSyntax(
                "keyframe offsets must be from, to, or percentages".into(),
            ))),
        })
    }
    fn parse_block<'t>(
        &mut self,
        offsets: Vec<f32>,
        _: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, StyleParseError>> {
        let mut declarations = Vec::new();
        let mut parser = DeclarationParserAdapter;
        for declaration in RuleBodyParser::new(input, &mut parser) {
            declarations.push(declaration.map_err(|(error, _)| error)?);
        }
        let order = self.source_order;
        self.source_order += 1;
        Ok((offsets, declarations, order))
    }
}

impl<'i> RuleBodyItemParser<'i, StyleRule, StyleParseError> for NestedRuleParser<'_> {
    fn parse_declarations(&self) -> bool {
        false
    }
    fn parse_qualified(&self) -> bool {
        true
    }
}

impl<'i> DeclarationParser<'i> for NestedRuleParser<'_> {
    type Declaration = StyleRule;
    type Error = StyleParseError;

    fn parse_value<'t>(
        &mut self,
        _name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        Err(input.new_custom_error(StyleParseError::CssSyntax(
            "declarations are not valid directly inside @media".into(),
        )))
    }
}

impl<'i> RuleBodyItemParser<'i, (Vec<f32>, Vec<StyleDeclaration>, usize), StyleParseError>
    for KeyframeParser
{
    fn parse_declarations(&self) -> bool {
        false
    }
    fn parse_qualified(&self) -> bool {
        true
    }
}

impl<'i> DeclarationParser<'i> for KeyframeParser {
    type Declaration = (Vec<f32>, Vec<StyleDeclaration>, usize);
    type Error = StyleParseError;

    fn parse_value<'t>(
        &mut self,
        _name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        Err(input.new_custom_error(StyleParseError::CssSyntax(
            "declarations require a keyframe selector".into(),
        )))
    }
}

struct DeclarationParserAdapter;

impl<'i> DeclarationParser<'i> for DeclarationParserAdapter {
    type Declaration = StyleDeclaration;
    type Error = StyleParseError;

    fn parse_value<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        let property = name.to_ascii_lowercase();
        parse_declaration(&property, input).map_err(|error| input.new_custom_error(error))
    }
}

impl<'i> AtRuleParser<'i> for DeclarationParserAdapter {
    type Prelude = ();
    type AtRule = StyleDeclaration;
    type Error = StyleParseError;
}

impl<'i> QualifiedRuleParser<'i> for DeclarationParserAdapter {
    type Prelude = ();
    type QualifiedRule = StyleDeclaration;
    type Error = StyleParseError;
}

impl<'i> RuleBodyItemParser<'i, StyleDeclaration, StyleParseError> for DeclarationParserAdapter {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

fn style_error_from_rule(
    (error, source): (cssparser::ParseError<'_, StyleParseError>, &str),
) -> StyleParseError {
    match error.kind {
        ParseErrorKind::Custom(error) => error,
        _ => StyleParseError::CssSyntax(source.trim().to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CssColor, CssOverflow, Display, Edges, FlexDirection, FontFamily, GradientDirection,
        Length, MediaEnvironment, StyleDeclaration, StyleParseError,
    };

    use super::parse_stylesheet;

    #[test]
    fn shipped_theme_and_showcase_stylesheets_parse() {
        parse_stylesheet(include_str!(
            "../../../tilt-ui-runtime/src/theme/default.css"
        ))
        .expect("default theme CSS");
        parse_stylesheet(include_str!(
            "../../../../examples/component-showcase/src-ui/pages/showcase.component.css"
        ))
        .expect("showcase CSS");
    }

    #[test]
    fn parses_linear_gradient_background_images() {
        let stylesheet = parse_stylesheet(
            "button { background-image: linear-gradient(to right, #D34CED, #8424F5); }",
        )
        .unwrap();
        let StyleDeclaration::BackgroundImage(Some(gradient)) =
            &stylesheet.rules()[0].declarations[0]
        else {
            panic!("expected a typed linear gradient");
        };
        assert_eq!(gradient.direction, GradientDirection::Right);
        assert_eq!(gradient.stops.len(), 2);
        assert!(gradient.stops[0].red > gradient.stops[1].red);
        assert!(parse_stylesheet("button { background-image: linear-gradient(red); }").is_err());
    }

    #[test]
    fn parses_an_element_rule_with_typed_length() {
        let stylesheet = parse_stylesheet("button { width: 200px; }").unwrap();

        assert_eq!(stylesheet.rules().len(), 1);
        assert_eq!(stylesheet.rules()[0].selectors.len(), 1);
        assert_eq!(stylesheet.rules()[0].specificity[0].type_count(), 1);
        assert_eq!(
            stylesheet.rules()[0].declarations,
            [StyleDeclaration::Width(Length::Px(200.0))]
        );
    }

    #[test]
    fn parses_supported_font_families() {
        let stylesheet = parse_stylesheet(
            "* { font-family: sans-serif; } code { font-family: monospace; } checkbox::mark { font-family: ui-symbols; }",
        )
        .unwrap();
        assert_eq!(
            stylesheet.rules()[0].declarations,
            [StyleDeclaration::FontFamily(FontFamily::SansSerif)]
        );
        assert_eq!(
            stylesheet.rules()[1].declarations,
            [StyleDeclaration::FontFamily(FontFamily::Monospace)]
        );
        assert_eq!(
            stylesheet.rules()[2].declarations,
            [StyleDeclaration::FontFamily(FontFamily::UiSymbols)]
        );
    }

    #[test]
    fn parses_overflow_axes_and_scrollbar_parts() {
        let stylesheet = parse_stylesheet(
            "div { overflow: auto hidden; overflow-y: scroll; } textarea::scrollbar-y-thumb { background-color: #aa33ee; } *::scrollbar-x-track { height: 7px; }",
        )
        .unwrap();
        assert_eq!(
            stylesheet.rules()[0].declarations,
            [
                StyleDeclaration::Overflow(CssOverflow::Auto, CssOverflow::Hidden),
                StyleDeclaration::OverflowY(CssOverflow::Scroll),
            ]
        );
        assert_eq!(stylesheet.rules().len(), 3);
        assert!(parse_stylesheet("div { overflow: banana; }").is_err());
    }

    #[test]
    fn parses_class_id_compound_and_combinator_selectors() {
        let stylesheet = parse_stylesheet(
            ".menu { display: flex; } #play { width: 50%; } button.primary { gap: 2px; } .menu button { height: 1px; } .menu > button { width: 1px; }",
        )
        .unwrap();

        assert_eq!(stylesheet.rules().len(), 5);
        assert_eq!(stylesheet.rules()[0].specificity[0].class_count(), 1);
        assert_eq!(stylesheet.rules()[1].specificity[0].id_count(), 1);
        assert_eq!(stylesheet.rules()[2].specificity[0].class_count(), 1);
        assert_eq!(stylesheet.rules()[2].specificity[0].type_count(), 1);
        assert_eq!(stylesheet.rules()[3].specificity[0].class_count(), 1);
        assert_eq!(stylesheet.rules()[4].selectors.len(), 1);
    }

    #[test]
    fn parses_multiple_selectors_rules_and_pseudo_classes() {
        let stylesheet = parse_stylesheet(
            "button, .primary { color: #fff; } button:hover { opacity: 0.9; } button:active { width: 90px; } input:focus { opacity: 1; } input:disabled { opacity: 0.5; } input:checked { opacity: 1; }",
        )
        .unwrap();

        assert_eq!(stylesheet.rules().len(), 6);
        assert_eq!(stylesheet.rules()[0].selectors.len(), 2);
        assert_eq!(stylesheet.rules()[1].specificity[0].class_count(), 1);
        assert_eq!(stylesheet.rules()[1].source_order, 1);
        assert_eq!(stylesheet.rules()[5].source_order, 5);
    }

    #[test]
    fn parses_editable_pseudo_states() {
        let stylesheet = parse_stylesheet(
            "input:readonly { color: #777777; } textarea:invalid { border-color: #ff0000; }",
        )
        .unwrap();
        assert_eq!(stylesheet.rules().len(), 2);
        assert_eq!(stylesheet.rules()[0].specificity[0].class_count(), 1);
        assert_eq!(stylesheet.rules()[1].specificity[0].class_count(), 1);
    }

    #[test]
    fn parses_flex_layout_and_multiple_declarations() {
        let stylesheet = parse_stylesheet(
            ".menu { display: flex; flex-direction: column; gap: 12px; width: 320px; padding: 16px; }",
        )
        .unwrap();

        assert_eq!(
            stylesheet.rules()[0].declarations,
            [
                StyleDeclaration::Display(Display::Flex),
                StyleDeclaration::FlexDirection(FlexDirection::Column),
                StyleDeclaration::Gap(Length::Px(12.0)),
                StyleDeclaration::Width(Length::Px(320.0)),
                StyleDeclaration::Padding(Edges::all(Length::Px(16.0))),
            ]
        );
    }

    #[test]
    fn parses_all_initial_length_units() {
        let stylesheet = parse_stylesheet(
            "div { width: auto; height: 10px; min-width: 50%; min-height: 20vw; max-height: 100vh; }",
        )
        .unwrap();

        assert_eq!(
            stylesheet.rules()[0].declarations,
            [
                StyleDeclaration::Width(Length::Auto),
                StyleDeclaration::Height(Length::Px(10.0)),
                StyleDeclaration::MinWidth(Length::Percent(50.0)),
                StyleDeclaration::MinHeight(Length::Vw(20.0)),
                StyleDeclaration::MaxHeight(Length::Vh(100.0)),
            ]
        );
    }

    #[test]
    fn parses_supported_color_forms() {
        let stylesheet = parse_stylesheet(
            "div { color: #fff; background-color: #112233; border-color: #ffffffff; } p { color: rgb(255, 128, 0); background-color: rgba(0, 0, 255, 0.5); border-color: transparent; }",
        )
        .unwrap();

        assert_eq!(
            stylesheet.rules()[0].declarations,
            [
                StyleDeclaration::Color(CssColor::rgba(1.0, 1.0, 1.0, 1.0)),
                StyleDeclaration::BackgroundColor(CssColor::rgba(
                    17.0 / 255.0,
                    34.0 / 255.0,
                    51.0 / 255.0,
                    1.0,
                )),
                StyleDeclaration::BorderColor(CssColor::rgba(1.0, 1.0, 1.0, 1.0)),
            ]
        );
        assert_eq!(
            stylesheet.rules()[1].declarations[2],
            StyleDeclaration::BorderColor(CssColor::transparent())
        );
    }

    #[test]
    fn expands_edge_shorthands() {
        let stylesheet = parse_stylesheet(
            "div { margin: 1px 2px 3px 4px; padding: 5px 6px; border-width: 7px; }",
        )
        .unwrap();

        assert_eq!(
            stylesheet.rules()[0].declarations,
            [
                StyleDeclaration::Margin(Edges {
                    top: Length::Px(1.0),
                    right: Length::Px(2.0),
                    bottom: Length::Px(3.0),
                    left: Length::Px(4.0),
                }),
                StyleDeclaration::Padding(Edges {
                    top: Length::Px(5.0),
                    right: Length::Px(6.0),
                    bottom: Length::Px(5.0),
                    left: Length::Px(6.0),
                }),
                StyleDeclaration::BorderWidth(Edges::all(Length::Px(7.0))),
            ]
        );
    }

    #[test]
    fn parses_the_representative_component_stylesheet() {
        let stylesheet = parse_stylesheet(
            r#"
                .menu {
                    display: flex;
                    flex-direction: column;
                    gap: 12px;
                    width: 320px;
                    padding: 16px;
                }
                .menu > button { width: 100%; height: 48px; }
                button.primary { background-color: #4285f4; color: #ffffff; border-radius: 6px; }
                button.primary:hover { opacity: 0.9; }
            "#,
        )
        .unwrap();

        assert_eq!(stylesheet.rules().len(), 4);
        assert!(matches!(
            stylesheet.rules()[2].declarations[0],
            StyleDeclaration::BackgroundColor(_)
        ));
        assert_eq!(stylesheet.rules()[3].specificity[0].class_count(), 2);
    }

    #[test]
    fn parses_control_part_pseudo_elements() {
        let stylesheet = parse_stylesheet(
            "checkbox::indicator { width: 20px; } checkbox:checked::mark { background-color: #ffffff; } switch-button::thumb { left: 2px; }",
        )
        .unwrap();

        assert_eq!(stylesheet.rules().len(), 3);
    }

    #[test]
    fn parses_and_evaluates_typed_media_conditions() {
        let stylesheet = parse_stylesheet(
            "@media screen and (width >= 600px) and (height < 900px) { .card { width: 200px; } } @media (600px < width < 900px), (orientation: portrait) { .compact { width: 100px; } } @media not (max-width: 600px) { .wide { width: 300px; } }",
        )
        .unwrap();
        assert_eq!(stylesheet.media_rules().len(), 3);
        assert!(stylesheet.media_rules()[0].matches(MediaEnvironment {
            width: 800.0,
            height: 700.0,
        }));
        assert!(!stylesheet.media_rules()[0].matches(MediaEnvironment {
            width: 500.0,
            height: 700.0,
        }));
        assert!(stylesheet.media_rules()[1].matches(MediaEnvironment {
            width: 800.0,
            height: 1000.0,
        }));
        assert!(stylesheet.media_rules()[2].matches(MediaEnvironment {
            width: 601.0,
            height: 600.0,
        }));
    }

    #[test]
    fn parses_keyframes_animation_transition_and_transform() {
        let stylesheet = parse_stylesheet(
            "@keyframes pulse { from { transform: scale(1); background-color: #000000; } 50%, 75% { transform: scale(1.05); } to { transform: scale(1); background-color: #ffffff; } } .pulse { animation: pulse 1.2s ease-in-out 100ms infinite alternate; transition: background-color 150ms ease-out, transform 200ms linear; }",
        )
        .unwrap();
        assert_eq!(stylesheet.keyframes().len(), 1);
        assert_eq!(stylesheet.keyframes()[0].frames.len(), 4);
        assert_eq!(stylesheet.rules().len(), 1);
        assert!(matches!(
            stylesheet.rules()[0].declarations[0],
            StyleDeclaration::Animation(_)
        ));
        assert!(matches!(
            stylesheet.rules()[0].declarations[1],
            StyleDeclaration::Transition(_)
        ));
    }

    #[test]
    fn parses_transition_none_and_longhands() {
        let stylesheet = parse_stylesheet(
            ".card { transition-property: background-color, border-radius; transition-duration: 150ms, 0.2s; transition-delay: 0ms, 50ms; transition-timing-function: ease-out, linear; } .plain { transition: none; } .disabled { transition-property: none; }",
        )
        .unwrap();
        assert!(matches!(
            stylesheet.rules()[0].declarations[0],
            StyleDeclaration::TransitionProperty(_)
        ));
        assert!(matches!(
            stylesheet.rules()[0].declarations[1],
            StyleDeclaration::TransitionDuration(_)
        ));
        assert_eq!(
            stylesheet.rules()[1].declarations,
            [StyleDeclaration::Transition(Vec::new())]
        );
        assert!(matches!(
            stylesheet.rules()[2].declarations[0],
            StyleDeclaration::TransitionProperty(_)
        ));
    }

    #[test]
    fn rejects_malformed_css_invalid_values_and_unsupported_properties() {
        let malformed = parse_stylesheet("button { width 20px; }");
        assert!(
            matches!(malformed, Err(StyleParseError::CssSyntax(_))),
            "{malformed:?}"
        );
        assert!(matches!(
            parse_stylesheet("button { width: 2em; }"),
            Err(StyleParseError::InvalidLength(_))
        ));
        assert!(matches!(
            parse_stylesheet("button { box-shadow: 1px 1px; }"),
            Err(StyleParseError::UnsupportedProperty(property)) if property == "box-shadow"
        ));
    }
}
