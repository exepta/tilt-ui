use std::collections::HashSet;
use tilt_ui_icons::*;

#[test]
fn names_are_unique_and_round_trip() {
    let names: HashSet<_> = Icon::ALL.iter().map(|icon| icon.name()).collect();
    assert_eq!(names.len(), Icon::ALL.len());
    assert_eq!(Icon::ALL.len(), 537);
    for &icon in Icon::ALL {
        assert_eq!(Icon::from_name(icon.name()), Some(icon));
        assert_eq!(Icon::from_name(&icon.name().replace('-', "_")), Some(icon));
    }
}

#[test]
fn workflow_and_feedback_icons_are_available() {
    for name in [
        "copy",
        "link",
        "external-link",
        "share",
        "filter",
        "refresh",
        "bookmark",
        "map-pin",
        "info",
        "warning",
        "help-circle",
        "check-circle",
        "play",
        "pause",
        "image",
        "phone",
    ] {
        assert!(Icon::from_name(name).is_some(), "missing {name}");
    }
}

#[test]
fn navigation_and_device_icons_are_available() {
    for name in [
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
    ] {
        assert!(Icon::from_name(name).is_some(), "missing {name}");
    }
}

#[test]
fn requested_families_and_sizes_are_present() {
    for (prefix, count) in [
        ("home", 5),
        ("settings", 8),
        ("user", 8),
        ("menu", 10),
        ("close", 4),
        ("briefcase", 4),
    ] {
        assert!(
            Icon::ALL
                .iter()
                .filter(|icon| icon.name().starts_with(prefix))
                .count()
                >= count,
            "{prefix}"
        );
    }
    for size in [IconSize::Px16, IconSize::Px32, IconSize::Px64] {
        let svg = Icon::Home.svg(size);
        assert!(svg.contains(&format!("width=\"{}\"", size.pixels())));
        assert!(svg.contains("viewBox=\"0 0 24 24\""));
        assert!(svg.contains("currentColor"));
        assert_eq!(
            Icon::Home.source(size),
            format!("tilt-icon:home@{}", size.pixels())
        );
    }
}

#[test]
fn source_parser_rejects_unknown_names_and_unsupported_sizes() {
    assert_eq!(
        parse_source("tilt-icon:briefcase-business@64"),
        Some((Icon::BriefcaseBusiness, IconSize::Px64))
    );
    assert_eq!(parse_source("tilt-icon:home@24"), None);
    assert_eq!(parse_source("tilt-icon:unknown@32"), None);
    assert_eq!(parse_source("home@32"), None);
}

#[test]
fn bevy_svg_variants_and_animated_icons_are_reusable() {
    for name in [
        "bevy",
        "bevy-circle",
        "bevy-badge",
        "bevy-orbit",
        "bevy-sparkle",
    ] {
        let icon = Icon::from_name(name).unwrap();
        for size in [IconSize::Px16, IconSize::Px32, IconSize::Px64] {
            assert!(icon.svg(size).contains("currentColor"));
            assert_eq!(parse_source(&icon.source(size)), Some((icon, size)));
        }
    }
    assert_eq!(
        Icon::ALL.iter().filter(|icon| icon.is_animated()).count(),
        30
    );
}

#[test]
fn animals_apps_and_new_animations_are_available() {
    for name in [
        "cat", "dog", "rabbit", "turtle", "discord", "steam", "github", "figma",
    ] {
        let icon = Icon::from_name(name).unwrap_or_else(|| panic!("missing {name}"));
        assert!(!icon.body().is_empty());
        for size in [IconSize::Px16, IconSize::Px32, IconSize::Px64] {
            assert_eq!(parse_source(&icon.source(size)), Some((icon, size)));
        }
    }
    for name in [
        "bevy-flight",
        "discord-pulse",
        "cat-bounce",
        "rocket-launch",
    ] {
        assert!(Icon::from_name(name).unwrap().is_animated());
    }
}

#[test]
fn expanded_families_resolve_at_every_size() {
    for name in [
        "flower",
        "cloud-lightning",
        "volcano",
        "butterfly",
        "cow",
        "penguin",
        "firefox",
        "react",
        "slack",
        "clipboard-copy",
        "settings-automation",
    ] {
        let icon = Icon::from_name(name).unwrap_or_else(|| panic!("missing {name}"));
        assert!(!icon.body().is_empty());
        for size in [IconSize::Px16, IconSize::Px32, IconSize::Px64] {
            assert_eq!(parse_source(&icon.source(size)), Some((icon, size)));
            assert!(icon.svg(size).contains("currentColor"));
        }
    }
}
