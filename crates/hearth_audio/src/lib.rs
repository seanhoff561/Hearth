//! Sound (V2-3): every sound made as it plays, from noise, filters and tones, with no
//! recordings. A mixer runs on the audio device's own thread; the game sends it commands:
//! sounds to play once (footsteps by the ground's surface, landings, splashes, strokes, the
//! body's gasps and hurts, the interface's clicks), the surroundings to ease toward (wind,
//! rain under the sky or a roof, water over the ears, an echo when shut in, the heart and the
//! breath), and the volumes of the options' categories.

pub mod beds;
pub mod dsp;
pub mod mixer;
pub mod sounds;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

pub use beds::Ambience;
pub use mixer::{BUSES, Bus, Command, Mixer};
pub use sounds::{Cry, Sound, Surface};

/// The game's sound output.
pub struct Audio {
    _stream: cpal::Stream,
    tx: Sender<Command>,
    failed: Arc<AtomicBool>,
    /// The device's name and its output format.
    pub device: String,
    pub rate: u32,
    pub channels: u16,
}

impl Audio {
    /// Opens the named output device (empty, or a name not found: the system's default).
    pub fn open(device: &str, seed: u32) -> Result<Audio, String> {
        let host = cpal::default_host();
        let chosen = if device.is_empty() {
            None
        } else {
            host.output_devices()
                .ok()
                .and_then(|mut all| all.find(|d| name_of(d).as_deref() == Some(device)))
        };
        let dev = match chosen {
            Some(d) => d,
            None => host
                .default_output_device()
                .ok_or_else(|| "no audio output device".to_owned())?,
        };
        let supported = dev.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config = supported.config();
        let (rate, channels) = (config.sample_rate, config.channels);
        let (tx, rx) = channel();
        let failed = Arc::new(AtomicBool::new(false));
        let mixer = Mixer::new(rate as f32, seed);
        let f = failed.clone();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&dev, config, mixer, rx, f),
            SampleFormat::F64 => build::<f64>(&dev, config, mixer, rx, f),
            SampleFormat::I16 => build::<i16>(&dev, config, mixer, rx, f),
            SampleFormat::U16 => build::<u16>(&dev, config, mixer, rx, f),
            SampleFormat::I32 => build::<i32>(&dev, config, mixer, rx, f),
            SampleFormat::U32 => build::<u32>(&dev, config, mixer, rx, f),
            SampleFormat::I8 => build::<i8>(&dev, config, mixer, rx, f),
            SampleFormat::U8 => build::<u8>(&dev, config, mixer, rx, f),
            other => Err(format!("unsupported sample format {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        let device = name_of(&dev).unwrap_or_default();
        log::info!("sound: {device}, {rate} Hz, {channels} channels, {format}");
        Ok(Audio {
            _stream: stream,
            tx,
            failed,
            device,
            rate,
            channels,
        })
    }

    pub fn send(&self, c: Command) {
        // The stream may be gone (its device lost); `failed` says so.
        let _ = self.tx.send(c);
    }

    /// Whether the device failed (unplugged, taken): open it again.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
}

/// The names of the output devices there are.
pub fn output_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut names: Vec<String> = match host.output_devices() {
        Ok(all) => all.filter_map(|d| name_of(&d)).collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names.dedup();
    names
}

fn name_of(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|x| x.name().to_owned())
}

fn build<T>(
    dev: &cpal::Device,
    config: StreamConfig,
    mut mixer: Mixer,
    rx: Receiver<Command>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = (config.channels as usize).max(1);
    let mut buf = vec![0.0f32; 2 * 4096];
    dev.build_output_stream::<T, _, _>(
        config,
        move |data: &mut [T], _| {
            while let Ok(c) = rx.try_recv() {
                mixer.apply(c);
            }
            let frames = data.len() / channels;
            if buf.len() < frames * 2 {
                buf.resize(frames * 2, 0.0);
            }
            let mixed = &mut buf[..frames * 2];
            mixer.render(mixed);
            for (frame, s) in data
                .chunks_exact_mut(channels)
                .zip(mixed.as_chunks::<2>().0)
            {
                if channels == 1 {
                    frame[0] = T::from_sample((s[0] + s[1]) * 0.5);
                } else {
                    frame[0] = T::from_sample(s[0]);
                    frame[1] = T::from_sample(s[1]);
                    for x in &mut frame[2..] {
                        *x = T::EQUILIBRIUM;
                    }
                }
            }
        },
        move |e| {
            log::warn!("sound output failed: {e}");
            failed.store(true, Ordering::Relaxed);
        },
        None,
    )
    .map_err(|e| e.to_string())
}
