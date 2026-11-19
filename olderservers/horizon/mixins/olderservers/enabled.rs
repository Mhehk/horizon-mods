// Joining an older release's server is on while the mod is.
fn olderservers_enabled() -> core::ops::ControlFlow<bool> {
    if crate::net::olderservers_switch::ON.load(std::sync::atomic::Ordering::Relaxed) {
        return core::ops::ControlFlow::Break(true);
    }
    core::ops::ControlFlow::Continue(())
}
