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

## Original drawing, no JEI material

Panels, slots, tabs and arrows are drawn with primitives (`recipe_paint.rs`);
items and text use Horizon's own item renderer and fonts. The layout follows
public JEI screenshots and control documentation only
(https://www.curseforge.com/minecraft/mc-mods/jei). No JEI implementation,
textures or licence files are part of this repository, and `cargo xtask check`
rejects reintroducing a JEI asset folder. Avoiding textures also removes PNG
decoding and per-frame atlas uploads.

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
