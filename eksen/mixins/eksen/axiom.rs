// Eksen — 16.10.2 (mihehk): edits through a server's AxiomPaper plugin.
//
// AxiomPaper (github.com/Moulberry/AxiomPaperPlugin, MIT) is the server half
// of Axiom. A client that answers its handshake gets the plugin's own path
// for edits: whole chunk sections in one packet (`set_buffer`) and single
// blocks with a reason (`set_block`), checked against the plugin's
// permissions, rate limit and protection integrations, instead of vanilla
// `fill` commands that need operator rights. This module speaks that wire
// format (API version 10, read from the plugin's source); it is plain Rust:
// the hooks hand it the server's payloads and send what it gives back.
//
// What travels:
//   server → `axiom:hello` (long id), every second until answered
//   client → `axiom:hello` (varint api, varint data version, varint protocol, long id)
//   server → `axiom:enable` (bool, config byte, varint blueprint version,
//            int split size, int max packet, varint, varint, identifiers)
//   server → `axiom:restrictions`, `axiom:update_available_dispatch_sends`,
//            `axiom:goodbye` (why it refused)
//   client → `axiom:tunnel`: [flags] + (identifier, int size, byte 0, body),
//            split into pieces of the server's split size.
use std::collections::{BTreeMap, HashSet, VecDeque};

pub type Pos = [i32; 3];

/// The plugin's `AxiomConstants.API_VERSION` this speaks.
pub const API_VERSION: i32 = 10;
/// `BlockPos.asLong(-33554432, -2048, -33554432)`: ends a block buffer.
pub const END: i64 = as_long([-33_554_432, -2048, -33_554_432]);
const FIRST: u8 = 1;
const LAST: u8 = 2;
/// The most distinct states one section's palette holds (8 bits, one kept
/// for "leave as is").
const PALETTE_MAX: usize = 255;
/// A set_buffer body stays under this share of the server's limit.
const BODY_SHARE: f64 = 0.9;

/// The channels Eksen listens on and sends to (`minecraft:register`).
pub const CHANNELS: [&str; 9] = [
    "axiom:hello",
    "axiom:enable",
    "axiom:goodbye",
    "axiom:restrictions",
    "axiom:update_available_dispatch_sends",
    "axiom:tunnel",
    "axiom:set_buffer",
    "axiom:set_block",
    "axiom:set_fly_speed",
];

/// `BlockPos.asLong`: x in the top 26 bits, z in the next 26, y in the low 12.
pub const fn as_long(p: Pos) -> i64 {
    ((p[0] as i64 & 0x3FF_FFFF) << 38) | ((p[2] as i64 & 0x3FF_FFFF) << 12) | (p[1] as i64 & 0xFFF)
}

// ---------------------------------------------------------------- the wire

/// A `FriendlyByteBuf` being written.
#[derive(Default)]
pub struct Out(pub Vec<u8>);

impl Out {
    pub fn byte(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.byte(u8::from(v))
    }
    pub fn varint(&mut self, v: i32) -> &mut Self {
        let mut u = v as u32;
        loop {
            if u & !0x7F == 0 {
                self.0.push(u as u8);
                return self;
            }
            self.0.push((u & 0x7F | 0x80) as u8);
            u >>= 7;
        }
    }
    pub fn int(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn long(&mut self, v: i64) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn float(&mut self, v: f32) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn utf(&mut self, s: &str) -> &mut Self {
        self.varint(s.len() as i32);
        self.0.extend_from_slice(s.as_bytes());
        self
    }
    pub fn uuid(&mut self, v: u128) -> &mut Self {
        self.long((v >> 64) as i64).long(v as i64)
    }
    pub fn pos(&mut self, p: Pos) -> &mut Self {
        self.long(as_long(p))
    }
}

/// A `FriendlyByteBuf` being read; every read is None past the end.
pub struct In<'a>(pub &'a [u8]);

