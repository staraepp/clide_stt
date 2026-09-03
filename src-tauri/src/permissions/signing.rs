//! Whether this build has a stable code signature.
//!
//! # Why Clide cares
//!
//! macOS keys the Accessibility grant to the app's **code signature**, not its
//! path. A properly signed app has a designated requirement naming its
//! certificate, which survives rebuilds. An ad-hoc signature has no
//! certificate, so its requirement names the binary's `cdhash` — and that
//! changes on every single rebuild.
//!
//! The result is a genuinely confusing bug: System Settings still lists Clide
//! with its switch **on**, because that list is keyed by path, while
//! `AXIsProcessTrusted()` returns **false**, because this binary is not the one
//! that was granted. The user sees "granted" and Clide says "not granted", and
//! both are telling the truth.
//!
//! Detecting it lets Clide say what is actually wrong instead of repeating a
//! permission prompt that will not help.

use std::sync::OnceLock;

/// True when this build is ad-hoc signed and will lose Accessibility on every
/// rebuild.
pub fn is_ad_hoc() -> bool {
    static AD_HOC: OnceLock<bool> = OnceLock::new();
    *AD_HOC.get_or_init(detect)
}

fn detect() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };

    // `codesign -dv` writes its report to stderr.
    let Ok(output) = std::process::Command::new("/usr/bin/codesign")
        .args(["-dv", "--verbose=2"])
        .arg(&exe)
        .output()
    else {
        return false;
    };

    let report = String::from_utf8_lossy(&output.stderr);
    report.contains("Signature=adhoc")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_answers_without_panicking() {
        let _ = is_ad_hoc();
    }

    #[test]
    fn the_answer_is_cached() {
        assert_eq!(is_ad_hoc(), is_ad_hoc());
    }
}
