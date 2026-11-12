fn aei_recipe_draw(game: &mut Game<'_>, batch: &mut HudBatch) -> std::ops::ControlFlow<()> {
    aei_recipe_overlay::draw(game, batch)
}
