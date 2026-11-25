// Eksen — 16.10.2 (mihehk): the editor screen, opened with Right Shift, as
// Axiom's documentation describes its editor mode ("Editor UI"): the mouse
// is free, the tools sit on the left, the active tool's options on the right,
// the capabilities and flight speed on their own tab, and a click in the
// world (the middle of the screen) uses the active tool where the pointer
// points. Written from scratch; no code, names or images of Axiom.
//
// Every button runs one of the `/eksen` commands (state.rs, editor.rs), so
// the screen and the chat commands can never disagree. This module is plain
// Rust: it lays out what to draw, says what a click hit, and edits the text
// fields; the hooks (screen.rs, mouse.rs …) do the drawing and the running.
use super::eksen as st;
use super::eksen_abilities as ab;
use super::eksen_core::{Blocks, Pos};

/// Something to draw, in GUI pixels: a fill ([x, y, w, h], ARGB) or a text
/// (at, RGB, shadow).
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Fill([f32; 4], u32),
    Text(String, [f32; 2], u32, bool),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Tools,
    Builder,
    Abilities,
    Select,
}

const TABS: [(Tab, &str); 4] = [
    (Tab::Tools, "Araçlar"),
    (Tab::Builder, "Yapı"),
    (Tab::Abilities, "Yetenekler"),
    (Tab::Select, "Seçim"),
];

/// A text field: what Enter sends it to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field {
    /// `/eksen blok …`
    Block,
    /// `/eksen metin …`
    Text,
    /// `/eksen ayar <option> …` (the active tool's option)
    Option(String),
    /// `/eksen ucus <setting> …`
    Flight(&'static str),
    /// `/eksen sihirli <option> …`
    Magic(&'static str),
}

impl Field {
    /// The command Enter runs with `text` typed.
    pub fn command(&self, text: &str) -> Vec<String> {
        let text = text.trim();
        let mut words: Vec<String> = match self {
            Field::Block => vec!["blok".into()],
            Field::Text => vec!["metin".into()],
            Field::Option(o) => vec!["ayar".into(), o.clone()],
            Field::Flight(s) => vec!["ucus".into(), (*s).into()],
            Field::Magic(o) => vec!["sihirli".into(), (*o).into()],
        };
        if *self == Field::Text {
            words.extend(text.split_whitespace().map(str::to_owned));
        } else if !text.is_empty() {
            words.push(text.split_whitespace().collect::<Vec<_>>().join("_"));
        }
        words
    }
}

/// What a click at a point hit.
#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    /// Run `/eksen <words>`.
    Run(Vec<String>),
    Tab(Tab),
    /// Start typing in a field, with its current text.
    Focus(Field, String),
    /// The world between the panels: the active tool.
    World,
    /// A panel's background or a label.
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Panel,
    Bar,
    Button,
    Selected,
    On,
    Off,
    Label,
    Dim,
    Field,
    Focused,
}

/// A laid-out element: where, what a click does, its text and look.
#[derive(Clone, Debug, PartialEq)]
struct Item {
    rect: [f32; 4],
    hit: Hit,
    text: String,
    style: Style,
}

// Sizes in GUI pixels (vanilla's font is 9 high).
const BAR: f32 = 14.0;
const ROW: f32 = 11.0;
const LEFT: f32 = 128.0;
const RIGHT: f32 = 136.0;
const BOTTOM: f32 = 26.0;
const PAD: f32 = 4.0;

// Colours (ARGB fills, RGB text).
const PANEL: u32 = 0xD0_10_12_18;
const BARC: u32 = 0xE8_1C_1F_28;
const BUTTON: u32 = 0x60_30_34_40;
const HOVER: u32 = 0x50_FF_FF_FF;
const SELECTED: u32 = 0xFF_2F_6F_B5;
const FIELD: u32 = 0xFF_08_09_0C;
const FOCUS: u32 = 0xFF_5A_9B_E6;
const TEXT: u32 = 0xFF_FF_FF;
const DIM: u32 = 0x9A_A0_AA;
const ON: u32 = 0x7B_E0_7B;
const OFF: u32 = 0x8A_8A_8A;

