# Content and advanced built-ins

TiltUI materializes these tags directly from component templates. Their visual defaults live in `crates/tilt-ui-runtime/src/theme/default.css`; component CSS has higher cascade priority.

| Element | Tags | Runtime behavior |
| --- | --- | --- |
| Paragraph | `p` | Retains authored text children; no generated label. |
| Headline | `h1` to `h6`, `headline` | Heading level is retained in `HeadlineLevel` and exposed as a `level` attribute for CSS. |
| Image | `img`, `image` | Loads `src` through direct filesystem decoding, SVG rasterization, or Bevy's `AssetServer`; retains `alt`; supports `set_image_source` without replacing the entity; and can preview a linked file input with `preview="input-id"`. |
| Avatar | `avatar` | Uses the image path and a persistent initials fallback derived from `alt`; `set_avatar_source` switches fallback visibility. |
| Badge | `badge` | Displays `value` or `max+` in a persistent text part; `set_badge_value` updates it. `for` resolves a component-local target and `anchor` positions the badge in one of its four corners. |
| Divider | `divider` | Retains horizontal or vertical `alignment`; the line is styled by CSS. |
| ChoiceBox | `choice-box`, `select` | Uses authored `<option>` children, a persistent popup, and one selected option. The popup shows up to three options before scrolling; `max-visible-items` changes that limit. `set_choice_open` controls visibility; arrow, Home, End, and Escape keys navigate/close it. |
| Option | `option` | Retains `value`, visible source text, static `selected`, and `disabled`. |
| ListBox | `list-box` | Displays authored options and supports single or `multiple` selection. Arrow/Home/End keys move through options; Space activates a focused option. |
| ColorPicker | `color-picker`, `colorpicker` | Editable hex, `rgb()`, or `rgba()` value with a saturation/value canvas, hue and alpha tracks, persistent palette, and ten recent colors. Clicking the input opens the popup; HEX/RGB/RGBA buttons change only the displayed notation. |
| DatePicker | `date-picker` | Persistent month calendar with valid ISO `YYYY-MM-DD` values, `min`, `max`, `format` (`mdy`, `dmy`, `ymd`), and component-local `for` resolution to a date input. Clicking the linked input opens its calendar. The closed calendar does not participate in layout. |
| ToolTip | `tooltip` | Resolves an implicit parent or component-local `for` target; supports hover, click, and drag triggers, pointer-follow placement, or target-anchored placement on a requested side with viewport fallback. |
| HyperLink | `hyperlink`, `a` | Retains `href`, uses shared control activation, emits `LinkActivated`, and opens HTTP(S) targets on supported native platforms. |

`OptionSelectionChanged`, `DatePickerChanged`, `ColorPickerChanged`, and `LinkActivated` are semantic Bevy messages. Programmatic setters update state without emitting a user-originated message. Static attributes initialize state once; they do not overwrite later interactions.

For example, `<choice-box max-visible-items="5">` shows up to five options before its popup scrolls. Values below one or nonnumeric values use the default of three.

## CSS parts

`choice-box::value`, `choice-box::indicator`, `choice-box::popup`, `avatar::placeholder`, `badge::value`, `date-picker::calendar`, `date-picker::day`, `date-picker::selected-day`, `date-picker::disabled-day`, and `date-picker::hovered-day` style persistent internal entities. ColorPicker exposes `::preview`, `::popup`, `::canvas`, `::canvas-thumb`, `::hue-track`, `::hue-thumb`, `::alpha-track`, `::alpha-thumb`, `::formats`, `::format`, `::selected-format`, `::swatches`, `::swatch`, `::recent-colors`, and `::recent-color`. Component-authored rules override default-theme rules. ChoiceBox and ColorPicker expose `:open`; invalid color text exposes `:invalid`.

## Color and tooltip examples

```html
<input id="avatar-file" type="file" extensions="[png, jpg, jpeg, svg]" />
<img preview="avatar-file" alt="Selected avatar" />

<color-picker value="rgba(168, 51, 234, 0.8)" format="rgba" />
<button id="help">Help</button>
<tooltip for="help" variant="follow" trigger="hover">Follows the pointer</tooltip>
<tooltip for="help" variant="point" prio="left" trigger="click">Anchored on the left</tooltip>
```

`ColorPickerState.value` remains an RGBA color when the display switches to HEX or RGB. RGB text edits preserve the current alpha channel. Completed canvas/track drags, typed commits, and swatch selections update the ten-entry recent-color history. `Tooltip` accepts `prio="left|right|top|bottom"` (or `priority`) and flips to the opposite side when the preferred side does not fit; `variant="follow"` tracks pointer movement while open.

## Current limits

- `Image` stores `alt` metadata but failed loads do not display `alt` as visible fallback text.
- Tooltip placement currently uses the primary Bevy window as its viewport. Tooltip content is visual-only and does not accept pointer interaction.
- The color canvas is a Bevy image texture rather than an HTML Canvas API surface. A primary press outside the picker closes its popup when Bevy picking reports a target.
- `DatePicker` has pointer selection and month navigation; localized month labels and keyboard calendar navigation remain future work.
- Browser hyperlink opening is event-only until a WASM navigation adapter is added.


## Runtime content and cursors

`set_inner_text(world, entity, text)` replaces a container's content with literal text.
Markup and `{{ ... }}` remain literal. `set_inner_bindings(world, entity, source)`
instead creates reactive text, for example `"Hello {{ user.name }}"`.
`set_inner_html(world, entity, source)` parses a TiltUI template fragment and
replaces the old subtree. This uses TiltUI tags, bindings, event handlers and
registered components/providers, not browser HTML or JavaScript. New nodes use
the host's CSS scope; component-local IDs are updated and removed entities are
despawned. Parse/validation errors leave the existing content intact. Reapplying
identical content keeps the existing entities and control state.

The setters return `Result<(), InnerContentError>` and support `body`, `div`,
`form`, `field-set`, `p`, `label`, headlines, `table-cell` and `button`. Internal
control parts such as scrollbars are preserved. For editable inputs and other
specialized controls, use their dedicated setters. Existing property bindings
on the host remain active and can overwrite a manual content update when their
source changes.

Templates can use `[innerText]="state.message"` (also `textContent`/`text`) and
`[innerHtml]="state.fragment"` (also `innerHTML`), including on empty containers.

Insert `UiCursor(CursorIcon::System(SystemCursorIcon::Crosshair))` on an element,
or call `set_ui_cursor(world, entity, Some(icon))`. For a custom image, pass
`CursorIcon::Custom(CustomCursor::Image(CustomCursorImage { handle, hotspot: (12, 12),
..Default::default() }))`, using Bevy's window cursor types. Keep the hotspot
inside the image dimensions. The nearest explicit cursor on the hovered element
or its ancestors wins over built-in control defaults. Passing `None` removes the
override. Without an override, built-in cursor behavior is retained.

`examples/component-showcase` demonstrates all three content setters, a newly
created HTML button, live progress bindings, a system crosshair and a custom
image cursor.
