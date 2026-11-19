// A server pushed a pack: with earlyPackStatusSend it is reported loaded at once.
fn noreloadscreen_push(packs: &mut ServerPacks, id: u128) {
    crate::server_packs::rrls_early::push(packs, id);
}
