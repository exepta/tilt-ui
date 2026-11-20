use bevy::{ecs::world::World, prelude::*};
use tilt_ui_core::ElementKind;
use tilt_ui_css::{MediaEnvironment, StyleSheet, parse_stylesheet};

use super::super::{SelectorView, matching_specificity};
use super::RuleIndex;
use crate::style::cascade::{Cascade, StyleOrigin, key};
use crate::{
    ComponentStyleOwner, ControlPart, ControlPartKind, ElementClasses, ElementId, ElementState,
    StaticAttribute, StaticAttributes, TiltElement,
};

fn fixture() -> (World, Entity, Entity, Entity) {
    let mut world = World::new();
    let owner = world.spawn_empty().id();
    let shell = world
        .spawn((
            TiltElement {
                kind: ElementKind::Div,
            },
            ComponentStyleOwner(owner),
            ElementClasses {
                classes: vec![
                    "card".into(),
                    "item-slot".into(),
                    "reference-slot".into(),
                    "square-player-slot".into(),
                ],
            },
            ElementState::default(),
            ChildOf(owner),
        ))
        .id();
    let button = world
        .spawn((
            TiltElement {
                kind: ElementKind::Button,
            },
            ComponentStyleOwner(owner),
            ElementId("inventory-cell".into()),
            ElementClasses {
                classes: vec!["item-cell".into(), "selected".into()],
            },
            ElementState {
                loading: true,
                ..default()
            },
            StaticAttributes {
                attributes: vec![StaticAttribute {
                    name: "data-kind".into(),
                    value: "stone".into(),
                }],
            },
            ChildOf(shell),
        ))
        .id();
    world.spawn((
        ControlPart {
            owner: button,
            kind: ControlPartKind::Spinner,
        },
        ComponentStyleOwner(owner),
        ChildOf(button),
    ));
    for class in ["item-count", "tooltip-name", "tooltip-description"] {
        world.spawn((
            TiltElement {
                kind: ElementKind::Paragraph,
            },
            ComponentStyleOwner(owner),
            ElementClasses {
                classes: vec![class.into()],
            },
            ChildOf(button),
        ));
    }
    for kind in ["head", "data"] {
        world.spawn((
            TiltElement {
                kind: ElementKind::TableCell,
            },
            ComponentStyleOwner(owner),
            StaticAttributes {
                attributes: vec![StaticAttribute {
                    name: "data-table-kind".into(),
                    value: kind.into(),
                }],
            },
            ChildOf(shell),
        ));
    }
    (world, owner, shell, button)
}

fn matches(
    sheet: &StyleSheet,
    index: &RuleIndex,
    view: &SelectorView,
    entity: Entity,
    environment: MediaEnvironment,
    candidates: &[usize],
) -> Vec<(usize, tilt_ui_css::Specificity)> {
    candidates
        .iter()
        .filter_map(|ordinal| {
            let rule = index.rule(sheet, *ordinal, environment)?;
            matching_specificity(view, entity, &rule.selectors)
                .map(|specificity| (*ordinal, specificity))
        })
        .collect()
}

fn cascade(
    sheet: &StyleSheet,
    index: &RuleIndex,
    environment: MediaEnvironment,
    matches: &[(usize, tilt_ui_css::Specificity)],
) -> crate::CascadedStyle {
    let mut cascade = Cascade::default();
    for (ordinal, specificity) in matches {
        let rule = index.rule(sheet, *ordinal, environment).unwrap();
        for (order, declaration) in rule.declarations.iter().enumerate() {
            cascade.apply(
                declaration,
                key(
                    StyleOrigin::Document,
                    *specificity,
                    rule.source_order,
                    order,
                ),
            );
        }
    }
    cascade.resolve_deferred(|_| tilt_ui_css::ValueContext::default());
    cascade.finish()
}

fn assert_equivalent(sheet: &StyleSheet, view: &SelectorView, environment: MediaEnvironment) {
    let index = RuleIndex::new(sheet);
    let all = (0..index.rules.len()).collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for entity in view.entities() {
        index.candidates(view, entity, &mut candidates);
        let exhaustive = matches(sheet, &index, view, entity, environment, &all);
        let indexed = matches(sheet, &index, view, entity, environment, &candidates);
        assert_eq!(
            indexed, exhaustive,
            "Selector specificity differs for {entity:?}"
        );
        assert_eq!(
            cascade(sheet, &index, environment, &indexed),
            cascade(sheet, &index, environment, &exhaustive)
        );
    }
}

