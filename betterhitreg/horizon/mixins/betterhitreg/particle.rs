// ParticleMixin: Hide All / Other Particles and Hide Other Fights.
fn betterhitreg_particle(options: &ParticleOptions, p: [f64; 3]) -> core::ops::ControlFlow<()> {
    if crate::particles::betterhitreg_particles::skip(options.ty, p) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
