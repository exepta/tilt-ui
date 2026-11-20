//! Conservative candidate selection for immutable document stylesheets.

use std::{borrow::Borrow, collections::HashMap};

use bevy::ecs::entity::Entity;
use selectors::parser::Component;
use tilt_ui_css::{MediaEnvironment, StyleRule, StyleSheet};

use super::SelectorView;

#[derive(Clone, Copy)]
enum RuleLocation {
    Base(usize),
    Media(usize, usize),
}

#[cfg(test)]
#[path = "rule_index_tests.rs"]
mod tests;

/// Prunes rules only; Servo still evaluates complete selectors and specificity.
///
/// Each selector contributes one required key from its rightmost compound. Rules
/// with no such key and all pseudo-element selectors remain in fallback buckets.
/// The index belongs to one immutable stylesheet, so asset replacement rebuilds
/// it and viewport/state/metadata changes never reuse a computed style result.
#[derive(Default)]
pub(crate) struct RuleIndex {
    rules: Vec<RuleLocation>,
    ids: HashMap<String, Vec<usize>>,
    classes: HashMap<String, Vec<usize>>,
    tags: HashMap<String, Vec<usize>>,
    fallback: Vec<usize>,
    pseudo_fallback: Vec<usize>,
}

impl RuleIndex {
    pub(crate) fn new(sheet: &StyleSheet) -> Self {
        let mut index = Self::default();
        for (rule, value) in sheet.rules().iter().enumerate() {
            index.insert(RuleLocation::Base(rule), value);
        }
        for (media, block) in sheet.media_rules().iter().enumerate() {
            for (rule, value) in block.rules.iter().enumerate() {
                index.insert(RuleLocation::Media(media, rule), value);
            }
        }
        index
    }

    fn insert(&mut self, location: RuleLocation, rule: &StyleRule) {
        let ordinal = self.rules.len();
        self.rules.push(location);
        for selector in rule.selectors.slice() {
            if selector.has_pseudo_element() {
                self.pseudo_fallback.push(ordinal);
                continue;
            }
            let mut class: Option<&str> = None;
            let mut tag: Option<&str> = None;
            let mut id: Option<&str> = None;
            // iter() stops at the first combinator; ancestor/sibling keys are
            // not requirements on the element currently being matched.
            for component in selector.iter() {
                match component {
                    Component::ID(value) => {
                        id = Some(value.borrow());
                        break;
                    }
                    Component::Class(value) => class = Some(value.borrow()),
                    Component::LocalName(value) => tag = Some(value.lower_name.borrow()),
                    _ => {}
                }
            }
            if let Some(id) = id {
                self.ids.entry(String::from(id)).or_default().push(ordinal);
            } else if let Some(class) = class {
                self.classes
                    .entry(String::from(class))
                    .or_default()
                    .push(ordinal);
            } else if let Some(tag) = tag {
                self.tags
                    .entry(String::from(tag))
                    .or_default()
                    .push(ordinal);
            } else {
                self.fallback.push(ordinal);
            }
        }
    }

    pub(crate) fn candidates(&self, view: &SelectorView, entity: Entity, out: &mut Vec<usize>) {
        out.clear();
        let Some(index) = view.by_entity.get(&entity) else {
            return;
        };
        let node = &view.nodes[*index];
        if node.part.is_some() {
            out.extend_from_slice(&self.pseudo_fallback);
        } else {
            out.extend_from_slice(&self.fallback);
            if let Some(id) = node.id.as_deref().and_then(|id| self.ids.get(id)) {
                out.extend_from_slice(id);
            }
            for class in &node.classes {
                if let Some(rules) = self.classes.get(class) {
                    out.extend_from_slice(rules);
                }
            }
            if let Some(rules) = self.tags.get(node.tag) {
                out.extend_from_slice(rules);
            }
            // TiltUI's native table-cell also matches authored th/td tags.
            if node.tag == "table-cell" {
                let alias = node.attributes.iter().find_map(|(name, value)| {
                    (name == "data-table-kind").then_some(value.as_str())
                });
                let alias = match alias {
                    Some("head") => Some("th"),
                    Some("data") => Some("td"),
                    _ => None,
                };
                if let Some(rules) = alias.and_then(|alias| self.tags.get(alias)) {
                    out.extend_from_slice(rules);
                }
            }
        }
        // A selector list can contribute several keys for the same rule. Keep
        // authored traversal order even when manually built rules tie in source order.
        out.sort_unstable();
        out.dedup();
    }

    pub(crate) fn rule<'a>(
        &self,
        sheet: &'a StyleSheet,
        ordinal: usize,
        environment: MediaEnvironment,
    ) -> Option<&'a StyleRule> {
        match self.rules[ordinal] {
            RuleLocation::Base(rule) => sheet.rules().get(rule),
            RuleLocation::Media(media, rule) => {
                let block = &sheet.media_rules()[media];
                block.matches(environment).then(|| &block.rules[rule])
            }
        }
    }
}