/// The screen's own state.
#[derive(Default)]
pub struct Ui {
    /// The screen is up (kept in step with the game by frame.rs).
    pub open: bool,
    /// Asked to close (the editor key again); frame.rs closes it.
    pub close_asked: bool,
    pub tab: Tab,
    /// The left list scrolled by this many rows.
    scroll: usize,
    /// The field being typed in and its text.
    pub field: Option<Field>,
    pub buffer: String,
    /// The mouse button held in the world (0 left, 1 right), if any.
    pub world_button: Option<i32>,
    /// The block under the pointer and its face, as the last frame found it.
    pub hover: Option<(Pos, Pos)>,
    /// The screen's size as the last frame drew it.
    pub size: [f32; 2],
    /// The last lines the commands answered, shown at the bottom.
    pub status: Vec<String>,
    /// Lines for the chat too (frame.rs sends them).
    pub chat: Vec<String>,
}

impl Ui {
    /// Lines a command answered: the last two show on the screen, all go
    /// to the chat.
    pub fn say(&mut self, lines: Vec<String>) {
        for l in &lines {
            self.status.push(l.trim_start_matches(st::PREFIX).to_owned());
        }
        let n = self.status.len();
        if n > 2 {
            self.status.drain(..n - 2);
        }
        self.chat.extend(lines);
    }

    /// A click (left button) at `at`: changes tabs and fields here and
    /// answers what else to do.
    /// What is at `at`, without clicking it: the front-most element, or the
    /// world when no panel is there.
    pub fn hit_at(&self, view: &View, at: [f32; 2], measure: &dyn Fn(&str) -> f32) -> Hit {
        items(self, view, self.size, measure)
            .into_iter()
            .rev()
            .find(|i| inside(i.rect, at))
            .map_or(Hit::World, |i| i.hit)
    }

    pub fn click(&mut self, view: &View, at: [f32; 2], measure: &dyn Fn(&str) -> f32) -> Hit {
        let hit = self.hit_at(view, at, measure);
        match &hit {
            Hit::Tab(t) => {
                self.tab = *t;
                self.scroll = 0;
                self.field = None;
            }
            Hit::Focus(f, text) => {
                self.field = Some(f.clone());
                self.buffer = text.clone();
            }
            _ => self.field = None,
        }
        hit
    }

    /// The wheel over the left list.
    pub fn scroll(&mut self, steps: f64) {
        if steps < 0.0 {
            self.scroll += 2;
        } else if steps > 0.0 {
            self.scroll = self.scroll.saturating_sub(2);
        }
    }

    /// A key while typing: Enter answers the command to run, Escape stops
    /// typing. True when the field took the key.
    pub fn field_key(&mut self, key: &str) -> (bool, Option<Vec<String>>) {
        let Some(field) = self.field.clone() else {
            return (false, None);
        };
        match key {
            "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
                self.field = None;
                (true, Some(field.command(&std::mem::take(&mut self.buffer))))
            }
            "key.keyboard.escape" => {
                self.field = None;
                (true, None)
            }
            "key.keyboard.backspace" => {
                self.buffer.pop();
                (true, None)
            }
            _ => (true, None),
        }
    }

    /// A character typed into the field.
    pub fn char(&mut self, c: char) {
        if self.field.is_some() && !c.is_control() && self.buffer.chars().count() < 256 {
            self.buffer.push(c);
        }
    }

    /// What to draw, with `mouse` for the hover highlight.
    pub fn draw(&self, view: &View, mouse: [f32; 2], measure: &dyn Fn(&str) -> f32) -> Vec<Draw> {
        let mut out = Vec::new();
        for i in items(self, view, self.size, measure) {
            let [x, y, w, h] = i.rect;
            let hovered = inside(i.rect, mouse) && !matches!(i.hit, Hit::Nothing | Hit::World);
            let fill = match i.style {
                Style::Panel => Some(PANEL),
                Style::Bar => Some(BARC),
                Style::Selected => Some(SELECTED),
                Style::Field => Some(FIELD),
                Style::Focused => Some(FOCUS),
                Style::Button | Style::On | Style::Off => Some(BUTTON),
                Style::Label | Style::Dim => None,
            };
            if let Some(c) = fill {
                out.push(Draw::Fill(i.rect, c));
                if i.style == Style::Focused {
                    out.push(Draw::Fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], FIELD));
                }
            }
            if hovered {
                out.push(Draw::Fill(i.rect, HOVER));
            }
            if !i.text.is_empty() {
                let rgb = match i.style {
                    Style::On => ON,
                    Style::Off | Style::Dim => if i.style == Style::Off { OFF } else { DIM },
                    _ => TEXT,
                };
                let text = fit(&i.text, w - 2.0 * PAD, measure);
                out.push(Draw::Text(text, [x + PAD, y + (h - 8.0) / 2.0], rgb, true));
            }
        }
        out
    }
}

fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}

/// `text` cut to `width` with an ellipsis.
fn fit(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> String {
    if measure(text) <= width {
        return text.to_owned();
    }
    let mut s: String = text.to_owned();
    while !s.is_empty() && measure(&format!("{s}…")) > width {
        s.pop();
    }
    format!("{s}…")
}

/// What the screen shows, read from the editor's state once a frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub tools: Vec<(&'static str, &'static str)>,
    pub tool: usize,
    pub options: Vec<String>,
    pub active: String,
    pub text: String,
    pub abilities: Vec<(ab::Ability, bool)>,
    pub flight: u16,
    pub camera_flight: bool,
    pub momentum: u8,
    pub reach: Option<u16>,
    pub liquid: u8,
    pub brightness: u8,
    pub type_replace: bool,
    pub keep_existing: bool,
    pub copy_air: bool,
    pub turn: String,
    pub symmetry: String,
    pub history: (usize, usize),
    pub selection: String,
    pub op: &'static str,
    pub magic: Vec<(&'static str, bool)>,
    pub compare: &'static str,
    pub limit: usize,
    pub axiom: String,
    pub axiom_on: bool,
    pub axiom_disabled: bool,
}

impl View {
    pub fn of(s: &mut st::State) -> View {
        let selection = if !s.set.is_empty() {
            format!("{} blok (Magic Select)", s.set.len())
        } else {
            match (s.pos1, s.pos2) {
                (Some(a), Some(b)) => {
                    let d = [0, 1, 2].map(|i| (a[i] - b[i]).abs() + 1);
                    format!("{}x{}x{}", d[0], d[1], d[2])
                }
                (Some(_), None) | (None, Some(_)) => "bir köşe daha".into(),
                _ => "yok".into(),
            }
        };
        let o = s.magic.clone().unwrap_or_default();
        let op = s.op.name();
        let axiom_lines = s.axiom().lines();
        let (axiom_on, axiom_disabled) = {
            let a = s.axiom();
            (a.takes_edits(), a.disabled)
        };
        let e = s.editor();
        let a = &e.abilities;
        let sy = &e.symmetry;
        View {
            tools: e.tools.iter().map(|t| (t.id(), t.name())).collect(),
            tool: e.tool,
            options: e.tools.get(e.tool).map(|t| t.describe()).unwrap_or_default(),
            active: e.active.clone(),
            text: e.text.clone(),
            abilities: ab::ALL.iter().map(|x| (*x, a.is_on(*x))).collect(),
            flight: a.flight_speed,
            camera_flight: a.flight_direction == ab::FlightDirection::Camera,
            momentum: a.flight_momentum,
            reach: a.reach_limit,
            liquid: a.liquid_opacity,
            brightness: a.min_brightness,
            type_replace: a.type_replace,
            keep_existing: e.flags.keep_existing,
            copy_air: e.flags.copy_air,
            turn: format!(
                "{}°{}{}{}",
                u32::from(e.transform.rotate) * 90,
                if e.transform.flip[0] { " ↔x" } else { "" },
                if e.transform.flip[1] { " ↕y" } else { "" },
                if e.transform.flip[2] { " ↔z" } else { "" }
            ),
            symmetry: if sy.on() { format!("açık · döndür {}", sy.rotate) } else { "kapalı".into() },
            history: e.history.len(),
            selection,
            op,
            magic: vec![
                ("yuzey", o.surface_only),
                ("yukari", o.up),
                ("asagi", o.down),
                ("yatay", o.horizontal),
                ("koseler", o.corners),
            ],
            compare: o.compare.name(),
            limit: o.limit,
            axiom: axiom_lines.first().map_or_else(String::new, |l| l.trim_start_matches(st::PREFIX).to_owned()),
            axiom_on,
            axiom_disabled,
        }
    }
}

/// A capability's name on the menu.
fn ability_name(a: ab::Ability) -> &'static str {
    match a {
        ab::Ability::Bulldozer => "Buldozer",
        ab::Ability::ReplaceMode => "Değiştirme Modu",
        ab::Ability::ForcePlace => "Zorla Koy",
        ab::Ability::NoUpdates => "Güncellemesiz",
        ab::Ability::Tinker => "Kurcala",
        ab::Ability::InfiniteReach => "Sonsuz Menzil",
        ab::Ability::FastPlace => "Hızlı Koy",
        ab::Ability::AngelPlacement => "Havada Koy",
        ab::Ability::NoClip => "Duvardan Geçme",
    }
}

