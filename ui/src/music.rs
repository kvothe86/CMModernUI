//! Menu music (switch `music`, user 2026-11-24): cmui's own synth tracks (ui\music\make_music.py)
//! from Data\cmui_music\*.wav, played in a loop by a thread of its own through winmm's waveOut,
//! which Windows mixes with the game's own DirectSound sounds. Each track plays its loop twice,
//! the second time fading out, then the next. A network client plays its own.
//!
//! The control sits at the foot of the menu bar, under its lowest button (only where there is
//! room): the track's name, play / pause, next, mute and a volume bar. cmui.ini keeps
//! music_playing, music_volume (0-100) and music_track.
//!
//! The files: MP3 (Windows' own decoder), 16-bit PCM WAV or mono IMA ADPCM WAV (cmui's own are
//! ADPCM, 22050 Hz), the name without a leading number as the track's name. The player's own
//! songs: `music_folder=<a folder>` in cmui.ini, played after cmui's tracks, or alone with
//! `music_cmui_tracks=0`. Under 2 minutes a track is a loop (twice, fading out), longer a song (once).
use crate::*;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::Relaxed};

static PLAYING: AtomicBool = AtomicBool::new(true);
static VOLUME: AtomicU32 = AtomicU32::new(50);
static TRACK: AtomicUsize = AtomicUsize::new(0);
static SKIP: AtomicBool = AtomicBool::new(false);
/// The thread went on to another track (the control's name is drawn again, cmui.ini told).
static CHANGED: AtomicBool = AtomicBool::new(false);
static TRACKS: std::sync::OnceLock<Vec<(String, std::path::PathBuf)>> = std::sync::OnceLock::new();
/// The volume before mute (to go back to).
static mut UNMUTED: u32 = 50;

/// The menu bar's buttons this frame (lib.rs's `sidebar_item` widens it) and from the last full
/// frame: the control goes under them.
pub(crate) static mut BAR_SEEN: Option<Rect> = None;
static mut BAR: Option<Rect> = None;
/// Where the control's parts are this frame: play / pause, next, mute, the volume bar.
static mut BOXES: [Rect; 4] = [(0, 0, -1, -1); 4];
static mut WAS_HELD: bool = false;

#[repr(C, packed)]
struct WaveFormat {
    tag: u16,
    channels: u16,
    rate: u32,
    bytes_per_sec: u32,
    align: u16,
    bits: u16,
    extra: u16,
}

#[repr(C)]
struct WaveHeader {
    data: *mut i16,
    length: u32,
    recorded: u32,
    user: usize,
    flags: u32,
    loops: u32,
    next: usize,
    reserved: usize,
}

#[link(name = "winmm")]
extern "system" {
    fn waveOutOpen(out: *mut usize, device: u32, format: *const WaveFormat, callback: usize, instance: usize, flags: u32) -> u32;
    fn waveOutPrepareHeader(out: usize, header: *mut WaveHeader, size: u32) -> u32;
    fn waveOutUnprepareHeader(out: usize, header: *mut WaveHeader, size: u32) -> u32;
    fn waveOutWrite(out: usize, header: *mut WaveHeader, size: u32) -> u32;
    fn waveOutReset(out: usize) -> u32;
    fn waveOutPause(out: usize) -> u32;
    fn waveOutRestart(out: usize) -> u32;
    fn waveOutClose(out: usize) -> u32;
}

