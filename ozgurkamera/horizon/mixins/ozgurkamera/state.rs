// Özgür Kamera — mhv1.2 (mihehk): a free camera, apart from the player, that
// flies through walls (FreeCamera). The view anchor is local only: it is never
// put in the world or sent to the server; the player's body keeps its normal
// physics and movement packets with neutral input.
use std::cell::{Cell, RefCell};
use std::time::Instant;

pub(crate) const ID: &str = "ozgurkamera";
pub(crate) const TOGGLE: &str = "toggle";
pub(crate) const TOGGLE_DEFAULT: &str = "key.keyboard.f6";

/// The free camera: its eye, its own look, and the dimension it was opened in.
struct Camera {
    eye: [f64; 3],
    yaw: f32,
    pitch: f32,
    dimension: &'static str,
    last_frame: Option<Instant>,
}

// The window's own state: a single player world's server runs a `Game` of its
// own (with no player) on its thread, and must neither see nor end the camera.
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static CAMERA: RefCell<Option<Camera>> = const { RefCell::new(None) };
}

pub(crate) fn active() -> bool {
    ACTIVE.get()
}

fn turkish(game: &crate::game::Game<'_>) -> bool {
    game.settings.file.string("lang").starts_with("tr")
}

fn message(game: &mut crate::game::Game<'_>, enabled: bool) {
    let text = match (turkish(game), enabled) {
        (true, true) => "Özgür Kamera açık",
        (true, false) => "Özgür Kamera kapalı",
        (false, true) => "Free camera enabled",
        (false, false) => "Free camera disabled",
    };
    game.send_overlay_message(horizon_client::text::Text::literal(text));
}

fn alive(game: &crate::game::Game<'_>) -> bool {
    game.player.as_ref().is_some_and(|p| !p.health.dead_or_dying())
}

/// `FreeCamera.toggle`.
pub(crate) fn toggle(game: &mut crate::game::Game<'_>) {
    if active() {
        disable(game);
        return;
    }
    if !alive(game) || game.sleeping.is_some() {
        return;
    }
    let (Some(eye), Some(p)) = (game.eye_position(), game.native_player()) else {
        return;
    };
    CAMERA.with(|c| {
        *c.borrow_mut() = Some(Camera {
            eye,
            yaw: p.yaw,
            pitch: p.pitch,
            dimension: p.dimension,
            last_frame: Some(Instant::now()),
        })
    });
    ACTIVE.set(true);
    // Local state only, no packets: the block being broken and the item in use.
    game.mining = None;
    game.using = None;
    message(game, true);
}

/// `FreeCamera.disable`.
pub(crate) fn disable(game: &mut crate::game::Game<'_>) {
    if !active() {
        return;
    }
    ACTIVE.set(false);
    CAMERA.with(|c| *c.borrow_mut() = None);
    if game.player.is_some() {
        message(game, false);
    }
}

/// `FreeCamera.tick`: the camera goes when its player, world or life does.
pub(crate) fn tick(game: &mut crate::game::Game<'_>) {
    if !active() || !game.native_window() {
        return;
    }
    let dimension = CAMERA.with(|c| c.borrow().as_ref().map(|c| c.dimension));
    let same_world = game.native_player().map(|p| p.dimension) == dimension;
    if !game.native_on(ID) || !alive(game) || game.sleeping.is_some() || !same_world {
        disable(game);
    }
}

/// The toggle key, pressed with no screen open and the window focused.
pub(crate) fn key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_on(ID) || !game.native_window() {
        return;
    }
    if key == game.native_mod_key(ID, TOGGLE, TOGGLE_DEFAULT)
        && game.native_playing()
        && game.focus_lost.is_none()
    {
        toggle(game);
    }
}

fn held(game: &mut crate::game::Game<'_>, name: &str) -> bool {
    game.native_mapping(name).is_some_and(|(down, _)| down)
}

fn axis(game: &mut crate::game::Game<'_>, positive: &str, negative: &str) -> f64 {
    f64::from(u8::from(held(game, positive))) - f64::from(u8::from(held(game, negative)))
}

/// `FreeCamera.updateFrame`: once per rendered frame, apart from the 20 Hz tick.
pub(crate) fn frame(game: &mut crate::game::Game<'_>) {
    if !active() || !game.native_window() {
        return;
    }
    let now = Instant::now();
    let seconds = CAMERA.with(|c| {
        let mut c = c.borrow_mut();
        let cam = c.as_mut()?;
        let s = cam
            .last_frame
            .map_or(0.0, |t| now.duration_since(t).as_secs_f64().clamp(0.0, 0.1));
        cam.last_frame = Some(now);
        Some(s)
    });
    let Some(seconds) = seconds else {
        return;
    };
    if !game.native_playing() || game.focus_lost.is_some() {
        return;
    }
    let forward = axis(game, "key.forward", "key.back");
    let sideways = axis(game, "key.right", "key.left");
    let vertical = axis(game, "key.jump", "key.sneak");
    let speed = if held(game, "key.sprint") { 42.0 } else { 14.0 };
    CAMERA.with(|c| {
        let mut c = c.borrow_mut();
        let Some(cam) = c.as_mut() else {
            return;
        };
        let yaw = f64::from(cam.yaw).to_radians();
        let pitch = f64::from(cam.pitch).to_radians();
        let mut m = [
            -yaw.sin() * pitch.cos() * forward + yaw.cos() * -sideways,
            -pitch.sin() * forward + vertical,
            yaw.cos() * pitch.cos() * forward + yaw.sin() * -sideways,
        ];
        // Combined axes keep one speed flying diagonally.
        let length = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
        if length * length > 1.0 {
            m = [m[0] / length, m[1] / length, m[2] / length];
        }
        for (e, d) in cam.eye.iter_mut().zip(m) {
            *e += d * speed * seconds;
        }
    });
}

/// `Entity.changeLookDirection` on the camera instead of the player.
pub(crate) fn turn(xo: f64, yo: f64) -> bool {
    if !active() {
        return false;
    }
    CAMERA.with(|c| {
        if let Some(cam) = c.borrow_mut().as_mut() {
            cam.pitch = (cam.pitch + yo as f32 * 0.15).clamp(-90.0, 90.0);
            cam.yaw += xo as f32 * 0.15;
        }
    });
    true
}

/// The world is drawn from the camera: its eye and its exact look.
pub(crate) fn pose(position: &mut [f64; 3], yaw: &mut f32, pitch: &mut f32) {
    if !active() {
        return;
    }
    CAMERA.with(|c| {
        if let Some(cam) = c.borrow().as_ref() {
            *position = cam.eye;
            *yaw = cam.yaw;
            *pitch = cam.pitch;
        }
    });
}
