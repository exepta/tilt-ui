use std::{
    borrow::Borrow,
    fmt::{self, Write},
};

use cssparser::{CowRcStr, Parser, ToCss, serialize_identifier};
use precomputed_hash::PrecomputedHash;
use selectors::{
    Parser as SelectorParser, SelectorImpl, SelectorList,
    parser::{ParseRelative, SelectorParseErrorKind},
};

use super::{TiltUiPseudoClass, TiltUiPseudoElement};

/// Connects Servo selector parsing to TiltUI's element and UI-state vocabulary.
#[derive(Debug, Clone)]
pub struct TiltUiSelectorImpl;

/// Stores a parsed Servo selector list using TiltUI selector types.
pub type ParsedSelectorList = SelectorList<TiltUiSelectorImpl>;

/// Stores an owned identifier required by Servo's selector representation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SelectorName(String);

impl SelectorName {
    fn hash(&self) -> u32 {
        self.0.bytes().fold(2_166_136_261, |hash, byte| {
            hash.wrapping_mul(16_777_619) ^ u32::from(byte)
        })
    }
}

impl From<&str> for SelectorName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for SelectorName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Borrow<str> for SelectorName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SelectorAttributeValue {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PrecomputedHash for SelectorName {
    fn precomputed_hash(&self) -> u32 {
        self.hash()
    }
}

impl ToCss for SelectorName {
    fn to_css<W>(&self, destination: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        serialize_identifier(&self.0, destination)
    }
}

/// Stores an owned attribute value required by Servo's selector representation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SelectorAttributeValue(String);

impl From<&str> for SelectorAttributeValue {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl ToCss for SelectorAttributeValue {
    fn to_css<W>(&self, destination: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        destination.write_char('"')?;
        write!(cssparser::CssStringWriter::new(destination), "{}", self.0)?;
        destination.write_char('"')
    }
}

impl SelectorImpl for TiltUiSelectorImpl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = SelectorAttributeValue;
    type Identifier = SelectorName;
    type LocalName = SelectorName;
    type NamespaceUrl = SelectorName;
    type NamespacePrefix = SelectorName;
    type BorrowedNamespaceUrl = str;
    type BorrowedLocalName = str;
    type NonTSPseudoClass = TiltUiPseudoClass;
    type PseudoElement = TiltUiPseudoElement;
}

pub(crate) struct SelectorParseFailure(String);

impl fmt::Debug for SelectorParseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SelectorParseFailure")
            .field(&self.0)
            .finish()
    }
}

impl<'i> From<SelectorParseErrorKind<'i>> for SelectorParseFailure {
    fn from(error: SelectorParseErrorKind<'i>) -> Self {
        Self(format!("{error:?}"))
    }
}

struct TiltUiSelectorParser;

impl<'i> SelectorParser<'i> for TiltUiSelectorParser {
    type Impl = TiltUiSelectorImpl;
    type Error = SelectorParseFailure;

    fn parse_non_ts_pseudo_class(
        &self,
        location: cssparser::SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<TiltUiPseudoClass, cssparser::ParseError<'i, Self::Error>> {
        let pseudo = if name.eq_ignore_ascii_case("hover") {
            TiltUiPseudoClass::Hover
        } else if name.eq_ignore_ascii_case("active") {
            TiltUiPseudoClass::Active
        } else if name.eq_ignore_ascii_case("focus") {
            TiltUiPseudoClass::Focus
        } else if name.eq_ignore_ascii_case("disabled") {
            TiltUiPseudoClass::Disabled
        } else if name.eq_ignore_ascii_case("checked") {
            TiltUiPseudoClass::Checked
        } else if name.eq_ignore_ascii_case("readonly") {
            TiltUiPseudoClass::Readonly
        } else if name.eq_ignore_ascii_case("invalid") {
            TiltUiPseudoClass::Invalid
        } else if name.eq_ignore_ascii_case("open") {
            TiltUiPseudoClass::Open
        } else {
            return Err(location.new_custom_error(SelectorParseFailure(format!(
                "unsupported pseudo-class :{name}"
            ))));
        };
        Ok(pseudo)
    }

