# Menu music (switch `music`, index 41)

Asked by the user 2026-11-24: "we have sound in cm, but i miss music". cmui plays its own menu
music beside the game's sounds, with a small player at the foot of the menu bar.

## The tracks (since 2026-11-24, late evening)

The user wanted music like Europa Universalis 4's (Andreas Waldetoft, Paradox: copyrighted, not
free to ship; "otherwise swap them"). Eight orchestral tracks by Alexander Nakarada
(CreatorChords, **CC BY 4.0: credit required**, `music\tracks\CREDITS.txt` goes into every
`Data\cmui_music`), picked by the site's own descriptions (epic / medieval / orchestral
cinematic; nothing dark, metal or rock): 01 The Crown, 02 Hymn To The Gods, 03 Adventure,
04 Medieval Chateau, 05 Glory, 06 Chivalry Fair, 07 Gates Of Glory, 08 Legends.

Downloaded from free-stock-music.com (`/music/alexander-nakarada/mp3/alexander-nakarada-<slug>.mp3`,
320 kbps; the file needs the track page as referrer, as its Download button sends), re-encoded
by ffmpeg to 128 kbps 44.1 kHz stereo, 25 MB for all eight. Bandcamp (bot check) and Scott
Buckley's site (ModSecurity) refused scripted downloads and were left alone. `install_all.ps1`
copies `music\tracks` and takes the earlier sets out of `Data\cmui_music` by name (the owner's
own songs there stay). The test `decodes_its_own_tracks` decodes every shipped MP3.

Before (same evening): eight Kevin MacLeod tracks (Electronica / Funk, picked by incompetech's
`pieces.json` tags); the user did not like them. Folder `music\kevin_macleod` deleted.

## The first synth tracks (deleted 2026-11-24)

Four synth tracks (Kick Off, Deadline Day, Sunday League, Matchday) rendered by a numpy script,
`ui\music\make_music.py`, as mono 22050 Hz IMA ADPCM WAVs. Deleted on the user's yes once the
Kevin MacLeod tracks were in; script and WAVs are in git at 00a8051
(`git checkout 00a8051 -- ui/music/make_music.py ui/music/out`). The player still plays such WAVs.

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
- The friends' zips: `cmui_package\make_cm25_zip.py` and `make_cm2627_zip.py` put cmui's tracks
  and CREDITS.txt in `Data\cmui_music`, taken from `ui\music\tracks` (never the game folder's own
  `Data\cmui_music`, where the owner's own songs may be). An update zip carries them as extras:
  `make_dll_zip.py <dll> <readme> "...\01 The Crown.mp3=Data/cmui_music/01 The Crown.mp3" ...`.
- The music plays on while the game is in the background or in a match (pause is one click).
