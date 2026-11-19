// The mod's Options > Mods page: the configuration screen's button (Mod Menu's config entry).
fn shulkerboxtooltip_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::shulkerboxtooltip::ID {
        crate::game::shulkerboxtooltip_screen::settings_buttons(buttons);
    }
}
