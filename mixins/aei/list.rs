fn aei_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: crate::game::aei::ID.into(),
        title: format!("{} {}", crate::game::aei::NAME, crate::game::aei::VERSION),
        description: crate::game::aei::DESCRIPTION.into(),
        keys: vec![
            [
                crate::game::aei::KEY_RECIPE[0].into(),
                crate::game::aei::KEY_RECIPE[1].into(),
                crate::game::aei::KEY_RECIPE[2].into(),
            ],
            [
                crate::game::aei::KEY_USES[0].into(),
                crate::game::aei::KEY_USES[1].into(),
                crate::game::aei::KEY_USES[2].into(),
            ],
        ],
    });
}
