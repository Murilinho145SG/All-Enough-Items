fn aei_allow_creative_panel(value: bool, owner: &(Owner, u64)) -> bool {
    value && owner.0.native_id() != "aei"
}
