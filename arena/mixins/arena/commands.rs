// Lists `/arena` so the chat box colours and completes it.
fn arena_commands(commands: &mut Vec<&'static str>) {
    if !commands.contains(&"arena") {
        commands.push("arena");
    }
}
