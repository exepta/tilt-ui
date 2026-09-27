//! Small, side-effect-free expression evaluator for reactive template values.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Value(Value),
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Question,
    Colon,
    Dot,
    Scope,
    Open,
    Close,
    Bracket,
    EndBracket,
}

pub(super) fn evaluate(source: &str, lookup: impl Fn(&str) -> Option<Value>) -> Option<Value> {
    let tokens = lex(source)?;
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        lookup: &lookup,
    };
    let value = parser.conditional()?;
    (parser.at == tokens.len()).then_some(value)
}

fn lex(source: &str) -> Option<Vec<Token>> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        let token = match ch {
            '(' => Token::Open,
            ')' => Token::Close,
            '[' => Token::Bracket,
            ']' => Token::EndBracket,
            '?' => Token::Question,
            '.' => Token::Dot,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '%' => Token::Percent,
            ':' if chars.next_if_eq(&':').is_some() => Token::Scope,
            ':' => Token::Colon,
            '!' if chars.next_if_eq(&'=').is_some() => Token::Ne,
            '!' => Token::Not,
            '=' if chars.next_if_eq(&'=').is_some() => Token::Eq,
            '<' if chars.next_if_eq(&'=').is_some() => Token::Le,
            '<' => Token::Lt,
            '>' if chars.next_if_eq(&'=').is_some() => Token::Ge,
            '>' => Token::Gt,
            '&' if chars.next_if_eq(&'&').is_some() => Token::And,
            '|' if chars.next_if_eq(&'|').is_some() => Token::Or,
            '"' | '\'' => {
                let mut value = String::new();
                loop {
                    let current = chars.next()?;
                    if current == ch {
                        break;
                    }
                    if current == '\\' {
                        value.push(chars.next()?);
                    } else {
                        value.push(current);
                    }
                }
                Token::Value(Value::String(value))
            }
            ch if ch.is_ascii_digit() => {
                let mut number = ch.to_string();
                while chars.peek().is_some_and(|next| next.is_ascii_digit()) {
                    number.push(chars.next()?);
                }
                if chars.peek() == Some(&'.')
                    && chars
                        .clone()
                        .nth(1)
                        .is_some_and(|next| next.is_ascii_digit())
                {
                    number.push(chars.next()?);
                    while chars.peek().is_some_and(|next| next.is_ascii_digit()) {
                        number.push(chars.next()?);
                    }
                }
                Token::Value(numeric(number.parse().ok()?))
            }
            ch if ch.is_ascii_alphabetic() || ch == '_' => {
                let mut name = ch.to_string();
                while chars
                    .peek()
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '_')
                {
                    name.push(chars.next()?);
                }
                match name.as_str() {
                    "true" => Token::Value(Value::Bool(true)),
                    "false" => Token::Value(Value::Bool(false)),
                    "null" => Token::Value(Value::Null),
                    _ => Token::Name(name),
                }
            }
            _ => return None,
        };
        tokens.push(token);
    }
    Some(tokens)
}

