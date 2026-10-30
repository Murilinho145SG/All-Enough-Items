fn aei_event(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    event: &crate::game::native_hooks::NativePanelEvent,
) {
    if mod_id == crate::game::aei::ID {
        crate::game::aei_glue::event(game, event);
    }
}
