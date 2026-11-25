// Lists `/pusula` so the chat box colours and completes it.
fn pusula_commands(commands: &mut Vec<&'static str>) {
    if !commands.contains(&"pusula") {
        commands.push("pusula");
    }
}