# tilt-ui-build

`tilt-ui-build` is the build-time component discovery crate for [TiltUI](https://github.com/exepta/tilt-ui). Applications normally use it as a build dependency alongside the main `tilt-ui` crate.

Add a `build.rs` file to discover `src-ui/index.html` and component files under `src-ui/`:

```rust
fn main() {
    tilt_ui_build::build().expect("TiltUI component discovery failed");
}
```

In the application, `tilt_ui::include_components!()` includes the generated component catalog. Use `build_from` when your UI source directory is not `src-ui/`.

For setup and runnable examples, see the [TiltUI project guide](https://github.com/exepta/tilt-ui#readme).
