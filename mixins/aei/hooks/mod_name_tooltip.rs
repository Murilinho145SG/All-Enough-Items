fn aei_mod_name_tooltip(
    mut value: Vec<(horizon_client::text::Text, u32)>,
    stack: &ItemStack,
) -> Vec<(horizon_client::text::Text, u32)> {
    if !value.is_empty()
        && let Some(name) = crate::game::aei_glue::tooltip_mod_name(&stack.item)
    {
        // JEI's style: blue italic.
        value.push((
            horizon_client::text::Text::legacy(&format!("\u{a7}9\u{a7}o{name}")),
            horizon_client::hud::rgba(85, 85, 255, 255),
        ));
    }
    if !value.is_empty()
        && let Some(line) = crate::game::aei_glue::tooltip_counts(&stack.item)
    {
        value.push((
            horizon_client::text::Text::legacy(&line),
            horizon_client::hud::rgba(170, 170, 170, 255),
        ));
    }
    value
}
