// Hesaplar — 16.10.1 (mihehk): the mod's state and its reach into the game
// and the machine. Out of a world, the menu screens show the account button
// (your head and name) in the top right; it drops the account menu down:
// pick an account to switch, "+ Hesap ekle" → "Tarayıcıyı aç" signs a new one
// in on Microsoft's page, hover an account and press × → "Kaldır" removes it.
//
// Sign-ins run on a thread of their own so the menu never stalls. The
// browser comes back to a one-shot listener on localhost; HTTP goes through
// Windows' own curl.exe (Windows 10 and later), its settings fed on stdin so
// no token shows on a command line; the account list is encrypted with
// Windows DPAPI (CurrentUser) through PowerShell, so only this Windows user
// can read %APPDATA%\horizon\hesaplar.dat. All std, nothing added to
// Horizon's build. What touches Horizon is marked UNVERIFIED.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::hesaplar_core::{self as core, Account, Click, Page, Request};

pub const ID: &str = "hesaplar";
pub const NAME: &str = "Hesaplar";
pub const VERSION: &str = "16.10.1";
pub const DESCRIPTION: &str =
    "Ana menüde Microsoft hesapları arasında geçiş: sağ üstteki hesap düğmesi. Hesap tarayıcıda Microsoft girişiyle eklenir; liste Windows şifrelemesiyle saklanır. Offline hesap yok.";
pub const PREFIX: &str = "[Hesaplar] ";

/// No keys: the account button is the way in, as Essential's.
pub const KEYS: [(&str, &str, &str); 0] = [];

/// How long the browser has to come back.
const BROWSER_WAIT: Duration = Duration::from_secs(300);

enum Event {
    Status(String),
    /// Signed in: the account (to save) and its game token.
    Ready(Account, String),
    Failed(String),
}

#[derive(Default)]
pub struct State {
    pub accounts: Vec<Account>,
    loaded: bool,
    job: Option<Receiver<Event>>,
    pub page: Page,
    pub status: String,
    /// The Azure client id while it is being typed.
    editing: Option<String>,
    pub open_asked: bool,
    pub close_asked: bool,
    /// The screen's size as last drawn (clicks are laid out against it).
    size: [f32; 2],
    /// The account button where it was last drawn.
    button: [f32; 4],
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub struct Guard(MutexGuard<'static, Option<State>>);

impl std::ops::Deref for Guard {
    type Target = State;
    fn deref(&self) -> &State {
        self.0.as_ref().expect("made in state()")
    }
}

impl std::ops::DerefMut for Guard {
    fn deref_mut(&mut self) -> &mut State {
        self.0.as_mut().expect("made in state()")
    }
}

pub fn state() -> Guard {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(State::default);
    if !s.loaded {
        s.loaded = true;
        match load() {
            Ok(list) => s.accounts = list,
            Err(e) => s.status = e,
        }
    }
    Guard(g)
}

// ---- the machine: files, DPAPI, curl, the browser ------------------------

/// Windows' "no console window" flag, so curl and PowerShell never flash.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn command(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn run(program: &str, args: &[&str], input: &str) -> Result<String, String> {
    let mut child = command(program, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program} açılamadı: {e}"))?;
    child.stdin.take().ok_or("stdin yok")?.write_all(input.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("{program}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

const PROTECT: &str = "Add-Type -AssemblyName System.Security; $t = [Console]::In.ReadToEnd(); \
    [Convert]::ToBase64String([Security.Cryptography.ProtectedData]::Protect([Text.Encoding]::UTF8.GetBytes($t), $null, 'CurrentUser'))";
const UNPROTECT: &str = "Add-Type -AssemblyName System.Security; $t = [Console]::In.ReadToEnd().Trim(); \
    [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($t), $null, 'CurrentUser'))";

fn powershell(script: &str, input: &str) -> Result<String, String> {
    run("powershell.exe", &["-NoProfile", "-NonInteractive", "-Command", script], input)
}

fn data_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("horizon").join("hesaplar.dat"))
}

fn load() -> Result<Vec<Account>, String> {
    let Some(path) = data_file() else { return Ok(Vec::new()) };
    let Ok(blob) = std::fs::read_to_string(&path) else { return Ok(Vec::new()) };
    let text = powershell(UNPROTECT, &blob).map_err(|_| "Kayıtlı hesaplar açılamadı (başka bir Windows kullanıcısının dosyası mı?).".to_string())?;
    Ok(core::read_accounts(&text))
}

fn save(list: &[Account]) -> Result<(), String> {
    let path = data_file().ok_or("APPDATA bulunamadı.")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let blob = powershell(PROTECT, &core::write_accounts(list))?;
    std::fs::write(&path, blob.trim()).map_err(|e| e.to_string())
}

/// A curl config value: quoted, with `\` and `"` escaped.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Runs `req`: the body (Microsoft's and Xbox's errors come as bodies too).
fn body(req: Request) -> Result<String, String> {
    let mut cfg = format!("url = {}\nrequest = {}\n", quoted(&req.url), req.method);
    for h in &req.headers {
        cfg += &format!("header = {}\n", quoted(h));
    }
    if let Some(b) = &req.body {
        cfg += &format!("data-binary = {}\n", quoted(b));
    }
    let out = run("curl.exe", &["--silent", "--show-error", "--max-time", "20", "--config", "-", "--write-out", "\n%{http_code}"], &cfg)
        .map_err(|_| "İnternete bağlanılamadı.".to_string())?;
    let (body, code) = out.rsplit_once('\n').unwrap_or(("", &out));
    if code.trim() == "429" {
        return Err("Çok fazla deneme; bir dakika bekle.".into());
    }
    Ok(body.to_owned())
}

/// A random URL-safe string (PKCE verifier, state): std's RandomState is
/// seeded from the OS's randomness.
fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut s = String::new();
    let mut n = Instant::now().elapsed().as_nanos();
    while s.len() < 64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(n);
        n = n.wrapping_add(1);
        s += &format!("{:016x}", h.finish());
    }
    s
}

