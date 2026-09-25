use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};

use bevy::ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    world::World,
};
use selectors::{
    Element, OpaqueElement,
    attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint},
    bloom::BloomFilter,
    context::{
        MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode, SelectorCaches,
    },
    matching::{ElementSelectorFlags, matches_selector},
};
use tilt_ui_css::{
    ParsedSelectorList, SelectorAttributeValue, SelectorName, TiltUiPseudoClass,
    TiltUiPseudoElement, TiltUiSelectorImpl,
};

use crate::{
    ComponentStyleOwner, ControlPart, ControlPartKind, ElementClasses, ElementId, ElementState,
    StaticAttributes, TiltElement,
};

#[derive(Debug)]
struct ViewNode {
    entity: Entity,
    tag: &'static str,
    id: Option<String>,
    classes: Vec<String>,
    attributes: Vec<(String, String)>,
    state: ElementState,
    part: Option<ControlPartKind>,
    control_owner: Option<Entity>,
    parent: Option<usize>,
    first_child: Option<usize>,
    previous: Option<usize>,
    next: Option<usize>,
}

/// Immutable selector metadata built once for a dirty component scope.
///
/// Servo's `Element` trait returns owned parent and sibling adapters. This
/// compact view supplies stable relationships without holding ECS borrows for
/// the complete matching pass and is discarded immediately afterwards.
#[derive(Debug)]
pub(crate) struct SelectorView {
    nodes: Vec<ViewNode>,
    by_entity: HashMap<Entity, usize>,
}

