//! Horizon Voice Chat: a client for the Simple Voice Chat protocol (an
//! unofficial one, not affiliated with Simple Voice Chat or its author), speaking it so a Horizon player talks with players on the
//! Java mod on any server that has the voice chat plugin or mod.
//!
//! The game thread calls [`tick`], [`payload`], [`key`], [`command`] and the
//! HUD and settings functions; the voice runs on threads of its own.
pub mod audio;
pub mod buf;
pub mod crypt;
pub mod denoise;
pub mod dylib;
pub mod hud;
pub mod icons;
pub mod labels;
pub mod mix;
pub mod net;
pub mod opus;
pub mod proto;
pub mod screens;
pub mod skin;
pub mod svc_assets;
pub mod text;
pub mod ui;
pub mod user_icons;

use audio::{AudioIo, Devices, MicQueue};
use mix::{Listener, Mixer};
use net::{Conn, Mode, Redirect, Shared, Status};
use proto::{COMPAT_26, COMPAT_27, Group, PlayerState, Secret};
use screens::{DeviceKind, Screen};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const ID: &str = "voicechat";
pub const NAME: &str = "Voice Chat";
pub const VERSION: &str = "— mhv2";
pub const DESCRIPTION: &str = "Talk with players who use the Simple Voice Chat mod, on servers that have it (unofficial; built on the MIT Horizon client by Nirokx and BnCatt). \
V opens its screens: settings, groups and each player's volume; H hides the icons. Push to talk (Caps Lock) or voice activation, with noise suppression when rnnoise.dll is beside libopus.";

pub const KEY_PTT: &str = "push_to_talk";
pub const KEY_WHISPER: &str = "whisper";
pub const KEY_MUTE: &str = "mute";
pub const KEY_DISABLE: &str = "disable";
/// Simple Voice Chat's defaults: push to talk and whisper unbound (the setup
/// guide asks for the push to talk key), M mutes, N turns voice chat off.
pub const DEFAULT_PTT: &str = "key.keyboard.unknown";
pub const DEFAULT_WHISPER: &str = "key.keyboard.unknown";
pub const DEFAULT_MUTE: &str = "key.keyboard.m";
pub const DEFAULT_DISABLE: &str = "key.keyboard.n";
pub const KEY_MENU: &str = "menu";
pub const DEFAULT_MENU: &str = "key.keyboard.v";
pub const KEY_HIDE_ICONS: &str = "hide_icons";
pub const DEFAULT_HIDE_ICONS: &str = "key.keyboard.h";
pub const KEY_SETTINGS: &str = "voice_chat_settings";
pub const KEY_GROUP: &str = "voice_chat_group";
pub const KEY_RECORD: &str = "toggle_recording";
pub const KEY_VOLUMES: &str = "adjust_volumes";
pub const UNBOUND: &str = "key.keyboard.unknown";

/// The mod's keys as Simple Voice Chat names them, in its order: their name,
/// what they do, and their default.
pub const HOTKEYS: [(&str, &str, &str); 10] = [
    (KEY_PTT, "Push to Talk", DEFAULT_PTT),
    (KEY_WHISPER, "Whisper", DEFAULT_WHISPER),
    (KEY_MUTE, "Mute Microphone", DEFAULT_MUTE),
    (KEY_DISABLE, "Disable Voice Chat", DEFAULT_DISABLE),
    (KEY_HIDE_ICONS, "Hide Voice Chat Icons", DEFAULT_HIDE_ICONS),
    (KEY_MENU, "Voice Chat Menu", DEFAULT_MENU),
    (KEY_SETTINGS, "Voice Chat Settings", UNBOUND),
    (KEY_GROUP, "Group Management", UNBOUND),
    (KEY_RECORD, "Toggle Recording", UNBOUND),
    (KEY_VOLUMES, "Adjust Volumes", UNBOUND),
];

/// A player's eyes above their feet (`Player.getEyeHeight`, standing).
pub const EYE_HEIGHT: f64 = 1.62;

/// The game's side of a tick in a server's world.
pub struct World<'a> {
    /// The address the player joined, as typed (`host` or `host:port`).
    pub address: &'a str,
    pub listener: Option<Listener>,
    /// Other players by UUID, at their eyes.
    pub players: Vec<(u128, [f64; 3])>,
    pub redirect: Redirect,
    /// Players' skins (their atlas sprites), for faces on the screens and HUD.
    pub skins: Vec<(u128, String)>,
    /// The keys bound now, named, for the screens.
    pub keys: ui::KeyNames,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    PushToTalk,
    Whisper,
    Mute,
    Disable,
    HideIcons,
    ToggleRecording,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Auto,
    V26,
    V27,
}

/// The settings page and what is stored of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub mode: Mode,
    pub muted: bool,
    pub disabled: bool,
    pub volume: u32,
    pub mic_gain: u32,
    pub threshold: i32,
    pub protocol: Protocol,
    pub hud: bool,
    pub group_labels: bool,
    /// Noise suppression (rnnoise) on the microphone.
    pub denoise: bool,
    /// No status icon, and no speakers over heads.
    pub hide_icons: bool,
    /// Its lines in the chat (mute, voice chat off/on, groups, errors).
    pub chat_messages: bool,
    /// The devices chosen by name; None: the system's default.
    pub mic_device: Option<String>,
    pub speaker_device: Option<String>,
    /// `automatic_gain_control` (else `gain_db`, `microphone_gain`).
    pub agc: bool,
    pub gain_db: f32,
    /// `voice_activity_detection`: automatic (else the threshold).
    pub vad: bool,
    /// 3D audio: 0 normal, 1 reduced, 2 off.
    pub audio_type: u8,
    /// `onboarding_finished`.
    pub onboarded: bool,
    /// Sounds no player makes, 0 to 400 %.
    pub other_volume: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::PushToTalk,
            muted: true,
            disabled: false,
            volume: 100,
            mic_gain: 100,
            threshold: -50,
            protocol: Protocol::Auto,
            hud: true,
            group_labels: false,
            denoise: true,
            hide_icons: false,
            chat_messages: true,
            mic_device: None,
            speaker_device: None,
            agc: true,
            gain_db: 0.0,
            vad: true,
            audio_type: 0,
            onboarded: false,
            other_volume: 100,
        }
    }
}

const VOLUMES: [u32; 9] = [0, 50, 75, 100, 125, 150, 200, 250, 300];
const THRESHOLDS: [i32; 7] = [-70, -60, -50, -45, -40, -30, -20];

fn next<T: Copy + PartialEq>(list: &[T], v: T) -> T {
    let i = list.iter().position(|x| *x == v).map_or(0, |i| (i + 1) % list.len());
    list[i]
}

impl Settings {
    pub fn load(get: &dyn Fn(&str) -> Option<String>) -> Self {
        let d = Self::default();
        let num = |k: &str| get(k).and_then(|v| v.parse::<i64>().ok());
        Self {
            mode: if get("mode").as_deref() == Some("voice") { Mode::Voice } else { Mode::PushToTalk },
            muted: get("muted").as_deref() != Some("false"),
            disabled: get("disabled").as_deref() == Some("true"),
            volume: num("volume").map_or(d.volume, |v| v.clamp(0, 300) as u32),
            mic_gain: num("mic_gain").map_or(d.mic_gain, |v| v.clamp(0, 500) as u32),
            threshold: num("threshold").map_or(d.threshold, |v| v.clamp(-127, 0) as i32),
            protocol: match get("protocol").as_deref() {
                Some("2.6") => Protocol::V26,
                Some("2.7") => Protocol::V27,
                _ => Protocol::Auto,
            },
            hud: get("hud").as_deref() != Some("false"),
            group_labels: get("group_labels").as_deref() == Some("true"),
            denoise: get("denoise").as_deref() != Some("false"),
            hide_icons: get("hide_icons").as_deref() == Some("true"),
            chat_messages: get("chat_messages").as_deref() != Some("false"),
            mic_device: get("mic_device").filter(|n| !n.is_empty()),
            speaker_device: get("speaker_device").filter(|n| !n.is_empty()),
            agc: get("agc").as_deref() != Some("false"),
            gain_db: get("gain_db").and_then(|v| v.parse::<f32>().ok()).map_or(d.gain_db, |v| v.clamp(-40.0, 24.0)),
            vad: get("vad").as_deref() != Some("false"),
            audio_type: num("audio_type").map_or(0, |v| v.clamp(0, 2) as u8),
            onboarded: get("onboarded").as_deref() == Some("true"),
            other_volume: num("other_volume").map_or(100, |v| v.clamp(0, 400) as u32),
        }
    }