fn open_browser(url: &str) -> Result<(), String> {
    command("rundll32.exe", &["url.dll,FileProtocolHandler", url]).spawn().map(|_| ()).map_err(|_| "Tarayıcı açılamadı.".to_string())
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Hesaplar</title>\
    <body style=\"font-family:sans-serif;background:#14161c;color:#eee;text-align:center;padding-top:15vh\">\
    <h2>Giriş tamamlandı</h2><p>Bu sekmeyi kapatıp oyuna dönebilirsin.</p>";

fn answer(stream: &mut TcpStream, status: &str, page: &str) {
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len());
}

/// Waits for the browser to come back to `listeners`: the code.
fn wait_for_browser(listeners: &[TcpListener], state: &str) -> Result<String, String> {
    let until = Instant::now() + BROWSER_WAIT;
    while Instant::now() < until {
        for l in listeners {
            let Ok((mut stream, _)) = l.accept() else { continue };
            let _ = stream.set_nonblocking(false);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]);
            let line = head.lines().next().unwrap_or("");
            // The browser also asks for /favicon.ico and the like.
            if !line.contains("?") {
                answer(&mut stream, "404 Not Found", "");
                continue;
            }
            let got = core::parse_redirect(line, state);
            answer(&mut stream, "200 OK", if got.is_ok() { DONE_PAGE } else { "<meta charset=utf-8>Giriş tamamlanamadı; oyuna dön." });
            return got;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Tarayıcıdan cevap gelmedi; yeniden dene.".into())
}

// ---- sign-in, on its own thread ------------------------------------------

/// From a Microsoft token to the game: Xbox Live, XSTS, Minecraft, profile.
fn finish(ms_token: &str, refresh: String, tx: &Sender<Event>) -> Result<(Account, String), String> {
    let _ = tx.send(Event::Status("Xbox Live...".into()));
    let (xbl, uhs) = core::parse_xbox_live(&body(core::xbox_live(ms_token))?)?;
    let xsts = core::parse_xsts(&body(core::xsts(&xbl))?)?;
    let _ = tx.send(Event::Status("Minecraft...".into()));
    let (mc, _) = core::parse_minecraft(&body(core::minecraft_login(&uhs, &xsts))?)?;
    let (name, uuid) = core::parse_profile(&body(core::profile(&mc))?)?;
    Ok((Account { name, uuid, refresh }, mc))
}

fn add_account(client_id: &str, tx: &Sender<Event>) -> Result<(Account, String), String> {
    // localhost may be 127.0.0.1 or ::1 to the browser: listen on both.
    let v4 = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("Yerel port açılamadı: {e}"))?;
    let port = v4.local_addr().map_err(|e| e.to_string())?.port();
    let mut listeners = vec![v4];
    if let Ok(v6) = TcpListener::bind(("::1", port)) {
        listeners.push(v6);
    }
    for l in &listeners {
        l.set_nonblocking(true).map_err(|e| e.to_string())?;
    }
    let redirect = format!("http://localhost:{port}");
    let (verifier, state) = (random_token(), random_token());
    open_browser(&core::authorize_url(client_id, &redirect, &verifier, &state))?;
    let _ = tx.send(Event::Status("Tarayıcıda giriş bekleniyor...".into()));
    let code = wait_for_browser(&listeners, &state)?;
    let _ = tx.send(Event::Status("Microsoft...".into()));
    let (access, refresh) = core::parse_tokens(&body(core::exchange(client_id, &code, &redirect, &verifier))?)?;
    finish(&access, refresh, tx)
}