fn run(words: &[&str]) -> Hit {
    Hit::Run(words.iter().map(|w| (*w).to_owned()).collect())
}

fn toggle(label: &str, on: bool, words: &[&str]) -> (String, Style, Hit) {
    (format!("{} {label}", if on { "■" } else { "□" }), if on { Style::On } else { Style::Off }, run(words))
}

/// A row of the active tool's options (`name: value`, as `describe` writes
/// them): what clicking it does.
fn option_hit(line: &str) -> Hit {
    let Some((name, value)) = line.split_once(':') else {
        return Hit::Nothing;
    };
    let (name, value) = (name.trim(), value.trim());
    // A toggle flips at once (`set` with no value flips it); anything else
    // (a number, a shape, a block) opens a field holding the value now.
    if matches!(value, "açık" | "kapalı") {
        run(&["ayar", name])
    } else {
        Hit::Focus(Field::Option(name.to_owned()), value.to_owned())
    }
}

/// The left list's rows for the current tab: text, look, click.
fn left_rows(view: &View, tab: Tab) -> Vec<(String, Style, Hit)> {
    let mut rows: Vec<(String, Style, Hit)> = Vec::new();
    let label = |t: &str| (t.to_owned(), Style::Dim, Hit::Nothing);
    match tab {
        Tab::Tools => {
            for (i, (id, name)) in view.tools.iter().enumerate() {
                let style = if i == view.tool { Style::Selected } else { Style::Button };
                rows.push(((*name).to_owned(), style, run(&["arac", *id])));
            }
        }
        Tab::Builder => {
            let (undo, redo) = view.history;
            rows.push(label("Geçmiş"));
            rows.push((format!("Geri al ({undo})  Ctrl+Z"), Style::Button, run(&["geri"])));
            rows.push((format!("Yinele ({redo})  Ctrl+Y"), Style::Button, run(&["ileri"])));
            rows.push(label(&format!("Taşı / Klonla dönüşü: {}", view.turn)));
            rows.push(("Döndür 90°  Ctrl+R".into(), Style::Button, run(&["cevir"])));
            rows.push(("Aynala  Ctrl+F".into(), Style::Button, run(&["aynala"])));
            rows.push(toggle("Var olanı koru", view.keep_existing, &["bayrak", "koru"]));
            rows.push(toggle("Havayı kopyala", view.copy_air, &["bayrak", "hava"]));
            rows.push(label("Bakılan blokta"));
            rows.push(("Uzat (yüzü)".into(), Style::Button, run(&["uzat"])));
            rows.push(("Kısalt (yüzü)".into(), Style::Button, run(&["kisalt"])));
            rows.push(("Bağlı sil".into(), Style::Button, run(&["silbagli"])));
            rows.push(label(&format!("Simetri: {}", view.symmetry)));
            rows.push(("Nokta koy (yüzde)".into(), Style::Button, run(&["simetri", "nokta"])));
            rows.push(("Nokta koy (blok ortası)".into(), Style::Button, run(&["simetri", "merkez"])));
            rows.push(("Aynalama ekseni".into(), Style::Button, run(&["simetri", "aynala"])));
            rows.push(("Döndürme 0/2/4".into(), Style::Button, run(&["simetri", "cevir"])));
            rows.push(("Simetriyi kaldır".into(), Style::Button, run(&["simetri", "kaldir"])));
            rows.push(label("Taşı/Klonla/Sür: /eksen tasi x y z"));
        }
        Tab::Abilities => {
            rows.push(label("Yetenekler"));
            for (a, on) in &view.abilities {
                let note = if a.wired() { "" } else { " *" };
                let (t, s, h) = toggle(&format!("{}{note}", ability_name(*a)), *on, &["yetenek", a.id()]);
                rows.push((t, s, h));
            }
            rows.push(label("* Horizon bağlantısı bekliyor"));
            rows.push(label(&format!("Uçuş hızı: %{}", view.flight)));
            rows.push(("  − 10".into(), Style::Button, run(&["ucus", "hiz", "-"])));
            rows.push(("  + 10".into(), Style::Button, run(&["ucus", "hiz", "+"])));
            rows.push((format!("  Değer: {}", view.flight), Style::Field, Hit::Focus(Field::Flight("hiz"), view.flight.to_string())));
            let dir = if view.camera_flight { "kamera" } else { "yatay" };
            let other = if view.camera_flight { "yatay" } else { "kamera" };
            rows.push((format!("Uçuş yönü: {dir}"), Style::Button, run(&["ucus", "yon", other])));
            rows.push((format!("İvme: {}", view.momentum), Style::Field, Hit::Focus(Field::Flight("ivme"), view.momentum.to_string())));
            let reach = view.reach.map_or("sonsuz".to_owned(), |r| r.to_string());
            rows.push((format!("Menzil sınırı: {reach}"), Style::Field, Hit::Focus(Field::Flight("menzil"), reach)));
            rows.push((format!("Sıvı opaklığı: %{}", view.liquid), Style::Field, Hit::Focus(Field::Flight("sivi"), view.liquid.to_string())));
            rows.push((format!("En az parlaklık: %{}", view.brightness), Style::Field, Hit::Focus(Field::Flight("parlaklik"), view.brightness.to_string())));
            rows.push(toggle("Türü koru (değiştir)", view.type_replace, &["ucus", "tur"]));
        }
        Tab::Select => {
            rows.push(label(&format!("Seçim: {}", view.selection)));
            rows.push(("Doldur (aktif blok)".into(), Style::Button, run(&["doldur", view.active.as_str()])));
            rows.push(("Sil".into(), Style::Button, run(&["sil"])));
            rows.push(("Temizle".into(), Style::Button, run(&["temizle"])));
            rows.push(label(&format!("Sonraki seçim: {}", view.op)));
            for (w, name) in [("ekle", "Ekle"), ("cikar", "Çıkar"), ("yerine", "Yerine koy"), ("kesisim", "Kesişim")] {
                let style = if view.op.eq_ignore_ascii_case(name) || view.op == w { Style::Selected } else { Style::Button };
                rows.push((name.to_owned(), style, run(&["islem", w])));
            }
            rows.push(label("Magic Select (G)"));
            let next = match view.compare {
                "blok" => "durum",
                "durum" => "kati",
                "kati" => "herhangi",
                _ => "blok",
            };
            rows.push((format!("Karşılaştır: {}", view.compare), Style::Button, run(&["sihirli", "karsilastir", next])));
            rows.push((format!("Limit: {}", view.limit), Style::Field, Hit::Focus(Field::Magic("limit"), view.limit.to_string())));
            for (w, on) in &view.magic {
                rows.push(toggle(w, *on, &["sihirli", *w]));
            }
            rows.push(label("AxiomPaper"));
            rows.push(label(&view.axiom));
            if view.axiom_disabled {
                rows.push(("Eklentiyi kullan".into(), Style::Button, run(&["axiom", "ac"])));
            } else {
                rows.push(("Komutla gönder (eklentiyi kapat)".into(), Style::Button, run(&["axiom", "kapat"])));
            }
        }
    }
    rows
}

