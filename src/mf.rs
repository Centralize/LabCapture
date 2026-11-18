//! MP4 (H.264) writing with Windows Media Foundation. It uses the graphics
//! card's video encoder when there is one, so large screens record smoothly.

use std::path::Path;

use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFByteStream, IMFMediaType, IMFSinkWriter, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
    MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS,
    MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING, MFAudioFormat_AAC,
    MFAudioFormat_PCM, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFMediaType_Audio, MFMediaType_Video, MFShutdown, MFStartup, MFVideoFormat_H264,
    MFVideoFormat_RGB32, MFVideoInterlace_Progressive,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::{GUID, HSTRING};

const MF_VERSION: u32 = (0x0002 << 16) | 0x0070;

fn err(e: windows::core::Error) -> String {
    format!("{e}")
}

pub struct MfWriter {
    writer: IMFSinkWriter,
    stream: u32,
    audio_stream: Option<u32>,
    width: u32,
    height: u32,
    finished: bool,
}

unsafe fn set_pair(attrs: &IMFMediaType, key: &GUID, hi: u32, lo: u32) -> windows::core::Result<()> {
    unsafe { attrs.SetUINT64(key, ((hi as u64) << 32) | lo as u64) }
}

impl MfWriter {
    /// `audio` adds a 48 kHz stereo AAC sound track fed with `write_audio`.
    pub fn create(path: &Path, width: u32, height: u32, fps: u32, bitrate: u32, audio: bool) -> Result<MfWriter, String> {
        // SAFETY: Media Foundation calls with valid arguments; objects are
        // reference counted by the smart pointers.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, 0).map_err(err)?;

            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 2).map_err(err)?;
            let attrs = attrs.ok_or("no attributes")?;
            attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1).map_err(err)?;
            attrs.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1).map_err(err)?;

            let _ = std::fs::remove_file(path);
            let url = HSTRING::from(path.as_os_str());
            let writer = MFCreateSinkWriterFromURL(&url, None::<&IMFByteStream>, &attrs).map_err(err)?;

            let out = MFCreateMediaType().map_err(err)?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(err)?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).map_err(err)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, bitrate).map_err(err)?;
            out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(err)?;
            set_pair(&out, &MF_MT_FRAME_SIZE, width, height).map_err(err)?;
            set_pair(&out, &MF_MT_FRAME_RATE, fps, 1).map_err(err)?;
            set_pair(&out, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1).map_err(err)?;
            let stream = writer.AddStream(&out).map_err(err)?;

            let input = MFCreateMediaType().map_err(err)?;
            input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(err)?;
            input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(err)?;
            input.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(err)?;
            set_pair(&input, &MF_MT_FRAME_SIZE, width, height).map_err(err)?;
            set_pair(&input, &MF_MT_FRAME_RATE, fps, 1).map_err(err)?;
            set_pair(&input, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1).map_err(err)?;
            // Positive stride: rows top to bottom.
            input.SetUINT32(&MF_MT_DEFAULT_STRIDE, width * 4).map_err(err)?;
            writer.SetInputMediaType(stream, &input, None::<&IMFAttributes>).map_err(err)?;

            let audio_stream = if audio {
                let rate = crate::audio::RATE;
                let channels = crate::audio::CHANNELS as u32;
                let aout = MFCreateMediaType().map_err(err)?;
                aout.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(err)?;
                aout.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC).map_err(err)?;
                aout.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(err)?;
                aout.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate).map_err(err)?;
                aout.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels).map_err(err)?;
                // 192 kbit/s.
                aout.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24_000).map_err(err)?;
                let astream = writer.AddStream(&aout).map_err(err)?;

                let ain = MFCreateMediaType().map_err(err)?;
                ain.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(err)?;
                ain.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).map_err(err)?;
                ain.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(err)?;
                ain.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate).map_err(err)?;
                ain.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels).map_err(err)?;
                ain.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, channels * 2).map_err(err)?;
                ain.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, rate * channels * 2).map_err(err)?;
                writer.SetInputMediaType(astream, &ain, None::<&IMFAttributes>).map_err(err)?;
                Some(astream)
            } else {
                None
            };

            writer.BeginWriting().map_err(err)?;

            Ok(MfWriter { writer, stream, audio_stream, width, height, finished: false })
        }
    }

    /// Add interleaved stereo 16-bit sound starting at `start` (100 ns units).
    pub fn write_audio(&mut self, samples: &[i16], start: i64) -> Result<(), String> {
        let Some(stream) = self.audio_stream else { return Ok(()) };
        if samples.is_empty() {
            return Ok(());
        }
        let bytes = samples.len() * 2;
        let frames = samples.len() as i64 / crate::audio::CHANNELS as i64;
        // SAFETY: the locked buffer is exactly `bytes` long.
        unsafe {
            let buffer = MFCreateMemoryBuffer(bytes as u32).map_err(err)?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None).map_err(err)?;
            let dst = std::slice::from_raw_parts_mut(ptr, bytes);
            for (d, s) in dst.chunks_exact_mut(2).zip(samples) {
                d.copy_from_slice(&s.to_le_bytes());
            }
            buffer.Unlock().map_err(err)?;
            buffer.SetCurrentLength(bytes as u32).map_err(err)?;
            let sample = MFCreateSample().map_err(err)?;
            sample.AddBuffer(&buffer).map_err(err)?;
            sample.SetSampleTime(start).map_err(err)?;
            sample.SetSampleDuration(frames * 10_000_000 / crate::audio::RATE as i64).map_err(err)?;
            self.writer.WriteSample(stream, &sample).map_err(err)?;
        }
        Ok(())
    }

    /// Add one RGBA frame shown from `start` for `duration` (both in 100 ns units).
    pub fn write(&mut self, rgba: &[u8], start: i64, duration: i64) -> Result<(), String> {
        let len = (self.width * self.height * 4) as usize;
        if rgba.len() < len {
            return Err("frame has the wrong size".into());
        }
        // SAFETY: the locked buffer is exactly `len` bytes long.
        unsafe {
            let buffer = MFCreateMemoryBuffer(len as u32).map_err(err)?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None).map_err(err)?;
            let dst = std::slice::from_raw_parts_mut(ptr, len);
            for (d, s) in dst.chunks_exact_mut(4).zip(rgba[..len].chunks_exact(4)) {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = 255;
            }
            buffer.Unlock().map_err(err)?;
            buffer.SetCurrentLength(len as u32).map_err(err)?;
            let sample = MFCreateSample().map_err(err)?;
            sample.AddBuffer(&buffer).map_err(err)?;
            sample.SetSampleTime(start).map_err(err)?;
            sample.SetSampleDuration(duration.max(1)).map_err(err)?;
            self.writer.WriteSample(self.stream, &sample).map_err(err)?;
        }
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        // SAFETY: finalising a writer we created.
        unsafe { self.writer.Finalize().map_err(err) }
    }
}

impl Drop for MfWriter {
    fn drop(&mut self) {
        let _ = self.finish();
        // SAFETY: balances the MFStartup in `create`.
        unsafe {
            let _ = MFShutdown();
        }
    }
}
