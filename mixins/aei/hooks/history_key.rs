fn aei_history_key(game: &mut crate::game::Game<'_>, key: &str) -> std::ops::ControlFlow<()> {
    if game.title.is_none() && crate::game::aei_glue::search_key(game, key) {
        return std::ops::ControlFlow::Break(());
    }
    if game.title.is_none()
        && key == "key.keyboard.c"
        && game.has_control_down()
        && !game.mod_browser_typing()
        && let Some(card) = game.aei_hovered_card()
        && crate::game::aei_glue::copy_recipe(game, card)
    {
        return std::ops::ControlFlow::Break(());
    }
    if game.title.is_none()
        && key == "key.keyboard.o"
        && game.has_control_down()
        && !game.mod_browser_typing()
    {
        crate::game::aei_glue::toggle_overlay(game);
        return std::ops::ControlFlow::Break(());
    }
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
        && (crate::game::aei_glue::navigation(game, key)
            || (key == "key.keyboard.backspace" && crate::game::aei_glue::back(game)))
    {
        return std::ops::ControlFlow::Break(());
    }
    std::ops::ControlFlow::Continue(())
}
