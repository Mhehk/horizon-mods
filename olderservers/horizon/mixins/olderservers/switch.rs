// Older Servers — mhv1 (mihehk): Horizon's own adapter for servers of older
// releases (1.20.2 to 26.2, for example the 1.21.11 servers Fabric players with
// Simple Voice Chat are on), on while this mod is on instead of only with the
// HORIZON_OLDER_SERVERS=1 environment variable. The version is still found by
// the server's status ping; a 26.3 server is joined as before.
use std::sync::atomic::AtomicBool;

/// Set each frame from the mod's row in Options > Mods.
pub static ON: AtomicBool = AtomicBool::new(true);
