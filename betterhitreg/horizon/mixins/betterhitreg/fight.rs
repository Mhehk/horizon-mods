// BetterHitreg — mhv2 for Horizon (mihehk), a port of BetterHitreg by Jass
// (Apache-2.0): the fight around the custom hitreg. `Hitreg.tick` (the fight
// state, jump resets, fight statistics, knockback delays), `HitTracker` (ghosted
// and misplaced hits), `PacketProcessor` / `Sound` (which hit sounds are yours,
// theirs or another fight's, held until a damage event decides), the score and
// Switch Hand keys, Debug Inputs and Debug Sounds, Hide Other Fights, Hide Armor
// and the perfect hit / jump reset colours.
use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::betterhitreg_config as config;
use super::C;

/// `HitType`, as a hit is decided and as a sound names it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HitType {
    TooEarly,
    Knockback,
    Critical,
    Sweep,
    FullPick,
    HalfPick,
}

impl HitType {
    fn name(self) -> &'static str {
        match self {
            Self::TooEarly => "too_early",
            Self::Knockback => "knockback",
            Self::Critical => "critical",
            Self::Sweep => "sweep",
            Self::FullPick => "full_pick",
            Self::HalfPick => "half_pick",
        }
    }
    /// `HitType.of(SoundEvent)`.
    fn of_sound(name: &str) -> Option<Self> {
        let path = name.strip_prefix("minecraft:").unwrap_or(name);
        Some(match path {
            "entity.player.attack.weak" => Self::TooEarly,
            "entity.player.attack.knockback" => Self::Knockback,
            "entity.player.attack.crit" => Self::Critical,
            "entity.player.attack.sweep" => Self::Sweep,
            "entity.player.attack.strong" => Self::FullPick,
            _ if path.contains("hurt") => Self::HalfPick,
            _ => return None,
        })
    }
}

/// `Hit`: what the tracker keeps of an attack.
struct Hit {
    at: Instant,
    kind: HitType,
    cooldown: f32,
    had_shield: bool,
    blocked: bool,
    new_target: bool,
    hit_by_another: bool,
    invisible: bool,
    animated: bool,
    server_right: bool,
    server_types: Vec<HitType>,
}

/// A hit sound the tracker heard (`Sound` registered with a hit type).
struct Heard {
    at: Instant,
    kind: HitType,
}

/// A sound packet held until a damage event says whose hit it was.
struct Held {
    packet: C,
    at: Instant,
    name: String,
    location: [f64; 3],
    held_for_hit: bool,
}

/// `Hitreg.last100Regs`: the last hundred delays, ghosts and misplaces.
#[derive(Default)]
struct Last100 {
    delays: VecDeque<i64>,
    ghosts: VecDeque<bool>,
    misplaces: VecDeque<bool>,
}

fn push<T>(q: &mut VecDeque<T>, v: T) {
    q.push_back(v);
    while q.len() > 100 {
        q.pop_front();
    }
}

/// `Input`, for Debug Inputs.
struct Input {
    name: &'static str,
    mapping: &'static str,
    toggled: bool,
    changed: Option<Instant>,
    previous: Duration,
}

struct Fight {
    tick: i64,
    /// `Hitreg.target` and `lastTarget`.
    target: Option<u32>,
    last_target: Option<u32>,
    last_attack: Option<Instant>,
    /// `lastAnimation`: the target's last damage event; `lastAttacked`: yours.
    last_animation: Option<Instant>,
    last_attacked: Option<Instant>,
    fighting: bool,
    fight_started: Option<Instant>,
    within_fight: bool,
    both_alive: bool,
    your_swings: u32,
    their_swings: u32,
    your_hits: u32,
    their_hits: u32,
    was_swinging: bool,
    last_tick_hit: i64,
    last_tick_jumped: i64,
    last_tick_backed: i64,
    last_tick_in_range: i64,
    last_tick_out_of_range: i64,
    last_perfect_hit: Option<Instant>,
    last_jump_reset: Option<Instant>,
    already_animated: bool,
    already_knocked_back: bool,
    target_location: [f64; 3],
    previous_target_location: [f64; 3],
    last_attack_location: [f64; 3],
    last_attack_blocked: bool,
    hits: VecDeque<Hit>,
    animations: VecDeque<Instant>,
    sounds: VecDeque<Heard>,
    was_ghosted: bool,
    last_non_ghost: Option<Instant>,
    fights_this_session: u32,
    /// `lastHitHandled` / `lastSwingHandled`, `lastSwingWasCrit`.
    hit_handled: bool,
    swing_handled: bool,
    swing_was_crit: bool,
    held: Vec<Held>,
    replaying: bool,
    last100: Last100,
    left_score: i32,
    right_score: i32,
    score_cooldown: u32,
    hand_cooldown: u32,
    clicks: Vec<&'static str>,
    inputs: Vec<Input>,
}

