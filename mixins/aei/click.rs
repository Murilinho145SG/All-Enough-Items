// A click on the mod's settings page flips "Click gives items".
fn aei_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::aei as m;
    if mod_id != m::ID || button != 0 {
        return;
    }
    let give = !m::gives(game.native_setting(m::ID, "give"));
    game.native_set_setting(m::ID, "give", if give { "true" } else { "false" });
}
