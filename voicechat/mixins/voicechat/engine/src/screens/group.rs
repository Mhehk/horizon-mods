// The group screens, as Simple Voice Chat's: `JoinGroupScreen` (the server's
// groups, a lock on those with a password, their members' faces, Create Group),
// `GroupScreen` (the members of yours, a talking outline, a volume slider on
// the one under the pointer, and the microphone, voice chat, group HUD and
// leave buttons), `CreateGroupScreen` and `EnterPasswordScreen`.
use super::{CELL, HEADER, Screen, UNIT, Ui, head, row, svc, window};
use crate::net::Mode;
use crate::text;
use crate::ui::{Action, Draw, Field, Screens, Slider, View, Widget};

const BG_FILL: u32 = 0xFF4A_4A4A;
const BG_FILL_SELECTED: u32 = 0xFF5A_5A5A;
const NAME_COLOR: u32 = 0xFF_FFFF;
const PADDING: f32 = 4.0;
const SKIN_SIZE: f32 = 12.0;
const GREY: u32 = 0xAA_AAAA;

/// How many 36 high rows fit in `units` of 18.
fn rows_in(units: usize) -> usize {
    ((units as f32 * UNIT) / CELL).floor().max(1.0) as usize
}

/// The magnifier / list mark Simple Voice Chat draws at the list's top left
/// (the 12x12 right of the background in its texture).
fn list_mark(ui: &mut Ui<'_>, texture: &str, rect: [f32; 4]) {
    if crate::svc_assets::available() {
        ui.draws.push(svc(texture, [rect[2], 0.0, 12.0, 12.0], [rect[0] + 10.0, rect[1] + HEADER + 6.0 - 2.0, 12.0, 12.0]));
    }
}

pub fn join(ui: &mut Ui<'_>, state: &Screens, view: &View) {
    let ([left, top, w, h], units) = ui.list_background("gui/gui_join_group.png", 32.0, 0.0);
    let list_top = top + HEADER;
    let rows = rows_in(units);
    let (first, shown) = window(state.scroll, view.groups.len(), rows);
    let mut tip = Vec::new();
    for (i, g) in view.groups.iter().skip(first).take(shown).enumerate() {
        let [x, y, rw, rh] = row(ui.size[0], list_top, i);
        let rect = [x, y, rw, rh];
        let hovered = ui.hover(rect);
        ui.draws.push(Draw::Fill(rect, if hovered { BG_FILL_SELECTED } else { BG_FILL }));
        if g.has_password {
            ui.draws.push(svc("icons/lock.png", [0.0, 0.0, 16.0, 16.0], [x + PADDING, y + rh / 2.0 - 8.0, 16.0, 16.0]));
        }
        let name_x = x + PADDING + if g.has_password { 16.0 + PADDING } else { 0.0 };
        ui.text(&g.name, [name_x, (y + rh / 2.0 - 4.0).floor()], NAME_COLOR, false);
        // The members' faces from the right, two to a column.
        let text_w = ui.width(&g.name) + if g.has_password { 16.0 + PADDING } else { 0.0 };
        let per_row = ((rw - (PADDING + text_w + PADDING + PADDING)) / (SKIN_SIZE + 1.0)).floor().max(0.0) as usize;
        for (m, skin) in g.skins.iter().enumerate() {
            if m >= per_row * 2 {
                break;
            }
            let (col, line) = ((m / 2) as f32, (m % 2) as f32);
            let hx = x + rw - SKIN_SIZE - PADDING - col * (SKIN_SIZE + 1.0);
            let hy = y + (rh / 2.0).floor() - (SKIN_SIZE * 2.0 + 2.0) / 2.0 + ((SKIN_SIZE * 2.0 + 2.0) / 2.0) * line;
            ui.draws.extend(head(skin.as_deref(), [hx, hy, SKIN_SIZE, SKIN_SIZE]));
        }
        if hovered {
            let mut lines = vec![if g.kind == 0 { text::group_title(&g.name) } else { text::group_type_title(&g.name, g.kind) }];
            if g.members.is_empty() {
                lines.push(text::NO_GROUP_MEMBERS.into());
            } else {
                lines.push(text::GROUP_MEMBERS.into());
                for n in g.members.iter().take(10) {
                    lines.push(format!("  {n}"));
                }
                if g.members.len() > 10 {
                    lines.push(text::more_members(g.members.len() - 10));
                }
            }
            tip = lines;
        }
        ui.widgets.push(Widget { rect, action: Action::Join(g.id) });
    }
    if !tip.is_empty() {
        ui.tip([0.0, 0.0, ui.size[0], ui.size[1]], tip);
    }
    if view.groups.is_empty() {
        ui.centred(text::NO_GROUPS, ui.size[0] / 2.0, list_top + (units as f32 * UNIT) / 2.0 - 4.0, GREY, true);
    }
    let usable = view.connected && view.groups_enabled;
    ui.button([left + 7.0, top + h - 20.0 - 7.0, w - 14.0, 20.0], text::CREATE_GROUP, usable.then_some(Action::Open(Screen::CreateGroup)));
    list_mark(ui, "gui/gui_join_group.png", [left, top, w, h]);
    ui.title(text::JOIN_CREATE_GROUP, left + w / 2.0, top + 5.0);
}

