//! Permisos que el usuario ya concedió ("Permitir en esta conversación").
//!
//! Se guardan en `~/.config/ianox/permissions.json` como una lista de
//! `sesión:clave` (`shell`, `read:/carpeta`), para que cerrar y volver a abrir
//! ianox no vuelva a preguntar lo mismo.

use std::collections::HashSet;
use std::path::PathBuf;

fn permissions_path() -> Option<PathBuf> {
    let dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.config", h)))
        .ok()?;
    Some(PathBuf::from(dir).join("ianox").join("permissions.json"))
}

/// Los permisos recordados de ejecuciones anteriores; vacío si no hay archivo.
pub fn load() -> HashSet<String> {
    let Some(path) = permissions_path() else {
        return HashSet::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return HashSet::new();
    };
    serde_json::from_str::<Vec<String>>(&text)
        .map(|entries| entries.into_iter().collect())
        .unwrap_or_default()
}

/// Escribe el conjunto completo. Un fallo al guardar no rompe la conversación:
/// el permiso sigue valiendo hasta que se cierre la app.
pub fn save(allowances: &HashSet<String>) {
    let Some(path) = permissions_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut entries: Vec<&String> = allowances.iter().collect();
    entries.sort();
    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        let _ = std::fs::write(&path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_the_file() {
        let dir = std::env::temp_dir().join(format!("ianox-perm-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: single-threaded test, restored before the end.
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        let mut set = HashSet::new();
        set.insert("s1:shell".to_string());
        set.insert("s1:read:/tmp".to_string());
        save(&set);

        let loaded = load();
        assert_eq!(loaded, set);

        std::fs::remove_dir_all(&dir).ok();
    }
}
