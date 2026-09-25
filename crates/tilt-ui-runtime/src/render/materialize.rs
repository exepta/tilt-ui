use bevy::{
    asset::AssetServer,
    ecs::{entity::Entity, world::World},
    ui::{
        Checkable, Checked, FocusPolicy, Interaction, InteractionDisabled, Node,
        widget::{Button, Text},
    },
};
use bevy_input_focus::tab_navigation::TabIndex;
use tilt_ui_core::{ElementKind, TemplateAttribute};

use crate::{
    ControlChecked, ControlTabIndex, ElementState, FieldSetSelection, TiltButton, TiltCheckbox,
    TiltControl, TiltRadioButton, TiltSwitchButton, TiltToggleButton,
};

use super::{
    classification::{ElementRenderKind, element_render_kind},
    image::image_node,
};

/// Adds the minimal Bevy UI representation required by a built-in element.
pub(crate) fn materialize_element(
    world: &mut World,
    entity: Entity,
    kind: ElementKind,
    attributes: &[TemplateAttribute],
) {
    match element_render_kind(kind) {
        ElementRenderKind::Container => {
            world.entity_mut(entity).insert(Node::default());
            match kind {
                ElementKind::Badge => {
                    crate::widgets::content::badge::materialize(world, entity, attributes)
                }
                ElementKind::ContextMenu => {
                    crate::control::context_menu::materialize(world, entity, attributes)
                }
                ElementKind::Divider => {
                    crate::widgets::content::divider::materialize(world, entity, attributes)
                }
                ElementKind::Dialog => {
                    crate::widgets::advanced::dialog::materialize(world, entity, attributes)
                }
                ElementKind::Headline => {
                    crate::widgets::content::headline::materialize(world, entity, attributes)
                }
                ElementKind::ToolTip => {
                    crate::widgets::advanced::tooltip::materialize(world, entity, attributes)
                }
                _ => {}
            }
            if kind == ElementKind::FieldSet {
                world
                    .entity_mut(entity)
                    .insert(FieldSetSelection::from_attributes(attributes));
            }
            if kind == ElementKind::Form {
                world.entity_mut(entity).insert(
                    crate::widgets::structure::form::FormSettings::from_attributes(attributes),
                );
            }
            if kind == ElementKind::TableCell {
                world.entity_mut(entity).insert(
                    crate::widgets::structure::table_cell::TableCellInfo::from_attributes(
                        attributes,
                    ),
                );
            }
            if kind == ElementKind::ProgressBar {
                crate::widgets::advanced::progress_bar::materialize_parts(
                    world, entity, attributes,
                );
            }
        }
        ElementRenderKind::Control => {
            let disabled = crate::component::has_boolean_static_attribute(attributes, "disabled");
            let checked = crate::component::has_boolean_static_attribute(attributes, "checked")
                || (selectable_kind(kind)
                    && crate::component::has_boolean_static_attribute(attributes, "selected"));
            let tab_index = static_tab_index(attributes)
                .unwrap_or(if kind == ElementKind::Option { -1 } else { 0 });
            {
                let mut materialized = world.entity_mut(entity);
                materialized.insert((
                    Node::default(),
                    TiltControl,
                    ElementState {
                        disabled,
                        checked,
                        ..Default::default()
                    },
                ));
                if kind != ElementKind::Button {
                    materialized.insert(Interaction::None);
                }
                if kind != ElementKind::Button {
                    materialized.insert((
                        FocusPolicy::Block,
                        ControlTabIndex(tab_index),
                        TabIndex(if disabled { -1 } else { tab_index }),
                    ));
                }
                if kind == ElementKind::Button {
                    materialized.insert((
                        Button,
                        TiltButton,
                        crate::widgets::structure::form::FormButton::from_attributes(attributes),
                        ControlTabIndex(tab_index),
                        TabIndex(if disabled { -1 } else { tab_index }),
                    ));
                }
                if selectable_kind(kind) {
                    materialized.insert((
                        ControlChecked(checked),
                        Checkable,
                        FocusPolicy::Block,
                        ControlTabIndex(tab_index),
                        TabIndex(if disabled { -1 } else { tab_index }),
                    ));
                    match kind {
                        ElementKind::Checkbox => materialized.insert(TiltCheckbox),
                        ElementKind::RadioButton => materialized.insert(TiltRadioButton),
                        ElementKind::SwitchButton => materialized.insert(TiltSwitchButton),
                        ElementKind::ToggleButton => materialized.insert(TiltToggleButton),
                        _ => unreachable!("selectable control kind"),
                    };
                    if checked {
                        materialized.insert(Checked);
                    }
                }
                if disabled {
                    materialized.insert(InteractionDisabled);
                }
            }
            materialize_control_parts(world, entity, kind);
            if kind == ElementKind::Input {
                crate::widgets::controls::input::materialize_parts(
                    world, entity, attributes, false,
                );
            }
            if kind == ElementKind::TextArea {
                crate::widgets::controls::text_area::materialize_parts(
                    world, entity, attributes, true,
                );
            }
            if kind == ElementKind::Slider {
                crate::widgets::controls::slider::materialize_parts(world, entity, attributes);
            }
            if kind == ElementKind::Scrollbar {
                crate::widgets::advanced::scrollbar::materialize_parts(world, entity, attributes);
            }
            match kind {
                ElementKind::ChoiceBox => {
                    crate::widgets::controls::choice_box::materialize(world, entity, attributes)
                }
                ElementKind::ListBox => {
                    crate::widgets::controls::list_box::materialize(world, entity, attributes)
                }
                ElementKind::Option => {
                    crate::widgets::controls::option::materialize(world, entity, attributes)
                }
                ElementKind::HyperLink => {
                    crate::widgets::advanced::hyperlink::materialize(world, entity, attributes)
                }
                ElementKind::DatePicker => {
                    crate::widgets::advanced::date_picker::materialize(world, entity, attributes)
                }
                ElementKind::ColorPicker => {
                    crate::widgets::advanced::color_picker::materialize(world, entity, attributes)
                }
                _ => {}
            }
        }
        ElementRenderKind::Image => {
            let image = image_node(attributes, world.get_resource::<AssetServer>());
            world.entity_mut(entity).insert(image);
            crate::widgets::content::image::materialize_metadata(world, entity, attributes);
            if kind == ElementKind::Avatar {
                crate::widgets::content::avatar::materialize(world, entity, attributes);
            }
        }
    }
}

