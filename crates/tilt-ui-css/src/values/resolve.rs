//! Resolves inherited CSS variables and supported math functions at style time.

use std::collections::BTreeMap;

/// Dimensions used when a mixed-unit expression must become CSS pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValueContext {
    /// Relevant containing-block dimension in logical pixels.
    pub percentage_base: f32,
    /// Current viewport width in logical pixels.
    pub viewport_width: f32,
    /// Current viewport height in logical pixels.
    pub viewport_height: f32,
}

/// Expands `var()` and evaluates `calc()`, `min()`, `max()`, and `sin()`.
/// An invalid or cyclic variable without a fallback invalidates this value.
pub fn resolve_value(
    source: &str,
    variables: &BTreeMap<String, String>,
    context: ValueContext,
) -> Result<String, String> {
    let expanded = expand_variables(source, variables, &mut Vec::new())?;
    rewrite_functions(&expanded, &["calc", "min", "max", "sin"], |name, args| {
        let expression = format!("{name}({args})");
        let value = MathParser::new(&expression, context).parse()?;
        Ok(match value {
            MathValue::Number(value) if value.is_finite() => format!("{value}"),
            MathValue::Length(value) if value.is_finite() => format!("{value}px"),
            _ => return Err("non-finite CSS expression result".into()),
        })
    })
}

fn expand_variables(
    source: &str,
    variables: &BTreeMap<String, String>,
    stack: &mut Vec<String>,
) -> Result<String, String> {
    rewrite_functions(source, &["var"], |_, args| {
        let (name, fallback) = split_first_comma(args);
        let name = name.trim();
        if !name.starts_with("--") || name.len() <= 2 {
            return Err(format!("invalid CSS variable name {name:?}"));
        }
        let candidate = variables
            .get(name)
            .filter(|_| !stack.iter().any(|item| item == name));
        if let Some(value) = candidate {
            if stack.len() >= 32 {
                return Err("CSS variable nesting exceeds 32 levels".into());
            }
            stack.push(name.to_owned());
            let result = expand_variables(value, variables, stack);
            stack.pop();
            match result {
                Ok(value) => Ok(value),
                Err(_) if fallback.is_some() => {
                    expand_variables(fallback.unwrap(), variables, stack)
                }
                Err(error) => Err(error),
            }
        } else if let Some(fallback) = fallback {
            expand_variables(fallback, variables, stack)
        } else {
            Err(format!("unresolved CSS variable {name}"))
        }
    })
}

fn split_first_comma(source: &str) -> (&str, Option<&str>) {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in source.char_indices() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == delimiter {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return (&source[..index], Some(source[index + 1..].trim())),
            _ => {}
        }
    }
    (source, None)
}

fn rewrite_functions(
    source: &str,
    names: &[&str],
    mut replace: impl FnMut(&str, &str) -> Result<String, String>,
) -> Result<String, String> {
    let mut result = String::with_capacity(source.len());
    let mut index = 0;
    let mut quote = None;
    while index < source.len() {
        let ch = source[index..].chars().next().unwrap();
        if let Some(delimiter) = quote {
            result.push(ch);
            index += ch.len_utf8();
            if ch == delimiter && !source[..index - ch.len_utf8()].ends_with('\\') {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            result.push(ch);
            index += ch.len_utf8();
            continue;
        }
        let previous_is_ident = source[..index]
            .chars()
            .last()
            .is_some_and(|previous| previous.is_ascii_alphanumeric() || previous == '-');
        if !previous_is_ident {
            if let Some(name) = names.iter().find(|name| {
                source[index..]
                    .get(..name.len())
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
                    && source[index..].len() > name.len()
                    && source.as_bytes()[index + name.len()] == b'('
            }) {
                let open = index + name.len();
                let close = matching_close(source, open)?;
                result.push_str(&replace(name, &source[open + 1..close])?);
                index = close + 1;
                continue;
            }
        }
        result.push(ch);
        index += ch.len_utf8();
    }
    Ok(result)
}

fn matching_close(source: &str, open: usize) -> Result<usize, String> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (offset, ch) in source[open..].char_indices() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == delimiter {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(open + offset);
                }
            }
            _ => {}
        }
    }
    Err("unclosed CSS function".into())
}