/// Everything on the screen, back to front.
fn items(ui: &Ui, view: &View, size: [f32; 2], measure: &dyn Fn(&str) -> f32) -> Vec<Item> {
    let [w, h] = size;
    let mut out = Vec::new();
    let mut push = |rect: [f32; 4], hit: Hit, text: String, style: Style| out.push(Item { rect, hit, text, style });

    // The bar: tabs, the version, the plugin.
    push([0.0, 0.0, w, BAR], Hit::Nothing, String::new(), Style::Bar);
    let mut x = PAD;
    for (tab, name) in TABS {
        let tw = measure(name) + 2.0 * PAD;
        let style = if ui.tab == tab { Style::Selected } else { Style::Button };
        push([x, 1.0, tw, BAR - 2.0], Hit::Tab(tab), name.to_owned(), style);
        x += tw + 2.0;
    }
    let title = format!("{} {}{}", st::NAME, st::VERSION, if view.axiom_on { " · AxiomPaper" } else { "" });
    let tw = measure(&title) + 2.0 * PAD;
    push([w - tw, 1.0, tw, BAR - 2.0], Hit::Nothing, title, Style::Dim);

    // The left list.
    let top = BAR + 2.0;
    let bottom = h - BOTTOM - 2.0;
    push([0.0, top, LEFT, bottom - top], Hit::Nothing, String::new(), Style::Panel);
    let rows = left_rows(view, ui.tab);
    let fits = (((bottom - top - PAD) / ROW).floor() as usize).max(1);
    let first = ui.scroll.min(rows.len().saturating_sub(fits));
    for (i, (text, style, hit)) in rows.into_iter().skip(first).take(fits).enumerate() {
        let y = top + PAD / 2.0 + i as f32 * ROW;
        push([2.0, y, LEFT - 4.0, ROW - 1.0], hit, text, style);
    }

    // The active tool's options on the right.
    let rx = w - RIGHT;
    push([rx, top, RIGHT, bottom - top], Hit::Nothing, String::new(), Style::Panel);
    let name = view.tools.get(view.tool).map_or("—", |t| t.1);
    push([rx + 2.0, top + PAD / 2.0, RIGHT - 4.0, ROW - 1.0], Hit::Nothing, format!("Araç: {name}"), Style::Label);
    let mut y = top + PAD / 2.0 + ROW;
    for line in &view.options {
        if y + ROW > bottom - 2.0 * ROW {
            break;
        }
        let field = matches!(&ui.field, Some(Field::Option(o)) if line.starts_with(&format!("{o}:")));
        let text = if field { format!("{}: {}_", line.split(':').next().unwrap_or(""), ui.buffer) } else { line.clone() };
        let style = if field { Style::Focused } else { Style::Button };
        push([rx + 2.0, y, RIGHT - 4.0, ROW - 1.0], option_hit(line), text, style);
        y += ROW;
    }
    push([rx + 2.0, bottom - 2.0 * ROW, RIGHT - 4.0, ROW], Hit::Nothing, "Sol tık: araç · sağ tık: 2. işlem".into(), Style::Dim);
    push([rx + 2.0, bottom - ROW, RIGHT - 4.0, ROW], Hit::Nothing, "Sağ Shift: kapat".into(), Style::Dim);

    // The bottom bar: the active block, the text, what happened last.
    push([0.0, h - BOTTOM, w, BOTTOM], Hit::Nothing, String::new(), Style::Bar);
    let half = (w - 3.0 * PAD) / 2.0;
    let field = |f: Field, label: &str, value: &str| -> (String, Style, Hit) {
        if ui.field.as_ref() == Some(&f) {
            (format!("{label}: {}_", ui.buffer), Style::Focused, Hit::Nothing)
        } else {
            (format!("{label}: {value}"), Style::Field, Hit::Focus(f, value.to_owned()))
        }
    };
    let (t, s, hit) = field(Field::Block, "Blok", &view.active);
    push([PAD, h - BOTTOM + 2.0, half, ROW], hit, t, s);
    let (t, s, hit) = field(Field::Text, "Metin", &view.text);
    push([2.0 * PAD + half, h - BOTTOM + 2.0, half, ROW], hit, t, s);
    let status = ui.status.join(" · ");
    push([PAD, h - ROW - 1.0, w - 2.0 * PAD, ROW], Hit::Nothing, status, Style::Dim);

    // A field focused in the left list shows what is typed.
    if let Some(f @ (Field::Flight(_) | Field::Magic(_))) = &ui.field {
        let label = match f {
            Field::Flight(s) | Field::Magic(s) => *s,
            _ => "",
        };
        push([LEFT + PAD, top + PAD, 140.0, ROW + 2.0], Hit::Nothing, format!("{label}: {}_  (Enter)", ui.buffer), Style::Focused);
    }
    out
}