impl SelectorView {
    pub(crate) fn build(world: &mut World, owner: Entity) -> Self {
        let mut nodes = Vec::new();
        let mut by_entity = HashMap::new();
        let mut query = world.query::<(
            Entity,
            &TiltElement,
            &ComponentStyleOwner,
            Option<&ElementId>,
            Option<&ElementClasses>,
            Option<&StaticAttributes>,
            Option<&ElementState>,
        )>();
        for (entity, element, style_owner, id, classes, attributes, state) in query.iter(world) {
            if style_owner.0 != owner {
                continue;
            }
            by_entity.insert(entity, nodes.len());
            nodes.push(ViewNode {
                entity,
                tag: element.kind.tag_name(),
                id: id.map(|id| id.0.clone()),
                classes: classes.map_or_else(Vec::new, |classes| classes.classes.clone()),
                attributes: attributes.map_or_else(Vec::new, |attrs| {
                    attrs
                        .attributes
                        .iter()
                        .map(|attribute| (attribute.name.clone(), attribute.value.clone()))
                        .collect()
                }),
                state: state.copied().unwrap_or_default(),
                part: None,
                control_owner: None,
                parent: None,
                first_child: None,
                previous: None,
                next: None,
            });
        }
        let mut parts = world.query::<(
            Entity,
            &ControlPart,
            &ComponentStyleOwner,
            Option<&ElementState>,
        )>();
        for (entity, part, style_owner, state) in parts.iter(world) {
            if style_owner.0 != owner {
                continue;
            }
            let mut control_owner = part.owner;
            while let Some(parent_part) = world.get::<ControlPart>(control_owner) {
                control_owner = parent_part.owner;
            }
            let Some(owner_index) = by_entity.get(&control_owner).copied() else {
                continue;
            };
            let Some(control) = world.get::<TiltElement>(control_owner) else {
                continue;
            };
            let control_state = state
                .copied()
                .or_else(|| world.get::<ElementState>(control_owner).copied())
                .unwrap_or_default();
            by_entity.insert(entity, nodes.len());
            nodes.push(ViewNode {
                entity,
                tag: control.kind.tag_name(),
                id: None,
                classes: Vec::new(),
                attributes: Vec::new(),
                state: control_state,
                part: Some(part.kind),
                control_owner: Some(control_owner),
                parent: Some(owner_index),
                first_child: None,
                previous: None,
                next: None,
            });
        }
        for node in &mut nodes {
            if node.part.is_some() {
                continue;
            }
            let mut current = node.entity;
            while let Some(parent) = world.get::<ChildOf>(current).map(|parent| parent.0) {
                if let Some(parent_index) = by_entity.get(&parent).copied() {
                    node.parent = Some(parent_index);
                    break;
                }
                current = parent;
            }
        }
        let order_keys = nodes
            .iter()
            .map(|node| document_order_key(world, node.entity, owner))
            .collect::<Vec<_>>();
        let mut siblings_by_parent: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
        for (index, node) in nodes.iter().enumerate() {
            siblings_by_parent
                .entry(node.parent)
                .or_default()
                .push(index);
        }
        for (parent, mut siblings) in siblings_by_parent {
            siblings.sort_by(|left, right| order_keys[*left].cmp(&order_keys[*right]));
            if let Some(parent) = parent {
                nodes[parent].first_child = siblings.first().copied();
            }
            for pair in siblings.windows(2) {
                nodes[pair[0]].next = Some(pair[1]);
                nodes[pair[1]].previous = Some(pair[0]);
            }
        }
        Self { nodes, by_entity }
    }
    pub(crate) fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.nodes.iter().map(|node| node.entity)
    }
    pub(crate) fn refresh(&mut self, world: &World, changed: &HashSet<Entity>) -> bool {
        for entity in changed {
            let Some(&index) = self.by_entity.get(entity) else {
                return false;
            };
            let node = &mut self.nodes[index];
            if node.part.is_some() {
                node.state = world
                    .get::<ElementState>(*entity)
                    .copied()
                    .or_else(|| {
                        node.control_owner
                            .and_then(|owner| world.get::<ElementState>(owner).copied())
                    })
                    .unwrap_or_default();
            } else {
                node.id = world.get::<ElementId>(*entity).map(|id| id.0.clone());
                node.classes = world
                    .get::<ElementClasses>(*entity)
                    .map_or_else(Vec::new, |classes| classes.classes.clone());
                node.attributes =
                    world
                        .get::<StaticAttributes>(*entity)
                        .map_or_else(Vec::new, |attrs| {
                            attrs
                                .attributes
                                .iter()
                                .map(|attribute| (attribute.name.clone(), attribute.value.clone()))
                                .collect()
                        });
                node.state = world
                    .get::<ElementState>(*entity)
                    .copied()
                    .unwrap_or_default();
            }
        }
        for node in &mut self.nodes {
            if node
                .control_owner
                .is_some_and(|owner| changed.contains(&owner))
                && world.get::<ElementState>(node.entity).is_none()
            {
                node.state = node
                    .control_owner
                    .and_then(|owner| world.get::<ElementState>(owner).copied())
                    .unwrap_or_default();
            }
        }
        true
    }
    pub(crate) fn affected_by(&self, changed: &HashSet<Entity>) -> Vec<Entity> {
        let indices = changed
            .iter()
            .filter_map(|entity| self.by_entity.get(entity).copied())
            .collect::<HashSet<_>>();
        if indices.len() != changed.len() {
            return self.entities().collect();
        }
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                indices
                    .iter()
                    .any(|changed| self.can_depend_on(index, *changed))
                    .then_some(node.entity)
            })
            .collect()
    }
    fn can_depend_on(&self, index: usize, changed: usize) -> bool {
        let mut ancestor = Some(index);
        while let Some(index) = ancestor {
            let mut sibling = Some(index);
            while let Some(index) = sibling {
                if index == changed {
                    return true;
                }
                sibling = self.nodes[index].previous;
            }
            ancestor = self.nodes[index].parent;
        }
        false
    }
    pub(crate) fn element(&self, entity: Entity) -> Option<SelectorElement<'_>> {
        self.by_entity
            .get(&entity)
            .copied()
            .map(|index| SelectorElement { view: self, index })
    }
}

fn document_order_key(world: &World, entity: Entity, owner: Entity) -> Vec<usize> {
    let mut key = Vec::new();
    let mut current = entity;
    while current != owner {
        let Some(parent) = world.get::<ChildOf>(current).map(|parent| parent.0) else {
            break;
        };
        let position = world
            .get::<Children>(parent)
            .and_then(|children| children.iter().position(|child| *child == current))
            .unwrap_or(usize::MAX);
        key.push(position);
        current = parent;
    }
    key.reverse();
    key
}

#[derive(Clone, Debug)]
pub(crate) struct SelectorElement<'a> {
    view: &'a SelectorView,
    index: usize,
}

impl SelectorElement<'_> {
    fn node(&self) -> &ViewNode {
        &self.view.nodes[self.index]
    }
    fn attribute(&self, name: &str) -> Option<String> {
        let node = self.node();
        match name {
            "id" => node.id.clone(),
            "class" => (!node.classes.is_empty()).then(|| node.classes.join(" ")),
            _ => node
                .attributes
                .iter()
                .find(|(attribute, _)| attribute == name)
                .map(|(_, value)| value.clone()),
        }
    }
}

