// ArmorMixin: Hide Armor leaves every armour layer without its texture, so
// `HumanoidArmorLayer` draws nothing.
fn betterhitreg_armor(kind: &str) -> core::ops::ControlFlow<Option<[f32; 4]>> {
    let _ = kind;
    if crate::game::remote::betterhitreg_fight::hide_armor() {
        return core::ops::ControlFlow::Break(None);
    }
    core::ops::ControlFlow::Continue(())
}
