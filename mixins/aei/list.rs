// The mod's row in Options > Mods, with its two keys.
fn aei_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::aei as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: m::KEYS.iter().map(|k| k.map(String::from)).collect(),
    });
}
