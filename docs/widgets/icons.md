# Built-in icons

Enable the catalog only in applications that use it:

```toml
tilt-ui = { version = "0.1", features = ["tilt-icons"] }
```

`tilt-icons` is off by default. It adds the optional `tilt-ui-icons` crate to the build. The crate contains SVG geometry, not a font or external asset files. The runtime rasterizes an icon on first use and shares its Bevy image handle between all uses of the same name and physical size. Supported output sizes are **16, 32, and 64 pixels**.

The runnable [icons-catalog example](../../examples/icons-catalog/README.md) shows all 537 names on light and dark surfaces across fifteen pages, with four families per page. Its search filters icons by name or family, and the category picker can show all categories or only animated icons, alongside live theme, color and size controls, plus a preset palette marked with the Bevy symbol. Click an icon to see HTML and CSS snippets in a dialog. The search icon keeps its own 16 px size and color as those controls change.

Use the semantic element in a component. Its `name` and `size` are static attributes; CSS `width` and `height` can change its layout size, while `color` tints the white SVG mask:

```html
<icon name="home" size="32" class="navigation-icon" />
<icon name="settings-sliders" size="16" />
<img src="tilt-icon:briefcase@64" alt="Work" />
<icon name="chart-line" size="32" loading="lazy" lazy-buffer="600" />
```

```css
.navigation-icon { color: #7254C9; }
.tile { background-image: url("tilt-icon:home-filled@64"); color: #7254C9; }
```

The same `tilt-icon:name@size` source works in `<img src>`, CSS `background-image`, and `set_image_source`. `<icon>` and catalog-backed `<img>` use CSS `color` for their tint. The regular background image path also accepts the source and uses CSS `color` as its tint. Use `background-size` and `background-position` as for other image backgrounds.

Rust can access the source SVG or obtain a cached Bevy image handle:

```rust
use tilt_ui::{Icon, IconSize, icon_image};

let svg = Icon::Home.svg(IconSize::Px32);
let source = Icon::Home.source(IconSize::Px32); // "tilt-icon:home@32"
let image_handle = icon_image(world, Icon::Home, IconSize::Px32);
```

The enum also exposes `Icon::ALL`, `Icon::name`, and `Icon::from_name` for search and pickers. Names are kebab-case; snake_case is accepted by `from_name`.
`set_icon_size(world, entity, IconSize::Px64)` switches an existing `<icon>` to another cached resolution without rebuilding the element. CSS `width` and `height` still override its layout size.

Use `loading="lazy"` for icons in a scrollable list. TiltUI creates their layout nodes immediately, then rasterizes icons only when they enter the scroll viewport or the surrounding buffer. The optional `lazy-buffer` is a distance in pixels (default 600, clamped to 0–4096). Once an icon leaves that area or a parent becomes hidden, its image handle is released; scrolling back loads it again. The icon cache keeps asset IDs rather than strong handles, so offscreen images can be reclaimed by Bevy while images still in use remain shared. This applies to `<icon>` elements; normal images retain their existing loading behavior.

