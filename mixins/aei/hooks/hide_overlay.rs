fn aei_hide_overlay(value: bool) -> bool {
    value || crate::game::aei_glue::overlay_hidden()
}