thread_local! {
    static FIGHT: RefCell<Fight> = RefCell::new(Fight {
        tick: 0,
        target: None,
        last_target: None,
        last_attack: None,
        last_animation: None,
        last_attacked: None,
        fighting: false,
        fight_started: None,
        within_fight: false,
        both_alive: false,
        your_swings: 0,
        their_swings: 0,
        your_hits: 0,
        their_hits: 0,
        was_swinging: false,
        last_tick_hit: i64::MIN / 2,
        last_tick_jumped: i64::MIN / 2,
        last_tick_backed: i64::MIN / 2,
        last_tick_in_range: i64::MIN / 2,
        last_tick_out_of_range: i64::MIN / 2,
        last_perfect_hit: None,
        last_jump_reset: None,
        already_animated: false,
        already_knocked_back: false,
        target_location: [0.0; 3],
        previous_target_location: [0.0; 3],
        last_attack_location: [0.0; 3],
        last_attack_blocked: false,
        hits: VecDeque::new(),
        animations: VecDeque::new(),
        sounds: VecDeque::new(),
        was_ghosted: false,
        last_non_ghost: None,
        fights_this_session: 0,
        hit_handled: false,
        swing_handled: false,
        swing_was_crit: false,
        held: Vec::new(),
        replaying: false,
        last100: Last100::default(),
        left_score: 0,
        right_score: 0,
        score_cooldown: 0,
        hand_cooldown: 0,
        clicks: Vec::new(),
        inputs: [
            ("Up", "key.forward"),
            ("Down", "key.back"),
            ("Left", "key.left"),
            ("Right", "key.right"),
            ("Jump", "key.jump"),
            ("LeftClick", "key.attack"),
            ("RightClick", "key.use"),
        ]
        .into_iter()
        .map(|(name, mapping)| Input { name, mapping, toggled: false, changed: None, previous: Duration::ZERO })
        .collect(),
    });
}

fn ms(d: Duration) -> i64 {
    d.as_millis() as i64
}

fn since(t: Option<Instant>, now: Instant) -> i64 {
    t.map_or(i64::MAX / 4, |t| ms(now.saturating_duration_since(t)))
}

/// `|a - b|` in milliseconds.
fn apart(a: Instant, b: Option<Instant>) -> i64 {
    match b {
        Some(b) if a >= b => ms(a - b),
        Some(b) => ms(b - a),
        None => i64::MAX / 4,
    }
}

fn base(m: &horizon_client::mobs::Mob) -> [f64; 3] {
    [m.position.x, m.position.y, m.position.z]
}

fn own_base(game: &crate::game::Game<'_>) -> Option<[f64; 3]> {
    game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z])
}