impl In<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    pub fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    pub fn bool(&mut self) -> Option<bool> {
        self.byte().map(|b| b != 0)
    }
    pub fn varint(&mut self) -> Option<i32> {
        let mut v: u32 = 0;
        for i in 0..5 {
            let b = self.byte()?;
            v |= u32::from(b & 0x7F) << (7 * i);
            if b & 0x80 == 0 {
                return Some(v as i32);
            }
        }
        None
    }
    pub fn int(&mut self) -> Option<i32> {
        self.take(4).map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn long(&mut self) -> Option<i64> {
        let b = self.take(8)?;
        let mut a = [0; 8];
        a.copy_from_slice(b);
        Some(i64::from_be_bytes(a))
    }
    pub fn utf(&mut self) -> Option<String> {
        let n = usize::try_from(self.varint()?).ok()?;
        self.take(n).map(|b| String::from_utf8_lossy(b).into_owned())
    }
}

/// A chunk section's blocks as `PalettedContainer.write` sends them (the
/// format since 1.21.5: no length before the longs): an 4 to 8 bit palette,
/// then the indices packed low bits first, `64 / bits` to a long, in
/// `(y << 8) | (z << 4) | x` order. `cells` maps that index to a state id;
/// every other cell gets `keep` (the plugin's "leave as is", void air).
pub fn container(cells: &BTreeMap<u16, u32>, keep: u32, out: &mut Out) {
    let mut palette = vec![keep];
    for id in cells.values() {
        if !palette.contains(id) {
            palette.push(*id);
        }
    }
    let bits = (usize::BITS - (palette.len() - 1).max(1).leading_zeros()).max(4);
    out.byte(bits as u8).varint(palette.len() as i32);
    for id in &palette {
        out.varint(*id as i32);
    }
    let per = 64 / bits as usize;
    let mut longs = vec![0u64; 4096_usize.div_ceil(per)];
    for (cell, id) in cells {
        let index = palette.iter().position(|p| p == id).unwrap_or(0) as u64;
        let i = usize::from(*cell);
        longs[i / per] |= index << ((i % per) * bits as usize);
    }
    for l in longs {
        out.long(l as i64);
    }
}

/// `body` as the tunnel carries it: (identifier, size, flags 0, body), cut
/// into pieces no longer than `split`, each led by FIRST / LAST flags.
pub fn tunnel(identifier: &str, body: &[u8], split: usize) -> Vec<Vec<u8>> {
    let mut frame = Out::default();
    frame.utf(identifier).int(body.len() as i32).byte(0);
    frame.0.extend_from_slice(body);
    let piece = split.max(2) - 1;
    let chunks: Vec<&[u8]> = frame.0.chunks(piece).collect();
    let n = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let mut flags = 0;
            if i == 0 {
                flags |= FIRST;
            }
            if i + 1 == n {
                flags |= LAST;
            }
            let mut v = Vec::with_capacity(c.len() + 1);
            v.push(flags);
            v.extend_from_slice(c);
            v
        })
        .collect()
}

// --------------------------------------------------------------- the link

/// What the server's plugin said in `axiom:enable`.
#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub split: usize,
    pub max_packet: usize,
    /// The packets it takes (`axiom:set_buffer` …).
    pub packets: HashSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Link {
    /// No plugin seen (or it switched Axiom off for us).
    #[default]
    None,
    /// Hello answered, waiting for `enable`.
    Answered,
    Active(Server),
    /// The plugin refused, with its reason.
    Refused(String),
}

/// The plugin connection and the edits waiting for its rate limit.
#[derive(Default)]
pub struct Axiom {
    pub link: Link,
    /// Off: Eksen sends commands even when the plugin is there.
    pub disabled: bool,
    /// The protocol version said in hello (the joined server's); 0: the
    /// game's own (see `eksen::protocol_version`).
    pub protocol_override: i32,
    pub allowed: Vec<String>,
    pub denied: Vec<String>,
    /// The plugin's infinite reach limit (-1: none).
    pub reach_limit: i32,
    /// Sections the plugin lets us send now, and at most.
    pub sends: i32,
    pub sends_max: i32,
    /// Payloads to send: (channel, data).
    pub out: VecDeque<(String, Vec<u8>)>,
    /// Sections waiting for the rate limit: (section, cells).
    sections: VecDeque<(Pos, BTreeMap<u16, u32>)>,
    /// Chat lines for the player.
    pub notes: Vec<String>,
    /// The dimension the waiting sections are for.
    dimension: String,
    /// Void air's state id: the cells a section leaves as they are.
    keep: u32,
    /// The last fly speed sent.
    fly: Option<f32>,
    seed: u64,
}

