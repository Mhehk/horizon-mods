// Every play packet from the server: the AxiomPaper plugin's payloads
// (`axiom:*`) go to Eksen's link with it (axiom.rs). Nothing is cancelled.
fn eksen_packet(packet: &horizon_protocol::packets::play::Clientbound) {
    use crate::game::eksen as m;
    if let horizon_protocol::packets::play::Clientbound::CustomPayload(p) = packet {
        if p.channel.starts_with("axiom:") {
            // The data version only shows in the plugin's mismatch message.
            m::state().axiom().on_payload(&p.channel, &p.data, (0, m::protocol_version()));
        }
    }
}