// --------------------------------------------------------------- the world

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// The direction through the pointer at `mouse` on a `size` screen, from a
/// camera (yaw, pitch, vertical field of view in degrees), the inverse of
/// the world's perspective projection.
pub fn mouse_ray(yaw: f32, pitch: f32, fov: f32, mouse: [f32; 2], size: [f32; 2]) -> [f64; 3] {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let f = [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()];
    let r = [-y.cos(), 0.0, -y.sin()];
    let u = cross(r, f);
    let t = (f64::from(fov).to_radians() / 2.0).tan();
    let aspect = f64::from(size[0]) / f64::from(size[1].max(1.0));
    let nx = 2.0 * f64::from(mouse[0]) / f64::from(size[0].max(1.0)) - 1.0;
    let ny = 1.0 - 2.0 * f64::from(mouse[1]) / f64::from(size[1].max(1.0));
    let d = [0, 1, 2].map(|i| f[i] + r[i] * nx * t * aspect + u[i] * ny * t);
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
    d.map(|v| v / len)
}

/// The first block that is not air along `dir` from `from`, up to `max`
/// blocks, and the face the ray came in by (a unit step out of it). Stops at
/// an unloaded block.
pub fn ray(world: &dyn Blocks, from: [f64; 3], dir: [f64; 3], max: f64) -> Option<(Pos, Pos)> {
    let mut cell: Pos = from.map(|v| v.floor() as i32);
    let step: Pos = dir.map(|d| if d > 0.0 { 1 } else if d < 0.0 { -1 } else { 0 });
    let delta = dir.map(|d| if d == 0.0 { f64::INFINITY } else { (1.0 / d).abs() });
    let mut next = [0, 1, 2].map(|i| {
        if step[i] > 0 {
            (f64::from(cell[i]) + 1.0 - from[i]) * delta[i]
        } else if step[i] < 0 {
            (from[i] - f64::from(cell[i])) * delta[i]
        } else {
            f64::INFINITY
        }
    });
    let mut face = [0; 3];
    let mut travelled = 0.0;
    while travelled <= max {
        let b = world.block(cell)?;
        if !b.air() && face != [0; 3] {
            return Some((cell, face));
        }
        let axis = (0..3).min_by(|a, b| next[*a].total_cmp(&next[*b])).unwrap_or(0);
        travelled = next[axis];
        next[axis] += delta[axis];
        cell[axis] += step[axis];
        face = [0; 3];
        face[axis] = -step[axis];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::eksen_core::Block;
    use super::*;

    fn measure(t: &str) -> f32 {
        t.chars().count() as f32 * 6.0
    }

    fn view() -> View {
        View {
            tools: vec![("kutu", "Kutu Seçimi"), ("boyaci", "Boyacı")],
            options: vec!["yaricap: 5".into(), "kati: açık".into()],
            active: "minecraft:stone".into(),
            abilities: ab::ALL.iter().map(|a| (*a, false)).collect(),
            ..View::default()
        }
    }

    fn ui() -> Ui {
        Ui { size: [480.0, 270.0], ..Ui::default() }
    }

    #[test]
    fn a_tool_row_picks_the_tool() {
        let mut u = ui();
        let hit = u.click(&view(), [20.0, BAR + 2.0 + PAD / 2.0 + ROW + 3.0], &measure);
        assert_eq!(hit, Hit::Run(vec!["arac".into(), "boyaci".into()]));
    }

    #[test]
    fn the_middle_is_the_world_and_tabs_switch() {
        let mut u = ui();
        assert_eq!(u.click(&view(), [240.0, 130.0], &measure), Hit::World);
        let hit = u.click(&view(), [PAD + measure("Araçlar") + 2.0 * PAD + 6.0, 6.0], &measure);
        assert_eq!(hit, Hit::Tab(Tab::Builder));
        assert_eq!(u.tab, Tab::Builder);
    }

    #[test]
    fn the_block_field_types_and_sends() {
        let mut u = ui();
        let hit = u.click(&view(), [10.0, 270.0 - BOTTOM + 4.0], &measure);
        assert!(matches!(hit, Hit::Focus(Field::Block, _)));
        u.buffer.clear();
        for c in "oak planks".chars() {
            u.char(c);
        }
        u.field_key("key.keyboard.backspace");
        let (took, cmd) = u.field_key("key.keyboard.enter");
        assert!(took);
        assert_eq!(cmd, Some(vec!["blok".to_owned(), "oak_plank".to_owned()]));
        assert_eq!(u.field, None);
    }

    #[test]
    fn the_status_keeps_two_lines() {
        let mut u = ui();
        u.say(vec!["[Eksen] a".into(), "[Eksen] b".into(), "[Eksen] c".into()]);
        assert_eq!(u.status, vec!["b".to_owned(), "c".to_owned()]);
        assert_eq!(u.chat.len(), 3);
    }

    #[test]
    fn the_centre_ray_is_the_view() {
        let d = mouse_ray(0.0, 0.0, 70.0, [240.0, 135.0], [480.0, 270.0]);
        assert!((d[2] - 1.0).abs() < 1e-9);
        let right = mouse_ray(0.0, 0.0, 70.0, [470.0, 135.0], [480.0, 270.0]);
        assert!(right[0] < 0.0, "facing south, the right is west");
        let up = mouse_ray(0.0, 0.0, 70.0, [240.0, 5.0], [480.0, 270.0]);
        assert!(up[1] > 0.0);
    }

    struct Floor;
    impl Blocks for Floor {
        fn block(&self, at: Pos) -> Option<Block> {
            let id = if at[1] < 0 { "minecraft:stone" } else { "minecraft:air" };
            Some(Block { id: id.into(), state: String::new(), solid: at[1] < 0 })
        }
    }

    #[test]
    fn the_ray_finds_the_floor_and_its_face() {
        let hit = ray(&Floor, [0.5, 5.5, 0.5], [0.0, -1.0, 0.0], 64.0);
        assert_eq!(hit, Some(([0, -1, 0], [0, 1, 0])));
        let slanted = ray(&Floor, [0.5, 1.5, 0.5], [0.6, -0.8, 0.0], 64.0).unwrap();
        assert_eq!(slanted.0[1], -1);
        assert_eq!(ray(&Floor, [0.5, 5.5, 0.5], [0.0, 1.0, 0.0], 32.0), None);
    }

    #[test]
    fn option_rows_toggle_or_type() {
        assert_eq!(option_hit("kati: açık"), Hit::Run(vec!["ayar".into(), "kati".into()]));
        assert_eq!(option_hit("yaricap: 5"), Hit::Focus(Field::Option("yaricap".into()), "5".into()));
        assert_eq!(option_hit("başlık"), Hit::Nothing);
        assert_eq!(
            Field::Option("yaricap".into()).command("8"),
            vec!["ayar".to_owned(), "yaricap".to_owned(), "8".to_owned()]
        );
    }

    #[test]
    fn long_text_is_cut() {
        assert_eq!(fit("abcdef", 30.0, &measure), "abcd…");
        assert_eq!(fit("abc", 30.0, &measure), "abc");
    }
}
