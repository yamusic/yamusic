# YaMusic

A terminal-based client for Yandex Music (unofficial). Not affiliated with Yandex in any way. Feature-complete for day to day use, but still a WIP.

<table>
<td><p align="center">My Wave</p><img src="https://github.com/user-attachments/assets/9f2c9541-8cbd-4e38-a003-3ba9d0d9391f" width="600"/></td>
<td><p align="center">Lyrics</p><img src="https://github.com/user-attachments/assets/0665f5b2-fabf-4d03-9d34-54073bfbfec5" width="600"/></td>
<td><p align="center">Equalizer</p><img src="https://github.com/user-attachments/assets/f7d8a64a-0e18-49d8-8c9c-8fa539485a3f" width="600"/></td>

</tr>
</table>

## Disclaimer

**yamusic** is an open-source project created for educational purposes. It makes use of unofficial and undocumented APIs that are not meant for public use. Use at your own risk.

## Installation

```bash
cargo install yamusic
```

## Features

- **Cross-platform**
- **Fast & Lightweight** (≈0.2–0.5% CPU on a Pentium)
- **GPU-Accelerated, Reactive Audio Visualizations**
- **Buffered Streaming Playback**
- **Efficient Track Preloading**
- **Modal Keymaps**
- **Synced Lyrics**
- **My Wave Stations**
- **Fuzzy Search**
- **Toast Notifications**

## Keymaps

yamusic uses a modal approach to keyboard shortcuts.

### UI Navigation
- `1` - Go to Search
- `2` - Go to Home
- `3` - Go to Liked Tracks
- `4` - Go to Playlists
- `Tab` / `Shift+Tab` - Cycle between UI tabs
- `Esc` - Go back / Dismiss overlay

## View Navigation
- `j` / `k` - Move down / up
- `gg` / `G` - Go to top / bottom
- `/` - Search within current view
- `Enter` - Play selected track or open selected item

### Playback Controls
- `Space` - Toggle Play/Pause
- `.` / `,` - Next / Previous track
- `+` / `-` - Volume Up / Down
- `m` - Toggle Mute
- `s` - Toggle Shuffle
- `r` - Cycle Repeat modes
- `>` / `<` - Seek Forward / Backward (10s)

### Action Prefixes
Some actions require a sequence of keys:

#### `v` (View/Library Actions)
- `v` + `f` - Like the entire current view (playlist, artist, album, etc.)
- `v` + `d` - Dislike the entire current view
- `v` + `q` - Queue all tracks in view
- `v` + `n` - Play all tracks in view next
- `v` + `w` - Start a Wave based on this view

#### `q` (Queue Management)
- `q` + `a` - Add selected to queue
- `q` + `n` - Play selected next
- `q` + `d` - Remove selected from queue
- `q` + `c` - Clear the queue

#### `c` (Context Actions)
- `c` + `f` - Like the currently playing track
- `c` + `d` - Dislike the currently playing track
- `c` + `w` - Start a Wave from the current track

#### `g` (Go/Jump)
- `g` + `q` - Open Queue
- `g` + `y` - Open Lyrics
- `g` + `e` - Open Effects

### Selection Actions
- `f` - Like selected track
- `d` - Dislike selected track
- `w` - Start "My Wave" from selected track
- `Ctrl+c` - Quit

### My Wave View
- `w` - Customize and start a "My Wave" station

## Acknowledgements

- [Audio EQ Cookbook](https://www.w3.org/TR/audio-eq-cookbook/)
- [Dattorro Effect Design](https://ccrma.stanford.edu/~dattorro/EffectDesignPart1.pdf)
