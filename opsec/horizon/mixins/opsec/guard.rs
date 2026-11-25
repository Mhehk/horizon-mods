// OpSec — 16.10.1 (mihehk): a Horizon port of Aurick's OpSec (GPL-3.0-only),
// its resource pack protection. Horizon is not Fabric, so OpSec's mod list,
// channel and translation key protections have nothing to hide here; what a
// server can still do to any client is probe it with resource packs:
//
// - TrackPack: many pack hashes sent to few URLs in a burst, to learn from the
//   answers which packs the client already has cached, and so who it is
//   across servers (`TrackPackDetector`, same thresholds). Once the pattern
//   shows, every further pack of the burst is answered "failed download"
//   without touching the cache, so the answers say nothing.
// - Local probing: a server on the internet pointing the client at a pack on
//   its own machine or network (127.0.0.1, 192.168.x, …) to scan its ports
//   (`LocalAddressUtil`). A server that is itself local may do so: LAN
//   servers are real.
//
// Added to `game::remote` (`horizon-mixin.toml`, `[[module]]`), whose private
// fields its children can read.

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::time::{Duration, Instant};

pub(crate) const ID: &str = "opsec";
pub(crate) const VERSION: &str = "16.10.1";

// `OpsecConstants.Detection` and `.Limits`.
const DETECTION_WINDOW: Duration = Duration::from_millis(5000);
const RAPID_WINDOW: Duration = Duration::from_millis(1000);
const RAPID_INTERVAL: Duration = Duration::from_millis(200);
const FINGERPRINT_THRESHOLD: usize = 10;
const RAPID_THRESHOLD: usize = 6;
const UNIQUE_HASH_THRESHOLD: usize = 3;
const HASHES_PER_URL_THRESHOLD: f64 = 3.0;
const MAX_RECENT: usize = 100;
const MAX_UNIQUE: usize = 50;

/// What becomes of a pack the server pushes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Horizon handles it as usual.
    Allow,
    /// Answered as a failed download, for this reason (shown in the chat).
    Block(String),
}

/// `TrackPackDetector`: the server's recent pack requests.
#[derive(Default)]
pub(crate) struct Detector {
    recent: VecDeque<(String, String, Instant)>,
    hashes: HashSet<String>,
    urls: HashSet<String>,
    last: Option<Instant>,
    rapid: usize,
    notified: bool,
}

impl Detector {
    /// `recordRequest`.
    pub(crate) fn record(&mut self, url: &str, hash: &str, now: Instant) {
        while self
            .recent
            .front()
            .is_some_and(|(_, _, at)| now.duration_since(*at) > DETECTION_WINDOW || self.recent.len() >= MAX_RECENT)
        {
            self.recent.pop_front();
        }
        if self.recent.is_empty() {
            self.hashes.clear();
            self.urls.clear();
        }
        if !hash.is_empty() {
            if self.hashes.len() >= MAX_UNIQUE {
                self.hashes.clear();
            }
            self.hashes.insert(hash.to_owned());
        }
        if !url.is_empty() {
            if self.urls.len() >= MAX_UNIQUE {
                self.urls.clear();
            }
            self.urls.insert(url.to_owned());
        }
        self.recent.push_back((url.to_owned(), hash.to_owned(), now));
        self.rapid = match self.last {
            Some(last) if now.duration_since(last) < RAPID_INTERVAL => self.rapid + 1,
            _ => 0,
        };
        self.last = Some(now);
    }

    fn hashes_per_url(&self) -> bool {
        let urls = self.urls.len().max(1);
        self.hashes.len() >= UNIQUE_HASH_THRESHOLD && self.hashes.len() as f64 / urls as f64 >= HASHES_PER_URL_THRESHOLD
    }

    /// `isRapidRequestPattern`: six requests 200 ms apart, or six in a second.
    pub(crate) fn rapid(&self, now: Instant) -> bool {
        self.rapid >= RAPID_THRESHOLD
            || self.recent.iter().filter(|(_, _, at)| now.duration_since(*at) < RAPID_WINDOW).count() >= RAPID_THRESHOLD
    }

