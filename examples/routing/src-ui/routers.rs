//! Central route registry. Route modules may live anywhere below `src-ui`.

use tilt_ui::{ComponentId, Routes, ui_routes};

#[path = "features/lab.route.rs"]
mod lab;
#[path = "routes/main.route.rs"]
mod main;

fn component(name: &str) -> ComponentId {
    super::tilt_ui_component_id(name)
        .unwrap_or_else(|| panic!("unknown component in route table: {name}"))
}

#[ui_routes]
pub fn app_routes() -> Routes {
    main::routes().merge(lab::routes())
}
