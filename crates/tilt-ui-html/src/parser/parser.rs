use quick_xml::{
    Reader, XmlVersion,
    escape::unescape,
    events::{BytesStart, Event},
};
use tilt_ui_core::{
    ComponentName, NodeId, Template, TemplateAttribute, TemplateNode, TemplateNodeKind,
};

use crate::mapping::map_element;

use super::{TemplateParseError, attribute::parse_attribute};

/// Parses a TiltUI component template into its core representation.
///
/// The parser reads XML-like events directly into a flat `Template` and does
/// not perform component discovery, expression evaluation, or runtime work.
pub fn parse_template(source: &str) -> Result<Template, TemplateParseError> {
    let mut reader = Reader::from_str(source);
    let mut template = Template {
        roots: Vec::new(),
        nodes: Vec::new(),
    };
    let mut stack = Vec::new();
    let mut open_tags = Vec::new();
    let mut tables = Vec::<TableContext>::new();

    loop {
        match reader.read_event()? {
            Event::Start(event) => {
                let tag = event.name().as_ref().to_owned();
                let mut attributes = parse_attributes(&event)?;
                if let Some(table) = tables.last_mut() {
                    table.enter(&tag, &mut attributes);
                }
                let node = if tables.last().is_some() && is_table_wrapper(&tag) {
                    *stack.last().expect("table wrapper has a parent")
                } else {
                    append_tag(&mut template, &stack, &tag, attributes)?
                };
                if tag == "table" {
                    tables.push(TableContext::default());
                }
                stack.push(node);
                open_tags.push(tag);
            }
            Event::Empty(event) => {
                let tag = event.name().as_ref().to_owned();
                let mut attributes = parse_attributes(&event)?;
                if let Some(table) = tables.last_mut() {
                    table.enter(&tag, &mut attributes);
                }
                if !(tables.last().is_some() && is_table_wrapper(&tag)) {
                    append_tag(&mut template, &stack, &tag, attributes)?;
                }
                if let Some(table) = tables.last_mut() {
                    table.leave(&tag);
                }
            }
            Event::End(event) => {
                let found = event.name().as_ref().to_owned();
                let expected = open_tags.pop().ok_or_else(|| {
                    TemplateParseError::ClosingTagWithoutOpenElement(found.clone())
                })?;

                if found != expected {
                    return Err(TemplateParseError::UnexpectedClosingTag { found, expected });
                }
                stack.pop();
                if found == "table" {
                    tables.pop();
                } else if let Some(table) = tables.last_mut() {
                    table.leave(&found);
                }
            }
            Event::Text(event) => append_text(
                &mut template,
                &stack,
                unescape(event.as_ref())
                    .map_err(quick_xml::Error::from)?
                    .into_owned(),
            )?,
            Event::CData(event) => append_text(&mut template, &stack, event.as_ref().to_owned())?,
            Event::GeneralRef(reference) => {
                let escaped_reference = format!("&{};", reference.as_ref());
                append_text(
                    &mut template,
                    &stack,
                    unescape(&escaped_reference)
                        .map_err(quick_xml::Error::from)?
                        .into_owned(),
                )?;
            }
            Event::Eof => {
                if let Some(tag) = open_tags.pop() {
                    return Err(TemplateParseError::UnclosedElement(tag));
                }
                return Ok(template);
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
}

fn is_table_wrapper(tag: &str) -> bool {
    matches!(tag, "tr" | "thead" | "tbody" | "tfoot")
}

#[derive(Default)]
struct TableContext {
    next_row: usize,
    current_row: Option<usize>,
    column: usize,
    section: Option<&'static str>,
}

impl TableContext {
    fn enter(&mut self, tag: &str, attributes: &mut Vec<TemplateAttribute>) {
        match tag {
            "thead" => self.section = Some("head"),
            "tbody" => self.section = Some("body"),
            "tfoot" => self.section = Some("foot"),
            "tr" => {
                self.current_row = Some(self.next_row);
                self.next_row += 1;
                self.column = 0;
            }
            "th" | "td" | "table-cell" => {
                let row = *self.current_row.get_or_insert_with(|| {
                    let row = self.next_row;
                    self.next_row += 1;
                    row
                });
                for (name, value) in [
                    ("data-table-row", row.to_string()),
                    ("data-table-column", self.column.to_string()),
                    (
                        "data-table-section",
                        self.section.unwrap_or("body").to_owned(),
                    ),
                    (
                        "data-table-kind",
                        if tag == "th" { "head" } else { "data" }.to_owned(),
                    ),
                ] {
                    attributes.push(TemplateAttribute::Static {
                        name: name.into(),
                        value,
                    });
                }
                self.column += 1;
            }
            _ => {}
        }
    }

    fn leave(&mut self, tag: &str) {
        match tag {
            "tr" => self.current_row = None,
            "thead" | "tbody" | "tfoot" => self.section = None,
            _ => {}
        }
    }
}

fn append_tag(
    template: &mut Template,
    stack: &[NodeId],
    tag: &str,
    mut attributes: Vec<TemplateAttribute>,
) -> Result<NodeId, TemplateParseError> {
    if let Some(level) = tag.strip_prefix('h').and_then(|value| value.parse::<u8>().ok())
        && (1..=6).contains(&level)
        && !attributes.iter().any(|attribute| matches!(attribute, TemplateAttribute::Static { name, .. } if name == "level"))
    {
        attributes.push(TemplateAttribute::Static {
            name: "level".into(),
            value: level.to_string(),
        });
    }
    let kind = match map_element(tag) {
        Some(kind) => TemplateNodeKind::Element(kind),
        None if is_valid_component_name(tag) => {
            TemplateNodeKind::Component(ComponentName::new(tag))
        }
        None => return Err(TemplateParseError::InvalidComponentName(tag.to_owned())),
    };
    append_node(template, stack, kind, attributes)
}

fn append_text(
    template: &mut Template,
    stack: &[NodeId],
    text: String,
) -> Result<(), TemplateParseError> {
    if !text.trim().is_empty() {
        append_node(template, stack, TemplateNodeKind::Text(text), Vec::new())?;
    }
    Ok(())
}

fn append_node(
    template: &mut Template,
    stack: &[NodeId],
    kind: TemplateNodeKind,
    attributes: Vec<TemplateAttribute>,
) -> Result<NodeId, TemplateParseError> {
    let id =
        NodeId(u32::try_from(template.nodes.len()).map_err(|_| TemplateParseError::TooManyNodes)?);
    let parent = stack.last().copied();

    template.nodes.push(TemplateNode {
        kind,
        parent,
        children: Vec::new(),
        attributes,
    });

    if let Some(parent) = parent {
        template.nodes[parent.0 as usize].children.push(id);
    } else {
        template.roots.push(id);
    }
    Ok(id)
}

fn parse_attributes(event: &BytesStart<'_>) -> Result<Vec<TemplateAttribute>, TemplateParseError> {
    event
        .attributes()
        .map(|attribute| {
            let attribute = attribute.map_err(quick_xml::Error::from)?;
            let name = attribute.key.as_ref().to_owned();
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)?
                .into_owned();
            parse_attribute(name, value)
        })
        .collect()
}

/// Returns whether a name follows TiltUI's custom component tag convention.
pub fn is_valid_component_name(name: &str) -> bool {
    !name.is_empty()
        && !name.ends_with('-')
        && !name.contains("--")
        && name.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit() && index > 0
                || byte == b'-' && index > 0
        })
}

