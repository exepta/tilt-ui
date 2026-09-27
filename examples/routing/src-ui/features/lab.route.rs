use tilt_ui::{Routes, lazy};

pub(super) fn routes() -> Routes {
    Routes::new()
        .route("/details", lazy!(super::component("details")))
        .fallback(super::component("missing"))
}
