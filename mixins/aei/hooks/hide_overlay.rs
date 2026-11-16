// Ctrl+O hid the overlay: no panel syncs, as with a title screen open.
fn aei_hide_overlay() -> std::ops::ControlFlow<Option<Owner>> {
    if crate::game::aei_glue::overlay_hidden() {
        return std::ops::ControlFlow::Break(None);
    }
    std::ops::ControlFlow::Continue(())
}
