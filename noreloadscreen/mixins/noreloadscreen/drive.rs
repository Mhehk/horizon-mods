// Before the game looks at its pack reload: return a done one, hide a fresh one.
fn noreloadscreen_drive(game: &mut crate::game::Game<'_>) {
    crate::game::packs::noreloadscreen::drive(game);
}
