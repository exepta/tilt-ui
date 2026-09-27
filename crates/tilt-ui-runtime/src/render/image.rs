use bevy::{ecs::world::World, ui::widget::ImageNode};
use tilt_ui_core::TemplateAttribute;

/// Creates an image node from a static template source when one is available.
pub(crate) fn image_node(world: &mut World, attributes: &[TemplateAttribute]) -> ImageNode {
    let Some(source) = attributes.iter().find_map(|attribute| match attribute {
        TemplateAttribute::Static { name, value } if name == "src" && !value.is_empty() => {
            Some(value)
        }
        _ => None,
    }) else {
        return ImageNode::default();
    };

    crate::widgets::content::image::load_image_handle(world, source)
        .map(ImageNode::new)
        .unwrap_or_default()
}
