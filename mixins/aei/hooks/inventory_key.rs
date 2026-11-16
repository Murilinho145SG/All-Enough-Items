// The inventory key (E) with AEI's recipes open closes the recipes and keeps the
// inventory, as JEI's recipe window closes on it.
fn aei_inventory_key(
    game: &mut crate::game::Game<'_>,
    key: KeyCode,
) -> std::ops::ControlFlow<Result<()>> {
    if key == KeyCode::KeyE
        && game.title.is_none()
        && !game.mod_browser_typing()
        && game.aei_recipe_is_modal()
    {
        game.mod_browser_closed();
        crate::game::aei_glue::close_recipes(game);
        return std::ops::ControlFlow::Break(Ok(()));
    }
    std::ops::ControlFlow::Continue(())
}