impl Element for SelectorElement<'_> {
    type Impl = TiltUiSelectorImpl;
    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(self.node())
    }
    fn parent_element(&self) -> Option<Self> {
        self.node().parent.map(|index| Self {
            view: self.view,
            index,
        })
    }
    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }
    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }
    fn is_pseudo_element(&self) -> bool {
        self.node().part.is_some()
    }
    fn prev_sibling_element(&self) -> Option<Self> {
        self.node().previous.map(|index| Self {
            view: self.view,
            index,
        })
    }
    fn next_sibling_element(&self) -> Option<Self> {
        self.node().next.map(|index| Self {
            view: self.view,
            index,
        })
    }
    fn first_element_child(&self) -> Option<Self> {
        self.node().first_child.map(|index| Self {
            view: self.view,
            index,
        })
    }
    fn is_html_element_in_html_document(&self) -> bool {
        true
    }
    fn has_local_name(&self, local_name: &str) -> bool {
        self.node().tag.eq_ignore_ascii_case(local_name)
            || (self.node().tag == "table-cell"
                && matches!(local_name, "th" | "td")
                && self.attribute("data-table-kind")
                    == Some(if local_name == "th" { "head" } else { "data" }.to_owned()))
    }
    fn has_namespace(&self, _ns: &str) -> bool {
        true
    }
    fn is_same_type(&self, other: &Self) -> bool {
        self.node().tag == other.node().tag
    }
    fn attr_matches(
        &self,
        _ns: &NamespaceConstraint<&SelectorName>,
        local_name: &SelectorName,
        operation: &AttrSelectorOperation<&SelectorAttributeValue>,
    ) -> bool {
        self.attribute(local_name.borrow())
            .is_some_and(|value| operation.eval_str(&value))
    }
    fn match_non_ts_pseudo_class(
        &self,
        pseudo: &TiltUiPseudoClass,
        _context: &mut selectors::matching::MatchingContext<TiltUiSelectorImpl>,
    ) -> bool {
        match pseudo {
            TiltUiPseudoClass::Hover => self.node().state.hovered,
            TiltUiPseudoClass::Active => self.node().state.active,
            TiltUiPseudoClass::Focus => self.node().state.focused,
            TiltUiPseudoClass::Disabled => self.node().state.disabled,
            TiltUiPseudoClass::Checked => self.node().state.checked,
            TiltUiPseudoClass::Readonly => self.node().state.readonly,
            TiltUiPseudoClass::Invalid => self.node().state.invalid,
            TiltUiPseudoClass::Open => self.node().state.open,
        }
    }
    fn match_pseudo_element(
        &self,
        pseudo: &TiltUiPseudoElement,
        _context: &mut selectors::matching::MatchingContext<TiltUiSelectorImpl>,
    ) -> bool {
        if self.node().part == Some(ControlPartKind::CalendarDay) {
            match pseudo {
                TiltUiPseudoElement::HoveredCalendarDay => return self.node().state.hovered,
                TiltUiPseudoElement::SelectedCalendarDay => return self.node().state.checked,
                TiltUiPseudoElement::DisabledCalendarDay => return self.node().state.disabled,
                _ => {}
            }
        }
        if self.node().part == Some(ControlPartKind::ColorFormat)
            && *pseudo == TiltUiPseudoElement::SelectedColorFormat
        {
            return self.node().state.checked;
        }
        matches!(
            (self.node().part, pseudo),
            (
                Some(ControlPartKind::Indicator),
                TiltUiPseudoElement::Indicator
            ) | (Some(ControlPartKind::Mark), TiltUiPseudoElement::Mark)
                | (Some(ControlPartKind::Track), TiltUiPseudoElement::Track)
                | (Some(ControlPartKind::Thumb), TiltUiPseudoElement::Thumb)
                | (Some(ControlPartKind::Fill), TiltUiPseudoElement::Fill)
                | (Some(ControlPartKind::Value), TiltUiPseudoElement::Value)
                | (
                    Some(ControlPartKind::Placeholder),
                    TiltUiPseudoElement::Placeholder
                )
                | (Some(ControlPartKind::Cursor), TiltUiPseudoElement::Cursor)
                | (Some(ControlPartKind::Dot), TiltUiPseudoElement::Dot)
                | (Some(ControlPartKind::Label), TiltUiPseudoElement::Label)
                | (Some(ControlPartKind::Tooltip), TiltUiPseudoElement::Tooltip)
                | (Some(ControlPartKind::Popup), TiltUiPseudoElement::Popup)
                | (
                    Some(ControlPartKind::Calendar),
                    TiltUiPseudoElement::Calendar
                )
                | (
                    Some(ControlPartKind::CalendarHeader),
                    TiltUiPseudoElement::CalendarHeader
                )
                | (
                    Some(ControlPartKind::CalendarPrevious),
                    TiltUiPseudoElement::CalendarPrevious
                )
                | (
                    Some(ControlPartKind::CalendarNext),
                    TiltUiPseudoElement::CalendarNext
                )
                | (
                    Some(ControlPartKind::CalendarDay),
                    TiltUiPseudoElement::CalendarDay
                )
                | (Some(ControlPartKind::Preview), TiltUiPseudoElement::Preview)
                | (Some(ControlPartKind::Swatch), TiltUiPseudoElement::Swatch)
                | (
                    Some(ControlPartKind::ColorCanvas),
                    TiltUiPseudoElement::ColorCanvas
                )
                | (
                    Some(ControlPartKind::ColorCanvasThumb),
                    TiltUiPseudoElement::ColorCanvasThumb
                )
                | (
                    Some(ControlPartKind::HueTrack),
                    TiltUiPseudoElement::HueTrack
                )
                | (
                    Some(ControlPartKind::HueThumb),
                    TiltUiPseudoElement::HueThumb
                )
                | (
                    Some(ControlPartKind::AlphaTrack),
                    TiltUiPseudoElement::AlphaTrack
                )
                | (
                    Some(ControlPartKind::AlphaThumb),
                    TiltUiPseudoElement::AlphaThumb
                )
                | (
                    Some(ControlPartKind::ColorFormat),
                    TiltUiPseudoElement::ColorFormat
                )
                | (
                    Some(ControlPartKind::ColorFormats),
                    TiltUiPseudoElement::ColorFormats
                )
                | (
                    Some(ControlPartKind::ColorSwatches),
                    TiltUiPseudoElement::ColorSwatches
                )
                | (
                    Some(ControlPartKind::RecentColors),
                    TiltUiPseudoElement::RecentColors
                )
                | (
                    Some(ControlPartKind::RecentColor),
                    TiltUiPseudoElement::RecentColor
                )
                | (
                    Some(ControlPartKind::ResizeHandle),
                    TiltUiPseudoElement::ResizeHandle
                )
                | (
                    Some(ControlPartKind::Selection),
                    TiltUiPseudoElement::Selection
                )
                | (
                    Some(ControlPartKind::ScrollbarYTrack),
                    TiltUiPseudoElement::ScrollbarYTrack
                )
                | (
                    Some(ControlPartKind::ScrollbarYThumb),
                    TiltUiPseudoElement::ScrollbarYThumb
                )
                | (
                    Some(ControlPartKind::ScrollbarXTrack),
                    TiltUiPseudoElement::ScrollbarXTrack
                )
                | (
                    Some(ControlPartKind::ScrollbarXThumb),
                    TiltUiPseudoElement::ScrollbarXThumb
                )
        )
    }
    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}
    fn is_link(&self) -> bool {
        false
    }
    fn is_html_slot_element(&self) -> bool {
        false
    }
    fn has_id(&self, id: &SelectorName, sensitivity: CaseSensitivity) -> bool {
        let id: &str = id.borrow();
        self.node()
            .id
            .as_ref()
            .is_some_and(|value| sensitivity.eq(value.as_bytes(), id.as_bytes()))
    }
    fn has_class(&self, class: &SelectorName, sensitivity: CaseSensitivity) -> bool {
        let class: &str = class.borrow();
        self.node()
            .classes
            .iter()
            .any(|value| sensitivity.eq(value.as_bytes(), class.as_bytes()))
    }
    fn has_custom_state(&self, _name: &SelectorName) -> bool {
        false
    }
    fn imported_part(&self, _name: &SelectorName) -> Option<SelectorName> {
        None
    }
    fn is_part(&self, _name: &SelectorName) -> bool {
        false
    }
    fn is_empty(&self) -> bool {
        !self
            .view
            .nodes
            .iter()
            .any(|node| node.parent == Some(self.index))
    }
    fn is_root(&self) -> bool {
        self.node().parent.is_none()
    }
    fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
        false
    }
}

