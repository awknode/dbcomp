# dBCOMP manual

Every control in dBCOMP, with its range, default and what it does.

## Signal flow

```
in ─► detector (linked stereo, ~3 ms RMS) ─► gain computer (6 dB soft knee) ─► GR ballistics
   ─► gain ─► [WARM] ─► GAIN ─► SAT ─► MIX with dry ─► SAFE
```

Zero latency.

## Faders

| Control | Range (default) | What it does |
|---|---|---|
| **THRESH** | -48 to 0 dB (0 dB) | Level where compression starts. Pull it down for more gain reduction. |
| **RATIO** | 1:1 to 20:1 (4:1) | How hard it compresses above the threshold. 2:1 gentle, 4:1 everyday, 8:1 and up for hard control. |
| **ATTACK** | 0.5 to 200 ms (10 ms) | How fast it grabs. Slower lets the front of a hit through (more punch); faster catches peaks. |
| **RELEASE** | 5 to 2000 ms (120 ms) | How fast it lets go. With ARC on, this is the centre the auto release moves around. |
| **GAIN** | -18 to +18 dB (0 dB) | Output (make-up) gain after the compressor. |

## Switches (footer)

| Control | Default | What it does |
|---|---|---|
| **ARC** | on | Auto release: the release follows the material's crest factor. Longer on sustained sounds (no pumping, clean lows), quicker on punchy ones. |
| **ELECTRO / OPTO** | ELECTRO | How the release moves with the gain reduction. ELECTRO lets go quickly near 0 dB (punchy). OPTO lets go quickly from deep reduction, then eases in slowly (smooth, vocal-friendly). |
| **WARM / SMOOTH** | WARM | WARM adds a little harmonic colour that grows with the gain reduction. SMOOTH is clean. |
| **SAFE** | on | Output limiter: never over 0 dBFS. |

## SAT

| Control | Range (default) | What it does |
|---|---|---|
| **SAT** | 0 to 100% (0%) | One-knob saturator after the compressor, level-matched so turning it up adds drive, not volume. |
| **CLEAN** | LOWS, NONE, HIGHS (NONE) | Keeps a part of the spectrum out of the saturation. LOWS: bass stays clean (good on 808 and bass). HIGHS: top stays clean (no fizz). NONE: saturate everything. |

## Top strip

| Control | Range (default) | What it does |
|---|---|---|
| **MIX** | 0 to 100% (100%) | Blend with the dry signal (parallel compression). |
| **Power** | on / off | Bypass. |

## Meters

**GR** shows gain reduction (red), **OUT** the output level. The number at top right is the current gain reduction.
