# All Enough Items

All Enough Items (AEI) is a client-side item and recipe browser, distributed
through Horizon's native mod system.

- The item list is the creative inventory's, in its tab order, with every
  variant it lists (enchanted books, potions, tipped arrows...), then any other
  registered item.
- Search with JEI's prefixes: names (plain words), mods (`@minecraft`), tooltip
  lines (`#sharpness`), tags (`$logs`), creative tabs (`%food`) and identifiers
  (`&minecraft:oak_log`). Combine terms, quoted phrases, exclusions (`-`) and
  alternatives (`|`). The settings page sets each prefix to *Enabled* (plain
  words search it too), *Require prefix* or *Disabled*; tooltips and identifiers
  are searched by plain words by default.
- Press **Ctrl+O** to hide or show the overlay (remembered between sessions).
- An optional setting adds the item's mod name to every item tooltip, in blue
  italics as JEI does (off by default, since it changes vanilla tooltips).
- Hover an item and press **R** for recipes or **U** for uses. In the catalogue,
  left-click shows recipes and right-click shows uses.
- Browse recipes in a JEI-style window: category tabs above it, the crafting
  station on the left, category and page arrows, recipes on bordered plates with
  animated furnace and brewing progress, cook time and experience. The item list
  stays on the right, with page arrows above it and the search field below.
- Click ingredients to follow recipes, click the category title to browse every
  recipe (one tab per category), and use **Left/Right** (or **Backspace** to go
  back) to navigate lookup history.
- Ingredients with alternatives cycle every second; hold **Shift** to pause them.
  A slot whose alternatives are exactly a tag says so in its tooltip ("Accepts
  any: #minecraft:planks"), and with advanced tooltips (F3+H) an output shows its
  recipe id.
- Press **A** over any item (the list, a recipe, an inventory slot) to bookmark
  it, as in JEI; bookmarks line up on the left of the screen with their own
  pages, open recipes and uses like the list, and are saved. The ribbon button
  at the top right of each recipe bookmarks that recipe: its output joins the
  list with a gold corner and opens on that recipe.
- Press **Ctrl+F** to focus the search field.
- Scroll or click arrows to turn pages. Right-click the search field to clear it.
- Click **+** to move a recipe's ingredients into the crafting grid through the
  vanilla recipe book (**Shift-click** for as many sets as the inventory allows);
  the recipe view then closes. The button only shows on crafting recipes that fit
  the open grid (2x2 in the inventory, 3x3 at a crafting table). It is orange
  when ingredients are missing (the grid shows the recipe as a ghost) and dark
  when the recipe isn't unlocked in the player's recipe book.

Recipes, stack components, item rendering, fonts and translations come from
Horizon's native API. Recipe placement remains subject to the player's learned
recipes and the server's normal recipe-book rules. The vanilla recipe book is
preserved.

The interface reproduces [JEI](https://www.curseforge.com/minecraft/mc-mods/jei)'s
layout and controls (see `docs/design.md`). Its graphics are drawing primitives,
vanilla GUI sprites and the JEI-style sprites Horizon already ships; no JEI
implementation or assets are included. The recipe sorting toggles are not implemented yet. Full JEI feature parity is not claimed: the pinned native contract
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

The repository is a Cargo workspace. Development commands live in the `xtask`
crate and run through Cargo:

```sh
cargo xtask check
horizon-mixins check --workspace /path/to/pinned/horizon \
  --mods /path/to/aei/mixins --out /path/to/aei-stage \
  --trust-native-mods --package horizon-client
```

`cargo xtask check` validates that the mod id, version and compatibility pins
agree across every manifest, that every file under `mixins/aei/` is one Horizon
keeps in a bundle, and that no packaged mod or JEI material is in the tree. It
then runs rustfmt over the workspace and the injected sources, Clippy and the
engine tests.

The second command compiles and tests the injected game in a scratch staging
directory, leaving the Horizon source unchanged. Use a clean git checkout of the
exact revision in `compatibility.json`, and put `--out` on a real disk rather
than a RAM-backed `/tmp`: the staged client's link step needs several
gigabytes. Keep game runs and packaged artifacts outside the source tree, with
fresh game directories and scratch homes as Horizon requires.

`cargo xtask install [--instance NAME] [--launcher PATH]` packages the working
tree into the instance's `horizon-native-mods/` (replacing other AEI bundles) and
runs `horizon-launcher native-build --trust-native-code` on it, so the next game
start plays the local source.

Recipe-window and item-list widgets label their stacks for automation as
`Item 3: minecraft:stone x1`; the control socket's widget id (`item-3`) stops at
the colon, so scripts keep stable ids and can still read what is shown.

For a disposable game already running with a control socket,
`cargo xtask ui-test /tmp/test-game/ctl.sock --out /tmp/aei-ui` exercises real
clicks, scrolling, categories, history and search and saves screenshots outside
the repository.

## Release

1. Bump the version in `mixins/aei/engine/Cargo.toml`,
   `mixins/aei/horizon-mixin.toml` and `horizon-native.json`. When moving to a
   new Horizon revision, update the pins in `compatibility.json`,
   `horizon-native.json` and `[compatibility]` in `horizon-mixin.toml` together.
2. Run both verification commands above.
3. Run `cargo xtask package` to write a reproducible
   `dist/aei-<version>.hnmod` and its `.sha256` (`dist/` is ignored by git).
4. Attach the `.hnmod` to a GitHub release of the tagged commit.

## Structure

```text
Cargo.toml                  workspace: the engine and xtask
compatibility.json          Horizon, horizon-mixins and hook pins
horizon-native.json         native manifest Horizon reads from GitHub
mixins/aei/                 the bundle Horizon builds into the game
  horizon-mixin.toml        mod, pins, support modules and injections
  engine/                   aei-engine: Horizon-independent browser model
    src/browser.rs          event dispatch
    src/browser/search.rs   catalogue filtering and search grammar
    src/browser/recipes.rs  recipe indexing and presentation
    src/browser/history.rs  lookup history
    src/viewer.rs           recipe geometry and pagination
    src/types.rs            browser models and actions
  modules/                  support modules injected into horizon-client
    runtime.rs              mod identity, key bindings, engine re-exports
    glue.rs                 native table lifecycle, conversion and actions
    centered_recipe_overlay.rs  recipe rendering and modal interaction
    recipe_paint.rs         original panel, slot, arrow and button drawing
  hooks/                    one small helper per [[injection]]
xtask/                      cargo xtask check | package | ui-test
docs/design.md              structural decisions and their reasons
```

The engine's `Cargo.toml` does not inherit workspace fields: Horizon copies the
crate out of this workspace when it builds the bundle.

AEI is MIT licensed; see [LICENSE](LICENSE).