#[derive(Debug, Clone, Copy)]
enum MathValue {
    Number(f32),
    Length(f32),
}

impl MathValue {
    fn add(self, other: Self, subtract: bool) -> Result<Self, String> {
        let sign = if subtract { -1.0 } else { 1.0 };
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => Ok(Self::Number(a + sign * b)),
            (Self::Length(a), Self::Length(b)) => Ok(Self::Length(a + sign * b)),
            _ => Err("cannot add a number and a CSS length".into()),
        }
    }
    fn multiply(self, other: Self, divide: bool) -> Result<Self, String> {
        let (left, right) = (self, other);
        if divide && matches!(right, Self::Number(value) if value == 0.0) {
            return Err("division by zero in CSS expression".into());
        }
        match (left, right, divide) {
            (Self::Number(a), Self::Number(b), false) => Ok(Self::Number(a * b)),
            (Self::Number(a), Self::Number(b), true) => Ok(Self::Number(a / b)),
            (Self::Length(a), Self::Number(b), false) => Ok(Self::Length(a * b)),
            (Self::Length(a), Self::Number(b), true) => Ok(Self::Length(a / b)),
            (Self::Number(a), Self::Length(b), false) => Ok(Self::Length(a * b)),
            _ => Err("invalid CSS expression multiplication or division".into()),
        }
    }
    fn negate(self) -> Self {
        match self {
            Self::Number(value) => Self::Number(-value),
            Self::Length(value) => Self::Length(-value),
        }
    }
}

struct MathParser<'a> {
    source: &'a str,
    offset: usize,
    context: ValueContext,
    depth: usize,
}

