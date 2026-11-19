//! The voice connection: a UDP socket to the server's voice port, as
//! `ClientVoicechatConnection` runs it (authenticate, then check the
//! connection, then keep it alive), and the mic thread that sends what the
//! player says.
use crate::audio::MicQueue;
use crate::mix::{Frame, Listener, Mixer, spatial};
use crate::opus::{Decoder, Encoder, FRAME};
use crate::proto::{In, Link, Out, Secret, Sound, SoundKind};
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Where a server address leads (its SRV record): the game's own resolver.
pub type Redirect = fn(&str, u16) -> (String, u16);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    PushToTalk,
    Voice,
}

/// What the player chose, read by the threads.
#[derive(Clone, Debug)]
pub struct Controls {
    pub mode: Mode,
    pub ptt: bool,
    pub whisper: bool,
    pub muted: bool,
    pub disabled: bool,
    pub threshold_db: f32,
    pub mic_gain: f32,
    pub volume: f32,
    pub player_volumes: HashMap<u128, f32>,
    /// Noise suppression (rnnoise) on the microphone.
    pub denoise: bool,
    /// Automatic gain control (else `mic_gain`, from the manual gain in dB).
    pub agc: bool,
    /// Automatic voice activity detection (else the threshold).
    pub vad: bool,
    /// 3D audio: 0 normal, 1 reduced, 2 off.
    pub audio_type: u8,
    /// The microphone played back to the player.
    pub mic_test: bool,
    /// Sounds no player makes.
    pub other_volume: f32,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            mode: Mode::PushToTalk,
            ptt: false,
            whisper: false,
            muted: false,
            disabled: false,
            threshold_db: -50.0,
            mic_gain: 1.0,
            volume: 1.0,
            player_volumes: HashMap::new(),
            denoise: true,
            agc: true,
            vad: true,
            audio_type: 0,
            mic_test: false,
            other_volume: 1.0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Status {
    #[default]
    Idle,
    Connecting,
    Connected,
    Failed(String),
}

/// What the game thread and the voice threads share.
#[derive(Default)]
pub struct Shared {
    pub listener: Mutex<Listener>,
    /// Players' eye positions by UUID, this tick.
    pub players: Mutex<HashMap<u128, [f64; 3]>>,
    /// Who was heard last and when, and whether whispering.
    pub talking: Mutex<HashMap<u128, (Instant, bool)>>,
    pub controls: Mutex<Controls>,
    pub status: Mutex<Status>,
    pub mic_error: Mutex<Option<String>>,
    /// The microphone's last peak, dB as f32 bits.
    pub mic_level: AtomicU32,
    pub transmitting: AtomicBool,
    /// A recording: each sender's voice so far (48 kHz mono), by sender.
    pub recording: Mutex<Option<HashMap<u128, Vec<i16>>>>,
}

impl Shared {
    pub fn level(&self) -> f32 {
        f32::from_bits(self.mic_level.load(Ordering::Relaxed))
    }
    fn set_status(&self, s: Status) {
        *self.status.lock().unwrap() = s;
    }
}

pub struct Conn {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Conn {
    pub fn start(
        secret: Secret,
        server: (String, u16),
        redirect: Redirect,
        shared: Arc<Shared>,
        mixer: Arc<Mutex<Mixer>>,
        mic: Arc<MicQueue>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        shared.set_status(Status::Connecting);
        let thread = std::thread::Builder::new()
            .name("voicechat-net".into())
            .spawn(move || {
                let result = run(secret, server, redirect, &shared, &mixer, mic, &flag);
                mixer.lock().unwrap().clear();
                shared.transmitting.store(false, Ordering::Relaxed);
                shared.set_status(match result {
                    Ok(()) => Status::Idle,
                    Err(e) => Status::Failed(e),
                });
            })
            .ok();
        Self { stop, thread }
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn resolve(host: &str, port: u16) -> Result<SocketAddr, String> {
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("unknown voice host {host}: {e}"))?
        .collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or(addrs.first())
        .copied()
        .ok_or_else(|| format!("unknown voice host {host}"))
}

fn run(
    secret: Secret,
    (server_host, server_port): (String, u16),
    redirect: Redirect,
    shared: &Arc<Shared>,
    mixer: &Arc<Mutex<Mixer>>,
    mic: Arc<MicQueue>,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    // The game connected to the SRV target, if any: so does the voice.
    let (game_host, _) = redirect(&server_host, server_port);
    let (host, port) = secret.voice_address(&game_host);
    let addr = resolve(&host, port)?;
    log::info!(target: "horizon_voicechat", "Voice Chat: connecting to the voice server {addr}");
    let socket = UdpSocket::bind(if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })
        .map_err(|e| format!("no UDP socket: {e}"))?;
    socket.connect(addr).map_err(|e| format!("voice server {addr}: {e}"))?;
    socket.set_read_timeout(Some(Duration::from_millis(50))).map_err(|e| e.to_string())?;
    let keep_alive = Duration::from_millis(u64::from(secret.keep_alive) * 10);
    let own = secret.player;
    let link = Arc::new(Link::new(secret));
    let connected = Arc::new(AtomicBool::new(false));

