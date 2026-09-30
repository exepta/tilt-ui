//! Interactive controls for the complete icon gallery.

use bevy::clipboard::Clipboard;
use bevy::prelude::*;
use bevy::ui::ScrollPosition;
use std::collections::BTreeMap;
use tilt_ui::{
    ColorPickerChanged, ControlActivated, EditableTextChanged, ElementClasses, ElementId, Icon,
    IconSize, ImageMetadata, OptionSelectionChanged, component_init, component_update, open_dialog,
    set_color_value, set_editable_text, set_icon_size, set_image_source, set_inner_text,
    spawn_component, switch_ui_theme,
};

use super::tilt_ui_component_id;

#[derive(Resource)]
struct CatalogSettings {
    size: IconSize,
    tint: Color,
    selected: Option<Icon>,
    query: String,
    category: String,
    page: usize,
}

impl Default for CatalogSettings {
    fn default() -> Self {
        Self {
            size: IconSize::Px32,
            tint: Color::srgba(
                0x76 as f32 / 255.0,
                0x5B as f32 / 255.0,
                0xD7 as f32 / 255.0,
                1.0,
            ),
            selected: None,
            query: String::new(),
            category: "all".into(),
            page: 0,
        }
    }
}

#[component_init]
fn open_catalog(mut commands: Commands) {
    commands.insert_resource(CatalogSettings::default());
    commands.queue(|world: &mut World| {
        switch_ui_theme(world, "light").expect("registered light theme");
    });
    spawn_component(
        &mut commands,
        tilt_ui_component_id("catalog").expect("catalog component metadata"),
    );
}

#[component_update]
fn update_controls(
    mut selections: MessageReader<OptionSelectionChanged>,
    mut colors: MessageReader<ColorPickerChanged>,
    ids: Query<&ElementId>,
    mut settings: ResMut<CatalogSettings>,
    mut commands: Commands,
) {
    for selection in selections.read() {
        if !selection.selected {
            continue;
        }
        let Ok(id) = ids.get(selection.control) else {
            continue;
        };
        match id.0.as_str() {
            "theme-choice" if matches!(selection.value.as_str(), "light" | "dark") => {
                let theme = selection.value.clone();
                commands.queue(move |world: &mut World| {
                    if let Err(error) = switch_ui_theme(world, &theme) {
                        warn!("Could not switch icon catalog theme: {error}");
                    }
                });
            }
            "category-choice" => {
                settings.category = selection.value.clone();
                settings.page = 0;
            }
            "size-choice" => {
                if let Some(size) = selection
                    .value
                    .parse::<u32>()
                    .ok()
                    .and_then(IconSize::from_pixels)
                {
                    settings.size = size;
                }
            }
            _ => {}
        }
    }
    for change in colors.read() {
        if ids.get(change.entity).is_ok_and(|id| id.0 == "icon-color") {
            let color = change.value;
            settings.tint = Color::srgba(color.red, color.green, color.blue, color.alpha);
            commands.queue(|world: &mut World| mark_palette_selected(world, None));
        }
    }
}

#[component_update]
fn select_palette(
    mut activations: MessageReader<ControlActivated>,
    ids: Query<&ElementId>,
    mut settings: ResMut<CatalogSettings>,
    mut commands: Commands,
) {
    for activation in activations.read() {
        let Ok(id) = ids.get(activation.entity) else {
            continue;
        };
        let color = match id.0.as_str() {
            "palette-violet" => (0x76, 0x5B, 0xD7),
            "palette-blue" => (0x6C, 0x8F, 0xE8),
            "palette-teal" => (0x39, 0xB7, 0xB4),
            "palette-coral" => (0xEC, 0x82, 0x7F),
            "palette-gold" => (0xE8, 0xB6, 0x65),
            _ => continue,
        };
        let (red, green, blue) = (
            color.0 as f32 / 255.0,
            color.1 as f32 / 255.0,
            color.2 as f32 / 255.0,
        );
        settings.tint = Color::srgb(red, green, blue);
        let selected = activation.entity;
        commands.queue(move |world: &mut World| {
            mark_palette_selected(world, Some(selected));
            let picker = world
                .query::<(Entity, &ElementId)>()
                .iter(world)
                .find_map(|(entity, id)| (id.0 == "icon-color").then_some(entity));
            if let Some(picker) = picker {
                set_color_value(
                    world,
                    picker,
                    tilt_ui::tilt_ui_css::CssColor::rgba(red, green, blue, 1.0),
                );
            }
        });
    }
}

fn mark_palette_selected(world: &mut World, selected: Option<Entity>) {
    let buttons = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .filter_map(|(entity, id)| id.0.starts_with("palette-").then_some(entity))
        .collect::<Vec<_>>();
    for entity in buttons {
        if let Some(mut classes) = world.get_mut::<ElementClasses>(entity) {
            classes.classes.retain(|class| class != "is-selected");
            if selected == Some(entity) {
                classes.classes.push("is-selected".into());
            }
        }
    }
}