    pub fn stored(&self) -> Vec<(&'static str, String)> {
        vec![
            ("mode", if self.mode == Mode::Voice { "voice" } else { "ptt" }.into()),
            ("muted", self.muted.to_string()),
            ("disabled", self.disabled.to_string()),
            ("volume", self.volume.to_string()),
            ("mic_gain", self.mic_gain.to_string()),
            ("threshold", self.threshold.to_string()),
            ("protocol", match self.protocol {
                Protocol::Auto => "auto",
                Protocol::V26 => "2.6",
                Protocol::V27 => "2.7",
            }
            .into()),
            ("hud", self.hud.to_string()),
            ("group_labels", self.group_labels.to_string()),
            ("denoise", self.denoise.to_string()),
            ("hide_icons", self.hide_icons.to_string()),
            ("chat_messages", self.chat_messages.to_string()),
            ("mic_device", self.mic_device.clone().unwrap_or_default()),
            ("speaker_device", self.speaker_device.clone().unwrap_or_default()),
            ("agc", self.agc.to_string()),
            ("gain_db", self.gain_db.to_string()),
            ("vad", self.vad.to_string()),
            ("audio_type", self.audio_type.to_string()),
            ("onboarded", self.onboarded.to_string()),
            ("other_volume", self.other_volume.to_string()),
        ]
    }

    pub fn protocol_name(&self) -> &'static str {
        match self.protocol {
            Protocol::Auto => "Automatic",
            Protocol::V26 => "2.6.x",
            Protocol::V27 => "2.7",
        }
    }

    /// The settings page's buttons, in order.
    pub fn buttons(&self) -> Vec<String> {
        let on_off = |b: bool| if b { "On" } else { "Off" };
        vec![
            format!("Activation: {}", if self.mode == Mode::Voice { "Voice" } else { "Push to talk" }),
            format!("Microphone: {}", if self.muted { "Muted" } else { "On" }),
            format!("Voice chat: {}", on_off(!self.disabled)),
            format!("Voice Chat Volume: {}%", self.volume),
            format!("Gain: {}", if self.agc { "Auto".to_owned() } else { format!("{} dB", self.gain_db.round() as i32) }),
            format!("Voice activation level: {} dB", self.threshold),
            format!("Protocol: {}", self.protocol_name()),
            format!("Group list on screen: {}", on_off(self.hud)),
            format!("Group names above players: {}", on_off(self.group_labels)),
            format!("Noise suppression: {}", on_off(self.denoise)),
            format!("Hide icons: {}", on_off(self.hide_icons)),
            format!("Chat messages: {}", on_off(self.chat_messages)),
        ]
    }

    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.mode = if self.mode == Mode::Voice { Mode::PushToTalk } else { Mode::Voice },
            1 => self.muted = !self.muted,
            2 => self.disabled = !self.disabled,
            3 => self.volume = next(&VOLUMES, self.volume),
            4 => self.agc = !self.agc,
            5 => self.threshold = next(&THRESHOLDS, self.threshold),
            6 => {
                self.protocol = match self.protocol {
                    Protocol::Auto => Protocol::V26,
                    Protocol::V26 => Protocol::V27,
                    Protocol::V27 => Protocol::Auto,
                }
            }
            7 => self.hud = !self.hud,
            8 => self.group_labels = !self.group_labels,
            9 => self.denoise = !self.denoise,
            10 => self.hide_icons = !self.hide_icons,
            11 => self.chat_messages = !self.chat_messages,
            _ => {}
        }
    }
}

/// One server visit.
struct Session {
    address: String,
    joined: Instant,
    /// The versions asked so far, and when the next is due.
    asked: Vec<i32>,
    next_ask: Instant,
    /// The version that answered here before.
    remembered: Option<i32>,
    secret: Option<Secret>,
    conn: Option<Conn>,
    audio: Option<AudioIo>,
    states: HashMap<u128, PlayerState>,
    groups: HashMap<u128, Group>,
    group: Option<u128>,
}

impl Session {
    fn new(address: &str, remembered: Option<i32>) -> Self {
        let now = Instant::now();
        Self {
            address: address.to_owned(),
            joined: now,
            asked: Vec::new(),
            // The server knows the player a moment after the join.
            next_ask: now + Duration::from_secs(1),
            remembered,
            secret: None,
            conn: None,
            audio: None,
            states: HashMap::new(),
            groups: HashMap::new(),
            group: None,
        }
    }

    /// The names of a group's members, sorted.
    fn members(&self, group: u128) -> Vec<String> {
        let mut names: Vec<String> = self
            .states
            .values()
            .filter(|p| p.group == Some(group) && !p.disconnected)
            .map(|p| p.name.clone())
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    fn compat(&self) -> i32 {
        self.secret.as_ref().map_or(COMPAT_26, |s| s.compat)
    }

    /// The versions to ask, in order: the one that answered here before, then
    /// the releases', then 2.7's, twice over for a server still starting.
    fn plan(&self, protocol: Protocol) -> Vec<i32> {
        match protocol {
            Protocol::V26 => vec![COMPAT_26, COMPAT_26, COMPAT_26],
            Protocol::V27 => vec![COMPAT_27, COMPAT_27, COMPAT_27],
            Protocol::Auto => match self.remembered {
                Some(COMPAT_27) => vec![COMPAT_27, COMPAT_26, COMPAT_27, COMPAT_26],
                _ => vec![COMPAT_26, COMPAT_27, COMPAT_26, COMPAT_27],
            },
        }
    }
}

struct Voice {
    settings: Settings,
    loaded: bool,
    session: Option<Session>,
    /// Custom payloads for the server, sent at the tick.
    outbox: Vec<(String, Vec<u8>)>,
    /// Settings to write back, and chat lines for the player.
    store: Vec<(String, String)>,
    chat: Vec<String>,
    shared: Arc<Shared>,
    mixer: Arc<Mutex<Mixer>>,
    mic: Arc<MicQueue>,
    devices: Arc<Mutex<Devices>>,
    redirect: Option<Redirect>,
    menu: ui::Screens,
    /// The sound card's devices of the kind being chosen.
    device_list: Vec<String>,
    /// Players whose stored volume was looked up.
    volumes_loaded: HashSet<u128>,
    /// The keys named, keys to bind (name, key), and players' skins.
    keys: ui::KeyNames,
    binds: Vec<(String, String)>,
    skins: HashMap<u128, String>,
    mic_test: bool,
    /// When the recording started.
    recording: Option<Instant>,
    /// "Press V to set up" was said on this server.
    set_up_said: bool,
    /// Where recordings go.
    record_dir: Option<std::path::PathBuf>,
}

fn voice() -> MutexGuard<'static, Voice> {
    static VOICE: LazyLock<Mutex<Voice>> = LazyLock::new(|| {
        Mutex::new(Voice {
            settings: Settings::default(),
            loaded: false,
            session: None,
            outbox: Vec::new(),
            store: Vec::new(),
            chat: Vec::new(),
            shared: Arc::default(),
            mixer: Arc::default(),
            mic: Arc::default(),
            devices: Arc::new(Mutex::new(Devices::default())),
            redirect: None,
            menu: ui::Screens::default(),
            device_list: Vec::new(),
            volumes_loaded: HashSet::new(),
            keys: ui::KeyNames::default(),
            binds: Vec::new(),
            skins: HashMap::new(),
            mic_test: false,
            recording: None,
            set_up_said: false,
            record_dir: None,
        })
    });
    VOICE.lock().unwrap_or_else(|e| e.into_inner())
}

static SOUND_CARD: AtomicBool = AtomicBool::new(true);

/// For tools and tests: leave the sound card alone, and feed [`mic_queue`]
/// and read [`mixer`] instead.
pub fn use_sound_card(on: bool) {
    SOUND_CARD.store(on, Ordering::Relaxed);
}

pub fn mic_queue() -> Arc<MicQueue> {
    voice().mic.clone()
}

pub fn mixer() -> Arc<Mutex<Mixer>> {
    voice().mixer.clone()
}

/// The settings key a server's remembered version is kept under.
fn compat_key(address: &str) -> String {
    let clean: String = address
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect();
    format!("compat.{clean}")
}

impl Voice {
    fn apply_controls(&self) {
        let s = &self.settings;
        let mut c = self.shared.controls.lock().unwrap();
        c.mode = s.mode;
        c.muted = s.muted;
        c.disabled = s.disabled;
        c.volume = s.volume as f32 / 100.0;
        c.mic_gain = 10f32.powf(s.gain_db / 20.0);
        c.threshold_db = s.threshold as f32;
        c.denoise = s.denoise;
        c.agc = s.agc;
        c.vad = s.vad;
        c.audio_type = s.audio_type;
        c.mic_test = self.mic_test;
        c.other_volume = s.other_volume as f32 / 100.0;
    }

