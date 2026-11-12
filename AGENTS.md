# All Enough Items (AEI)

AEI is a client-side native Horizon mod that provides a searchable item catalogue and recipe/usage lookup. It is distributed through Horizon's mod system, not built into the Horizon client and not shipped as a standalone Wasm mod.

## Project goals

- Browse the items Horizon already loads.
- Look up recipes that produce an item and recipes that use it.
- Preserve the vanilla recipe book and its behavior.
- Use Horizon's own item, recipe, language, and rendering data through the published native-hook contract.
- Keep AEI logic and integration in this repository; keep Horizon-specific hook support in the Horizon repository only when the published contract is insufficient.

## JEI reference policy

- Do not place any JEI jar, source archive, decompiled source, or asset in this repository.
- Do not copy or decompile JEI implementation code or copy its assets.
- Base implementation on public documentation and observed behavior; keep all local reference files outside the repository.
- Do not contact mod authors or publish releases on the user's behalf.

## Development

- This repository is a `horizon-mixins` native-mod bundle. Keep compatibility pins synchronized in `compatibility.json` and `mixins/aei/horizon-mixin.toml`.
- The root is a Cargo workspace (the engine and `xtask`). Run `cargo xtask check` before committing; keep injected support modules in `mixins/aei/modules/` and injection helpers in `mixins/aei/hooks/`.
- Follow Horizon's `AGENTS.md`, the native-mod trust model, vanilla behavior requirements, and the rule that Horizon-only UI features stay off by default when they change the vanilla look.
- Keep this repository free of generated game data, game assets, and mod binaries.
