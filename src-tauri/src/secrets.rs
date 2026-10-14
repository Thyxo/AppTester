//! Vercel bypass-tokens pr. app i OS'ets keychain
//! (Windows Credential Manager / macOS Keychain / Secret Service).

const SERVICE: &str = "dk.thyxo.pwalauncher";

fn entry(app_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, &format!("vercel-bypass:{app_id}"))
        .map_err(|e| format!("Keychain er ikke tilgængelig: {e}"))
}

pub fn bypass_token(app_id: &str) -> Option<String> {
    match entry(app_id).ok()?.get_password() {
        Ok(token) if !token.is_empty() => Some(token),
        Ok(_) | Err(keyring::Error::NoEntry) => None,
        Err(err) => {
            eprintln!("Kunne ikke læse bypass-token for {app_id}: {err}");
            None
        }
    }
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
