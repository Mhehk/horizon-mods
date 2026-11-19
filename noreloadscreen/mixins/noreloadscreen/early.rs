// Remove Reloading Screen — mhv2 (mihehk), port of RRLS by dima_dencep (OSL-3.0):
// `earlyPackStatusSend` (ServerPackManagerMixin). The game side sets EARLY each
// frame from the config.
use std::sync::atomic::{AtomicBool, Ordering};

pub static EARLY: AtomicBool = AtomicBool::new(false);

/// `pushNewPack`, at `registerForUpdate`: the server hears at once that the
/// pack loaded, before it is downloaded.
pub fn push(packs: &mut super::ServerPacks, id: u128) {
    if EARLY.load(Ordering::Relaxed) && packs.prompt != super::Prompt::Declined {
        packs.report(id, super::action::SUCCESSFULLY_LOADED);
    }
}
