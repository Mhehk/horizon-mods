// The settings screen, as Simple Voice Chat's `VoiceChatSettingsScreen`
// (248x219): the voice chat volume, the gain (automatic, or a manual gain in
// dB), noise suppression, the microphone test and the activation method, then
// the push to talk key or voice activation (automatic or by a threshold with a
// live level meter), 3D audio, and the microphone and speaker.
use super::{DeviceKind, Screen, Ui};
use crate::net::Mode;
use crate::skin;
use crate::text;
use crate::ui::{Action, Draw, Screens, Slider, View};

pub const WIDTH: f32 = 248.0;
pub const HEIGHT: f32 = 219.0;
/// `voice_chat_volume`: 0 to 300 %.
pub const VOLUME_MAX: f32 = 300.0;
/// `VolumeManager.MIN_GAIN` / `MAX_GAIN`, in dB.
pub const GAIN_MIN: f32 = -40.0;
pub const GAIN_MAX: f32 = 24.0;
/// `AudioUtils.LOWEST_DB`.
pub const THRESHOLD_MIN: f32 = -127.0;

/// A slider's place (0..=1) for a value, and back.
pub fn volume_value(percent: u32) -> f32 {
    percent as f32 / VOLUME_MAX
}
pub fn gain_value(db: f32) -> f32 {
    (db - GAIN_MIN) / (GAIN_MAX - GAIN_MIN)
}
pub fn threshold_value(db: i32) -> f32 {
    (db as f32 - THRESHOLD_MIN) / -THRESHOLD_MIN
}
pub fn volume_of(v: f32) -> u32 {
    (v.clamp(0.0, 1.0) * VOLUME_MAX).round() as u32
}
pub fn gain_of(v: f32) -> f32 {
    (v.clamp(0.0, 1.0) * (GAIN_MAX - GAIN_MIN) + GAIN_MIN).round()
}
pub fn threshold_of(v: f32) -> i32 {
    (THRESHOLD_MIN + v.clamp(0.0, 1.0) * -THRESHOLD_MIN).round() as i32
}

/// The microphone's level (`level_db`, -127 to 0) inside a slider's track
/// at `rect`: bright past the threshold, as Simple Voice Chat's meter.
pub fn level_meter(rect: [f32; 4], level_db: f32, threshold_db: i32) -> Vec<Draw> {
    let [x, y, w, h] = rect;
    let part = ((level_db.clamp(THRESHOLD_MIN, 0.0) - THRESHOLD_MIN) / -THRESHOLD_MIN) * (w - 2.0);
    let loud = level_db > threshold_db as f32;
    vec![Draw::Fill([x + 1.0, y + 1.0, part, h - 2.0], if loud { 0xC040_C040 } else { 0x8030_6030 })]
}

/// A slider drawn inactive (dark, no handle to drag), with its label.
fn inactive_slider(ui: &mut Ui<'_>, rect: [f32; 4], label: &str) {
    ui.draws.extend(skin::nine_slice(&skin::BUTTON_DISABLED, rect));
    let label = ui.fit(label, rect[2] - 8.0);
    let w = ui.width(&label);
    ui.text(&label, [(rect[0] + (rect[2] - w) / 2.0).floor(), rect[1] + 6.0], skin::LABEL_DISABLED, true);
}

pub fn layout(ui: &mut Ui<'_>, state: &Screens, view: &View) {
    let s = &view.settings;
    let [left, top, w, _] = ui.background("gui/gui_voicechat_settings.png", WIDTH, HEIGHT);
    let full = w - 20.0;
    let mut y = top + 20.0;

    ui.slider([left + 10.0, y, full, 20.0], volume_value(s.volume), &text::voice_chat_volume(s.volume), Slider::Volume, Vec::new());
    y += 21.0;

    // The gain: Auto beside a manual gain that only Manual can move.
    let agc = [left + 10.0, y, 80.0, 20.0];
    ui.button(agc, &text::gain(s.agc), Some(Action::Agc));
    if !s.agc {
        ui.tip(agc, vec![text::GAIN_MANUAL_WARNING.into()]);
    }
    let gain = [left + 10.0 + 81.0, y, full - 80.0 - 1.0, 20.0];
    if s.agc {
        inactive_slider(ui, gain, &text::microphone_gain(s.gain_db));
    } else {
        ui.slider(gain, gain_value(s.gain_db), &text::microphone_gain(s.gain_db), Slider::Gain, Vec::new());
        if s.gain_db > 10.0 {
            ui.tip(gain, vec![text::GAIN_WARNING.into()]);
        }
    }
    y += 21.0;

    let denoise = [left + 10.0, y, full, 20.0];
    match &view.denoise {
        Ok(()) => ui.button(denoise, &text::denoiser(s.denoise), Some(Action::Denoise)),
        Err(why) => {
            ui.button(denoise, &text::denoiser(false), None);
            ui.tip(denoise, vec![why.clone()]);
        }
    }
    y += 21.0;

    // The microphone test beside the activation method.
    let test_tip = if !view.connected {
        text::MIC_TEST_UNAVAILABLE
    } else if view.mic_test {
        text::MIC_TEST_ENABLED
    } else {
        text::MIC_TEST_DISABLED
    };
    ui.toggle_button([left + 10.0, y, 20.0, 20.0], "microphone_button", !view.mic_test, view.connected.then_some(Action::MicTest), test_tip);
    let voice = s.mode == Mode::Voice;
    ui.button([left + 10.0 + 20.0 + 1.0, y, full - 20.0 - 1.0, 20.0], &text::activation(voice), Some(Action::Mode));
    let row1 = [left + 10.0, y + 21.0, full, 20.0];
    let row2 = [left + 10.0, y + 42.0, full, 20.0];
    if voice {
        ui.button(row1, &text::vad(s.vad), Some(Action::Vad));
        let meter = level_meter(row2, view.level_db, s.threshold);
        if s.vad {
            inactive_slider(ui, row2, &text::threshold(s.threshold));
        } else {
            ui.slider(row2, threshold_value(s.threshold), &text::threshold(s.threshold), Slider::Threshold, meter);
            if s.threshold >= 0 {
                ui.tip(row2, vec![text::VOICE_ACTIVATION_OFF.into()]);
            }
        }
    } else {
        let key = if state.listening { format!("> {} <", view.keys.ptt) } else { view.keys.ptt.clone() };
        ui.button(row1, &text::ptt_key(&key), Some(Action::BindPtt));
        ui.tip(row1, vec![text::PRESS_TO_ASSIGN.into()]);
    }
    y += 21.0 * 3.0;

    let audio = text::AUDIO_TYPES[s.audio_type.min(2) as usize];
    ui.button([left + 10.0, y, full, 20.0], &text::audio_type(audio), Some(Action::AudioType));
    y += 21.0;
    let half = (full / 2.0 - 1.0).floor();
    ui.button([left + 10.0, y, half, 20.0], text::SELECT_MICROPHONE, Some(Action::Open(Screen::Devices(DeviceKind::Input))));
    ui.button([left + w / 2.0 + 1.0, y, half, 20.0], text::SELECT_SPEAKER, Some(Action::Open(Screen::Devices(DeviceKind::Output))));

    ui.title(text::SETTINGS_TITLE, left + w / 2.0, top + 7.0);
}
