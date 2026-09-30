//! Platform-gated file selection for TiltUI file Inputs.

use std::sync::{
    Mutex,
    mpsc::{Receiver, channel},
};

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        message::{MessageReader, Messages},
        system::{Commands, Query},
        world::World,
    },
    ui::InteractionDisabled,
};

use crate::{
    ControlActivated, EditableTextChanged, set_editable_text,
    widgets::controls::input::{FileInputOptions, FileInputSelected, FileInputSelection},
};

#[derive(Component)]
pub(crate) struct PendingFileDialog(Mutex<Receiver<FileDialogOutcome>>);

enum FileDialogOutcome {
    Cancelled,
    RejectedTooLarge,
    Selected(FileInputSelection),
}

/// Opens a platform file dialog only for enabled file Inputs that were activated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn open_file_dialogs(
    mut activations: MessageReader<ControlActivated>,
    inputs: Query<(
        &FileInputOptions,
        Option<&InteractionDisabled>,
        Option<&PendingFileDialog>,
    )>,
    mut commands: Commands,
) {
    for activation in activations.read() {
        let Ok((options, disabled, pending)) = inputs.get(activation.entity) else {
            continue;
        };
        if disabled.is_some() || pending.is_some() {
            continue;
        }
        let options = options.clone();
        let (send, receive) = channel();
        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new();
            if !options.folder && !options.extensions.is_empty() {
                let extensions: Vec<&str> = options.extensions.iter().map(String::as_str).collect();
                dialog = dialog.add_filter("Allowed files", &extensions);
            }
            let path = if options.folder {
                dialog.pick_folder()
            } else {
                dialog.pick_file()
            };
            let outcome = path.map_or(FileDialogOutcome::Cancelled, |path| {
                let size_bytes = if options.folder {
                    None
                } else {
                    std::fs::metadata(&path).ok().map(|metadata| metadata.len())
                };
                if options
                    .max_size_bytes
                    .is_some_and(|limit| size_bytes.is_some_and(|size| size > limit))
                {
                    return FileDialogOutcome::RejectedTooLarge;
                }
                FileDialogOutcome::Selected(FileInputSelection {
                    name: path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    size_bytes,
                    native_path: Some(path),
                })
            });
            let _ = send.send(outcome);
        });
        commands
            .entity(activation.entity)
            .insert(PendingFileDialog(Mutex::new(receive)));
    }
}

/// Opens a browser file picker without exposing filesystem paths.
#[cfg(target_arch = "wasm32")]
pub(crate) fn open_file_dialogs(
    mut activations: MessageReader<ControlActivated>,
    inputs: Query<(
        &FileInputOptions,
        Option<&InteractionDisabled>,
        Option<&PendingFileDialog>,
    )>,
    mut commands: Commands,
) {
    for activation in activations.read() {
        let Ok((options, disabled, pending)) = inputs.get(activation.entity) else {
            continue;
        };
        if disabled.is_some() || pending.is_some() {
            continue;
        }
        if options.folder {
            bevy::log::warn!(
                "Browser folder selection is not supported by the file dialog backend"
            );
            continue;
        }
        let options = options.clone();
        let (send, receive) = channel();
        wasm_bindgen_futures::spawn_local(async move {
            let mut dialog = rfd::AsyncFileDialog::new();
            if !options.extensions.is_empty() {
                let extensions: Vec<&str> = options.extensions.iter().map(String::as_str).collect();
                dialog = dialog.add_filter("Allowed files", &extensions);
            }
            let result = dialog.pick_file().await;
            let outcome = result.map_or(FileDialogOutcome::Cancelled, |file| {
                let size_bytes = Some(file.inner().size() as u64);
                if options
                    .max_size_bytes
                    .is_some_and(|limit| size_bytes.is_some_and(|size| size > limit))
                {
                    return FileDialogOutcome::RejectedTooLarge;
                }
                FileDialogOutcome::Selected(FileInputSelection {
                    name: file.file_name(),
                    size_bytes,
                    native_path: None,
                })
            });
            let _ = send.send(outcome);
        });
        commands
            .entity(activation.entity)
            .insert(PendingFileDialog(Mutex::new(receive)));
    }
}

