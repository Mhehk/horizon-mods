// Zoomify's settings (YACL's screen) on the mod's settings page.
fn zoomify_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::fov::zoomify::ID {
        crate::game::fov::zoomify::settings(buttons);
    }
}
