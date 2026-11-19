// Choosing the microphone or the speaker, as Simple Voice Chat's
// `SelectMicrophoneScreen` / `SelectSpeakerScreen`: System Default and each
// device by name, the chosen one darker with a tick, and Back.
use super::{DeviceKind, HEADER, UNIT, CELL, Ui, row, svc, window};
use crate::text;
use crate::ui::{Action, Draw, Screens, View, Widget};

const PADDING: f32 = 4.0;
const BG_FILL: u32 = 0xFF4A_4A4A;
const BG_FILL_HOVERED: u32 = 0xFF5A_5A5A;
const BG_FILL_SELECTED: u32 = 0xFF28_2828;

pub fn layout(ui: &mut Ui<'_>, state: &Screens, view: &View, kind: DeviceKind) {
    let ([left, top, w, h], units) = ui.list_background("gui/gui_audio_devices.png", 32.0, 0.0);
    let (title, icon, empty) = match kind {
        DeviceKind::Input => (text::SELECT_MICROPHONE, "icons/microphone.png", text::NO_MICROPHONE),
        DeviceKind::Output => (text::SELECT_SPEAKER, "icons/speaker.png", text::NO_SPEAKER),
    };
    let list_top = top + HEADER;
    let rows = (((units as f32 * UNIT) / CELL).floor().max(1.0)) as usize;
    let chosen = match kind {
        DeviceKind::Input => view.settings.mic_device.as_deref(),
        DeviceKind::Output => view.settings.speaker_device.as_deref(),
    };
    // A device chosen before but gone now is not listed: the default is in use.
    let chosen = chosen.filter(|c| view.devices.iter().any(|d| d == c));
    let entries: Vec<Option<&str>> = std::iter::once(None).chain(view.devices.iter().map(|d| Some(d.as_str()))).collect();
    let (first, shown) = window(state.scroll, entries.len(), rows);
    for (i, entry) in entries.iter().skip(first).take(shown).enumerate() {
        let [x, y, rw, rh] = row(ui.size[0], list_top, i);
        let rect = [x, y, rw, rh];
        let selected = *entry == chosen;
        let fill = if selected {
            BG_FILL_SELECTED
        } else if ui.hover(rect) {
            BG_FILL_HOVERED
        } else {
            BG_FILL
        };
        ui.draws.push(Draw::Fill(rect, fill));
        let at = [x + PADDING, (y + rh / 2.0 - 8.0).floor(), 16.0, 16.0];
        ui.draws.push(svc(icon, [0.0, 0.0, 16.0, 16.0], at));
        if selected {
            ui.draws.push(svc("icons/device_selected.png", [0.0, 0.0, 16.0, 16.0], at));
        }
        let label = entry.unwrap_or(text::DEFAULT_DEVICE);
        let space = rw - PADDING - 16.0 - PADDING - PADDING;
        let shown_label = ui.fit(label, space);
        ui.text(&shown_label, [x + PADDING + 16.0 + PADDING, (y + rh / 2.0 - 4.0).floor()], 0xFF_FFFF, false);
        if shown_label != label {
            ui.tip(rect, vec![label.to_owned()]);
        }
        ui.widgets.push(Widget { rect, action: Action::Device(kind, entry.map(str::to_owned)) });
    }
    if view.devices.is_empty() && entries.len() <= 1 {
        ui.centred(empty, ui.size[0] / 2.0, list_top + (units as f32 * UNIT) / 2.0 + CELL / 2.0, 0xFF_FFFF, true);
    }
    ui.back([left + 7.0, top + h - 20.0 - 7.0, w - 14.0, 20.0], text::BACK);
    if crate::svc_assets::available() {
        ui.draws.push(svc("gui/gui_audio_devices.png", [w, 0.0, 12.0, 12.0], [left + 10.0, top + HEADER + 6.0 - 2.0, 12.0, 12.0]));
    }
    ui.title(title, left + w / 2.0, top + 5.0);
}
