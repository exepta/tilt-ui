# Routing showcase

Run `cargo run -p tilt-ui-example-routing --bin routing-showcase`.

The central route registry is [`src-ui/routers.rs`](src-ui/routers.rs). TiltUI's build step includes this optional file through `include_components!()`, so `src/main.rs` only sets up the app. The registry explicitly imports and merges [`routes/main.route.rs`](src-ui/routes/main.route.rs) and [`features/lab.route.rs`](src-ui/features/lab.route.rs). The folder name is your choice: a `*.route.rs` file can live anywhere under `src-ui` and can be named `main`, `bob`, or anything else. Import it in `routers.rs` with a relative `#[path = "..."]` declaration, then merge its `Routes` table.

The dashboard shows three lifetimes. Home is transient and receives a new instance on each visit. `load!(settings)` creates a cached page with the outlet; `lazy!(details)` creates it on first visit and keeps it afterward. Type into the Settings or Details input, navigate away, and return to see the local widget state persist. Redirect and fallback are available in the sidebar.
