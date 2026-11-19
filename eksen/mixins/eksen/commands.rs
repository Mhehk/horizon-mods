// Lists `/eksen` so the chat box colours and completes it.
fn eksen_commands(commands: &mut Vec<&'static str>) {
    if !commands.contains(&"eksen") {
        commands.push("eksen");
    }
}
