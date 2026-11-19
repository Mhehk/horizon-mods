// BetterHitreg — mhv1 for Horizon (mihehk), a port of BetterHitreg by Jass
// (https://github.com/jasspir/BetterHitreg, Apache-2.0; see LICENSE beside this
// port). Changes: rewritten in Rust for Horizon's client; only the core is
// ported: a melee hit's hurt flash, sounds and crit particles play the moment
// you attack, and the server's late copy of the same feedback is skipped.
//
// The hit's type is decided as the client sees it when the attack packet goes
// out (knockback, crit, sweep, strong or weak), as `Player.attack` would.
use std::cell::RefCell;
use std::time::{Duration, Instant};

pub(crate) const ID: &str = "betterhitreg";
/// How long the server's copy of a predicted hit is waited for.
const WINDOW: Duration = Duration::from_millis(1000);
/// A target's invulnerability after a hit (10 ticks), less half a tick: a hit
/// sooner than this deals no damage, so nothing is predicted for it.
const INVULNERABLE: Duration = Duration::from_millis(475);

const KNOCKBACK: &str = "minecraft:entity.player.attack.knockback";
const CRIT: &str = "minecraft:entity.player.attack.crit";
const SWEEP: &str = "minecraft:entity.player.attack.sweep";
const STRONG: &str = "minecraft:entity.player.attack.strong";
const WEAK: &str = "minecraft:entity.player.attack.weak";

struct Pending {
    target: u32,
    sounds: Vec<&'static str>,
    crit: bool,
    /// The Hitreg delay (ms): when the hit plays (`Scheduler.schedule`).
    due: Instant,
}

use super::betterhitreg_config as config;

struct Predicted {
    target: u32,
    at: Instant,
    /// The server's damage event for the target, not yet skipped.
    hurt_left: bool,
    /// Attack sounds the server will echo, not yet skipped.
    sounds_left: usize,
}

