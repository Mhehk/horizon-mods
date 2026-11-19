// ShulkerBoxTooltip's row in Options > Mods (Mod Menu's entry).
fn shulkerboxtooltip_listed(mods: &mut Vec<NativeMod>) {
    use crate::game::shulkerboxtooltip as m;
    mods.push(NativeMod {
        id: m::ID.into(),
        title: m::TITLE.into(),
        description: "View the contents of shulker boxes from your inventory. Port of ShulkerBoxTooltip by MisterPeModder (MIT).".into(),
        keys: Vec::new(),
    });
}
