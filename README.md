# All Enough Items

All Enough Items (AEI) is a searchable item and recipe browser for Horizon. It is
distributed as a native Horizon mod in this repository, independently of the
game source.

## What works

- Browse items beside inventory and container screens.
- Search by item name or id, namespace (`@minecraft`) and tag (`#logs`).
- Open recipes with **R** and uses with **U**, or use left and right clicks on
  panel items and recipe ingredients.
- Browse recipes by station, inspect ingredient alternatives and return through
  lookup history.
- Place a recipe the player's recipe book knows into the open crafting grid.
- Optionally give items from the panel in Creative mode.

Recipes come from Horizon's native recipe tables and retain item components for
display. On other servers, the viewer shows the recipes the client has received;
recipe placement remains subject to the server's normal recipe-book rules.

## Install

The Horizon launcher can install a native mod from this GitHub repository. Open
**Mods → Add from GitHub**, enter this repository URL, install **AEI**, then build
the native-mod game when the launcher offers it. Native mods run as compiled Rust
with the player's account permissions. Horizon shows a trust warning before
building them.

## Build from source

This is a `horizon-mixins` bundle. Its compatibility file requires native hook
API 2, which provides the item panel, recipe pages, lookups and recipe-book
placement.

```sh
cargo test
horizon-mixins check --workspace /path/to/horizon \
  --mods /path/to/All-Enough-Items --out /tmp/aei-stage \
  --trust-native-mods
```

The bundle pins a Horizon revision and a `horizon-mixins` revision in
`compatibility.json`. Use the `horizon-mixins` revision listed there when
building manually. The Horizon launcher resolves and builds these dependencies
for GitHub installations.

## Current coverage

The native recipe API currently exposes crafting, smelting, blasting, smoking,
campfire cooking, stonecutting, smithing and brewing. The original in-game AEI
also had dynamic anvil, grindstone, fuel and compost categories, and could fill
compatible menus without relying on the recipe book. Those need additional
native hook support before this mod can match that behavior; they are not
silently represented as supported here.

## Project structure

- `mixins/aei/runtime.rs` contains the viewer state, search and recipe lookup
  logic; it also contains the logic tests.
- `mixins/aei/glue.rs` adapts Horizon's native hook data to the viewer.
- `mixins/aei/horizon-mixin.toml` declares the module and hook points.
- `compatibility.json` pins the Horizon and `horizon-mixins` revisions.

The runtime is based on Horizon's official native Recipes viewer, adapted for
the standalone AEI project under the MIT license; see [LICENSE](LICENSE).
