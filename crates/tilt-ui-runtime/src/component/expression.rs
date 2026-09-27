//! Small, side-effect-free expression evaluator for reactive template values.

use serde_json::Value;

pub(super) const MAX_SOURCE_BYTES: usize = 4096;
pub(super) const MAX_TOKENS: usize = 256;
const MAX_NESTING: usize = 16;
const MAX_ARGUMENTS: usize = 8;

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
    Comma,
}

#[cfg(test)]
pub(super) fn evaluate(source: &str, lookup: impl Fn(&str) -> Option<Value>) -> Option<Value> {
    evaluate_with_methods(source, lookup, |_, _, _| None)
}

pub(super) fn evaluate_with_methods(
    source: &str,
    lookup: impl Fn(&str) -> Option<Value>,
    call: impl Fn(&str, &Value, &[Value]) -> Option<Value>,
) -> Option<Value> {
    let tokens = lex(source)?;
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        lookup: &lookup,
        call: &call,
        nesting: 0,
        active: true,
    };
    let value = parser.conditional()?;
    (parser.at == tokens.len()).then_some(value)
}

fn lex(source: &str) -> Option<Vec<Token>> {
    if source.len() > MAX_SOURCE_BYTES {
        return None;
    }
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
            ',' => Token::Comma,
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
            ch if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' => {
                let mut name = ch.to_string();
                while chars
                    .peek()
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '_')
                {
                    name.push(chars.next()?);
                }
                if name.starts_with('$') && name != "$event" {
                    return None;
                }
                match name.as_str() {
                    "true" => Token::Value(Value::Bool(true)),
                    "false" => Token::Value(Value::Bool(false)),
                    "null" => Token::Value(Value::Null),
                    "oder" | "or" => Token::Or,
                    _ => Token::Name(name),
                }
            }
            _ => return None,
        };
        if tokens.len() == MAX_TOKENS {
            return None;
        }
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

struct Parser<'a, F, C> {
    tokens: &'a [Token],
    at: usize,
    lookup: &'a F,
    call: &'a C,
    nesting: usize,
    active: bool,
}

