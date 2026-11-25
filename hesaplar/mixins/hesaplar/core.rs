// Hesaplar — 16.10.1 (mihehk): switch Microsoft accounts from the main menu,
// the way Essential's Account Manager does (its help page: an account button
// with your head in the main menu's top right; pick another account to
// switch; "+ Add Account" → "Open Browser" → sign in with Microsoft; hover an
// account, press its remove button, confirm "Remove"; main menu only).
// No Essential code, names or images. Plain Rust: the sign-in chain as
// requests and their answers, the saved list, and the menu's layout.
//
// Sign-in is Microsoft's browser sign-in for desktop apps (authorization
// code, PKCE, a redirect to http://localhost on a port of ours), then Xbox
// Live, XSTS, Minecraft services and the profile. Only real accounts that
// own the game: no offline accounts and no pasted tokens. The Azure client id
// is the player's own (an app they registered); none is built in.

/// An HTTP request for `curl` (state.rs runs it).
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<String>,
    pub body: Option<String>,
}

const MS: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";

fn form(url: String, pairs: &[(&str, &str)]) -> Request {
    let body = pairs.iter().map(|(k, v)| format!("{k}={}", url_encode(v))).collect::<Vec<_>>().join("&");
    Request { method: "POST", url, headers: vec!["Content-Type: application/x-www-form-urlencoded".into()], body: Some(body) }
}

fn json_post(url: &str, body: String) -> Request {
    Request {
        method: "POST",
        url: url.into(),
        headers: vec!["Content-Type: application/json".into(), "Accept: application/json".into()],
        body: Some(body),
    }
}

pub fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The page the browser opens: Microsoft's sign-in, back to `redirect`.
/// PKCE with the "plain" method (the verifier is the challenge): std has no
/// SHA-256, and the code only ever travels to our own localhost port.
pub fn authorize_url(client_id: &str, redirect: &str, verifier: &str, state: &str) -> String {
    format!(
        "{MS}/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=plain&prompt=select_account",
        url_encode(client_id),
        url_encode(redirect),
        url_encode(SCOPE),
        url_encode(state),
        url_encode(verifier)
    )
}

/// The browser came back to us: the first line of its request
/// (`GET /?code=…&state=… HTTP/1.1`). Ok(code), or why not.
pub fn parse_redirect(request_line: &str, state: &str) -> Result<String, String> {
    let target = request_line.split_whitespace().nth(1).ok_or("Tarayıcıdan bozuk cevap.")?;
    let query = target.split_once('?').map_or("", |q| q.1);
    let get = |k: &str| query.split('&').find_map(|p| p.split_once('=').filter(|kv| kv.0 == k).map(|kv| url_decode(kv.1)));
    if let Some(e) = get("error") {
        return Err(match e.as_str() {
            "access_denied" => "Giriş iptal edildi.".into(),
            _ => format!("Microsoft: {}", get("error_description").unwrap_or(e)),
        });
    }
    if get("state").as_deref() != Some(state) {
        return Err("Tarayıcıdan gelen cevap bu girişe ait değil.".into());
    }
    get("code").ok_or_else(|| "Tarayıcıdan kod gelmedi.".into())
}

/// The code from the browser, traded for tokens.
pub fn exchange(client_id: &str, code: &str, redirect: &str, verifier: &str) -> Request {
    form(
        format!("{MS}/token"),
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("redirect_uri", redirect),
            ("code_verifier", verifier),
            ("scope", SCOPE),
        ],
    )
}

/// A saved account signs in again with its refresh token.
pub fn refresh(client_id: &str, refresh_token: &str) -> Request {
    form(format!("{MS}/token"), &[("grant_type", "refresh_token"), ("client_id", client_id), ("refresh_token", refresh_token), ("scope", SCOPE)])
}

pub fn xbox_live(ms_token: &str) -> Request {
    json_post(
        "https://user.auth.xboxlive.com/user/authenticate",
        format!(
            r#"{{"Properties":{{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":"d={ms_token}"}},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}}"#
        ),
    )
}

