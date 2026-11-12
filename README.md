# All Enough Items

All Enough Items (AEI) is a client-side item and recipe browser, distributed
through Horizon's native mod system.

- Search item names and identifiers, namespaces (`@minecraft`), tags (`#logs`)
  and explicit identifiers (`&minecraft:oak_log`). Combine terms, quoted phrases,
  exclusions (`-`) and alternatives (`|`).
- Hover an item and press **R** for recipes or **U** for uses. In the catalogue,
  left-click shows recipes and right-click shows uses.
- Browse a centered recipe viewer with category tabs above the window, a catalyst
  on the left, category and page navigation, beveled recipe cards, full crafting
  grids and output slots. The item catalogue remains on the right when it fits.
- Click ingredients to follow recipes, click the category title to browse
  recipes from that category, and use **Left/Right** to navigate lookup history.
- Press **Ctrl+F** to focus the search field.
- Scroll or click arrows to turn pages. Right-click the search field to clear it.
- Click **+** to place a known recipe through the vanilla recipe book;
  **Shift-click** requests as many sets as the player's inventory permits.

Recipes, stack components, item rendering, fonts and translations come from
Horizon's native API. Recipe placement remains subject to the player's learned
recipes and the server's normal recipe-book rules. The vanilla recipe book is
preserved.

The viewer follows the public [JEI screenshots and control documentation](https://www.curseforge.com/minecraft/mc-mods/jei).
Its graphics are original drawing primitives; no JEI implementation or assets
are included. Full JEI feature parity is not claimed: the pinned native contract
provides crafting, cooking, stonecutting, smithing and brewing, but not dynamic
anvil, grindstone, fuel or compost categories. AEI extends Horizon's creative
inventory panel guard for its own recipe viewer and leaves the vanilla recipe
book panel available. The viewer works from any inventory screen Horizon opens;
each query/category shows at most 256 recipes, matching the native API limit.

## Install

In Horizon, select **Mods → Add from GitHub**, enter this repository URL and
install AEI (or run `horizon-launcher mods add-github <url> --trust-native-code`).
Horizon displays the native-code trust notice before building the instance.
Native mods run with the player's account permissions.

The launcher installs the latest release's `.hnmod` asset when there is one;
otherwise it builds the pinned commit from source. Both use the revisions in
`horizon-native.json`.

## Develop and verify

```sh
python3 tools/check.py
horizon-mixins check --workspace /path/to/pinned/horizon \
  --mods /path/to/aei/mixins --out /tmp/aei-stage \
  --trust-native-mods --package horizon-client
```

`tools/check.py` validates that the compatibility pins and versions agree, that
every file under `mixins/aei/` is one Horizon accepts in a bundle, source
hygiene, formatting of the engine and the injected helpers, Clippy and the
engine tests.

The second command compiles and tests the injected game in a scratch staging
directory, leaving the Horizon source unchanged. Use a clean git checkout of the
exact revision in `compatibility.json`. Keep game runs and all packaged
artifacts outside the source tree, with fresh game directories and scratch homes
as Horizon requires.

For a disposable game already running with a control socket, run
`python3 tools/test_viewer.py /tmp/test-game/ctl.sock --out /tmp/aei-ui`
to exercise real clicks, scrolling, categories, history and search and capture
screenshots outside the repository.

## Release

1. Bump the version in `Cargo.toml`, `mixins/aei/engine/Cargo.toml`,
   `mixins/aei/horizon-mixin.toml` and `horizon-native.json`. When moving to a
   new Horizon revision, update the pins in `compatibility.json`,
   `horizon-native.json` and `[compatibility]` in `horizon-mixin.toml` together.
2. Run both verification commands above.
3. Run `python3 tools/package.py` to write a reproducible
   `dist/aei-<version>.hnmod` and its `.sha256` (`dist/` is ignored by git).
4. Attach the `.hnmod` to a GitHub release of the tagged commit.

## Structure

All paths below are relative to `mixins/aei/`, the bundle Horizon builds.

- `horizon-mixin.toml`: the mod, its pins, support modules and hook injections.
- `engine/src/browser.rs`: event dispatch.
- `engine/src/browser/search.rs`: catalogue filtering and search grammar.
- `engine/src/browser/recipes.rs`: recipe indexing and presentation.
- `engine/src/browser/history.rs`: lookup history.
- `engine/src/viewer.rs`: renderer-independent recipe geometry and pagination.
- `engine/src/types.rs`: browser models and actions.
- `runtime.rs`: mod identity, key bindings and the engine re-exports.
- `glue.rs`: native table lifecycle, model conversion and actions.
- `centered_recipe_overlay.rs`: recipe rendering and modal interaction.
- `recipe_paint.rs`: original panel, slot, arrow and button drawing.
- The remaining `*.rs` files are the small helpers each injection calls.

Outside the bundle: `src/lib.rs` exposes the engine to Cargo tooling,
`compatibility.json` and `horizon-native.json` hold the distribution pins,
`tools/` holds the check, packaging and UI test scripts, and `docs/design.md`
records the structural decisions and their reasons.

AEI is MIT licensed; see [LICENSE](LICENSE).