impl Axiom {
    pub fn active(&self) -> Option<&Server> {
        match &self.link {
            Link::Active(s) if !self.disabled => Some(s),
            _ => None,
        }
    }

    /// The plugin's permission check, as `Restrictions` sends it: allowed
    /// by the permission, its parent group or `axiom.*` / `axiom.all`, and
    /// denied by neither the permission nor its group.
    pub fn may(&self, permission: &str) -> bool {
        let group = permission.rsplit_once('.').map(|(g, _)| format!("{g}.*"));
        let has = |list: &[String], name: &str| list.iter().any(|p| p == name);
        let chain = [Some(permission.to_owned()), group.clone()];
        let allowed = has(&self.allowed, "axiom.all")
            || has(&self.allowed, "axiom.*")
            || chain.iter().flatten().any(|n| has(&self.allowed, n));
        let denied = chain.iter().flatten().any(|n| has(&self.denied, n));
        allowed && !denied
    }

    /// Edits go through the plugin: it is there, takes block buffers and
    /// lets us build sections.
    pub fn takes_edits(&self) -> bool {
        self.active().is_some_and(|s| s.packets.contains("axiom:set_buffer")) && self.may("axiom.build.section")
    }

    /// Off the server: forget it all.
    pub fn reset(&mut self) {
        let (disabled, protocol_override) = (self.disabled, self.protocol_override);
        *self = Axiom { disabled, protocol_override, ..Axiom::default() };
    }

    /// A payload from the server. `versions` are (data version, protocol
    /// version) for the hello answer. True when it was the plugin's.
    pub fn on_payload(&mut self, channel: &str, data: &[u8], versions: (i32, i32)) -> bool {
        let Some(name) = channel.strip_prefix("axiom:") else {
            return false;
        };
        let mut r = In(data);
        match name {
            "hello" => {
                let Some(id) = r.long() else { return true };
                if matches!(self.link, Link::None | Link::Refused(_)) {
                    self.out.push_back(("minecraft:register".into(), CHANNELS.join("\0").into_bytes()));
                }
                // Only the newest id counts: an answer not sent yet is stale.
                self.out.retain(|(c, _)| c != "axiom:hello");
                let protocol = if self.protocol_override > 0 { self.protocol_override } else { versions.1 };
                let mut o = Out::default();
                o.varint(API_VERSION).varint(versions.0).varint(protocol).long(id);
                self.out.push_back(("axiom:hello".into(), o.0));
                self.link = Link::Answered;
            }
            "enable" => {
                if r.bool() != Some(true) {
                    self.link = Link::None;
                    return true;
                }
                let server = (|| {
                    let _config = r.byte()?;
                    let _blueprint = r.varint()?;
                    let split = usize::try_from(r.int()?).ok()?;
                    let max_packet = usize::try_from(r.int()?).ok()?;
                    // Blocks with custom data, ignored rotations: both empty
                    // from the plugin; skip what a newer one may list.
                    for _ in 0..2 {
                        let n = r.varint()?;
                        for _ in 0..n {
                            r.utf()?;
                        }
                    }
                    let n = r.varint()?;
                    let mut packets = HashSet::new();
                    for _ in 0..n {
                        packets.insert(r.utf()?);
                    }
                    Some(Server { split, max_packet, packets })
                })();
                match server {
                    Some(s) => {
                        let was = matches!(self.link, Link::Active(_));
                        self.link = Link::Active(s);
                        if !was {
                            self.notes.push("Sunucuda AxiomPaper var: düzenlemeler eklentinin yolundan gidecek.".into());
                        }
                    }
                    None => self.link = Link::Refused("enable paketi okunamadı".into()),
                }
            }
            "goodbye" => {
                let why = r.utf().unwrap_or_default();
                self.notes.push(format!("AxiomPaper reddetti: {why}"));
                self.link = Link::Refused(why);
            }
            "restrictions" => {
                let list = |r: &mut In| -> Option<Vec<String>> {
                    let n = r.varint()?;
                    (0..n).map(|_| r.utf()).collect()
                };
                if let (Some(a), Some(d), Some(reach)) = (list(&mut r), list(&mut r), r.int()) {
                    self.allowed = a;
                    self.denied = d;
                    self.reach_limit = reach;
                }
            }
            "update_available_dispatch_sends" => {
                if let (Some(add), Some(max)) = (r.varint(), r.varint()) {
                    self.sends_max = max;
                    self.sends = (self.sends + add).min(max);
                }
            }
            _ => {}
        }
        true
    }

