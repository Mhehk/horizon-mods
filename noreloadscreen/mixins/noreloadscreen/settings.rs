// The mod's Options > Mods page: RRLS's configuration (its sections, then a section's values).
fn noreloadscreen_settings(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::packs::noreloadscreen::ID {
        crate::game::packs::noreloadscreen::settings(buttons);
    }
}
