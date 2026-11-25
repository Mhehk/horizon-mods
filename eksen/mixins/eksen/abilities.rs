// Eksen — 16.10.1 (mihehk): the left side of the builder menu (capabilities)
// and its right side (flight speed, the toolbox). Written from scratch after
// the behaviour Axiom's documentation describes
// (.build-tools/axiom-docs/axiom-docs.md: "Context Menu", "Capabilities",
// "Tinker" … "Bulldozer"); no code, names or images of Axiom are used.
//
// What works through commands is done here: placing and breaking from any
// distance, in the air, by replacing, without block updates (`strict`).
// What changes the player's own movement or the renderer (flight speed and
// direction, momentum, no clip, liquid opacity, brightness) needs a Horizon
// hook no mod has today; those settings are kept and shown, and `wired()`
// says which ones take effect.

/// A capability (a toggle on the menu's left side).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ability {
    Bulldozer,
    ReplaceMode,
    ForcePlace,
    NoUpdates,
    Tinker,
    InfiniteReach,
    FastPlace,
    AngelPlacement,
    NoClip,
}

pub const ALL: [Ability; 9] = [
    Ability::Bulldozer,
    Ability::ReplaceMode,
    Ability::ForcePlace,
    Ability::NoUpdates,
    Ability::Tinker,
    Ability::InfiniteReach,
    Ability::FastPlace,
    Ability::AngelPlacement,
    Ability::NoClip,
];

impl Ability {
    pub fn id(self) -> &'static str {
        match self {
            Ability::Bulldozer => "buldozer",
            Ability::ReplaceMode => "degistirme",
            Ability::ForcePlace => "zorla",
            Ability::NoUpdates => "guncellemesiz",
            Ability::Tinker => "kurcala",
            Ability::InfiniteReach => "sonsuzmenzil",
            Ability::FastPlace => "hizlikoy",
            Ability::AngelPlacement => "havadakoy",
            Ability::NoClip => "duvargecis",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Ability::Bulldozer => "blokları normalden hızlı kırar",
            Ability::ReplaceMode => "baktığın bloğu elindeki (aktif) blokla değiştirir",
            Ability::ForcePlace => "yerleştirme kısıtlamalarını aşar",
            Ability::NoUpdates => "koyma/kırmada çevre bloklar güncellenmez",
            Ability::Tinker => "boş elle sağ tık: bloğun özelliklerini değiştirir",
            Ability::InfiniteReach => "her mesafeden koyar ve kırar",
            Ability::FastPlace => "koyma bekleme süresini kaldırır",
            Ability::AngelPlacement => "destek bloğu olmadan havaya koyar",
            Ability::NoClip => "katı bloklardan geçerek uçarsın",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        ALL.iter().copied().find(|a| a.id() == word)
    }

    /// Whether Eksen can make it work through commands, without fighting the
    /// game's own placing and breaking: past vanilla's reach, in the air,
    /// one break a tick, and `strict` edits. The rest (replacing what vanilla
    /// would place next to, force place, fast place, tinker, no clip) need a
    /// Horizon hook on block interaction or movement.
    pub fn wired(self) -> bool {
        matches!(self, Ability::Bulldozer | Ability::NoUpdates | Ability::InfiniteReach | Ability::AngelPlacement)
    }
}

/// "Flight direction": vanilla flies level; camera flies where you look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightDirection {
    Horizontal,
    Camera,
}

/// The menu's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Abilities {
    on: Vec<Ability>,
    /// Percent of normal flight speed, 1 to 999 (100: normal).
    pub flight_speed: u16,
    pub flight_direction: FlightDirection,
    /// 0 (instant) to 100 (slow to change speed).
    pub flight_momentum: u8,
    /// Blocks; None: infinite (the default). Else 5 to 100.
    pub reach_limit: Option<u16>,
    /// Water opacity, percent (100 normal, 0 invisible).
    pub liquid_opacity: u8,
    /// Percent (0 default, 100 no shadows).
    pub min_brightness: u8,
    /// Replace keeps the stair/slab variant of what it replaces.
    pub type_replace: bool,
    pub key_hints: bool,
}

pub const FLIGHT_MAX: u16 = 999;
/// The reach of Infinite Reach when no limit is set.
pub const INFINITE: f32 = 512.0;
/// Vanilla's creative reach.
pub const VANILLA_REACH: f32 = 5.0;