    /// `isFingerprinting`: ten requests in five seconds, three or more hashes
    /// for each URL.
    pub(crate) fn fingerprinting(&self, now: Instant) -> bool {
        let count = self.recent.iter().filter(|(_, _, at)| now.duration_since(*at) < DETECTION_WINDOW).count();
        count >= FINGERPRINT_THRESHOLD && self.hashes_per_url()
    }

    /// `consumeNotifyPatternOnce`: true the first time only.
    pub(crate) fn notify_once(&mut self) -> bool {
        !std::mem::replace(&mut self.notified, true)
    }

    /// `reset`: a new server.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The host and port of `url`, or None when it is not http(s).
pub(crate) fn host_port(url: &str) -> Option<(String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let default = match scheme.to_ascii_lowercase().as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (host, after) = v6.split_once(']')?;
        (host, after.strip_prefix(':'))
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port = match port {
        Some(p) => p.parse().ok()?,
        None => default,
    };
    (!host.is_empty()).then(|| (host.to_ascii_lowercase(), port))
}

/// `LocalAddressUtil.isLocalAddress` for one address: any, loopback, private
/// (site local), link local; for IPv6 also unique local (fc00::/7).
pub(crate) fn local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_unspecified() || v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return local_ip(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            v6.is_unspecified() || v6.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

/// An address literal, also the forms a browser takes for IPv4 that
/// `IpAddr` does not: one decimal or hex number (`2130706433`, `0x7f000001`).
fn literal(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }
    let n = match host.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None if host.bytes().all(|b| b.is_ascii_digit()) => host.parse().ok()?,
        None => return None,
    };
    Some(IpAddr::V4(Ipv4Addr::from(n)))
}

/// Whether `host` is local: by name (`localhost`), as a literal, else every
/// address it resolves to (as OpSec, `InetAddress.getAllByName`). A name that
/// does not resolve is not local: the download fails on its own.
pub(crate) fn local_host(host: &str, port: u16) -> bool {
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    if let Some(ip) = literal(host) {
        return local_ip(ip);
    }
    (host, port).to_socket_addrs().is_ok_and(|mut all| all.any(|a| local_ip(a.ip())))
}

/// The pack pushed by a server (`server_local`: whether the server itself is
/// on this machine or network) at `url` with `hash`, seen by `detector`.
pub(crate) fn decide(detector: &mut Detector, server_local: bool, url: &str, hash: &str, now: Instant) -> Verdict {
    detector.record(url, hash, now);
    if detector.fingerprinting(now) {
        return Verdict::Block("TrackPack: sunucu önbellekteki paketleri yoklayarak seni tanımaya çalışıyor".into());
    }
    if !server_local {
        if let Some((host, port)) = host_port(url).filter(|(h, p)| local_host(h, *p)) {
            return Verdict::Block(format!("sunucu yerel ağındaki bir adresi ({host}:{port}) yokluyor"));
        }
    }
    Verdict::Allow
}

thread_local! {
    static STATE: RefCell<(Detector, String, u32)> = RefCell::new((Detector::default(), String::new(), 0));
}

/// The host of the address the player joined (`host` or `host:port`).
fn server_host(address: &str) -> (String, u16) {
    host_port(&format!("http://{address}")).map_or_else(|| (address.to_owned(), 25565), |(h, _)| (h, 25565))
}

