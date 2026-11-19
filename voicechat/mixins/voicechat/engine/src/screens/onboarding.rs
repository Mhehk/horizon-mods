// The first-time setup guide, as Simple Voice Chat's onboarding screens: on
// half the screen's width from 20 down, a bold title, wrapped text, and Back /
// Next at the bottom. Introduction (or skip it), the microphone, the speaker,
// the activation method, then the push to talk key or voice activation, and
// the end, which says the keys to use.
use super::{CELL, DeviceKind, Screen, Ui, row, svc, window};
use crate::net::Mode;
use crate::text;
use crate::ui::{Action, Draw, Screens, Slider, View, Widget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Introduction,
    Skip,
    Microphone,
    Speaker,
    Activation,
    PushToTalk,
    Voice,
    Final,
}

const PADDING: f32 = 8.0;
const BUTTON: f32 = 20.0;
const WHITE: u32 = 0xFF_FFFF;

struct Frame {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

fn frame(ui: &Ui<'_>) -> Frame {
    let width = (ui.size[0] / 2.0).floor();
    let top = 20.0;
    Frame { left: ((ui.size[0] - width) / 2.0).floor(), top, width, height: ui.size[1] - top * 2.0 }
}

/// Next (or `next`'s label) on the right half of the bottom row.
fn next_button(ui: &mut Ui<'_>, f: &Frame, label: &str, action: Action) {
    ui.button([f.left + f.width / 2.0 + PADDING / 2.0, f.top + f.height - BUTTON, f.width / 2.0 - PADDING / 2.0, BUTTON], label, Some(action));
}

/// Back (or Cancel), half wide beside Next or the whole row (`big`).
fn back_button(ui: &mut Ui<'_>, f: &Frame, label: &str, big: bool) {
    let w = if big { f.width } else { f.width / 2.0 - PADDING / 2.0 };
    ui.back([f.left, f.top + f.height - BUTTON, w, BUTTON], label);
}

fn title_and_text(ui: &mut Ui<'_>, f: &Frame, title: &str, body: &str) {
    ui.centred(title, ui.size[0] / 2.0, f.top, WHITE, true);
    // Bold: drawn twice a pixel apart, as a bold font is.
    let w = ui.width(title);
    ui.text(title, [(ui.size[0] / 2.0 - w / 2.0).floor() + 1.0, f.top], WHITE, false);
    if !body.is_empty() {
        ui.wrapped(body, ui.size[0] / 2.0, f.top + 9.0 + 20.0, f.width, WHITE, true);
    }
}

fn devices(ui: &mut Ui<'_>, state: &Screens, view: &View, f: &Frame, kind: DeviceKind) {
    let top = f.top + 9.0 + PADDING;
    let height = f.height - 9.0 - BUTTON - PADDING * 2.0;
    let rows = ((height / CELL).floor().max(1.0)) as usize;
    let chosen = match kind {
        DeviceKind::Input => view.settings.mic_device.as_deref(),
        DeviceKind::Output => view.settings.speaker_device.as_deref(),
    }
    .filter(|c| view.devices.iter().any(|d| d == c));
    let icon = if kind == DeviceKind::Input { "icons/microphone.png" } else { "icons/speaker.png" };
    let entries: Vec<Option<&str>> = std::iter::once(None).chain(view.devices.iter().map(|d| Some(d.as_str()))).collect();
    let (first, shown) = window(state.scroll, entries.len(), rows);
    for (i, e) in entries.iter().skip(first).take(shown).enumerate() {
        let [x, y, rw, rh] = row(ui.size[0], top, i);
        let rect = [x, y, rw, rh];
        let selected = *e == chosen;
        let fill = if selected { 0xFF28_2828 } else if ui.hover(rect) { 0xFF5A_5A5A } else { 0xFF4A_4A4A };
        ui.draws.push(Draw::Fill(rect, fill));
        let at = [x + 4.0, (y + rh / 2.0 - 8.0).floor(), 16.0, 16.0];
        ui.draws.push(svc(icon, [0.0, 0.0, 16.0, 16.0], at));
        if selected {
            ui.draws.push(svc("icons/device_selected.png", [0.0, 0.0, 16.0, 16.0], at));
        }
        let label = e.unwrap_or(text::DEFAULT_DEVICE);
        let label = ui.fit(label, rw - 28.0);
        ui.text(&label, [x + 24.0, (y + rh / 2.0 - 4.0).floor()], WHITE, false);
        ui.widgets.push(Widget { rect, action: Action::Device(kind, e.map(str::to_owned)) });
    }
}

pub fn layout(ui: &mut Ui<'_>, state: &Screens, view: &View, step: Step) {
    let f = frame(ui);
    match step {
        Step::Introduction => {
            title_and_text(ui, &f, text::ONBOARD_TITLE, text::ONBOARD_INTRO);
            ui.button(
                [f.left, f.top + f.height - BUTTON * 2.0 - PADDING, f.width, BUTTON],
                text::ONBOARD_SKIP,
                Some(Action::Open(Screen::Onboarding(Step::Skip))),
            );
            next_button(ui, &f, text::ONBOARD_NEXT, Action::Open(Screen::Onboarding(Step::Microphone)));
            back_button(ui, &f, text::ONBOARD_CANCEL, false);
        }
        Step::Skip => {
            title_and_text(ui, &f, text::ONBOARD_SKIP_TITLE, text::ONBOARD_SKIP_DESCRIPTION);
            next_button(ui, &f, text::ONBOARD_CONFIRM, Action::FinishOnboarding);
            back_button(ui, &f, text::ONBOARD_BACK, false);
        }
        Step::Microphone | Step::Speaker => {
            let (title, kind, next) = if step == Step::Microphone {
                (text::ONBOARD_MICROPHONE, DeviceKind::Input, Step::Speaker)
            } else {
                (text::ONBOARD_SPEAKER, DeviceKind::Output, Step::Activation)
            };
            devices(ui, state, view, &f, kind);
            title_and_text(ui, &f, title, "");
            next_button(ui, &f, text::ONBOARD_NEXT, Action::Open(Screen::Onboarding(next)));
            back_button(ui, &f, text::ONBOARD_BACK, false);
        }
        Step::Activation => {
            title_and_text(ui, &f, text::ONBOARD_ACTIVATION_TITLE, text::ONBOARD_ACTIVATION);
            let y = f.top + f.height - BUTTON * 2.0 - PADDING;
            let half = f.width / 2.0 - PADDING / 2.0;
            ui.button([f.left, y, half, BUTTON], text::PUSH_TO_TALK, Some(Action::ChooseMode(false)));
            ui.button([f.left + f.width / 2.0 + PADDING / 2.0, y, half, BUTTON], text::VOICE_ACTIVATION, Some(Action::ChooseMode(true)));
            back_button(ui, &f, text::ONBOARD_BACK, true);
        }
        Step::PushToTalk => {
            title_and_text(ui, &f, text::ONBOARD_PTT_TITLE, text::ONBOARD_PTT);
            let pos = f.top + f.height - BUTTON * 3.0 - PADDING * 2.0 - 40.0;
            ui.centred(text::ONBOARD_PTT_BUTTON, ui.size[0] / 2.0, pos - 9.0 - PADDING, WHITE, true);
            let key = if state.listening { format!("> {} <", view.keys.ptt) } else { view.keys.ptt.clone() };
            ui.button([f.left + 40.0, pos, f.width - 80.0, BUTTON], &key, Some(Action::BindPtt));
            next_button(ui, &f, text::ONBOARD_NEXT, Action::Open(Screen::Onboarding(Step::Final)));
            back_button(ui, &f, text::ONBOARD_BACK, false);
        }
        Step::Voice => {
            title_and_text(ui, &f, text::ONBOARD_VOICE_TITLE, text::ONBOARD_VOICE);
            let s = &view.settings;
            let y = f.top + f.height - BUTTON * 3.0 - PADDING * 2.0 - 40.0;
            let test_tip = if view.mic_test { text::MIC_TEST_ENABLED } else { text::MIC_TEST_DISABLED };
            ui.toggle_button([f.left, y, 20.0, 20.0], "microphone_button", !view.mic_test, view.connected.then_some(Action::MicTest), test_tip);
            ui.button([f.left + 21.0, y, f.width - 21.0, BUTTON], &text::vad(s.vad), Some(Action::Vad));
            let slider = [f.left, y + 21.0, f.width, BUTTON];
            if !s.vad {
                let meter = super::settings::level_meter(slider, view.level_db, s.threshold);
                ui.slider(slider, super::settings::threshold_value(s.threshold), &text::threshold(s.threshold), Slider::Threshold, meter);
            }
            next_button(ui, &f, text::ONBOARD_NEXT, Action::Open(Screen::Onboarding(Step::Final)));
            back_button(ui, &f, text::ONBOARD_BACK, false);
        }
        Step::Final => {
            let ptt = (view.settings.mode == Mode::PushToTalk).then_some(view.keys.ptt.as_str());
            let body = text::onboard_final(&view.keys.menu, ptt, &view.keys.mute);
            title_and_text(ui, &f, text::ONBOARD_FINAL_TITLE, &body);
            ui.button([f.left, f.top + f.height - BUTTON, f.width, BUTTON], text::ONBOARD_FINISH, Some(Action::FinishOnboarding));
        }
    }
}
