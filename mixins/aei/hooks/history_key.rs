fn aei_history_key(game: &mut crate::game::Game<'_>, key: &str) -> std::ops::ControlFlow<()> {
    if game.title.is_none()
        && key == "key.keyboard.f"
        && game.has_control_down()
        && game.aei_focus_search()
    {
        return std::ops::ControlFlow::Break(());
    }
    if game.title.is_none()
        && matches!(
            game.screen,
            crate::game::Screen::Inventory | crate::game::Screen::Creative
        )
        && !game.mod_browser_typing()
        && crate::game::aei_glue::navigation(game, key)
    {
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