    /// Queues `edit` (position to state id) as block buffer sections.
    /// `keep` is void air's id (the cells a section leaves alone).
    pub fn queue_edit(&mut self, dimension: &str, edit: &BTreeMap<Pos, u32>, keep: u32) {
        self.dimension = dimension.to_owned();
        let mut by_section: BTreeMap<Pos, BTreeMap<u16, u32>> = BTreeMap::new();
        for (p, id) in edit {
            let s = [p[0] >> 4, p[1] >> 4, p[2] >> 4];
            let cell = ((p[1] & 15) << 8 | (p[2] & 15) << 4 | (p[0] & 15)) as u16;
            by_section.entry(s).or_default().insert(cell, *id);
        }
        for (s, cells) in by_section {
            // More kinds than one palette holds: the section goes in parts.
            let mut part: BTreeMap<u16, u32> = BTreeMap::new();
            let mut kinds: HashSet<u32> = HashSet::new();
            for (cell, id) in cells {
                if !kinds.contains(&id) && kinds.len() == PALETTE_MAX {
                    self.sections.push_back((s, std::mem::take(&mut part)));
                    kinds.clear();
                }
                kinds.insert(id);
                part.insert(cell, id);
            }
            if !part.is_empty() {
                self.sections.push_back((s, part));
            }
        }
        self.keep = keep;
    }

    /// Sections still waiting.
    pub fn waiting(&self) -> usize {
        self.sections.len()
    }

    /// Each tick: as many waiting sections as the rate limit and the packet
    /// size allow go out as one `set_buffer` through the tunnel.
    pub fn tick(&mut self) {
        let Some(server) = self.active().cloned() else {
            return;
        };
        if self.sections.is_empty() || self.sends <= 0 {
            return;
        }
        let budget = (server.max_packet as f64 * BODY_SHARE) as usize;
        let mut body = Out::default();
        body.utf(&self.dimension).uuid(self.next_uuid()).byte(0);
        let mut sent = 0;
        while self.sends > sent {
            let Some((s, cells)) = self.sections.front() else { break };
            let mut one = Out::default();
            one.pos(*s);
            container(cells, self.keep, &mut one);
            one.varint(0); // no block entities
            if sent > 0 && body.0.len() + one.0.len() + 16 > budget {
                break;
            }
            body.0.extend_from_slice(&one.0);
            self.sections.pop_front();
            sent += 1;
        }
        self.sends -= sent;
        body.long(END).varint(self.sends.max(0));
        for piece in tunnel("axiom:set_buffer", &body.0, server.split) {
            self.out.push_back(("axiom:tunnel".into(), piece));
        }
        if self.sections.is_empty() {
            self.notes.push("AxiomPaper: değişikliğin tamamı gönderildi.".into());
        }
    }

    /// One block placed past vanilla's reach or in the air (`set_block`):
    /// `angel` marks Angel Placement (the plugin skips the interact event).
    pub fn place(&mut self, at: Pos, id: u32, face: u8, angel: bool, updates: bool) -> bool {
        let Some(server) = self.active().cloned() else {
            return false;
        };
        if !server.packets.contains("axiom:set_block") || !self.may("axiom.build.place") {
            return false;
        }
        let mut o = Out::default();
        o.varint(1).pos(at).varint(id as i32);
        o.bool(updates);
        if updates {
            o.varint(0); // no positions kept from updates
        }
        o.varint(if angel { 128 } else { 0 }).bool(false);
        // The hit: the block itself, its centre, the face looked at.
        o.pos(at).byte(face).float(0.5).float(0.5).float(0.5).bool(false).bool(false);
        o.varint(0).varint(-1); // main hand, no sequence
        for piece in tunnel("axiom:set_block", &o.0, server.split) {
            self.out.push_back(("axiom:tunnel".into(), piece));
        }
        true
    }

