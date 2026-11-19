// Adjust Volumes, as Simple Voice Chat's `AdjustVolumesScreen`: a search box,
// then "Other" (sounds no player makes) and each player with voice chat: a
// 24 pixel face, the name and a 100 wide slider, 0 to 400 % (`Amplification`).
use super::{CELL, HEADER, UNIT, Ui, head, row, svc, window};
use crate::text;
use crate::ui::{Action, Draw, Field, Screens, Slider, View};

pub const PLAYER_MAX: f32 = 400.0;
const SEARCH_HEIGHT: f32 = 16.0;
const SKIN_SIZE: f32 = 24.0;
const PADDING: f32 = 4.0;
const SLIDER_WIDTH: f32 = 100.0;
const BG_FILL: u32 = 0xFF4A_4A4A;

pub fn player_value(percent: u32) -> f32 {
    percent as f32 / PLAYER_MAX
}

pub fn player_of(v: f32) -> u32 {
    (v.clamp(0.0, 1.0) * PLAYER_MAX).round() as u32
}

pub fn layout(ui: &mut Ui<'_>, state: &Screens, view: &View) {
    let ([left, top, w, _], units) = ui.list_background("gui/gui_volumes.png", 8.0, SEARCH_HEIGHT);
    let list_top = top + HEADER + SEARCH_HEIGHT;
    let list_h = units as f32 * UNIT - SEARCH_HEIGHT;
    let rows = ((list_h / CELL).floor().max(1.0)) as usize;

    let search = state.search.to_lowercase();
    // "Other" first, then the players the search finds.
    let mut entries: Vec<Option<&crate::ui::PlayerRow>> = Vec::new();
    if search.is_empty() {
        entries.push(None);
    }
    entries.extend(view.players.iter().filter(|p| p.name.to_lowercase().contains(&search)).map(Some));
    let (first, shown) = window(state.scroll, entries.len(), rows);
    for (i, e) in entries.iter().skip(first).take(shown).enumerate() {
        let [x, y, rw, rh] = row(ui.size[0], list_top, i);
        ui.draws.push(Draw::Fill([x, y, rw, rh], BG_FILL));
        let skin_at = [x + PADDING, y + ((rh - SKIN_SIZE) / 2.0).floor(), SKIN_SIZE, SKIN_SIZE];
        let text_x = skin_at[0] + SKIN_SIZE + PADDING;
        let text_y = (y + (rh - 8.0) / 2.0).floor();
        let space = rw - PADDING - SKIN_SIZE - PADDING - PADDING - SLIDER_WIDTH - PADDING;
        let (name, value, slider) = match e {
            Some(p) => {
                ui.draws.extend(head(p.skin.as_deref(), skin_at));
                (p.name.as_str(), p.volume, Slider::Player(p.id))
            }
            None => {
                ui.draws.push(svc("icons/other_volume.png", [0.0, 0.0, 16.0, 16.0], skin_at));
                ui.tip([x, y, rw - SLIDER_WIDTH - PADDING * 2.0, rh], vec![text::OTHER_VOLUME_DESCRIPTION.into()]);
                (text::OTHER_VOLUME, view.other_volume, Slider::Other)
            }
        };
        let shown_name = ui.fit(name, space);
        ui.text(&shown_name, [text_x, text_y], 0xFF_FFFF, false);
        let srect = [x + (rw - SLIDER_WIDTH - PADDING), y + ((rh - 20.0) / 2.0).floor(), SLIDER_WIDTH, 20.0];
        ui.slider(srect, player_value(value), &text::amplification(value), slider, Vec::new());
    }
    if entries.is_empty() {
        ui.centred(text::SEARCH_EMPTY, ui.size[0] / 2.0, top + HEADER + (units as f32 * UNIT) / 2.0 - 4.0, 0xAA_AAAA, true);
    }
    // The search box, with its hint in grey while empty.
    if crate::svc_assets::available() {
        ui.draws.push(svc("gui/gui_volumes.png", [w, 0.0, 12.0, 12.0], [left + 10.0, top + HEADER + 6.0 - 2.0, 12.0, 12.0]));
    }
    let field = [left + 28.0, top + HEADER + 6.0, 196.0, SEARCH_HEIGHT];
    let focused = state.focus == Some(Field::Search);
    if state.search.is_empty() && !focused {
        ui.text(text::SEARCH_HINT, [field[0], field[1]], 0xAA_AAAA, false);
        ui.widgets.push(crate::ui::Widget { rect: field, action: Action::Focus(Field::Search) });
    } else {
        let caret = if focused { "_" } else { "" };
        let t = format!("{}{caret}", state.search);
        ui.text(&t, [field[0], field[1]], 0xFF_FFFF, true);
        ui.widgets.push(crate::ui::Widget { rect: field, action: Action::Focus(Field::Search) });
    }
    ui.title(text::ADJUST_VOLUMES, ui.size[0] / 2.0, top + 5.0);
}
