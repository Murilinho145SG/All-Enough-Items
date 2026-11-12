fn aei_recipe_press(game: &mut Game<'_>, right: bool) -> std::ops::ControlFlow<bool> {
    aei_recipe_overlay::press(game, right)
}