pub fn in_group(ui: &mut Ui<'_>, state: &Screens, view: &View) {
    let Some((group, members)) = &view.group else {
        return join(ui, state, view);
    };
    let ([left, top, w, h], units) = ui.list_background("gui/gui_group.png", 32.0, 0.0);
    let list_top = top + HEADER;
    let rows = rows_in(units);
    let (first, shown) = window(state.scroll, members.len(), rows);
    for (i, m) in members.iter().skip(first).take(shown).enumerate() {
        let [x, y, rw, rh] = row(ui.size[0], list_top, i);
        let rect = [x, y, rw, rh];
        ui.draws.push(Draw::Fill(rect, BG_FILL));
        // The face in a 10 unit square scaled to the row, outlined while talking.
        let outline = rh - PADDING * 2.0;
        let k = outline / 10.0;
        let (ox, oy) = (x + PADDING, y + PADDING);
        if !m.disabled && m.talking {
            ui.draws.push(svc("icons/talk_outline.png", [0.0, 0.0, 16.0, 16.0], [ox, oy, outline, outline]));
        }
        ui.draws.extend(head(m.skin.as_deref(), [ox + k, oy + k, 8.0 * k, 8.0 * k]));
        if m.disabled {
            ui.draws.push(svc("icons/speaker_off.png", [0.0, 0.0, 16.0, 16.0], [ox + k, oy + k, 8.0 * k, 8.0 * k]));
        }
        let name_x = x + PADDING + outline + PADDING;
        ui.text(&m.name, [name_x, (y + rh / 2.0 - 4.0).floor()], NAME_COLOR, false);
        if ui.hover(rect) && !m.own {
            let sw = (rw - (PADDING + outline + PADDING + ui.width(&m.name) + PADDING + PADDING)).min(100.0);
            let slider = [x + (rw - sw - PADDING), y + ((rh - 20.0) / 2.0).floor(), sw, 20.0];
            ui.slider(slider, super::volumes::player_value(m.volume), &text::amplification(m.volume), Slider::Player(m.id), Vec::new());
        }
    }
    let by = top + h - 20.0 - 7.0;
    let s = &view.settings;
    let can_mute = s.mode == Mode::Voice;
    let mute_tip = if !can_mute {
        text::MUTE_PTT
    } else if s.muted {
        text::MUTE_ENABLED
    } else {
        text::MUTE_DISABLED
    };
    ui.toggle_button([left + 7.0, by, 20.0, 20.0], "microphone_button", s.muted && can_mute, can_mute.then_some(Action::Mute), mute_tip);
    let disable_tip = if s.disabled { text::DISABLE_ENABLED } else { text::DISABLE_DISABLED };
    ui.toggle_button([left + 7.0 + 23.0, by, 20.0, 20.0], "speaker_button", s.disabled, Some(Action::Disable), disable_tip);
    let hud_tip = if s.hud { text::GROUP_HUD_ENABLED } else { text::GROUP_HUD_DISABLED };
    ui.toggle_button([left + 7.0 + 46.0, by, 20.0, 20.0], "group_hud_button", s.hud, Some(Action::GroupHud), hud_tip);
    ui.image_button([left + w - 20.0 - 7.0, by, 20.0, 20.0], "leave", Some(Action::Leave), Some(text::LEAVE_GROUP));
    list_mark(ui, "gui/gui_group.png", [left, top, w, h]);
    let title = if group.kind == 0 { text::group_title(&group.name) } else { text::group_type_title(&group.name, group.kind) };
    ui.title(&title, left + w / 2.0, top + 5.0);
}