/// A WAV's samples, interleaved: (channels, rate, samples). 16-bit PCM or mono IMA ADPCM.
pub(crate) fn decode(file: &[u8]) -> Option<(u16, u32, Vec<i16>)> {
    if file.get(0..4)? != b"RIFF" || file.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut fmt, mut data, mut count) = (None, None, None);
    let mut at = 12;
    while at + 8 <= file.len() {
        let size = u32::from_le_bytes(file[at + 4..at + 8].try_into().ok()?) as usize;
        let body = file.get(at + 8..(at + 8 + size).min(file.len()))?;
        match &file[at..at + 4] {
            b"fmt " => fmt = Some(body),
            b"data" => data = Some(body),
            b"fact" if body.len() >= 4 => count = Some(u32::from_le_bytes(body[0..4].try_into().ok()?) as usize),
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    let (fmt, data) = (fmt?, data?);
    let u16_at = |i: usize| fmt.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let (tag, channels, rate, align, bits) = (u16_at(0)?, u16_at(2)?, u32::from_le_bytes(fmt.get(4..8)?.try_into().ok()?), u16_at(12)? as usize, u16_at(14)?);
    match (tag, bits) {
        (1, 16) if channels > 0 => Some((channels, rate, data.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect())),
        (0x11, 4) if channels == 1 && align > 4 => {
            let mut out = ima_mono(data, align);
            if let Some(n) = count {
                out.truncate(n);
            }
            Some((1, rate, out))
        }
        _ => None,
    }
}

#[repr(C, packed)]
struct Mp3Format {
    wave: WaveFormat,
    id: u16,
    flags: u32,
    block_size: u16,
    frames_per_block: u16,
    codec_delay: u16,
}

#[repr(C)]
struct AcmHeader {
    size: u32,
    status: u32,
    user: usize,
    src: *mut u8,
    src_length: u32,
    src_used: u32,
    src_user: usize,
    dst: *mut u8,
    dst_length: u32,
    dst_used: u32,
    dst_user: usize,
    reserved: [u32; 10],
}

#[link(name = "msacm32")]
extern "system" {
    fn acmStreamOpen(stream: *mut usize, driver: usize, src: *const Mp3Format, dst: *const WaveFormat, filter: usize, callback: usize, instance: usize, flags: u32) -> u32;
    fn acmStreamSize(stream: usize, input: u32, output: *mut u32, flags: u32) -> u32;
    fn acmStreamPrepareHeader(stream: usize, header: *mut AcmHeader, flags: u32) -> u32;
    fn acmStreamConvert(stream: usize, header: *mut AcmHeader, flags: u32) -> u32;
    fn acmStreamUnprepareHeader(stream: usize, header: *mut AcmHeader, flags: u32) -> u32;
    fn acmStreamClose(stream: usize, flags: u32) -> u32;
}

/// The first MPEG layer III frame header past an ID3v2 tag: (offset, channels, rate, kbps, MPEG 1).
fn mp3_first_frame(file: &[u8]) -> Option<(usize, u16, u32, u32, bool)> {
    let mut at = 0;
    if file.starts_with(b"ID3") && file.len() > 10 {
        let size = file[6..10].iter().fold(0usize, |s, &b| s << 7 | (b & 0x7f) as usize);
        at = 10 + size + if file[5] & 0x10 != 0 { 10 } else { 0 };
    }
    while at + 4 <= file.len() {
        let h = &file[at..at + 4];
        let (version, layer, bitrate, rate) = (h[1] >> 3 & 3, h[1] >> 1 & 3, h[2] >> 4, h[2] >> 2 & 3);
        if h[0] == 0xff && h[1] & 0xe0 == 0xe0 && version != 1 && layer == 1 && (1..15).contains(&bitrate) && rate != 3 {
            let mpeg1 = version == 3;
            let kbps = if mpeg1 { [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320] } else { [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160] }[bitrate as usize];
            let hz = [44100, 48000, 32000][rate as usize] >> match version { 3 => 0, 2 => 1, _ => 2 };
            return Some((at, if h[3] >> 6 == 3 { 1 } else { 2 }, hz, kbps, mpeg1));
        }
        at += 1;
    }
    None
}

/// An MP3 through Windows' own MP3 decoder (the ACM codec every Windows has): interleaved
/// 16-bit samples, the whole song at once.
/// ponytail: the whole song in memory (~10 MB a stereo minute); stream it if long sets matter.
pub(crate) fn decode_mp3(file: &[u8]) -> Option<(u16, u32, Vec<i16>)> {
    let (start, channels, rate, kbps, mpeg1) = mp3_first_frame(file)?;
    let end = if file.len() >= start + 128 && &file[file.len() - 128..file.len() - 125] == b"TAG" { file.len() - 128 } else { file.len() };
    let mut input = &file[start..end];
    let block = (if mpeg1 { 144_000 } else { 72_000 } * kbps / rate) as u16;
    let src = Mp3Format {
        wave: WaveFormat { tag: 0x55, channels, rate, bytes_per_sec: kbps * 125, align: 1, bits: 0, extra: 12 },
        id: 1,         // MPEGLAYER3_ID_MPEG
        flags: 2,      // MPEGLAYER3_FLAG_PADDING_OFF
        block_size: block.max(1),
        frames_per_block: 1,
        codec_delay: 1393,
    };
    let dst = WaveFormat { tag: 1, channels, rate, bytes_per_sec: rate * channels as u32 * 2, align: channels * 2, bits: 16, extra: 0 };
    unsafe {
        let mut stream = 0usize;
        if acmStreamOpen(&mut stream, 0, &src, &dst, 0, 0, 0, 0) != 0 {
            log("music: no MP3 decoder (acmStreamOpen)");
            return None;
        }
        const CHUNK: usize = 16 * 1024;
        let mut out_size = 0u32;
        acmStreamSize(stream, CHUNK as u32, &mut out_size, 0);
        let (mut src_buf, mut dst_buf) = (vec![0u8; CHUNK], vec![0u8; (out_size as usize).max(CHUNK * 16)]);
        let mut header: AcmHeader = std::mem::zeroed();
        header.size = std::mem::size_of::<AcmHeader>() as u32;
        (header.src, header.src_length, header.dst, header.dst_length) = (src_buf.as_mut_ptr(), CHUNK as u32, dst_buf.as_mut_ptr(), dst_buf.len() as u32);
        let mut pcm: Vec<u8> = Vec::with_capacity(input.len() * 12);
        if acmStreamPrepareHeader(stream, &mut header, 0) == 0 {
            let mut held = 0; // input bytes from the last call the codec left
            loop {
                let take = (CHUNK - held).min(input.len());
                src_buf[held..held + take].copy_from_slice(&input[..take]);
                input = &input[take..];
                let n = held + take;
                let last = input.is_empty();
                header.src_length = n as u32;
                // ACM_STREAMCONVERTF_BLOCKALIGN while more comes, END to flush
                if acmStreamConvert(stream, &mut header, if last { 0x20 } else { 4 }) != 0 {
                    break;
                }
                pcm.extend_from_slice(&dst_buf[..header.dst_used as usize]);
                let used = (header.src_used as usize).min(n);
                held = n - used;
                src_buf.copy_within(used..n, 0);
                if last && (held == 0 || header.dst_used == 0) || (used == 0 && header.dst_used == 0 && take == 0) {
                    break;
                }
            }
            header.src_length = CHUNK as u32;
            acmStreamUnprepareHeader(stream, &mut header, 0);
        }
        acmStreamClose(stream, 0);
        let samples: Vec<i16> = pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        (!samples.is_empty()).then_some((channels, rate, samples))
    }
}

/// IMA ADPCM, mono, blocks of `align` bytes: the first sample and the step index, then two
/// samples a byte, low nibble first.
fn ima_mono(data: &[u8], align: usize) -> Vec<i16> {
    const STEPS: [i32; 89] = [
        7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279,
        307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428,
        4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
    ];
    const INDEX: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];
    let mut out = Vec::with_capacity(data.len() * 2);
    for block in data.chunks(align).filter(|b| b.len() > 4) {
        let mut pred = i16::from_le_bytes([block[0], block[1]]) as i32;
        let mut index = (block[2] as i32).min(88);
        out.push(pred as i16);
        for &byte in &block[4..] {
            for code in [byte & 15, byte >> 4] {
                let step = STEPS[index as usize];
                let mut diff = step >> 3;
                if code & 4 != 0 {
                    diff += step;
                }
                if code & 2 != 0 {
                    diff += step >> 1;
                }
                if code & 1 != 0 {
                    diff += step >> 2;
                }
                pred = (if code & 8 != 0 { pred - diff } else { pred + diff }).clamp(-32768, 32767);
                index = (index + INDEX[(code & 7) as usize]).clamp(0, 88);
                out.push(pred as i16);
            }
        }
    }
    out
}