fn sign_in_again(client_id: &str, a: &Account, tx: &Sender<Event>) -> Result<(Account, String), String> {
    let _ = tx.send(Event::Status(format!("{} giriş yapıyor...", a.name)));
    let (access, refresh) = core::parse_tokens(&body(core::refresh(client_id, &a.refresh))?)?;
    finish(&access, refresh, tx)
}

fn start(s: &mut State, job: impl FnOnce(&Sender<Event>) -> Result<(Account, String), String> + Send + 'static) {
    let (tx, rx) = channel();
    s.job = Some(rx);
    std::thread::spawn(move || {
        let _ = tx.send(match job(&tx) {
            Ok((a, token)) => Event::Ready(a, token),
            Err(e) => Event::Failed(e),
        });
    });
}

/// Each frame: what the sign-in thread said (a finished sign-in is saved
/// and Horizon switches to it); the menu opens or closes when asked.
pub(crate) fn frame(game: &mut super::Game<'_>) {
    let ready = {
        let mut s = state();
        let mut events = Vec::new();
        let mut ended = false;
        if let Some(rx) = &s.job {
            loop {
                match rx.try_recv() {
                    Ok(e) => events.push(e),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                }
            }
        }
        let mut ready = None;
        for e in events {
            match e {
                Event::Status(t) => s.status = t,
                Event::Failed(t) => {
                    s.status = t;
                    ended = true;
                }
                Event::Ready(a, token) => {
                    core::upsert(&mut s.accounts, a.clone());
                    if let Err(e) = save(&s.accounts) {
                        s.status = format!("Hesap kaydedilemedi: {e}");
                    }
                    s.page = Page::List;
                    ready = Some((a, token));
                    ended = true;
                }
            }
        }
        if ended {
            s.job = None;
        }
        ready
    };
    if let Some((a, token)) = ready {
        let said = match horizon_switch(game, &a, &token) {
            Ok(()) => format!("{} hesabına geçildi.", a.name),
            Err(e) => format!("Hesap kaydedildi ama geçilemedi: {e}"),
        };
        state().status = said;
    }
    let (open, close) = {
        let mut s = state();
        (std::mem::take(&mut s.open_asked), std::mem::take(&mut s.close_asked))
    };
    let ours = game.native_mod_screen() == Some(ID);
    if open && !ours && !in_world(game) {
        game.native_mod_screen_open(ID);
    }
    if close && ours {
        game.native_mod_screen_close(ID);
    }
}

// ---- the menu -------------------------------------------------------------

/// Playing on a server or in a single player world (both are fields other
/// mods read: `remote`, `has_singleplayer_server`).
pub fn in_world(game: &super::Game<'_>) -> bool {
    game.remote.is_some() || game.has_singleplayer_server()
}

/// The account button shows on the menu screens out of a world (another
/// mod's screen aside). Essential shows it on the main menu; Horizon has no
/// call we know that names the screen, so every menu screen gets it.
pub fn shows_button(game: &super::Game<'_>) -> bool {
    game.native_on(ID) && !in_world(game) && game.native_mod_screen().is_none_or(|s| s == ID)
}

fn client_id(game: &super::Game<'_>) -> String {
    game.native_setting(ID, "client_id").unwrap_or_default()
}

fn with_view<R>(game: &super::Game<'_>, s: &State, f: impl FnOnce(&core::View) -> R) -> R {
    let id = client_id(game);
    let shown = s.editing.as_deref().unwrap_or(&id);
    let view = core::View {
        page: s.page,
        accounts: &s.accounts,
        current: horizon_current(game),
        status: &s.status,
        client_id: (shown, s.editing.is_some()),
        waiting: s.job.is_some(),
    };
    f(&view)
}

fn paint(game: &super::Game<'_>, batch: &mut horizon_client::hud::HudBatch, draws: Vec<core::Draw>) {
    for d in draws {
        match d {
            core::Draw::Fill(r, argb) => game.native_fill(batch, r, argb),
            core::Draw::Text(t, at, rgb, shadow) => game.native_text(batch, &t, at, rgb, shadow),
        }
    }
}

