// Each frame: the item panel follows the world and the mod's on/off.
fn aei_frame(game: &mut crate::game::Game<'_>) {
    crate::game::aei_glue::frame(game);
}