#[test]
fn indexed_document_rules_preserve_complete_matching_cascade_and_live_metadata() {
    let sheet = parse_stylesheet(
        r#"
        * { color: white; }
        .unrelated, #elsewhere { color: red; }
        .item-cell, #inventory-cell { color: blue; }
        DIV.item-slot > BUTTON.item-cell { padding: 2px; }
        .card:hover .item-cell { color: green !important; }
        .item-cell + p { color: yellow; }
        p ~ p.tooltip-description { color: purple; }
        [data-kind="stone"] { background-color: black; }
        [class~="selected"] { width: 22px; }
        :not(.selected) { border-width: 1px; }
        p:nth-child(2n + 1) { margin-left: 3px; }
        button:disabled { padding: 5px; }
        button:loading::spinner { width: 16px; }
        button::spinner, #inventory-cell { height: 17px; }
        th { min-width: 15px; }
        td { min-width: 16px; }
        @media (max-width: 700px) { .item-cell { width: 31px; } }
        @media (min-width: 701px) { #inventory-cell { width: 66px; } }
    "#,
    )
    .unwrap();
    let (mut world, owner, shell, button) = fixture();
    for phase in 0..4 {
        world.get_mut::<ElementState>(shell).unwrap().hovered = phase % 2 == 0;
        world.get_mut::<ElementState>(button).unwrap().disabled = phase % 2 != 0;
        world.get_mut::<ElementState>(button).unwrap().loading = phase < 2;
        world.get_mut::<ElementClasses>(button).unwrap().classes = if phase < 2 {
            vec!["item-cell".into(), "selected".into()]
        } else {
            vec!["item-cell".into()]
        };
        world.get_mut::<ElementId>(button).unwrap().0 = if phase == 3 {
            "changed"
        } else {
            "inventory-cell"
        }
        .into();
        world
            .get_mut::<StaticAttributes>(button)
            .unwrap()
            .attributes[0]
            .value = if phase == 2 { "dirt" } else { "stone" }.into();
        let view = SelectorView::build(&mut world, owner);
        for width in [640.0, 1280.0] {
            assert_equivalent(
                &sheet,
                &view,
                MediaEnvironment {
                    width,
                    height: 800.0,
                },
            );
        }
    }
}

#[test]
fn index_rebuild_on_stylesheet_replacement_keeps_rule_order_and_group_specificity() {
    let (mut world, owner, _, _) = fixture();
    let view = SelectorView::build(&mut world, owner);
    for source in [
        ".item-cell { width: 10px; } #inventory-cell, .selected { width: 20px; }",
        "#inventory-cell { width: 30px; } .item-cell { width: 40px !important; }",
    ] {
        let mut sheet = parse_stylesheet(source).unwrap();
        // Preserve traversal order even for a manually constructed equal-order pair.
        sheet.rules[1].source_order = sheet.rules[0].source_order;
        assert_equivalent(
            &sheet,
            &view,
            MediaEnvironment {
                width: 1280.0,
                height: 800.0,
            },
        );
    }
}

#[test]
#[ignore = "requires TILT_UI_INDEX_CSS_FIXTURE pointing to a merged application document stylesheet"]
fn application_document_css_matches_exhaustive_and_reduces_cold_candidate_work() {
    let source =
        std::fs::read_to_string(std::env::var("TILT_UI_INDEX_CSS_FIXTURE").unwrap()).unwrap();
    let sheet = parse_stylesheet(&source).unwrap();
    let (mut world, owner, _, _) = fixture();
    let view = SelectorView::build(&mut world, owner);
    let environment = MediaEnvironment {
        width: 1280.0,
        height: 800.0,
    };
    assert_equivalent(&sheet, &view, environment);
    let index = RuleIndex::new(&sheet);
    let all = (0..index.rules.len()).collect::<Vec<_>>();
    let mut candidates = Vec::new();
    let exhaustive_work = all.len() * view.entities().count();
    let mut indexed_work = 0;
    for entity in view.entities() {
        index.candidates(&view, entity, &mut candidates);
        indexed_work += candidates.len();
    }
    assert!(
        indexed_work * 5 < exhaustive_work,
        "Application fixture must prune most full selector matches"
    );
    let started = std::time::Instant::now();
    for _ in 0..30 {
        for entity in view.entities() {
            std::hint::black_box(matches(&sheet, &index, &view, entity, environment, &all));
        }
    }
    let exhaustive_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = std::time::Instant::now();
    for _ in 0..30 {
        for entity in view.entities() {
            index.candidates(&view, entity, &mut candidates);
            std::hint::black_box(matches(
                &sheet,
                &index,
                &view,
                entity,
                environment,
                &candidates,
            ));
        }
    }
    let indexed_ms = started.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "O09 candidate benchmark rules={} nodes={} exhaustive_candidates={} indexed_candidates={} exhaustive_ms={exhaustive_ms:.3} indexed_ms={indexed_ms:.3}",
        index.rules.len(),
        view.entities().count(),
        exhaustive_work,
        indexed_work
    );
}
