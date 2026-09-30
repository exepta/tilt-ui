# Checkbox SVG markers

`checked-icon` and `unchecked-icon` select local SVG files for a checkbox's `::mark` part. `unchecked-icon` is optional. Paths are relative to `UiRuntimeConfiguration.assets_path` inside the configured TiltUI source root; `tilt-ui://...` paths start directly at that root.

```html
<checkbox class="favorite" checked-icon="media/heart-filled.svg"
          unchecked-icon="media/heart-outline.svg">Favorite</checkbox>
```

```css
checkbox.favorite::indicator,
checkbox.favorite:checked::indicator {
    width: 22px;
    height: 22px;
    border-width: 0px;
    background-color: transparent;
}
checkbox.favorite::mark { width: 20px; height: 20px; color: #A833EA; }
checkbox.favorite:checked::mark { color: #D52784; }
```

Native builds with the `svg` feature rasterize each distinct file once to a small alpha mask. The marker uses the SVG's transparency and CSS `color`; colors embedded in the SVG are intentionally ignored. Static SVG paths, shapes, groups, transforms, fills, strokes, and `viewBox` work through `resvg`. Animated SVG, scripts, external resources, non-SVG files, and files larger than 1 MiB are not supported as markers. Missing or unsupported checked icons use the usual checkmark; missing or unsupported unchecked icons leave the box empty. Builds without native SVG support use those same fallbacks.
