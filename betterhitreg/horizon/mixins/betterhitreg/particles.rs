// BetterHitreg — mhv2 for Horizon (mihehk), a port of BetterHitreg by Jass
// (Apache-2.0): `ParticleMixin`. Set each tick by the client's BetterHitreg.
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

/// Bit 0 Hide All Particles, bit 1 Hide Other Particles (only crit and sweep),
/// bit 2 Hide Other Fights (none farther than sqrt(30) blocks from you).
pub static FLAGS: AtomicU8 = AtomicU8::new(0);
/// Where you stand.
pub static PLAYER: Mutex<Option<[f64; 3]>> = Mutex::new(None);

/// Whether `ParticleEngine.makeParticle` makes nothing of this particle.
pub fn skip(ty: crate::particles::ParticleType, p: [f64; 3]) -> bool {
    use crate::particles::ParticleType;
    let flags = FLAGS.load(Ordering::Relaxed);
    if flags == 0 || ty == ParticleType::Firework {
        return false;
    }
    if flags & 1 != 0 {
        return true;
    }
    if flags & 2 != 0 && ty != ParticleType::Crit && ty != ParticleType::SweepAttack {
        return true;
    }
    if flags & 4 != 0
        && let Ok(at) = PLAYER.lock()
        && let Some(at) = *at
    {
        let d = (at[0] - p[0]).powi(2) + (at[1] - p[1]).powi(2) + (at[2] - p[2]).powi(2);
        return d > 30.0;
    }
    false
}