#[cfg(test)]
mod tests {
    use tilt_ui_core::{ElementKind, NodeId, TemplateAttribute, TemplateNodeKind};

    use super::parse_template;

    #[test]
    fn parses_a_builtin_element_with_text() {
        let template = parse_template("<button>Play</button>").unwrap();

        assert_eq!(template.roots(), [NodeId(0)]);
        assert!(matches!(
            template.get(NodeId(0)).map(|node| &node.kind),
            Some(TemplateNodeKind::Element(ElementKind::Button))
        ));
        assert!(matches!(
            template.get(NodeId(1)).map(|node| &node.kind),
            Some(TemplateNodeKind::Text(text)) if text == "Play"
        ));
    }

    #[test]
    fn flattens_html_table_rows_and_preserves_cell_positions() {
        let template = parse_template(
            "<table><thead><tr><th>A</th><th>B</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table>",
        ).unwrap();
        let table = template.get(NodeId(0)).unwrap();
        assert_eq!(table.children.len(), 4);
        for (index, cell_id) in table.children.iter().enumerate() {
            let cell = template.get(*cell_id).unwrap();
            assert!(matches!(
                cell.kind,
                TemplateNodeKind::Element(ElementKind::TableCell)
            ));
            let value = |name: &str| {
                cell.attributes
                    .iter()
                    .find_map(|attribute| match attribute {
                        TemplateAttribute::Static { name: key, value } if key == name => {
                            Some(value.as_str())
                        }
                        _ => None,
                    })
            };
            assert_eq!(
                value("data-table-row"),
                Some(if index < 2 { "0" } else { "1" })
            );
            assert_eq!(
                value("data-table-column"),
                Some(if index % 2 == 0 { "0" } else { "1" })
            );
            assert_eq!(
                value("data-table-kind"),
                Some(if index < 2 { "head" } else { "data" })
            );
        }
    }

