fn aei_world_lookup(game: &mut crate::game::Game<'_>, key: KeyCode, pressed: bool, repeat: bool) {
    if pressed
        && !repeat
        && let Some(name) = keybinds::key_name(key)
    {
        crate::game::aei_glue::world_key(game, name);
    }
}
