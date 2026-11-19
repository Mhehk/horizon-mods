// Freelook — mhv1 (mihehk): a Horizon port of Celibistrial's Freelook
// (https://github.com/Celibistrial/freelook), licensed like the original under
// the GNU Affero General Public License v3 (LICENSE beside this mod).
//
// While the key is held (or toggled), the mouse turns the camera around the
// player instead of the player: the body keeps looking and moving where it was.
// The settings are the original's (`config/freeLook.json`), on Options > Mods >
// Freelook, which the menu key (M) opens like the original's Freelook Config.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

pub(crate) const ID: &str = "freelook";

/// The keys, as `NativeMod.keys` lists them: name, label, default.
pub(crate) const KEYS: [(&str, &str, &str); 5] = [
    ("activate", "Activate Freelook", "key.keyboard.left.alt"),
    ("activate_first_person", "Enable First Person Freelook", "key.keyboard.unknown"),
    ("activate_second_person", "Enable Second Person Freelook", "key.keyboard.unknown"),
    ("activate_third_person", "Enable Third Person Freelook", "key.keyboard.unknown"),
    ("menu", "Open Freelook Menu", "key.keyboard.m"),
];

// ---------------------------------------------------------------------------------------------
// FreeLookConfig (config/freeLook.json)
// ---------------------------------------------------------------------------------------------

struct Config {
    toggle: bool,
    /// 1 first person, 2 second (front), 3 third (back).
    perspective: i32,
    max_head_yaw: f32,
    block_list: Vec<String>,
    stamp: Option<SystemTime>,
    checked: Instant,
}

impl Config {
    fn reset(&mut self) {
        self.toggle = false;
        self.perspective = 3;
        self.max_head_yaw = 360.0;
    }
}

fn path() -> Option<PathBuf> {
    crate::game::Game::native_config_dir()
        .ok()
        .map(|d| d.join("freeLook.json"))
}

fn stamp() -> Option<SystemTime> {
    std::fs::metadata(path()?).and_then(|m| m.modified()).ok()
}

/// `FreeLookConfig.load`: a missing file resets; a bad one keeps the values.
fn load(c: &mut Config) {
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        c.reset();
        return;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        log::error!(target: "Freelook", "Failed to read file freeLook.json");
        return;
    };
    c.perspective = v.get("perspective").and_then(|p| p.as_i64()).unwrap_or(0).clamp(1, 3) as i32;
    c.toggle = v.get("isToggle").and_then(|t| t.as_bool()).unwrap_or(false);
    c.max_head_yaw = (v.get("maxHeadYaw").and_then(|y| y.as_f64()).unwrap_or(0.0) as f32).clamp(5.0, 360.0);
    c.block_list = v
        .get("blockList")
        .and_then(|l| l.as_array())
        .map(|l| l.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// `FreeLookConfig.save`, as Gson's pretty printer writes it.
fn save(c: &mut Config) {
    let Some(path) = path() else {
        return;
    };
    let list = if c.block_list.is_empty() {
        "[]".to_owned()
    } else {
        let items: Vec<String> = c.block_list.iter().map(|s| format!("    {}", json_string(s))).collect();
        format!("[\n{}\n  ]", items.join(",\n"))
    };
    let text = format!(
        "{{\n  \"isToggle\": {},\n  \"perspective\": {},\n  \"maxHeadYaw\": {:?},\n  \"blockList\": {}\n}}",
        c.toggle, c.perspective, c.max_head_yaw, list
    );
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, text) {
        Ok(()) => log::info!(target: "Freelook", "Saved config"),
        Err(e) => log::error!(target: "Freelook", "Failed to write file freeLook.json: {e}"),
    }
    c.stamp = stamp();
}

thread_local! {
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
}

fn with_config<R>(f: impl FnOnce(&mut Config) -> R) -> R {
    CONFIG.with(|cell| {
        let mut cell = cell.borrow_mut();
        let c = cell.get_or_insert_with(|| {
            let mut c = Config {
                toggle: false,
                perspective: 3,
                max_head_yaw: 360.0,
                block_list: vec!["hypixel.net".to_owned()],
                stamp: None,
                checked: Instant::now(),
            };
            load(&mut c);
            if path().is_some_and(|p| !p.exists()) {
                save(&mut c);
            }
            c.stamp = stamp();
            c
        });
        f(c)
    })
}

