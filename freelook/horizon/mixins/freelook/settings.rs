// Freelook Config (FreelookScreen), on the mod's settings page.
fn freelook_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::perspective::freelook::ID {
        crate::game::perspective::freelook::settings(buttons);
    }
}
