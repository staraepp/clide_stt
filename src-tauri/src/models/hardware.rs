//! What this Mac can actually run.
//!
//! The model feed ranks local models by how well they will run *here*, so this
//! has to be measured rather than assumed. Everything comes from `sysctl`, so
//! there is no dependency and no permission prompt.

use std::sync::OnceLock;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    /// e.g. "Apple M2 Pro". Shown so the user can see what the ranking assumed.
    pub chip: String,
    pub total_memory_bytes: u64,
    /// Performance cores where the OS reports them, otherwise all cores.
    pub performance_cores: u32,
    /// Apple Silicon means Metal acceleration for whisper.cpp and the ANE for
    /// ONNX, which is the single biggest factor in local speed.
    pub apple_silicon: bool,
}

impl Hardware {
    /// Memory a model may realistically claim.
    ///
    /// Not all of RAM: the OS, the browser the user is dictating into, and
    /// Clide itself all need room. Two thirds is conservative enough that a
    /// model rated "runs well" does not cause swapping.
    pub fn usable_memory_bytes(&self) -> u64 {
        self.total_memory_bytes / 3 * 2
    }

    pub fn memory_label(&self) -> String {
        format!("{} GB", self.total_memory_bytes / 1_073_741_824)
    }
}

/// Read once — none of this changes while the app is running.
pub fn hardware() -> &'static Hardware {
    static HARDWARE: OnceLock<Hardware> = OnceLock::new();
    HARDWARE.get_or_init(detect)
}

fn detect() -> Hardware {
    let apple_silicon = sysctl_u64("hw.optional.arm64").unwrap_or(0) == 1;

    Hardware {
        chip: sysctl_string("machdep.cpu.brand_string")
            .unwrap_or_else(|| "Unknown processor".into()),
        total_memory_bytes: sysctl_u64("hw.memsize").unwrap_or(8 * 1_073_741_824),
        performance_cores: sysctl_u64("hw.perflevel0.physicalcpu")
            .or_else(|| sysctl_u64("hw.physicalcpu"))
            .unwrap_or(4) as u32,
        apple_silicon,
    }
}

fn sysctl_string(name: &str) -> Option<String> {
    let output = std::process::Command::new("sysctl")
        .args(["-n", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn sysctl_u64(name: &str) -> Option<u64> {
    sysctl_string(name)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_machine_reports_something_plausible() {
        let hardware = hardware();

        assert!(
            hardware.total_memory_bytes >= 1_073_741_824,
            "reported less than 1 GB of RAM"
        );
        assert!(hardware.performance_cores >= 1);
        assert!(!hardware.chip.is_empty());
    }

    #[test]
    fn usable_memory_leaves_headroom_for_the_rest_of_the_system() {
        let hardware = hardware();
        assert!(hardware.usable_memory_bytes() < hardware.total_memory_bytes);
    }

    #[test]
    fn detection_is_cached_rather_than_shelling_out_repeatedly() {
        assert!(std::ptr::eq(hardware(), hardware()));
    }
}
