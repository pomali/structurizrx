//! The edit: hard cuts in 30 fps frames. Shots that share a `clock_start` keep their
//! shader's motion continuous across a colour cut.

use crate::Effect;

pub(crate) const FRAMES: usize = 900;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShotSpec {
    pub name: &'static str,
    pub start: usize,
    pub end: usize,
    pub effect: Option<Effect>,
    pub variant: u8,
    pub clock_start: usize,
}

impl ShotSpec {
    pub const fn frames(self) -> usize {
        self.end - self.start
    }
}

const fn shot(name: &'static str, start: usize, end: usize, effect: Option<Effect>, variant: u8, clock_start: usize) -> ShotSpec {
    ShotSpec { name, start, end, effect, variant, clock_start }
}

use Effect::*;

pub(crate) const SHOTS: &[ShotSpec] = &[
    // Cover: a binary boot glitch hands over to a glass lens.
    shot("Boot", 0, 22, Some(Binary), 0, 0),
    shot("Cover", 22, 70, Some(Glass), 0, 22),
    shot("CoverAmber", 70, 108, Some(Glass), 1, 22),
    // The problem: stale diagrams flicker past, then the statement on chrome.
    shot("Fig1", 108, 120, Some(Aurora), 1, 108),
    shot("Fig2", 120, 132, Some(Aurora), 2, 108),
    shot("Fig3", 132, 144, Some(Aurora), 3, 108),
    shot("Fig4", 144, 154, Some(Aurora), 4, 108),
    shot("Drift", 154, 228, Some(LiquidChrome), 0, 154),
    shot("Break", 228, 236, None, 0, 228),
    // One model, as text.
    shot("ModelTitle", 236, 274, Some(Gradient), 0, 236),
    shot("Model", 274, 452, Some(Flow), 0, 274),
    // Views are queries.
    shot("ViewsTitle", 452, 484, Some(Rays), 0, 452),
    shot("View1", 484, 516, Some(Obsidian), 0, 484),
    shot("View2", 516, 548, Some(Obsidian), 1, 484),
    shot("View3", 548, 580, Some(Particles), 0, 548),
    shot("Agree", 580, 614, Some(Spectral), 0, 580),
    // Checked like code.
    shot("Checked", 614, 766, Some(Spectral), 2, 580),
    // The wordmark, embossed in four finishes.
    shot("Brand1", 766, 778, Some(Emboss), 0, 766),
    shot("Brand2", 778, 790, Some(Emboss), 1, 766),
    shot("Brand3", 790, 802, Some(Emboss), 2, 766),
    shot("Brand4", 802, 816, Some(Emboss), 3, 766),
    shot("Pause", 816, 822, None, 0, 816),
    shot("Outro", 822, 900, Some(Eclipse), 0, 822),
];
