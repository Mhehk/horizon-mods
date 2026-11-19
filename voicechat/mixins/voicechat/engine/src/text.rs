//! Every line the voice chat's screens show, worded as Simple Voice Chat's
//! English interface words them, so a player finds the same labels in both.

pub const TITLE: &str = "Voice Chat";
pub const SETTINGS_TITLE: &str = "Voice Chat Settings";
pub const SETTINGS: &str = "Settings";
pub const GROUP: &str = "Group";
pub const ADJUST_VOLUMES: &str = "Adjust Volumes";
pub const SELECT_MICROPHONE: &str = "Select Microphone";
pub const SELECT_SPEAKER: &str = "Select Speaker";
pub const BACK: &str = "Back";
pub const DEFAULT_DEVICE: &str = "System Default";
pub const NO_MICROPHONE: &str = "No microphones available";
pub const NO_SPEAKER: &str = "No speakers available";

pub const JOIN_CREATE_GROUP: &str = "Join or Create Group";
pub const CREATE_GROUP: &str = "Create Group";
pub const CREATE: &str = "Create";
pub const GROUP_NAME: &str = "Group Name";
pub const OPTIONAL_PASSWORD: &str = "Password (Optional)";
pub const NO_GROUPS: &str = "No existing groups";
pub const GROUP_MEMBERS: &str = "Group members:";
pub const NO_GROUP_MEMBERS: &str = "Empty group";
pub const ENTER_GROUP_PASSWORD: &str = "Enter Group Password";
pub const PASSWORD: &str = "Password";
pub const JOIN_GROUP: &str = "Join Group";
pub const LEAVE_GROUP: &str = "Press to leave the group.";
pub const WRONG_PASSWORD: &str = "Incorrect Password";
pub const GROUPS_DISABLED: &str = "Groups are disabled on this server";
pub const NOT_CONNECTED: &str = "Voice chat not connected";
pub const UNAVAILABLE: &str = "Voice chat unavailable";

pub const SEARCH_HINT: &str = "Search...";
pub const SEARCH_EMPTY: &str = "Couldn't find any players with that name";
pub const OTHER_VOLUME: &str = "Other";
pub const OTHER_VOLUME_DESCRIPTION: &str = "The volume of any sound not listed below.";

pub const MUTE_ENABLED: &str = "Press to unmute your microphone.";
pub const MUTE_DISABLED: &str = "Press to mute your microphone.";
pub const MUTE_PTT: &str = "You can't mute your microphone when using push to talk.";
pub const DISABLE_ENABLED: &str = "Press to enable all voice chat sounds.";
pub const DISABLE_DISABLED: &str = "Press to disable all voice chat sounds.";
pub const HIDE_ENABLED: &str = "Press to show all voice chat icons.";
pub const HIDE_DISABLED: &str = "Press to hide all voice chat icons.";
pub const GROUP_HUD_ENABLED: &str = "Press to hide the group chat HUD.";
pub const GROUP_HUD_DISABLED: &str = "Press to show the group chat HUD.";
pub const RECORDING_DISABLED: &str = "Press to start recording.";
pub const RECORDING_ENABLED: &str = "Press to stop recording.";
pub const RECORDING_OFF_SERVER: &str = "Recording is disabled on this server";
pub const MIC_TEST_DISABLED: &str = "Press to enable microphone testing.";
pub const MIC_TEST_ENABLED: &str = "Press to disable microphone testing.";
pub const MIC_TEST_UNAVAILABLE: &str = "Microphone testing unavailable!";
pub const TEST_SPEAKER: &str = "Press to play a test sound.";
pub const PRESS_TO_ASSIGN: &str = "Press to assign a key.";
pub const VOICE_ACTIVATION_OFF: &str = "0 dB deactivates the microphone!";
pub const GAIN_WARNING: &str = "Exceeding 10 dB amplification may produce audio distortion and other issues!";
pub const GAIN_MANUAL_WARNING: &str =
    "Please keep automatic microphone gain adjustment enabled to ensure consistent volume levels for everyone.";

pub fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

pub fn voice_chat_volume(percent: u32) -> String {
    format!("Voice Chat Volume: {percent}%")
}

pub fn gain(auto: bool) -> String {
    format!("Gain: {}", if auto { "Auto" } else { "Manual" })
}

pub fn microphone_gain(db: f32) -> String {
    format!("Microphone Gain: {} dB", db.round() as i32)
}

pub fn denoiser(on: bool) -> String {
    format!("Noise Suppression: {}", on_off(on))
}

pub fn activation(voice: bool) -> String {
    format!("Activation Method: {}", if voice { "Voice" } else { "Push to Talk" })
}

pub fn vad(auto: bool) -> String {
    format!("Voice Activation: {}", if auto { "Auto" } else { "Manual" })
}

pub fn threshold(db: i32) -> String {
    format!("Voice Activation Threshold: {db} dB")
}

