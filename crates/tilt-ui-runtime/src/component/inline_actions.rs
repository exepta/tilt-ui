//! Small, cached event scripts for explicit store mutations.

use bevy::ecs::world::World;
use serde_json::{Map, Number, Value};

use super::{UiBindingStore, binding_runtime::resolve_with_event, handlers::HtmlEvent};

const MAX_SCRIPT_BYTES: usize = 8192;
const MAX_CALLS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    Set,
    Add,
    Subtract,
    Multiply,
    Divide,
    Clamp,
    Toggle,
    Append,
}

impl Operation {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "set" => Self::Set,
            "add" => Self::Add,
            "min" | "sub" => Self::Subtract,
            "mul" => Self::Multiply,
            "div" => Self::Divide,
            "clamp" => Self::Clamp,
            "toggle" => Self::Toggle,
            "append" => Self::Append,
            _ => return None,
        })
    }

    fn argument_count(self) -> usize {
        match self {
            Self::Toggle => 0,
            Self::Clamp => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EventCall {
    Handler(String),
    Mutation {
        operation: Operation,
        target: String,
        arguments: Vec<String>,
    },
}

pub(super) fn parse_script(source: &str) -> Result<Vec<EventCall>, String> {
    if source.len() > MAX_SCRIPT_BYTES {
        return Err("event script exceeds 8192 bytes".into());
    }
    let parts = split_top_level(source, ';')?;
    if parts.len() > MAX_CALLS {
        return Err("event script exceeds 32 calls".into());
    }
    let mut calls = Vec::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if let Some(rest) = part.strip_prefix('$') {
            let Some(open) = rest.find('(') else {
                return Err(format!("inline action '{part}' is missing '('"));
            };
            if !rest.ends_with(')') {
                return Err(format!("inline action '{part}' is missing ')'"));
            }
            let name = rest[..open].trim();
            let operation = Operation::from_name(name)
                .ok_or_else(|| format!("unknown inline action '${name}'"))?;
            let arguments = split_top_level(&rest[open + 1..rest.len() - 1], ',')?;
            if arguments.len() != operation.argument_count() + 1 {
                return Err(format!(
                    "'${name}' expects {} argument(s) after its target",
                    operation.argument_count()
                ));
            }
            let target = parse_target(arguments[0])
                .ok_or_else(|| format!("invalid inline target '{}'", arguments[0]))?;
            calls.push(EventCall::Mutation {
                operation,
                target,
                arguments: arguments[1..]
                    .iter()
                    .map(|part| (*part).to_owned())
                    .collect(),
            });
        } else {
            let name = part.strip_suffix("()").unwrap_or(part).trim();
            if name.is_empty() || name.contains('(') || name.contains(')') {
                return Err(format!("invalid registered handler '{part}'"));
            }
            calls.push(EventCall::Handler(name.to_owned()));
        }
    }
    if calls.is_empty() {
        return Err("event script has no calls".into());
    }
    Ok(calls)
}

fn split_top_level(source: &str, delimiter: char) -> Result<Vec<&str>, String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in source.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => stack.push(ch),
            ')' | ']' | '}' => {
                let expected = match ch {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if stack.pop() != Some(expected) {
                    return Err("unbalanced event action delimiters".into());
                }
            }
            _ if ch == delimiter && stack.is_empty() => {
                parts.push(source[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if quote.is_some() || !stack.is_empty() {
        return Err("unterminated event action string or delimiter".into());
    }
    parts.push(source[start..].trim());
    Ok(parts)
}

fn parse_target(source: &str) -> Option<String> {
    let source = source.trim();
    let bytes = source.as_bytes();
    let mut at = 0;
    let mut result = read_identifier(bytes, &mut at)?.to_owned();
    while at < bytes.len() {
        match bytes[at] {
            b'.' => {
                at += 1;
                result.push('.');
                result.push_str(read_identifier(bytes, &mut at)?);
            }
            b'[' => {
                at += 1;
                let start = at;
                while at < bytes.len() && bytes[at].is_ascii_digit() {
                    at += 1;
                }
                if start == at || bytes.get(at) != Some(&b']') {
                    return None;
                }
                let index = source[start..at].parse::<usize>().ok()?;
                result.push('.');
                result.push_str(&index.to_string());
                at += 1;
            }
            _ => return None,
        }
    }
    (result.len() <= 256).then_some(result)
}

fn read_identifier<'a>(bytes: &'a [u8], at: &mut usize) -> Option<&'a str> {
    let start = *at;
    let first = *bytes.get(*at)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    *at += 1;
    while bytes
        .get(*at)
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == b'_')
    {
        *at += 1;
    }
    std::str::from_utf8(&bytes[start..*at]).ok()
}

pub(super) fn execute_mutation(
    world: &mut World,
    event: &HtmlEvent,
    call: &EventCall,
) -> Result<(), String> {
    let EventCall::Mutation {
        operation,
        target,
        arguments,
    } = call
    else {
        return Ok(());
    };
    let event_value = event_value(event);
    let arguments = arguments
        .iter()
        .map(|source| {
            if source.starts_with('[') || source.starts_with('{') {
                serde_json::from_str::<Value>(source)
                    .map_err(|_| format!("invalid JSON literal '{source}'"))
            } else if source == "$event" {
                Ok(event_value.get("value").cloned().unwrap_or(Value::Null))
            } else {
                resolve_with_event(world, event.target, source, &event_value)
                    .ok_or_else(|| format!("cannot resolve inline value '{source}'"))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let current = world
        .resource::<UiBindingStore>()
        .json_path(target)
        .ok_or_else(|| format!("unknown inline target '{target}'"))?;
    let next = apply(*operation, current, &arguments)
        .ok_or_else(|| format!("invalid value for inline target '{target}'"))?;
    world
        .resource_mut::<UiBindingStore>()
        .set_json_path(target, next)
        .ok_or_else(|| format!("target '{target}' is read-only or rejected the value"))?;
    Ok(())
}

fn apply(operation: Operation, current: Value, arguments: &[Value]) -> Option<Value> {
    match operation {
        Operation::Set => Some(arguments.first()?.clone()),
        Operation::Add => number_value(number(&current)? + number(arguments.first()?)?),
        Operation::Subtract => number_value(number(&current)? - number(arguments.first()?)?),
        Operation::Multiply => number_value(number(&current)? * number(arguments.first()?)?),
        Operation::Divide => {
            let divisor = number(arguments.first()?)?;
            (divisor != 0.0)
                .then(|| number_value(number(&current)? / divisor))
                .flatten()
        }
        Operation::Clamp => {
            let low = number(arguments.first()?)?;
            let high = number(arguments.get(1)?)?;
            (low <= high)
                .then(|| number_value(number(&current)?.clamp(low, high)))
                .flatten()
        }
        Operation::Toggle => Some(Value::Bool(!current.as_bool()?)),
        Operation::Append => match (current, arguments.first()?) {
            (Value::String(mut text), Value::String(value)) => {
                text.push_str(value);
                Some(Value::String(text))
            }
            (Value::Array(mut items), value) => {
                items.push(value.clone());
                Some(Value::Array(items))
            }
            _ => None,
        },
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse().ok(),
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        _ => None,
    }
    .filter(|value: &f64| value.is_finite())
}

fn number_value(value: f64) -> Option<Value> {
    if !value.is_finite() {
        return None;
    }
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value < i64::MAX as f64 {
        Some(Value::from(value as i64))
    } else {
        Number::from_f64(value).map(Value::Number)
    }
}

fn event_value(event: &HtmlEvent) -> Value {
    let mut fields = Map::new();
    fields.insert("kind".into(), Value::String(event.kind.into()));
    fields.insert(
        "value".into(),
        event
            .value
            .as_deref()
            .map(|value| typed_event_value(value, event.data.get("value_type").map(String::as_str)))
            .unwrap_or(Value::Null),
    );
    let mut data = Map::new();
    for (key, value) in &event.data {
        if key == "value_type" {
            continue;
        }
        let converted = typed_event_value(
            value,
            match key.as_str() {
                "checked" | "selected" | "repeat" => Some("boolean"),
                "x" | "y" | "delta_x" | "delta_y" | "distance_x" | "distance_y" | "red"
                | "green" | "blue" | "alpha" | "upper" => Some("number"),
                _ => None,
            },
        );
        fields.insert(key.clone(), converted.clone());
        data.insert(key.clone(), converted);
    }
    fields.insert("data".into(), Value::Object(data));
    Value::Object(fields)
}

fn typed_event_value(value: &str, kind: Option<&str>) -> Value {
    match kind {
        Some("boolean") => value
            .parse::<bool>()
            .map(Value::Bool)
            .unwrap_or(Value::Null),
        Some("number") => value
            .parse::<f64>()
            .ok()
            .and_then(number_value)
            .unwrap_or(Value::Null),
        _ => Value::String(value.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::World;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use super::{EventCall, Operation, execute_mutation, parse_script};
    use crate::component::{HtmlEvent, UiBindingStore, UiStore};

    #[derive(Default, Serialize, Deserialize, PartialEq, Debug)]
    struct TestState {
        count: f64,
        enabled: bool,
        name: String,
        tags: Vec<String>,
    }

    impl UiStore for TestState {
        const STORE_KEY: &'static str = "TestState";
        const STORE_PATH: &'static str = "test::TestState";
    }

    fn world_and_event(handler: &str) -> (World, HtmlEvent) {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let mut stores = UiBindingStore::default();
        stores.register_mutable::<TestState>();
        stores.set_store(TestState {
            count: 4.0,
            enabled: true,
            name: "Ada".into(),
            tags: vec!["first".into()],
        });
        world.insert_resource(stores);
        let event = HtmlEvent {
            target,
            kind: "change",
            value: Some("Grace".into()),
            submitter: None,
            data: Default::default(),
            handler: handler.into(),
        };
        (world, event)
    }

    #[test]
    fn parses_semicolons_inside_strings_and_mixed_handlers() {
        let calls =
            parse_script("$set(testState.name, 'A; B'); save(); $add(testState.count, 2)").unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[1], EventCall::Handler("save".into()));
        assert!(matches!(
            calls[2],
            EventCall::Mutation {
                operation: Operation::Add,
                ..
            }
        ));
        assert!(parse_script("$set(testState.name, 'oops)").is_err());
        assert!(parse_script("$unknown(testState.count, 2)").is_err());
        assert!(parse_script("$set(testState.missing[foo], 2)").is_err());
    }

    #[test]
    fn mutates_typed_store_sequentially_with_event_values() {
        let script = "$set(testState.name, $event); $add(testState.count, 2); \
            $min(testState.count, 1); $mul(testState.count, 3); \
            $div(testState.count, 5); $clamp(testState.count, 0, 2); \
            $toggle(testState.enabled); $append(testState.tags, $event.value)";
        let (mut world, event) = world_and_event(script);
        for call in parse_script(script).unwrap() {
            execute_mutation(&mut world, &event, &call).unwrap();
        }
        let stores = world.resource::<UiBindingStore>();
        assert_eq!(
            stores.json("testState"),
            Some(&json!({
                "count": 2.0, "enabled": false, "name": "Grace", "tags": ["first", "Grace"]
            }))
        );
        assert_eq!(stores.get_store::<TestState>().unwrap().count, 2.0);
        assert_eq!(stores.get_store::<TestState>().unwrap().name, "Grace");
    }

    #[test]
    fn rejects_invalid_writes_without_changing_rust_state() {
        let (mut world, event) = world_and_event("test");
        for source in [
            "$div(testState.count, 0)",
            "$set(testState.count, 'not a number')",
            "$set(testState.unknown, 1)",
            "$toggle(testState.count)",
        ] {
            let call = parse_script(source).unwrap().remove(0);
            assert!(execute_mutation(&mut world, &event, &call).is_err());
            assert_eq!(
                world
                    .resource::<UiBindingStore>()
                    .get_store::<TestState>()
                    .unwrap()
                    .count,
                4.0
            );
        }
    }

    #[test]
    fn checked_event_is_boolean_for_set() {
        let (mut world, mut event) = world_and_event("$set(testState.enabled, $event.checked)");
        event.data.insert("checked".into(), "false".into());
        let call = parse_script(&event.handler).unwrap().remove(0);
        execute_mutation(&mut world, &event, &call).unwrap();
        assert!(
            !world
                .resource::<UiBindingStore>()
                .get_store::<TestState>()
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn numeric_event_values_and_indexed_targets_use_expression_engine() {
        let (mut world, mut event) = world_and_event("$set(testState.count, $event.value + 2)");
        event.value = Some("3".into());
        event.data.insert("value_type".into(), "number".into());
        let call = parse_script(&event.handler).unwrap().remove(0);
        execute_mutation(&mut world, &event, &call).unwrap();
        assert_eq!(
            world
                .resource::<UiBindingStore>()
                .json_path("testState.count"),
            Some(json!(5.0))
        );

        event.handler = "$set(testState.tags[0], 'changed')".into();
        let call = parse_script(&event.handler).unwrap().remove(0);
        execute_mutation(&mut world, &event, &call).unwrap();
        assert_eq!(
            world
                .resource::<UiBindingStore>()
                .json_path("testState.tags.0"),
            Some(json!("changed"))
        );
    }

    #[test]
    fn read_only_stores_cannot_be_changed_by_actions() {
        let (mut world, event) = world_and_event("$add(testState.count, 1)");
        let mut stores = UiBindingStore::default();
        stores.register::<TestState>();
        world.insert_resource(stores);
        let call = parse_script(&event.handler).unwrap().remove(0);
        assert!(execute_mutation(&mut world, &event, &call).is_err());
        assert_eq!(
            world
                .resource::<UiBindingStore>()
                .json_path("testState.count"),
            Some(json!(0.0))
        );
    }
}
