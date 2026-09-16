//! Format keluaran terpadu Aurion CLI (Human-readable Text vs Machine-readable JSON).
//! Mematuhi Invariant AUR-CLI-007.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

impl OutputFormat {
    /// Ekstrak format keluaran dari argumen baris perintah (`--output json` atau `-o json`).
    pub fn parse_and_strip(args: &[String]) -> (Self, Vec<String>) {
        let mut format = Self::Text;
        let mut filtered = Vec::with_capacity(args.len());
        let mut i = 0;

        while i < args.len() {
            if (args[i] == "--output" || args[i] == "-o" || args[i] == "--format") && i + 1 < args.len() {
                if args[i + 1].eq_ignore_ascii_case("json") {
                    format = Self::Json;
                }
                i += 2;
                continue;
            } else if args[i].starts_with("--output=") {
                let val = &args[i]["--output=".len()..];
                if val.eq_ignore_ascii_case("json") {
                    format = Self::Json;
                }
                i += 1;
                continue;
            } else if args[i].starts_with("--format=") {
                let val = &args[i]["--format=".len()..];
                if val.eq_ignore_ascii_case("json") {
                    format = Self::Json;
                }
                i += 1;
                continue;
            }
            filtered.push(args[i].clone());
            i += 1;
        }

        (format, filtered)
    }

    /// Cetak data terstruktur: format JSON jika diminta, atau teks standar manusiawi.
    pub fn print<T: Serialize>(&self, data: &T, text_fn: impl FnOnce()) {
        match self {
            Self::Json => {
                let s = serde_json::to_string_pretty(data).unwrap_or_else(|e| {
                    format!("{{\"status\":\"error\",\"message\":\"{}\"}}", e)
                });
                println!("{s}");
            }
            Self::Text => {
                text_fn();
            }
        }
    }
}