pub fn audio_type(name: &str) -> String {
    format!("3D Audio: {name}")
}

pub const AUDIO_TYPES: [&str; 3] = ["Normal", "Reduced", "OFF"];

pub fn ptt_key(key: &str) -> String {
    format!("Push to Talk: {key}")
}

pub fn amplification(percent: u32) -> String {
    format!("Amplification: {percent}%")
}

pub fn group_title(name: &str) -> String {
    name.to_owned()
}

pub fn group_type_title(name: &str, kind: i16) -> String {
    format!("{name} ({})", GROUP_TYPES[kind.rem_euclid(3) as usize])
}

pub const GROUP_TYPE: &str = "Group Type";
pub const GROUP_TYPES: [&str; 3] = ["Normal", "Open", "Isolated"];
pub const GROUP_TYPE_DESCRIPTIONS: [&str; 3] = [
    "Players who are not in your group can't hear you, but you can hear them.",
    "Players who are not in your group can hear you, and you can hear them too.",
    "Players who are not in your group can't hear you, and you can't hear them either.",
];

pub fn group_type(kind: i16) -> String {
    format!("{GROUP_TYPE}: {}", GROUP_TYPES[kind.rem_euclid(3) as usize])
}

pub fn more_members(n: usize) -> String {
    format!("and {n} more...")
}

pub fn storage(size: &str) -> String {
    format!("Estimated disk space usage: {size}.")
}

// Onboarding (the first-time setup guide).
pub const ONBOARD_NEXT: &str = "Next";
pub const ONBOARD_BACK: &str = "Back";
pub const ONBOARD_CANCEL: &str = "Cancel";
pub const ONBOARD_CONFIRM: &str = "Confirm";
pub const ONBOARD_TITLE: &str = "Set Up Simple Voice Chat";
pub const ONBOARD_INTRO: &str = "This setup guide will help you configure your voice chat.\n\nYou can skip it if you know what you are doing.";
pub const ONBOARD_SKIP: &str = "I Know What I Am Doing - Skip";
pub const ONBOARD_SKIP_TITLE: &str = "Skip Setup";
pub const ONBOARD_SKIP_DESCRIPTION: &str =
    "Are you sure you want to skip this setup guide?\n\nIf you skip it, you will have to adjust all the settings manually.";
pub const ONBOARD_MICROPHONE: &str = "Select Microphone";
pub const ONBOARD_SPEAKER: &str = "Select Speaker";
pub const ONBOARD_ACTIVATION_TITLE: &str = "Select Activation Method";
pub const ONBOARD_ACTIVATION: &str = "Please choose the method to activate your microphone.\n\nWhen selecting Push to Talk, you need to press and hold a button to speak.\n\nVoice Activation automatically activates your microphone when you speak.";
pub const PUSH_TO_TALK: &str = "Push to Talk";
pub const VOICE_ACTIVATION: &str = "Voice Activation";
pub const ONBOARD_PTT_TITLE: &str = "Choose Push to Talk Keybind";
pub const ONBOARD_PTT: &str = "Please choose the key that should be used to activate your microphone.";
pub const ONBOARD_PTT_BUTTON: &str = "Press to assign";
pub const ONBOARD_VOICE_TITLE: &str = "Adjust Voice Activation";
pub const ONBOARD_VOICE: &str = "Enable microphone testing and make sure you can hear your voice when speaking.\nIf this doesn't work, use manual voice activation and adjust the slider until your voice is detected.";
pub const ONBOARD_FINAL_TITLE: &str = "Finish Setup";
pub const ONBOARD_FINISH: &str = "Finish Setup";

pub fn onboard_final(menu_key: &str, ptt: Option<&str>, mute_key: &str) -> String {
    let mut s = format!("You have successfully configured your voice chat!\n\nOpen the voice chat menu with {menu_key}.");
    match ptt {
        Some(k) => s.push_str(&format!("\n\nHold {k} while speaking to use the voice chat.")),
        None => s.push_str(&format!("\n\nYou will be muted after exiting this screen. Press {mute_key} to unmute.")),
    }
    s.push_str("\n\nYou can always change all the settings in the voice chat menu. Keybinds, however, can be changed just in the Minecraft controls settings.");
    s
}

pub fn set_up(menu_key: &str) -> String {
    format!("Press {menu_key} to set up")
}

// Chat lines.
pub const ICONS_HIDDEN: &str = "Voice chat icons hidden";
pub const ICONS_VISIBLE: &str = "Voice chat icons visible";
pub const RECORDING_STARTED: &str = "Started Recording";
pub const RECORDING_STOPPED: &str = "Stopped Recording";

pub fn join_successful(name: &str) -> String {
    format!("Successfully joined the group {name}")
}

pub const LEAVE_SUCCESSFUL: &str = "Successfully left the group";

pub fn save_session(path: &str) -> String {
    format!("Saved session to {path}")
}
