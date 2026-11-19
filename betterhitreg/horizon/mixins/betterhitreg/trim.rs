// ArmorMixin: Hide Armor leaves no armour trim floating either.
fn betterhitreg_trim(kind: &str) -> core::ops::ControlFlow<Option<TrimTexture>> {
    let _ = kind;
    if crate::game::remote::betterhitreg_fight::hide_armor() {
        return core::ops::ControlFlow::Break(None);
    }
    core::ops::ControlFlow::Continue(())
}