    /// The fly speed (vanilla's units, 0.05 normal), sent when it changes.
    pub fn fly_speed(&mut self, speed: f32) {
        let speed = speed.clamp(-1.0, 1.0);
        let Some(server) = self.active() else { return };
        if self.fly == Some(speed) || !server.packets.contains("axiom:set_fly_speed") || !self.may("axiom.player.speed") {
            return;
        }
        self.fly = Some(speed);
        let mut o = Out::default();
        o.float(speed);
        self.out.push_back(("axiom:set_fly_speed".into(), o.0));
    }

    /// The state as chat lines (`/eksen axiom`).
    pub fn lines(&self) -> Vec<String> {
        let link = match &self.link {
            Link::None => "yok (sunucuda AxiomPaper görülmedi)".to_owned(),
            Link::Answered => "el sıkışılıyor".to_owned(),
            Link::Active(s) => format!("etkin ({} paket, parça {} bayt)", s.packets.len(), s.split),
            Link::Refused(why) => format!("reddedildi: {why}"),
        };
        let mut out = vec![format!("[Eksen] AxiomPaper: {link}")];
        if self.disabled {
            out.push("Kapalı: düzenlemeler komutla gidiyor (/eksen axiom ac).".into());
        }
        if matches!(self.link, Link::Active(_)) {
            out.push(format!(
                "Blok tamponu: {} · tek blok: {} · uçuş hızı: {}",
                yes(self.may("axiom.build.section")),
                yes(self.may("axiom.build.place")),
                yes(self.may("axiom.player.speed")),
            ));
            out.push(format!("Hız sınırı: {}/{} bölüm · bekleyen {}", self.sends.max(0), self.sends_max, self.sections.len()));
        }
        out
    }

    fn next_uuid(&mut self) -> u128 {
        let mut x = self.seed ^ 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let v = (u128::from(next()) << 64) | u128::from(next());
        self.seed = x;
        // Version 4, variant 1, as `UUID.randomUUID`.
        (v & !(0xF << 76) | (0x4 << 76)) & !(0x3 << 62) | (0x2 << 62)
    }
}

