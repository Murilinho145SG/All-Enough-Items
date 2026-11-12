fn aei_hide_container(
    game: &mut crate::game::Game<'_>,
    batch: &horizon_client::hud::HudBatch,
) -> std::ops::ControlFlow<()> {
    if game.aei_recipe_modal(batch) {
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
