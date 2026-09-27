//! HTML5 recovery is used once per document, or as a fallback for malformed components.

use kuchiki::{NodeRef, traits::TendrilSink};
use tilt_ui_core::{NodeId, Template, TemplateAttribute, TemplateUse};

use super::{
    TemplateParseError,
    attribute::parse_attribute,
    parser::{TableContext, append_tag, append_text, is_table_wrapper},
};

/// Head data retained from a TiltUI `index.html`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentHead {
    pub title: Option<String>,
    pub lang: Option<String>,
    pub controller: Option<String>,
    pub component_names: Vec<String>,
    pub stylesheet_links: Vec<String>,
}

/// Parsed HTML5 document; the body is a normal TiltUI template.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub head: DocumentHead,
    pub template: Template,
}

/// Parses a full document with HTML5 error recovery.
///
/// This function is intended for asset loading and build validation, never a frame system.
pub fn parse_document(source: &str) -> Result<ParsedDocument, TemplateParseError> {
    let source = super::control::expand(source)?;
    let dom = kuchiki::parse_html().one(expand_self_closing(&source));
    let mut head = DocumentHead::default();
    if let Ok(html) = dom.select_first("html") {
        head.lang = attribute(html.as_node(), "lang");
    }
    if let Ok(head_node) = dom.select_first("head") {
        for node in head_node.as_node().children() {
            let Some(element) = node.as_element() else {
                continue;
            };
            match element.name.local.as_ref() {
                "title" => {
                    let title = node.text_contents().trim().to_owned();
                    if !title.is_empty() {
                        head.title = Some(title);
                    }
                }
                "link" => {
                    if attribute(&node, "rel").is_some_and(|rel| {
                        rel.split_ascii_whitespace()
                            .any(|part| part.eq_ignore_ascii_case("stylesheet"))
                    }) && let Some(href) =
                        attribute(&node, "href").filter(|href| !href.is_empty())
                    {
                        head.stylesheet_links.push(href);
                    }
                }
                "meta" => {
                    if let Some(name) = attribute(&node, "name").filter(|name| !name.is_empty()) {
                        head.component_names.push(name);
                    }
                    if let Some(controller) =
                        attribute(&node, "controller").filter(|name| !name.is_empty())
                    {
                        head.controller = Some(controller);
                    }
                }
                _ => {}
            }
        }
    }
    let body = dom
        .select_first("body")
        .expect("HTML5 parser synthesizes a body");
    let mut template = empty_template(Vec::new());
    let mut stack = Vec::new();
    let mut tables = Vec::new();
    walk(body.as_node(), &mut template, &mut stack, &mut tables)?;
    Ok(ParsedDocument { head, template })
}

/// Converts a recovered HTML fragment into TiltUI's compact template representation.
pub(super) fn parse_html_fragment(
    source: &str,
    uses: Vec<TemplateUse>,
) -> Result<Template, TemplateParseError> {
    let html = format!(
        "<!doctype html><html><body>{}</body></html>",
        expand_self_closing(source)
    );
    let dom = kuchiki::parse_html().one(html);
    let body = dom
        .select_first("body")
        .expect("HTML5 parser synthesizes a body");
    let mut template = empty_template(uses);
    let mut stack = Vec::new();
    let mut tables = Vec::new();
    for child in body.as_node().children() {
        walk(&child, &mut template, &mut stack, &mut tables)?;
    }
    Ok(template)
}

fn empty_template(uses: Vec<TemplateUse>) -> Template {
    Template {
        roots: Vec::new(),
        nodes: Vec::new(),
        uses,
    }
}

fn attribute(node: &NodeRef, name: &str) -> Option<String> {
    node.as_element()?
        .attributes
        .borrow()
        .get(name)
        .map(str::trim)
        .map(str::to_owned)
}