/// Any menu screen drawn: the account button, and the menu when it is open.
pub(crate) fn screen(game: &super::Game<'_>, batch: &mut horizon_client::hud::HudBatch, mouse: [f32; 2]) {
    if !shows_button(game) {
        return;
    }
    let size = [batch.width, batch.height];
    let measure = |t: &str| game.native_text_width(t);
    let open = game.native_mod_screen() == Some(ID);
    let draws = {
        let mut s = state();
        s.size = size;
        let current = horizon_current(game).and_then(|u| s.accounts.iter().find(|a| a.uuid == u));
        let name = current.map_or("Hesap", |a| a.name.as_str());
        let button = core::account_button(size, measure(name));
        let mut d = core::draw_button(size, current, "Hesap", core::on_button(button, mouse) || open, &measure);
        s.button = button;
        if open {
            d.extend(with_view(game, &s, |v| core::draw(v, size, mouse)));
        }
        d
    };
    paint(game, batch, draws);
}

fn store_client_id(game: &super::Game<'_>, s: &mut State) {
    if let Some(id) = s.editing.take() {
        game.native_set_setting(ID, "client_id", id.trim());
    }
}

/// A left click on a menu screen. True when it was ours (the game's own
/// buttons then do not see it).
pub(crate) fn press(game: &super::Game<'_>, at: [f32; 2]) -> bool {
    if !shows_button(game) {
        return false;
    }
    let mut s = state();
    let (size, button) = (s.size, s.button);
    if game.native_mod_screen() != Some(ID) {
        if core::on_button(button, at) {
            s.page = Page::List;
            s.open_asked = true;
            return true;
        }
        return false;
    }
    let (click, on_menu) = with_view(game, &s, |v| (core::click(v, size, at), core::on_menu(v, size, at, button)));
    if click != Click::ClientId {
        store_client_id(game, &mut s);
    }
    let busy = s.job.is_some();
    match click {
        Click::Close => s.close_asked = true,
        Click::Nothing => {
            // Outside the drop-down, or on the account button again: it closes.
            if !on_menu || core::on_button(button, at) {
                s.close_asked = true;
            }
        }
        Click::ClientId => {
            if s.editing.is_none() {
                s.editing = Some(client_id(game));
            }
        }
        Click::Add => {
            s.page = Page::Add;
            s.status.clear();
        }
        Click::Cancel => {
            s.page = Page::List;
            s.status.clear();
        }
        Click::AskRemove(i) => s.page = Page::Confirm(i),
        Click::Remove(i) => {
            if i < s.accounts.len() {
                let gone = s.accounts.remove(i);
                s.status = match save(&s.accounts) {
                    Ok(()) => format!("{} kaldırıldı.", gone.name),
                    Err(e) => format!("Kaydedilemedi: {e}"),
                };
            }
            s.page = Page::List;
        }
        Click::OpenBrowser | Click::Switch(_) => {
            if let Err(why) = core::may_switch(in_world(game), busy) {
                s.status = why.into();
                return true;
            }
            let id = client_id(game);
            if id.trim().is_empty() {
                s.page = Page::Add;
                s.status = "Önce Azure uygulama kimliğini yaz (README).".into();
                return true;
            }
            if let Click::Switch(i) = click {
                let a = s.accounts[i].clone();
                if horizon_current(game) == Some(a.uuid) {
                    return true;
                }
                start(&mut s, move |tx| sign_in_again(&id, &a, tx));
            } else {
                start(&mut s, move |tx| add_account(&id, tx));
            }
        }
    }
    true
}

/// A character typed: into the client id while it is being edited.
pub(crate) fn char(c: char) -> bool {
    let mut s = state();
    let Some(field) = s.editing.as_mut() else { return false };
    if (c.is_ascii_alphanumeric() || c == '-') && field.len() < 64 {
        field.push(c);
    }
    true
}

/// A key on the menu: Backspace and Enter edit the client id. True when
/// the key was the menu's.
pub(crate) fn key(game: &super::Game<'_>, key: &str) -> bool {
    let mut s = state();
    if s.editing.is_none() {
        return false;
    }
    match key {
        "key.keyboard.backspace" => {
            s.editing.as_mut().map(String::pop);
            true
        }
        "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
            store_client_id(game, &mut s);
            true
        }
        _ => false,
    }
}

// ---- Horizon guesses (UNVERIFIED: Horizon's source is private) ----------
// No mod we have touches Horizon's sign-in, so both of these are guesses:
// if the first build fails, these are the lines to fix.

/// The UUID Horizon plays as now.
fn horizon_current(game: &super::Game<'_>) -> Option<u128> {
    game.account.as_ref().map(|a| a.uuid)
}

/// Horizon plays as `a` from now on (the next server joined uses it).
fn horizon_switch(game: &mut super::Game<'_>, a: &Account, token: &str) -> Result<(), String> {
    game.account = Some(horizon_client::auth::Account { name: a.name.clone(), uuid: a.uuid, access_token: token.to_owned() });
    Ok(())
}
