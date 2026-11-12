fn aei_modal_mouse_release(game: &mut crate::game::Game<'_>) -> std::ops::ControlFlow<()> {
    if game.aei_recipe_is_modal() {
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