#[component_update]
fn icon_details(
    mut activations: MessageReader<ControlActivated>,
    ids: Query<&ElementId>,
    displayed: Query<&DisplayedIcon>,
    mut settings: ResMut<CatalogSettings>,
    mut clipboard: Option<ResMut<Clipboard>>,
    mut commands: Commands,
) {
    for activation in activations.read() {
        let Ok(id) = ids.get(activation.entity) else {
            continue;
        };
        if let Some(icon) = displayed.get(activation.entity).ok().map(|shown| shown.0) {
            settings.selected = Some(icon);
            let size = settings.size;
            commands.queue(move |world: &mut World| present_icon_details(world, icon, size));
            continue;
        }
        let Some(selected) = settings.selected else {
            continue;
        };
        let snippet = match id.0.as_str() {
            "copy-icon-code" => Some(icon_html(selected, settings.size)),
            "copy-image-code" => Some(icon_image_html(selected, settings.size)),
            "copy-css-code" => Some(icon_css(selected, settings.size)),
            _ => None,
        };
        if let Some(snippet) = snippet
            && clipboard
                .as_deref_mut()
                .is_some_and(|clipboard| clipboard.set_text(snippet).is_ok())
        {
            let button = activation.entity;
            commands.queue(move |world: &mut World| {
                let _ = set_inner_text(world, button, "Copied");
            });
        }
    }
}

fn icon_html(icon: Icon, size: IconSize) -> String {
    format!(
        "<icon name=\"{}\" size=\"{}\" />",
        icon.name(),
        size.pixels()
    )
}

fn icon_image_html(icon: Icon, size: IconSize) -> String {
    format!(
        "<img src=\"{}\" alt=\"{}\" />",
        icon.source(size),
        icon.name().replace('-', " ")
    )
}

fn icon_css(icon: Icon, size: IconSize) -> String {
    format!("background-image: url(\"{}\");", icon.source(size))
}

fn present_icon_details(world: &mut World, icon: Icon, size: IconSize) {
    let entities = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .map(|(entity, id)| (id.0.clone(), entity))
        .collect::<BTreeMap<_, _>>();
    let title = icon
        .name()
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    for (id, value) in [
        ("details-name", title),
        ("details-slug", icon.name().to_owned()),
        ("details-html", icon_html(icon, size)),
        ("details-image-code", icon_image_html(icon, size)),
        ("details-css", icon_css(icon, size)),
    ] {
        if let Some(&entity) = entities.get(id) {
            let _ = set_inner_text(world, entity, value);
        }
    }
    for id in ["copy-icon-code", "copy-image-code", "copy-css-code"] {
        if let Some(&entity) = entities.get(id) {
            let _ = set_inner_text(world, entity, "Copy");
        }
    }
    if let Some(&entity) = entities.get("details-icon") {
        set_image_source(world, entity, Some(icon.source(IconSize::Px64)));
    }
    if let Some(&dialog) = entities.get("icon-details-dialog") {
        open_dialog(world, dialog);
    }
}

#[component_update]
fn search_catalog(
    mut edits: MessageReader<EditableTextChanged>,
    mut activations: MessageReader<ControlActivated>,
    ids: Query<(Entity, &ElementId)>,
    mut commands: Commands,
    mut settings: ResMut<CatalogSettings>,
) {
    let mut query = None;
    for edit in edits.read() {
        if ids
            .get(edit.entity)
            .is_ok_and(|(_, id)| id.0 == "icon-search")
        {
            query = Some(edit.value.clone());
        }
    }
    for activation in activations.read() {
        if !ids
            .get(activation.entity)
            .is_ok_and(|(_, id)| id.0 == "clear-search")
        {
            continue;
        }
        query = Some(String::new());
        if let Some((search, _)) = ids.iter().find(|(_, id)| id.0 == "icon-search") {
            commands.queue(move |world: &mut World| {
                set_editable_text(world, search, "");
            });
        }
    }
    if let Some(query) = query {
        settings.query = query;
        settings.page = 0;
    }
}

#[derive(Clone, Copy)]
struct CatalogGroup {
    key: &'static str,
    title: &'static str,
    description: &'static str,
    names: &'static [&'static str],
}

