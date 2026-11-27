//! What this machine can actually run.
//!
//! The model feed ranks local models by how well they will run *here*, so this
//! has to be measured rather than assumed. macOS uses `sysctl`; Windows uses
//! the OS memory API and, in CUDA builds, `nvidia-smi`. No permission prompt.

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
    /// NVIDIA adapter reported by the driver, when a CUDA build can use it.
    pub gpu_name: Option<String>,
    /// Acceleration available to the local engines in this build.
    pub accelerator: Option<String>,
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

#[cfg(target_os = "windows")]
fn detect() -> Hardware {
    #[repr(C)]
    struct Memory {
        length: u32,
        load: u32,
        total: u64,
        available: u64,
        page: u64,
        avail_page: u64,
        virtual_total: u64,
        avail_virtual: u64,
        extended: u64,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(info: *mut Memory) -> i32;
    }

    let mut memory: Memory = unsafe { std::mem::zeroed() };
    memory.length = std::mem::size_of::<Memory>() as u32;
    let total = if unsafe { GlobalMemoryStatusEx(&mut memory) } != 0 {
        memory.total
    } else {
        0
    };
    let gpu_name = nvidia_gpu_name();

    Hardware {
        chip: std::env::var("PROCESSOR_IDENTIFIER").unwrap_or("Windows processor".into()),
        total_memory_bytes: total,
        performance_cores: std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1),
        apple_silicon: false,
        accelerator: gpu_name.as_ref().map(|_| "CUDA".into()),
        gpu_name,
    }
}

#[cfg(target_os = "windows")]
fn nvidia_gpu_name() -> Option<String> {
    #[cfg(feature = "cuda")]
    {
        let output = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=name", "--format=csv,noheader,nounits"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_string)
    }
    #[cfg(not(feature = "cuda"))]
    {
        None
    }
}

#[cfg(not(target_os = "windows"))]
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
        gpu_name: None,
        accelerator: apple_silicon.then(|| "Metal".into()),
    }
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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
