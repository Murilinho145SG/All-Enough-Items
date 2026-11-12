fn aei_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::aei::{KEY_BACK, KEY_FORWARD, KEY_RECIPE, KEY_USES};
    let mut title = String::from(crate::game::aei::NAME);
    title.push(' ');
    title.push_str(crate::game::aei::VERSION);
    let keys = [KEY_RECIPE, KEY_USES, KEY_BACK, KEY_FORWARD]
        .iter()
        .map(|key| [key[0].into(), key[1].into(), key[2].into()])
        .collect();
    mods.push(crate::game::native_hooks::NativeMod {
        id: crate::game::aei::ID.into(),
        title,
        description: crate::game::aei::DESCRIPTION.into(),
        keys,
    });
}