fn yes(b: bool) -> &'static str {
    if b {
        "izinli"
    } else {
        "izin yok"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_end_marker_is_the_plugins() {
        assert_eq!(END as u64, 0b1000000000000000000000000010000000000000000000000000100000000000);
        assert_eq!(as_long([1, 2, 3]), (1 << 38) | (3 << 12) | 2);
    }

    #[test]
    fn varints_round_trip() {
        for v in [0, 1, 127, 128, 255, 25565, 2_097_151, i32::MAX, -1, i32::MIN] {
            let mut o = Out::default();
            o.varint(v);
            assert_eq!(In(&o.0).varint(), Some(v));
        }
        let mut o = Out::default();
        o.varint(300);
        assert_eq!(o.0, [0xAC, 0x02]);
    }

    #[test]
    fn a_container_packs_low_bits_first() {
        let mut cells = BTreeMap::new();
        cells.insert(0u16, 7);
        cells.insert(1u16, 9);
        cells.insert(4095u16, 7);
        let mut o = Out::default();
        container(&cells, 0, &mut o);
        let mut r = In(&o.0);
        assert_eq!(r.byte(), Some(4));
        assert_eq!(r.varint(), Some(3));
        assert_eq!([r.varint(), r.varint(), r.varint()], [Some(0), Some(7), Some(9)]);
        let first = r.long().unwrap() as u64;
        assert_eq!(first & 0xF, 1);
        assert_eq!((first >> 4) & 0xF, 2);
        // 16 entries a long at 4 bits: 256 longs, the last holds cell 4095.
        let mut last = 0;
        for _ in 1..256 {
            last = r.long().unwrap() as u64;
        }
        assert_eq!(last >> 60, 1);
        assert!(r.0.is_empty());
    }

    #[test]
    fn a_big_palette_widens_the_bits() {
        let cells: BTreeMap<u16, u32> = (0..40u16).map(|i| (i, u32::from(i) + 1)).collect();
        let mut o = Out::default();
        container(&cells, 0, &mut o);
        assert_eq!(o.0[0], 6);
    }

    #[test]
    fn the_tunnel_splits_and_flags() {
        let body = vec![1u8; 100];
        let pieces = tunnel("axiom:set_buffer", &body, 40);
        assert!(pieces.len() > 1);
        assert_eq!(pieces[0][0], FIRST);
        assert_eq!(pieces.last().unwrap()[0], LAST);
        assert!(pieces.iter().all(|p| p.len() <= 40));
        let joined: Vec<u8> = pieces.iter().flat_map(|p| p[1..].to_vec()).collect();
        let mut r = In(&joined);
        assert_eq!(r.utf().as_deref(), Some("axiom:set_buffer"));
        assert_eq!(r.int(), Some(100));
        assert_eq!(r.byte(), Some(0));
        assert_eq!(r.0, &body[..]);
        let one = tunnel("axiom:set_block", &[5], 31000);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0][0], FIRST | LAST);
    }

    fn enable(packets: &[&str]) -> Vec<u8> {
        let mut o = Out::default();
        o.bool(true).byte(0).varint(2).int(31000).int(2_097_152).varint(0).varint(0).varint(packets.len() as i32);
        for p in packets {
            o.utf(p);
        }
        o.0
    }

    #[test]
    fn hello_is_answered_and_enable_activates() {
        let mut a = Axiom::default();
        let mut hello = Out::default();
        hello.long(42);
        assert!(a.on_payload("axiom:hello", &hello.0, (4000, 800)));
        let (ch, data) = a.out.pop_front().unwrap();
        assert_eq!(ch, "minecraft:register");
        assert!(String::from_utf8(data).unwrap().contains("axiom:enable"));
        let (ch, data) = a.out.pop_front().unwrap();
        assert_eq!(ch, "axiom:hello");
        let mut r = In(&data);
        assert_eq!([r.varint(), r.varint(), r.varint()], [Some(API_VERSION), Some(4000), Some(800)]);
        assert_eq!(r.long(), Some(42));
        assert!(!a.on_payload("voicechat:secret", &[], (0, 0)));

        a.on_payload("axiom:enable", &enable(&["axiom:set_buffer", "axiom:tunnel"]), (0, 0));
        assert!(a.active().is_some());
        assert!(!a.takes_edits(), "no permissions yet");
        let mut rs = Out::default();
        rs.varint(1).utf("axiom.all").varint(0).int(-1).varint(0);
        a.on_payload("axiom:restrictions", &rs.0, (0, 0));
        assert!(a.takes_edits());
        a.disabled = true;
        assert!(!a.takes_edits());
    }

    #[test]
    fn permissions_follow_groups_and_denials() {
        let a = Axiom {
            allowed: vec!["axiom.build.*".into(), "axiom.player.speed".into()],
            denied: vec!["axiom.build.nbt".into()],
            ..Axiom::default()
        };
        assert!(a.may("axiom.build.section"));
        assert!(!a.may("axiom.build.nbt"));
        assert!(a.may("axiom.player.speed"));
        assert!(!a.may("axiom.player.teleport"));
    }

    #[test]
    fn edits_wait_for_the_rate_limit() {
        let mut a = Axiom::default();
        a.on_payload("axiom:enable", &enable(&["axiom:set_buffer"]), (0, 0));
        let mut sends = Out::default();
        sends.varint(1).varint(1024);
        a.on_payload("axiom:update_available_dispatch_sends", &sends.0, (0, 0));
        let edit: BTreeMap<Pos, u32> = [([0, 0, 0], 1), ([20, 0, 0], 1), ([0, 40, 0], 2)].into();
        a.queue_edit("minecraft:overworld", &edit, 0);
        assert_eq!(a.waiting(), 3);
        a.tick();
        assert_eq!(a.waiting(), 2, "one send allowed");
        assert_eq!(a.out.len(), 1);
        a.tick();
        assert_eq!(a.waiting(), 2, "none left until the server adds");
    }

    #[test]
    fn a_section_with_many_kinds_goes_in_parts() {
        let mut a = Axiom::default();
        let edit: BTreeMap<Pos, u32> = (0..300).map(|i| ([i % 16, i / 256, (i / 16) % 16], i as u32 + 1)).collect();
        a.queue_edit("minecraft:overworld", &edit, 0);
        assert_eq!(a.waiting(), 2);
    }

    #[test]
    fn uuids_are_version_four() {
        let mut a = Axiom::default();
        let u = a.next_uuid();
        assert_eq!((u >> 76) & 0xF, 4);
        assert_eq!((u >> 62) & 0x3, 2);
        assert_ne!(u, a.next_uuid());
    }
}
