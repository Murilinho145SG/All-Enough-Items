fn aei_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id != crate::game::aei::ID || button != 0 {
        return;
    }
    let enabled = game.native_setting(mod_id, "give").as_deref() == Some("true");
    game.native_set_setting(mod_id, "give", if enabled { "false" } else { "true" });
}
