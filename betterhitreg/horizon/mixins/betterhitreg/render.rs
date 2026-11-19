// BetterHitreg — mhv1.2 for Horizon (mihehk), a port of BetterHitreg by Jass
// (Apache-2.0): the Render options (`Render.render`). The last attacked target's
// hitbox as it is drawn (interpolated) and as the server last placed it, the
// approach hitbox, a cross on its closest point, and your and their hit and
// jump ranges as rings on the ground; near (in reach) and far colours are the
// styles of `hitreg.properties` (`/hitreg color`).
use std::cell::Cell;

use super::betterhitreg_config as config;
use horizon_client::world_primitives::WorldPrimitives;

/// Lines are three pixels wide, as `Gizmos.line(.., 3)`.
const WIDTH: f32 = 3.0;

thread_local! {
    /// `Hitreg.target`: the entity last attacked.
    static TARGET: Cell<Option<u32>> = const { Cell::new(None) };
}

/// `AttackMixin`: an attack on a living entity makes it the target.
pub(crate) fn set_target(id: u32) {
    TARGET.set(Some(id));
}

/// A style's colour and opacity as ARGB (`Style.argb`).
fn style(name: &str) -> u32 {
    let hex = config::get(&format!("{name}_color"));
    let rgb = u32::from_str_radix(hex.trim().trim_start_matches('#'), 16).unwrap_or(0xFF_FFFF) & 0xFF_FFFF;
    let alpha = config::int(&format!("{name}_opacity")).clamp(0, 255) as u32;
    alpha << 24 | rgb
}

fn lerp3(from: [f64; 3], to: [f64; 3], t: f64) -> [f64; 3] {
    [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t, from[2] + (to[2] - from[2]) * t]
}

/// `Hitreg.getGround`: the feet when on the ground, else the top of the first
/// block in the three below, else none (in the sky).
fn ground(game: &crate::game::Game<'_>, at: [f64; 3], on_ground: bool) -> Option<f64> {
    if on_ground {
        return Some(at[1]);
    }
    let (x, y, z) = (at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32);
    (0..=3).map(|i| y - i).find_map(|under| {
        let state = game.world.block(x, under, z);
        (!game.res.facts().is_air(state)).then_some(f64::from(under) + 1.0)
    })
}