pub fn xsts(xbl_token: &str) -> Request {
    json_post(
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        format!(r#"{{"Properties":{{"SandboxId":"RETAIL","UserTokens":["{xbl_token}"]}},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}}"#),
    )
}

pub fn minecraft_login(uhs: &str, xsts_token: &str) -> Request {
    json_post(
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        format!(r#"{{"identityToken":"XBL3.0 x={uhs};{xsts_token}"}}"#),
    )
}

pub fn profile(mc_token: &str) -> Request {
    Request {
        method: "GET",
        url: "https://api.minecraftservices.com/minecraft/profile".into(),
        headers: vec![format!("Authorization: Bearer {mc_token}")],
        body: None,
    }
}

// ---------------------------------------------------------------- answers

/// A string field anywhere in a JSON body (the answers here have each name
/// once, so a flat search is enough).
pub fn json_str(body: &str, key: &str) -> Option<String> {
    let rest = value_after(body, key)?.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?).unwrap_or('?'));
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

pub fn json_num(body: &str, key: &str) -> Option<f64> {
    let rest = value_after(body, key)?;
    let end = rest.find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn value_after<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let at = body.find(&format!("\"{key}\""))? + key.len() + 2;
    Some(body[at..].trim_start().strip_prefix(':')?.trim_start())
}

fn ms_error(body: &str) -> String {
    match json_str(body, "error").as_deref() {
        Some("invalid_client" | "unauthorized_client") => "Azure uygulama kimliği geçersiz ya da kişisel hesaplara açık değil.".into(),
        Some("invalid_grant") => "Hesabın oturumu süresi dolmuş; hesabı silip yeniden ekle.".into(),
        Some(e) => format!("Microsoft: {e}"),
        None => "Microsoft beklenmeyen bir cevap verdi.".into(),
    }
}

/// A token answer: (access token, refresh token).
pub fn parse_tokens(body: &str) -> Result<(String, String), String> {
    match (json_str(body, "access_token"), json_str(body, "refresh_token")) {
        (Some(a), Some(r)) => Ok((a, r)),
        _ => Err(ms_error(body)),
    }
}

/// Xbox Live's answer: (token, user hash).
pub fn parse_xbox_live(body: &str) -> Result<(String, String), String> {
    match (json_str(body, "Token"), json_str(body, "uhs")) {
        (Some(t), Some(u)) => Ok((t, u)),
        _ => Err("Xbox Live girişi başarısız.".into()),
    }
}

pub fn parse_xsts(body: &str) -> Result<String, String> {
    if let Some(t) = json_str(body, "Token") {
        return Ok(t);
    }
    Err(match json_num(body, "XErr").map(|e| e as u64) {
        Some(2148916233) => "Bu Microsoft hesabının Xbox profili yok; önce minecraft.net'te giriş yap.".into(),
        Some(2148916235) => "Xbox Live bu ülkede kullanılamıyor.".into(),
        Some(2148916236 | 2148916237) => "Hesabın Xbox'ta yetişkin doğrulaması gerekiyor.".into(),
        Some(2148916238) => "Çocuk hesabı: bir aile grubuna eklenmesi gerekiyor.".into(),
        Some(e) => format!("XSTS hatası {e}."),
        None => "XSTS girişi başarısız.".into(),
    })
}

/// Minecraft services' answer: the game token and seconds it lasts.
pub fn parse_minecraft(body: &str) -> Result<(String, u64), String> {
    let token = json_str(body, "access_token").ok_or("Minecraft girişi başarısız (uygulama kimliğin Mojang onaylı mı?).")?;
    Ok((token, json_num(body, "expires_in").unwrap_or(86400.0) as u64))
}

/// The profile: (name, UUID). An account without the game has none.
pub fn parse_profile(body: &str) -> Result<(String, u128), String> {
    let id = json_str(body, "id").ok_or("Bu hesapta Minecraft yok.")?;
    let uuid = u128::from_str_radix(&id.replace('-', ""), 16).map_err(|_| "Profil UUID'si okunamadı.".to_string())?;
    Ok((json_str(body, "name").unwrap_or_default(), uuid))
}

// ---------------------------------------------------------------- accounts

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub uuid: u128,
    /// Microsoft's refresh token (kept encrypted on disk, state.rs).
    pub refresh: String,
}

