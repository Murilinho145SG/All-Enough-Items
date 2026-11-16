// Horizon keeps the creative screen's panel for its own recipe viewer; AEI's runs there
// too. On the creative screen with AEI's panel up, the sync runs as on the inventory
// screen (the only place the screen matters), then the screen comes back.
fn aei_allow_creative_panel(game: &mut Game<'_>) -> std::ops::ControlFlow<Option<Owner>> {
    if game.screen != Screen::Creative || game.title.is_some() {
        return std::ops::ControlFlow::Continue(());
    }
    let disabled = &game.settings.mods.disabled;
    let aei = game
        .panel
        .shown(|id| !disabled.contains(id))
        .is_some_and(|shown| shown.owner.native_id() == "aei");
    if !aei {
        return std::ops::ControlFlow::Continue(());
    }
    game.screen = Screen::Inventory;
    let owner = game.browser_sync();
    game.screen = Screen::Creative;
    std::ops::ControlFlow::Break(owner)
}
