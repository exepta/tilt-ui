# Sources for the 100 expanded catalog icons

The 100 icons added in the nature, animal, app and utility families embed SVG geometry from these pinned upstream revisions:

| Source | Revision | Families | License notice |
| --- | --- | --- | --- |
| [Tabler Icons](https://github.com/tabler/tabler-icons) | `74929e50416e2b7c0abb8368cdc74bdcb2560ab6` | Plants, landscapes, animals, app marks, utilities | [MIT](TABLER-LICENSE) |
| [Lucide Icons](https://github.com/lucide-icons/lucide) | `66d8f9fc394b8530377e5f6112f0b8908ba01280` | Sky and landscape icons | [ISC/MIT](LUCIDE-LICENSE) |
| [Phosphor Icons](https://github.com/phosphor-icons/core) | `2b75f3ad12b420c9504ef05df8d2564a28f8500e` | Cow, beetle and simple fish | [MIT](PHOSPHOR-LICENSE) |
| [Material Design Icons](https://github.com/Templarian/MaterialDesign-SVG) | `9e04201d4557e729822fb57f62a316c3dea1d4a8` | Bee, elephant, kangaroo, owl, penguin, shark and snake | [Pictogrammers Free License / Apache 2.0](MDI-LICENSE) |

The original 24 app marks are separately documented in [Simple Icons sources](SIMPLE-ICONS-SOURCES.md). The 30 new Tabler app marks are `firefox`, `chrome`, `safari`, `edge`, `opera`, `vivaldi`, `arc-browser`, `yandex`, `google`, `apple-logo`, `angular`, `react`, `vue`, `svelte`, `typescript`, `javascript`, `python`, `vs-code`, `node-js`, `npm`, `slack`, `zoom`, `linkedin`, `pinterest`, `mastodon`, `signal`, `matrix`, `dropbox`, `trello` and `asana`. Their use remains subject to the respective brand owners' trademark rules.

All imported geometry is embedded in the crate as `currentColor` SVG content inside the shared 24×24 icon wrapper. Each glyph module identifies the upstream revision used for its contents.