| Family | Available names |
| --- | --- |
| Home (5) | `home`, `home-filled`, `home-modern`, `home-roof`, `home-circle` |
| Settings (8) | `settings`, `settings-filled`, `settings-sliders`, `settings-tune`, `settings-wrench`, `settings-adjust`, `settings-circle`, `settings-horizontal` |
| User (6) | `user`, `user-filled`, `user-circle`, `user-square`, `user-plus`, `user-group` |
| Menu (10) | `menu`, `menu-wide`, `menu-compact`, `menu-dots`, `menu-dots-vertical`, `menu-grid`, `menu-grid-filled`, `menu-list`, `menu-chevron`, `menu-circle` |
| Close (4) | `close`, `close-circle`, `close-square`, `close-bold` |
| Briefcase (4) | `briefcase`, `briefcase-filled`, `briefcase-medical`, `briefcase-business` |
| General (41) | `search`, `bell`, `heart`, `star`, `check`, `plus`, `minus`, `arrow-left`, `arrow-right`, `chevron-left`, `chevron-right`, `chevron-down`, `download`, `upload`, `trash`, `edit`, `mail`, `calendar`, `clock`, `folder`, `file`, `lock`, `eye`, `sun`, `moon`, `copy`, `link`, `external-link`, `share`, `filter`, `refresh`, `bookmark`, `map-pin`, `info`, `warning`, `help-circle`, `check-circle`, `play`, `pause`, `image`, `phone` |
| Navigation (12) | `dashboard`, `list`, `grid`, `layers`, `target`, `globe`, `compass`, `map`, `route`, `arrow-up`, `arrow-down`, `chevron-up` |
| Devices & more (12) | `camera`, `video`, `music`, `mic`, `volume`, `wifi`, `battery`, `monitor`, `smartphone`, `printer`, `gift`, `shopping-cart` |
| Actions (10) | `check-check`, `circle-x`, `ellipsis`, `ellipsis-vertical`, `rotate-ccw`, `rotate-cw`, `undo-2`, `redo-2`, `maximize`, `minimize` |
| Files & cloud (10) | `file-plus`, `file-minus`, `file-check`, `file-search`, `folder-open`, `folder-plus`, `archive`, `cloud`, `cloud-upload`, `cloud-download` |
| Commerce (10) | `shopping-bag`, `credit-card`, `wallet`, `receipt`, `tag`, `tags`, `percent`, `coins`, `banknote`, `store` |
| Communication (10) | `message-square`, `message-circle`, `send`, `inbox`, `at-sign`, `hash`, `headphones`, `megaphone`, `radio`, `contact` |
| Media (10) | `skip-back`, `skip-forward`, `rewind`, `fast-forward`, `square`, `circle-dot`, `volume-x`, `film`, `scan`, `picture-in-picture` |
| Editor (10) | `crop`, `scissors`, `brush`, `paint-bucket`, `pen-tool`, `eraser`, `ruler`, `move`, `list-indent-increase`, `list-indent-decrease` |
| Weather & nature (10) | `cloud-sun`, `cloud-rain`, `cloud-snow`, `wind`, `umbrella`, `thermometer`, `droplet`, `flame`, `leaf`, `mountain` |
| Data & code (10) | `chart-column`, `chart-line`, `chart-pie`, `trending-up`, `trending-down`, `activity`, `database`, `server`, `terminal`, `code` |
| Security (10) | `shield`, `shield-check`, `key`, `scan-face`, `lock-open`, `user-round-check`, `user-round-minus`, `log-in`, `log-out`, `contact-round` |
| Travel & places (10) | `car`, `bus`, `train-front`, `plane`, `bike`, `ship`, `building`, `coffee`, `utensils`, `flag` |

The new nature, animal, app and utility families adapt geometry from [Lucide](https://github.com/lucide-icons/lucide), [Tabler Icons](https://github.com/tabler/tabler-icons), [Phosphor Icons](https://github.com/phosphor-icons/core) and [Material Design Icons](https://github.com/Templarian/MaterialDesign-SVG). Their pinned sources and license notices are recorded in [additional icon sources](../../crates/tilt-ui-icons/ADDITIONAL-ICON-SOURCES.md).

The embedded icons use static SVG paths, rectangles, circles, fills and strokes. Exported SVGs use `currentColor`; the runtime renders their geometry through `resvg` once per icon and size, then stores a tintable alpha mask. Scripts, SVG-embedded animation, external references and font files are not part of the catalog; the 30 animated icons use runtime UI transforms. Since the SVG data is embedded, catalog loading requires no filesystem paths or network access, including on WASM. Arbitrary user-provided SVG files follow TiltUI's separate image loader and its platform support.

The Bevy bird is available as five SVG-backed names: `bevy`, `bevy-circle`, `bevy-badge`, `bevy-orbit`, and `bevy-sparkle`. The standalone SVG files are in [`assets/bevy`](../../crates/tilt-ui-icons/assets/bevy/README.md). Thirty icons animate when used as `<icon>`, including `loading-spin`, `heart-beat`, `bevy-flight`, and `rocket-launch`. Animation changes the UI transform of visible icons; it does not rasterize a new SVG every frame.

The catalog includes 54 app marks. The earlier 24, such as `discord`, `steam`, `github`, and `figma`, come from [Simple Icons](https://github.com/simple-icons/simple-icons) at a pinned commit. The new browser, developer and community marks come from Tabler Icons. See the [source and entry-license table](../../crates/tilt-ui-icons/SIMPLE-ICONS-SOURCES.md); trademark and brand-use rules remain with the respective owners. `bevy-flight` animates the Bevy bird. Animated icons update visible UI transforms at 30 FPS and reuse their rasterized SVG textures.
