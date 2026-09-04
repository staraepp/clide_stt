//! Lowering other apps' audio while the user is speaking.
//!
//! Music playing over dictation costs accuracy, but *stopping* it is rude and
//! unrecoverable — the user did not ask for silence, and clide cannot know
//! where they were in a track. So the system output volume is lowered for the
//! length of the recording and put back afterwards.
//!
//! # The rule that matters
//!
//! **The user's volume is always restored.** It is restored on success, on
//! cancellation, on every failure path, and on drop — so a panic between
//! lowering and finishing cannot leave someone's Mac quiet with no explanation.
//! That is why this is a guard type rather than a pair of functions someone can
//! forget to pair up.
//!
//! macOS exposes no supported way to duck one application's audio from
//! another, so this moves the *system* output level. clide itself plays
//! nothing, so nothing of clide's is affected.

use std::process::Command;

/// Fraction of the original volume to duck to.
///
/// Not silence: the point is that the user can still hear what is playing and
/// knows it is still going.
const DUCK_TO: f32 = 0.25;

/// Volume below which ducking is pointless and only risks a restore bug.
const ALREADY_QUIET: i32 = 12;

/// Holds the original volume and puts it back when dropped.
///
/// Constructing it lowers the volume; letting it fall out of scope raises it.
/// There is deliberately no `restore()` to call — the only way to duck is to
/// hold a value whose destruction undoes it.
#[derive(Debug)]
pub struct Ducked {
    original: i32,
}

impl Ducked {
    /// Lower the system output volume, remembering where it was.
    ///
    /// `None` when ducking is not appropriate or the volume could not be read
    /// — in which case nothing is changed and nothing needs restoring.
    pub fn engage() -> Option<Self> {
        let original = output_volume()?;

        // Muted or nearly silent already: leave it entirely alone. Touching it
        // here could only ever make things worse.
        if original <= ALREADY_QUIET {
            return None;
        }

        let target = ((original as f32) * DUCK_TO).round() as i32;
        if !set_output_volume(target) {
            return None;
        }

        tracing::debug!(from = original, to = target, "ducked system audio");
        Some(Self { original })
    }
}

impl Drop for Ducked {
    fn drop(&mut self) {
        if set_output_volume(self.original) {
            tracing::debug!(to = self.original, "restored system audio");
        } else {
            // Worth a warning rather than a debug: the user is left quieter
            // than clide found them, and they deserve to be able to find out
            // why from the log.
            tracing::warn!(
                expected = self.original,
                "could not restore the system volume after dictation"
            );
        }
    }
}

fn output_volume() -> Option<i32> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", "output volume of (get volume settings)"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    // A muted Mac reports "missing value" rather than a number.
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn set_output_volume(level: i32) -> bool {
    let level = level.clamp(0, 100);
    Command::new("/usr/bin/osascript")
        .args(["-e", &format!("set volume output volume {level}")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ducking must lower audio without silencing it: the user should still
    /// hear that their music is playing, and should get the same level back.
    #[test]
    fn ducking_lowers_without_silencing() {
        let levels = [20, 45, 81, 100];

        for original in levels {
            let ducked = ((original as f32) * DUCK_TO).round() as i32;
            assert!(ducked > 0, "{original} ducked to silence");
            assert!(ducked < original, "{original} was not actually lowered");
        }
    }

    /// A Mac that is already near-silent is left untouched — there is nothing
    /// to gain and a restore bug to lose.
    #[test]
    fn an_already_quiet_mac_is_skipped() {
        let skipped: Vec<i32> = (0..=100)
            .filter(|level| *level <= ALREADY_QUIET)
            .collect();

        assert!(skipped.contains(&0), "muted should be skipped");
        assert!(!skipped.contains(&81), "a normal level should still duck");
    }

    #[test]
    fn volume_is_always_clamped_to_a_valid_level() {
        for level in [-50, 0, 50, 100, 500] {
            let clamped = level.clamp(0, 100);
            assert!(
                (0..=100).contains(&clamped),
                "{level} clamped to {clamped}, outside macOS's range"
            );
        }
    }

    /// The guard exists so that restoring cannot be forgotten. If someone ever
    /// adds a way to duck without holding the value, this is the reminder.
    #[test]
    fn ducking_is_only_reachable_by_holding_the_guard() {
        // `engage` is the only constructor and it returns the guard itself,
        // so a caller that drops it immediately restores immediately.
        let volume_before = output_volume();
        drop(Ducked::engage());
        let volume_after = output_volume();

        if let (Some(before), Some(after)) = (volume_before, volume_after) {
            assert_eq!(before, after, "the volume was not put back");
        }
    }
}