fn walk(
    node: &NodeRef,
    template: &mut Template,
    stack: &mut Vec<NodeId>,
    tables: &mut Vec<TableContext>,
) -> Result<(), TemplateParseError> {
    if let Some(text) = node.as_text() {
        append_text(template, stack, text.borrow().to_string())?;
        return Ok(());
    }
    let Some(element) = node.as_element() else {
        return Ok(());
    };
    let tag = element.name.local.to_string();
    let mut attributes = element
        .attributes
        .borrow()
        .map
        .iter()
        .map(|(name, value)| parse_attribute(name.local.to_string(), value.value.clone()))
        .collect::<Result<Vec<TemplateAttribute>, _>>()?;
    if let Some(table) = tables.last_mut() {
        table.enter(&tag, &mut attributes);
    }
    let parent = if !tables.is_empty() && is_table_wrapper(&tag) {
        *stack.last().expect("table wrapper has a parent")
    } else {
        append_tag(template, stack, &tag, attributes)?
    };
    if tag == "table" {
        tables.push(TableContext::default());
    }
    stack.push(parent);
    for child in node.children() {
        walk(&child, template, stack, tables)?;
    }
    stack.pop();
    if tag == "table" {
        tables.pop();
    } else if let Some(table) = tables.last_mut() {
        table.leave(&tag);
    }
    Ok(())
}

// HTML5 treats <app-card /> as an open tag. Preserve TiltUI's existing
// self-closing convention before sending malformed input through recovery.
fn expand_self_closing(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut copied = 0;
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'<'
            || cursor + 1 == bytes.len()
            || !bytes[cursor + 1].is_ascii_alphabetic()
        {
            cursor += 1;
            continue;
        }
        let start = cursor + 1;
        let mut name_end = start;
        while name_end < bytes.len()
            && (bytes[name_end].is_ascii_alphanumeric() || bytes[name_end] == b'-')
        {
            name_end += 1;
        }
        let mut end = name_end;
        let mut quote = 0;
        while end < bytes.len() {
            let byte = bytes[end];
            if quote != 0 {
                if byte == quote {
                    quote = 0;
                }
            } else if byte == b'"' || byte == b'\'' {
                quote = byte;
            } else if byte == b'>' {
                break;
            }
            end += 1;
        }
        if end == bytes.len() {
            break;
        }
        let mut slash = end;
        while slash > name_end && bytes[slash - 1].is_ascii_whitespace() {
            slash -= 1;
        }
        if slash > name_end && bytes[slash - 1] == b'/' {
            let tag = &source[start..name_end];
            if !matches!(
                tag,
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
            ) {
                output.push_str(&source[copied..slash - 1]);
                output.push('>');
                output.push_str("</");
                output.push_str(tag);
                output.push('>');
                copied = end + 1;
            }
        }
        cursor = end + 1;
    }
    output.push_str(&source[copied..]);
    output
}

#[cfg(test)]
mod tests {
    use tilt_ui_core::TemplateNodeKind;

    use super::*;

    #[test]
    fn recovers_html5_nesting_and_preserves_head_order() {
        let parsed = parse_document(
            "<!doctype html><html lang='de'><head><title>Tilt</title><meta name='app-main'><meta controller='main'><link rel='stylesheet' href='first.css'><link rel='stylesheet' href='second.css'></head><body><p>First<p>Second<app-main /></body></html>",
        ).unwrap();
        assert_eq!(parsed.head.title.as_deref(), Some("Tilt"));
        assert_eq!(parsed.head.lang.as_deref(), Some("de"));
        assert_eq!(parsed.head.controller.as_deref(), Some("main"));
        assert_eq!(parsed.head.component_names, ["app-main"]);
        assert_eq!(parsed.head.stylesheet_links, ["first.css", "second.css"]);
        assert!(matches!(
            parsed.template.nodes[0].kind,
            TemplateNodeKind::Element(_)
        ));
        assert_eq!(parsed.template.nodes[0].children.len(), 2);
    }

    #[test]
    fn basic_document_mounts_main_in_body() {
        let document =
            parse_document(include_str!("../../../../examples/basic/src-ui/index.html")).unwrap();
        assert!(document.template.nodes.iter().any(|node| matches!(&node.kind, TemplateNodeKind::Component(name) if name.as_str() == "main")));
    }

    #[test]
    fn recovers_component_markup_only_after_fast_parse_fails() {
        let parsed = crate::parse_template("<div><p>First<p>Second<app-card /></div>").unwrap();
        assert_eq!(parsed.roots.len(), 1);
        assert!(parsed.nodes.iter().any(|node| matches!(&node.kind, TemplateNodeKind::Component(name) if name.as_str() == "app-card")));
    }
}
