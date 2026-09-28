//! Expands authored nested selector rules before the typed CSS parser sees them.

use super::StyleParseError;

pub(super) fn expand_nesting(source: &str) -> Result<String, StyleParseError> {
    if !source.contains('&') {
        return Ok(source.to_owned());
    }
    rewrite_stylesheet(source, 0)
}

fn rewrite_stylesheet(source: &str, depth: usize) -> Result<String, StyleParseError> {
    if depth > 16 {
        return Err(syntax("CSS nesting exceeds 16 levels"));
    }
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(open) = find_top_level(source, cursor, '{') {
        let close = matching_brace(source, open)?;
        let prelude = &source[cursor..open];
        let body = &source[open + 1..close];
        let meaningful_prelude = strip_leading_trivia(prelude);
        if meaningful_prelude.starts_with("@media") {
            output.push_str(prelude);
            output.push('{');
            output.push_str(&rewrite_stylesheet(body, depth + 1)?);
            output.push('}');
        } else if meaningful_prelude.starts_with('@') {
            output.push_str(&source[cursor..=close]);
        } else {
            rewrite_rule(prelude, body, depth, &mut output)?;
        }
        cursor = close + 1;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

fn strip_leading_trivia(mut source: &str) -> &str {
    loop {
        source = source.trim_start();
        let Some(comment) = source.strip_prefix("/*") else {
            return source;
        };
        let Some(end) = comment.find("*/") else {
            return source;
        };
        source = &comment[end + 2..];
    }
}

fn rewrite_rule(
    selector: &str,
    body: &str,
    depth: usize,
    output: &mut String,
) -> Result<(), StyleParseError> {
    if depth > 16 {
        return Err(syntax("CSS nesting exceeds 16 levels"));
    }
    let mut cursor = 0;
    while let Some(open) = find_top_level(body, cursor, '{') {
        let close = matching_brace(body, open)?;
        let preceding = &body[cursor..open];
        let split = last_top_level_semicolon(preceding).map_or(0, |index| index + 1);
        let declarations = &preceding[..split];
        if !declarations.trim().is_empty() {
            output.push_str(selector);
            output.push('{');
            output.push_str(declarations);
            output.push('}');
        }
        let child = preceding[split..].trim();
        if child.starts_with('@') {
            return Err(syntax(
                "nested at-rules inside a selector are not supported",
            ));
        }
        let combined = combine_selectors(selector, child)?;
        rewrite_rule(&combined, &body[open + 1..close], depth + 1, output)?;
        cursor = close + 1;
    }
    let tail = &body[cursor..];
    if !tail.trim().is_empty() {
        output.push_str(selector);
        output.push('{');
        output.push_str(tail);
        output.push('}');
    }
    Ok(())
}

fn combine_selectors(parent: &str, child: &str) -> Result<String, StyleParseError> {
    let parents = split_selectors(parent)?;
    let children = split_selectors(child)?;
    let mut combined = Vec::new();
    for parent in &parents {
        for child in &children {
            let parent = parent.trim();
            let child = child.trim();
            if parent.is_empty() || child.is_empty() {
                return Err(syntax("empty nested selector"));
            }
            combined.push(if child.contains('&') {
                child.replace('&', parent)
            } else {
                format!("{parent} {child}")
            });
        }
    }
    Ok(combined.join(", "))
}

fn split_selectors(source: &str) -> Result<Vec<&str>, StyleParseError> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut parens = 0usize;
    let mut brackets = 0usize;
    let mut quote = None;
    for (index, ch) in source.char_indices() {
        if let Some(delimiter) = quote {
            if ch == delimiter {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => parens += 1,
            ')' => {
                parens = parens
                    .checked_sub(1)
                    .ok_or_else(|| syntax("unbalanced nested selector"))?
            }
            '[' => brackets += 1,
            ']' => {
                brackets = brackets
                    .checked_sub(1)
                    .ok_or_else(|| syntax("unbalanced nested selector"))?
            }
            ',' if parens == 0 && brackets == 0 => {
                parts.push(&source[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if parens != 0 || brackets != 0 || quote.is_some() {
        return Err(syntax("unbalanced nested selector"));
    }
    parts.push(&source[start..]);
    Ok(parts)
}

fn last_top_level_semicolon(source: &str) -> Option<usize> {
    let mut result = None;
    let mut parens = 0usize;
    let mut quote = None;
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let ch = bytes[index];
        if let Some(delimiter) = quote {
            if ch == b'\\' {
                index += 2;
                continue;
            }
            if ch == delimiter {
                quote = None;
            }
        } else if ch == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let Some(end) = source[index + 2..].find("*/") else {
                return result;
            };
            index += end + 4;
            continue;
        } else {
            match ch {
                b'\'' | b'"' => quote = Some(ch),
                b'(' => parens += 1,
                b')' => parens = parens.saturating_sub(1),
                b';' if parens == 0 => result = Some(index),
                _ => {}
            }
        }
        index += 1;
    }
    result
}

fn find_top_level(source: &str, from: usize, target: char) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = from;
    let mut parens = 0usize;
    let mut quote = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(delimiter) = quote {
            if byte == b'\\' {
                index += 2;
                continue;
            }
            if byte == delimiter {
                quote = None;
            }
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index = source[index + 2..]
                .find("*/")
                .map(|offset| index + offset + 4)?;
            continue;
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'(' => parens += 1,
                b')' => parens = parens.saturating_sub(1),
                _ if byte == target as u8 && parens == 0 => return Some(index),
                _ => {}
            }
        }
        index += 1;
    }
    None
}

fn matching_brace(source: &str, open: usize) -> Result<usize, StyleParseError> {
    let bytes = source.as_bytes();
    let mut index = open;
    let mut depth = 0usize;
    let mut quote = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(delimiter) = quote {
            if byte == b'\\' {
                index += 2;
                continue;
            }
            if byte == delimiter {
                quote = None;
            }
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index = source[index + 2..]
                .find("*/")
                .map(|offset| index + offset + 4)
                .ok_or_else(|| syntax("unclosed CSS comment"))?;
            continue;
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(index);
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    Err(syntax("unclosed CSS rule"))
}

fn syntax(message: &str) -> StyleParseError {
    StyleParseError::CssSyntax(message.to_owned())
}
