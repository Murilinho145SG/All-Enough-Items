fn aei_modal_hotkey(game: &mut crate::game::Game<'_>) -> std::ops::ControlFlow<bool> {
    if game.mod_browser_typing() || game.aei_recipe_is_modal() {
        return std::ops::ControlFlow::Break(true);
    }
    std::ops::ControlFlow::Continue(())
}
