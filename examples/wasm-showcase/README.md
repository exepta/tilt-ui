# TiltUI WASM showcase

From this directory, run `trunk serve --open` or `trunk build`.
The root `index.html` is Trunk's browser shell. `src-ui/index.html` is
TiltUI's required UI document; Trunk copies the complete `src-ui` directory
so the `tilt-ui://` asset source can load the document, linked CSS, and
component templates in the browser.
The SVG image uses the `.svg` asset loader. With no `lang` attribute in
`src-ui/index.html`, Fluent starts in the browser language when available and
falls back to English. The example includes English and German catalogs.