/// "03 Sunday League.wav" -> "Sunday League"
fn track_name(path: &std::path::Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '-' || c == '_' || c == '.');
    if name.is_empty() { stem } else { name.to_string() }
}

/// Once: the tracks, the settings, the player's thread.
unsafe fn start() {
    if TRACKS.get().is_some() {
        return;
    }
    // cmui's own tracks (unless music_cmui_tracks=0), then the player's own folder (music_folder=)
    let songs = |dir: &str| {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav") || e.eq_ignore_ascii_case("mp3"))).collect())
            .unwrap_or_default();
        files.sort();
        files
    };
    let mut files = if setting("music_cmui_tracks").as_deref() == Some("0") { Vec::new() } else { songs("Data\\cmui_music") };
    let own = setting("music_folder").unwrap_or_default();
    let own = own.trim().trim_matches('"');
    if !own.is_empty() {
        files.extend(songs(own));
    }
    let tracks: Vec<_> = files.into_iter().map(|p| (track_name(&p), p)).collect();
    log(&format!("music: {} tracks (Data\\cmui_music, music_folder {own:?})", tracks.len()));
    let empty = tracks.is_empty();
    let _ = TRACKS.set(tracks);
    PLAYING.store(setting("music_playing").as_deref() != Some("0"), Relaxed);
    VOLUME.store(setting("music_volume").and_then(|v| v.parse().ok()).unwrap_or(50u32).min(100), Relaxed);
    UNMUTED = VOLUME.load(Relaxed).max(10);
    TRACK.store(setting("music_track").and_then(|v| v.parse().ok()).unwrap_or(0), Relaxed);
    if !empty {
        std::thread::spawn(run);
    }
}

