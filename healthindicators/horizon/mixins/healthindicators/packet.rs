// EntityDamageMixin: an entity was hurt, by whom.
fn healthindicators_packet(game: &mut crate::game::Game<'_>, packet: &C) -> core::ops::ControlFlow<Result<()>> {
    if let C::DamageEvent(p) = packet {
        crate::game::remote::healthindicators::damage(game, p.id, p.cause);
    }
    core::ops::ControlFlow::Continue(())
}
