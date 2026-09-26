//! Resolusi password keystore yang aman (Anti-Exposure AUR-APP-01).
//!
//! Menggantikan penetimaan `--password <STR>` pada argv yang mengekspos
//! kredensial melalui process table, shell history, dan audit log.
//! Menggunakan LTE 3-tingkat (3-Tier Precedence):
//! 1. Stdin (flag `--password-stdin` atau input non-TTY / pipa).
//! 2. Environment variable.
//! 3. Prompt terminal interaktif via `rpassword` (tanpa echo), dengan
//!    konfirmasi ganda untuk password baru.

use std::io::{self, BufRead, IsTerminal};

/// Nama environment variable kanonikal untuk password wallet.
pub const ENV_WALLET_PASSWORD: &str = "AURION_WALLET_PASSWORD";

/// Nama environment variable kanonikal untuk mnemonic/seed phrase wallet.
pub const ENV_WALLET_MNEMONIC: &str = "AURION_WALLET_MNEMONIC";

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("Gagal membaca password: {0}")]
    Io(#[from] io::Error),
    #[error("Password konfirmasi tidak cocok")]
    Mismatch,
    #[error("Password tidak boleh kosong")]
    Empty,
}

/// Mengambil password dengan 3-tier precedence:
/// 1. Stdin (jika diminta via flag atau terminal non-interaktif)
/// 2. Environment variable
/// 3. Interactive prompt via `rpassword` (no echo)
pub fn resolve_password(
    from_stdin: bool,
    env_var_name: Option<&str>,
    prompt_text: &str,
    confirm: bool,
) -> Result<String, PasswordError> {
    // Tier 1: Stdin
    let stdin_is_tty = io::stdin().is_terminal();
    if from_stdin || !stdin_is_tty {
        let stdin = io::stdin();
        return resolve_password_from_reader(stdin.lock());
    }

    // Tier 2: Environment variable
    if let Some(var) = env_var_name {
        if let Ok(val) = std::env::var(var) {
            if !val.trim().is_empty() {
                return Ok(val);
            }
        }
    }

    // Tier 3: Prompt interaktif (no echo via rpassword)
    eprint!("{prompt_text}: ");
    io::Write::flush(&mut io::stderr())?;
    let pwd = rpassword::read_password()?;
    if pwd.is_empty() {
        return Err(PasswordError::Empty);
    }

    if confirm {
        eprint!("Konfirmasi {prompt_text}: ");
        io::Write::flush(&mut io::stderr())?;
        let confirmation = rpassword::read_password()?;
        if confirmation != pwd {
            return Err(PasswordError::Mismatch);
        }
    }

    Ok(pwd)
}

fn resolve_password_from_reader<R: BufRead>(mut reader: R) -> Result<String, PasswordError> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let pwd = trim_line_endings(&line);
    if pwd.is_empty() {
        return Err(PasswordError::Empty);
    }
    Ok(pwd)
}

/// Mengambil mnemonic/seed phrase dengan LTE 3-tingkat yang sama seperti password:
/// 1. Stdin (flag `--mnemonic-stdin` atau input non-TTY / pipa).
/// 2. Environment variable `AURION_WALLET_MNEMONIC`.
/// 3. Prompt terminal interaktif via `rpassword` (tanpa echo).
///
/// Nilai mnemonic tidak pernah diterima melalui argv agar tidak bocor ke
/// process table, shell history, maupun audit log.
pub fn resolve_mnemonic(from_stdin: bool, prompt_text: &str) -> Result<String, PasswordError> {
    resolve_password(from_stdin, Some(ENV_WALLET_MNEMONIC), prompt_text, false)
}

/// Menghapus `\r` dan `\n` dari akhir baris yang dibaca.
fn trim_line_endings(line: &str) -> String {
    line.trim_end_matches('\n')
        .trim_end_matches('\r')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_line_endings_handles_crlf_and_lf() {
        assert_eq!(trim_line_endings("rahasia123\r\n"), "rahasia123");
        assert_eq!(trim_line_endings("rahasia123\n"), "rahasia123");
        assert_eq!(trim_line_endings("rahasia123\r"), "rahasia123");
        assert_eq!(trim_line_endings("rahasia123"), "rahasia123");
    }

    #[test]
    fn test_env_constant_defined() {
        assert_eq!(ENV_WALLET_PASSWORD, "AURION_WALLET_PASSWORD");
        assert_eq!(ENV_WALLET_MNEMONIC, "AURION_WALLET_MNEMONIC");
    }

    #[test]
    fn test_empty_mnemonic_rejected_when_non_tty_stdin() {
        // Stdin pada harness test bukan TTY; tanpa input apa pun, hasilnya Empty.
        let res = resolve_password_from_reader(std::io::Cursor::new("\n"));
        assert!(res.is_err());
    }

    #[test]
    fn test_empty_password_rejected_when_non_tty_stdin() {
        // Stdin pada harness test bukan TTY; tanpa input apa pun, hasilnya Empty.
        let res = resolve_password_from_reader(std::io::Cursor::new("\r\n"));
        assert!(res.is_err());
    }
}
