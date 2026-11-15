fn aei_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::aei::ID {
        buttons.extend(crate::game::aei_glue::settings_labels(game));
    }
}
