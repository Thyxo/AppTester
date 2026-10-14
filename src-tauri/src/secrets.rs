//! Vercel bypass-tokens pr. app i OS'ets keychain
//! (Windows Credential Manager / macOS Keychain / Secret Service).

const SERVICE: &str = "dk.thyxo.pwalauncher";

fn entry(app_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, &format!("vercel-bypass:{app_id}"))
        .map_err(|e| format!("Keychain er ikke tilgængelig: {e}"))
}

/// Gemt token, eller `None` hvis der intet er (eller keychain ikke kan læses –
/// så åbnes preview bare uden token).
pub fn bypass_token(app_id: &str) -> Option<String> {
    entry(app_id)
        .ok()?
        .get_password()
        .ok()
        .filter(|t| !t.is_empty())
}

/// Gemmer tokenet, eller sletter det ved `None`/tom streng.
pub fn set_bypass_token(app_id: &str, token: Option<&str>) -> Result<(), String> {
    let entry = entry(app_id)?;
    match token.map(str::trim).filter(|t| !t.is_empty()) {
        Some(token) => entry
            .set_password(token)
            .map_err(|e| format!("Kunne ikke gemme token i keychain: {e}")),
        None => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Kunne ikke slette token: {e}")),
        },
    }
}
