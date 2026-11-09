//! UI strings for the messages Rust produces.
//!
//! The JSON is generated from `locales/*.po` (`npm run i18n`) and embedded in the
//! binary, so it works the same on Linux, Windows and macOS without libintl or
//! ficheros que instalar.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::OnceLock;

const ES: &str = include_str!("../locales/es.json");
const EN: &str = include_str!("../locales/en.json");

fn parse(json: &str) -> HashMap<String, String> {
    serde_json::from_str(json).unwrap_or_default()
}

fn table(lang: &str) -> &'static HashMap<String, String> {
    static ES_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    static EN_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();

    if lang == "es" {
        ES_TABLE.get_or_init(|| parse(ES))
    } else {
        EN_TABLE.get_or_init(|| parse(EN))
    }
}

/// Translated text. If the key is missing it falls back to English, then to the key itself.
pub fn t(lang: &str, key: &str) -> String {
    table(lang)
        .get(key)
        .or_else(|| table("en").get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traduce_en_los_dos_idiomas() {
        assert_eq!(t("es", "Updates"), "Actualizaciones");
        assert_eq!(t("en", "Updates"), "Updates");
    }

    #[test]
    fn clave_desconocida_devuelve_la_clave() {
        assert_eq!(t("en", "No such key"), "No such key");
    }

    #[test]
    fn idioma_desconocido_cae_a_ingles() {
        assert_eq!(t("de", "Assistant"), "Assistant");
    }
}
