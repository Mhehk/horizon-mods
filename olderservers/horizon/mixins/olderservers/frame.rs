// Each frame: the switch follows the mod's row.
fn olderservers_frame(game: &mut crate::game::Game<'_>) {
    let on = game.native_on("olderservers");
    horizon_client::net::olderservers_switch::ON.store(on, std::sync::atomic::Ordering::Relaxed);
}
