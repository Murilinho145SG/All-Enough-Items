fn aei_pad_button(
    game: &mut crate::game::Game<'_>,
    button: horizon_client::gamepad::Button,
    down: bool,
) -> std::ops::ControlFlow<Result<()>> {
    if game.aei_pad_button(button, down) {
        return std::ops::ControlFlow::Break(Ok(()));
    }
    std::ops::ControlFlow::Continue(())
}
