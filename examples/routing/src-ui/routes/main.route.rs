use tilt_ui::{Routes, load};

pub(super) fn routes() -> Routes {
    Routes::new()
        .route("/", super::component("home"))
        .route("/settings", load!(super::component("settings")))
        .redirect("/preferences", "/settings")
}
