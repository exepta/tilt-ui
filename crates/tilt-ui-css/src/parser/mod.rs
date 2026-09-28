mod declaration;
mod error;
mod grid;
mod nesting;
mod stylesheet;
mod value;

pub use declaration::parse_declaration_value;
pub use error::StyleParseError;
pub use stylesheet::parse_stylesheet;

/// Parses one CSS color value using the stylesheet declaration parser.
pub fn parse_color_value(source: &str) -> Result<crate::CssColor, StyleParseError> {
    let mut input = cssparser::ParserInput::new(source);
    let mut parser = cssparser::Parser::new(&mut input);
    let color = value::parse_color(&mut parser)?;
    parser
        .expect_exhausted()
        .map_err(|_| StyleParseError::InvalidColor("unexpected trailing color input".into()))?;
    Ok(color)
}