/// `Hitreg.distanceFrom`: horizontal.
fn flat(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn target_mob<'g>(game: &'g crate::game::Game<'_>, f: &Fight) -> Option<&'g horizon_client::mobs::Mob> {
    f.target.and_then(|id| game.mobs.iter().find(|m| m.id == id))
}

fn message(game: &mut crate::game::Game<'_>, text: &str) {
    config::message(game, text);
}

// ---------------------------------------------------------------------------------------------
// AttackMixin: an attack the player makes
// ---------------------------------------------------------------------------------------------

/// What `state::on_send` decided of an attack on a living target.
pub(crate) struct Attack {
    pub target: u32,
    pub kind: HitType,
    pub cooldown: f32,
    pub handled: bool,
    pub had_shield: bool,
    pub blocked: bool,
    pub invisible: bool,
    pub hurt_recently: bool,
}

pub(crate) fn attack(game: &crate::game::Game<'_>, a: Attack) {
    let now = Instant::now();
    let me = own_base(game).unwrap_or_default();
    FIGHT.with(|cell| {
        let mut f = cell.borrow_mut();
        f.target = Some(a.target);
        let since_last = since(f.last_attack, now);
        let early = since_last < 475;
        let new_target = f.last_target != Some(a.target);
        let hit_by_another = a.hurt_recently && since_last >= 1000;
        if !early {
            if !f.fighting {
                f.fight_started = Some(now);
            }
            f.fighting = true;
            f.last_attack_blocked = a.blocked;
            f.last_attack_location = me;
            f.last_attack = Some(now);
            f.last_target = Some(a.target);
            f.already_animated = false;
            f.already_knocked_back = false;
            f.your_hits += 1;
            if f.last_tick_in_range == f.tick && f.last_tick_out_of_range == f.tick - 1 {
                f.last_perfect_hit = Some(now);
            }
        }
        let kind = if early { HitType::TooEarly } else { a.kind };
        f.swing_was_crit = kind == HitType::Critical;
        f.swing_handled = a.handled;
        if !early {
            f.hits.push_back(Hit {
                at: now,
                kind,
                cooldown: a.cooldown,
                had_shield: a.had_shield,
                blocked: a.blocked,
                new_target,
                hit_by_another,
                invisible: a.invisible,
                animated: false,
                server_right: false,
                server_types: Vec::new(),
            });
            f.hit_handled = a.handled;
        }
    });
    debug(&format!(
        "attack {}{}",
        a.kind.name(),
        if a.handled { ", custom hitreg playing" } else { ", custom hitreg not playing" }
    ));
}

/// `Hitreg.isToggled`'s fight part: both alive and within 30 blocks.
pub(crate) fn within_fight() -> bool {
    FIGHT.with(|f| f.borrow().within_fight)
}

// ---------------------------------------------------------------------------------------------
// PacketProcessor: the server's damage events, animations and sounds
// ---------------------------------------------------------------------------------------------

fn debug(text: &str) {
    if config::on("debugSounds") {
        log::info!(target: "BetterHitreg", "[debug sounds] {text}");
    }
}

/// A held sound is being played again: nothing judges it twice.
pub(crate) fn replaying() -> bool {
    FIGHT.with(|f| f.borrow().replaying)
}

/// Whether the packet is dropped (a muted or held sound, a replaced particle).
pub(crate) fn packet(game: &mut crate::game::Game<'_>, packet: &C) -> bool {
    if FIGHT.with(|f| f.borrow().replaying) {
        return false;
    }
    let me = game.remote.as_ref().map(|r| r.entity_id);
    let now = Instant::now();
    match packet {
        C::DamageEvent(p) => {
            damage(game, p.id, p.cause, me, now);
            false
        }
        C::Animate(p) => FIGHT.with(|cell| {
            let mut f = cell.borrow_mut();
            if f.last_target.map(|t| t as i32) != Some(p.id) {
                return false;
            }
            let within = f.within_fight;
            if p.action == 0 || p.action == 3 {
                f.their_swings += 1;
            }
            // The enchanted hit particles are replaced when every hit plays its own.
            p.action == 5 && within && config::on("particlesEveryHit") && f.hit_handled
        }),
        C::Sound(p) => {
            let at = [f64::from(p.x) / 8.0, f64::from(p.y) / 8.0, f64::from(p.z) / 8.0];
            sound(game, packet, &p.sound, p.source, at, now)
        }
        C::SoundEntity(p) => {
            let at = if me == Some(p.id) {
                own_base(game)
            } else {
                game.mobs.iter().find(|m| m.id as i32 == p.id).map(base)
            };
            match at {
                Some(at) => sound(game, packet, &p.sound, p.source, at, now),
                None => false,
            }
        }
        _ => false,
    }
}

fn damage(game: &mut crate::game::Game<'_>, id: i32, cause: Option<i32>, me: Option<i32>, now: Instant) {
    let target_hit = FIGHT.with(|f| f.borrow().last_target.map(|t| t as i32) == Some(id));
    if target_hit {
        let (delay, animated, within, handled) = FIGHT.with(|cell| {
            let mut f = cell.borrow_mut();
            let animated = f.already_animated;
            f.animations.push_back(now);
            f.last_animation = Some(now);
            f.already_animated = true;
            let delay = since(f.last_attack, now);
            if delay <= 500 {
                push(&mut f.last100.delays, delay);
            }
            (delay, animated, f.within_fight, f.hit_handled)
        });
        debug(&format!(
            "damage event on target +{delay}ms after attack{}",
            if animated { " (already animated)" } else { "" }
        ));
        if config::on("alertDelays") && !animated && delay <= 500 {
            message(game, &format!("hitreg §7was §f{delay}§7ms"));
        }
        if !handled && within && config::on("particlesEveryHit") && !config::on("hideAllParticles") {
            if let Some(m) = game.mobs.iter().find(|m| m.id as i32 == id) {
                use horizon_client::particles::{ParticleOptions, ParticleType};
                let body = super::mob_tracking_body(m);
                game.particles.tracking_emitter(
                    ParticleOptions::simple(ParticleType::EnchantedHit),
                    super::TrackingTarget::Entity(id),
                    body,
                );
            }
        }
        release_held(game, true, now);
    } else if me == Some(id) {
        FIGHT.with(|cell| {
            let mut f = cell.borrow_mut();
            f.last_attacked = Some(now);
            f.last_tick_hit = f.tick;
            if cause.is_some() && cause == f.last_target.map(|t| t as i32) {
                f.their_hits += 1;
            }
            debug(&format!("damage event on you +{}ms after your attack", since(f.last_attack, now)));
        });
        release_held(game, false, now);
    }
}

/// `Sound.nearPlayer` / `nearTarget` (5.5 blocks, horizontally).
fn near(game: &crate::game::Game<'_>, f: &Fight, at: [f64; 3]) -> (f64, f64) {
    let you = own_base(game).map_or(f64::MAX, |p| flat(p, at));
    let them = target_mob(game, f).map_or(f64::MAX, |m| flat(base(m), at));
    (you, them)
}

fn recency(name: &str) -> i64 {
    let _ = name;
    if config::on("silenceOtherFights") { 15 } else { config::int("sound_recency_threshold") }
}

/// `PacketProcessor.processSound`: whether this sound packet is dropped.
fn sound(
    game: &mut crate::game::Game<'_>,
    packet: &C,
    holder: &(i32, Option<(String, Option<f32>)>),
    source: i32,
    at: [f64; 3],
    now: Instant,
) -> bool {
    let Some(name) = game.remote_sound_name(holder) else {
        return false;
    };
    let attack = name.contains("player.attack");
    let hurt = name.contains("hurt");
    // `SoundMixin`: Mute Non-hit Sounds, the players' sounds other than hits.
    if source == 7 && !attack && !hurt && config::on("silenceNonHits") {
        return true;
    }
    let legacy = holder.0 == 0 && holder.1.is_some();
    let modern = !legacy && (hurt || attack);
    if !(modern || legacy) {
        return false;
    }
    let decision = FIGHT.with(|cell| {
        let mut f = cell.borrow_mut();
        let (you_d, them_d) = near(game, &f, at);
        let near_target = them_d <= 5.5;
        let within = you_d <= 5.5 || near_target;
        let could_be_yours = f.last_attack.is_some_and(|a| now >= a && ms(now - a) <= 500) && f.within_fight && near_target;
        let kind = HitType::of_sound(&name);
        // `Sound.register`.
        if could_be_yours && let Some(kind) = kind {
            f.sounds.push_back(Heard { at: now, kind });
        }
        let closer_you = you_d + 1.0 < them_d;
        let closer_them = them_d + 1.0 < you_d;
        let yours = if attack { closer_you } else if hurt { closer_them } else { false };
        let theirs = if attack { closer_them } else if hurt { closer_you } else { false };
        if !within {
            return Some(config::on("silenceOtherFights"));
        }
        if f.last_attack_blocked && kind == Some(HitType::Knockback) && could_be_yours {
            return Some(true);
        }
        if f.swing_handled && name.contains("nodamage") {
            return Some(true);
        }
        let r = recency(&name);
        let you_t = apart(now, f.last_animation);
        let them_t = apart(now, f.last_attacked);
        let from_you = since(f.last_attack, now) <= 1000
            && !(theirs || (you_t > them_t && !yours))
            && you_t <= r;
        let from_them = !yours && them_t <= you_t && them_t <= r;
        if !from_you && !from_them {
            let held_for_hit = could_be_yours && !f.already_animated && kind.is_some();
            if held_for_hit || name.contains("knockback") {
                f.held.push(Held { packet: packet.clone(), at: now, name: name.clone(), location: at, held_for_hit });
                debug(&format!("held {name}: held for a damage event"));
                return Some(true);
            }
            if config::on("silenceOtherFights") {
                return Some(true);
            }
        }
        let unpredicted_crit = kind == Some(HitType::Critical) && !f.swing_was_crit;
        if from_you && (config::on("silenceSelf") || (f.hit_handled && !unpredicted_crit)) {
            return Some(true);
        }
        if from_them && config::on("silenceThem") {
            return Some(true);
        }
        if !legacy && config::on("legacySounds") && !hurt {
            return Some(true);
        }
        // The server's own sound with the custom hitreg off plays as it came.
        if !f.hit_handled && !f.swing_handled && !config::on("silenceSelf") && !config::on("silenceThem")
            && !config::on("silenceOtherFights")
        {
            return None;
        }
        Some(false)
    });
    let dropped = decision.unwrap_or(false);
    if config::on("debugSounds") && !dropped {
        let possible_double = FIGHT.with(|f| {
            let f = f.borrow();
            f.hit_handled && HitType::of_sound(&name).is_some_and(|k| !(k == HitType::Critical && !f.swing_was_crit))
                && attack
        });
        if possible_double && within_fight() {
            message(game, &format!("§cpossible double §7{}", name.trim_start_matches("minecraft:")));
        }
    }
    dropped
}

/// `processDelayedSounds` and the timeout in `PacketProcessor.tick`: held
/// sounds go to you, them or another fight, and play unless that side is muted.
fn release_held(game: &mut crate::game::Game<'_>, from_you: bool, damage: Instant) {
    let window = config::int("sound_hit_window");
    let release: Vec<(C, &'static str)> = FIGHT.with(|cell| {
        let mut f = cell.borrow_mut();
        let you = own_base(game);
        let them = target_mob(game, &f).map(base);
        let mut out = Vec::new();
        let held = std::mem::take(&mut f.held);
        for h in held {
            let w = if from_you && h.held_for_hit { window } else { config::int("sound_recency_threshold") };
            let claimed = apart(damage, Some(h.at)) <= w;
            let (yd, td) = (you.map_or(f64::MAX, |p| flat(p, h.location)), them.map_or(f64::MAX, |p| flat(p, h.location)));
            let attack = h.name.contains("player.attack");
            let hurt = h.name.contains("hurt");
            let yours = if attack { yd + 1.0 < td } else if hurt { td + 1.0 < yd } else { false };
            let theirs = if attack { td + 1.0 < yd } else if hurt { yd + 1.0 < td } else { false };
            if !from_you && h.held_for_hit && (!claimed || yours) {
                f.held.push(h);
                continue;
            }
            if from_you && theirs {
                f.held.push(h);
                continue;
            }
            let owner = if !claimed { "other" } else if from_you { "you" } else { "them" };
            out.push((h.packet, owner));
        }
        out
    });
    play_released(game, release);
}