    /// A toggle's line for the chat, unless chat messages are off.
    fn say(&self, line: &str) -> Option<String> {
        self.settings.chat_messages.then(|| line.to_owned())
    }

    fn save(&mut self) {
        self.apply_controls();
        let stored = self.settings.stored();
        self.store.extend(stored.into_iter().map(|(k, v)| (k.to_owned(), v)));
    }

    fn send(&mut self, (channel, data): (&str, Vec<u8>)) {
        self.outbox.push((channel.to_owned(), data));
    }

    /// Tells the server whether the player muted or turned voice chat off.
    fn send_state(&mut self) {
        let Some(compat) = self.session.as_ref().filter(|s| s.secret.is_some()).map(Session::compat) else {
            return;
        };
        let (muted, disabled) = (self.settings.muted, self.settings.disabled);
        self.send(proto::update_state(compat, muted, disabled));
    }

    fn leave(&mut self) {
        if self.recording.is_some() {
            self.stop_recording();
        }
        self.set_up_said = false;
        // The connection's threads stop as it drops.
        self.session = None;
        self.shared.transmitting.store(false, Ordering::Relaxed);
        self.shared.talking.lock().unwrap().clear();
        *self.shared.status.lock().unwrap() = Status::Idle;
        *self.shared.mic_error.lock().unwrap() = None;
        self.mixer.lock().unwrap().clear();
    }

    fn on_secret(&mut self, data: &[u8]) {
        let Ok(secret) = Secret::parse(data) else {
            self.chat.push("Voice chat: the server sent a secret this version can't read".into());
            return;
        };
        let Some(redirect) = self.redirect else { return };
        let Some(session) = self.session.as_mut() else { return };
        if session.remembered != Some(secret.compat) {
            session.remembered = Some(secret.compat);
            self.store.push((compat_key(&session.address), secret.compat.to_string()));
        }
        // A new secret replaces the connection.
        session.conn = None;
        let server = match proto::split_host_port(&session.address) {
            Some((host, port)) => (host, port.unwrap_or(25565)),
            None => (session.address.clone(), 25565),
        };
        if let Err(e) = opus::check() {
            *self.shared.mic_error.lock().unwrap() = Some(e.clone());
            self.chat.push(format!("Voice chat can't start: {e}"));
        }
        session.secret = Some(secret.clone());
        session.conn = Some(Conn::start(
            secret,
            server,
            redirect,
            self.shared.clone(),
            self.mixer.clone(),
            self.mic.clone(),
        ));
        if session.audio.is_none() && SOUND_CARD.load(Ordering::Relaxed) {
            session.audio = Some(AudioIo::start(
                self.mixer.clone(),
                self.mic.clone(),
                self.devices.clone(),
                self.settings.mic_device.clone(),
                self.settings.speaker_device.clone(),
            ));
        }
        self.send_state();
    }