pub(crate) fn matching_specificity(
    view: &SelectorView,
    entity: Entity,
    selectors: &ParsedSelectorList,
) -> Option<tilt_ui_css::Specificity> {
    let element = view.element(entity)?;
    let is_part = element.node().part.is_some();
    let mut caches = SelectorCaches::default();
    let mut context = selectors::matching::MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    selectors
        .slice()
        .iter()
        .filter_map(|selector| {
            (selector.has_pseudo_element() == is_part
                && matches_selector(selector, 0, None, &element, &mut context))
            .then_some(tilt_ui_css::Specificity(selector.specificity()))
        })
        .max()
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::World;
    use tilt_ui_core::ElementKind;
    use tilt_ui_css::parse_stylesheet;

    use super::{SelectorView, matching_specificity};
    use crate::{ComponentStyleOwner, ControlPart, ControlPartKind, ElementState, TiltElement};

    #[test]
    fn generated_parts_match_only_explicit_pseudo_element_selectors() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let input = world
            .spawn((
                TiltElement {
                    kind: ElementKind::TextArea,
                },
                ComponentStyleOwner(scope),
            ))
            .id();
        let handle = world
            .spawn((
                ControlPart {
                    owner: input,
                    kind: ControlPartKind::ResizeHandle,
                },
                ComponentStyleOwner(scope),
            ))
            .id();
        world.entity_mut(scope).add_child(input);
        world.entity_mut(input).add_child(handle);
        let view = SelectorView::build(&mut world, scope);
        let sheet = parse_stylesheet(
            "textarea { min-width: 240px; } textarea::resize-handle { width: 20px; }",
        )
        .unwrap();
        assert!(matching_specificity(&view, input, &sheet.rules()[0].selectors).is_some());
        assert!(matching_specificity(&view, handle, &sheet.rules()[0].selectors).is_none());
        assert!(matching_specificity(&view, handle, &sheet.rules()[1].selectors).is_some());
        assert!(matching_specificity(&view, input, &sheet.rules()[1].selectors).is_none());
    }

    #[test]
    fn scrollbar_inside_a_popup_inherits_its_control_selector() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let choice = world
            .spawn((
                TiltElement {
                    kind: ElementKind::ChoiceBox,
                },
                ComponentStyleOwner(scope),
            ))
            .id();
        let popup = world
            .spawn((
                ControlPart {
                    owner: choice,
                    kind: ControlPartKind::Popup,
                },
                ComponentStyleOwner(scope),
            ))
            .id();
        let track = world
            .spawn((
                ControlPart {
                    owner: popup,
                    kind: ControlPartKind::ScrollbarYTrack,
                },
                ComponentStyleOwner(scope),
            ))
            .id();
        world.entity_mut(scope).add_child(choice);
        world.entity_mut(choice).add_child(popup);
        world.entity_mut(popup).add_child(track);

        let view = SelectorView::build(&mut world, scope);
        let sheet = parse_stylesheet(
            "choice-box::scrollbar-y-track { width: 10px; } *::scrollbar-y-track { height: 100%; }",
        )
        .unwrap();
        assert!(matching_specificity(&view, track, &sheet.rules()[0].selectors).is_some());
        assert!(matching_specificity(&view, track, &sheet.rules()[1].selectors).is_some());
    }

    #[test]
    fn calendar_day_state_matches_before_its_pseudo_element() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let picker = world
            .spawn((
                TiltElement {
                    kind: ElementKind::DatePicker,
                },
                ComponentStyleOwner(scope),
                ElementState::default(),
            ))
            .id();
        let selected = world
            .spawn((
                ControlPart {
                    owner: picker,
                    kind: ControlPartKind::CalendarDay,
                },
                ComponentStyleOwner(scope),
                ElementState {
                    checked: true,
                    ..Default::default()
                },
            ))
            .id();
        let other = world
            .spawn((
                ControlPart {
                    owner: picker,
                    kind: ControlPartKind::CalendarDay,
                },
                ComponentStyleOwner(scope),
                ElementState::default(),
            ))
            .id();
        world.entity_mut(scope).add_child(picker);
        world
            .entity_mut(picker)
            .add_child(selected)
            .add_child(other);
        let view = SelectorView::build(&mut world, scope);
        let sheet =
            parse_stylesheet("date-picker::selected-day { background-color: #A833EA; }").unwrap();
        assert!(matching_specificity(&view, selected, &sheet.rules()[0].selectors).is_some());
        assert!(matching_specificity(&view, other, &sheet.rules()[0].selectors).is_none());
    }
}