    #[test]
    fn parses_native_widget_aliases_without_treating_them_as_components() {
        let template = parse_template(
            "<select><option value=\"low\">Low</option></select><fieldset><radio>Low</radio></fieldset>",
        )
        .unwrap();

        assert!(matches!(
            template.get(NodeId(0)).map(|node| &node.kind),
            Some(TemplateNodeKind::Element(ElementKind::ChoiceBox))
        ));
        assert!(matches!(
            template.get(NodeId(1)).map(|node| &node.kind),
            Some(TemplateNodeKind::Element(ElementKind::Option))
        ));
        assert!(matches!(
            template.get(NodeId(3)).map(|node| &node.kind),
            Some(TemplateNodeKind::Element(ElementKind::FieldSet))
        ));
        assert!(matches!(
            template.get(NodeId(4)).map(|node| &node.kind),
            Some(TemplateNodeKind::Element(ElementKind::RadioButton))
        ));
    }

    #[test]
    fn parses_nested_elements_without_formatting_text() {
        let template = parse_template("<div>\n    <button>Play</button>\n</div>").unwrap();

        assert_eq!(template.nodes().len(), 3);
        assert_eq!(template.get(NodeId(0)).unwrap().children, [NodeId(1)]);
        assert_eq!(template.get(NodeId(1)).unwrap().children, [NodeId(2)]);
    }

    #[test]
    fn parses_multiple_roots() {
        let template = parse_template("<h1>Settings</h1><p>Description</p>").unwrap();

        assert_eq!(template.roots(), [NodeId(0), NodeId(2)]);
    }

    #[test]
    fn parses_a_custom_component() {
        let template = parse_template("<app-header />").unwrap();

        assert!(matches!(
            template.get(NodeId(0)).map(|node| &node.kind),
            Some(TemplateNodeKind::Component(name)) if name.as_str() == "app-header"
        ));
    }

    #[test]
    fn parses_attribute_forms() {
        let static_template = parse_template("<div class=\"menu\" />").unwrap();
        let property_template = parse_template("<button [disabled]=\"loading\" />").unwrap();
        let event_template = parse_template("<button (click)=\"start_game()\" />").unwrap();

        assert!(matches!(
            static_template.get(NodeId(0)).unwrap().attributes.as_slice(),
            [TemplateAttribute::Static { name, value }] if name == "class" && value == "menu"
        ));
        assert!(matches!(
            property_template.get(NodeId(0)).unwrap().attributes.as_slice(),
            [TemplateAttribute::PropertyBinding { name, expression }]
                if name == "disabled" && expression == "loading"
        ));
        assert!(matches!(
            event_template.get(NodeId(0)).unwrap().attributes.as_slice(),
            [TemplateAttribute::EventBinding { name, expression }]
                if name == "click" && expression == "start_game()"
        ));
    }

    #[test]
    fn parses_a_combined_component_template() {
        let template = parse_template(
            r#"<div class="menu">
                <app-header />
                <button [disabled]="loading" (click)="start_game()">Start</button>
            </div>"#,
        )
        .unwrap();

        assert_eq!(template.nodes().len(), 4);
        assert_eq!(
            template.get(NodeId(0)).unwrap().children,
            [NodeId(1), NodeId(2)]
        );
        assert_eq!(template.get(NodeId(2)).unwrap().children, [NodeId(3)]);
    }

    #[test]
    fn parses_self_closing_nodes() {
        let template = parse_template("<img />").unwrap();

        assert!(matches!(
            template.get(NodeId(0)).map(|node| &node.kind),
            Some(TemplateNodeKind::Element(ElementKind::Image))
        ));
    }

    #[test]
    fn rejects_invalid_nesting() {
        assert!(parse_template("<div><button></div>").is_err());
    }
}