    fn on_payload(&mut self, channel: &str, data: &[u8]) {
        if channel == "voicechat:secret" {
            self.on_secret(data);
            return;
        }
        let Some(session) = self.session.as_mut() else { return };
        let compat = session.compat();
        match channel {
            "voicechat:states" => {
                if let Ok(states) = PlayerState::parse_all(data, compat) {
                    session.states = states.into_iter().map(|s| (s.uuid, s)).collect();
                }
            }
            "voicechat:state" => {
                if let Ok(s) = PlayerState::parse(data, compat) {
                    session.states.insert(s.uuid, s);
                }
            }
            "voicechat:remove_state" => {
                if let Ok(id) = proto::parse_uuid(data) {
                    session.states.remove(&id);
                }
            }
            "voicechat:add_group" => {
                if let Ok(g) = Group::parse(data) {
                    session.groups.insert(g.id, g);
                }
            }
            "voicechat:remove_group" => {
                if let Ok(id) = proto::parse_uuid(data) {
                    session.groups.remove(&id);
                }
            }
            "voicechat:joined_group" => {
                if let Ok((group, wrong_password)) = proto::parse_joined_group(data) {
                    if wrong_password {
                        self.menu.say(text::WRONG_PASSWORD, true);
                    } else {
                        session.group = group;
                        // Joined: the group's screen replaces joining, as the mod's does.
                        if group.is_some()
                            && matches!(self.menu.top(), Some(Screen::JoinGroup | Screen::EnterPassword(_) | Screen::CreateGroup))
                        {
                            self.menu.stack.retain(|s| !matches!(s, Screen::JoinGroup | Screen::EnterPassword(_) | Screen::CreateGroup));
                            self.menu.push(Screen::Group);
                        }
                        if group.is_none() && self.menu.top() == Some(&Screen::Group) {
                            self.menu.stack.pop();
                            self.menu.push(Screen::JoinGroup);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self, world: World, get: &dyn Fn(&str) -> Option<String>) {
        if !self.loaded {
            self.loaded = true;
            self.settings = Settings::load(get);
            self.apply_controls();
        }
        self.redirect = Some(world.redirect);
        self.keys = world.keys.clone();
        self.skins = world.skins.iter().cloned().collect();
        if self.session.as_ref().is_some_and(|s| s.address != world.address) {
            self.leave();
        }
        if self.session.is_none() {
            let remembered = get(&compat_key(world.address)).and_then(|v| v.parse().ok());
            self.session = Some(Session::new(world.address, remembered));
            self.send(("minecraft:register", proto::register_payload()));
        }
        if let Some(l) = world.listener {
            *self.shared.listener.lock().unwrap() = l;
        }
        *self.shared.players.lock().unwrap() = world.players.into_iter().collect();
        self.load_player_volumes(get);

        // `OnboardingManager`: "Press V to set up" once connected, until set up.
        let connected = self.session.as_ref().is_some_and(|s| s.secret.is_some())
            && *self.shared.status.lock().unwrap() == Status::Connected;
        if connected && !self.settings.onboarded && !self.set_up_said {
            self.set_up_said = true;
            self.chat.push(text::set_up(&self.keys.menu));
        }
        let protocol = self.settings.protocol;
        let session = self.session.as_mut().expect("made above");
        let now = Instant::now();
        if session.secret.is_none() && now >= session.next_ask {
            let plan = session.plan(protocol);
            if let Some(&compat) = plan.get(session.asked.len()) {
                session.asked.push(compat);
                session.next_ask = now + Duration::from_secs(3);
                self.send(proto::request_secret(compat));
            }
        }
        // The connection timed out: ask again, as the mod reconnects.
        let session = self.session.as_mut().expect("made above");
        let failed = matches!(*self.shared.status.lock().unwrap(), Status::Failed(_));
        if failed && session.secret.is_some() && now - session.joined > Duration::from_secs(5) {
            let compat = session.compat();
            session.secret = None;
            session.conn = None;
            session.asked.clear();
            session.next_ask = now + Duration::from_secs(5);
            session.remembered = Some(compat);
        }
    }

    /// The volumes stored for players seen for the first time.
    fn load_player_volumes(&mut self, get: &dyn Fn(&str) -> Option<String>) {
        let Some(session) = self.session.as_ref() else { return };
        let new: Vec<u128> = session.states.keys().filter(|id| !self.volumes_loaded.contains(id)).copied().collect();
        if new.is_empty() {
            return;
        }
        let mut c = self.shared.controls.lock().unwrap();
        for id in new {
            self.volumes_loaded.insert(id);
            if let Some(p) = get(&volume_key(id)).and_then(|v| v.parse::<u32>().ok()) {
                c.player_volumes.insert(id, p.min(screens::PLAYER_MAX as u32) as f32 / 100.0);
            }
        }
    }
}

/// A tick in a server's world (`world`), or out of one (None). Returns the
/// custom payloads to send the server and the settings to store.
pub fn tick(world: Option<World>, get: &dyn Fn(&str) -> Option<String>) -> Tick {
    let mut v = voice();
    match world {
        Some(w) => v.tick(w, get),
        None => {
            if v.session.is_some() {
                v.leave();
            }
        }
    }
    Tick {
        payloads: std::mem::take(&mut v.outbox),
        store: std::mem::take(&mut v.store),
        binds: std::mem::take(&mut v.binds),
        chat: {
            let lines = std::mem::take(&mut v.chat);
            if v.settings.chat_messages { lines } else { Vec::new() }
        },
    }
}

#[derive(Default)]
pub struct Tick {
    pub payloads: Vec<(String, Vec<u8>)>,
    pub store: Vec<(String, String)>,
    /// Keys to bind: (the mod's key name, the key).
    pub binds: Vec<(String, String)>,
    pub chat: Vec<String>,
}

/// A custom payload from the server: true when it was the voice chat's.
pub fn payload(channel: &str, data: &[u8]) -> bool {
    if !channel.starts_with("voicechat:") {
        return false;
    }
    voice().on_payload(channel, data);
    true
}

/// A key went down or up. Returns a line for the chat, for toggles.
pub fn key(which: Key, down: bool) -> Option<String> {
    let mut v = voice();
    match which {
        Key::PushToTalk => v.shared.controls.lock().unwrap().ptt = down,
        Key::Whisper => v.shared.controls.lock().unwrap().whisper = down,
        // No muting in push to talk (`MuteTooltipSupplier.canMuteMic`).
        Key::Mute if down && v.settings.mode == Mode::Voice => {
            v.settings.muted = !v.settings.muted;
            v.save();
            v.send_state();
        }
        Key::Disable if down => {
            v.settings.disabled = !v.settings.disabled;
            v.save();
            v.send_state();
            if v.settings.disabled {
                v.mixer.lock().unwrap().clear();
            }
        }
        Key::HideIcons if down => {
            v.settings.hide_icons = !v.settings.hide_icons;
            v.save();
            return v.say(if v.settings.hide_icons { text::ICONS_HIDDEN } else { text::ICONS_VISIBLE });
        }
        Key::ToggleRecording if down => {
            v.toggle_recording();
            return v.chat.pop();
        }
        _ => {}
    }
    None
}

pub fn settings_buttons() -> Vec<String> {
    voice().settings.buttons()
}

pub fn settings_click(button: usize) {
    voice().click(button);
}

/// The key a player's own volume is kept under: `volume.<uuid>`.
pub fn volume_key(id: u128) -> String {
    let h = format!("{id:032x}");
    format!("volume.{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

impl Voice {
    fn click(&mut self, button: usize) {
        let before = (self.settings.muted, self.settings.disabled);
        self.settings.click(button);
        self.save();
        if before != (self.settings.muted, self.settings.disabled) {
            self.send_state();
        }
        if self.settings.disabled {
            self.mixer.lock().unwrap().clear();
        }
    }

    /// The sound card again, with the devices chosen now.
    fn restart_audio(&mut self) {
        let Some(session) = self.session.as_mut() else { return };
        if session.audio.is_some() {
            session.audio = None;
            session.audio = Some(AudioIo::start(
                self.mixer.clone(),
                self.mic.clone(),
                self.devices.clone(),
                self.settings.mic_device.clone(),
                self.settings.speaker_device.clone(),
            ));
        }
    }

    /// `toggleRecording`: each player's voice kept while it runs, then written
    /// as one WAV per player in a folder named for when it started.
    fn toggle_recording(&mut self) {
        if self.recording.is_some() {
            self.stop_recording();
            return;
        }
        let allowed = self.session.as_ref().and_then(|s| s.secret.as_ref()).is_some_and(|s| s.allow_recording);
        if !allowed {
            self.chat.push(text::RECORDING_OFF_SERVER.into());
            return;
        }
        *self.shared.recording.lock().unwrap() = Some(HashMap::new());
        self.recording = Some(Instant::now());
        self.chat.push(text::RECORDING_STARTED.into());
    }

    fn stop_recording(&mut self) {
        let Some(tracks) = self.shared.recording.lock().unwrap().take() else { return };
        self.recording = None;
        self.chat.push(text::RECORDING_STOPPED.into());
        let Some(base) = self.record_dir.clone().or_else(|| dylib::data_dir().map(|d| d.join("recordings"))) else { return };
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let dir = base.join(format!("session-{stamp}"));
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.chat.push(format!("Failed to save recording session: {e}"));
            return;
        }
        let names: HashMap<u128, String> = self
            .session
            .as_ref()
            .map(|s| s.states.values().map(|p| (p.uuid, p.name.clone())).collect())
            .unwrap_or_default();
        for (id, pcm) in tracks {
            let name = names.get(&id).cloned().unwrap_or_else(|| format!("{id:032x}"));
            let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
            if let Err(e) = std::fs::write(dir.join(format!("{clean}.wav")), wav(&pcm)) {
                self.chat.push(format!("Failed to save recording session: {e}"));
                return;
            }
        }
        self.chat.push(text::save_session(&dir.display().to_string()));
    }

    /// The speaker test: half a second of a soft tone.
    fn test_sound(&mut self) {
        let mut m = self.mixer.lock().unwrap();
        for k in 0..25 {
            let pcm = (0..opus::FRAME)
                .map(|i| ((k * opus::FRAME + i) as f32 / 48_000.0 * 440.0 * std::f32::consts::TAU).sin() * 0.2)
                .collect();
            m.push(u128::MAX - 2, mix::Frame { pcm, gain: [1.0, 1.0] });
        }
        m.end(u128::MAX - 2);
    }

    fn status_line(&self) -> (String, u32) {
        let Some(s) = self.session.as_ref() else {
            return ("Not on a server".into(), GREY);
        };
        match &s.secret {
            None if s.asked.len() >= s.plan(self.settings.protocol).len() => ("No voice chat on this server".into(), GREY),
            None => ("Asking the server...".into(), YELLOW),
            Some(secret) => {
                let version = if secret.compat >= COMPAT_27 { "2.7" } else { "2.6.x" };
                match &*self.shared.status.lock().unwrap() {
                    Status::Connected => (format!("Connected (protocol {version})"), GREEN),
                    Status::Failed(e) => (e.clone(), RED),
                    _ => ("Connecting...".into(), YELLOW),
                }
            }
        }
    }

    fn view(&self) -> ui::View {
        let session = self.session.as_ref();
        let connected = session.is_some_and(|s| s.secret.is_some())
            && *self.shared.status.lock().unwrap() == Status::Connected;
        let mut groups: Vec<ui::GroupRow> = session
            .map(|s| {
                s.groups
                    .values()
                    .filter(|g| !g.hidden || s.group == Some(g.id))
                    .map(|g| {
                        let members = s.members(g.id);
                        let skins = members
                            .iter()
                            .map(|n| {
                                let id = s.states.values().find(|p| &p.name == n).map(|p| p.uuid)?;
                                self.skins.get(&id).cloned()
                            })
                            .collect();
                        ui::GroupRow {
                            id: g.id,
                            name: g.name.clone(),
                            has_password: g.has_password,
                            members,
                            ours: s.group == Some(g.id),
                            kind: g.kind,
                            skins,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Yours first, then by name.
        groups.sort_by_key(|g| (!g.ours, g.name.to_lowercase()));
        let volumes = self.shared.controls.lock().unwrap().player_volumes.clone();
        let own = session.and_then(|s| s.secret.as_ref()).map(|s| s.player);
        let mut players: Vec<ui::PlayerRow> = session
            .map(|s| {
                s.states
                    .values()
                    .filter(|p| !p.disconnected && Some(p.uuid) != own)
                    .map(|p| ui::PlayerRow {
                        id: p.uuid,
                        name: p.name.clone(),
                        volume: (volumes.get(&p.uuid).copied().unwrap_or(1.0) * 100.0).round() as u32,
                        skin: self.skins.get(&p.uuid).cloned(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        players.sort_by_key(|p| p.name.to_lowercase());
        // The player's own group and its members, yourself among them.
        let group = session.and_then(|s| {
            let id = s.group?;
            let row = groups.iter().find(|g| g.id == id)?.clone();
            let now = Instant::now();
            let talking = self.shared.talking.lock().unwrap();
            let transmitting = self.shared.transmitting.load(Ordering::Relaxed);
            let mut members: Vec<ui::Member> = s
                .states
                .values()
                .filter(|p| p.group == Some(id) && !p.disconnected)
                .map(|p| {
                    let heard = talking.get(&p.uuid).filter(|(at, _)| now - *at < TALKING);
                    let is_own = Some(p.uuid) == own;
                    ui::Member {
                        id: p.uuid,
                        name: p.name.clone(),
                        skin: self.skins.get(&p.uuid).cloned(),
                        talking: heard.is_some() || (is_own && transmitting),
                        whispering: heard.is_some_and(|(_, w)| *w),
                        disconnected: p.disconnected,
                        disabled: p.disabled,
                        volume: (volumes.get(&p.uuid).copied().unwrap_or(1.0) * 100.0).round() as u32,
                        own: is_own,
                    }
                })
                .collect();
            members.sort_by_key(|m| m.name.to_lowercase());
            Some((row, members))
        });
        let allow_recording = session.and_then(|s| s.secret.as_ref()).is_some_and(|s| s.allow_recording);
        let recording = self.recording.map(|t| {
            let secs = t.elapsed().as_secs();
            format!("{:02}:{:02}", secs / 60, secs % 60)
        });
        ui::View {
            group,
            allow_recording,
            recording,
            mic_test: self.mic_test,
            other_volume: self.settings.other_volume,
            keys: self.keys.clone(),
            status: self.status_line(),
            settings: self.settings.clone(),
            level_db: self.shared.level(),
            connected,
            groups_enabled: session.and_then(|s| s.secret.as_ref()).is_some_and(|s| s.groups),
            groups,
            players,
            devices: self.device_list.clone(),
            denoise: denoise::available(),
            protocol: self.settings.protocol_name().into(),
        }
    }

    /// A slider moved: the setting follows at once; it is stored when the
    /// button comes up ([`Voice::store_slider`]).
    fn slide(&mut self, slider: ui::Slider, v: f32) {
        use ui::Slider as S;
        match slider {
            S::Volume => self.settings.volume = screens::volume_of(v),
            S::Gain => self.settings.gain_db = screens::gain_of(v),
            S::Other => self.settings.other_volume = screens::player_of(v),
            S::Threshold => self.settings.threshold = screens::threshold_of(v),
            S::Player(id) => {
                let percent = screens::player_of(v);
                self.volumes_loaded.insert(id);
                self.shared.controls.lock().unwrap().player_volumes.insert(id, percent as f32 / 100.0);
            }
        }
        self.apply_controls();
    }

    fn store_slider(&mut self, slider: ui::Slider) {
        match slider {
            ui::Slider::Player(id) => {
                let v = self.shared.controls.lock().unwrap().player_volumes.get(&id).copied().unwrap_or(1.0);
                self.store.push((volume_key(id), ((v * 100.0).round() as u32).to_string()));
            }
            _ => self.save(),
        }
    }

    /// What a click or Enter on the screens asks of the voice chat.
    fn menu_action(&mut self, action: ui::Action) {
        use ui::Action as A;
        let in_group = self.session.as_ref().is_some_and(|s| s.group.is_some());
        let action = match action {
            // Group: the group's screen when in one, else joining one.
            A::Open(Screen::Group) if !in_group => A::Open(Screen::JoinGroup),
            other => other,
        };
        let kind_listed = match &action {
            A::Open(Screen::Devices(kind)) => Some(*kind),
            A::Open(Screen::Onboarding(screens::Step::Microphone)) => Some(DeviceKind::Input),
            A::Open(Screen::Onboarding(screens::Step::Speaker)) => Some(DeviceKind::Output),
            _ => None,
        };
        if let Some(kind) = kind_listed {
            self.device_list = if SOUND_CARD.load(Ordering::Relaxed) { audio::device_names(kind) } else { Vec::new() };
        }
        let Some(action) = self.menu.act(action) else { return };
        let group_name = |v: &Voice, id: u128| {
            v.session.as_ref().and_then(|s| s.groups.get(&id)).map(|g| (g.name.clone(), g.has_password))
        };
        match action {
            A::Mode => self.click(0),
            A::Mute => self.click(1),
            A::Disable => self.click(2),
            A::Protocol => self.click(6),
            A::GroupHud => self.click(7),
            A::Labels => self.click(8),
            A::Denoise => self.click(9),
            A::HideIcons => self.click(10),
            A::ChatMessages => self.click(11),
            A::Slide(slider, v) => self.slide(slider, v),
            A::Device(kind, name) => {
                match kind {
                    DeviceKind::Input => self.settings.mic_device = name,
                    DeviceKind::Output => self.settings.speaker_device = name,
                }
                self.save();
                self.restart_audio();
                // The device screens close on a choice; the setup guide's lists stay.
                if matches!(self.menu.top(), Some(Screen::Devices(_))) {
                    self.menu.pop();
                }
            }
            A::Join(id) => match group_name(self, id) {
                Some((_, true)) => {
                    self.menu.push(Screen::EnterPassword(id));
                    self.menu.focus = Some(ui::Field::JoinPassword);
                }
                Some((_, false)) => self.send(proto::join_group(id, None)),
                None => {}
            },
            A::JoinWithPassword => {
                let Some(Screen::EnterPassword(id)) = self.menu.top().copied() else { return };
                let password = self.menu.join_password.clone();
                if password.is_empty() {
                    return;
                }
                self.send(proto::join_group(id, Some(&password)));
            }
            A::Leave => {
                self.send(proto::leave_group());
                if self.menu.top() == Some(&Screen::Group) {
                    self.menu.stack.pop();
                    self.menu.push(Screen::JoinGroup);
                }
            }
            A::Agc => {
                self.settings.agc = !self.settings.agc;
                self.save();
            }
            A::Vad => {
                self.settings.vad = !self.settings.vad;
                self.save();
            }
            A::AudioType => {
                self.settings.audio_type = (self.settings.audio_type + 1) % 3;
                self.save();
            }
            A::MicTest => {
                self.mic_test = !self.mic_test;
                self.apply_controls();
                if !self.mic_test {
                    self.mixer.lock().unwrap().end(net::MIC_TEST_CHANNEL);
                }
            }
            A::BindPtt => self.menu.listening = true,
            A::Record => self.toggle_recording(),
            A::TestSpeaker => self.test_sound(),
            A::ChooseMode(voice) => {
                self.settings.mode = if voice { Mode::Voice } else { Mode::PushToTalk };
                self.save();
                let next = if voice { screens::Step::Voice } else { screens::Step::PushToTalk };
                self.menu.push(Screen::Onboarding(next));
            }
            A::FinishOnboarding => {
                self.settings.onboarded = true;
                // Voice activation starts muted (`FinalOnboardingScreen`).
                if self.settings.mode == Mode::Voice {
                    self.settings.muted = true;
                }
                self.mic_test = false;
                self.save();
                self.send_state();
                self.menu.close();
            }
            A::Create => {
                let name = self.menu.name.trim().to_owned();
                if name.is_empty() {
                    self.menu.say("Type a name for the group", true);
                    self.menu.focus = Some(ui::Field::Name);
                    return;
                }
                if self.session.as_ref().is_none_or(|s| s.secret.is_none()) {
                    self.menu.say("Voice chat isn't connected", true);
                    return;
                }
                let password = std::mem::take(&mut self.menu.password);
                let password = (!password.is_empty()).then_some(password);
                self.send(proto::create_group(&name, password.as_deref(), self.menu.kind));
                self.menu.name.clear();
                self.menu.focus = None;
                // Back to the groups, where the new one shows up.
                if self.menu.top() == Some(&Screen::CreateGroup) {
                    self.menu.pop();
                }
                self.menu.say(format!("Creating {name}..."), false);
            }
            _ => {}
        }
    }
}

/// Opens the voice chat's screens, opened by a key typing `opened_by`: the
/// setup guide until it was done (or skipped), as the mod's key does.
pub fn menu_open(opened_by: Option<char>) {
    let mut v = voice();
    v.menu.open(opened_by);
    if !v.settings.onboarded {
        v.menu.stack = vec![Screen::Onboarding(screens::Step::Introduction)];
    }
}

/// Opens one screen straight away (the settings, group and volumes keys).
pub fn menu_open_at(screen: Screen, opened_by: Option<char>) {
    let mut v = voice();
    v.menu.open(opened_by);
    let screen = match screen {
        Screen::Group if v.session.as_ref().is_none_or(|s| s.group.is_none()) => Screen::JoinGroup,
        other => other,
    };
    v.menu.stack = vec![screen];
}

pub fn menu_is_open() -> bool {
    voice().menu.is_open()
}

/// The game's screen is (`ours`) or isn't the voice chat's.
pub fn menu_sync(ours: bool) {
    voice().menu.sync_open(ours);
}

/// Whether the game should close the screens now (once).
pub fn menu_take_close() -> bool {
    voice().menu.take_close()
}

/// Draws the screen on top of the stack.
pub fn menu_draw(size: [f32; 2], mouse: [f32; 2], measure: &dyn Fn(&str) -> f32) -> Vec<ui::Draw> {
    let mut v = voice();
    let Some(screen) = v.menu.top().copied() else {
        return Vec::new();
    };
    let view = v.view();
    let (draws, widgets) = screens::layout(&screen, &v.menu, &view, size, mouse, measure);
    v.menu.set_widgets(widgets);
    draws
}

/// The left button went down at `at`.
pub fn menu_click(at: [f32; 2]) {
    let mut v = voice();
    if let Some(action) = v.menu.press(at) {
        v.menu_action(action);
    }
}

/// The pointer moved with the button down: a slider being dragged follows.
/// True when one is (the game should not drag anything else).
pub fn menu_drag(at: [f32; 2]) -> bool {
    let mut v = voice();
    match v.menu.drag(at) {
        Some(action) => {
            v.menu_action(action);
            true
        }
        None => false,
    }
}

/// The button came up: a dragged slider's value is stored.
pub fn menu_release() {
    let mut v = voice();
    if let Some(slider) = v.menu.release() {
        v.store_slider(slider);
    }
}

/// A key on the screens: `menu_key` (the open key) closes them unless a
/// field takes the typing.
pub fn menu_key(key: &str, shift: bool, menu_key: &str) {
    let mut v = voice();
    // The push to talk button waits for a key: this one (Escape keeps the old).
    if v.menu.listening {
        v.menu.listening = false;
        if key != "key.keyboard.escape" {
            v.binds.push((KEY_PTT.to_owned(), key.to_owned()));
            v.keys.ptt = key_label(key);
        }
        return;
    }
    if key == menu_key && !v.menu.typing() {
        v.menu.close();
        return;
    }
    if let Some(action) = v.menu.key(key, shift) {
        v.menu_action(action);
    }
}

pub fn menu_char(c: char) {
    voice().menu.char_typed(c);
}

pub fn menu_scroll(steps: f64) {
    voice().menu.scroll_by(steps.signum() as i32);
}

/// `key.keyboard.v` → `v`: the character a key types, for single letters
/// and digits.
pub fn key_char(key: &str) -> Option<char> {
    let name = key.strip_prefix("key.keyboard.")?;
    let mut chars = name.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

/// Loads the stored settings before the first world, for the settings page.
pub fn load(get: &dyn Fn(&str) -> Option<String>) {
    let mut v = voice();
    if !v.loaded {
        v.loaded = true;
        v.settings = Settings::load(get);
        v.apply_controls();
    }
}

/// Settings changed since the last call, to store.
pub fn take_store() -> Vec<(String, String)> {
    std::mem::take(&mut voice().store)
}

/// Draws `draws` with `paint` (the game's fill, text and sprite helpers;
/// false when the atlas has no such sprite). A missing sprite is drawn as a
/// stand-in instead: the mod's icons from their compiled-in pixels, vanilla
/// sprites as plain fills (named in the log once).
pub fn render(draws: &[ui::Draw], paint: &mut dyn FnMut(&ui::Draw) -> bool) {
    static WARNED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);
    for d in draws {
        if paint(d) {
            continue;
        }
        if let ui::Draw::Sprite { name, .. } = d
            && skin::Icon::from_sprite(name).is_none()
            && !name.starts_with("svc:")
            && WARNED.lock().unwrap_or_else(|e| e.into_inner()).insert(name.clone())
        {
            log::warn!(target: "horizon_voicechat", "Voice Chat: no sprite {name} in the GUI atlas, drawing a plain stand-in");
        }
        for f in skin::fallback_all(d) {
            paint(&f);
        }
    }
}

const GREY: u32 = 0xAAAAAA;
const GREEN: u32 = 0x55FF55;
const RED: u32 = 0xFF5555;
const YELLOW: u32 = 0xFFFF55;

/// What the HUD draws this frame.
#[derive(Default)]
pub struct Hud {
    /// The status icon, bottom left.
    pub status: Vec<ui::Draw>,
    /// What doesn't work, above it.
    pub problems: Vec<ui::Draw>,
    /// The group's members, laid out from (0, 0), and the room they take.
    pub group: Vec<ui::Draw>,
    pub group_size: [f32; 2],
}

/// How long after a voice packet a player still counts as talking.
const TALKING: Duration = Duration::from_millis(300);

/// The HUD on a `size` screen: nothing on a server without voice chat.
pub fn hud(size: [f32; 2], measure: &dyn Fn(&str) -> f32) -> Hud {
    let v = voice();
    let Some(session) = v.session.as_ref().filter(|s| s.secret.is_some()) else {
        return Hud::default();
    };
    let status = v.shared.status.lock().unwrap().clone();
    let whisper = v.shared.controls.lock().unwrap().whisper;
    let transmitting = v.shared.transmitting.load(Ordering::Relaxed);
    let mut out = Hud::default();
    // `RenderEvents`: no icon in the first five seconds while connecting.
    let startup = status != Status::Connected && session.joined.elapsed() < Duration::from_secs(5);
    if !v.settings.hide_icons && !startup {
        let state = hud::HudState {
            connected: status == Status::Connected,
            muted: v.settings.muted && v.settings.mode == Mode::Voice,
            disabled: v.settings.disabled,
            transmitting,
            whispering: whisper && transmitting,
        };
        out.status = hud::status_icon(&state, size);
    }
    let mut problems = Vec::new();
    if let Status::Failed(e) = &status {
        problems.push(format!("Voice chat: {e}"));
    }
    if let Some(e) = v.shared.mic_error.lock().unwrap().as_ref() {
        problems.push(format!("Voice chat: {e}"));
    }
    {
        let d = v.devices.lock().unwrap();
        if let Err(e) = &d.input
            && e != "not open"
        {
            problems.push(format!("Microphone: {e}"));
        }
        if let Err(e) = &d.output
            && e != "not open"
        {
            problems.push(format!("Speakers: {e}"));
        }
    }
    out.problems = hud::problems(&problems, size, measure);
    if v.settings.hud
        && let Some(g) = session.group
    {
        let now = Instant::now();
        let talking = v.shared.talking.lock().unwrap();
        let own = session.secret.as_ref().map(|s| s.player);
        let mut members: Vec<hud::GroupFace> = session
            .states
            .values()
            .filter(|p| p.group == Some(g) && !p.disconnected)
            .map(|p| {
                let heard = talking.get(&p.uuid).is_some_and(|(at, _)| now - *at < TALKING);
                hud::GroupFace {
                    name: p.name.clone(),
                    skin: v.skins.get(&p.uuid).cloned(),
                    talking: heard || (Some(p.uuid) == own && transmitting),
                    disabled: p.disabled,
                }
            })
            .collect();
        members.sort_by_key(|m| m.name.to_lowercase());
        out.group = hud::group_faces(&members);
        out.group_size = [0.0, 0.0];
    }
    out
}

/// The group names to draw above the players the camera sees, on a `size`
/// screen.
pub fn group_labels(camera: &labels::Camera, size: [f32; 2]) -> Vec<labels::Label> {
    let v = voice();
    let Some(session) = v.session.as_ref().filter(|s| v.settings.group_labels && s.secret.is_some()) else {
        return Vec::new();
    };
    let players = v.shared.players.lock().unwrap();
    let shown: Vec<([f64; 3], String, bool)> = players
        .iter()
        .filter_map(|(id, eye)| {
            let (name, ours) = shown_group(session, *id)?;
            Some((*eye, name, ours))
        })
        .collect();
    labels::place(camera, size, &shown)
}

/// The group shown above player `id`, and whether it is the player's own:
/// hidden groups stay hidden, unless they are the player's own.
fn shown_group(session: &Session, id: u128) -> Option<(String, bool)> {
    let group = session.states.get(&id)?.group?;
    let g = session.groups.get(&group).filter(|g| !g.hidden || session.group == Some(group))?;
    Some((g.name.clone(), session.group == Some(group)))
}

/// `RenderEvents.onRenderName`: an icon right of each other player's name
/// tag: whispering, talking, disconnected, in another group, or voice chat off.
pub fn talk_icons(camera: &labels::Camera, size: [f32; 2], measure: &dyn Fn(&str) -> f32) -> Vec<ui::Draw> {
    let v = voice();
    let Some(session) = v.session.as_ref().filter(|s| !v.settings.hide_icons && s.secret.is_some()) else {
        return Vec::new();
    };
    let now = Instant::now();
    let players = v.shared.players.lock().unwrap();
    let talking = v.shared.talking.lock().unwrap();
    let own_group = session.group;
    let tags: Vec<([f64; 3], f32, &'static str)> = players
        .iter()
        .filter_map(|(id, eye)| {
            let state = session.states.get(id)?;
            let heard = talking.get(id).filter(|(at, _)| now - *at < TALKING);
            let icon = if heard.is_some_and(|(_, w)| *w) {
                "icons/speaker_whisper.png"
            } else if heard.is_some() {
                "icons/speaker.png"
            } else if state.disconnected {
                "icons/disconnected.png"
            } else if state.group.is_some() && state.group != own_group {
                "icons/group.png"
            } else if state.disabled {
                "icons/speaker_off.png"
            } else {
                return None;
            };
            Some((*eye, measure(&state.name), icon))
        })
        .collect();
    labels::name_icons(camera, size, &tags)
}

/// `key.keyboard.caps.lock` → `Caps Lock`.
pub fn key_label(key: &str) -> String {
    let name = key.strip_prefix("key.keyboard.").or_else(|| key.strip_prefix("key.mouse.")).unwrap_or(key);
    if name == "unknown" {
        return "Not Bound".into();
    }
    name.split('.')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub const COMMAND: &str = "vc";

const HELP: &[&str] = &[
    "/vc status - the connection, devices and who has voice chat",
    "/vc mute, /vc unmute - the microphone",
    "/vc off, /vc on - all of voice chat",
    "/vc mode ptt|voice - push to talk or voice activation",
    "/vc volume <0-200> - everyone's voices",
    "/vc volume <player> <0-200> - one player's voice",
    "/vc gain <10-500> - the microphone's gain",
    "/vc threshold <-127-0> - the voice activation level, in dB",
    "/vc groups - the server's groups",
    "/vc join <group> [password], /vc leave",
    "/vc create <name> [password] [normal|open|isolated]",
];

/// `/vc …`, without the slash: the chat lines it answers, or None when the
/// command isn't the voice chat's.
pub fn command(line: &str) -> Option<Vec<String>> {
    let mut words = line.split_whitespace();
    if words.next() != Some(COMMAND) {
        return None;
    }
    let args: Vec<&str> = words.collect();
    let mut v = voice();
    let out = |s: &str| vec![s.to_owned()];
    Some(match args.as_slice() {
        [] | ["help"] => HELP.iter().map(|s| s.to_string()).collect(),
        ["status"] => v.status(),
        ["mute"] | ["unmute"] => {
            v.settings.muted = args[0] == "mute";
            v.save();
            v.send_state();
            out(if v.settings.muted { "Microphone muted" } else { "Microphone on" })
        }
        ["off"] | ["on"] => {
            v.settings.disabled = args[0] == "off";
            v.save();
            v.send_state();
            out(if v.settings.disabled { "Voice chat off" } else { "Voice chat on" })
        }
        ["mode", m] => {
            v.settings.mode = match *m {
                "voice" | "va" => Mode::Voice,
                "ptt" | "push" => Mode::PushToTalk,
                _ => return Some(out("Modes: ptt, voice")),
            };
            v.save();
            out(if v.settings.mode == Mode::Voice { "Voice activation" } else { "Push to talk" })
        }
        ["volume", n] => match n.parse::<u32>() {
            Ok(n) if n <= 300 => {
                v.settings.volume = n;
                v.save();
                vec![format!("Voice volume {n}%")]
            }
            _ => out("Volume: 0 to 300"),
        },
        ["volume", name, n] => {
            let Ok(n) = n.parse::<u32>().map(|n| n.min(400)) else {
                return Some(out("Volume: 0 to 400"));
            };
            let id = v.session.as_ref().and_then(|s| {
                s.states.values().find(|p| p.name.eq_ignore_ascii_case(name)).map(|p| p.uuid)
            });
            match id {
                Some(id) => {
                    v.shared.controls.lock().unwrap().player_volumes.insert(id, n as f32 / 100.0);
                    v.volumes_loaded.insert(id);
                    v.store.push((volume_key(id), n.to_string()));
                    vec![format!("{name}'s voice at {n}%")]
                }
                None => vec![format!("No voice chat player named {name}")],
            }
        }
        ["gain", n] => match n.parse::<f32>() {
            Ok(n) if (-40.0..=24.0).contains(&n) => {
                v.settings.gain_db = n.round();
                v.settings.agc = false;
                v.save();
                vec![format!("Microphone gain {} dB", n.round())]
            }
            _ => out("Gain: -40 to 24 dB"),
        },
        ["threshold", n] => match n.parse::<i32>() {
            Ok(n) if (-127..=0).contains(&n) => {
                v.settings.threshold = n;
                v.save();
                vec![format!("Voice activation at {n} dB (now {:.0} dB)", v.shared.level())]
            }
            _ => out("Threshold: -127 to 0 dB"),
        },
        ["groups"] => {
            let Some(s) = v.session.as_ref().filter(|s| s.secret.is_some()) else {
                return Some(out("Voice chat isn't connected"));
            };
            let mut groups: Vec<&Group> = s.groups.values().filter(|g| !g.hidden || s.group == Some(g.id)).collect();
            groups.sort_by_key(|g| g.name.to_lowercase());
            let lines: Vec<String> = groups
                .into_iter()
                .map(|g| {
                    let members = s.members(g.id);
                    format!(
                        "{}{}{}: {}",
                        g.name,
                        if g.has_password { " (password)" } else { "" },
                        if s.group == Some(g.id) { " (yours)" } else { "" },
                        if members.is_empty() { "nobody".to_owned() } else { members.join(", ") }
                    )
                })
                .collect();
            if lines.is_empty() { out("No groups yet: /vc create <name>") } else { lines }
        }
        ["join", name, rest @ ..] => {
            let found = v.session.as_ref().and_then(|s| {
                s.groups.values().find(|g| g.name.eq_ignore_ascii_case(name)).map(|g| (g.id, g.has_password))
            });
            match found {
                Some((id, has_password)) => {
                    let password = rest.first().copied();
                    if has_password && password.is_none() {
                        return Some(vec![format!("{name} needs a password: /vc join {name} <password>")]);
                    }
                    v.send(proto::join_group(id, password));
                    vec![format!("Joining {name}...")]
                }
                None => vec![format!("No group named {name}: /vc groups")],
            }
        }
        ["leave"] => {
            v.send(proto::leave_group());
            out("Leaving the group...")
        }
        ["create", name, rest @ ..] => {
            let (mut password, mut kind) = (None, 0i16);
            for word in rest {
                match *word {
                    "normal" => kind = 0,
                    "open" => kind = 1,
                    "isolated" => kind = 2,
                    p => password = Some(p),
                }
            }
            if v.session.as_ref().is_none_or(|s| s.secret.is_none()) {
                return Some(out("Voice chat isn't connected"));
            }
            v.send(proto::create_group(name, password, kind));
            vec![format!("Creating the group {name}...")]
        }
        _ => HELP.iter().map(|s| s.to_string()).collect(),
    })
}

impl Voice {
    fn status(&self) -> Vec<String> {
        let mut out = Vec::new();
        let Some(s) = self.session.as_ref() else {
            return vec!["Not on a server".into()];
        };
        match &s.secret {
            None if s.asked.len() >= s.plan(self.settings.protocol).len() => {
                out.push("This server has no Simple Voice Chat (or a version this client doesn't speak)".into())
            }
            None => out.push("Asking the server for voice chat...".into()),
            Some(secret) => {
                let version = if secret.compat >= COMPAT_27 { "2.7" } else { "2.6.x" };
                out.push(format!(
                    "Voice chat {:?} (protocol {version}, distance {:.0} blocks)",
                    *self.shared.status.lock().unwrap(),
                    secret.distance
                ));
            }
        }
        let d = self.devices.lock().unwrap();
        let dev = |r: &Result<String, String>| r.clone().unwrap_or_else(|e| format!("none ({e})"));
        out.push(format!("Microphone: {}, speakers: {}", dev(&d.input), dev(&d.output)));
        if let Some(e) = self.shared.mic_error.lock().unwrap().as_ref() {
            out.push(format!("Problem: {e}"));
        }
        let mut names: Vec<String> = s
            .states
            .values()
            .filter(|p| !p.disconnected)
            .map(|p| format!("{}{}", p.name, if p.disabled { " (off)" } else if p.muted { " (muted)" } else { "" }))
            .collect();
        names.sort();
        if !names.is_empty() {
            out.push(format!("With voice chat: {}", names.join(", ")));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(address: &str) -> World<'_> {
        World { address, listener: None, players: Vec::new(), redirect: |h, p| (h.to_owned(), p), skins: Vec::new(), keys: ui::KeyNames::default() }
    }

    /// The tests share the one voice chat.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn asks_for_the_secret_with_both_versions() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let none = |_: &str| None;
        let t = tick(Some(world("play.example.net")), &none);
        assert_eq!(t.payloads[0].0, "minecraft:register");
        assert!(t.payloads.iter().all(|(c, _)| c != "voicechat:request_secret"));
        voice().session.as_mut().unwrap().next_ask = Instant::now();
        let t = tick(Some(world("play.example.net")), &none);
        assert_eq!(t.payloads, vec![("voicechat:request_secret".into(), 20i32.to_be_bytes().to_vec())]);
        voice().session.as_mut().unwrap().next_ask = Instant::now();
        let t = tick(Some(world("play.example.net")), &none);
        assert_eq!(t.payloads[0].1, 21i32.to_be_bytes().to_vec());
        assert!(!payload("minecraft:brand", b""));
        let t = tick(None, &none);
        assert!(t.payloads.is_empty());
        assert!(voice().session.is_none());
    }

    #[test]
    fn shows_other_players_groups() {
        use buf::Writer;
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let none = |_: &str| None;
        tick(None, &none);
        let mut w = world("groups.example.net");
        w.players = vec![(0xA, [0.0, 65.62, 10.0]), (0xB, [3.0, 65.62, 12.0])];
        tick(Some(w), &none);
        // As a 2.6 server sends them: a secret, the groups, then the states.
        let mut secret = Writer::new();
        secret.raw(&[1; 16]).i32(24454).uuid(0xC).u8(0).i32(1275);
        secret.raw(&48f64.to_be_bytes()).i32(1000).bool(true).utf("127.0.0.1").bool(false);
        {
            let mut v = voice();
            let s = v.session.as_mut().unwrap();
            s.secret = Some(Secret::parse(&secret.finish()).unwrap());
        }
        for (id, name) in [(0x61u128, "Builders"), (0x62, "Miners")] {
            let mut g = Writer::new();
            g.uuid(id).utf(name).bool(false).bool(true).bool(false).i16(0);
            assert!(payload("voicechat:add_group", &g.finish()));
        }
        let mut states = Writer::new();
        states.i32(3);
        for (id, name, group) in [(0xAu128, "Alex", 0x61u128), (0xB, "Bea", 0x62), (0xC, "Me", 0x61)] {
            states.bool(false).bool(false).uuid(id).utf(name).bool(true).uuid(group);
        }
        assert!(payload("voicechat:states", &states.finish()));
        let mut joined = Writer::new();
        joined.bool(true).uuid(0x61).bool(false);
        payload("voicechat:joined_group", &joined.finish());

        let camera = labels::Camera { pos: [0.0, 65.62, 0.0], yaw: 0.0, pitch: 0.0, fov: 70.0 };
        // Group names over players are this port's own extra, off unless chosen.
        assert!(group_labels(&camera, [400.0, 200.0]).is_empty());
        voice().settings.group_labels = true;
        let shown = group_labels(&camera, [400.0, 200.0]);
        voice().settings.group_labels = false;
        let texts: Vec<_> = shown.iter().map(|l| (l.text.as_str(), l.rgb)).collect();
        assert_eq!(texts, vec![("[Miners]", 0xFFFF55), ("[Builders]", 0x55FF55)]);
        let list = command("vc groups").unwrap();
        assert_eq!(list, vec!["Builders (yours): Alex, Me", "Miners: Bea"]);
        let measure = |t: &str| t.len() as f32 * 6.0;
        *voice().shared.status.lock().unwrap() = Status::Connected;
        // The group's faces on the HUD: one per member, from (4, 4).
        let h = hud([480.0, 270.0], &measure);
        assert!(!h.group.is_empty());

        // The menu opens on the setup guide until it is done.
        {
            let mut v = voice();
            let s = v.session.as_mut().unwrap();
            s.groups.get_mut(&0x62).unwrap().has_password = true;
            s.group = None;
        }
        menu_open(Some('v'));
        assert_eq!(voice().menu.top(), Some(&Screen::Onboarding(screens::Step::Introduction)));
        voice().menu_action(ui::Action::FinishOnboarding);
        assert!(voice().settings.onboarded && !menu_is_open());
        let _ = menu_take_close();

        // Group: not in one, so joining; a password group asks for it, then joins.
        menu_open(Some('v'));
        menu_draw([480.0, 270.0], [0.0, 0.0], &measure);
        let group = voice().menu.hit_rect(&ui::Action::Open(Screen::Group)).unwrap();
        menu_click([group[0] + 1.0, group[1] + 1.0]);
        assert_eq!(voice().menu.top(), Some(&Screen::JoinGroup));
        let draws = menu_draw([480.0, 270.0], [0.0, 0.0], &measure);
        let has = |d: &[ui::Draw], want: &str| d.iter().any(|d| matches!(d, ui::Draw::Text(t, ..) if t == want));
        assert!(has(&draws, "Join or Create Group") && has(&draws, "Miners"), "{:?}", screens::testing::texts(&draws));
        let join = voice().menu.hit_rect(&ui::Action::Join(0x62)).unwrap();
        tick(Some(world("groups.example.net")), &none);
        menu_click([join[0] + 1.0, join[1] + 1.0]);
        assert_eq!(voice().menu.top(), Some(&Screen::EnterPassword(0x62)));
        menu_char('v');
        for c in "hunter2".chars() {
            menu_char(c);
        }
        menu_key("key.keyboard.enter", false, DEFAULT_MENU);
        let t = tick(Some(world("groups.example.net")), &none);
        assert_eq!(t.payloads, vec![proto::join_group(0x62, Some("hunter2"))].into_iter().map(|(c, d)| (c.to_owned(), d)).collect::<Vec<_>>());
        // V types while the password box has the focus; out of it, V closes.
        menu_key("key.keyboard.escape", false, DEFAULT_MENU);
        menu_key(DEFAULT_MENU, false, DEFAULT_MENU);
        assert!(menu_take_close() && !menu_is_open());

        // One player's volume, dragged on Adjust Volumes, is stored and comes back.
        menu_open(None);
        voice().menu_action(ui::Action::Open(Screen::Volumes));
        menu_draw([480.0, 270.0], [0.0, 0.0], &measure);
        let r = voice().menu.hit_rect(&ui::Action::Slide(ui::Slider::Player(0xA), 0.25)).unwrap();
        menu_click([r[0] + 4.0, r[1] + 5.0]);
        assert!(menu_drag([r[0] + 4.0 + 0.5 * (r[2] - 8.0), r[1] + 5.0]));
        menu_release();
        let stored = take_store();
        let key = volume_key(0xA);
        assert_eq!(key, "volume.00000000-0000-0000-0000-00000000000a");
        assert!(stored.contains(&(key.clone(), "200".into())), "{stored:?}");
        voice().shared.controls.lock().unwrap().player_volumes.clear();
        voice().volumes_loaded.clear();
        tick(Some(world("groups.example.net")), &|k: &str| (k == key).then(|| "50".to_owned()));
        assert_eq!(voice().shared.controls.lock().unwrap().player_volumes.get(&0xA), Some(&0.5));
        voice().menu.close();
        tick(None, &none);
    }

    /// As the game's glue calls it: sprites the atlas has are drawn as
    /// they are, the mod's missing icons from their pixels, missing vanilla
    /// ones as plain fills.
    #[test]
    fn render_draws_stand_ins_for_missing_sprites() {
        let draws = vec![
            ui::Draw::Fill([0.0, 0.0, 4.0, 4.0], 0xFF00_0000),
            skin::icon(skin::Icon::Mic, [0.0, 0.0, 16.0, 16.0]),
            ui::Draw::Sprite { name: skin::BUTTON.name.into(), src: [0.0, 0.0, 200.0, 20.0], dst: [0.0, 0.0, 200.0, 20.0] },
        ];
        let (mut fills, mut sprites) = (0, Vec::new());
        render(&draws, &mut |d| match d {
            ui::Draw::Fill(..) => {
                fills += 1;
                true
            }
            ui::Draw::Text(..) => true,
            ui::Draw::Sprite { name, .. } => {
                sprites.push(name.clone());
                false
            }
        });
        assert_eq!(sprites.len(), 2);
        // The icon's pixels are many fills; the button is one.
        assert!(fills > 3, "{fills}");
    }

    #[test]
    fn settings_round_trip_and_cycle() {
        let mut s = Settings::default();
        s.click(0);
        s.click(3);
        s.click(6);
        s.click(9);
        s.click(10);
        s.mic_device = Some("USB Mic".into());
        let stored: HashMap<_, _> = s.stored().into_iter().collect();
        let back = Settings::load(&|k| stored.get(k).cloned());
        assert_eq!(back, s);
        assert_eq!((back.mode, back.volume, back.protocol), (Mode::Voice, 125, Protocol::V26));
        assert_eq!((back.denoise, back.hide_icons, back.speaker_device), (false, true, None));
        assert_eq!(key_label("key.keyboard.caps.lock"), "Caps Lock");
        assert_eq!(compat_key("Play.Example.net:25565"), "compat.play.example.net_25565");
    }
}

/// 48 kHz mono 16 bit PCM as a WAV file.
fn wav(pcm: &[i16]) -> Vec<u8> {
    let data = pcm.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&48_000u32.to_le_bytes());
    out.extend_from_slice(&96_000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}
