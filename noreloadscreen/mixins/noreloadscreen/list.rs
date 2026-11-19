// Remove Reloading Screen's row in Options > Mods (Mod Menu's entry).
fn noreloadscreen_listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: "noreloadscreen".into(),
        title: "Remove Reloading Screen — mhv2".into(),
        description: "Removes the reloading screen: resources load in the background while a small progress bar fades out. Port of Remove Reloading Screen (RRLS) by dima_dencep (OSL-3.0).".into(),
        keys: Vec::new(),
    });
}
