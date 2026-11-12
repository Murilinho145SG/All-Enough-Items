fn aei_recipe_scroll(game: &crate::game::Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    *canceled |= game.aei_recipe_scroll(scroll[1]);
}