fn play_released(game: &mut crate::game::Game<'_>, release: Vec<(C, &'static str)>) {
    for (packet, owner) in release {
        let (handled, crit) = FIGHT.with(|f| {
            let f = f.borrow();
            (f.hit_handled, f.swing_was_crit)
        });
        let unpredicted_crit = match &packet {
            C::Sound(p) => game.remote_sound_name(&p.sound),
            C::SoundEntity(p) => game.remote_sound_name(&p.sound),
            _ => None,
        }
        .is_some_and(|n| HitType::of_sound(&n) == Some(HitType::Critical) && !crit);
        let play = match owner {
            "you" => !config::on("silenceSelf") && (!handled || unpredicted_crit),
            "them" => !config::on("silenceThem"),
            _ => !config::on("silenceOtherFights"),
        };
        debug(&format!("held sound released as {owner}: {}", if play { "played" } else { "blocked" }));
        if play {
            FIGHT.with(|f| f.borrow_mut().replaying = true);
            let _ = game.handle_packet(packet);
            FIGHT.with(|f| f.borrow_mut().replaying = false);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Hitreg.tick, HitTracker.process, the keys
// ---------------------------------------------------------------------------------------------

/// `Hitreg.targetTakingKnockback(120, 1e-3, true)`.
fn taking_knockback(f: &Fight) -> bool {
    let (mvx, mvz) = (f.target_location[0] - f.previous_target_location[0], f.target_location[2] - f.previous_target_location[2]);
    let (pvx, pvz) = (f.last_attack_location[0] - f.target_location[0], f.last_attack_location[2] - f.target_location[2]);
    let m = (mvx * mvx + mvz * mvz).sqrt();
    if m < 1e-3 {
        return false;
    }
    let p = (pvx * pvx + pvz * pvz).sqrt();
    if p == 0.0 {
        return false;
    }
    (mvx * pvx + mvz * pvz) / (m * p) < 120f64.to_radians().cos()
}

/// `Hitreg.formatTime`.
fn format_time(duration: u64) -> String {
    let (minutes, seconds) = (duration / 60, duration % 60);
    if minutes == 0 && seconds == 0 {
        return "no time".to_owned();
    }
    let m = match minutes {
        0 => String::new(),
        1 => "1 minute".to_owned(),
        n => format!("{n} minutes"),
    };
    let s = match seconds {
        0 => String::new(),
        1 => "1 second".to_owned(),
        n => format!("{n} seconds"),
    };
    if minutes == 0 {
        s
    } else if seconds == 0 {
        m
    } else {
        format!("{m} {s}")
    }
}

/// `HitTracker.process`: each hit half a second old is matched with the
/// target's damage events and the hit sounds heard after it.
fn process_tracker(f: &mut Fight, now: Instant, lines: &mut Vec<String>) {
    let Some(oldest) = f.hits.front().map(|h| h.at) else {
        return;
    };
    while f.sounds.front().is_some_and(|s| s.at < oldest) {
        f.sounds.pop_front();
    }
    while let Some(hit) = f.hits.front_mut() {
        if now.saturating_duration_since(hit.at) < Duration::from_millis(500) {
            break;
        }
        let max = hit.at + Duration::from_millis(500);
        let mut animation = None;
        while f.animations.front().is_some_and(|a| *a < hit.at) {
            f.animations.pop_front();
        }
        if let Some(a) = f.animations.front().copied().filter(|a| *a <= max) {
            hit.animated = true;
            animation = Some(a);
            f.animations.pop_front();
        }
        if hit.animated {
            for s in &f.sounds {
                if s.at < hit.at {
                    continue;
                }
                if s.at > max {
                    break;
                }
                if apart(s.at, animation) <= 50 {
                    hit.server_types.push(s.kind);
                }
            }
            hit.server_right = hit.server_types.contains(&hit.kind);
        } else {
            hit.server_right = true;
        }
        let (kind, cooldown, animated, right, shield) = (hit.kind, hit.cooldown, hit.animated, hit.server_right, hit.had_shield);
        if !hit.new_target && !hit.hit_by_another && !hit.invisible && !hit.blocked && !f.was_ghosted {
            push(&mut f.last100.ghosts, !animated);
            if config::on("alertGhosts") && !animated {
                lines.push(format!("§7server §rghosted §7your §f{} §7hit (cooldown: {})", kind.name(), decimal(cooldown)));
                if shield {
                    lines.push("§7this may have been due to shield desync".to_owned());
                }
            }
        }
        if kind == HitType::Knockback || kind == HitType::Critical {
            push(&mut f.last100.misplaces, !right);
            if config::on("alertInconsistencies") && !right {
                lines.push(format!("§7server §rmisplaced §7your §f{} §7hit (cooldown: {})", kind.name(), decimal(cooldown)));
            }
        }
        let at = f.hits.front().map(|h| h.at);
        f.was_ghosted = !animated;
        if animated {
            f.last_non_ghost = at;
        }
        f.hits.pop_front();
    }
}

/// `DecimalFormat("#.##")`.
fn decimal(v: f32) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `Hitreg.getGround` of the target, for nothing but `updateGround`'s sake: not needed here.
const KEYS: [(&str, &str, &str); 5] = [
    ("hand", "Switch Hand", "key.keyboard.unknown"),
    ("left", "Increase Left Score", "key.keyboard.left"),
    ("right", "Increase Right Score", "key.keyboard.right"),
    ("up", "Send Score to Chat", "key.keyboard.up"),
    ("down", "Reset Score", "key.keyboard.down"),
];

pub(crate) fn keys() -> Vec<[String; 3]> {
    KEYS.iter().map(|(n, l, d)| [(*n).into(), (*l).into(), (*d).into()]).collect()
}

/// A key went down: the Switch Hand and score keys.
pub(crate) fn key(game: &crate::game::Game<'_>, key: &str, pressed: bool) {
    if !pressed || key == "key.keyboard.unknown" || !game.native_window() || !game.native_playing() {
        return;
    }
    let names: Vec<&'static str> = KEYS
        .iter()
        .filter(|(n, _, d)| game.native_mod_key(super::betterhitreg::ID, n, d) == key)
        .map(|(n, _, _)| *n)
        .collect();
    FIGHT.with(|f| f.borrow_mut().clicks.extend(names));
}

/// `Hitreg.tick` and the END_CLIENT_TICK keys.
pub(crate) fn tick(game: &mut crate::game::Game<'_>) {
    if !game.native_window() || !game.native_on(super::betterhitreg::ID) || game.player.is_none() {
        FIGHT.with(|f| f.borrow_mut().clicks.clear());
        return;
    }
    let now = Instant::now();
    let attack_down = game.native_mapping("key.attack").is_some_and(|m| m.0);
    let jump_down = game.native_mapping("key.jump").is_some_and(|m| m.0);
    let back_down = game.native_mapping("key.back").is_some_and(|m| m.0);
    let (me, on_ground, eye, alive) = {
        let p = game.player.as_ref().expect("checked");
        let me = [p.position.x, p.position.y, p.position.z];
        (me, p.ground.on_ground, [me[0], me[1] + f64::from(p.dimensions.eye_height), me[2]], !p.health.dead_or_dying())
    };
    let spectator = game.spectator();
    let mut lines = Vec::new();
    let mut fight_ended: Option<u64> = None;
    let mut knockback_delay = None;
    FIGHT.with(|cell| {
        let mut f = cell.borrow_mut();
        f.tick += 1;
        // `updateFightState`.
        let target = target_mob(game, &f).map(|m| (base(m), m.alive(), m.hitbox()));
        f.both_alive = alive && !spectator && target.is_some_and(|t| t.1);
        f.within_fight = f.both_alive && target.is_some_and(|t| flat(me, t.0) <= 30.0);
        if attack_down && !f.was_swinging {
            f.your_swings += 1;
        }
        f.was_swinging = attack_down;
        let tick = f.tick;
        let in_range = f.both_alive
            && target.is_some_and(|(_, _, b)| {
                let (lo, hi) = (b.min_position(), b.max_position());
                let c = [eye[0].clamp(lo.x, hi.x), eye[1].clamp(lo.y, hi.y), eye[2].clamp(lo.z, hi.z)];
                (eye[0] - c[0]).powi(2) + (eye[1] - c[1]).powi(2) + (eye[2] - c[2]).powi(2) <= 9.0
            });
        if in_range {
            f.last_tick_in_range = tick;
        } else {
            f.last_tick_out_of_range = tick;
        }
        if jump_down && on_ground {
            f.last_tick_jumped = tick;
        }
        if back_down {
            f.last_tick_backed = tick;
        }
        if tick - f.last_tick_hit == 3 && tick - f.last_tick_backed > 5 {
            let jump_reset = f.last_tick_jumped - f.last_tick_hit;
            let alert = config::on("AlertJumpResets");
            if (-1..=1).contains(&jump_reset) {
                f.last_jump_reset = Some(now);
                if alert {
                    lines.push(match jump_reset {
                        -1 => "Jump Reset §7was §f1 §7tick before you were hit §a(landed)".to_owned(),
                        0 => "Jump Reset §7was §fon the §7tick you were hit §a(landed)".to_owned(),
                        _ => "Jump Reset §7was §f1 §7tick after you were hit §a(landed)".to_owned(),
                    });
                }
            } else if (-3..=3).contains(&jump_reset) && alert {
                lines.push(if jump_reset < 0 {
                    format!("Jump Reset §7was {} ticks before you were hit §c(missed)", jump_reset.abs())
                } else {
                    format!("Jump Reset §7was {jump_reset} ticks after you were hit §c(missed)")
                });
            }
        }
        if !f.fighting {
            f.your_swings = 0;
            f.their_swings = 0;
            f.your_hits = 0;
            f.their_hits = 0;
        }
        if !f.within_fight {
            if f.fighting {
                let duration = f.fight_started.map_or(0, |s| now.saturating_duration_since(s).as_secs());
                let counted = f.last_non_ghost.zip(f.fight_started).is_some_and(|(g, s)| g >= s);
                if (10..=600).contains(&duration) && counted {
                    f.fights_this_session += 1;
                    fight_ended = Some(duration);
                }
            }
            f.fighting = false;
            f.target_location = [0.0; 3];
            f.previous_target_location = [0.0; 3];
            f.hits.clear();
            f.animations.clear();
            f.sounds.clear();
        } else {
            process_tracker(&mut f, now, &mut lines);
        }
        if f.target.is_some() && taking_knockback(&f) && !f.already_knocked_back {
            knockback_delay = Some(since(f.last_attack, now));
            f.already_knocked_back = true;
        }
        if let Some((at, _, _)) = target {
            f.previous_target_location = f.target_location;
            f.target_location = at;
        }
    });
    // `PacketProcessor.tick`: held sounds whose damage event never came.
    let hold = config::int("sound_hit_window").max(config::int("sound_recency_threshold"));
    let timed_out: Vec<(C, &'static str)> = FIGHT.with(|cell| {
        let mut f = cell.borrow_mut();
        let (old, keep): (Vec<Held>, Vec<Held>) =
            std::mem::take(&mut f.held).into_iter().partition(|h| since(Some(h.at), now) > hold);
        f.held = keep;
        old.into_iter().map(|h| (h.packet, "other")).collect()
    });
    play_released(game, timed_out);
    if let Some(duration) = fight_ended {
        let total = config::int("total_fights") + 1;
        config::set_value("total_fights", total.to_string());
        let playtime = config::int("fight_playtime_(seconds)") + duration as i64;
        config::set_value("fight_playtime_(seconds)", playtime.to_string());
        if config::on("alertFights") {
            let (n, yh, ys, th, ts) = FIGHT.with(|f| {
                let f = f.borrow();
                (f.fights_this_session, f.your_hits, f.your_swings, f.their_hits, f.their_swings)
            });
            message(game, &format!("fight §7took §f{} §7(#{n}/#{total})", format_time(duration)));
            let accuracy = |h: u32, s: u32| ((h as f32 / s as f32) * 100.0).round().min(100.0) as i32;
            if yh != 0 && ys != 0 {
                message(game, &format!("Your §7Accuracy: §f{}% §7({yh}/{ys})", accuracy(yh, ys)));
            }
            if th != 0 && ts != 0 {
                message(game, &format!("Their §7Accuracy: §f{}% §7({th}/{ts})", accuracy(th, ts)));
            }
        }
        FIGHT.with(|f| {
            let mut f = f.borrow_mut();
            f.your_swings = 0;
            f.their_swings = 0;
            f.your_hits = 0;
            f.their_hits = 0;
        });
    }
    if let Some(delay) = knockback_delay
        && config::on("alertDelays")
        && delay <= 500
    {
        message(game, &format!("knockback §7took at minimum §f{delay}§7ms"));
    }
    for line in lines {
        message(game, &line);
    }
    // The keys (END_CLIENT_TICK).
    let screen_open = !game.native_playing();
    let clicks = FIGHT.with(|f| std::mem::take(&mut f.borrow_mut().clicks));
    let (hand_ready, score_ready) = FIGHT.with(|f| {
        let f = f.borrow();
        (f.hand_cooldown == 0, f.score_cooldown == 0)
    });
    if hand_ready && !screen_open && clicks.contains(&"hand") {
        game.settings.main_hand_left = !game.settings.main_hand_left;
        game.settings.save(&crate::game::Game::options_path());
        FIGHT.with(|f| f.borrow_mut().hand_cooldown = 5);
    }
    if score_ready && !screen_open {
        let mut said = false;
        let mut send = None;
        FIGHT.with(|cell| {
            let mut f = cell.borrow_mut();
            if clicks.contains(&"left") {
                f.left_score += 1;
                said = true;
            }
            if clicks.contains(&"right") {
                f.right_score += 1;
                said = true;
            }
            if clicks.contains(&"up") && (f.left_score > 0 || f.right_score > 0) {
                send = Some(format!("{}-{}", f.left_score, f.right_score));
                said = true;
            }
            if clicks.contains(&"down") {
                f.left_score = 0;
                f.right_score = 0;
                said = true;
            }
            if said {
                f.score_cooldown = 5;
            }
        });
        if let Some(text) = send
            && game.remote.is_some()
        {
            game.send_chat(&text);
        }
        if said {
            message(game, "§7scores are controlled via your arrow keys by default");
        }
    }
    FIGHT.with(|cell| {
        let mut f = cell.borrow_mut();
        f.hand_cooldown = f.hand_cooldown.saturating_sub(1);
        f.score_cooldown = f.score_cooldown.saturating_sub(1);
    });
}

/// Each frame: Debug Inputs' key states (`InputTracker.update`).
pub(crate) fn frame(game: &mut crate::game::Game<'_>) {
    if !game.native_window() || !config::on("debugInputs") {
        return;
    }
    let now = Instant::now();
    let states: Vec<bool> = FIGHT.with(|f| f.borrow().inputs.iter().map(|i| i.mapping).collect::<Vec<_>>())
        .into_iter()
        .map(|m| game.native_mapping(m).is_some_and(|s| s.0))
        .collect();
    FIGHT.with(|cell| {
        let mut f = cell.borrow_mut();
        for (input, down) in f.inputs.iter_mut().zip(states) {
            if input.toggled != down {
                input.previous = input.changed.map_or(Duration::ZERO, |c| now - c);
                input.changed = Some(now);
            }
            input.toggled = down;
        }
    });
}

/// The score (`registerText`) and Debug Inputs on the HUD.
pub(crate) fn hud(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    if !game.native_window() || !game.native_on(super::betterhitreg::ID) || game.player.is_none() {
        return;
    }
    let atlas = &game.gui.atlas;
    let white = horizon_client::hud_layout::abgr(0xFFFF_FFFF);
    let (left, right) = FIGHT.with(|f| {
        let f = f.borrow();
        (f.left_score, f.right_score)
    });
    if left != 0 || right != 0 {
        let x = config::int("score_x") as f32;
        let y = config::int("score_y") as f32;
        batch.text_styled(atlas, &horizon_client::text::Text::literal(format!("{left} - {right}")), x, y, white, true);
    }
    if !config::on("debugInputs") || !game.native_playing() {
        return;
    }
    let now = Instant::now();
    let rows: Vec<(String, bool)> = FIGHT.with(|f| {
        let f = f.borrow();
        let mut rows = Vec::new();
        for i in &f.inputs {
            let duration = i.changed.map_or(Duration::ZERO, |c| now - c);
            if duration.is_zero() && i.previous.is_zero() {
                rows.push((String::new(), false));
                continue;
            }
            let ms = i.previous.as_secs_f64() * 1000.0;
            let shown = if ms < 10.0 {
                format!("{ms:.2}")
            } else if ms < 100.0 {
                format!("{ms:.1}")
            } else {
                format!("{ms:.0}")
            };
            let value = if i.toggled { "§ayes" } else { "§cno" };
            rows.push((format!("§f{}: {value} §7(§e{shown}ms§7) §7(§areal§7)", i.name), true));
        }
        // `MouseΔX` / `MouseΔY`: raw mouse motion isn't read on 26.3, as the original says.
        for name in ["MouseΔX", "MouseΔY"] {
            rows.push((format!("§f{name}: §c0 §7(§e0.00ms§7) §7(§826.3 not supported§7)"), true));
        }
        rows
    });
    let mut y = 1.0;
    let mut updating = true;
    for (line, ok) in rows {
        if !ok {
            updating = false;
            continue;
        }
        batch.text_styled(atlas, &horizon_client::text::Text::legacy(&line), 1.0, y, white, true);
        y += 10.0;
    }
    if !updating {
        batch.text_styled(
            atlas,
            &horizon_client::text::Text::literal(
                "Inputs aren't updating, this may be caused by a mod that modifies input processing, like Ixeris",
            ),
            1.0,
            y,
            white,
            true,
        );
    }
}

// ---------------------------------------------------------------------------------------------
// RenderMixin, GlowMixin and the colours the Render options share
// ---------------------------------------------------------------------------------------------

/// `RenderMixin.shouldRenderPlayer` with Hide Other Fights: another player
/// away from your fight is not drawn while you fight.
pub(crate) fn hidden(game: &crate::game::Game<'_>, id: u32) -> bool {
    if !game.native_window() || !game.native_on(super::betterhitreg::ID) || !config::on("hideOtherFights") {
        return false;
    }
    let Some(m) = game.mobs.iter().find(|m| m.id == id) else {
        return false;
    };
    if &*m.kind.id != "minecraft:player" {
        return false;
    }
    let now = Instant::now();
    FIGHT.with(|cell| {
        let f = cell.borrow();
        if f.last_target == Some(id) || !f.within_fight || since(f.last_attack, now) > 5000 {
            return false;
        }
        let Some(me) = own_base(game) else {
            return false;
        };
        let at = base(m);
        let Some(t) = target_mob(game, &f).map(base) else {
            return false;
        };
        !(flat(me, at) <= 5.0 || flat(t, at) <= 5.0 || flat(me, t) > 10.0)
    })
}

/// Color Perfect Hits / Color Jump Resets: the colour for the next half second.
pub(crate) fn flash_color() -> Option<&'static str> {
    let now = Instant::now();
    FIGHT.with(|cell| {
        let f = cell.borrow();
        if config::on("PerfectHitColor") && since(f.last_perfect_hit, now) <= 500 {
            Some("perfect_hit")
        } else if config::on("JumpResetColor") && since(f.last_jump_reset, now) <= 500 {
            Some("jump_reset")
        } else {
            None
        }
    })
}

/// `GlowMixin`: the target glows in the flash colour while no hitbox is drawn.
pub(crate) fn outline(game: &crate::game::Game<'_>, id: u32, outline: &mut Option<u32>) {
    if !game.native_window() || !game.native_on(super::betterhitreg::ID) {
        return;
    }
    let ours = FIGHT.with(|f| {
        let f = f.borrow();
        f.within_fight && f.target == Some(id)
    });
    if !ours || config::on("renderHitbox") || config::on("RenderServerHitbox") {
        return;
    }
    if let Some(style) = flash_color() {
        *outline = Some(config::style_argb(style) & 0xFF_FFFF);
    }
}

/// `last100Regs`: the average delay and the ghost and misplace shares, for the menu.
pub(crate) fn last100() -> (Option<i64>, Option<i64>, Option<i64>) {
    FIGHT.with(|cell| {
        let f = cell.borrow();
        let l = &f.last100;
        let avg = (!l.delays.is_empty()).then(|| l.delays.iter().sum::<i64>() / l.delays.len() as i64);
        let share = |q: &VecDeque<bool>| {
            (!q.is_empty()).then(|| (q.iter().filter(|b| **b).count() as f64 / q.len() as f64 * 100.0).round() as i64)
        };
        (avg, share(&l.ghosts), share(&l.misplaces))
    })
}

/// Your and the target's ground (`Hitreg.ground`), for the floor.
pub(crate) fn player_position(game: &crate::game::Game<'_>) -> Option<[f64; 3]> {
    own_base(game)
}

thread_local! {
    /// Hide Armor, as the last tick of the window's game read it.
    static HIDE_ARMOR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `ArmorMixin`: Hide Armor is on.
pub(crate) fn hide_armor() -> bool {
    HIDE_ARMOR.get()
}

/// The particle filter's flags and where you stand (`ParticleMixin`), and Hide Armor.
pub(crate) fn particle_flags(game: &crate::game::Game<'_>) {
    use std::sync::atomic::Ordering;
    let on = game.native_on(super::betterhitreg::ID);
    HIDE_ARMOR.set(on && config::on("hideArmor"));
    let flags = if on {
        u8::from(config::on("hideAllParticles"))
            | u8::from(config::on("hideOtherParticles")) << 1
            | u8::from(config::on("hideOtherFights")) << 2
    } else {
        0
    };
    horizon_client::particles::betterhitreg_particles::FLAGS.store(flags, Ordering::Relaxed);
    if let Some(p) = own_base(game)
        && let Ok(mut at) = horizon_client::particles::betterhitreg_particles::PLAYER.lock()
    {
        *at = Some(p);
    }
}