impl<'a> MathParser<'a> {
    fn new(source: &'a str, context: ValueContext) -> Self {
        Self {
            source,
            offset: 0,
            context,
            depth: 0,
        }
    }
    fn parse(mut self) -> Result<MathValue, String> {
        let value = self.expression()?;
        self.space();
        if self.offset != self.source.len() {
            return Err("unexpected token in CSS expression".into());
        }
        Ok(value)
    }
    fn space(&mut self) {
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }
    fn consume(&mut self, token: u8) -> bool {
        self.space();
        if self.source.as_bytes().get(self.offset) == Some(&token) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
    fn expression(&mut self) -> Result<MathValue, String> {
        let mut value = self.term()?;
        loop {
            if self.consume(b'+') {
                value = value.add(self.term()?, false)?;
            } else if self.consume(b'-') {
                value = value.add(self.term()?, true)?;
            } else {
                break;
            }
        }
        Ok(value)
    }
    fn term(&mut self) -> Result<MathValue, String> {
        let mut value = self.primary()?;
        loop {
            if self.consume(b'*') {
                value = value.multiply(self.primary()?, false)?;
            } else if self.consume(b'/') {
                value = value.multiply(self.primary()?, true)?;
            } else {
                break;
            }
        }
        Ok(value)
    }
    fn primary(&mut self) -> Result<MathValue, String> {
        if self.depth >= 32 {
            return Err("CSS math nesting exceeds 32 levels".into());
        }
        self.depth += 1;
        let result = self.primary_inner();
        self.depth -= 1;
        result
    }
    fn primary_inner(&mut self) -> Result<MathValue, String> {
        self.space();
        if self.consume(b'+') {
            return self.primary();
        }
        if self.consume(b'-') {
            return Ok(self.primary()?.negate());
        }
        if self.consume(b'(') {
            let value = self.expression()?;
            if !self.consume(b')') {
                return Err("unclosed CSS expression".into());
            }
            return Ok(value);
        }
        if self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            let start = self.offset;
            while self
                .source
                .as_bytes()
                .get(self.offset)
                .is_some_and(u8::is_ascii_alphabetic)
            {
                self.offset += 1;
            }
            let name = self.source[start..self.offset].to_ascii_lowercase();
            if !self.consume(b'(') {
                return Err(format!("expected '(' after {name}"));
            }
            let first = self.expression()?;
            let result = match name.as_str() {
                "calc" => first,
                "sin" => match first {
                    MathValue::Number(value) => MathValue::Number(value.sin()),
                    _ => return Err("sin() requires an angle or number".into()),
                },
                "min" | "max" => {
                    let mut best = first;
                    while self.consume(b',') {
                        let next = self.expression()?;
                        best = match (best, next) {
                            (MathValue::Number(a), MathValue::Number(b)) => {
                                MathValue::Number(if name == "min" { a.min(b) } else { a.max(b) })
                            }
                            (MathValue::Length(a), MathValue::Length(b)) => {
                                MathValue::Length(if name == "min" { a.min(b) } else { a.max(b) })
                            }
                            _ => return Err(format!("mixed types in {name}()")),
                        };
                    }
                    best
                }
                _ => return Err(format!("unsupported CSS math function {name}")),
            };
            if !self.consume(b')') {
                return Err(format!("unclosed {name}()"));
            }
            return Ok(result);
        }
        let start = self.offset;
        let bytes = self.source.as_bytes();
        while bytes.get(self.offset).is_some_and(u8::is_ascii_digit) {
            self.offset += 1;
        }
        if bytes.get(self.offset) == Some(&b'.') {
            self.offset += 1;
            while bytes.get(self.offset).is_some_and(u8::is_ascii_digit) {
                self.offset += 1;
            }
        }
        if self.offset == start {
            return Err("expected a CSS number or length".into());
        }
        if bytes
            .get(self.offset)
            .is_some_and(|byte| matches!(byte, b'e' | b'E'))
        {
            let exponent_start = self.offset;
            self.offset += 1;
            if bytes
                .get(self.offset)
                .is_some_and(|byte| matches!(byte, b'+' | b'-'))
            {
                self.offset += 1;
            }
            let digits_start = self.offset;
            while bytes.get(self.offset).is_some_and(u8::is_ascii_digit) {
                self.offset += 1;
            }
            if self.offset == digits_start {
                self.offset = exponent_start;
            }
        }
        let number: f32 = self.source[start..self.offset]
            .parse()
            .map_err(|_| "invalid CSS number")?;
        let unit_start = self.offset;
        while bytes
            .get(self.offset)
            .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'%')
        {
            self.offset += 1;
        }
        let unit = self.source[unit_start..self.offset].to_ascii_lowercase();
        let value = match unit.as_str() {
            "" => MathValue::Number(number),
            "px" => MathValue::Length(number),
            "%" => MathValue::Length(self.context.percentage_base * number / 100.0),
            "vw" => MathValue::Length(self.context.viewport_width * number / 100.0),
            "vh" => MathValue::Length(self.context.viewport_height * number / 100.0),
            "deg" => MathValue::Number(number.to_radians()),
            "rad" => MathValue::Number(number),
            _ => return Err(format!("unsupported CSS unit {unit:?}")),
        };
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_fallbacks_cycles_and_mixed_math() {
        let variables = BTreeMap::from([
            ("--space".into(), "10px".into()),
            ("--loop".into(), "var(--loop)".into()),
        ]);
        let context = ValueContext {
            percentage_base: 200.0,
            viewport_width: 800.0,
            viewport_height: 600.0,
        };
        assert_eq!(
            resolve_value("calc(50% + var(--space))", &variables, context).unwrap(),
            "110px"
        );
        assert_eq!(
            resolve_value("min(20vw, 150px)", &variables, context).unwrap(),
            "150px"
        );
        assert_eq!(
            resolve_value("calc(10px * sin(90deg))", &variables, context).unwrap(),
            "10px"
        );
        assert_eq!(
            resolve_value("var(--missing, max(2px, 4px))", &variables, context).unwrap(),
            "4px"
        );
        assert!(resolve_value("var(--loop)", &variables, context).is_err());
        assert_eq!(
            resolve_value("var(--loop, 5px)", &variables, context).unwrap(),
            "5px"
        );
    }
}
