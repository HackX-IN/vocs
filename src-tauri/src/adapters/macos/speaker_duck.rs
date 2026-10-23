/*!
 * SOURCE OF TRUTH KEYWORDS: MacosSpeakerDucker, DuckedSpeakers, should_duck, speaker_duck_marker,
 *   kAudioDevicePropertyMute, VIRTUAL_MAIN_VOLUME, built_in_speakers, meeting_bleed
 * WHAT:  Mutes the Mac's built-in speakers while a recording runs and puts them
 *        back when the guard is dropped.
 * WHY:   See ports/speaker.rs. This is the CoreAudio half: find the default
 *        output, check it is the internal speakers, mute it (or, where the
 *        device has no mute control, take its volume to zero), and remember
 *        what it was.
 *
 *        ONLY the built-in speakers. A headset or AirPods cannot leak into the
 *        microphone, and silencing them would only stop the user hearing the
 *        meeting for no gain. The test is the device's transport type (built
 *        in) plus its data source, because the headphone jack is the same
 *        device with a different source — and `ispk` / `hdpn` are the values
 *        CoreAudio reports for them.
 *
 *        A mute the user already had on is left alone and reports "nothing to
 *        do": taking it over would unmute their Mac when the recording ended.
 *
 *        THE STATE CHANGE IS WRITTEN TO DISK first. A guard's Drop does not run
 *        if the process is killed or crashes, and a Mac that stays muted after
 *        a crash is a worse bug than the one this fixes, so the previous value
 *        goes into a small marker file and `recover` undoes it at the next
 *        launch. Recovery re-resolves the CURRENT default output rather than
 *        trusting a stored device id, because ids are only stable until reboot.
 * WHERE: Constructed in bootstrap::build_ports with a path under the data
 *        directory; driven by the session actor.
 */

use std::ffi::c_void;
use std::mem::size_of;
use std::path::PathBuf;
use std::ptr::{null, NonNull};

use objc2_core_audio::{
    kAudioDevicePropertyDataSource, kAudioDevicePropertyMute, kAudioDevicePropertyTransportType,
    kAudioDevicePropertyVolumeScalar, kAudioDeviceTransportTypeBuiltIn,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject,
    AudioObjectGetPropertyData, AudioObjectHasProperty, AudioObjectIsPropertySettable,
    AudioObjectPropertyAddress, AudioObjectSetPropertyData,
};
use serde::{Deserialize, Serialize};

use crate::ports::speaker::{DuckGuard, SpeakerDucker};

/// The data source CoreAudio reports for the internal speakers: 'ispk'.
const SOURCE_INTERNAL_SPEAKER: u32 = 0x6973_706b;
/// The volume the menu-bar slider changes: 'vmvc'. Present on the built-in
/// output where the plain per-channel scalar is not.
const VIRTUAL_MAIN_VOLUME: u32 = 0x766d_7663;

/**
 * WHAT:  Whether an output device is the internal speakers rather than
 *        something worn or plugged in.
 * WHY:   Pulled out of the CoreAudio calls so the one decision that matters can
 *        be tested without a sound card. A missing data source counts as the
 *        speakers: desktop Macs report none, and their built-in output is a
 *        speaker.
 * WHERE: `MacosSpeakerDucker::duck` and `recover`.
 */
fn should_duck(transport: u32, source: Option<u32>) -> bool {
    transport == kAudioDeviceTransportTypeBuiltIn
        && source.is_none_or(|source| source == SOURCE_INTERNAL_SPEAKER)
}

/// What was changed, and what it was before.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "control", rename_all = "snake_case")]
enum Control {
    Mute { previous: u32 },
    Volume { selector: u32, previous: f32 },
}

fn address(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn read<T: Copy + Default>(object: u32, address: AudioObjectPropertyAddress) -> Option<T> {
    let mut value = T::default();
    let mut size = size_of::<T>() as u32;
    // SAFETY: `address`, `size` and `value` are live locals for the call, and
    // `size` is exactly the size of `value`, which is what the API writes into.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&address),
            0,
            null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast::<c_void>(),
        )
    };
    (status == 0).then_some(value)
}

fn write<T: Copy>(object: u32, address: AudioObjectPropertyAddress, value: T) -> bool {
    // SAFETY: as in `read`; the API only reads `size_of::<T>()` bytes from `value`.
    let status = unsafe {
        AudioObjectSetPropertyData(
            object,
            NonNull::from(&address),
            0,
            null(),
            size_of::<T>() as u32,
            NonNull::from(&value).cast::<c_void>(),
        )
    };
    status == 0
}

/// Whether the device has this property AND lets it be changed.
fn settable(object: u32, address: AudioObjectPropertyAddress) -> bool {
    // SAFETY: `address` is a live local; the API only reads it.
    if !unsafe { AudioObjectHasProperty(object, NonNull::from(&address)) } {
        return false;
    }
    let mut settable: u8 = 0;
    // SAFETY: `settable` is a live one-byte local, which is a CoreAudio Boolean.
    let status =
        unsafe { AudioObjectIsPropertySettable(object, NonNull::from(&address), NonNull::from(&mut settable)) };
    status == 0 && settable != 0
}

