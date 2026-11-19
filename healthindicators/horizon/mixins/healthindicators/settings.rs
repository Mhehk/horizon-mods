// The config screen on the mod's settings page.
fn healthindicators_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::remote::healthindicators::ID {
        crate::game::remote::healthindicators::settings(buttons);
    }
}
