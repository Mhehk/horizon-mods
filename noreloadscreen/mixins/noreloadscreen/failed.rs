// A failed reload of the player's packs: RRLS's resetResources and doubleLoad.
fn noreloadscreen_failed(game: &mut crate::game::Game<'_>, error: &str) -> core::ops::ControlFlow<()> {
    crate::game::packs::noreloadscreen::reload_failed(game, error)
}