impl Abilities {
    pub const fn new() -> Self {
        Abilities {
            on: Vec::new(),
            flight_speed: 100,
            flight_direction: FlightDirection::Horizontal,
            flight_momentum: 0,
            reach_limit: None,
            liquid_opacity: 100,
            min_brightness: 0,
            type_replace: false,
            key_hints: false,
        }
    }

    pub fn is_on(&self, a: Ability) -> bool {
        self.on.contains(&a)
    }

    /// Flips `a`; true when it is now on.
    pub fn toggle(&mut self, a: Ability) -> bool {
        if let Some(i) = self.on.iter().position(|x| *x == a) {
            self.on.remove(i);
            false
        } else {
            self.on.push(a);
            self.on.sort();
            true
        }
    }

    /// How far placing and breaking reach.
    pub fn reach(&self) -> f32 {
        if self.is_on(Ability::InfiniteReach) {
            self.reach_limit.map_or(INFINITE, f32::from)
        } else {
            VANILLA_REACH
        }
    }

    /// The tail `fill`/`setblock` get: `strict` with No Updates.
    pub fn tail(&self) -> &'static str {
        if self.is_on(Ability::NoUpdates) {
            " strict"
        } else {
            ""
        }
    }

    /// The flight speed slider's +/- buttons: steps of 10 percent.
    pub fn nudge_flight(&mut self, up: bool) {
        let v = i32::from(self.flight_speed) + if up { 10 } else { -10 };
        self.flight_speed = v.clamp(1, i32::from(FLIGHT_MAX)) as u16;
    }

    /// The vanilla flying speed (0.05 per tick) scaled by the slider.
    pub fn flying_speed(&self) -> f32 {
        0.05 * f32::from(self.flight_speed) / 100.0
    }

    /// One tick of flight: the velocity `current` eases towards `wanted` by
    /// the momentum (0 jumps straight there).
    pub fn ease(&self, current: [f64; 3], wanted: [f64; 3]) -> [f64; 3] {
        let k = 1.0 - f64::from(self.flight_momentum.min(100)) / 101.0;
        [0, 1, 2].map(|i| current[i] + (wanted[i] - current[i]) * k)
    }

    /// The menu as chat lines.
    pub fn lines(&self) -> Vec<String> {
        let mut out = vec!["[Eksen] Yetenekler (/eksen yetenek <ad>):".to_owned()];
        for a in ALL {
            let mark = if self.is_on(a) { "■" } else { "□" };
            let note = if a.wired() { "" } else { " (Horizon bağlantısı bekliyor)" };
            out.push(format!("{mark} {} — {}{note}", a.id(), a.description()));
        }
        let dir = match self.flight_direction {
            FlightDirection::Horizontal => "yatay",
            FlightDirection::Camera => "kamera",
        };
        let reach = self.reach_limit.map_or("sonsuz".to_owned(), |r| r.to_string());
        out.push(format!(
            "Uçuş hızı %{} · yön {dir} · ivme {} · menzil sınırı {reach} · sıvı opaklığı %{} · en düşük parlaklık %{} · tür koruyarak değiştir {}",
            self.flight_speed,
            self.flight_momentum,
            self.liquid_opacity,
            self.min_brightness,
            if self.type_replace { "açık" } else { "kapalı" }
        ));
        out.push("Uçuş hızı, yön, ivme, duvardan geçme, sıvı opaklığı ve parlaklık Horizon bağlantısı bekliyor.".to_owned());
        out
    }

    /// `/eksen ucus|arac-kutusu <setting> <value>`.
    pub fn set(&mut self, setting: &str, value: Option<&str>) -> Result<(), String> {
        let num = |lo: i64, hi: i64| -> Result<i64, String> {
            let v: i64 = value.and_then(|w| w.trim_start_matches('%').parse().ok()).ok_or(format!("Sayı gerekli ({lo}–{hi})."))?;
            if !(lo..=hi).contains(&v) {
                return Err(format!("Değer {lo} ile {hi} arasında olmalı."));
            }
            Ok(v)
        };
        match setting {
            "hiz" | "hız" | "speed" => match value {
                Some("+") => self.nudge_flight(true),
                Some("-") => self.nudge_flight(false),
                _ => self.flight_speed = num(1, i64::from(FLIGHT_MAX))? as u16,
            },
            "yon" | "yön" | "direction" => {
                self.flight_direction = match value {
                    Some("yatay" | "horizontal") => FlightDirection::Horizontal,
                    Some("kamera" | "camera") => FlightDirection::Camera,
                    _ => return Err("yatay veya kamera bekleniyordu.".into()),
                }
            }
            "ivme" | "momentum" => self.flight_momentum = num(0, 100)? as u8,
            "menzil" | "reach" => {
                self.reach_limit = match value {
                    Some("sonsuz" | "infinite") => None,
                    _ => Some(num(5, 100)? as u16),
                }
            }
            "sivi" | "sıvı" | "liquid" => self.liquid_opacity = num(0, 100)? as u8,
            "parlaklik" | "parlaklık" | "brightness" => self.min_brightness = num(0, 100)? as u8,
            "tur" | "tür" | "type" => self.type_replace = !self.type_replace,
            "tuslar" | "tuşlar" | "keys" => self.key_hints = !self.key_hints,
            other => return Err(format!("Bilinmeyen ayar: {other}")),
        }
        Ok(())
    }
}

