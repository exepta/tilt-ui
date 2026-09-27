//! Rewrites Angular-style control blocks into reserved, layout-neutral template tags.
//! This runs only while an asset is parsed; the runtime receives a compact template.

use super::TemplateParseError;

pub(super) fn expand(source: &str) -> Result<String, TemplateParseError> {
    let mut parser = Controls {
        source,
        at: 0,
        html_depth: 0,
        stopped_by_brace: false,
    };
    parser.sequence(false, None)
}

struct Controls<'a> {
    source: &'a str,
    at: usize,
    html_depth: usize,
    stopped_by_brace: bool,
}

impl Controls<'_> {
    fn rest(&self) -> &str {
        &self.source[self.at..]
    }

    fn skip_space(&mut self) {
        while self.rest().starts_with(char::is_whitespace) {
            self.advance_char();
        }
    }

    fn advance_char(&mut self) {
        self.at += self.rest().chars().next().unwrap().len_utf8();
    }

    fn take_keyword(&mut self, keyword: &str) -> bool {
        if self.rest().starts_with(keyword)
            && !self.rest()[keyword.len()..]
                .starts_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.at += keyword.len();
            true
        } else {
            false
        }
    }

    fn group(&mut self, open: char, close: char) -> Result<String, TemplateParseError> {
        self.skip_space();
        if !self.rest().starts_with(open) {
            return Err(TemplateParseError::InvalidControl(format!(
                "expected `{open}` near {}",
                self.rest().chars().take(32).collect::<String>()
            )));
        }
        self.advance_char();
        let start = self.at;
        let mut depth = 1;
        let mut quote = None;
        while !self.rest().is_empty() {
            let ch = self.rest().chars().next().unwrap();
            self.advance_char();
            if let Some(active) = quote {
                if ch == '\\' {
                    if !self.rest().is_empty() {
                        self.advance_char();
                    }
                } else if ch == active {
                    quote = None;
                }
            } else if ch == '\'' || ch == '"' {
                quote = Some(ch);
            } else if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return Ok(self.source[start..self.at - ch.len_utf8()]
                        .trim()
                        .to_owned());
                }
            }
        }
        Err(TemplateParseError::InvalidControl(format!(
            "unclosed `{open}`"
        )))
    }

    fn header(&mut self) -> Result<String, TemplateParseError> {
        self.skip_space();
        if self.rest().starts_with('(') {
            return self.group('(', ')');
        }
        let start = self.at;
        let mut quote = None;
        while !self.rest().is_empty() {
            let ch = self.rest().chars().next().unwrap();
            if quote.is_none() && ch == '{' {
                return Ok(self.source[start..self.at].trim().to_owned());
            }
            self.advance_char();
            if let Some(active) = quote {
                if ch == active {
                    quote = None;
                }
            } else if ch == '\'' || ch == '"' {
                quote = Some(ch);
            }
        }
        Err(TemplateParseError::InvalidControl(
            "expected `{` after directive".into(),
        ))
    }

    fn sequence(
        &mut self,
        block: bool,
        limit_depth: Option<usize>,
    ) -> Result<String, TemplateParseError> {
        self.stopped_by_brace = false;
        let mut output = String::new();
        while !self.rest().is_empty() {
            if self.rest().starts_with("<!--") {
                let end = self.rest().find("-->").ok_or_else(|| {
                    TemplateParseError::InvalidControl("unclosed HTML comment".into())
                })? + 3;
                output.push_str(&self.rest()[..end]);
                self.at += end;
            } else if self.rest().starts_with('<') {
                if limit_depth == Some(self.html_depth) && self.rest().starts_with("</") {
                    self.stopped_by_brace = false;
                    return Ok(output);
                }
                let mut quote = None;
                let start = self.at;
                while !self.rest().is_empty() {
                    let ch = self.rest().chars().next().unwrap();
                    self.advance_char();
                    if let Some(active) = quote {
                        if ch == active {
                            quote = None;
                        }
                    } else if ch == '\'' || ch == '"' {
                        quote = Some(ch);
                    } else if ch == '>' {
                        break;
                    }
                }
                let tag = &self.source[start..self.at];
                if tag.starts_with("</") {
                    self.html_depth = self.html_depth.saturating_sub(1);
                } else if tag.starts_with('<')
                    && !tag.starts_with("<!")
                    && !tag.starts_with("<?")
                    && !tag.ends_with("/>")
                    && !is_void_tag(tag)
                {
                    self.html_depth += 1;
                }
                output.push_str(tag);
            } else if self.rest().starts_with("{{") {
                let end = self.rest().find("}}").ok_or_else(|| {
                    TemplateParseError::InvalidControl("unclosed interpolation".into())
                })? + 2;
                output.push_str(&self.rest()[..end]);
                self.at += end;
            } else if block && self.rest().starts_with('}') {
                self.at += 1;
                self.stopped_by_brace = true;
                return Ok(output);
            } else if self.take_keyword("@if") {
                let expression = self.group('(', ')')?;
                self.skip_space();
                let yes = self.group_body()?;
                output.push_str(&format!(
                    "<tilt-flow-if expression=\"{}\"><tilt-flow-then>{yes}</tilt-flow-then>",
                    escape(&expression)
                ));
                self.skip_space();
                if self.take_keyword("@else") {
                    self.skip_space();
                    let no = if self.rest().starts_with("@if")
                        || self.rest().starts_with("if ")
                        || self.rest().starts_with("if(")
                    {
                        self.sequence_one_if()?
                    } else {
                        self.group_body()?
                    };
                    output.push_str(&format!("<tilt-flow-else>{no}</tilt-flow-else>"));
                }
                output.push_str("</tilt-flow-if>");
            } else if self.take_keyword("@for") {
                let declaration = self.header()?;
                let (variables, expression) = declaration
                    .split_once(" in ")
                    .or_else(|| declaration.split_once(" of "))
                    .ok_or_else(|| TemplateParseError::InvalidControl(declaration.clone()))?;
                let (expression, track) = expression
                    .split_once("; track ")
                    .map_or((expression, ""), |(expression, track)| (expression, track));
                let variables = variables
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')');
                let mut parts = variables.split(',').map(str::trim);
                let variable = parts.next().unwrap_or("");
                let index = parts.next().unwrap_or("");
                if !identifier(variable)
                    || (!index.is_empty() && !identifier(index))
                    || parts.next().is_some()
                {
                    return Err(TemplateParseError::InvalidControl(declaration));
                }
                let body = self.group_body()?;
                output.push_str(&format!(
                    "<tilt-flow-for variable=\"{}\" index=\"{}\" expression=\"{}\" track=\"{}\">{body}</tilt-flow-for>",
                    escape(variable), escape(index), escape(expression.trim()), escape(track.trim())
                ));
            } else if self.take_keyword("@match") {
                let expression = self.header()?;
                self.skip_space();
                if !self.rest().starts_with('{') {
                    return Err(TemplateParseError::InvalidControl(
                        "expected match body".into(),
                    ));
                }
                self.at += 1;
                output.push_str(&format!(
                    "<tilt-flow-match expression=\"{}\">",
                    escape(&expression)
                ));
                loop {
                    self.skip_space();
                    if self.rest().starts_with('}') {
                        self.at += 1;
                        break;
                    }
                    let pattern = self.until_arrow()?;
                    let body = self.group_body()?;
                    output.push_str(&format!(
                        "<tilt-flow-case pattern=\"{}\">{body}</tilt-flow-case>",
                        escape(pattern.trim())
                    ));
                    self.skip_space();
                    if self.rest().starts_with(',') {
                        self.at += 1;
                    }
                }
                output.push_str("</tilt-flow-match>");
            } else if self.take_keyword("@let") {
                let declaration = self.until_semicolon()?;
                let (name, expression) = declaration
                    .split_once('=')
                    .ok_or_else(|| TemplateParseError::InvalidControl(declaration.clone()))?;
                let name = name.trim();
                if !identifier(name) || expression.trim().is_empty() {
                    return Err(TemplateParseError::InvalidControl(declaration));
                }
                let rest = self.sequence(block, Some(self.html_depth))?;
                output.push_str(&format!(
                    "<tilt-flow-let name=\"{}\" expression=\"{}\">{rest}</tilt-flow-let>",
                    escape(name),
                    escape(expression.trim())
                ));
                if self.stopped_by_brace || self.rest().is_empty() {
                    return Ok(output);
                }
            } else {
                let ch = self.rest().chars().next().unwrap();
                output.push(ch);
                self.advance_char();
            }
        }
        if block {
            Err(TemplateParseError::InvalidControl(
                "unclosed control block".into(),
            ))
        } else {
            self.stopped_by_brace = false;
            Ok(output)
        }
    }

    fn group_body(&mut self) -> Result<String, TemplateParseError> {
        self.skip_space();
        if !self.rest().starts_with('{') {
            return Err(TemplateParseError::InvalidControl("expected `{`".into()));
        }
        self.at += 1;
        self.sequence(true, None)
    }

    fn sequence_one_if(&mut self) -> Result<String, TemplateParseError> {
        let start = self.at;
        // Parse an else-if as a nested if by temporarily finding its complete span.
        let bare = !self.rest().starts_with('@');
        if bare {
            self.take_keyword("if");
        } else {
            self.take_keyword("@if");
        }
        self.group('(', ')')?;
        self.group_body()?;
        self.skip_space();
        if self.take_keyword("@else") {
            self.skip_space();
            if self.rest().starts_with("@if")
                || self.rest().starts_with("if ")
                || self.rest().starts_with("if(")
            {
                self.sequence_one_if()?;
            } else {
                self.group_body()?;
            }
        }
        let source = &self.source[start..self.at];
        if bare {
            expand(&format!("@{source}"))
        } else {
            expand(source)
        }
    }

    fn until_arrow(&mut self) -> Result<String, TemplateParseError> {
        let start = self.at;
        let mut quote = None;
        while !self.rest().is_empty() {
            if quote.is_none() && self.rest().starts_with("=>") {
                let pattern = self.source[start..self.at].trim().to_owned();
                self.at += 2;
                return Ok(pattern);
            }
            let ch = self.rest().chars().next().unwrap();
            self.advance_char();
            if let Some(active) = quote {
                if ch == active {
                    quote = None;
                }
            } else if ch == '\'' || ch == '"' {
                quote = Some(ch);
            }
        }
        Err(TemplateParseError::InvalidControl(
            "match arm needs `=>`".into(),
        ))
    }

    fn until_semicolon(&mut self) -> Result<String, TemplateParseError> {
        let start = self.at;
        let mut quote = None;
        while !self.rest().is_empty() {
            let ch = self.rest().chars().next().unwrap();
            self.advance_char();
            if let Some(active) = quote {
                if ch == active {
                    quote = None;
                }
            } else if ch == '\'' || ch == '"' {
                quote = Some(ch);
            } else if ch == ';' {
                return Ok(self.source[start..self.at - 1].trim().to_owned());
            }
        }
        Err(TemplateParseError::InvalidControl("@let needs `;`".into()))
    }
}

fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn is_void_tag(tag: &str) -> bool {
    let name = tag[1..]
        .split(|ch: char| ch.is_whitespace() || ch == '>')
        .next()
        .unwrap_or("");
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn expands_nested_controls_and_keeps_interpolation() {
        let result = expand("@let title = 'Hi'; @if (title == 'Hi' && true) {<p>{{ title }}</p>} @else {<p>No</p>} @for (item, index in values; track item.id) {<p>{{ item }}</p>} @match (title) { 'Hi' | 'Hey' => {<b>yes</b>}, _ => {<b>no</b>} }").unwrap();
        assert!(result.contains("<tilt-flow-let"));
        assert!(result.contains("&amp;&amp;"));
        assert!(result.contains("<tilt-flow-for"));
        assert!(result.contains("<tilt-flow-match"));
        assert!(result.contains("{{ item }}"));
    }

    #[test]
    fn supports_angular_else_if() {
        let result =
            expand("@if (one) {<p>1</p>} @else if (two) {<p>2</p>} @else {<p>3</p>}").unwrap();
        assert_eq!(result.matches("<tilt-flow-if").count(), 2);
        assert!(result.contains("<p>3</p>"));
    }

    #[test]
    fn supports_rust_style_for_and_match_headers() {
        let result = expand("@for item in items {<p>{{ item }}</p>} @match mode { 'a' => {<p>A</p>}, _ => {<p>B</p>} }").unwrap();
        assert!(result.contains("expression=\"items\""));
        assert!(result.contains("expression=\"mode\""));
    }
}