    fn parse_pseudo_element(
        &self,
        location: cssparser::SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<TiltUiPseudoElement, cssparser::ParseError<'i, Self::Error>> {
        let pseudo = if name.eq_ignore_ascii_case("indicator") {
            TiltUiPseudoElement::Indicator
        } else if name.eq_ignore_ascii_case("mark") {
            TiltUiPseudoElement::Mark
        } else if name.eq_ignore_ascii_case("track") {
            TiltUiPseudoElement::Track
        } else if name.eq_ignore_ascii_case("thumb") {
            TiltUiPseudoElement::Thumb
        } else if name.eq_ignore_ascii_case("fill") {
            TiltUiPseudoElement::Fill
        } else if name.eq_ignore_ascii_case("value") {
            TiltUiPseudoElement::Value
        } else if name.eq_ignore_ascii_case("placeholder") {
            TiltUiPseudoElement::Placeholder
        } else if name.eq_ignore_ascii_case("cursor") {
            TiltUiPseudoElement::Cursor
        } else if name.eq_ignore_ascii_case("selection") {
            TiltUiPseudoElement::Selection
        } else if name.eq_ignore_ascii_case("dot") {
            TiltUiPseudoElement::Dot
        } else if name.eq_ignore_ascii_case("label") {
            TiltUiPseudoElement::Label
        } else if name.eq_ignore_ascii_case("tooltip") {
            TiltUiPseudoElement::Tooltip
        } else if name.eq_ignore_ascii_case("resize-handle") {
            TiltUiPseudoElement::ResizeHandle
        } else if name.eq_ignore_ascii_case("scrollbar-y-track") {
            TiltUiPseudoElement::ScrollbarYTrack
        } else if name.eq_ignore_ascii_case("scrollbar-y-thumb") {
            TiltUiPseudoElement::ScrollbarYThumb
        } else if name.eq_ignore_ascii_case("scrollbar-x-track") {
            TiltUiPseudoElement::ScrollbarXTrack
        } else if name.eq_ignore_ascii_case("scrollbar-x-thumb") {
            TiltUiPseudoElement::ScrollbarXThumb
        } else if name.eq_ignore_ascii_case("popup") {
            TiltUiPseudoElement::Popup
        } else if name.eq_ignore_ascii_case("calendar") {
            TiltUiPseudoElement::Calendar
        } else if name.eq_ignore_ascii_case("calendar-header") {
            TiltUiPseudoElement::CalendarHeader
        } else if name.eq_ignore_ascii_case("calendar-previous") {
            TiltUiPseudoElement::CalendarPrevious
        } else if name.eq_ignore_ascii_case("calendar-next") {
            TiltUiPseudoElement::CalendarNext
        } else if name.eq_ignore_ascii_case("day") {
            TiltUiPseudoElement::CalendarDay
        } else if name.eq_ignore_ascii_case("hovered-day") {
            TiltUiPseudoElement::HoveredCalendarDay
        } else if name.eq_ignore_ascii_case("selected-day") {
            TiltUiPseudoElement::SelectedCalendarDay
        } else if name.eq_ignore_ascii_case("disabled-day") {
            TiltUiPseudoElement::DisabledCalendarDay
        } else if name.eq_ignore_ascii_case("preview") {
            TiltUiPseudoElement::Preview
        } else if name.eq_ignore_ascii_case("swatch") {
            TiltUiPseudoElement::Swatch
        } else if name.eq_ignore_ascii_case("canvas") {
            TiltUiPseudoElement::ColorCanvas
        } else if name.eq_ignore_ascii_case("canvas-thumb") {
            TiltUiPseudoElement::ColorCanvasThumb
        } else if name.eq_ignore_ascii_case("hue-track") {
            TiltUiPseudoElement::HueTrack
        } else if name.eq_ignore_ascii_case("hue-thumb") {
            TiltUiPseudoElement::HueThumb
        } else if name.eq_ignore_ascii_case("alpha-track") {
            TiltUiPseudoElement::AlphaTrack
        } else if name.eq_ignore_ascii_case("alpha-thumb") {
            TiltUiPseudoElement::AlphaThumb
        } else if name.eq_ignore_ascii_case("format") {
            TiltUiPseudoElement::ColorFormat
        } else if name.eq_ignore_ascii_case("formats") {
            TiltUiPseudoElement::ColorFormats
        } else if name.eq_ignore_ascii_case("swatches") {
            TiltUiPseudoElement::ColorSwatches
        } else if name.eq_ignore_ascii_case("selected-format") {
            TiltUiPseudoElement::SelectedColorFormat
        } else if name.eq_ignore_ascii_case("recent-colors") {
            TiltUiPseudoElement::RecentColors
        } else if name.eq_ignore_ascii_case("recent-color") {
            TiltUiPseudoElement::RecentColor
        } else {
            return Err(location.new_custom_error(SelectorParseFailure(format!(
                "unsupported pseudo-element ::{name}"
            ))));
        };
        Ok(pseudo)
    }
}

/// Parses a selector list using the Servo selector representation.
pub(crate) fn parse_selector_list<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<ParsedSelectorList, cssparser::ParseError<'i, SelectorParseFailure>> {
    let selector_parser = TiltUiSelectorParser;
    SelectorList::parse(&selector_parser, input, ParseRelative::No)
}