impl Default for Abilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Where Angel Placement puts a block: `reach` blocks along the view from
/// the eyes when nothing is hit (rounded to the block there).
pub fn angel_target(eye: [f64; 3], look: [f64; 3], distance: f64) -> [i32; 3] {
    [0, 1, 2].map(|i| (eye[i] + look[i] * distance).floor() as i32)
}

/// Type Replace: `new` takes the variant properties of `old` it shares
/// (stairs' facing/half/shape, slabs' type, …), so a stair stays a stair's
/// shape when replaced by another stair.
pub fn keep_variant(old: &str, new: &str) -> String {
    let props = |b: &str| -> Vec<(String, String)> {
        b.strip_suffix(']')
            .and_then(|b| b.split_once('[').map(|(_, p)| p.to_owned()))
            .map(|p| p.split(',').filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned()))).collect())
            .unwrap_or_default()
    };
    let id = |b: &str| b.split('[').next().unwrap_or(b).to_owned();
    let kind = |b: &str| id(b).rsplit('_').next().unwrap_or("").to_owned();
    if kind(old) != kind(new) {
        return new.to_owned();
    }
    let mut out = props(new);
    for (k, v) in props(old) {
        if let Some(e) = out.iter_mut().find(|(ok, _)| *ok == k) {
            e.1 = v;
        } else {
            out.push((k, v));
        }
    }
    if out.is_empty() {
        return id(new);
    }
    let p: Vec<String> = out.into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{}[{}]", id(new), p.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_reach_and_tail() {
        let mut a = Abilities::new();
        assert_eq!(a.reach(), VANILLA_REACH);
        assert!(a.toggle(Ability::InfiniteReach));
        assert_eq!(a.reach(), INFINITE);
        a.set("menzil", Some("40")).unwrap();
        assert_eq!(a.reach(), 40.0);
        assert!(a.set("menzil", Some("3")).is_err());
        assert_eq!(a.tail(), "");
        a.toggle(Ability::NoUpdates);
        assert_eq!(a.tail(), " strict");
        assert!(!a.toggle(Ability::NoUpdates));
        assert_eq!(Ability::parse("havadakoy"), Some(Ability::AngelPlacement));
    }

    #[test]
    fn flight_speed_slider() {
        let mut a = Abilities::new();
        a.set("hiz", Some("%250")).unwrap();
        assert_eq!(a.flight_speed, 250);
        assert!((a.flying_speed() - 0.125).abs() < 1e-6);
        a.flight_speed = 995;
        a.nudge_flight(true);
        assert_eq!(a.flight_speed, FLIGHT_MAX);
        assert!(a.set("hiz", Some("1000")).is_err());
        // No momentum: straight to the wanted speed; full momentum: barely.
        assert_eq!(a.ease([0.0; 3], [1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
        a.flight_momentum = 100;
        assert!(a.ease([0.0; 3], [1.0, 0.0, 0.0])[0] < 0.02);
    }

    #[test]
    fn angel_and_type_replace() {
        assert_eq!(angel_target([0.5, 64.0, 0.5], [0.0, 0.0, 1.0], 4.0), [0, 64, 4]);
        assert_eq!(
            keep_variant("minecraft:oak_stairs[facing=east,half=top]", "minecraft:stone_brick_stairs"),
            "minecraft:stone_brick_stairs[facing=east,half=top]"
        );
        assert_eq!(keep_variant("minecraft:oak_stairs[facing=east]", "minecraft:stone"), "minecraft:stone");
    }
}
