# Menu music (switch `music`, index 41)

Asked by the user 2026-11-24: "we have sound in cm, but i miss music". cmui plays its own synth
menu music beside the game's sounds, with a small player at the foot of the menu bar.

## The music

`ui\music\make_music.py` (numpy only) renders four original tracks into `ui\music\out\`:

| File | Tempo, key | Feel | Loop |
|---|---|---|---|
| `01 Kick Off.wav` | 124, D major | house piano stabs, square lead, brass-synth second theme, arp break | 62 s |
| `02 Deadline Day.wav` | 132, E minor | driving 16th arp and octave bass, saw lead, brass long notes, bell break | 58 s |
| `03 Sunday League.wav` | 98, F major | FM electric piano, flute-like lead with vibrato, bell counter-line, shaker groove | 78 s |
| `04 Matchday.wav` | 128, G major | synth strings, tom intro, brass-synth anthem, square lead, piano stabs | 75 s |

Each track is written as sections over a chord list (intro 4 bars, theme A 8, theme B 8, break 4,
theme again), the melodies hand-written per bar (`melody()`: `NOTE:beats`, bars split by `|`, each
bar checked to be 4 beats; connected notes overlap a little, legato). Parts: `pads`, `bassline`
(offbeat / anthem / octave16 / drive / chill), `stabs`, `arp`, `beat` (house / drive / chill),
`roll`, `crash`. Voices are synthesised: polyBLEP saw and square, FFT 2-pole filters, a bright and a
dark copy crossfaded for a filter sweep, FM for the electric piano and bell, additive partials for
the piano; drums made once (kick, snare, clap, hats, shaker, crash, tom). Mix: bass and pads duck
under each kick (sidechain), a dotted-eighth echo on the melody, a small hall, a high-pass at 30 Hz,
loudness evened out with a tanh.

**Seamless loops:** everything that rings past a track's end (notes, echo, hall) is wrapped round
to its start, so the file plays back to back without a seam; the script asserts the jump from the
last sample to the first is no bigger than the track's own sample steps.

**Format:** mono, 22050 Hz, IMA ADPCM WAV (4 bits a sample, about 11 KB a second): 2.9 MB for all
four. Windows' own players play these files too. `--pcm` also writes 16-bit previews (two loops each)
to `music\out\pcm\` for listening outside the game. The ADPCM error is printed per track (~4-6 %
of the signal's RMS).

Re-render: `python music\make_music.py`, then `install_all.ps1` (it copies `music\out\*.wav` to
every folder's `Data\cmui_music\`).

## The player (`src\music.rs`)

- Started on the first flip: lists `Data\cmui_music\*.wav` by name (the track's name is the file
  name without its leading number), reads `cmui.ini`, starts one thread.
- The thread decodes a track (16-bit PCM of any channels and rate, or mono IMA ADPCM) and feeds
  winmm `waveOut` with four 100 ms buffers. Windows mixes it with the game's DirectSound sounds.
  Each track plays its loop twice, the second time fading out over the last 4 seconds, then the
  next track.
- Volume is applied to the samples (gain = (volume/100)^2). A waveOut volume would be the whole
  process's on Windows 7 and up, so it would turn the game's own sounds down too.
- Pause holds the device (`waveOutPause`); Next ends the track at once.
- `cmui.ini`: `music_playing` (1/0), `music_volume` (0-100, default 50), `music_track` (the one
  playing; play starts there next time). Written only from the game's thread.
## Your own music (2026-11-24, asked by the user)

- `music_folder=C:\Users\me\Music\CM` in the game folder's `cmui.ini`: its MP3s and WAVs play
  after cmui's own tracks, in file-name order (a leading number is left out of the name shown).
  Read when the game starts.
- `music_cmui_tracks=0`: only your own folder's songs.
- Files in `Data\cmui_music` itself play too (MP3 or WAV).
- MP3: Windows' own MP3 decoder through the ACM (`msacm32`; the Fraunhofer codec every Windows
  has), CBR and VBR, MPEG 1 and 2, an ID3v2 tag in front and an ID3v1 tag at the end skipped
  (`decode_mp3`, `mp3_first_frame`). Test `decodes_an_mp3`: three MP3s made by ffmpeg (CBR stereo,
  CBR mono MPEG 2, VBR) decode to 3 s of 440 Hz. WAV: 16-bit PCM (any channels and rate) or mono
  IMA ADPCM.
- A track under 2 minutes is taken for a loop (played twice, the second time fading out); a
  longer one is a song, played once as it is.
- The whole song is decoded into memory before it plays (about 10 MB a stereo minute, freed after
  it): a very long file takes that much while it plays.

## The control

At the foot of the menu bar, under its lowest button: `sidebar_item` widens `music::BAR_SEEN`
with every bar button it draws (Game Options at its moved place included). On a full frame that
becomes the bar, and the control is drawn under it only where there is room (3.3 rows of
30 px at 1680x1050, scaled with the screen height). It is not drawn on screens without the menu
bar, or under the season review card.

- Row 1: the track's name (gold while playing, grey when paused), clipped to the bar.
- Row 2: play / pause, next, mute (Segoe Fluent / MDL2 icons E768 / E769, E893, E767 / E74F).
- Row 3: the volume bar (gold to the volume). A click sets the volume in steps of 10.

Clicks are taken when the left button is let go over a part, from `poll_input` (most screens) and
from the flip (a live match runs without `poll_input`); one edge detector is shared, so a click
counts once. The game's click flag is cleared for it. A track change by the thread sets `REDRAW`
so the name is drawn again.

## Network

Each game plays its own music: a client's DLL starts its own thread. Nothing goes over the network.

## Not done / open

- Heard by no one yet: the tracks were checked by numbers only (levels a section, the share of
  energy per band, no clipping, the loop seam), and the control has not been seen in the game.
- 800x600 and 1280x800: the control is drawn only where there is room under the bar's buttons;
  on a small screen the bar may be full, and then there is no control (the music still plays).
- The friends' zips: `cmui_package\make_cm25_zip.py` and `make_cm2627_zip.py` put cmui's four
  tracks in `Data\cmui_music`, taken from `ui\music\out` (never the game folder's own
  `Data\cmui_music`, where the owner's own songs may be). An update zip carries them as extras:
  `make_dll_zip.py <dll> <readme> "...\01 Kick Off.wav=Data/cmui_music/01 Kick Off.wav" ...`.
- The music plays on while the game is in the background or in a match (pause is one click).