#[derive(Default)]
struct State {
    pending: Vec<Pending>,
    predicted: Vec<Predicted>,
    last_damage: Option<Instant>,
    /// The player's attack speed, read each frame (reading it needs the game
    /// mutably, which sending a packet doesn't have).
    attack_speed: Option<f64>,
    /// Safe Regs Only: the last target, whether its last predicted hit never
    /// came back from the server (a ghost), whether someone else hit it, and
    /// where the last attack was made from.
    last_target: Option<u32>,
    ghosted: bool,
    hit_by_another: bool,
    last_attack_at: Option<[f64; 3]>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The attack packet is about to go out: decide the hit as the client sees it.
pub(crate) fn on_send(game: &crate::game::Game<'_>, packet: &super::S) {
    let super::S::Attack(attack) = packet else {
        return;
    };
    if !game.native_on(ID) {
        return;
    }
    let target = attack.id as u32;
    let Some(mob) = game.mobs.iter().find(|m| m.id == target) else {
        return;
    };
    // `AttackMixin`: the attacked entity is the target the Render options show.
    super::betterhitreg_render::set_target(target);
    if !mob.alive() || &*mob.kind.id == "minecraft:armor_stand" {
        return;
    }
    // `Hitreg.isToggled`: custom hitreg on, the target not blocking with a
    // shield, and with Safe Regs Only none of a new target, a ghosted last
    // hit, a hit by someone else or an attack far (50 blocks) from the last.
    let at = game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]);
    let shield = |h: &Option<std::sync::Arc<str>>| h.as_deref() == Some("minecraft:shield");
    let has_shield = shield(&mob.extra.main_hand) || shield(&mob.extra.off_hand);
    let unsafe_reg = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let new_target = s.last_target != Some(target);
        let far = match (s.last_attack_at, at) {
            (Some(a), Some(b)) => (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2) >= 2500.0,
            _ => false,
        };
        let risky = new_target || s.ghosted || s.hit_by_another || far;
        if new_target {
            s.ghosted = false;
            s.hit_by_another = false;
        }
        s.last_target = Some(target);
        s.last_attack_at = at;
        risky
    });
    let now = Instant::now();
    // `getAttackStrengthScale(0.5)`, as `remote_player_attack` reads it, with
    // the attack speed the last frame read (vanilla's default is 4).
    let speed = STATE.with(|s| s.borrow().attack_speed).unwrap_or(4.0);
    let strength = game.hand_strength.scale(0.5, speed);
    let full = strength > 0.9;
    let (sprinting, on_ground, slow) = game.player.as_ref().map_or((false, false, true), |p| {
        let h = (p.velocity.x * p.velocity.x + p.velocity.z * p.velocity.z).sqrt();
        (p.sprinting, p.ground.on_ground, h < 0.1)
    });
    let sword = game
        .held()
        .is_some_and(|s| s.item.to_string().ends_with("_sword"));
    let knockback = full && sprinting;
    let crit = !knockback && game.critical_attack(strength);
    let sweep = full && !knockback && !crit && sword && on_ground && slow;
    // `Hitreg.isToggled` (an attack in reach is within the 30 blocks of a fight).
    let handled = config::on("toggle")
        && !(has_shield && mob.extra.blocking)
        && !(config::on("safeRegsOnly") && unsafe_reg)
        && !(config::on("ignoreShieldHolders") && has_shield);
    // `AttackMixin` / `Hit.load`: the fight hears of every attack.
    use super::betterhitreg_fight::HitType;
    super::betterhitreg_fight::attack(
        game,
        super::betterhitreg_fight::Attack {
            target,
            kind: if knockback {
                HitType::Knockback
            } else if crit {
                HitType::Critical
            } else if sweep {
                HitType::Sweep
            } else if full {
                HitType::FullPick
            } else {
                HitType::HalfPick
            },
            cooldown: strength as f32,
            handled,
            had_shield: has_shield,
            blocked: has_shield && mob.extra.blocking,
            invisible: mob.is_invisible(),
            hurt_recently: mob.hurt_time > 0,
        },
    );
    if !config::on("toggle")
        || (has_shield && mob.extra.blocking)
        || (config::on("safeRegsOnly") && unsafe_reg)
        || (config::on("ignoreShieldHolders") && has_shield)
    {
        return;
    }
    let too_early = STATE.with(|s| {
        s.borrow()
            .last_damage
            .is_some_and(|t| now.duration_since(t) < INVULNERABLE)
    });
    if too_early {
        return;
    }
    let mut sounds = Vec::new();
    if knockback {
        sounds.push(KNOCKBACK);
    }
    if crit {
        sounds.push(CRIT);
    } else if sweep {
        sounds.push(SWEEP);
    } else {
        sounds.push(if full { STRONG } else { WEAK });
    }
    // A player's crit particles come from Horizon itself (`remote_player_attack`).
    let crit_particles = crit && &*mob.kind.id != "minecraft:player";
    let delay = Duration::from_millis(config::int("hitreg").clamp(0, 10_000) as u64);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.last_damage = Some(now);
        // The server's copy is waited for from now, even while the hit waits its delay.
        s.predicted.push(Predicted {
            target,
            at: now,
            hurt_left: true,
            sounds_left: sounds.len(),
        });
        s.pending.push(Pending {
            target,
            sounds,
            crit: crit_particles,
            due: now + delay,
        });
    });
}

/// Each frame: play the hits decided since, and forget old predictions.
pub(crate) fn on_frame(game: &mut crate::game::Game<'_>) {
    if game.remote.is_some() && game.native_on(ID) {
        let speed = game
            .own_attribute("minecraft:attack_speed")
            .unwrap_or_else(|| game.main_hand_attack_speed());
        STATE.with(|s| s.borrow_mut().attack_speed = Some(speed));
    }
    let now = Instant::now();
    let pending = STATE.with(|s| {
        let mut s = s.borrow_mut();
        // A predicted hit the server never confirmed is a ghost (HitTracker).
        let mut ghost = false;
        s.predicted.retain(|p| {
            let keep = now.duration_since(p.at) < WINDOW;
            ghost |= !keep && p.hurt_left;
            keep
        });
        if ghost {
            s.ghosted = true;
        }
        let (due, wait): (Vec<Pending>, Vec<Pending>) =
            std::mem::take(&mut s.pending).into_iter().partition(|p| p.due <= now);
        s.pending = wait;
        due
    });
    if !game.native_on(ID) {
        return;
    }
    // Hide Animations: no hurt flash for anyone (NetworkMixin's DontAnimate).
    let hide_animations = config::on("hideAnimations");
    if hide_animations {
        for m in game.mobs.iter_mut() {
            m.hurt_time = 0;
        }
    }
    // Mute Hits: You, and 1.8 Hit Sounds (1.8 had no attack sounds).
    let quiet = config::on("silenceSelf") || config::on("legacySounds");
    for hit in pending {
        let Some(m) = game.mob_index.get_mut(&mut game.mobs, hit.target) else {
            continue;
        };
        // `LivingEntity.handleDamageEvent`: the walk jolt and the hurt flash.
        if !hide_animations {
            m.limb_speed = 1.5;
            m.hurt_time = 10;
        }
        let at = [m.position.x, m.position.y, m.position.z];
        let id = m.id;
        // `Hit.run`: no particles with Hide All Particles; the enchanted hit's with
        // Always Hit Particles, unless Hide Other Particles.
        let particles = !config::on("hideAllParticles");
        let sharpness = particles && !config::on("hideOtherParticles") && config::on("particlesEveryHit");
        let body = super::mob_tracking_body(&*m);
        use horizon_client::particles::{ParticleOptions, ParticleType};
        if hit.crit && particles {
            game.particles.tracking_emitter(
                ParticleOptions::simple(ParticleType::Crit),
                super::TrackingTarget::Entity(id as i32),
                body,
            );
        }
        if sharpness {
            game.particles.tracking_emitter(
                ParticleOptions::simple(ParticleType::EnchantedHit),
                super::TrackingTarget::Entity(id as i32),
                body,
            );
        }
        if !quiet {
            for sound in &hit.sounds {
                game.audio.play(sound, Some(at), 1.0, 1.0);
            }
        }
    }
}