impl<F: Fn(&str) -> Option<Value>, C: Fn(&str, &Value, &[Value]) -> Option<Value>>
    Parser<'_, F, C>
{
    fn with_active(
        &mut self,
        active: bool,
        parse: impl FnOnce(&mut Self) -> Option<Value>,
    ) -> Option<Value> {
        let previous = self.active;
        self.active &= active;
        let result = parse(self);
        self.active = previous;
        result
    }

    fn take(&mut self, token: Token) -> bool {
        if self.tokens.get(self.at) == Some(&token) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn conditional(&mut self) -> Option<Value> {
        if self.nesting == MAX_NESTING {
            return None;
        }
        self.nesting += 1;
        let result = self.conditional_inner();
        self.nesting -= 1;
        result
    }

    fn conditional_inner(&mut self) -> Option<Value> {
        let condition = self.or()?;
        if self.take(Token::Question) {
            let yes = self.with_active(truthy(&condition), Self::conditional)?;
            self.take(Token::Colon).then_some(())?;
            let no = self.with_active(!truthy(&condition), Self::conditional)?;
            Some(if !self.active {
                Value::Null
            } else if truthy(&condition) {
                yes
            } else {
                no
            })
        } else {
            Some(condition)
        }
    }

    fn or(&mut self) -> Option<Value> {
        let mut value = self.and()?;
        while self.take(Token::Or) {
            let right = self.with_active(!truthy(&value), Self::and)?;
            value = if self.active {
                Value::Bool(truthy(&value) || truthy(&right))
            } else {
                Value::Null
            };
        }
        Some(value)
    }

    fn and(&mut self) -> Option<Value> {
        let mut value = self.equality()?;
        while self.take(Token::And) {
            let right = self.with_active(truthy(&value), Self::equality)?;
            value = if self.active {
                Value::Bool(truthy(&value) && truthy(&right))
            } else {
                Value::Null
            };
        }
        Some(value)
    }

    fn equality(&mut self) -> Option<Value> {
        let mut value = self.comparison()?;
        loop {
            if self.take(Token::Eq) {
                let right = self.comparison()?;
                value = if self.active {
                    Value::Bool(value == right)
                } else {
                    Value::Null
                };
            } else if self.take(Token::Ne) {
                let right = self.comparison()?;
                value = if self.active {
                    Value::Bool(value != right)
                } else {
                    Value::Null
                };
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
            if !self.active {
                value = Value::Null;
                continue;
            }
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
                value = if !self.active {
                    Value::Null
                } else if value.is_string() || right.is_string() {
                    Value::String(text(&value) + &text(&right))
                } else {
                    numeric(value.as_f64()? + right.as_f64()?)
                };
            } else if self.take(Token::Minus) {
                let right = self.multiply()?;
                value = if self.active {
                    numeric(value.as_f64()? - right.as_f64()?)
                } else {
                    Value::Null
                };
            } else {
                return Some(value);
            }
        }
    }

    fn multiply(&mut self) -> Option<Value> {
        let mut value = self.unary()?;
        loop {
            if self.take(Token::Star) {
                let right = self.unary()?;
                value = if self.active {
                    numeric(value.as_f64()? * right.as_f64()?)
                } else {
                    Value::Null
                };
            } else if self.take(Token::Slash) {
                let right = self.unary()?;
                value = if self.active {
                    numeric(value.as_f64()? / right.as_f64()?)
                } else {
                    Value::Null
                };
            } else if self.take(Token::Percent) {
                let right = self.unary()?;
                value = if self.active {
                    numeric(value.as_f64()? % right.as_f64()?)
                } else {
                    Value::Null
                };
            } else {
                return Some(value);
            }
        }
    }

    fn unary(&mut self) -> Option<Value> {
        if self.take(Token::Not) {
            let value = self.unary()?;
            return Some(if self.active {
                Value::Bool(!truthy(&value))
            } else {
                Value::Null
            });
        }
        if self.take(Token::Minus) {
            let value = self.unary()?;
            return Some(if self.active {
                numeric(-value.as_f64()?)
            } else {
                Value::Null
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> Option<Value> {
        let value = if self.take(Token::Open) {
            let result = self.conditional()?;
            self.take(Token::Close).then_some(())?;
            result
        } else {
            let token = self.tokens.get(self.at)?.clone();
            self.at += 1;
            match token {
                Token::Value(value) => value,
                Token::Name(name) => return self.path(name),
                _ => return None,
            }
        };
        self.postfix(value, None)
    }

    fn path(&mut self, name: String) -> Option<Value> {
        if self.take(Token::Scope) {
            loop {
                let Token::Name(next) = self.tokens.get(self.at)?.clone() else {
                    return None;
                };
                self.at += 1;
                if !self.take(Token::Scope) {
                    return self.postfix(Value::String(next), None);
                }
            }
        }
        let value = if self.active {
            (self.lookup)(&name).unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        self.postfix(value, Some(name))
    }

    fn postfix(&mut self, mut value: Value, mut path: Option<String>) -> Option<Value> {
        loop {
            if self.take(Token::Dot) {
                let Token::Name(field) = self.tokens.get(self.at)?.clone() else {
                    return None;
                };
                self.at += 1;
                if self.take(Token::Open) {
                    let mut arguments = Vec::new();
                    if !self.take(Token::Close) {
                        loop {
                            if arguments.len() == MAX_ARGUMENTS {
                                return None;
                            }
                            arguments.push(self.conditional()?);
                            if self.take(Token::Close) {
                                break;
                            }
                            self.take(Token::Comma).then_some(())?;
                        }
                    }
                    let method_path = path.as_ref().map(|path| format!("{path}.{field}"));
                    value = if self.active {
                        string_method(&value, &field, &arguments).or_else(|| {
                            method_path
                                .as_deref()
                                .and_then(|path| (self.call)(path, &value, &arguments))
                        })?
                    } else {
                        Value::Null
                    };
                    path = None;
                } else {
                    if let Some(path) = &mut path {
                        path.push('.');
                        path.push_str(&field);
                    }
                    value = if self.active {
                        match value {
                            Value::Object(map) => map.get(&field).cloned().unwrap_or(Value::Null),
                            Value::Array(items) if field == "length" => Value::from(items.len()),
                            Value::String(text) if field == "length" => {
                                Value::from(text.chars().count())
                            }
                            _ => Value::Null,
                        }
                    } else {
                        Value::Null
                    };
                }
            } else if self.take(Token::Bracket) {
                let index = self.conditional()?;
                self.take(Token::EndBracket).then_some(())?;
                value = if self.active {
                    match (value, index) {
                        (Value::Array(items), Value::Number(index)) => index
                            .as_u64()
                            .and_then(|index| items.get(index as usize).cloned())
                            .unwrap_or(Value::Null),
                        (Value::Object(map), Value::String(index)) => {
                            map.get(&index).cloned().unwrap_or(Value::Null)
                        }
                        _ => Value::Null,
                    }
                } else {
                    Value::Null
                };
            } else {
                return Some(value);
            }
        }
    }
}

fn string_method(receiver: &Value, method: &str, arguments: &[Value]) -> Option<Value> {
    let Value::String(receiver) = receiver else {
        return None;
    };
    let [Value::String(argument)] = arguments else {
        return None;
    };
    let result = match method {
        "equals" => receiver == argument,
        "equalsIgnoreCase" => receiver.to_lowercase() == argument.to_lowercase(),
        "startWith" | "startsWith" => receiver.starts_with(argument.as_str()),
        "endsWith" => receiver.ends_with(argument.as_str()),
        "contains" => receiver.contains(argument.as_str()),
        _ => return None,
    };
    Some(Value::Bool(result))
}

#[cfg(test)]
mod tests {
    use super::{evaluate, evaluate_with_methods};
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

    #[test]
    fn string_methods_and_boolean_operators() {
        let lookup = |root: &str| (root == "name").then(|| json!("Abc"));
        for source in [
            "name.equals('Abc')",
            "name.equalsIgnoreCase('aBC')",
            "name.startWith('A')",
            "name.endsWith('c')",
            "name.contains('b')",
            "'x'.equals('x')",
            "name != 'abc' && 4 >= 3 && 3 <= 4",
            "false oder true",
        ] {
            assert_eq!(evaluate(source, lookup), Some(json!(true)), "{source}");
        }
        assert_eq!(
            evaluate("name.contains('z') || false", lookup),
            Some(json!(false))
        );
    }

    #[test]
    fn only_registered_member_calls_receive_resolved_values_and_arguments() {
        let lookup =
            |root: &str| (root == "user").then(|| json!({"first": "Ada", "last": "Lovelace"}));
        let call = |path: &str, receiver: &serde_json::Value, args: &[serde_json::Value]| match (
            path, args,
        ) {
            ("user.full_name", []) => Some(json!(format!(
                "{} {}",
                receiver.get("first")?.as_str()?,
                receiver.get("last")?.as_str()?
            ))),
            ("user.greeting", [serde_json::Value::String(prefix)]) => Some(json!(format!(
                "{prefix}, {}!",
                receiver.get("first")?.as_str()?
            ))),
            _ => None,
        };
        assert_eq!(
            evaluate_with_methods("user.full_name()", lookup, call),
            Some(json!("Ada Lovelace"))
        );
        assert_eq!(
            evaluate_with_methods("user.greeting('Hello')", lookup, call),
            Some(json!("Hello, Ada!"))
        );
        assert_eq!(
            evaluate_with_methods("user.full_name().contains('Ada')", lookup, call),
            Some(json!(true))
        );
        assert_eq!(evaluate("user.full_name()", lookup), None);
        assert_eq!(evaluate_with_methods("user.secret()", lookup, call), None);
        assert_eq!(
            evaluate_with_methods("user.full_name(1)", lookup, call),
            None
        );
    }

    #[test]
    fn expression_limits_reject_oversized_or_deep_calls() {
        let lookup = |_: &str| Some(json!({}));
        assert_eq!(evaluate(&"x".repeat(4097), lookup), None);
        assert_eq!(evaluate(&vec!["true"; 129].join(" || "), lookup), None);
        assert_eq!(
            evaluate(&format!("{}true{}", "(".repeat(16), ")".repeat(16)), lookup),
            None
        );
        assert_eq!(
            evaluate_with_methods("user.method(1,2,3,4,5,6,7,8)", lookup, |_, _, args| {
                Some(json!(args.len()))
            }),
            Some(json!(8))
        );
        assert_eq!(
            evaluate_with_methods("user.method(1,2,3,4,5,6,7,8,9)", lookup, |_, _, _| {
                Some(json!(true))
            }),
            None
        );
    }

    #[test]
    fn inactive_boolean_and_ternary_branches_do_not_call_methods() {
        let calls = std::cell::Cell::new(0);
        let call = |_: &str, _: &serde_json::Value, _: &[serde_json::Value]| {
            calls.set(calls.get() + 1);
            None
        };
        let lookup = |_: &str| Some(json!({}));
        assert_eq!(
            evaluate_with_methods("true || user.fail()", lookup, call),
            Some(json!(true))
        );
        assert_eq!(
            evaluate_with_methods("false && user.fail()", lookup, call),
            Some(json!(false))
        );
        assert_eq!(
            evaluate_with_methods("true ? 'ok' : user.fail()", lookup, call),
            Some(json!("ok"))
        );
        assert_eq!(
            evaluate_with_methods("false ? user.fail() : 'ok'", lookup, call),
            Some(json!("ok"))
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(
            evaluate_with_methods("false || user.fail()", lookup, call),
            None
        );
        assert_eq!(calls.get(), 1);
    }
}