const CATALOG_GROUPS: &[CatalogGroup] = &[
    CatalogGroup {
        key: "home",
        title: "Home",
        description: "Places & navigation",
        names: &[
            "home",
            "home-filled",
            "home-modern",
            "home-roof",
            "home-circle",
        ],
    },
    CatalogGroup {
        key: "settings",
        title: "Settings",
        description: "Adjustments & tools",
        names: &[
            "settings",
            "settings-filled",
            "settings-sliders",
            "settings-tune",
            "settings-wrench",
            "settings-adjust",
            "settings-circle",
            "settings-horizontal",
        ],
    },
    CatalogGroup {
        key: "user",
        title: "User",
        description: "People & profiles",
        names: &[
            "user",
            "user-filled",
            "user-circle",
            "user-square",
            "user-plus",
            "user-group",
        ],
    },
    CatalogGroup {
        key: "menu",
        title: "Menu",
        description: "Navigation patterns",
        names: &[
            "menu",
            "menu-wide",
            "menu-compact",
            "menu-dots",
            "menu-dots-vertical",
            "menu-grid",
            "menu-grid-filled",
            "menu-list",
            "menu-chevron",
            "menu-circle",
        ],
    },
    CatalogGroup {
        key: "close",
        title: "Close",
        description: "Dismiss actions",
        names: &["close", "close-circle", "close-square", "close-bold"],
    },
    CatalogGroup {
        key: "briefcase",
        title: "Briefcase",
        description: "Work & business",
        names: &[
            "briefcase",
            "briefcase-filled",
            "briefcase-medical",
            "briefcase-business",
        ],
    },
    CatalogGroup {
        key: "essentials",
        title: "Essentials",
        description: "Common actions",
        names: &[
            "search",
            "bell",
            "heart",
            "star",
            "check",
            "plus",
            "minus",
            "arrow-left",
            "arrow-right",
        ],
    },
    CatalogGroup {
        key: "directions",
        title: "Directions",
        description: "Arrows and chevrons",
        names: &[
            "chevron-left",
            "chevron-right",
            "chevron-down",
            "download",
            "upload",
            "trash",
            "edit",
            "mail",
        ],
    },
    CatalogGroup {
        key: "daily",
        title: "Daily",
        description: "Everyday symbols",
        names: &[
            "calendar", "clock", "folder", "file", "lock", "eye", "sun", "moon",
        ],
    },
    CatalogGroup {
        key: "workflow",
        title: "Workflow",
        description: "Everyday actions",
        names: &[
            "copy",
            "link",
            "external-link",
            "share",
            "filter",
            "refresh",
            "bookmark",
            "map-pin",
        ],
    },
    CatalogGroup {
        key: "feedback",
        title: "Feedback & media",
        description: "Status and content",
        names: &[
            "info",
            "warning",
            "help-circle",
            "check-circle",
            "play",
            "pause",
            "image",
            "phone",
        ],
    },
    CatalogGroup {
        key: "navigation",
        title: "Navigation",
        description: "Pages & directions",
        names: &[
            "dashboard",
            "list",
            "grid",
            "layers",
            "target",
            "globe",
            "compass",
            "map",
            "route",
            "arrow-up",
            "arrow-down",
            "chevron-up",
        ],
    },
    CatalogGroup {
        key: "devices",
        title: "Devices & more",
        description: "Media, hardware & shopping",
        names: &[
            "camera",
            "video",
            "music",
            "mic",
            "volume",
            "wifi",
            "battery",
            "monitor",
            "smartphone",
            "printer",
            "gift",
            "shopping-cart",
        ],
    },
    CatalogGroup {
        key: "actions",
        title: "Actions",
        description: "Everyday controls",
        names: &[
            "check-check",
            "circle-x",
            "ellipsis",
            "ellipsis-vertical",
            "rotate-ccw",
            "rotate-cw",
            "undo-2",
            "redo-2",
            "maximize",
            "minimize",
        ],
    },
    CatalogGroup {
        key: "files",
        title: "Files & cloud",
        description: "Documents and storage",
        names: &[
            "file-plus",
            "file-minus",
            "file-check",
            "file-search",
            "folder-open",
            "folder-plus",
            "archive",
            "cloud",
            "cloud-upload",
            "cloud-download",
        ],
    },
    CatalogGroup {
        key: "commerce",
        title: "Commerce",
        description: "Payments and shopping",
        names: &[
            "shopping-bag",
            "credit-card",
            "wallet",
            "receipt",
            "tag",
            "tags",
            "percent",
            "coins",
            "banknote",
            "store",
        ],
    },
    CatalogGroup {
        key: "communication",
        title: "Communication",
        description: "Messages and contacts",
        names: &[
            "message-square",
            "message-circle",
            "send",
            "inbox",
            "at-sign",
            "hash",
            "headphones",
            "megaphone",
            "radio",
            "contact",
        ],
    },
    CatalogGroup {
        key: "media",
        title: "Media",
        description: "Playback and viewing",
        names: &[
            "skip-back",
            "skip-forward",
            "rewind",
            "fast-forward",
            "square",
            "circle-dot",
            "volume-x",
            "film",
            "scan",
            "picture-in-picture",
        ],
    },
    CatalogGroup {
        key: "editor",
        title: "Editor",
        description: "Drawing and layout",
        names: &[
            "crop",
            "scissors",
            "brush",
            "paint-bucket",
            "pen-tool",
            "eraser",
            "ruler",
            "move",
            "list-indent-increase",
            "list-indent-decrease",
        ],
    },
    CatalogGroup {
        key: "weather",
        title: "Weather & nature",
        description: "Elements and outdoors",
        names: &[
            "cloud-sun",
            "cloud-rain",
            "cloud-snow",
            "wind",
            "umbrella",
            "thermometer",
            "droplet",
            "flame",
            "leaf",
            "mountain",
        ],
    },
    CatalogGroup {
        key: "data",
        title: "Data & code",
        description: "Charts and development",
        names: &[
            "chart-column",
            "chart-line",
            "chart-pie",
            "trending-up",
            "trending-down",
            "activity",
            "database",
            "server",
            "terminal",
            "code",
        ],
    },
    CatalogGroup {
        key: "security",
        title: "Security",
        description: "Access and identity",
        names: &[
            "shield",
            "shield-check",
            "key",
            "scan-face",
            "lock-open",
            "user-round-check",
            "user-round-minus",
            "log-in",
            "log-out",
            "contact-round",
        ],
    },
    CatalogGroup {
        key: "travel",
        title: "Travel & places",
        description: "Places and transport",
        names: &[
            "car",
            "bus",
            "train-front",
            "plane",
            "bike",
            "ship",
            "building",
            "coffee",
            "utensils",
            "flag",
        ],
    },
    CatalogGroup {
        key: "productivity",
        title: "Productivity",
        description: "Planning and notes",
        names: &[
            "alarm-clock",
            "calendar-days",
            "calendar-check",
            "timer",
            "list-checks",
            "notebook",
            "clipboard",
            "clipboard-check",
            "sticky-note",
            "book-open",
        ],
    },
    CatalogGroup {
        key: "development",
        title: "Development",
        description: "Code and hardware",
        names: &[
            "bug",
            "git-branch",
            "git-commit-horizontal",
            "git-merge",
            "braces",
            "brackets",
            "binary",
            "cpu",
            "hard-drive",
            "keyboard",
        ],
    },
    CatalogGroup {
        key: "health",
        title: "Health",
        description: "Care and medicine",
        names: &[
            "ambulance",
            "bandage",
            "heart-pulse",
            "pill",
            "syringe",
            "stethoscope",
            "hospital",
            "dna",
            "microscope",
            "flask-conical",
        ],
    },
    CatalogGroup {
        key: "food",
        title: "Food & drink",
        description: "Kitchen favorites",
        names: &[
            "apple", "banana", "cake", "chef-hat", "cookie", "cup-soda", "fish", "pizza",
            "sandwich", "wine",
        ],
    },
    CatalogGroup {
        key: "sports",
        title: "Sports & outdoors",
        description: "Games and movement",
        names: &[
            "dumbbell",
            "award",
            "goal",
            "medal",
            "person-standing",
            "trophy",
            "volleyball",
            "waves-ladder",
            "sailboat",
            "tent",
        ],
    },
    CatalogGroup {
        key: "social",
        title: "Social",
        description: "People and reactions",
        names: &[
            "users-round",
            "user-round-plus",
            "user-round-search",
            "message-square-heart",
            "heart-plus",
            "thumbs-up",
            "thumbs-down",
            "handshake",
            "party-popper",
            "heart-handshake",
        ],
    },
    CatalogGroup {
        key: "interface",
        title: "App layout",
        description: "Panels and windows",
        names: &[
            "panel-left",
            "panel-right",
            "panel-left-open",
            "panel-left-close",
            "app-window",
            "columns-2",
            "rows-2",
            "layout-template",
            "split",
            "between-horizontal-start",
        ],
    },
    CatalogGroup {
        key: "finance",
        title: "Finance",
        description: "Money and tickets",
        names: &[
            "badge-dollar-sign",
            "circle-dollar-sign",
            "landmark",
            "piggy-bank",
            "hand-coins",
            "chart-no-axes-combined",
            "calculator",
            "scan-line",
            "qr-code",
            "ticket",
        ],
    },
    CatalogGroup {
        key: "science",
        title: "Science & tech",
        description: "Discovery and invention",
        names: &[
            "atom",
            "beaker",
            "flask-round",
            "telescope",
            "satellite",
            "bot",
            "circuit-board",
            "microchip",
            "zap",
            "radio-tower",
        ],
    },
    CatalogGroup {
        key: "misc",
        title: "Everyday objects",
        description: "Useful extras",
        names: &[
            "anchor",
            "crown",
            "diamond",
            "gem",
            "lightbulb",
            "puzzle",
            "rocket",
            "snowflake",
            "tree-pine",
            "wand-sparkles",
        ],
    },
    CatalogGroup {
        key: "bevy",
        title: "Bevy",
        description: "Five bird SVG variants",
        names: &[
            "bevy",
            "bevy-circle",
            "bevy-badge",
            "bevy-orbit",
            "bevy-sparkle",
        ],
    },
    CatalogGroup {
        key: "animated",
        title: "Animated",
        description: "Motion built into the icon",
        names: &[
            "loading-spin",
            "loading-dots",
            "heart-beat",
            "bell-ring",
            "sparkle-pulse",
            "hourglass-flip",
            "wifi-pulse",
            "orbit-spin",
            "flame-dance",
            "arrow-slide",
        ],
    },
    CatalogGroup {
        key: "animals",
        title: "Animals",
        description: "Cats, dogs and small friends",
        names: &[
            "cat",
            "dog",
            "rabbit",
            "bird",
            "birdhouse",
            "turtle",
            "squirrel",
            "paw-print",
            "snail",
            "worm",
        ],
    },
    CatalogGroup {
        key: "wildlife",
        title: "Wildlife",
        description: "Nature and animal details",
        names: &[
            "rat",
            "fish-off",
            "fish-symbol",
            "fishing-hook",
            "fishing-rod",
            "bug-off",
            "bug-play",
            "egg",
            "shell",
            "feather",
        ],
    },
    CatalogGroup {
        key: "gaming",
        title: "Gaming",
        description: "Controllers and dice",
        names: &[
            "gamepad",
            "gamepad-2",
            "gamepad-directional",
            "dice-1",
            "dice-2",
            "dice-3",
            "dice-4",
            "dice-5",
            "dice-6",
            "dices",
        ],
    },
    CatalogGroup {
        key: "boardgames",
        title: "Board games",
        description: "Chess and adventures",
        names: &[
            "chess-bishop",
            "chess-king",
            "chess-knight",
            "chess-pawn",
            "chess-queen",
            "chess-rook",
            "joystick",
            "sword",
            "swords",
            "bow-arrow",
        ],
    },
    CatalogGroup {
        key: "learning",
        title: "Learning",
        description: "Books and school",
        names: &[
            "graduation-cap",
            "school",
            "book",
            "book-text",
            "book-check",
            "library",
            "pencil",
            "pencil-line",
            "backpack",
            "scroll-text",
        ],
    },
    CatalogGroup {
        key: "household",
        title: "Home life",
        description: "Furniture and rooms",
        names: &[
            "sofa",
            "bed",
            "bed-double",
            "bath",
            "shower-head",
            "lamp",
            "lamp-desk",
            "door-open",
            "door-closed",
            "house-plus",
        ],
    },
    CatalogGroup {
        key: "creative",
        title: "Creative",
        description: "Art, music and media",
        names: &[
            "palette",
            "paintbrush",
            "paint-roller",
            "aperture",
            "camera-off",
            "clapperboard",
            "drum",
            "guitar",
            "piano",
            "speaker",
        ],
    },
    CatalogGroup {
        key: "mobility",
        title: "Mobility",
        description: "More ways to travel",
        names: &[
            "train-track",
            "tram-front",
            "truck",
            "ship-wheel",
            "traffic-cone",
            "fuel",
        ],
    },
    CatalogGroup {
        key: "apps-gaming",
        title: "Apps · Gaming",
        description: "Games and creative engines",
        names: &[
            "discord",
            "steam",
            "epic-games",
            "itch-io",
            "gog",
            "godot",
            "unity",
            "unreal-engine",
        ],
    },
    CatalogGroup {
        key: "apps-social",
        title: "Apps · Social",
        description: "Popular media and social apps",
        names: &[
            "youtube",
            "twitch",
            "reddit",
            "spotify",
            "telegram",
            "whatsapp",
            "instagram",
            "tiktok",
        ],
    },
    CatalogGroup {
        key: "apps-tools",
        title: "Apps · Tools",
        description: "Developer and design tools",
        names: &[
            "github",
            "gitlab",
            "figma",
            "blender",
            "rust-logo",
            "docker",
            "obs-studio",
            "notion",
        ],
    },
    CatalogGroup {
        key: "animated-creatures",
        title: "Animated · Characters",
        description: "Animated animals and app marks",
        names: &[
            "bevy-flight",
            "discord-pulse",
            "cat-bounce",
            "dog-wag",
            "bird-flap",
            "rabbit-hop",
            "paw-step",
            "gamepad-shake",
            "dice-roll",
            "sword-swing",
        ],
    },
    CatalogGroup {
        key: "animated-motion",
        title: "Animated · Motion",
        description: "More moving interface symbols",
        names: &[
            "book-pulse",
            "music-bounce",
            "camera-flash",
            "cloud-drift",
            "sun-spin",
            "moon-rock",
            "rocket-launch",
            "message-pop",
            "check-bounce",
            "download-drop",
        ],
    },
    CatalogGroup {
        key: "nature-flora",
        title: "Flowers & plants",
        description: "Leaves, trees and garden life",
        names: &[
            "flower",
            "leaf-maple",
            "leaf-two",
            "plant",
            "plant-two",
            "seedling",
            "tree",
            "trees",
            "cactus",
            "mushroom",
        ],
    },
    CatalogGroup {
        key: "nature-sky",
        title: "Sky & weather",
        description: "Light, clouds and changing skies",
        names: &[
            "sunrise",
            "sunset",
            "moon-star",
            "sun-dim",
            "cloud-drizzle",
            "cloud-fog",
            "cloud-hail",
            "cloud-lightning",
            "cloud-moon",
            "cloud-rain-wind",
        ],
    },
    CatalogGroup {
        key: "nature-landscape",
        title: "Land & water",
        description: "Landscapes and natural patterns",
        names: &[
            "rainbow",
            "volcano",
            "beach",
            "windmill",
            "droplets",
            "mountain-snow",
            "earth",
            "waves-horizontal",
            "sun-snow",
            "cloud-sun-rain",
        ],
    },
    CatalogGroup {
        key: "animals-more-details",
        title: "More animals",
        description: "Wildlife and familiar companions",
        names: &[
            "bat",
            "butterfly",
            "deer",
            "horse",
            "pig",
            "spider",
            "fish-bone",
            "dog-bowl",
            "paw",
            "paw-off",
        ],
    },
    CatalogGroup {
        key: "animals-more-wildlife",
        title: "More wildlife",
        description: "Beetles, birds and larger animals",
        names: &[
            "cow",
            "bug-beetle",
            "fish-simple",
            "bee",
            "elephant",
            "kangaroo",
            "owl",
            "penguin",
            "shark",
            "snake",
        ],
    },
    CatalogGroup {
        key: "apps-more-browsers",
        title: "Browsers & platforms",
        description: "Web browsers and platforms",
        names: &[
            "firefox",
            "chrome",
            "safari",
            "edge",
            "opera",
            "vivaldi",
            "arc-browser",
            "yandex",
            "google",
            "apple-logo",
        ],
    },
    CatalogGroup {
        key: "apps-more-developer",
        title: "Developer apps",
        description: "Frameworks, languages and tools",
        names: &[
            "angular",
            "react",
            "vue",
            "svelte",
            "typescript",
            "javascript",
            "python",
            "vs-code",
            "node-js",
            "npm",
        ],
    },
    CatalogGroup {
        key: "apps-more-community",
        title: "Community apps",
        description: "Teams, sharing and social",
        names: &[
            "slack",
            "zoom",
            "linkedin",
            "pinterest",
            "mastodon",
            "signal",
            "matrix",
            "dropbox",
            "trello",
            "asana",
        ],
    },
    CatalogGroup {
        key: "utilities-workflow",
        title: "Workflow utilities",
        description: "Copy, capture, dates and search",
        names: &[
            "clipboard-copy",
            "clipboard-list",
            "copy-check",
            "copy-x",
            "text-scan-two",
            "camera-plus",
            "calendar-plus",
            "calendar-minus",
            "search-off",
            "filter-off",
        ],
    },
    CatalogGroup {
        key: "utilities-tools",
        title: "More utilities",
        description: "Sort, adjust and work faster",
        names: &[
            "sort-ascending",
            "sort-descending",
            "arrows-sort",
            "settings-automation",
            "adjustments",
            "adjustments-horizontal",
            "tool",
            "tools-off",
            "wand",
            "wand-off",
        ],
    },

];