/// The file edited by hand: read again once its time changes (checked each second).
fn watch(now: Instant) {
    with_config(|c| {
        if now.duration_since(c.checked) < Duration::from_secs(1) {
            return;
        }
        c.checked = now;
        let s = stamp();
        if s != c.stamp {
            c.stamp = s;
            load(c);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// FreeLookMod: the keys and the state
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum View {
    First,
    Front,
    Back,
}

impl View {
    fn of(perspective: i32) -> Self {
        match perspective {
            1 => View::First,
            2 => View::Front,
            _ => View::Back,
        }
    }
    fn camera(self) -> super::CameraType {
        match self {
            View::First => super::CameraType::FirstPerson,
            View::Front => super::CameraType::ThirdPersonFront,
            View::Back => super::CameraType::ThirdPersonBack,
        }
    }
}

#[derive(Default)]
struct Look {
    free_looking: bool,
    last_perspective: Option<super::CameraType>,
    view: Option<View>,
    /// The key name (`KEYS`) that holds Freelook in hold mode.
    hold: Option<&'static str>,
    blocked: bool,
    connected: bool,
    /// The camera's own look (`CameraOverriddenEntity`) and its anchor.
    camera_yaw: f32,
    camera_pitch: f32,
    anchor_yaw: f32,
    has_anchor: bool,
    first_time: bool,
    /// Keys held and presses not consumed yet, by mapping name.
    held: HashSet<&'static str>,
    clicks: Vec<&'static str>,
}

thread_local! {
    static LOOK: RefCell<Look> = RefCell::new(Look { first_time: true, ..Look::default() });
    static IN_CAMERA: Cell<bool> = const { Cell::new(false) };
    /// The settings page's sub-page: the disabled servers list.
    static SERVERS_PAGE: Cell<bool> = const { Cell::new(false) };
}

fn bound(game: &crate::game::Game<'_>, name: &str, default: &str) -> String {
    game.native_mod_key(ID, name, default)
}

/// A key went down or up: the mappings bound to it hold or click.
pub(crate) fn key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(ID) || !game.native_window() || key == "key.keyboard.unknown" {
        return;
    }
    let playing = game.native_playing();
    let names: Vec<&'static str> = KEYS
        .iter()
        .filter(|(n, _, d)| bound(game, n, d) == key)
        .map(|(n, _, _)| *n)
        .collect();
    LOOK.with(|l| {
        let mut l = l.borrow_mut();
        for n in names {
            if pressed {
                // A key mapping clicks and holds only in the world (KeyMapping.click).
                if playing {
                    l.held.insert(n);
                    l.clicks.push(n);
                }
            } else {
                l.held.remove(n);
            }
        }
    });
}

/// `KeyMapping.consumeClick` (this tick's presses of that mapping).
fn consume(l: &mut Look, name: &str) -> bool {
    let clicked = l.clicks.contains(&name);
    l.clicks.retain(|n| *n != name);
    clicked
}

fn open_menu(game: &mut crate::game::Game<'_>) {
    game.list_native_mods();
    if let Some(index) = game.settings.mods.installed.iter().position(|m| m.id == ID) {
        SERVERS_PAGE.set(false);
        game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(index));
    }
}

fn start(game: &mut crate::game::Game<'_>, l: &mut Look, view: View) {
    l.last_perspective = Some(game.camera_type);
    set_view(game, l, view);
    l.free_looking = true;
}

fn set_view(game: &mut crate::game::Game<'_>, l: &mut Look, view: View) {
    l.view = Some(view);
    // Only switch from first person: it looks weird otherwise.
    if l.last_perspective.is_some_and(|p| p.first_person()) {
        game.camera_type = view.camera();
    }
}

fn stop(game: &mut crate::game::Game<'_>, l: &mut Look) {
    l.free_looking = false;
    l.view = None;
    l.hold = None;
    if let Some(p) = l.last_perspective {
        game.camera_type = p;
    }
}

/// `ClientPlayConnectionEvents.JOIN` / `DISCONNECT`: a server in the block list
/// turns Freelook off there.
fn connection(game: &crate::game::Game<'_>, l: &mut Look) {
    let connected = game.remote.is_some();
    if connected && !l.connected {
        if let Some(address) = game.server_packs.address.as_deref() {
            let ip = address.to_lowercase();
            l.blocked = with_config(|c| c.block_list.iter().any(|b| ip.contains(&b.to_lowercase())));
        }
    } else if !connected && l.connected {
        l.blocked = false;
    }
    l.connected = connected;
}

/// `FreeLookMod.onTickEnd`.
pub(crate) fn tick(game: &mut crate::game::Game<'_>) {
    // A single player world's server runs a `Game` of its own: the window's only.
    if !game.native_window() {
        return;
    }
    watch(Instant::now());
    let on = game.native_on(ID);
    let mut look = LOOK.with(|l| std::mem::take(&mut *l.borrow_mut()));
    connection(game, &mut look);
    if !on {
        if look.free_looking {
            stop(game, &mut look);
        }
        look.clicks.clear();
        LOOK.with(|l| *l.borrow_mut() = look);
        return;
    }
    if consume(&mut look, "menu") {
        open_menu(game);
    }
    let (toggle, perspective) = with_config(|c| (c.toggle, c.perspective));
    if !look.blocked {
        if !toggle {
            let requested = ["activate_first_person", "activate_second_person", "activate_third_person", "activate"]
                .into_iter()
                .find(|n| look.held.contains(n));
            if !look.free_looking && let Some(n) = requested {
                let view = view_of(n, perspective);
                start(game, &mut look, view);
                look.hold = Some(n);
            } else if look.free_looking && look.hold.is_some_and(|n| !look.held.contains(n)) {
                stop(game, &mut look);
                look.hold = None;
            }
        } else {
            let requested = [
                ("activate_first_person", View::First),
                ("activate_second_person", View::Front),
                ("activate_third_person", View::Back),
                ("activate", View::of(perspective)),
            ]
            .into_iter()
            .find(|(n, _)| consume(&mut look, n))
            .map(|(_, v)| v);
            if let Some(view) = requested {
                if !look.free_looking {
                    start(game, &mut look, view);
                } else if look.view == Some(view) {
                    stop(game, &mut look);
                } else {
                    set_view(game, &mut look, view);
                }
            }
        }
    }
    look.clicks.clear();
    LOOK.with(|l| *l.borrow_mut() = look);
}

fn view_of(name: &str, perspective: i32) -> View {
    match name {
        "activate_first_person" => View::First,
        "activate_second_person" => View::Front,
        "activate_third_person" => View::Back,
        _ => View::of(perspective),
    }
}

/// `EntityMixin.changeCameraLookDirection`: the mouse turns the camera.
pub(crate) fn turn(xo: f64, yo: f64) -> bool {
    let max = with_config(|c| c.max_head_yaw);
    LOOK.with(|l| {
        let mut l = l.borrow_mut();
        if !l.free_looking {
            l.has_anchor = false;
            return false;
        }
        let pitch_delta = yo * 0.15;
        let yaw_delta = xo * 0.15;
        if !l.has_anchor {
            l.anchor_yaw = l.camera_yaw;
            l.has_anchor = true;
        }
        l.camera_pitch = (l.camera_pitch + pitch_delta as f32).clamp(-90.0, 90.0);
        if max >= 360.0 {
            l.camera_yaw += yaw_delta as f32;
        } else {
            l.camera_yaw = (l.camera_yaw + yaw_delta as f32).clamp(l.anchor_yaw - max, l.anchor_yaw + max);
        }
        true
    })
}

/// `CameraMixin.lockRotation`: the camera takes the free look's rotation (from
/// the player's on the first frame), then moves back along it as usual.
pub(crate) fn camera_entity(
    game: &mut crate::game::Game<'_>,
) -> core::ops::ControlFlow<Option<super::CameraEntity>> {
    if IN_CAMERA.get() {
        return core::ops::ControlFlow::Continue(());
    }
    let free = LOOK.with(|l| l.borrow().free_looking);
    if !free {
        LOOK.with(|l| l.borrow_mut().first_time = true);
        return core::ops::ControlFlow::Continue(());
    }
    IN_CAMERA.set(true);
    let entity = game.player_camera_entity();
    IN_CAMERA.set(false);
    let (yaw, pitch) = (game.yaw, game.pitch);
    let mirrored = matches!(game.camera_type, super::CameraType::ThirdPersonFront);
    let entity = entity.map(|mut e| {
        LOOK.with(|l| {
            let mut l = l.borrow_mut();
            if l.first_time {
                l.camera_pitch = pitch;
                l.camera_yaw = yaw;
                l.anchor_yaw = yaw;
                l.has_anchor = true;
                l.first_time = false;
            }
            // The front view mirrors the rotation; the free look's is set after it.
            let (y, x) = if mirrored {
                (l.camera_yaw + 180.0, -l.camera_pitch)
            } else {
                (l.camera_yaw, l.camera_pitch)
            };
            e.y_rot = y;
            e.y_rot_o = y;
            e.x_rot = x;
            e.x_rot_o = x;
        });
        e
    });
    core::ops::ControlFlow::Break(entity)
}

// ---------------------------------------------------------------------------------------------
// FreelookScreen and DisabledServerScreen, on the mod's settings page
// ---------------------------------------------------------------------------------------------

const HEAD_YAW_STEPS: [f32; 12] = [360.0, 5.0, 15.0, 30.0, 45.0, 60.0, 90.0, 120.0, 150.0, 180.0, 240.0, 300.0];

pub(crate) fn settings(buttons: &mut Vec<String>) {
    with_config(|c| {
        if SERVERS_PAGE.get() {
            buttons.push("Add".to_owned());
            for ip in &c.block_list {
                buttons.push(format!("{ip} — Remove"));
            }
            buttons.push("Back".to_owned());
            return;
        }
        buttons.push(if c.toggle { "Freelook is set to Toggle" } else { "Freelook is set to Hold" }.to_owned());
        buttons.push(
            match View::of(c.perspective) {
                View::First => "Default Perspective: First Person",
                View::Front => "Default Perspective: Second Person",
                View::Back => "Default Perspective: Third Person",
            }
            .to_owned(),
        );
        buttons.push(if c.max_head_yaw >= 360.0 {
            "Head Rotation Limit: 360° (Full)".to_owned()
        } else {
            format!("Head Rotation Limit: {}°", c.max_head_yaw as i32)
        });
        buttons.push("Disabled Servers".to_owned());
    });
}

pub(crate) fn click(game: &mut crate::game::Game<'_>, button: usize) {
    let back = game.modifiers.shift_key();
    if SERVERS_PAGE.get() {
        let count = with_config(|c| c.block_list.len());
        match button {
            // "Enter IP" and Add: the IP is typed into the config file.
            0 => {
                if let Some(p) = path() {
                    if !p.exists() {
                        with_config(save);
                    }
                    let _ = std::process::Command::new("notepad.exe").arg(&p).spawn();
                }
            }
            b if b <= count => with_config(|c| {
                c.block_list.remove(b - 1);
                save(c);
            }),
            _ => SERVERS_PAGE.set(false),
        }
        return;
    }
    with_config(|c| {
        match button {
            0 => c.toggle = !c.toggle,
            // nextPerspective: third, first, second.
            1 => {
                c.perspective = if back {
                    match c.perspective {
                        1 => 3,
                        2 => 1,
                        _ => 2,
                    }
                } else {
                    match c.perspective {
                        1 => 2,
                        3 => 1,
                        _ => 3,
                    }
                }
            }
            2 => {
                let n = HEAD_YAW_STEPS.len();
                let i = HEAD_YAW_STEPS.iter().position(|s| *s == c.max_head_yaw);
                let next = match i {
                    Some(i) if back => (i + n - 1) % n,
                    Some(i) => (i + 1) % n,
                    None => 0,
                };
                c.max_head_yaw = HEAD_YAW_STEPS[next];
            }
            3 => {
                SERVERS_PAGE.set(true);
                return;
            }
            _ => return,
        }
        save(c);
    });
}

/// Leaving the settings page goes back to its first page.
pub(crate) fn frame(game: &crate::game::Game<'_>) {
    use horizon_client::options_ui::Page;
    let on_page = game.screen == crate::game::Screen::Options
        && matches!(game.ui.options.page(), Page::NativeMod(i)
            if game.settings.mods.installed.get(i).is_some_and(|m| m.id == ID));
    if !on_page {
        SERVERS_PAGE.set(false);
    }
}
