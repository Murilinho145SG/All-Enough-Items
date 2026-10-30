// The mod's settings page and its item-giving option.
fn aei_page(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    buttons: &mut Vec<String>,
) {
    use crate::game::aei as m;
    if mod_id != m::ID {
        return;
    }
    buttons.push(m::give_label(m::gives(game.native_setting(m::ID, "give"))));
}