/// A `ResourcePackPush` from the server: true when OpSec answered it and
/// Horizon must not handle it.
pub(crate) fn on_packet(game: &mut crate::game::Game<'_>, packet: &super::C) -> bool {
    if !game.native_on(ID) {
        return false;
    }
    // UNVERIFIED: the variant and its fields are named as Mojang's
    // `ClientboundResourcePackPushPacket` (id, url, hash, required, prompt),
    // as Horizon names its other packets (`DamageEvent`, `HurtAnimation`).
    let super::C::ResourcePackPush(push) = packet else {
        return false;
    };
    let Some(address) = game.remote.as_ref().map(|r| r.session.address.clone()) else {
        return false;
    };
    let now = Instant::now();
    let (verdict, first, blocked) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.1 != address {
            s.0.reset();
            s.1.clone_from(&address);
        }
        let (host, port) = server_host(&address);
        let verdict = decide(&mut s.0, local_host(&host, port), &push.url, &push.hash, now);
        let first = matches!(verdict, Verdict::Block(_)) && s.0.notify_once();
        if matches!(verdict, Verdict::Block(_)) {
            s.2 += 1;
        }
        (verdict, first, s.2)
    });
    let Verdict::Block(reason) = verdict else {
        return false;
    };
    if first {
        game.native_chat(&format!("[OpSec] Kaynak paketi engellendi: {reason}."));
    }
    log::info!("OpSec: pack {} blocked ({blocked} so far): {reason}", push.url);
    // As a download that failed: accepted, then failed, as vanilla answers one.
    // UNVERIFIED: the serverbound packet and its action enum are named as
    // Mojang's `ServerboundResourcePackPacket` and its `Action`.
    use horizon_protocol::packets::common::{ResourcePack, ResourcePackAction};
    for action in [ResourcePackAction::Accepted, ResourcePackAction::FailedDownload] {
        let reply = super::S::ResourcePack(ResourcePack { id: push.id, action });
        if let Err(e) = game.native_send_packet(reply) {
            log::warn!("OpSec: the pack answer was not sent: {e}");
        }
    }
    true
}

/// `/opsec`: what OpSec has done on this server.
pub(crate) fn status() -> String {
    STATE.with(|s| {
        let s = s.borrow();
        format!("[OpSec {VERSION}] Bu sunucuda engellenen kaynak paketi: {}.", s.2)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_split_into_host_and_port() {
        assert_eq!(host_port("http://127.0.0.1:8080/p.zip"), Some(("127.0.0.1".into(), 8080)));
        assert_eq!(host_port("https://Packs.Example.com/x?y"), Some(("packs.example.com".into(), 443)));
        assert_eq!(host_port("http://user@[::1]:25565/"), Some(("::1".into(), 25565)));
        assert_eq!(host_port("file:///etc/passwd"), None);
    }

    #[test]
    fn local_addresses_in_every_spelling() {
        for host in ["localhost", "127.0.0.1", "192.168.1.5", "10.0.0.1", "172.20.0.1", "169.254.1.1", "::1", "fd00::1", "2130706433", "0x7f000001"] {
            assert!(local_host(host, 80), "{host}");
        }
        assert!(!local_host("8.8.8.8", 80));
        assert!(!local_host("2606:4700::1111", 80));
    }

    #[test]
    fn a_remote_server_may_not_probe_the_lan_but_a_lan_server_may() {
        let now = Instant::now();
        let url = "http://192.168.0.10:3000/pack.zip";
        assert!(matches!(decide(&mut Detector::default(), false, url, "a", now), Verdict::Block(_)));
        assert_eq!(decide(&mut Detector::default(), true, url, "a", now), Verdict::Allow);
        assert_eq!(decide(&mut Detector::default(), false, "https://8.8.8.8/p.zip", "a", now), Verdict::Allow);
    }

    #[test]
    fn a_trackpack_burst_is_caught_and_a_pack_stack_is_not() {
        let now = Instant::now();
        // ~24 hashes at one URL in a burst, as the proof of concept sends.
        let mut d = Detector::default();
        let verdicts: Vec<_> = (0..24)
            .map(|i| decide(&mut d, true, "http://example.com:0/", &format!("h{i}"), now + Duration::from_millis(i * 50)))
            .collect();
        assert_eq!(verdicts[..9].iter().filter(|v| **v == Verdict::Allow).count(), 9);
        assert!(verdicts[9..].iter().all(|v| matches!(v, Verdict::Block(_))));
        assert!(d.notify_once() && !d.notify_once());
        // Ten packs, each at its own URL: a real stack.
        let mut d = Detector::default();
        for i in 0..10 {
            let v = decide(&mut d, true, &format!("https://example.com/{i}.zip"), &format!("h{i}"), now);
            assert_eq!(v, Verdict::Allow);
        }
    }
}