fn selectable_kind(kind: ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::Checkbox
            | ElementKind::RadioButton
            | ElementKind::SwitchButton
            | ElementKind::ToggleButton
    )
}

fn static_tab_index(attributes: &[TemplateAttribute]) -> Option<i32> {
    attributes.iter().find_map(|attribute| match attribute {
        TemplateAttribute::Static { name, value } if name == "tabindex" => value.parse().ok(),
        _ => None,
    })
}

fn materialize_control_parts(world: &mut World, owner: Entity, kind: ElementKind) {
    match kind {
        ElementKind::Checkbox => {
            crate::widgets::controls::checkbox::materialize_parts(world, owner)
        }
        ElementKind::RadioButton => {
            crate::widgets::controls::radio_button::materialize_parts(world, owner)
        }
        ElementKind::SwitchButton => {
            crate::widgets::controls::switch_button::materialize_parts(world, owner)
        }
        _ => {}
    }
}

/// Adds Bevy UI text to the semantic entity that owns the source text node.
pub(crate) fn materialize_text(world: &mut World, entity: Entity, value: &str) {
    world.entity_mut(entity).insert((
        Text::new(value),
        Node::default(),
        bevy::text::TextLayout::linebreak(bevy::text::LineBreak::WordOrCharacter),
    ));
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::{entity::Entity, world::World},
        ui::{
            Checked, Node,
            widget::{ImageNode, Text},
        },
    };
    use bevy_picking::Pickable;
    use tilt_ui_core::{ElementKind, NodeId, TemplateAttribute};

    use super::{materialize_element, materialize_text};
    use crate::{
        ControlChecked, ControlPart, ControlPartKind, ElementState, TemplateNodeRef, TiltButton,
        TiltCheckbox, TiltControl, TiltText, set_control_checked,
    };

    #[test]
    fn materializes_every_element_kind_with_its_intended_primitive() {
        let cases = [
            (ElementKind::Avatar, false, true),
            (ElementKind::Badge, false, false),
            (ElementKind::Body, false, false),
            (ElementKind::Button, true, false),
            (ElementKind::Checkbox, true, false),
            (ElementKind::ChoiceBox, true, false),
            (ElementKind::Option, true, false),
            (ElementKind::ColorPicker, true, false),
            (ElementKind::ContextMenu, false, false),
            (ElementKind::DatePicker, true, false),
            (ElementKind::Dialog, false, false),
            (ElementKind::Div, false, false),
            (ElementKind::Divider, false, false),
            (ElementKind::FieldSet, false, false),
            (ElementKind::Form, false, false),
            (ElementKind::Headline, false, false),
            (ElementKind::HyperLink, true, false),
            (ElementKind::Image, false, true),
            (ElementKind::Input, true, false),
            (ElementKind::Label, false, false),
            (ElementKind::ListBox, true, false),
            (ElementKind::Paragraph, false, false),
            (ElementKind::ProgressBar, false, false),
            (ElementKind::RadioButton, true, false),
            (ElementKind::Scrollbar, true, false),
            (ElementKind::Slider, true, false),
            (ElementKind::SwitchButton, true, false),
            (ElementKind::Table, false, false),
            (ElementKind::TableCell, false, false),
            (ElementKind::TextArea, true, false),
            (ElementKind::ToggleButton, true, false),
            (ElementKind::ToolTip, false, false),
        ];
        let mut world = World::new();

        for (kind, is_control, is_image) in cases {
            let entity = world.spawn_empty().id();
            materialize_element(&mut world, entity, kind, &[]);

            assert!(world.get::<Node>(entity).is_some(), "{kind:?} needs Node");
            assert_eq!(world.get::<TiltControl>(entity).is_some(), is_control);
            assert_eq!(world.get::<ImageNode>(entity).is_some(), is_image);
        }
    }

    #[test]
    fn image_materialization_accepts_a_static_source_without_asset_server_state() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let attributes = [TemplateAttribute::Static {
            name: "src".into(),
            value: "ui/test.png".into(),
        }];

        materialize_element(&mut world, entity, ElementKind::Image, &attributes);

        assert!(world.get::<ImageNode>(entity).is_some());
        assert!(world.get::<Node>(entity).is_some());
    }

    #[test]
    fn button_materialization_uses_native_button_primitives_without_extra_content() {
        let mut world = World::new();
        let button = world.spawn_empty().id();
        materialize_element(&mut world, button, ElementKind::Button, &[]);

        assert!(world.get::<bevy::ui::widget::Button>(button).is_some());
        assert!(world.get::<TiltControl>(button).is_some());
        assert!(world.get::<TiltButton>(button).is_some());
        assert!(world.get::<Text>(button).is_none());
    }

    #[test]
    fn editable_controls_materialize_native_editor_and_persistent_parts() {
        use crate::widgets::state::{EditableTextOptions, EditableTextParts};
        use bevy::ecs::hierarchy::Children;

        let mut world = World::new();
        let input = world.spawn_empty().id();
        materialize_element(
            &mut world,
            input,
            ElementKind::Input,
            &[
                TemplateAttribute::Static {
                    name: "value".into(),
                    value: "TiltUI".into(),
                },
                TemplateAttribute::Static {
                    name: "placeholder".into(),
                    value: "Type here".into(),
                },
            ],
        );
        let semantic = world.get::<crate::EditableText>(input).unwrap();
        assert_eq!(semantic.value, "TiltUI");
        assert_eq!(
            world
                .get::<bevy::text::EditableText>(input)
                .unwrap()
                .value()
                .to_string(),
            "TiltUI"
        );
        let parts = *world.get::<EditableTextParts>(input).unwrap();
        let children = world.get::<Children>(input).unwrap();
        assert_eq!(children.len(), 4);
        assert!(children.contains(&parts.placeholder));
        assert!(world.get::<EditableTextOptions>(input).is_some());
        assert_eq!(
            world
                .get::<bevy::text::TextLayout>(input)
                .unwrap()
                .linebreak,
            bevy::text::LineBreak::NoWrap
        );

        let area = world.spawn_empty().id();
        materialize_element(
            &mut world,
            area,
            ElementKind::TextArea,
            &[TemplateAttribute::Static {
                name: "max-lines".into(),
                value: "3".into(),
            }],
        );
        assert!(world.get::<crate::EditableText>(area).unwrap().multiline);
        assert_eq!(world.get::<crate::EditableText>(area).unwrap().cursor, 0);
        assert_eq!(
            world.get::<bevy::text::TextLayout>(area).unwrap().linebreak,
            bevy::text::LineBreak::WordOrCharacter
        );
        assert_eq!(
            world.get::<EditableTextOptions>(area).unwrap().max_lines,
            Some(3)
        );
        let handle = world.get::<Children>(area).unwrap().iter().find(|child| {
            world
                .get::<ControlPart>(**child)
                .is_some_and(|part| part.kind == ControlPartKind::ResizeHandle)
        });
        assert!(handle.is_some());
    }

    #[test]
    fn input_types_and_file_options_initialize_semantic_state_once() {
        use crate::widgets::controls::input::FileInputOptions;
        use tilt_ui_core::InputType;

        let mut world = World::new();
        for (tag, kind) in [
            ("email", InputType::Email),
            ("password", InputType::Password),
            ("number", InputType::Number),
            ("date", InputType::Date),
            ("file", InputType::File),
        ] {
            let entity = world.spawn_empty().id();
            materialize_element(
                &mut world,
                entity,
                ElementKind::Input,
                &[
                    TemplateAttribute::Static {
                        name: "type".into(),
                        value: tag.into(),
                    },
                    TemplateAttribute::Static {
                        name: "readonly".into(),
                        value: "true".into(),
                    },
                ],
            );
            let value = world.get::<crate::EditableText>(entity).unwrap();
            assert_eq!(value.input_type, kind);
            assert!(value.readonly);
            assert!(world.get::<ElementState>(entity).unwrap().readonly);
        }

        let file = world.spawn_empty().id();
        materialize_element(
            &mut world,
            file,
            ElementKind::Input,
            &[
                TemplateAttribute::Static {
                    name: "type".into(),
                    value: "file".into(),
                },
                TemplateAttribute::Static {
                    name: "folder".into(),
                    value: "true".into(),
                },
                TemplateAttribute::Static {
                    name: "extensions".into(),
                    value: "[json, png]".into(),
                },
                TemplateAttribute::Static {
                    name: "show-size".into(),
                    value: "true".into(),
                },
                TemplateAttribute::Static {
                    name: "max-size".into(),
                    value: "1024".into(),
                },
            ],
        );
        let options = world.get::<FileInputOptions>(file).unwrap();
        assert!(options.folder);
        assert_eq!(options.extensions, ["json", "png"]);
        assert!(options.show_size);
        assert_eq!(options.max_size_bytes, Some(1024));
    }

    #[test]
    fn numeric_widgets_attach_one_stable_track_and_fill() {
        use crate::widgets::state::NumericParts;
        use bevy::ecs::hierarchy::Children;

        let mut world = World::new();
        for (kind, has_thumb) in [
            (ElementKind::Slider, true),
            (ElementKind::ProgressBar, false),
        ] {
            let entity = world.spawn_empty().id();
            materialize_element(
                &mut world,
                entity,
                kind,
                &[
                    TemplateAttribute::Static {
                        name: "min".into(),
                        value: "0".into(),
                    },
                    TemplateAttribute::Static {
                        name: "max".into(),
                        value: "100".into(),
                    },
                    TemplateAttribute::Static {
                        name: "value".into(),
                        value: "72".into(),
                    },
                ],
            );
            let parts = *world.get::<NumericParts>(entity).unwrap();
            assert_eq!(world.get::<Children>(entity).unwrap().len(), 1);
            assert!(
                world
                    .get::<Children>(entity)
                    .unwrap()
                    .contains(&parts.track)
            );
            assert!(
                world
                    .get::<Children>(parts.track)
                    .unwrap()
                    .contains(&parts.fill)
            );
            assert_eq!(parts.thumb.is_some(), has_thumb);
            assert_eq!(
                world.get::<crate::NumericRange>(entity).unwrap().value,
                72.0
            );
        }
    }

    #[test]
    fn slider_optional_parts_are_materialized_once_and_survive_value_changes() {
        use crate::widgets::state::NumericParts;

        let mut world = World::new();
        let slider = world.spawn_empty().id();
        materialize_element(
            &mut world,
            slider,
            ElementKind::Slider,
            &[
                TemplateAttribute::Static {
                    name: "dots".into(),
                    value: "4".into(),
                },
                TemplateAttribute::Static {
                    name: "show-labels".into(),
                    value: String::new(),
                },
                TemplateAttribute::Static {
                    name: "show-tip".into(),
                    value: String::new(),
                },
            ],
        );
        let parts = *world.get::<NumericParts>(slider).unwrap();
        let mut query = world.query::<(Entity, &ControlPart)>();
        let before = query
            .iter(&world)
            .filter(|(_, part)| part.owner == slider)
            .map(|(entity, part)| (entity, part.kind))
            .collect::<Vec<_>>();
        assert_eq!(
            before
                .iter()
                .filter(|(_, kind)| *kind == ControlPartKind::Dot)
                .count(),
            5
        );
        assert_eq!(
            before
                .iter()
                .filter(|(_, kind)| *kind == ControlPartKind::Label)
                .count(),
            2
        );
        assert!(parts.tip.is_some());
        assert!(crate::widgets::controls::slider::set_slider_value(
            &mut world, slider, 64.0
        ));
        let after = query
            .iter(&world)
            .filter(|(_, part)| part.owner == slider)
            .map(|(entity, part)| (entity, part.kind))
            .collect::<Vec<_>>();
        assert_eq!(after, before);
        assert_eq!(world.get::<Text>(parts.tip.unwrap()).unwrap().0, "64");
    }

    #[test]
    fn checkbox_materialization_creates_one_persistent_indicator() {
        let mut world = World::new();
        let checkbox = world.spawn_empty().id();
        materialize_element(&mut world, checkbox, ElementKind::Checkbox, &[]);

        assert!(world.get::<TiltCheckbox>(checkbox).is_some());
        assert_eq!(
            world.get::<ControlChecked>(checkbox),
            Some(&ControlChecked(false))
        );
        let mut parts = world.query::<(Entity, &ControlPart)>();
        let parts = parts.iter(&world).collect::<Vec<_>>();
        assert_eq!(parts.len(), 2);
        let (indicator, part) = parts
            .iter()
            .find(|(_, part)| part.kind == ControlPartKind::Indicator)
            .copied()
            .expect("indicator part");
        assert_eq!(part.owner, checkbox);
        assert_eq!(part.kind, ControlPartKind::Indicator);
        assert_eq!(world.get::<Pickable>(indicator), Some(&Pickable::IGNORE));
        let (mark, part) = parts
            .iter()
            .find(|(_, part)| part.kind == ControlPartKind::Mark)
            .copied()
            .expect("mark part");
        assert_eq!(part.owner, checkbox);
        assert_eq!(
            world
                .get::<bevy::ecs::hierarchy::ChildOf>(mark)
                .unwrap()
                .parent(),
            indicator
        );
        assert_eq!(world.get::<Text>(mark).expect("checkbox mark text").0, "✓");

        assert!(set_control_checked(&mut world, checkbox, true));
        assert!(world.get::<ControlPart>(indicator).is_some());
        assert!(world.get::<ControlPart>(mark).is_some());
    }

    #[test]
    fn switch_materialization_creates_persistent_track_and_thumb() {
        let mut world = World::new();
        let switch = world.spawn_empty().id();
        materialize_element(&mut world, switch, ElementKind::SwitchButton, &[]);

        let mut parts = world.query::<(Entity, &ControlPart)>();
        let parts = parts.iter(&world).collect::<Vec<_>>();
        let (track, _) = parts
            .iter()
            .find(|(_, part)| part.kind == ControlPartKind::Track)
            .copied()
            .expect("track part");
        let (thumb, _) = parts
            .iter()
            .find(|(_, part)| part.kind == ControlPartKind::Thumb)
            .copied()
            .expect("thumb part");
        assert_eq!(
            world
                .get::<bevy::ecs::hierarchy::ChildOf>(thumb)
                .unwrap()
                .parent(),
            track
        );
        assert_eq!(world.get::<Pickable>(track), Some(&Pickable::IGNORE));
        assert_eq!(world.get::<Pickable>(thumb), Some(&Pickable::IGNORE));

        assert!(set_control_checked(&mut world, switch, true));
        assert!(world.get::<ControlPart>(track).is_some());
        assert!(world.get::<ControlPart>(thumb).is_some());
    }

    #[test]
    fn checked_static_attribute_initializes_native_and_semantic_state() {
        let mut world = World::new();
        let checkbox = world.spawn_empty().id();
        let attributes = [TemplateAttribute::Static {
            name: "checked".into(),
            value: "true".into(),
        }];
        materialize_element(&mut world, checkbox, ElementKind::Checkbox, &attributes);

        assert_eq!(
            world.get::<ControlChecked>(checkbox),
            Some(&ControlChecked(true))
        );
        assert!(world.get::<ElementState>(checkbox).unwrap().checked);
        assert!(world.get::<Checked>(checkbox).is_some());
    }

    #[test]
    fn text_materialization_uses_the_semantic_text_entity() {
        let mut world = World::new();
        let entity = world
            .spawn((
                TiltText {
                    value: "Play".into(),
                },
                TemplateNodeRef { node: NodeId(4) },
            ))
            .id();

        materialize_text(&mut world, entity, "Play");

        assert_eq!(world.get::<Text>(entity).expect("Bevy text").0, "Play");
        assert_eq!(
            world.get::<Node>(entity).unwrap().min_width,
            bevy::ui::Val::Auto
        );
        assert_eq!(
            world.get::<TemplateNodeRef>(entity).expect("node").node,
            NodeId(4)
        );
    }
}