#[derive(Component, Clone, Copy)]
struct DisplayedIcon(Icon);

const GROUPS_PER_PAGE: usize = 4;
const CARDS_PER_GROUP: usize = 12;

#[derive(Resource, Default)]
struct PagedGallery {
    handles: BTreeMap<String, Entity>,
    groups: Vec<(usize, Vec<Icon>)>,
    query: String,
    category: String,
    page: Option<usize>,
    size: Option<IconSize>,
}

fn filtered_groups(query: &str, category: &str) -> Vec<(usize, Vec<Icon>)> {
    let words = query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    CATALOG_GROUPS
        .iter()
        .enumerate()
        .filter_map(|(index, group)| {
            if category != "all"
                && group.key != category
                && !(category == "animated" && group.key.starts_with("animated"))
                && !(matches!(category, "nature" | "animals-more" | "apps-more" | "utilities")
                    && group.key.starts_with(category))
            {
                return None;
            }
            let icons = group
                .names
                .iter()
                .filter_map(|name| Icon::from_name(name))
                .filter(|icon| {
                    let searchable = format!(
                        "{} {} {}",
                        icon.name().replace('-', " "),
                        group.key,
                        group.title.to_lowercase()
                    );
                    words.iter().all(|word| searchable.contains(word))
                })
                .collect::<Vec<_>>();
            (!icons.is_empty()).then_some((index, icons))
        })
        .collect()
}