/// The saved list as text: one account a line, `name<TAB>uuid<TAB>refresh`.
pub fn write_accounts(list: &[Account]) -> String {
    list.iter().map(|a| format!("{}\t{:032x}\t{}\n", a.name, a.uuid, a.refresh)).collect()
}

pub fn read_accounts(text: &str) -> Vec<Account> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let (name, uuid, refresh) = (f.next()?, f.next()?, f.next()?);
            Some(Account { name: name.into(), uuid: u128::from_str_radix(uuid, 16).ok()?, refresh: refresh.into() })
        })
        .collect()
}

/// Adds `a`, or refreshes the account with its UUID (a new name, a new token).
pub fn upsert(list: &mut Vec<Account>, a: Account) {
    match list.iter_mut().find(|x| x.uuid == a.uuid) {
        Some(x) => *x = a,
        None => list.push(a),
    }
}

/// Switching is allowed now? `in_world`: playing on a server or a world;
/// `busy`: a sign-in is already running. Err says why not, for the menu.
/// As Essential: the main menu only, one sign-in at a time.
pub fn may_switch(in_world: bool, busy: bool) -> Result<(), &'static str> {
    if in_world {
        Err("Hesap yalnızca ana menüde değiştirilebilir.")
    } else if busy {
        Err("Bir giriş zaten sürüyor.")
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------- the menu

/// Something to draw, in GUI pixels (as Arena HUD's).
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Fill([f32; 4], u32),
    Text(String, [f32; 2], u32, bool),
}

/// Which page of the menu is up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    List,
    /// "+ Add Account": the page with "Open Browser".
    Add,
    /// Removing account i: "Remove" or "Cancel".
    Confirm(usize),
}

/// What a click hit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Click {
    Switch(usize),
    AskRemove(usize),
    Remove(usize),
    Cancel,
    Add,
    OpenBrowser,
    ClientId,
    Close,
    Nothing,
}

pub const W: f32 = 220.0;
const ROW: f32 = 20.0;

/// The menu as shown: `current` is the account Horizon plays as now.
pub struct View<'a> {
    pub page: Page,
    pub accounts: &'a [Account],
    pub current: Option<u128>,
    pub status: &'a str,
    /// The client id field: its text and whether it is being edited.
    pub client_id: (&'a str, bool),
    /// Waiting for the browser.
    pub waiting: bool,
}

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// The main menu's account button, top right: a head and the name.
pub fn account_button(screen: [f32; 2], name_width: f32) -> [f32; 4] {
    let w = 8.0 + 12.0 + name_width + 8.0;
    [screen[0] - w - 6.0, 6.0, w, 20.0]
}

/// The panel drops down under the account button, its right edge kept.
fn panel(view: &View, screen: [f32; 2]) -> [f32; 4] {
    let rows = match view.page {
        Page::List => view.accounts.len() as f32 * ROW + 2.0 * ROW,
        Page::Add => 5.0 * 12.0 + 2.0 * ROW,
        Page::Confirm(_) => 3.0 * 12.0 + ROW,
    };
    [screen[0] - W - 6.0, 30.0, W, rows + 22.0]
}

