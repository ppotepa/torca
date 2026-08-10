# torca_ui

Presentation-only Flutter package for Torca. It owns theme families, palette variants,
design tokens, icon mappings and reusable visual primitives. It must not import Torca
gateway, contract, runtime or domain types.

## Families

- **Modern** — Ocean, Graphite and Forest palettes with Hero Icons.
- **Terminal** — Gruvbox, Dracula and Solarized palettes with PixelArt Icons and
  Press Start 2P display typography.

Both families support light/dark brightness and compact/comfortable density. The
application owns persistence of the selected `TorcaAppearance`; this package only
describes and renders it.

## Assets and licenses

- Hero Icons and PixelArt Icons are provided by `iconsx_plus` (MIT; individual source
  icon projects retain their respective open-source licenses).
- Press Start 2P is bundled under the SIL Open Font License in `assets/fonts/OFL.txt`.