/// `Render.ring`: a flat circle of `segments` lines.
fn ring(out: &mut WorldPrimitives, center: [f64; 3], radius: f64, color: u32) {
    let segments = 64;
    let delta = 2.0 * std::f64::consts::PI / f64::from(segments);
    let (cos, sin) = (delta.cos(), delta.sin());
    let (mut x, mut z) = (radius, 0.0);
    let mut prev = [center[0] + x, center[1], center[2] + z];
    for _ in 0..segments {
        let (nx, nz) = (x * cos - z * sin, x * sin + z * cos);
        let next = [center[0] + nx, center[1], center[2] + nz];
        out.line(prev, next, color, WIDTH);
        (x, z) = (nx, nz);
        prev = next;
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn length(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
fn normalize(a: [f64; 3]) -> [f64; 3] {
    let l = length(a);
    if l < 1e-12 { a } else { scale(a, 1.0 / l) }
}
fn cross_product(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `Render.cross`: a plus facing the camera, its arms `half` pixels long.
fn cross(out: &mut WorldPrimitives, camera: [f64; 3], center: [f64; 3], half: f64, nudge: f64, color: u32) {
    let mut center = center;
    let mut position = sub(camera, center);
    if nudge > 0.0 {
        center = add(center, scale(normalize(position), nudge));
        position = sub(camera, center);
    }
    let distance = length(sub(camera, center));
    let half_length = half * distance * 0.001;
    let forward = normalize(position);
    let right = cross_product([0.0, 1.0, 0.0], forward);
    let right = if length(right).powi(2) < 1e-6 { [1.0, 0.0, 0.0] } else { normalize(right) };
    let up = normalize(cross_product(forward, right));
    out.line(add(center, scale(right, -half_length)), add(center, scale(right, half_length)), color, WIDTH);
    out.line(add(center, scale(up, -half_length)), add(center, scale(up, half_length)), color, WIDTH);
}

/// Once a frame while the world is drawn.
pub(crate) fn draw(game: &crate::game::Game<'_>, partial: f32, camera: [f64; 3], out: &mut WorldPrimitives) {
    if !game.native_on(super::betterhitreg::ID) || !game.native_window() {
        return;
    }
    let hitbox = config::on("renderHitbox");
    let cross_on = config::on("renderCross");
    let server_hitbox = config::on("RenderServerHitbox");
    let approach = config::on("RenderApproachHitbox");
    let your_reach = config::on("RenderYourReach");
    let their_reach = config::on("RenderTheirReach");
    let your_jump = config::on("RenderYourJump");
    let their_jump = config::on("RenderTheirJump");
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let t = f64::from(partial);
    let prev = [game.previous_position.x, game.previous_position.y, game.previous_position.z];
    let me = lerp3(prev, [p.position.x, p.position.y, p.position.z], t);
    let eye = [me[0], me[1] + f64::from(p.dimensions.eye_height), me[2]];
    let my_ground = ground(game, me, p.ground.on_ground);
    // `Render.renderFloor`: Custom Ground and Grid Floor, at your ground.
    if let Some(g) = ground(game, [p.position.x, p.position.y, p.position.z], p.ground.on_ground) {
        floor(out, camera, g);
    }
    if !(hitbox || cross_on || server_hitbox || approach || your_reach || their_reach || your_jump || their_jump) {
        return;
    }

    let target = TARGET.get().and_then(|id| game.mobs.iter().find(|m| m.id == id));
    let player_alive = !p.health.dead_or_dying() && !game.spectator();
    let show_invisible = config::on("invisHitboxes");
    let target = target.filter(|m| player_alive && m.alive() && (!m.is_invisible() || show_invisible));
    let Some(m) = target else {
        // `bothAlive` false: only your own rings, in their far colours.
        if let Some(g) = my_ground {
            let center = [me[0], g, me[2]];
            if your_reach {
                ring(out, center, 3.0, style("your_reach_far"));
            }
            if your_jump {
                ring(out, center, 4.0, style("your_jump_far"));
            }
        }
        return;
    };

    // `getBoundingBox`: the hitbox moved to where the target is drawn this frame.
    let base = m.hitbox();
    let (lo, hi) = (base.min_position(), base.max_position());
    let pos = [m.position.x, m.position.y, m.position.z];
    let drawn = lerp3([m.previous.x, m.previous.y, m.previous.z], pos, t);
    let d = sub(drawn, pos);
    let (min, max) = ([lo.x + d[0], lo.y + d[1], lo.z + d[2]], [hi.x + d[0], hi.y + d[1], hi.z + d[2]]);
    // `getClosestPoint` / `getDistance`: from your eyes, squared.
    let closest = [eye[0].clamp(min[0], max[0]), eye[1].clamp(min[1], max[1]), eye[2].clamp(min[2], max[2])];
    let distance = sub(eye, closest);
    let distance = distance[0] * distance[0] + distance[1] * distance[1] + distance[2] * distance[2];
    let in_hit_range = distance <= 9.0;
    let in_jump_range = distance <= 16.0;

    if let Some(g) = my_ground {
        let center = [me[0], g, me[2]];
        if your_reach {
            ring(out, center, 3.0, style(if in_hit_range { "your_reach_near" } else { "your_reach_far" }));
        }
        if your_jump {
            ring(out, center, 4.0, style(if in_jump_range { "your_jump_near" } else { "your_jump_far" }));
        }
    }
    if their_reach || their_jump {
        if let Some(g) = ground(game, drawn, m.on_ground) {
            let center = [drawn[0], g, drawn[2]];
            if their_reach {
                ring(out, center, 3.0, style(if in_hit_range { "their_reach_near" } else { "their_reach_far" }));
            }
            if their_jump {
                ring(out, center, 4.0, style(if in_jump_range { "their_jump_near" } else { "their_jump_far" }));
            }
        }
    }
    // Color Perfect Hits / Color Jump Resets tint both hitboxes for half a second.
    let flash = super::betterhitreg_fight::flash_color().map(style);
    if hitbox {
        let color = flash.unwrap_or_else(|| style(if in_hit_range { "hitbox_near" } else { "hitbox_far" }));
        out.box_outline(min, max, color, WIDTH);
    }
    if cross_on && distance <= 100.0 {
        let name = match (hitbox || server_hitbox, in_hit_range) {
            (true, true) => "cross_near_with_hitbox",
            (true, false) => "cross_far_with_hitbox",
            (false, true) => "cross_near",
            (false, false) => "cross_far",
        };
        cross(out, camera, closest, 30.0, 0.005, style(name));
    }
    if server_hitbox {
        out.box_outline(
            [lo.x, lo.y, lo.z],
            [hi.x, hi.y, hi.z],
            flash.unwrap_or_else(|| style(if in_hit_range { "server_hitbox_near" } else { "server_hitbox_far" })),
            WIDTH,
        );
    }
    let range = config::get("approach_hitbox_range").trim().parse::<f64>().unwrap_or(6.0);
    if approach && distance >= 9.0 && distance <= range * range {
        let grow = ((distance - 9.0) / 9.0) * 0.5;
        out.box_outline(
            [min[0] - grow, min[1] - grow, min[2] - grow],
            [max[0] + grow, max[1] + grow, max[2] + grow],
            style("approach_hitbox"),
            WIDTH,
        );
    }
}

/// `Render.renderFloor`: a flat ground of `ground_size` around the camera, and
/// grid lines every `floor_grid_size` blocks fading out over `floor_grid_range`.
fn floor(out: &mut WorldPrimitives, camera: [f64; 3], ground: f64) {
    let custom = config::on("CustomGround");
    let step = config::int("floor_grid_size");
    if !custom && step <= 0 {
        return;
    }
    let y = ground + config::float("ground_height");
    if custom {
        let size = config::int("ground_size") as f64;
        out.filled_box(
            [camera[0] - size, y, camera[2] - size],
            [camera[0] + size, y, camera[2] + size],
            style("ground"),
        );
    }
    if step != 0 {
        let radius = config::float("floor_grid_range");
        let step_f = step as f64;
        let min_x = ((camera[0] - radius) / step_f).floor() as i64 * step;
        let max_x = ((camera[0] + radius) / step_f).ceil() as i64 * step;
        let min_z = ((camera[2] - radius) / step_f).floor() as i64 * step;
        let max_z = ((camera[2] + radius) / step_f).ceil() as i64 * step;
        let rgb = style("grid") & 0xFF_FFFF;
        let mut x = min_x;
        while x <= max_x {
            let mut z = min_z;
            while z <= max_z {
                let a = [x as f64, y, z as f64];
                if x < max_x {
                    grid_line(out, camera, a, [(x + step) as f64, y, z as f64], radius, rgb);
                }
                if z < max_z {
                    grid_line(out, camera, a, [x as f64, y, (z + step) as f64], radius, rgb);
                }
                z += step;
            }
            x += step;
        }
    }
}

/// `drawLineIfVisible`: a grid line faded by its distance from the camera.
fn grid_line(out: &mut WorldPrimitives, camera: [f64; 3], a: [f64; 3], b: [f64; 3], fade_end: f64, rgb: u32) {
    let seg = (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let distance = if seg == 0.0 {
        length(sub(camera, a))
    } else {
        let fraction = (((camera[0] - a[0]) * (b[0] - a[0]) + (camera[2] - a[2]) * (b[2] - a[2])) / seg).clamp(0.0, 1.0);
        let closest = [a[0] + fraction * (b[0] - a[0]), a[1], a[2] + fraction * (b[2] - a[2])];
        length(sub(camera, closest))
    };
    if distance > fade_end {
        return;
    }
    let progress = (distance / fade_end).clamp(0.0, 1.0);
    let progress = progress * progress * (3.0 - 2.0 * progress);
    let alpha = (255.0 * (1.0 - progress)) as u32;
    out.line(a, b, alpha << 24 | rgb, WIDTH);
}