/// The buttons on the page and what each does; `hover` shows an account's
/// remove button, as Essential shows it only on the hovered account.
pub fn buttons(view: &View, screen: [f32; 2], hover: [f32; 2]) -> Vec<([f32; 4], Click)> {
    let p = panel(view, screen);
    let mut out = vec![([p[0] + p[2] - 16.0, p[1] + 3.0, 13.0, 12.0], Click::Close)];
    match view.page {
        Page::List => {
            for i in 0..view.accounts.len() {
                let row = [p[0] + 3.0, p[1] + 18.0 + i as f32 * ROW, p[2] - 6.0, ROW - 2.0];
                if inside(row, hover) && view.current != Some(view.accounts[i].uuid) {
                    out.push(([row[0] + row[2] - 16.0, row[1] + 3.0, 13.0, 12.0], Click::AskRemove(i)));
                }
                out.push((row, Click::Switch(i)));
            }
            let y = p[1] + 18.0 + view.accounts.len() as f32 * ROW;
            out.push(([p[0] + 3.0, y, p[2] - 6.0, ROW - 2.0], Click::Add));
        }
        Page::Add => {
            let y = p[1] + 18.0 + 5.0 * 12.0;
            out.push(([p[0] + 3.0, y - 14.0, p[2] - 6.0, 12.0], Click::ClientId));
            if !view.waiting {
                out.push(([p[0] + 3.0, y, p[2] / 2.0 - 4.0, ROW - 2.0], Click::OpenBrowser));
            }
            out.push(([p[0] + p[2] / 2.0 + 1.0, y, p[2] / 2.0 - 4.0, ROW - 2.0], Click::Cancel));
        }
        Page::Confirm(i) => {
            let y = p[1] + 18.0 + 3.0 * 12.0;
            out.push(([p[0] + 3.0, y, p[2] / 2.0 - 4.0, ROW - 2.0], Click::Remove(i)));
            out.push(([p[0] + p[2] / 2.0 + 1.0, y, p[2] / 2.0 - 4.0, ROW - 2.0], Click::Cancel));
        }
    }
    out
}

/// `at` is on the panel or the account button (a click elsewhere closes
/// the menu, as a drop-down does).
pub fn on_menu(view: &View, screen: [f32; 2], at: [f32; 2], button: [f32; 4]) -> bool {
    inside(panel(view, screen), at) || inside(button, at)
}

pub fn on_button(button: [f32; 4], at: [f32; 2]) -> bool {
    inside(button, at)
}

/// The first button under `at` (the remove button comes before its row).
pub fn click(view: &View, screen: [f32; 2], at: [f32; 2]) -> Click {
    buttons(view, screen, at).into_iter().find(|(r, _)| inside(*r, at)).map_or(Click::Nothing, |(_, c)| c)
}

/// A head: the skin's face is not readable here yet, so a square in a
/// colour of the UUID with the name's first letter.
fn head(at: [f32; 2], uuid: u128, name: &str) -> Vec<Draw> {
    let rgb = 0x40_4040 | ((uuid as u32) & 0x7F_7F7F);
    let letter = name.chars().next().map_or(String::new(), |c| c.to_uppercase().collect());
    vec![Draw::Fill([at[0], at[1], 12.0, 12.0], 0xFF00_0000 | rgb), Draw::Text(letter, [at[0] + 3.0, at[1] + 2.0], 0xFFFFFF, true)]
}

/// The account button on the main menu.
pub fn draw_button(screen: [f32; 2], current: Option<&Account>, fallback: &str, hover: bool, measure: &dyn Fn(&str) -> f32) -> Vec<Draw> {
    let name = current.map_or(fallback, |a| a.name.as_str());
    let r = account_button(screen, measure(name));
    let mut out = vec![Draw::Fill(r, if hover { 0xC040_4550 } else { 0xA020_2228 })];
    out.extend(head([r[0] + 6.0, r[1] + 4.0], current.map_or(0, |a| a.uuid), name));
    out.push(Draw::Text(name.to_owned(), [r[0] + 22.0, r[1] + 6.0], 0xFFFFFF, true));
    out
}