    // The mic thread stops with this one, whatever ends it.
    let quit = Arc::new(AtomicBool::new(false));
    let mic_thread = {
        let (socket, link, shared, connected, quit, mixer) = (
            socket.try_clone().map_err(|e| e.to_string())?,
            link.clone(),
            shared.clone(),
            connected.clone(),
            quit.clone(),
            mixer.clone(),
        );
        std::thread::Builder::new()
            .name("voicechat-mic".into())
            .spawn(move || mic_loop(&socket, &link, &shared, &mic, &quit, &connected, &mixer))
            .ok()
    };

    let send = |p: &Out| {
        let _ = socket.send(&link.write(p));
    };
    let mut authenticated = false;
    let mut last_try: Option<Instant> = None;
    let mut last_keep_alive = Instant::now();
    let mut channels: HashMap<u128, Channel> = HashMap::new();
    let mut buf = vec![0u8; 4096];
    let mut heard: u64 = 0;
    let result = loop {
        if stop.load(Ordering::Relaxed) {
            break Ok(());
        }
        let now = Instant::now();
        if !connected.load(Ordering::Relaxed) {
            if last_try.is_none_or(|t| now - t >= Duration::from_secs(1)) {
                last_try = Some(now);
                send(if authenticated { &Out::ConnectionCheck } else { &Out::Authenticate });
            }
        } else if now - last_keep_alive > keep_alive {
            break Err("the voice server stopped answering".into());
        }
        let n = match socket.recv(&mut buf) {
            Ok(n) => n,
            // Timeouts, and ICMP's "unreachable" until the server is up.
            Err(_) => continue,
        };
        let Ok(Some(packet)) = link.read(&buf[..n]) else { continue };
        match packet {
            In::AuthenticateAck { nonce } => {
                if !authenticated {
                    log::info!(target: "horizon_voicechat", "Voice Chat: authenticated");
                    authenticated = true;
                    *link.nonce.lock().unwrap() = nonce;
                    last_try = None;
                }
            }
            In::ConnectionCheckAck => {
                if authenticated && !connected.swap(true, Ordering::Relaxed) {
                    log::info!(target: "horizon_voicechat", "Voice Chat: connected");
                    shared.set_status(Status::Connected);
                    last_keep_alive = now;
                }
            }
            In::KeepAlive => {
                last_keep_alive = now;
                send(&Out::KeepAlive);
            }
            In::Ping { id, timestamp } => send(&Out::Ping { id, timestamp }),
            In::Sound(sound) => {
                heard += 1;
                if heard == 1 || heard % 250 == 0 {
                    log::info!(target: "horizon_voicechat", "Voice Chat: {heard} sound packets heard");
                }
                if !shared.controls.lock().unwrap().disabled {
                    play(sound, own, shared, mixer, &mut channels);
                }
            }
        }
        channels.retain(|_, c| now - c.used < Duration::from_secs(30));
    };
    if let Err(e) = &result {
        log::warn!(target: "horizon_voicechat", "Voice Chat: the voice connection ended: {e}");
    }
    connected.store(false, Ordering::Relaxed);
    quit.store(true, Ordering::Relaxed);
    if let Some(t) = mic_thread {
        let _ = t.join();
    }
    result
}

struct Channel {
    decoder: Decoder,
    last: i64,
    used: Instant,
}

/// `AudioChannel.run` and `writeToSpeaker`: decode in order, conceal what
/// was lost, and place the voice where its sender is.
fn play(s: Sound, own: u128, shared: &Shared, mixer: &Mutex<Mixer>, channels: &mut HashMap<u128, Channel>) {
    let now = Instant::now();
    let ch = match channels.entry(s.channel) {
        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
        std::collections::hash_map::Entry::Vacant(e) => match Decoder::new() {
            Ok(decoder) => e.insert(Channel { decoder, last: -1, used: now }),
            Err(_) => return,
        },
    };
    ch.used = now;
    if s.data.is_empty() {
        // The sender stopped talking.
        ch.last = -1;
        ch.decoder.reset();
        mixer.lock().unwrap().end(s.channel);
        shared.talking.lock().unwrap().remove(&s.sender);
        return;
    }
    let mut frames = Vec::new();
    if s.sequence < 0 {
        // A plugin's raw 16 bit little-endian PCM.
        let pcm: Vec<i16> = s.data.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        frames.extend(pcm.chunks(FRAME).map(|c| {
            let mut f = c.to_vec();
            f.resize(FRAME, 0);
            f
        }));
    } else {
        if ch.last >= 0 && s.sequence <= ch.last {
            return;
        }
        let mut lost = if ch.last >= 0 { s.sequence - ch.last - 1 } else { 0 };
        if lost > 5 {
            ch.decoder.reset();
            lost = 0;
        }
        for i in 0..lost {
            frames.push(if i == lost - 1 {
                ch.decoder.decode(Some(&s.data), true)
            } else {
                ch.decoder.decode(None, false)
            });
        }
        frames.push(ch.decoder.decode(Some(&s.data), false));
        ch.last = s.sequence;
    }

    let (volume, listener, audio_type) = {
        let c = shared.controls.lock().unwrap();
        // A sound at a place (a plugin's) is "Other"; a player's has their volume.
        let own_volume = match s.kind {
            SoundKind::Location { .. } => c.other_volume,
            _ => c.player_volumes.get(&s.sender).copied().unwrap_or(1.0),
        };
        (c.volume * own_volume, *shared.listener.lock().unwrap(), c.audio_type)
    };
    if let Some(rec) = shared.recording.lock().unwrap().as_mut() {
        let track = rec.entry(s.sender).or_default();
        for f in &frames {
            track.extend_from_slice(f);
        }
    }
    let gain = match s.kind {
        SoundKind::Group => Some([1.0, 1.0]),
        SoundKind::Entity { .. } if s.sender == own => Some([1.0, 1.0]),
        SoundKind::Entity { distance, .. } => {
            let at = shared.players.lock().unwrap().get(&s.sender).copied();
            at.and_then(|at| spatial(&listener, at, distance))
        }
        SoundKind::Location { at, distance } => spatial(&listener, at, distance),
    };
    let Some(gain) = gain else { return };
    // 3D audio: Reduced pans halfway, OFF not at all (the distance still fades it).
    let mid = (gain[0] + gain[1]) / 2.0;
    let gain = match audio_type {
        1 => [(gain[0] + mid) / 2.0, (gain[1] + mid) / 2.0],
        2 => [mid, mid],
        _ => gain,
    };
    let gain = gain.map(|g| g * volume);
    let mut m = mixer.lock().unwrap();
    for f in frames {
        m.push(s.channel, Frame { pcm: f.iter().map(|&v| f32::from(v) / 32768.0).collect(), gain });
    }
    drop(m);
    if gain.iter().any(|g| *g > 0.0) {
        let whispering = matches!(s.kind, SoundKind::Entity { whispering: true, .. });
        shared.talking.lock().unwrap().insert(s.sender, (now, whispering));
    }
}

/// `MicThread`: each 20 ms frame of the microphone, sent while the push to
/// talk key is held or the voice is loud enough, and a little after.
/// The channel the microphone test plays on.
pub const MIC_TEST_CHANNEL: u128 = u128::MAX - 1;
/// Automatic gain control: the level speech is brought to (RMS, -1..1).
const AGC_TARGET: f32 = 0.1;
/// `VolumeManager.MAX_GAIN`: 24 dB, and as much down.
const AGC_MAX: f32 = 15.85;
const AGC_MIN: f32 = 0.063;
/// rnnoise's chance a frame is speech, above which voice activity is detected.
const VAD_SPEECH: f32 = 0.5;

fn mic_loop(
    socket: &UdpSocket,
    link: &Link,
    shared: &Shared,
    mic: &MicQueue,
    stop: &AtomicBool,
    connected: &AtomicBool,
    mixer: &Mutex<Mixer>,
) {
    let mut encoder = match Encoder::new(link.secret.opus_application(), link.secret.mtu) {
        Ok(e) => e,
        Err(e) => {
            *shared.mic_error.lock().unwrap() = Some(e);
            return;
        }
    };
    let mut sequence: i64 = 0;
    let mut sending = false;
    let mut hold = 0u32;
    let send = |p: &Out| {
        let _ = socket.send(&link.write(p));
    };
    // For testing without a microphone: a steady 440 Hz tone, always sent.
    let tone = std::env::var_os("HORIZON_VOICECHAT_TEST_TONE").is_some();
    if tone {
        log::warn!(target: "horizon_voicechat", "Voice Chat: HORIZON_VOICECHAT_TEST_TONE is set, sending a test tone");
    }
    let mut tone_at: u64 = 0;
    let mut sent: u64 = 0;
    let mut denoiser = crate::denoise::Lazy::default();
    let mut agc_gain = 1.0f32;
    while !stop.load(Ordering::Relaxed) {
        let mut frame: Vec<f32> = if tone {
            std::thread::sleep(Duration::from_millis(20));
            (0..FRAME)
                .map(|_| {
                    tone_at += 1;
                    let t = tone_at as f32 / crate::opus::SAMPLE_RATE as f32;
                    (t * 440.0 * std::f32::consts::TAU).sin() * 0.25
                })
                .collect()
        } else {
            let mut q = mic.samples.lock().unwrap();
            if q.len() < FRAME {
                q = mic.ready.wait_timeout(q, Duration::from_millis(100)).unwrap().0;
            }
            if q.len() < FRAME {
                continue;
            }
            q.drain(..FRAME).collect()
        };
        let c = shared.controls.lock().unwrap().clone();
        // Noise suppression first, on the microphone's own level, as the mod's
        // `MicThread`; rnnoise also judges whether it is speech, for automatic
        // voice activation.
        let auto_vad = c.vad && c.mode == Mode::Voice;
        let mut speech = None;
        if let Ok(f) = <&mut [f32; FRAME]>::try_from(frame.as_mut_slice()) {
            speech = denoiser.apply((c.denoise || auto_vad) && !tone, c.denoise, f);
        }
        // The gain: automatic (brought toward a speaking level, slowly, and only
        // on sound above the noise floor) or the manual one.
        let rms = (frame.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt();
        if c.agc && rms > 0.003 {
            let want = (AGC_TARGET / rms).clamp(AGC_MIN, AGC_MAX);
            let rate = if want < agc_gain { 0.2 } else { 0.02 };
            agc_gain += (want - agc_gain) * rate;
        }
        let gain = if c.agc { agc_gain } else { c.mic_gain };
        let mut pcm = [0i16; FRAME];
        let mut peak = 0f32;
        for (o, s) in pcm.iter_mut().zip(&frame) {
            let v = (s * gain).clamp(-1.0, 1.0);
            peak = peak.max(v.abs());
            *o = (v * 32767.0) as i16;
        }
        // The microphone test: the player hears themselves.
        if c.mic_test {
            let samples = pcm.iter().map(|&v| f32::from(v) / 32768.0).collect();
            mixer.lock().unwrap().push(MIC_TEST_CHANNEL, Frame { pcm: samples, gain: [c.volume, c.volume] });
        }
        let db = if peak > 0.0 { (20.0 * peak.log10()).max(-127.0) } else { -127.0 };
        shared.mic_level.store(db.to_bits(), Ordering::Relaxed);

        let can = connected.load(Ordering::Relaxed) && !c.muted && !c.disabled;
        let mut talk = can
            && (tone
                || match c.mode {
                    Mode::PushToTalk => c.ptt || c.whisper,
                    Mode::Voice => match speech.filter(|_| auto_vad) {
                        Some(p) => p > VAD_SPEECH,
                        None => db > c.threshold_db,
                    },
                });
        if talk {
            hold = if c.mode == Mode::PushToTalk { 5 } else { 25 };
        } else if can && hold > 0 {
            hold -= 1;
            talk = true;
        }
        if talk {
            if let Ok(data) = encoder.encode(&pcm) {
                send(&Out::Mic { data, sequence, whispering: c.whisper });
                sequence += 1;
                sending = true;
                sent += 1;
                if sent == 1 || sent % 250 == 0 {
                    log::info!(target: "horizon_voicechat", "Voice Chat: {sent} microphone packets sent");
                }
            }
        } else if sending {
            send(&Out::Mic { data: Vec::new(), sequence, whispering: false });
            sequence += 1;
            encoder.reset();
            sending = false;
            hold = 0;
        }
        shared.transmitting.store(sending, Ordering::Relaxed);
    }
}
