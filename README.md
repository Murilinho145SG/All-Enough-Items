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
- Beyond JEI, plain words ignore accents (`maca` finds *Maçã*), also match the
  English name in another language, and forgive one typo in words of four
  letters or more (exact matches come first). Filters narrow the list:
  `is:food`, `is:block`, `is:enchantable`, `is:tool`, `is:craftable` (what one
  recipe makes from the inventory in the open grid, updated as it changes),
  `has:recipe`, `no:recipe`,
  `rarity:epic`, and numbers with `stack`, `dur` and `nutrition` (`stack:1`,
  `dur>500`, `nutrition<4`).
- Press **Ctrl+O** to hide or show the overlay (remembered between sessions).
- The settings page orders the list by creative tabs (the default), mod or name,
  and has JEI's edit mode: while it is on, clicking an entry hides it (Ctrl+click
  hides every stack of that item); hidden entries show shaded red until edit mode
  is off, then leave the list. The hidden list is saved. It also limits the
  list's columns and can move the search bar under the screen, as JEI's options do.
- An optional setting adds the item's mod name to every item tooltip, in blue
  italics as JEI does (off by default, since it changes vanilla tooltips).
- Hover an item and press **R** for recipes or **U** for uses (also over
  inventory slots and the vanilla recipe book's buttons). In the catalogue,
  left-click shows recipes and right-click shows uses.
- Browse recipes in a JEI-style window: category tabs above it, the crafting
  station on the left, category and page arrows, recipes on bordered plates with
  animated furnace and brewing progress, cook time and experience. The item list
  stays on the right, with page arrows above it and the search field below.
- Recipe categories follow the order in the `categories` file of AEI's mod data
  folder (`horizon-mods/.horizon-mod-data/aei/`), which AEI writes with every
  category on first use, like JEI's category order configuration: move lines to
  reorder the tabs, or put `!` before one to hide that category.
- The category's catalysts (crafting table and crafter, both campfires, ...)
  stand in a column left of the window; click one for its recipes, right-click
  for its uses.
- Click ingredients to follow recipes, click the category title to browse every
  recipe (one tab per category), and use **Left/Right** (or **Backspace** to go
  back) to navigate lookup history.
- Beyond JEI, press **T** over an item for the material calculator: the raw
  materials a number of it takes (buttons or the wheel change the count), the
  crafting steps in order and what is left over. It picks crafting, then cooking,
  never unpacks storage blocks, avoids loops, and counts what the inventory
  already holds first (short materials are red). Escape closes it; clicking an
  entry opens its recipes or uses. While it is open, **T** over another item
  adds it to a shopping list whose materials are summed; click a target to
  change its count, right-click it to remove it.
- An optional row at the bottom left holds the nine items looked up last (off by
  default; saved per instance).
- Beyond JEI, the last ten lookups line up above the category tabs as a
  breadcrumb trail (the shown one framed in gold): click one to go back to it.
- Ingredients with alternatives cycle every second; hold **Shift** to pause them.
  A slot whose alternatives are exactly a tag says so in its tooltip ("Accepts
  any: #minecraft:planks"), and with advanced tooltips (F3+H) an output shows its
  recipe id.
- Press **A** over any item (the list, a recipe, an inventory slot) to bookmark
  it, as in JEI; bookmarks line up on the left of the screen with their own
  pages, open recipes and uses like the list, and are saved. Drag one onto
  another to reorder them. The ribbon button
  at the top right of each recipe bookmarks that recipe: its output joins the
  list with a gold corner and opens on that recipe.
- Cheat mode (Ctrl+click the settings button, or the settings page) tints the
  list red; then clicking an item, bookmark or recipe ingredient gives it, a full
  stack with the left button and one with the right, as JEI does. In Creative the
  items go to the inventory (or the cursor, on the creative screen, if set);
  otherwise AEI sends `/give`, which the server only allows with permission.
  A stack dropped on the list on the creative screen is deleted.
- Over any item, **Ctrl+C** copies its id and **Ctrl+Shift+C** a `/give`
  command for that exact stack, components included.
- Press **Ctrl+F** to focus the search field. While typing, **Up** and **Down**
  recall earlier searches (kept when you press Enter or open an item from them).
- Scroll or click arrows to turn pages. Right-click the search field to clear it.
- Click **+** to move a recipe's ingredients into the crafting grid through the
  vanilla recipe book (**Shift-click** for as many sets as the inventory allows);
  the recipe view then closes. The button only shows on recipes the open screen's
  book takes: crafting recipes that fit the open grid (2x2 in the inventory, 3x3
  at a crafting table), and smelting, blasting or smoking recipes at their
  furnace. It is orange
  when ingredients are missing (the grid shows the recipe as a ghost) and dark
  when the recipe isn't unlocked in the player's recipe book.

Recipes, stack components, item rendering, fonts and translations come from
Horizon's native API. Recipe placement remains subject to the player's learned
recipes and the server's normal recipe-book rules. The vanilla recipe book is
preserved.

The interface reproduces [JEI](https://www.curseforge.com/minecraft/mc-mods/jei)'s
layout and controls (see `docs/design.md`). Its graphics are drawing primitives,
vanilla GUI sprites and the JEI-style sprites Horizon already ships; no JEI
implementation or assets are included.

JEI's vanilla features are covered: the item list with its search prefixes,
modes, sorting, edit mode and options; bookmarks; cheat mode; the recipe window
with catalysts, transfers, cycling ingredients and category order; and the
crafting, cooking, campfire, stonecutting, smithing, brewing, anvil, fuel and
compostable categories. The pinned native contract provides the first six; AEI
adds fuel and compostable from the items' own `fuel` and `compostable`
components, and the anvil from the anvil's own rules. What remains different is
listed in `docs/roadmap-jei.md`: fluids (water and lava) aren't listed, since
the GUI atlas has no fluid sprites and no vanilla recipe uses them, and furnace
transfers follow a Horizon issue at the pinned revision (see `docs/design.md`).

AEI extends Horizon's creative inventory panel guard for its own recipe viewer
and leaves the vanilla recipe book panel available. The viewer works from any
inventory screen Horizon opens. The native panel holds 256 recipes at a time, so
longer categories are sent a window at a time; the page numbers count the whole
category.

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
