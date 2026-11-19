// Every frame: a reload started between ticks is hidden before the next frame
// would draw its overlay, the overlays move on, and the config file is watched.
// Only `drive` hands reloads back.
fn noreloadscreen_frame(game: &mut crate::game::Game<'_>) {
    crate::game::packs::noreloadscreen::frame(game);
}