/// Applies completed dialog results without scanning inactive file Inputs.
pub(crate) fn finish_file_dialogs(
    dialogs: Query<(Entity, &PendingFileDialog, &FileInputOptions)>,
    mut commands: Commands,
) {
    for (entity, pending, options) in &dialogs {
        let Ok(receiver) = pending.0.lock() else {
            continue;
        };
        let Ok(result) = receiver.try_recv() else {
            continue;
        };
        let show_size = options.show_size;
        commands.queue(move |world: &mut World| {
            world.entity_mut(entity).remove::<PendingFileDialog>();
            let selection = match result {
                FileDialogOutcome::Cancelled => return,
                FileDialogOutcome::RejectedTooLarge => {
                    if let Some(mut state) = world.get_mut::<crate::ElementState>(entity) {
                        state.invalid = true;
                    }
                    return;
                }
                FileDialogOutcome::Selected(selection) => selection,
            };
            let name = selection.name.clone();
            let text_changed = set_editable_text(world, entity, &name);
            let upload_button = world.get::<crate::FileUploadButton>(entity);
            if text_changed || upload_button.is_some() {
                let form_name = world
                    .get::<crate::EditableTextOptions>(entity)
                    .and_then(|options| options.name.clone())
                    .or_else(|| upload_button.and_then(|button| button.name.clone()));
                world
                    .resource_mut::<Messages<EditableTextChanged>>()
                    .write(EditableTextChanged {
                        entity,
                        name: form_name,
                        value: name.clone(),
                    });
            }
            if show_size
                && let Some(size) = selection.size_bytes
                && let Some(parts) = world.get::<crate::widgets::state::EditableTextParts>(entity)
                && let Some(mut label) = world.get_mut::<bevy::ui::widget::Text>(parts.value)
            {
                label.0 = format!("{name} ({size} B)");
            }
            if let Some(mut state) = world.get_mut::<crate::ElementState>(entity) {
                state.invalid = false;
            }
            world.entity_mut(entity).insert(selection.clone());
            crate::widgets::structure::form::mark_form_dirty(world, entity);
            world
                .resource_mut::<Messages<FileInputSelected>>()
                .write(FileInputSelected { entity, selection });
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, Update},
        ecs::message::{MessageCursor, Messages},
    };
    use tilt_ui_core::{ElementKind, TemplateAttribute};

    use super::{FileDialogOutcome, PendingFileDialog, finish_file_dialogs};
    use crate::{
        EditableText, ElementState,
        widgets::controls::input::{FileInputSelected, FileInputSelection},
    };

    #[test]
    fn oversized_file_marks_invalid_but_cancel_does_not_change_the_value() {
        let mut app = App::new();
        app.add_message::<crate::EditableTextChanged>()
            .add_message::<FileInputSelected>()
            .add_systems(Update, finish_file_dialogs);
        let entity = app.world_mut().spawn_empty().id();
        crate::render::materialize_element(
            app.world_mut(),
            entity,
            ElementKind::Input,
            &[
                TemplateAttribute::Static {
                    name: "type".into(),
                    value: "file".into(),
                },
                TemplateAttribute::Static {
                    name: "max-size".into(),
                    value: "10".into(),
                },
            ],
        );

        let send = |app: &mut App, outcome| {
            let (sender, receiver) = std::sync::mpsc::channel();
            sender.send(outcome).unwrap();
            app.world_mut()
                .entity_mut(entity)
                .insert(PendingFileDialog(std::sync::Mutex::new(receiver)));
            app.update();
        };
        send(&mut app, FileDialogOutcome::RejectedTooLarge);
        assert!(app.world().get::<ElementState>(entity).unwrap().invalid);
        send(&mut app, FileDialogOutcome::Cancelled);
        assert!(app.world().get::<ElementState>(entity).unwrap().invalid);
        assert!(
            app.world()
                .get::<EditableText>(entity)
                .unwrap()
                .value
                .is_empty()
        );
        send(
            &mut app,
            FileDialogOutcome::Selected(FileInputSelection {
                name: "small.txt".into(),
                size_bytes: Some(8),
                native_path: None,
            }),
        );
        assert!(!app.world().get::<ElementState>(entity).unwrap().invalid);
        assert_eq!(
            app.world().get::<EditableText>(entity).unwrap().value,
            "small.txt"
        );
        let mut cursor = MessageCursor::<FileInputSelected>::default();
        assert_eq!(
            cursor
                .read(app.world().resource::<Messages<FileInputSelected>>())
                .count(),
            1
        );
    }

    #[test]
    fn upload_button_emits_the_same_selection_event_without_changing_its_label() {
        let mut app = App::new();
        app.add_message::<crate::EditableTextChanged>()
            .add_message::<FileInputSelected>()
            .add_systems(Update, finish_file_dialogs);
        let button = app.world_mut().spawn_empty().id();
        crate::render::materialize_element(
            app.world_mut(),
            button,
            ElementKind::Button,
            &[
                TemplateAttribute::Static {
                    name: "type".into(),
                    value: "file".into(),
                },
                TemplateAttribute::Static {
                    name: "max-size".into(),
                    value: "10".into(),
                },
                TemplateAttribute::Static {
                    name: "name".into(),
                    value: "attachment".into(),
                },
            ],
        );
        let complete = |app: &mut App, outcome| {
            let (sender, receiver) = std::sync::mpsc::channel();
            sender.send(outcome).unwrap();
            app.world_mut()
                .entity_mut(button)
                .insert(PendingFileDialog(std::sync::Mutex::new(receiver)));
            app.update();
        };
        complete(&mut app, FileDialogOutcome::RejectedTooLarge);
        assert!(app.world().get::<ElementState>(button).unwrap().invalid);
        complete(
            &mut app,
            FileDialogOutcome::Selected(FileInputSelection {
                name: "small.txt".into(),
                size_bytes: Some(8),
                native_path: None,
            }),
        );
        assert!(!app.world().get::<ElementState>(button).unwrap().invalid);
        assert_eq!(
            app.world().get::<FileInputSelection>(button).unwrap().name,
            "small.txt"
        );
        assert!(app.world().get::<EditableText>(button).is_none());
        let mut cursor = MessageCursor::<FileInputSelected>::default();
        let events = cursor
            .read(app.world().resource::<Messages<FileInputSelected>>())
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity, button);
        assert_eq!(events[0].selection.size_bytes, Some(8));
        let mut changed_cursor = MessageCursor::<crate::EditableTextChanged>::default();
        let changed = changed_cursor
            .read(
                app.world()
                    .resource::<Messages<crate::EditableTextChanged>>(),
            )
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].entity, button);
        assert_eq!(changed[0].name.as_deref(), Some("attachment"));
        assert_eq!(changed[0].value, "small.txt");
    }
}
