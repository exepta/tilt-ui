# TiltUI WASM showcase

From this directory, run `trunk serve --open` or `trunk build`.
The root `index.html` is Trunk's browser shell. `src-ui/index.html` is
TiltUI's required UI document; Trunk copies the complete `src-ui` directory
so the `tilt-ui://` asset source can load the document, linked CSS, and
component templates in the browser.