pub fn draw(view: &View, screen: [f32; 2], mouse: [f32; 2]) -> Vec<Draw> {
    let p = panel(view, screen);
    let mut out = vec![Draw::Fill(p, 0xF014_161C)];
    let title = match view.page {
        Page::List => "Hesaplar",
        Page::Add => "Hesap ekle",
        Page::Confirm(_) => "Hesabı kaldır",
    };
    out.push(Draw::Text(title.into(), [p[0] + 6.0, p[1] + 5.0], 0xFFFFFF, true));
    let text = |out: &mut Vec<Draw>, i: usize, t: String, rgb: u32| out.push(Draw::Text(t, [p[0] + 6.0, p[1] + 20.0 + i as f32 * 12.0], rgb, true));
    match view.page {
        Page::Add => {
            text(&mut out, 0, "Microsoft hesabınla giriş yap.".into(), 0xCCCCCC);
            text(&mut out, 1, "Tarayıcıda giriş sayfası açılır;".into(), 0xAAAAAA);
            text(&mut out, 2, "giriş yapınca buraya dönülür.".into(), 0xAAAAAA);
        }
        Page::Confirm(i) => {
            let name = view.accounts.get(i).map_or("", |a| a.name.as_str());
            text(&mut out, 0, format!("{name} listeden kaldırılsın mı?"), 0xCCCCCC);
            text(&mut out, 1, "Tekrar eklemek için yeniden giriş gerekir.".into(), 0xAAAAAA);
        }
        Page::List => {}
    }
    for (r, c) in buttons(view, screen, mouse) {
        let hover = inside(r, mouse);
        let (label, fill) = match &c {
            Click::Close => ("×".to_owned(), 0),
            Click::AskRemove(_) => ("×".to_owned(), if hover { 0xFFB0_3030 } else { 0xFF60_2020 }),
            Click::Switch(i) => {
                let a = &view.accounts[*i];
                let now = view.current == Some(a.uuid);
                out.push(Draw::Fill(r, if now { 0xFF24_3A28 } else if hover { 0xFF30_343E } else { 0xFF20_232A }));
                out.extend(head([r[0] + 3.0, r[1] + 3.0], a.uuid, &a.name));
                out.push(Draw::Text(a.name.clone(), [r[0] + 19.0, r[1] + 5.0], if now { 0x7BE07B } else { 0xFFFFFF }, true));
                if now {
                    out.push(Draw::Text("✔".into(), [r[0] + r[2] - 12.0, r[1] + 5.0], 0x7BE07B, true));
                }
                continue;
            }
            Click::Add => ("+ Hesap ekle".to_owned(), 0),
            Click::OpenBrowser => ("Tarayıcıyı aç".to_owned(), 0),
            Click::Remove(_) => ("Kaldır".to_owned(), if hover { 0xFFB0_3030 } else { 0xFF80_2828 }),
            Click::Cancel => ("Vazgeç".to_owned(), 0),
            Click::ClientId => {
                let (id, editing) = view.client_id;
                let shown = if id.is_empty() && !editing { "(boş, README)".to_owned() } else { id.to_owned() };
                (format!("Azure kimliği: {shown}{}", if editing { "_" } else { "" }), if editing { 0xFF30_343E } else { 0xFF1A_1D23 })
            }
            Click::Nothing => continue,
        };
        let fill = if fill != 0 { fill } else if hover { 0xFF3A_3F4A } else { 0xFF26_2A33 };
        out.push(Draw::Fill(r, fill));
        out.push(Draw::Text(label, [r[0] + 4.0, r[1] + (r[3] - 8.0) / 2.0], 0xE0E0E0, false));
    }
    if !view.status.is_empty() {
        out.push(Draw::Text(view.status.to_owned(), [p[0] + 6.0, p[1] + p[3] - 11.0], 0xFFFF55, true));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browser_round_trip() {
        let url = authorize_url("abc", "http://localhost:51234", "v3r", "s7");
        assert!(url.starts_with("https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?client_id=abc&response_type=code"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A51234") && url.contains("code_challenge=v3r&code_challenge_method=plain"));
        assert_eq!(parse_redirect("GET /?code=M.C5%2Fx&state=s7 HTTP/1.1", "s7"), Ok("M.C5/x".into()));
        assert!(parse_redirect("GET /?code=x&state=other HTTP/1.1", "s7").is_err());
        assert_eq!(parse_redirect("GET /?error=access_denied&state=s7 HTTP/1.1", "s7"), Err("Giriş iptal edildi.".into()));
        let ex = exchange("abc", "C", "http://localhost:1", "v");
        assert!(ex.body.unwrap().starts_with("grant_type=authorization_code&client_id=abc&code=C&redirect_uri=http%3A%2F%2Flocalhost%3A1&code_verifier=v"));
    }

    #[test]
    fn answers_are_read() {
        assert_eq!(parse_tokens(r#"{"access_token":"a","refresh_token":"r"}"#), Ok(("a".into(), "r".into())));
        assert!(parse_tokens(r#"{"error":"unauthorized_client"}"#).unwrap_err().contains("Azure"));
        assert_eq!(parse_xbox_live(r#"{"Token":"t","DisplayClaims":{"xui":[{"uhs":"h"}]}}"#), Ok(("t".into(), "h".into())));
        assert!(parse_xsts(r#"{"XErr":2148916233}"#).unwrap_err().contains("Xbox profili yok"));
        assert_eq!(parse_minecraft(r#"{"access_token":"mc","expires_in":86400}"#), Ok(("mc".into(), 86400)));
        assert_eq!(
            parse_profile(r#"{"id":"069a79f444e94726a5befca90e38aaf5","name":"Notch"}"#),
            Ok(("Notch".into(), 0x069a79f444e94726a5befca90e38aaf5))
        );
        assert!(parse_profile(r#"{"path":"/minecraft/profile","error":"NOT_FOUND"}"#).is_err());
        assert_eq!(json_str(r#"{"a" : "x\"yA"}"#, "a").as_deref(), Some("x\"yA"));
        assert_eq!(minecraft_login("U", "X").body.unwrap(), r#"{"identityToken":"XBL3.0 x=U;X"}"#);
    }

    #[test]
    fn accounts_round_trip_and_update() {
        let mut list = vec![Account { name: "A".into(), uuid: 1, refresh: "r1".into() }];
        upsert(&mut list, Account { name: "B".into(), uuid: 2, refresh: "r2".into() });
        upsert(&mut list, Account { name: "A2".into(), uuid: 1, refresh: "r3".into() });
        assert_eq!(read_accounts(&write_accounts(&list)), list);
        assert_eq!(list[0].name, "A2");
        assert_eq!(read_accounts("bozuk satır\n"), []);
    }

    #[test]
    fn main_menu_only_and_one_at_a_time() {
        assert!(may_switch(false, false).is_ok());
        assert!(may_switch(true, false).is_err());
        assert!(may_switch(false, true).is_err());
    }

    #[test]
    fn remove_shows_on_hover_and_asks_first() {
        let accounts = [Account { name: "A".into(), uuid: 1, refresh: String::new() }, Account { name: "B".into(), uuid: 2, refresh: String::new() }];
        let screen = [400.0, 300.0];
        let view = View { page: Page::List, accounts: &accounts, current: Some(1), status: "", client_id: ("", false), waiting: false };
        let rows: Vec<_> = buttons(&view, screen, [0.0, 0.0]).into_iter().filter(|b| matches!(b.1, Click::Switch(_))).collect();
        let b = rows[1].0;
        let on_b = [b[0] + b[2] - 10.0, b[1] + 6.0];
        assert_eq!(click(&view, screen, on_b), Click::AskRemove(1), "remove sits on the hovered row");
        assert_eq!(click(&view, screen, [b[0] + 30.0, b[1] + 6.0]), Click::Switch(1));
        let a = rows[0].0;
        assert_eq!(click(&view, screen, [a[0] + a[2] - 10.0, a[1] + 6.0]), Click::Switch(0), "the current account has no remove");
        let confirm = View { page: Page::Confirm(1), ..view };
        assert!(buttons(&confirm, screen, [0.0, 0.0]).iter().any(|b| b.1 == Click::Remove(1)));
    }
}
