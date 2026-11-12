fn aei_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::aei::ID {
        let active = game.native_setting(mod_id, "give").as_deref() == Some("true");
        let mut label = String::from("Panel clicks give items in Creative: ");
        label.push_str(if active { "ON" } else { "OFF" });
        buttons.push(label);
    }
}
