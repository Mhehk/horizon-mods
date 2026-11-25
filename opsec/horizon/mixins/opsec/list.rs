// OpSec's row in Options > Mods.
fn opsec_listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: "opsec".into(),
        title: "OpSec — 16.10.1".into(),
        description: "Sunucuların kaynak paketleriyle seni tanımasını engeller: önbellekteki paketleri yoklayan TrackPack saldırısını yakalar, internetteki bir sunucunun yerel ağını (127.0.0.1, 192.168.x …) yoklamasını durdurur; bu paketler indirilmeden \"indirilemedi\" diye yanıtlanır. Durum: /opsec. Aurick'in OpSec modunun Horizon portu (GPL-3.0).".into(),
        keys: Vec::new(),
    });
}
