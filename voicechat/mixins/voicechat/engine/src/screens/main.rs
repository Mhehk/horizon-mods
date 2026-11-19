// The main screen (V), as Simple Voice Chat's `VoiceChatScreen`: 195x76,
// Settings and Group on top, and along the bottom the microphone, voice chat,
// Adjust Volumes, recording (when the server allows it) and hide icons buttons.
use super::{Screen, Ui};
use crate::net::Mode;
use crate::text;
use crate::ui::{Action, View};

pub const WIDTH: f32 = 195.0;
pub const HEIGHT: f32 = 76.0;

pub fn layout(ui: &mut Ui<'_>, view: &View) {
    let [left, top, w, h] = ui.background("gui/gui_voicechat.png", WIDTH, HEIGHT);
    let s = &view.settings;
    let y = top + h - 6.0 - 20.0;

    // `MuteTooltipSupplier.canMuteMic`: no muting in push to talk.
    let can_mute = s.mode == Mode::Voice;
    let mute_tip = if !can_mute {
        text::MUTE_PTT
    } else if s.muted {
        text::MUTE_ENABLED
    } else {
        text::MUTE_DISABLED
    };
    ui.toggle_button([left + 6.0, y, 20.0, 20.0], "microphone_button", s.muted && can_mute, can_mute.then_some(Action::Mute), mute_tip);
    let disable_tip = if s.disabled { text::DISABLE_ENABLED } else { text::DISABLE_DISABLED };
    ui.toggle_button([left + 28.0, y, 20.0, 20.0], "speaker_button", s.disabled, Some(Action::Disable), disable_tip);
    ui.image_button([left + 50.0, y, 20.0, 20.0], "adjust_volumes", Some(Action::Open(Screen::Volumes)), Some(text::ADJUST_VOLUMES));

    if view.allow_recording || view.recording.is_some() {
        let tip = if view.recording.is_some() { text::RECORDING_ENABLED } else { text::RECORDING_DISABLED };
        ui.toggle_button([left + w - 6.0 - 20.0 - 2.0 - 20.0, y, 20.0, 20.0], "record_button", view.recording.is_some(), Some(Action::Record), tip);
    }
    let hide_tip = if s.hide_icons { text::HIDE_ENABLED } else { text::HIDE_DISABLED };
    ui.toggle_button([left + w - 6.0 - 20.0, y, 20.0, 20.0], "hide_button", s.hide_icons, Some(Action::HideIcons), hide_tip);

    ui.button([left + 6.0, top + 6.0 + 15.0, 75.0, 20.0], text::SETTINGS, Some(Action::Open(Screen::Settings)));
    let groups = view.connected && view.groups_enabled;
    ui.button([left + w - 6.0 - 75.0 + 1.0, top + 6.0 + 15.0, 75.0, 20.0], text::GROUP, groups.then_some(Action::Open(Screen::Group)));

    ui.title(text::TITLE, left + w / 2.0, top + 7.0);
    // The recording's length between the buttons, in dark red.
    if let Some(time) = &view.recording {
        let area = [left + 6.0 + 66.0, y, w - ((6.0 + 20.0 + 2.0 + 20.0 + 2.0) * 2.0 + 20.0 + 2.0), 20.0];
        let tw = ui.width(time);
        ui.text(time, [(area[0] + area[2] / 2.0 - tw / 2.0).floor(), area[1] + 6.0], 0xAA_0000, false);
    }
}