pub fn create(ui: &mut Ui<'_>, state: &Screens) {
    let [left, top, w, h] = ui.background("gui/gui_create_group.png", 195.0, 124.0);
    ui.title(text::CREATE_GROUP, left + w / 2.0, top + 7.0);
    ui.text(text::GROUP_NAME, [left + 8.0, top + 7.0 + 9.0 + 5.0], super::FONT_COLOR, false);
    ui.field([left + 7.0, top + 30.0, w - 14.0, 14.0], &state.name, "", false, state.focus == Some(Field::Name), Action::Focus(Field::Name));
    ui.text(text::OPTIONAL_PASSWORD, [left + 8.0, top + 7.0 + 14.0 * 2.0 + 12.0], super::FONT_COLOR, false);
    ui.field([left + 7.0, top + 56.0, w - 14.0, 14.0], &state.password, "", false, state.focus == Some(Field::Password), Action::Focus(Field::Password));
    let kind = [left + 6.0, top + 74.0, w - 12.0, 20.0];
    ui.button(kind, &text::group_type(state.kind), Some(Action::Kind));
    ui.tip(kind, vec![text::GROUP_TYPE_DESCRIPTIONS[state.kind.rem_euclid(3) as usize].into()]);
    let ok = !state.name.trim().is_empty();
    ui.button([left + 6.0, top + h - 27.0, w - 12.0, 20.0], text::CREATE, ok.then_some(Action::Create));
    if let Some((m, rgb)) = &state.message {
        let m = ui.fit(m, ui.size[0] - 8.0);
        ui.centred(&m, left + w / 2.0, top + h + 6.0, *rgb, true);
    }
}

pub fn password(ui: &mut Ui<'_>, state: &Screens) {
    let [left, top, w, h] = ui.background("gui/gui_enter_password.png", 195.0, 74.0);
    ui.title(text::ENTER_GROUP_PASSWORD, left + w / 2.0, top + 7.0);
    ui.text(text::PASSWORD, [left + 8.0, top + 7.0 + 9.0 + 5.0], super::FONT_COLOR, false);
    let focus = state.focus == Some(Field::JoinPassword);
    ui.field([left + 7.0, top + 7.0 + 14.0 * 2.0 - 5.0, w - 14.0, 14.0], &state.join_password, "", false, focus, Action::Focus(Field::JoinPassword));
    let ok = !state.join_password.is_empty();
    ui.button([left + 7.0, top + h - 20.0 - 7.0, w - 14.0, 20.0], text::JOIN_GROUP, ok.then_some(Action::JoinWithPassword));
    if let Some((m, rgb)) = &state.message {
        let m = ui.fit(m, ui.size[0] - 8.0);
        ui.centred(&m, left + w / 2.0, top + h + 6.0, *rgb, true);
    }
}