/// Whether the server's packet is the late copy of a hit already shown.
pub(crate) fn skip_packet(game: &mut crate::game::Game<'_>, packet: &super::C) -> bool {
    if !game.native_on(ID) || super::betterhitreg_fight::replaying() {
        return false;
    }
    let skip = predicted_copy(game, packet);
    // A predicted copy is skipped; the fight still hears of a damage event.
    if skip && !matches!(packet, super::C::DamageEvent(_)) {
        return true;
    }
    let dropped = super::betterhitreg_fight::packet(game, packet);
    skip || dropped
}

fn predicted_copy(game: &mut crate::game::Game<'_>, packet: &super::C) -> bool {
    let me = game.remote.as_ref().map(|r| r.entity_id);
    let now = Instant::now();
    match packet {
        super::C::DamageEvent(p) => {
            let found = STATE.with(|s| {
                let mut s = s.borrow_mut();
                let hit = s.predicted.iter_mut().find(|h| {
                    h.hurt_left && h.target == p.id as u32 && now.duration_since(h.at) < WINDOW
                });
                match hit {
                    Some(h) => {
                        h.hurt_left = false;
                        true
                    }
                    None => false,
                }
            });
            let by_me = p.direct.is_some_and(|d| Some(d) == me);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                if found {
                    s.ghosted = false;
                } else if s.last_target == Some(p.id as u32) && !by_me {
                    s.hit_by_another = true;
                }
            });
            if found {
                // The combo counter still hears of the hit.
                game.pvp_hurt(p.id, false, p.direct.map(|d| Some(d) == me));
            }
            found
        }
        super::C::HurtAnimation(p) => STATE.with(|s| {
            s.borrow()
                .predicted
                .iter()
                .any(|h| h.target == p.id as u32 && now.duration_since(h.at) < WINDOW)
        }),
        super::C::Sound(p) => {
            let Some(name) = game.remote_sound_name(&p.sound) else {
                return false;
            };
            if !name.starts_with("minecraft:entity.player.attack.") {
                return false;
            }
            // 1.8 Hit Sounds: no attack sounds at all (SoundMixin).
            if config::on("legacySounds") {
                return true;
            }
            // `Player.attack` plays its sounds at the attacker: this player.
            let at = [f64::from(p.x) / 8.0, f64::from(p.y) / 8.0, f64::from(p.z) / 8.0];
            let near = game.player.as_ref().is_some_and(|pl| {
                let d = [pl.position.x - at[0], pl.position.y - at[1], pl.position.z - at[2]];
                d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < 9.0
            });
            near && take_sound(now)
        }
        super::C::SoundEntity(p) => {
            let Some(name) = game.remote_sound_name(&p.sound) else {
                return false;
            };
            if !name.starts_with("minecraft:entity.player.attack.") {
                return false;
            }
            // 1.8 Hit Sounds: no attack sounds at all (SoundMixin).
            if config::on("legacySounds") {
                return true;
            }
            me == Some(p.id) && take_sound(now)
        }
        _ => false,
    }
}

fn take_sound(now: Instant) -> bool {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        match s
            .predicted
            .iter_mut()
            .find(|h| h.sounds_left > 0 && now.duration_since(h.at) < WINDOW)
        {
            Some(h) => {
                h.sounds_left -= 1;
                true
            }
            None => false,
        }
    })
}