fn page_count(group_count: usize) -> usize {
    group_count.div_ceil(GROUPS_PER_PAGE).max(1)
}

fn named(gallery: &PagedGallery, id: &str) -> Option<Entity> {
    gallery.handles.get(id).copied()
}

fn show_slot(world: &mut World, entity: Entity, show: bool) {
    let is_empty = world
        .get::<ElementId>(entity)
        .is_some_and(|id| id.0 == "empty-state");
    if let Some(mut classes) = world.get_mut::<ElementClasses>(entity) {
        let class = if is_empty {
            "search-visible"
        } else {
            "slot-hidden"
        };
        let should_have = if is_empty { show } else { !show };
        let has = classes.classes.iter().any(|value| value == class);
        if should_have && !has {
            classes.classes.push(class.into());
        }
        if !should_have && has {
            classes.classes.retain(|value| value != class);
        }
    }
    if let Some(mut node) = world.get_mut::<Node>(entity) {
        let display = if show { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

fn set_pager_button(world: &mut World, entity: Entity, enabled: bool) {
    if let Some(mut classes) = world.get_mut::<ElementClasses>(entity) {
        let disabled = classes.classes.iter().any(|value| value == "is-disabled");
        if enabled && disabled {
            classes.classes.retain(|value| value != "is-disabled");
        }
        if !enabled && !disabled {
            classes.classes.push("is-disabled".into());
        }
    }
}

fn bind_gallery_slot(
    world: &mut World,
    gallery: &PagedGallery,
    slot: usize,
    group_position: Option<usize>,
    size: IconSize,
) {
    let family_id = format!("virtual-family-{slot}");
    let Some(family) = named(gallery, &family_id) else {
        return;
    };
    let Some(position) = group_position else {
        show_slot(world, family, false);
        return;
    };
    let (group_index, icons) = &gallery.groups[position];
    let group = &CATALOG_GROUPS[*group_index];
    show_slot(world, family, true);
    for (part, value) in [
        ("index", format!("{:02}", group_index + 1)),
        ("overline", format!("ICON FAMILY · {}", icons.len())),
        ("title", group.title.to_owned()),
        ("desc", group.description.to_owned()),
    ] {
        if let Some(entity) = named(gallery, &format!("virtual-{part}-{slot}")) {
            let _ = set_inner_text(world, entity, value);
        }
    }
    for card_index in 0..CARDS_PER_GROUP {
        let card_id = format!("virtual-card-{slot}-{card_index}");
        let Some(card) = named(gallery, &card_id) else {
            continue;
        };
        let Some(&icon) = icons.get(card_index) else {
            show_slot(world, card, false);
            world.entity_mut(card).remove::<DisplayedIcon>();
            for side in ["light", "dark"] {
                if let Some(entity) = named(gallery, &format!("virtual-{side}-{slot}-{card_index}"))
                {
                    set_image_source(world, entity, None);
                }
            }
            continue;
        };
        show_slot(world, card, true);
        world.entity_mut(card).insert(DisplayedIcon(icon));
        let label = icon
            .name()
            .split('-')
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        for (part, value) in [("label", label), ("name", icon.name().to_owned())] {
            if let Some(entity) = named(gallery, &format!("virtual-{part}-{slot}-{card_index}")) {
                let _ = set_inner_text(world, entity, value);
            }
        }
        for side in ["light", "dark"] {
            if let Some(entity) = named(gallery, &format!("virtual-{side}-{slot}-{card_index}")) {
                if world
                    .get::<ImageMetadata>(entity)
                    .is_some_and(|metadata| metadata.source.is_none())
                {
                    set_image_source(world, entity, Some(icon.source(IconSize::Px32)));
                }
                set_icon_size(world, entity, size);
                set_image_source(world, entity, Some(icon.source(size)));
            }
        }
    }
}

#[component_update]
fn paginate_catalog(
    mut activations: MessageReader<ControlActivated>,
    ids: Query<&ElementId>,
    gallery: Option<Res<PagedGallery>>,
    mut settings: ResMut<CatalogSettings>,
) {
    for activation in activations.read() {
        let Ok(id) = ids.get(activation.entity) else {
            continue;
        };
        if !matches!(id.0.as_str(), "page-prev" | "page-next") {
            continue;
        }
        let max_page = gallery
            .as_ref()
            .filter(|gallery| {
                gallery.query == settings.query && gallery.category == settings.category
            })
            .map(|gallery| page_count(gallery.groups.len()) - 1)
            .unwrap_or_else(|| {
                page_count(filtered_groups(&settings.query, &settings.category).len()) - 1
            });
        match id.0.as_str() {
            "page-prev" => settings.page = settings.page.saturating_sub(1),
            "page-next" => settings.page = (settings.page + 1).min(max_page),
            _ => {}
        }
    }
}

#[component_update]
fn update_paged_gallery(world: &mut World) {
    let Some(settings) = world.get_resource::<CatalogSettings>() else {
        return;
    };
    let (query, category, requested_page, size) = (
        settings.query.clone(),
        settings.category.clone(),
        settings.page,
        settings.size,
    );
    if world.get_resource::<PagedGallery>().is_some_and(|gallery| {
        gallery
            .handles
            .get("catalog-scroll")
            .is_some_and(|entity| world.get::<ElementId>(*entity).is_some())
            && gallery.query == query
            && gallery.category == category
            && gallery.page == Some(requested_page)
            && gallery.size == Some(size)
    }) {
        return;
    }
    let mut gallery = world.remove_resource::<PagedGallery>().unwrap_or_default();
    if gallery.handles.is_empty() {
        gallery.handles = world
            .query::<(Entity, &ElementId)>()
            .iter(world)
            .map(|(entity, id)| (id.0.clone(), entity))
            .collect();
    }
    let Some(scroll_entity) = named(&gallery, "catalog-scroll")
        .filter(|entity| world.get::<ElementId>(*entity).is_some())
    else {
        gallery.handles.clear();
        world.insert_resource(gallery);
        return;
    };
    let filters_changed =
        gallery.page.is_none() || gallery.query != query || gallery.category != category;
    if filters_changed {
        gallery.query = query;
        gallery.category = category;
        gallery.groups = filtered_groups(&gallery.query, &gallery.category);
        let total = gallery
            .groups
            .iter()
            .map(|(_, icons)| icons.len())
            .sum::<usize>();
        if let Some(entity) = named(&gallery, "results-count") {
            let _ = set_inner_text(
                world,
                entity,
                format!("{total} {}", if total == 1 { "icon" } else { "icons" }),
            );
        }
        if let Some(entity) = named(&gallery, "empty-state") {
            show_slot(world, entity, total == 0);
        }
    }
    let pages = page_count(gallery.groups.len());
    let page = requested_page.min(pages - 1);
    if page != requested_page {
        world.resource_mut::<CatalogSettings>().page = page;
    }
    if gallery.page != Some(page) || filters_changed || gallery.size != Some(size) {
        if let Some(mut scroll) = world.get_mut::<ScrollPosition>(scroll_entity) {
            scroll.0.y = 0.0;
        }
        let start = page * GROUPS_PER_PAGE;
        for slot in 0..GROUPS_PER_PAGE {
            let position = start + slot;
            bind_gallery_slot(
                world,
                &gallery,
                slot,
                (position < gallery.groups.len()).then_some(position),
                size,
            );
        }
        if let Some(entity) = named(&gallery, "page-status") {
            let _ = set_inner_text(world, entity, format!("Page {} of {}", page + 1, pages));
        }
        if let Some(entity) = named(&gallery, "page-prev") {
            set_pager_button(world, entity, page > 0);
        }
        if let Some(entity) = named(&gallery, "page-next") {
            set_pager_button(world, entity, page + 1 < pages);
        }
        gallery.page = Some(page);
        gallery.size = Some(size);
    }
    world.insert_resource(gallery);
}

#[component_update]
fn sync_catalog_icons(
    settings: Res<CatalogSettings>,
    images: Query<(
        Entity,
        &ElementClasses,
        Ref<ImageMetadata>,
        &bevy::ui::widget::ImageNode,
    )>,
    mut commands: Commands,
) {
    let settings_changed = settings.is_changed();
    for (entity, classes, metadata, image) in &images {
        if !classes.classes.iter().any(|class| class == "catalog-icon") {
            continue;
        }
        if !settings_changed && !metadata.is_changed() {
            continue;
        }
        let Some((_, current_size)) = metadata
            .source
            .as_deref()
            .and_then(tilt_ui::tilt_ui_icons::parse_source)
        else {
            continue;
        };
        if current_size == settings.size && image.color == settings.tint {
            continue;
        }
        let size = settings.size;
        let tint = settings.tint;
        commands.queue(move |world: &mut World| {
            set_icon_size(world, entity, size);
            if let Some(mut image) = world.get_mut::<bevy::ui::widget::ImageNode>(entity)
                && image.color != tint
            {
                image.color = tint;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_icon_has_exactly_one_catalog_group() {
        let names = CATALOG_GROUPS
            .iter()
            .flat_map(|group| group.names.iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), Icon::ALL.len());
        assert_eq!(
            names.iter().copied().collect::<HashSet<_>>().len(),
            Icon::ALL.len()
        );
        assert!(names.iter().all(|name| Icon::from_name(name).is_some()));
        assert_eq!(
            filtered_groups("", "animated")
                .iter()
                .map(|(_, icons)| icons.len())
                .sum::<usize>(),
            30
        );
        assert_eq!(
            filtered_groups("", "bevy")
                .iter()
                .map(|(_, icons)| icons.len())
                .sum::<usize>(),
            5
        );
        for category in ["nature", "animals-more", "apps-more", "utilities"] {
            assert!(
                filtered_groups("", category)
                    .iter()
                    .flat_map(|(_, icons)| icons)
                    .count()
                    >= 20,
                "empty expanded category: {category}"
            );
        }
    }

    #[test]
    fn gallery_uses_a_bounded_reusable_pool() {
        let html = include_str!("catalog.component.html");
        assert_eq!(
            html.matches("class=\"icon-card slot-hidden\"").count(),
            GROUPS_PER_PAGE * CARDS_PER_GROUP
        );
        assert_eq!(
            html.matches("class=\"catalog-icon\"").count(),
            GROUPS_PER_PAGE * CARDS_PER_GROUP * 2
        );
        assert!(html.contains("All categories"));
        assert!(html.contains("value=\"animated\""));
        assert!(html.contains("id=\"page-prev\""));
        assert!(html.contains("id=\"page-next\""));
    }

    #[test]
    fn page_count_covers_all_filtered_groups_without_empty_extra_page() {
        let groups = filtered_groups("", "all");
        assert_eq!(groups.len(), 58);
        assert_eq!(page_count(groups.len()), 15);
        assert_eq!(page_count(4), 1);
        assert_eq!(page_count(5), 2);
        assert_eq!(page_count(0), 1);
        let visited = (0..page_count(groups.len()))
            .flat_map(|page| {
                groups
                    .iter()
                    .skip(page * GROUPS_PER_PAGE)
                    .take(GROUPS_PER_PAGE)
            })
            .flat_map(|(_, icons)| icons.iter())
            .count();
        assert_eq!(visited, Icon::ALL.len());
    }

    #[test]
    fn search_matches_names_and_families() {
        let matches = filtered_groups("circle", "home");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1, vec![Icon::HomeCircle]);
        assert!(filtered_groups("impossible", "all").is_empty());
        assert_eq!(
            filtered_groups("", "all")
                .iter()
                .map(|(_, icons)| icons.len())
                .sum::<usize>(),
            Icon::ALL.len()
        );
    }

    #[test]
    fn pager_moves_between_pages_and_stops_at_the_edges() {
        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .insert_resource(CatalogSettings::default())
            .insert_resource(PagedGallery {
                groups: filtered_groups("", "all"),
                query: String::new(),
                category: "all".into(),
                ..Default::default()
            })
            .add_systems(Update, paginate_catalog);
        let previous = app.world_mut().spawn(ElementId("page-prev".into())).id();
        let next = app.world_mut().spawn(ElementId("page-next".into())).id();
        let emit = |app: &mut App, entity| {
            app.world_mut()
                .resource_mut::<bevy::ecs::message::Messages<ControlActivated>>()
                .write(ControlActivated { entity });
            app.update();
        };
        emit(&mut app, previous);
        assert_eq!(app.world().resource::<CatalogSettings>().page, 0);
        let last_page = page_count(filtered_groups("", "all").len()) - 1;
        for _ in 0..last_page + 2 {
            emit(&mut app, next);
        }
        assert_eq!(app.world().resource::<CatalogSettings>().page, last_page);
        emit(&mut app, previous);
        assert_eq!(app.world().resource::<CatalogSettings>().page, last_page - 1);
    }

    #[test]
    fn snippets_use_selected_resolution() {
        assert_eq!(
            icon_html(Icon::Bevy, IconSize::Px64),
            "<icon name=\"bevy\" size=\"64\" />"
        );
        assert_eq!(
            icon_image_html(Icon::Bevy, IconSize::Px16),
            "<img src=\"tilt-icon:bevy@16\" alt=\"bevy\" />"
        );
    }
}
