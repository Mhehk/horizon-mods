// BetterHitreg's menu (UIScreen) on the mod's settings page.
fn betterhitreg_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::remote::betterhitreg::ID {
        crate::game::remote::betterhitreg_config::settings(buttons);
    }
}
