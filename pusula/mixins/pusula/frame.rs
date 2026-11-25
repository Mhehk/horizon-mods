// Each frame: chunks scanned, the map saved now and then, a death noticed,
// the world map's commands sent (state.rs).
fn pusula_frame(game: &mut crate::game::Game<'_>) {
    crate::game::remote::pusula::frame(game);
}