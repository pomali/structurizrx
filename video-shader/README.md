# structurizrx intro — shader-mode style

A 30-second, 1920×1080 introduction to structurizrx, in the style of fframes'
[`shader-mode`](https://github.com/dmtrKovalenko/fframes/tree/main/examples/shader-mode)
example: every shot is a Skia GPU shader under SVG typography, cut hard on the frame.
The story matches `../video` (problem → one model → views → verification → release).

```sh
cargo run --release -- preview          # real-time window
cargo run --release -- render -o out.mp4
```

- `src/edit.rs` — the shot list (frames, shader, variant).
- `src/overlays.rs` — typography and panels per shot; `src/transitions.rs` — punches, flashes, glitch bars.
- `media/wordmark-sdf.png` — signed-distance mask of the wordmark used by the emboss shots;
  regenerate with `python3 tools/make_sdf.py media/DMSans-Medium.ttf media/wordmark-sdf.png structurizrx 900`.

## Credits and licenses

The shaders in `src/shaders/`, `media/binary-atlas.png` and the outro/transition code are
adapted from the fframes `shader-mode` example. Parts of those shaders are adapted from
[Shader Effects Inc.](https://github.com/shader-effects-inc/shaders/tree/935f71a7789f0e07811dfe6fd0d8f707e9848238)
under the MIT License:

> MIT License
>
> Copyright (c) 2026 Shader Effects Inc.
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

Fonts (DM Sans, Instrument Serif, JetBrains Mono) are under the SIL Open Font License.
