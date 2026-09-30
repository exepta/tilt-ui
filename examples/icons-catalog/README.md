# Icon catalog example

Run the complete, interactive icon gallery with:

```sh
cargo run -p tilt-ui-example-icons-catalog
```

All 537 icons from `tilt-ui-icons` appear on light and dark preview surfaces. The gallery fills the application window and has its own scrollable area, while the search and controls remain visible. Search across all pages by icon name or family and filter by category or animated icons; the result count and visible groups update as you type, and Clear restores the complete catalog. Click any icon card to open a dialog with a larger preview and copyable `<icon>`, `<img>` and CSS snippets for the selected size. The toolbar switches the page theme, changes the tint of catalog previews through the ColorPicker, and changes their native resolution between 16, 32 and 64 pixels. The Bevy-marked palette provides five quick colors. The search icon stays at 16 pixels with its own color. Pagination shows four icon families at a time, using a fixed pool of at most 48 cards; gallery previews load only near the scroll viewport with a 400 px buffer and release their image handles when scrolled away. Visible previews of the same icon share one Bevy image asset.

The catalog includes nature, animal and utility icons, 54 app marks (including Discord, Steam, Firefox and React), and five reusable Bevy bird SVG variants (`bevy`, `bevy-circle`, `bevy-badge`, `bevy-orbit`, `bevy-sparkle`) and 30 animated `<icon>` variants, including `bevy-flight`. Click a card to copy the exact HTML for use in another UI.

The ColorPicker palette flips at window edges and uses a shorter color canvas in compact windows without a popup scrollbar.