/// The default output device, if it is the internal speakers.
fn built_in_speakers() -> Option<u32> {
    let device: u32 = read(
        kAudioObjectSystemObject as u32,
        address(kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyScopeGlobal),
    )?;
    let transport: u32 = read(device, address(kAudioDevicePropertyTransportType, kAudioObjectPropertyScopeGlobal))?;
    let source: Option<u32> = read(device, address(kAudioDevicePropertyDataSource, kAudioObjectPropertyScopeOutput));
    should_duck(transport, source).then_some(device)
}

/// Silences `device` and returns how to undo it; `None` when it is already
/// silent or offers no control.
fn silence(device: u32) -> Option<Control> {
    let output = kAudioObjectPropertyScopeOutput;

    let mute = address(kAudioDevicePropertyMute, output);
    if settable(device, mute) {
        let previous: u32 = read(device, mute)?;
        if previous != 0 {
            return None;
        }
        return write(device, mute, 1_u32).then_some(Control::Mute { previous });
    }

    for selector in [VIRTUAL_MAIN_VOLUME, kAudioDevicePropertyVolumeScalar] {
        let volume = address(selector, output);
        if !settable(device, volume) {
            continue;
        }
        let previous: f32 = read(device, volume)?;
        if previous <= 0.0 {
            return None;
        }
        return write(device, volume, 0.0_f32).then_some(Control::Volume { selector, previous });
    }
    None
}

fn restore(device: u32, control: Control) {
    let output = kAudioObjectPropertyScopeOutput;
    let restored = match control {
        Control::Mute { previous } => write(device, address(kAudioDevicePropertyMute, output), previous),
        Control::Volume { selector, previous } => write(device, address(selector, output), previous),
    };
    if !restored {
        tracing::warn!(?control, "could not give the speakers back");
    }
}

pub struct MacosSpeakerDucker {
    /// Where the pending restore is written. See the module WHY.
    marker: PathBuf,
}

impl MacosSpeakerDucker {
    pub fn new(marker: PathBuf) -> Self {
        Self { marker }
    }
}

/// Holds the speakers quiet; dropping it gives them back and clears the marker.
struct DuckedSpeakers {
    device: u32,
    control: Control,
    marker: PathBuf,
}

impl DuckGuard for DuckedSpeakers {}

impl Drop for DuckedSpeakers {
    fn drop(&mut self) {
        restore(self.device, self.control);
        let _ = std::fs::remove_file(&self.marker);
    }
}

impl SpeakerDucker for MacosSpeakerDucker {
    fn duck(&self) -> Option<Box<dyn DuckGuard>> {
        let device = built_in_speakers()?;
        let control = silence(device)?;

        // Written AFTER the change and before anything else can fail, so a
        // crash from here on is recoverable. A write error is logged, not
        // fatal: the guard still restores in the normal case.
        match serde_json::to_vec(&control) {
            Ok(bytes) => {
                if let Err(err) = std::fs::write(&self.marker, bytes) {
                    tracing::warn!(error = %err, "could not record the muted speakers; a crash would leave them muted");
                }
            }
            Err(err) => tracing::warn!(error = %err, "could not encode the speaker marker"),
        }

        Some(Box::new(DuckedSpeakers {
            device,
            control,
            marker: self.marker.clone(),
        }))
    }

    fn recover(&self) {
        let Ok(bytes) = std::fs::read(&self.marker) else {
            return;
        };
        if let (Ok(control), Some(device)) = (serde_json::from_slice::<Control>(&bytes), built_in_speakers()) {
            tracing::info!(?control, "restoring speakers a previous run left quiet");
            restore(device, control);
        }
        let _ = std::fs::remove_file(&self.marker);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_internal_speakers_are_ducked() {
        let built_in = kAudioDeviceTransportTypeBuiltIn;
        assert!(should_duck(built_in, Some(SOURCE_INTERNAL_SPEAKER)));
        // A desktop Mac reports no data source.
        assert!(should_duck(built_in, None));
        // The headphone jack is the same device with another source: 'hdpn'.
        assert!(!should_duck(built_in, Some(0x6864_706e)));
        // Bluetooth, USB and HDMI outputs are never touched.
        assert!(!should_duck(objc2_core_audio::kAudioDeviceTransportTypeBluetooth, None));
        assert!(!should_duck(objc2_core_audio::kAudioDeviceTransportTypeUSB, None));
    }

    #[test]
    fn the_marker_round_trips_either_control() {
        for control in [
            Control::Mute { previous: 0 },
            Control::Volume { selector: VIRTUAL_MAIN_VOLUME, previous: 0.5 },
        ] {
            let bytes = serde_json::to_vec(&control).expect("encodes");
            assert_eq!(serde_json::from_slice::<Control>(&bytes).expect("decodes"), control);
        }
    }

    #[test]
    fn querying_the_output_never_panics_without_a_sound_card() {
        // On a CI runner there may be no output at all; the answer is only
        // that nothing panics and nothing is changed.
        let _ = built_in_speakers();
    }
}
