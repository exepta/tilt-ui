use bevy::{asset::AssetServer, image::Image, ui::widget::ImageNode};
use tilt_ui_core::TemplateAttribute;

/// Creates an image node from a static template source when one is available.
pub(crate) fn image_node(
    attributes: &[TemplateAttribute],
    asset_server: Option<&AssetServer>,
) -> ImageNode {
    let Some(source) = attributes.iter().find_map(|attribute| match attribute {
        TemplateAttribute::Static { name, value } if name == "src" && !value.is_empty() => {
            Some(value)
        }
        _ => None,
    }) else {
        return ImageNode::default();
    };
    let Some(asset_server) = asset_server else {
        return ImageNode::default();
    };

    ImageNode::new(asset_server.load::<Image>(source.to_owned()))
}