fn numeric(number: f64) -> Value {
    if number.is_finite()
        && number.fract() == 0.0
        && number >= i64::MIN as f64
        && number < i64::MAX as f64
    {
        Value::from(number as i64)
    } else {
        serde_json::Number::from_f64(number).map_or(Value::Null, Value::Number)
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

struct Parser<'a, F> {
    tokens: &'a [Token],
    at: usize,
    lookup: &'a F,
}

impl<F: Fn(&str) -> Option<Value>> Parser<'_, F> {
    fn take(&mut self, token: Token) -> bool {
        if self.tokens.get(self.at) == Some(&token) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn conditional(&mut self) -> Option<Value> {
        let condition = self.or()?;
        if self.take(Token::Question) {
            let yes = self.conditional()?;
            self.take(Token::Colon).then_some(())?;
            let no = self.conditional()?;
            Some(if truthy(&condition) { yes } else { no })
        } else {
            Some(condition)
        }
    }

    fn or(&mut self) -> Option<Value> {
        let mut value = self.and()?;
        while self.take(Token::Or) {
            let right = self.and()?;
            value = Value::Bool(truthy(&value) || truthy(&right));
        }
        Some(value)
    }

    fn and(&mut self) -> Option<Value> {
        let mut value = self.equality()?;
        while self.take(Token::And) {
            let right = self.equality()?;
            value = Value::Bool(truthy(&value) && truthy(&right));
        }
        Some(value)
    }

    fn equality(&mut self) -> Option<Value> {
        let mut value = self.comparison()?;
        loop {
            if self.take(Token::Eq) {
                value = Value::Bool(value == self.comparison()?);
            } else if self.take(Token::Ne) {
                value = Value::Bool(value != self.comparison()?);
            } else {
                return Some(value);
            }
        }
    }

    fn comparison(&mut self) -> Option<Value> {
        let mut value = self.addition()?;
        loop {
            let operation = if self.take(Token::Le) {
                0
            } else if self.take(Token::Lt) {
                1
            } else if self.take(Token::Ge) {
                2
            } else if self.take(Token::Gt) {
                3
            } else {
                return Some(value);
            };
            let right = self.addition()?;
            let order = match (&value, &right) {
                (Value::Number(a), Value::Number(b)) => a.as_f64()?.partial_cmp(&b.as_f64()?),
                (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
                _ => None,
            };
            value = Value::Bool(order.is_some_and(|order| match operation {
                0 => order.is_le(),
                1 => order.is_lt(),
                2 => order.is_ge(),
                _ => order.is_gt(),
            }));
        }
    }

    fn addition(&mut self) -> Option<Value> {
        let mut value = self.multiply()?;
        loop {
            if self.take(Token::Plus) {
                let right = self.multiply()?;
                value = if value.is_string() || right.is_string() {
                    Value::String(text(&value) + &text(&right))
                } else {
                    numeric(value.as_f64()? + right.as_f64()?)
                };
            } else if self.take(Token::Minus) {
                value = numeric(value.as_f64()? - self.multiply()?.as_f64()?);
            } else {
                return Some(value);
            }
        }
    }

    fn multiply(&mut self) -> Option<Value> {
        let mut value = self.unary()?;
        loop {
            if self.take(Token::Star) {
                value = numeric(value.as_f64()? * self.unary()?.as_f64()?);
            } else if self.take(Token::Slash) {
                value = numeric(value.as_f64()? / self.unary()?.as_f64()?);
            } else if self.take(Token::Percent) {
                value = numeric(value.as_f64()? % self.unary()?.as_f64()?);
            } else {
                return Some(value);
            }
        }
    }

    fn unary(&mut self) -> Option<Value> {
        if self.take(Token::Not) {
            return Some(Value::Bool(!truthy(&self.unary()?)));
        }
        if self.take(Token::Minus) {
            return Some(numeric(-self.unary()?.as_f64()?));
        }
        self.primary()
    }

    fn primary(&mut self) -> Option<Value> {
        if self.take(Token::Open) {
            let result = self.conditional()?;
            self.take(Token::Close).then_some(result)
        } else {
            let token = self.tokens.get(self.at)?.clone();
            self.at += 1;
            match token {
                Token::Value(value) => Some(value),
                Token::Name(name) => self.path(name),
                _ => None,
            }
        }
    }

    fn path(&mut self, name: String) -> Option<Value> {
        if self.take(Token::Scope) {
            loop {
                let Token::Name(next) = self.tokens.get(self.at)?.clone() else {
                    return None;
                };
                self.at += 1;
                if !self.take(Token::Scope) {
                    return Some(Value::String(next));
                }
            }
        }
        let mut value = (self.lookup)(&name).unwrap_or(Value::Null);
        loop {
            if self.take(Token::Dot) {
                let Token::Name(field) = self.tokens.get(self.at)?.clone() else {
                    return None;
                };
                self.at += 1;
                value = match value {
                    Value::Object(map) => map.get(&field).cloned().unwrap_or(Value::Null),
                    Value::Array(items) if field == "length" => Value::from(items.len()),
                    Value::String(text) if field == "length" => Value::from(text.chars().count()),
                    _ => Value::Null,
                };
            } else if self.take(Token::Bracket) {
                let index = self.conditional()?;
                self.take(Token::EndBracket).then_some(())?;
                value = match (value, index) {
                    (Value::Array(items), Value::Number(index)) => index
                        .as_u64()
                        .and_then(|index| items.get(index as usize).cloned())
                        .unwrap_or(Value::Null),
                    (Value::Object(map), Value::String(index)) => {
                        map.get(&index).cloned().unwrap_or(Value::Null)
                    }
                    _ => Value::Null,
                };
            } else {
                return Some(value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate;
    use serde_json::json;

    #[test]
    fn expressions_obey_precedence_and_resolve_paths() {
        let lookup = |root: &str| {
            (root == "state").then(|| json!({"count": 3, "items": ["A", "B"], "enabled": true}))
        };
        assert_eq!(evaluate("state.count + 2 * 4", lookup), Some(json!(11)));
        assert_eq!(
            evaluate("state.enabled && state.items[1] == 'B'", lookup),
            Some(json!(true))
        );
        assert_eq!(
            evaluate("state.count > 2 ? 'ready' : 'wait'", lookup),
            Some(json!("ready"))
        );
        assert_eq!(evaluate("state.items.length", lookup), Some(json!(2)));
        assert_eq!(evaluate("state.count / 0", lookup), Some(json!(null)));
        assert!(evaluate("state.count +", lookup).is_none());
    }
}
