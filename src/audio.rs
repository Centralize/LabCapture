//! Sound for recordings: what the computer plays (loopback) and/or the
//! microphone, mixed into 48 kHz stereo and kept in step with the video's
//! clock. Every sound packet is placed by the time it arrived, so silence
//! (when nothing plays) and pauses stay in sync.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

use crate::recorder::Clock;

pub const RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
/// Sound is mixed this far behind real time, so late packets still fit.
const MIX_DELAY: f64 = 0.15;

pub struct AudioChunk {
    /// Recording time of the first sample.
    pub at: Duration,
    /// Interleaved stereo samples.
    pub samples: Vec<i16>,
}

struct SourceBuf {
    rate: u32,
    /// Position (in this source's frames) of `data[0]` on the recording clock.
    start: u64,
    data: VecDeque<[f32; 2]>,
}

impl SourceBuf {
    fn end(&self) -> u64 {
        self.start + self.data.len() as u64
    }
}

pub struct AudioCapture {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl AudioCapture {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Start capturing. `ready` gets `Ok(note)` once at least one source runs
/// (the note names any source that failed), or `Err` if none could start.
pub fn start(
    system: bool,
    mic: bool,
    clock: Arc<Clock>,
    tx: Sender<AudioChunk>,
    ready: SyncSender<Result<Option<String>, String>>,
) -> AudioCapture {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let handle = std::thread::Builder::new()
        .name("lapcapture-audio".into())
        .spawn(move || run(system, mic, clock, tx, ready, stop2))
        .ok();
    AudioCapture { stop, handle }
}

fn run(
    system: bool,
    mic: bool,
    clock: Arc<Clock>,
    tx: Sender<AudioChunk>,
    ready: SyncSender<Result<Option<String>, String>>,
    stop: Arc<AtomicBool>,
) {
    let host = cpal::default_host();
    let mut streams = Vec::new();
    let mut buffers: Vec<Arc<Mutex<SourceBuf>>> = Vec::new();
    let mut problems = Vec::new();

    if system {
        match host.default_output_device() {
            Some(device) => match open(&device, true, &clock) {
                Ok((stream, buf)) => {
                    streams.push(stream);
                    buffers.push(buf);
                }
                Err(e) => problems.push(format!("computer sound ({e})")),
            },
            None => problems.push("computer sound (no speakers or headphones found)".into()),
        }
    }
    if mic {
        match host.default_input_device() {
            Some(device) => match open(&device, false, &clock) {
                Ok((stream, buf)) => {
                    streams.push(stream);
                    buffers.push(buf);
                }
                Err(e) => problems.push(format!("microphone ({e})")),
            },
            None => problems.push("microphone (none found)".into()),
        }
    }

    let note = (!problems.is_empty()).then(|| format!("no {}", problems.join(" and no ")));
    if buffers.is_empty() {
        let _ = ready.send(Err(note.unwrap_or_else(|| "no sound source".into())));
        return;
    }
    let _ = ready.send(Ok(note));

    let mut produced: u64 = 0; // output frames sent so far
    loop {
        // On stop, one last pass mixes right up to now so the ending isn't cut.
        let stopping = stop.load(Ordering::SeqCst);
        if !stopping {
            std::thread::sleep(Duration::from_millis(20));
        }
        let delay = if stopping { 0.0 } else { MIX_DELAY };
        let Some(now) = clock.now() else {
            if stopping {
                break;
            }
            continue;
        };
        if clock.is_paused() && !stopping {
            continue;
        }
        let target = ((now.as_secs_f64() - delay).max(0.0) * RATE as f64) as u64;
        if target <= produced {
            if stopping {
                break;
            }
            continue;
        }
        let n = ((target - produced) as usize).min(RATE as usize);
        let mut mix = vec![[0f32; 2]; n];
        for buf in &buffers {
            let Ok(mut b) = buf.lock() else { continue };
            let ratio = b.rate as f64 / RATE as f64;
            for (i, out) in mix.iter_mut().enumerate() {
                let sp = (produced + i as u64) as f64 * ratio;
                let base = sp.floor() as i64 - b.start as i64;
                let frac = (sp - sp.floor()) as f32;
                if base < 0 || base as usize >= b.data.len() {
                    continue; // no sound for this moment: silence
                }
                let a = b.data[base as usize];
                let c = b.data.get(base as usize + 1).copied().unwrap_or(a);
                out[0] += a[0] + (c[0] - a[0]) * frac;
                out[1] += a[1] + (c[1] - a[1]) * frac;
            }
            // Forget what has been used.
            let used = (((produced + n as u64) as f64 * ratio).floor() as u64).saturating_sub(1);
            while b.start < used && !b.data.is_empty() {
                b.data.pop_front();
                b.start += 1;
            }
            if b.data.is_empty() && b.start < used {
                b.start = used;
            }
        }
        let mut samples = Vec::with_capacity(n * 2);
        for f in &mix {
            samples.push((f[0].clamp(-1.0, 1.0) * 32767.0) as i16);
            samples.push((f[1].clamp(-1.0, 1.0) * 32767.0) as i16);
        }
        let at = Duration::from_secs_f64(produced as f64 / RATE as f64);
        produced += n as u64;
        if tx.send(AudioChunk { at, samples }).is_err() || stopping {
            break;
        }
    }
    drop(streams);
}

fn open(
    device: &cpal::Device,
    loopback: bool,
    clock: &Arc<Clock>,
) -> Result<(cpal::Stream, Arc<Mutex<SourceBuf>>), String> {
    let supported = if loopback { device.default_output_config() } else { device.default_input_config() }
        .map_err(|e| e.to_string())?;
    let config = supported.config();
    let buf = Arc::new(Mutex::new(SourceBuf { rate: config.sample_rate.max(1), start: 0, data: VecDeque::new() }));
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(device, config, buf.clone(), clock.clone()),
        SampleFormat::I16 => build::<i16>(device, config, buf.clone(), clock.clone()),
        SampleFormat::I32 => build::<i32>(device, config, buf.clone(), clock.clone()),
        SampleFormat::U16 => build::<u16>(device, config, buf.clone(), clock.clone()),
        SampleFormat::U8 => build::<u8>(device, config, buf.clone(), clock.clone()),
        SampleFormat::F64 => build::<f64>(device, config, buf.clone(), clock.clone()),
        other => Err(format!("unsupported sound format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, buf))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    buf: Arc<Mutex<SourceBuf>>,
    clock: Arc<Clock>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Sample + Send + 'static,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let rate = config.sample_rate.max(1) as u64;
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _info| {
                let Ok(mut b) = buf.lock() else { return };
                let Some(now) = clock.now() else {
                    b.data.clear();
                    return;
                };
                if clock.is_paused() {
                    return;
                }
                let frames = data.len() / channels;
                let now_pos = (now.as_secs_f64() * rate as f64) as u64;
                let arrive = now_pos.saturating_sub(frames as u64);
                let end = b.end();
                let mut skip = 0usize;
                if b.data.is_empty() {
                    // First sound, or sound after everything was used: place it by the clock.
                    b.start = arrive;
                } else if arrive > end + rate / 25 {
                    // Sound after a silent gap (nothing was playing): fill the gap with silence.
                    let gap = (arrive - end).min(rate * 2);
                    for _ in 0..gap {
                        b.data.push_back([0.0, 0.0]);
                    }
                } else if end > arrive + rate / 8 {
                    // The device runs slightly fast: drop a little to stay in sync.
                    skip = ((end - arrive) as usize).min(frames);
                }
                for frame in data.chunks_exact(channels).skip(skip) {
                    let l = f32::from_sample_(frame[0]);
                    let r = if channels > 1 { f32::from_sample_(frame[1]) } else { l };
                    b.data.push_back([l, r]);
                }
                // Never hold more than a few seconds.
                let cap = (rate * 4) as usize;
                if b.data.len() > cap {
                    let extra = b.data.len() - cap;
                    b.data.drain(..extra);
                    b.start += extra as u64;
                }
            },
            |e| eprintln!("lapcapture: sound capture problem: {e}"),
            Some(Duration::from_secs(3)),
        )
        .map_err(|e| e.to_string())
}
