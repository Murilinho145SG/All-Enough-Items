# AEI design notes

Structural decisions behind the AEI bundle and the reasons for them.

## Engine owns the viewer geometry

The recipe window's geometry, pagination and hit areas live in the
Horizon-independent engine (`mixins/aei/engine/src/viewer.rs`). Rendering, hit
testing and scrolling consume the same rectangles, so clicks, hover and wheel
input always agree with what is drawn, and blank areas of the window never reach
the container underneath. This keeps the geometry testable with plain
`cargo test`; the Horizon adapters (`glue.rs`, `centered_recipe_overlay.rs`)
stay thin.

Crafting recipes keep their full grid positions rather than shrinking with the
ingredient count, and malformed slots cannot escape their card or shift the
indexes of the remaining slots (output-slot indexes are preserved when
unsupported inputs are hidden).

## JEI's look, without JEI material

The recipe screen and item list reproduce JEI's interface. Measurements and
behaviour were taken from JEI's public MIT-licensed source and its documentation
(https://www.curseforge.com/minecraft/mc-mods/jei), written down as numbers and
reimplemented here; no JEI code, textures or licence files are part of this
repository, and `cargo xtask check` rejects reintroducing a JEI asset folder.

- Recipe window: 198 wide, height `screen - 58` clamped to 175..350, top at
  y = 45 (plus half of any height above 350). Two 13-px header rows (category,
  page) with a translucent strip between the arrows and white shadowed text.
- Recipes per page: as many rows and up to two columns of cells as fit the area
  below the header, spread with equal vertical gaps. Each recipe sits on a
  bordered plate with the "+" button 2 px to its right, flush with its bottom.
- Per-category content sizes and slot positions (crafting 116x54 with a 26-px
  output slot, furnace 116x54, blast furnace and smoker 82x54, campfire 82x44,
  stonecutting 82x34, smithing 108x28, brewing 114x61). Cooking shows the
  experience top right and the cook time bottom centre in grey, with the
  vanilla flame and arrow sprites animated over the cook time.
- Category tabs (24 px, eight per row) overlap the window top by 3 px; the
  station tab sits left of the window, open on its right.
- Item list: up to 9 x 16 cells aligned to the right and top, 6 px from the
  screen edge, shrinking to the rows a short list needs; a 20-px page bar above
  it and a search field with a settings button at the bottom. It replaces
  Horizon's default panel whenever AEI owns the panel, recipe open or not.

Window frames, slots, plates and tabs are drawn with fills in `recipe_paint.rs`.
Buttons, page arrows, the search field and the settings icon use the JEI-style
sprites Horizon ships in its own atlas (`horizon/jei/...`, `horizon/recipes/
settings`), and the flame, cook arrow and brewing art use vanilla GUI sprites,
so AEI itself carries no images.

## Separate lookup, presentation and history

`browser.rs` only dispatches events. Catalogue filtering and the search grammar
are in `browser/search.rs`, recipe indexing and presentation in
`browser/recipes.rs`, and lookup history in `browser/history.rs`.

- Recipes are indexed once per recipe table instead of scanning every recipe
  per lookup.
- Registry indexes are kept independent of catalogue truncation, so recipe
  ingredient indexes keep their meaning in large registries.
- History is a single navigation stack: Back returns to the catalogue and
  Forward can reopen the first lookup.

## Recipe-table lifecycle

`glue.rs` tracks the identity of Horizon's recipe table and the set of known
recipes. A new table rebuilds the browser; a change in recipe knowledge
refreshes the placement (`+`) actions. Shutdown clearing happens in one place.

Recipe placement always goes through the vanilla recipe book, so learned
recipes and the server's rules stay authoritative.

## Workspace layout

The repository root is a virtual Cargo workspace with two members: the engine
(`mixins/aei/engine`) and `xtask`, which holds the development commands so the
whole project is driven through Cargo. Inside the bundle, `modules/` holds the
support modules injected into `horizon-client` and `hooks/` holds one helper per
injection, so `horizon-mixin.toml` reads as a map of the integration. The engine
crate stays self-contained (no `workspace = true` fields) because Horizon copies
it out of this workspace to build it.

## Compatibility pins in one place

The Horizon revision, horizon-mixins revision, hook version, mod id and version
appear in `compatibility.json`, `horizon-native.json`,
`mixins/aei/horizon-mixin.toml` and the engine's `Cargo.toml`. `cargo xtask check`
fails if any of them disagree, and also checks that every file under
`mixins/aei/` is one Horizon accepts in a bundle. Injected helpers are not part
of the Cargo build, so the check formats `modules/` and `hooks/` with `rustfmt`
separately.

## Known contract limits

These need Horizon native-contract support and are documented rather than
worked around:

- Dynamic anvil, grindstone, fuel and compost categories are not exposed.
- A query or category shows at most 256 recipes (the native page limit).
- Third-party panels in the creative inventory are not supported.

## Verification

- `cargo xtask check`: pins, bundle contents, formatting, Clippy and the
  engine regression tests.
- `horizon-mixins check` against a clean checkout of the pinned Horizon revision
  compiles and tests the injected client (see the README).
- `cargo xtask ui-test` drives a disposable running game through pagination,
  wheel input, categories, history, search and modal clicks, and saves
  screenshots outside the repository.
