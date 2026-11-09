//! Compatibility with machines where WebKitGTK's hardware rendering misbehaves, mainly
//! virtual machines (blank or black window, or a crash, when the virtual GPU is not up to it).
//!
//! In a VM ianox switches WebKit to its software paths before the web view exists.
//! `IANOX_SAFE_RENDERING=1` forces that on any machine; `=0` turns it off even in a VM.

/// Vendor and product names reported by the firmware of the usual hypervisors.
const HYPERVISOR_NAMES: &[&str] = &[
    "qemu", "kvm", "vmware", "virtualbox", "innotek", "xen", "parallels", "bochs", "bhyve",
    "virtual machine", "hvm domu", "google compute",
];

/// Whether the hardware description says this is a virtual machine.
pub fn looks_like_vm(cpuinfo: &str, dmi_vendor: &str, dmi_product: &str) -> bool {
    // x86 CPUs inside a hypervisor advertise the "hypervisor" flag.
    let flagged = cpuinfo
        .lines()
        .filter(|line| line.starts_with("flags"))
        .any(|line| line.split_whitespace().any(|flag| flag == "hypervisor"));
    let dmi = format!("{dmi_vendor} {dmi_product}").to_lowercase();
    flagged || HYPERVISOR_NAMES.iter().any(|name| dmi.contains(name))
}

/// What the user asked for through `IANOX_SAFE_RENDERING`, if anything.
fn forced(setting: Option<&str>) -> Option<bool> {
    match setting.map(str::trim) {
        Some("1") | Some("true") | Some("on") => Some(true),
        Some("0") | Some("false") | Some("off") => Some(false),
        _ => None,
    }
}

/// Whether the software-rendering workarounds should be applied.
pub fn needs_safe_rendering(setting: Option<&str>, detected_vm: bool) -> bool {
    forced(setting).unwrap_or(detected_vm)
}

/// Sets the WebKitGTK variables that avoid the GPU paths. Call it first thing in `main`,
/// before GTK or the web view are initialised. Variables the user already set are respected.
#[cfg(target_os = "linux")]
pub fn apply_rendering_workarounds() {
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
    let vm = looks_like_vm(
        &read("/proc/cpuinfo"),
        &read("/sys/class/dmi/id/sys_vendor"),
        &read("/sys/class/dmi/id/product_name"),
    );
    let setting = std::env::var("IANOX_SAFE_RENDERING").ok();
    if !needs_safe_rendering(setting.as_deref(), vm) {
        return;
    }
    for name in ["WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_DISABLE_COMPOSITING_MODE"] {
        if std::env::var_os(name).is_none() {
            // Runs first thing in `main`, single-threaded, before GTK or any thread exists.
            std::env::set_var(name, "1");
        }
    }
    eprintln!("[ianox] software rendering enabled (virtual machine or IANOX_SAFE_RENDERING)");
}

#[cfg(not(target_os = "linux"))]
pub fn apply_rendering_workarounds() {}

/// Notices printed by system libraries on every start that the user cannot act on: a GTK
/// module that the desktop environment injects but that is broken or missing, and the
/// deprecation notice of the tray library. Everything else is still printed.
pub fn is_known_system_noise(domain: Option<&str>, message: &str) -> bool {
    match domain {
        Some("Gtk") => message.starts_with("Failed to load module"),
        Some("libayatana-appindicator") => message.contains("is deprecated"),
        _ => false,
    }
}

/// Installs the filter above. Call it first thing in `main`.
#[cfg(target_os = "linux")]
pub fn silence_known_system_notices() {
    use gtk::glib::{self, LogLevel, LogLevels};
    let filter = |domain: Option<&str>, level: LogLevel, message: &str| {
        if !is_known_system_noise(domain, message) {
            glib::log_default_handler(domain, level, Some(message));
        }
    };
    glib::log_set_handler(Some("Gtk"), LogLevels::LEVEL_MESSAGE, false, false, filter);
    glib::log_set_handler(Some("libayatana-appindicator"), LogLevels::LEVEL_WARNING, false, false, filter);
}

#[cfg(not(target_os = "linux"))]
pub fn silence_known_system_notices() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_virtual_machines() {
        let flags = "processor : 0\nflags\t\t: fpu vme sse sse2 hypervisor lahf_lm\n";
        assert!(looks_like_vm(flags, "", ""));
        assert!(looks_like_vm("", "QEMU", "Standard PC (Q35 + ICH9, 2009)"));
        assert!(looks_like_vm("", "innotek GmbH", "VirtualBox"));
        assert!(looks_like_vm("", "Microsoft Corporation", "Virtual Machine"));
        assert!(looks_like_vm("", "VMware, Inc.", "VMware Virtual Platform"));
    }

    #[test]
    fn only_the_known_notices_are_silenced() {
        assert!(is_known_system_noise(Some("Gtk"), "Failed to load module \"appmenu-gtk-module\": ..."));
        assert!(is_known_system_noise(Some("libayatana-appindicator"), "libayatana-appindicator is deprecated. Please use ..."));
        // Other messages from the same libraries must still show.
        assert!(!is_known_system_noise(Some("Gtk"), "Unable to locate theme engine"));
        assert!(!is_known_system_noise(Some("libayatana-appindicator"), "Failed to register on the bus"));
        assert!(!is_known_system_noise(None, "Failed to load module"));
    }

    #[test]
    fn real_hardware_is_not_a_vm() {
        let flags = "flags\t\t: fpu vme sse sse2 avx2 vmx\n"; // vmx is the host side of virtualization
        assert!(!looks_like_vm(flags, "ASUSTeK COMPUTER INC.", "ROG STRIX B550-F GAMING"));
        assert!(!looks_like_vm("", "", ""));
    }

    #[test]
    fn the_user_setting_wins_over_detection() {
        assert!(needs_safe_rendering(Some("1"), false));
        assert!(!needs_safe_rendering(Some("0"), true));
        assert!(needs_safe_rendering(None, true));
        assert!(!needs_safe_rendering(None, false));
        assert!(needs_safe_rendering(Some("maybe"), true)); // unknown value: detection decides
    }
}
