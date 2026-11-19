// FreeLookMod.onTickEnd: start, stop or switch the free look; the menu key.
fn freelook_tick(game: &mut crate::game::Game<'_>) {
    crate::game::perspective::freelook::tick(game);
}