fn run() {
    let tracks = TRACKS.get().unwrap();
    loop {
        if !PLAYING.load(Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        let k = TRACK.load(Relaxed) % tracks.len();
        let path = &tracks[k].1;
        let mp3 = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
        match std::fs::read(path).ok().as_deref().and_then(if mp3 { decode_mp3 } else { decode }) {
            Some((channels, rate, pcm)) if !pcm.is_empty() => unsafe { play(channels, rate, &pcm) },
            _ => {
                log(&format!("music: cannot play {:?}", tracks[k].1));
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
        SKIP.store(false, Relaxed);
        // (the next one, unless a click chose another meanwhile)
        let _ = TRACK.compare_exchange(k, (k + 1) % tracks.len(), Relaxed, Relaxed);
        CHANGED.store(true, Relaxed);
    }
}

/// One track: a loop (under 2 minutes, as cmui's own) twice, the second time fading out over its
/// last 4 seconds; a longer one, a song, once as it is. Pause holds
/// the device where it is, next (SKIP) ends it at once. The volume is applied to the samples (a
/// waveOut volume is the whole process's on Windows 7 and up: the game's own sounds too).
unsafe fn play(channels: u16, rate: u32, pcm: &[i16]) {
    let format = WaveFormat { tag: 1, channels, rate, bytes_per_sec: rate * channels as u32 * 2, align: channels * 2, bits: 16, extra: 0 };
    let mut out = 0usize;
    if waveOutOpen(&mut out, 0xffff_ffff, &format, 0, 0, 0) != 0 {
        log("music: waveOutOpen failed");
        PLAYING.store(false, Relaxed);
        return;
    }
    const BUFFERS: usize = 4;
    let ch = channels as usize;
    let frames = pcm.len() / ch;
    let chunk = (rate as usize / 10).max(1); // 100 ms a buffer: a volume change is heard soon
    let looped = frames < 120 * rate as usize;
    let (total, fade) = if looped { (2 * frames, 4 * rate as usize) } else { (frames, 1) };
    let mut buffers = vec![vec![0i16; chunk * ch]; BUFFERS];
    let size = std::mem::size_of::<WaveHeader>() as u32;
    let mut headers: Vec<WaveHeader> = buffers
        .iter_mut()
        .map(|b| WaveHeader { data: b.as_mut_ptr(), length: (b.len() * 2) as u32, recorded: 0, user: 0, flags: 0, loops: 0, next: 0, reserved: 0 })
        .collect();
    for h in headers.iter_mut() {
        waveOutPrepareHeader(out, h, size);
    }
    let mut queued = [false; BUFFERS];
    let (mut at, mut paused) = (0usize, false);
    while !SKIP.load(Relaxed) {
        let on = PLAYING.load(Relaxed);
        if on == paused {
            paused = !on;
            if paused { waveOutPause(out) } else { waveOutRestart(out) };
        }
        if !paused {
            let gain = (VOLUME.load(Relaxed) as f32 / 100.0).powi(2);
            for i in 0..BUFFERS {
                if queued[i] && std::ptr::read_volatile(&headers[i].flags) & 1 == 0 {
                    continue; // (WHDR_DONE not yet)
                }
                queued[i] = false;
                if at >= total {
                    continue;
                }
                for (f, frame) in buffers[i].chunks_exact_mut(ch).enumerate() {
                    let p = at + f;
                    let g = if p >= total { 0.0 } else { gain * ((total - p) as f32 / fade as f32).min(1.0) };
                    for (c, s) in frame.iter_mut().enumerate() {
                        *s = (pcm[(p % frames) * ch + c] as f32 * g) as i16;
                    }
                }
                at += chunk;
                waveOutWrite(out, &mut headers[i], size);
                queued[i] = true;
            }
            if at >= total && !queued.iter().any(|&q| q) {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    waveOutReset(out);
    for h in headers.iter_mut() {
        waveOutUnprepareHeader(out, h, size);
    }
    waveOutClose(out);
}

/// cmui.ini after a change made here (the main thread only: set_setting rewrites the file).
fn save() {
    set_setting("music_playing", if PLAYING.load(Relaxed) { "1" } else { "0" });
    set_setting("music_volume", &VOLUME.load(Relaxed).to_string());
}

/// The control, drawn at every flip (in a live match too); its clicks are taken here as well
/// as in poll_input (a match runs without it).
#[allow(static_mut_refs)]
pub unsafe fn flip(full_frame: bool) {
    start();
    if full_frame {
        BAR = BAR_SEEN.take();
    }
    if CHANGED.swap(false, Relaxed) {
        set_setting("music_track", &TRACK.load(Relaxed).to_string());
    }
    let _ = input(*(MOUSE_X as *const i32), *(MOUSE_Y as *const i32));
    BOXES = [(0, 0, -1, -1); 4];
    let (Some(bar), Some(tracks)) = (BAR, TRACKS.get()) else { return };
    if tracks.is_empty() || season::page_open() {
        return;
    }
    let (_, sh) = screen();
    let u = (sh * 30 / 1050).max(16); // a row: 30 px at 1680x1050
    let (x1, x2) = (bar.0, bar.2);
    let y2 = sh - u / 2;
    let y1 = y2 - u * 3 - u / 3;
    if y1 < bar.3 + u / 2 || x2 - x1 < 3 * u / 2 {
        return; // no room under the bar's buttons
    }
    fill(x1, y1, x2, y2, [0x1c, 0x20, 0x26]);
    fill(x1, y1, x2, y1 + 1, [0x3a, 0x40, 0x4a]);
    let font = SMALL_FONT.unwrap_or(0xffff0001);
    // the track's name
    let (playing, volume) = (PLAYING.load(Relaxed), VOLUME.load(Relaxed));
    let name = &tracks[TRACK.load(Relaxed) % tracks.len()].0;
    let name = clip_text(font, &to_latin1(name), x2 - x1 - 8);
    text_at((x1 + x2) / 2, y1 + u / 6, y1 + u, font, if playing { GOLD } else { GREY }, &name, 1);
    // play / pause, next, mute
    let (by1, by2) = (y1 + u, y1 + u * 2 + u / 3);
    let w = (x2 - x1) / 3;
    let icons = [if playing { 0xE769 } else { 0xE768 }, 0xE893, if volume == 0 { 0xE74F } else { 0xE767 }];
    for (k, code) in icons.into_iter().enumerate() {
        let b = (x1 + w * k as i32, by1, x1 + w * (k as i32 + 1), by2);
        BOXES[k] = b;
        draw_icon(code, (b.0 + b.2) / 2, by1 + (by2 - by1 - u * 2 / 3) / 2, u * 2 / 3, [0xd8, 0xdc, 0xe2]);
    }
    // the volume bar
    let (vx1, vx2, vy) = (x1 + u / 3, x2 - u / 3, by2 + u / 3);
    BOXES[3] = (vx1, vy - u / 3, vx2, vy + u / 3);
    fill(vx1, vy - 2, vx2, vy + 2, [0x3a, 0x40, 0x4a]);
    fill(vx1, vy - 2, vx1 + (vx2 - vx1) * volume as i32 / 100, vy + 2, GOLD);
}

/// A click let go over the control: Some(redraw) when it took it; the edge of the button seen
/// from either caller once.
pub unsafe fn input(mx: i32, my: i32) -> Option<bool> {
    let held = *(BUTTON_HELD as *const u32) != 0;
    let released = std::mem::replace(&mut WAS_HELD, held) && !held;
    let inside = |r: Rect| (r.0..=r.2).contains(&mx) && (r.1..=r.3).contains(&my);
    #[allow(static_mut_refs)]
    let k = BOXES.iter().position(|&b| inside(b) && released)?;
    *(LEFT_CLICK as *mut u32) = 0;
    match k {
        0 => {
            PLAYING.fetch_xor(true, Relaxed);
        }
        1 => {
            if let Some(n) = TRACKS.get().map(|t| t.len()).filter(|&n| n > 0) {
                TRACK.store((TRACK.load(Relaxed) + 1) % n, Relaxed);
                SKIP.store(true, Relaxed);
                PLAYING.store(true, Relaxed);
                set_setting("music_track", &TRACK.load(Relaxed).to_string());
            }
        }
        2 => {
            let v = VOLUME.load(Relaxed);
            if v > 0 {
                UNMUTED = v;
            }
            VOLUME.store(if v > 0 { 0 } else { UNMUTED.max(10) }, Relaxed);
        }
        _ => {
            let (x1, x2) = (BOXES[3].0, BOXES[3].2);
            let v = ((mx - x1) * 100 / (x2 - x1).max(1)).clamp(0, 100);
            VOLUME.store(((v + 5) / 10 * 10) as u32, Relaxed); // (steps of 10)
        }
    }
    save();
    Some(true)
}

/// The thread changed track: the screen is drawn again (the control's name).
pub fn changed() -> bool {
    CHANGED.load(Relaxed)
}

/// MP3s made by ffmpeg (an ID3v2 tag in front, as it writes): 3 s of 440 Hz back as 3 s of 440 Hz.
#[cfg(test)]
#[test]
fn decodes_an_mp3() {
    let dir = std::env::temp_dir().join("cmui_mp3_test");
    let _ = std::fs::create_dir_all(&dir);
    // (CBR stereo, CBR mono MPEG 2, and VBR as most people's MP3s are)
    for (rate, ch, quality) in [(44100u32, 2u16, ["-b:a", "128k"]), (22050, 1, ["-b:a", "64k"]), (44100, 2, ["-q:a", "2"])] {
        let mp3 = dir.join(format!("t{rate}{}.mp3", quality[0]));
        let made = std::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=3", "-ar", &rate.to_string(), "-ac", &ch.to_string(), quality[0], quality[1]])
            .arg(&mp3)
            .status()
            .is_ok_and(|s| s.success());
        if !made {
            eprintln!("no ffmpeg: MP3 test skipped");
            return;
        }
        let (c, r, pcm) = decode_mp3(&std::fs::read(&mp3).unwrap()).expect("decoded");
        assert_eq!((c, r), (ch, rate));
        let frames = pcm.len() / c as usize;
        assert!((frames as f64 / rate as f64 - 3.0).abs() < 0.2, "{frames} frames");
        // one second from the middle, the first channel: 880 zero crossings for 440 Hz
        let x: Vec<i16> = pcm.iter().step_by(c as usize).skip(rate as usize).take(rate as usize).copied().collect();
        let crossings = x.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count();
        assert!((crossings as i32 - 880).abs() < 10, "{crossings} crossings");
    }
}

#[cfg(test)]
#[test]
fn decodes_its_own_tracks() {
    // the IMA decoder against a block made by hand: first sample 100, index 0, codes 7 and 0x0f
    let mut wav = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
    wav.extend(20u32.to_le_bytes());
    for v in [0x11u16, 1] {
        wav.extend(v.to_le_bytes());
    }
    wav.extend(22050u32.to_le_bytes());
    wav.extend(11100u32.to_le_bytes());
    for v in [6u16, 4, 2, 5] {
        wav.extend(v.to_le_bytes());
    }
    wav.extend(b"fact");
    wav.extend(4u32.to_le_bytes());
    wav.extend(3u32.to_le_bytes());
    wav.extend(b"data");
    wav.extend(6u32.to_le_bytes());
    wav.extend([100, 0, 0, 0, 0xf7, 0]);
    // code 7 at step 7: 0+7+3+1 = 11 -> 111, index 0+8; code 0xf at step 16: -(2+16+8+4) = -30 -> 81
    let (ch, rate, pcm) = decode(&wav).unwrap();
    assert_eq!((ch, rate, &pcm[..3]), (1, 22050, &[100i16, 111, 81][..]));
    assert_eq!(track_name(std::path::Path::new("Data\\cmui_music\\03 Sunday League.wav")), "Sunday League");
    // the tracks themselves, when built
    for f in std::fs::read_dir("music\\out").into_iter().flatten().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "wav")) {
        let (ch, rate, pcm) = decode(&std::fs::read(f.path()).unwrap()).unwrap();
        assert!(ch == 1 && rate == 22050 && pcm.len() > 22050 * 30, "{:?}", f.path());
    }
}
