//! Camera gestures and cut accents, in global frames.

use fframes::Svgr;

fn progress(index: usize, start: usize, end: usize) -> f32 {
    ((index as f32 - start as f32) / (end - start) as f32).clamp(0.0, 1.0)
}

pub(crate) fn apply<'a>(index: usize, name: &str, composition: Svgr<'a>) -> Svgr<'a> {
    let (mut scale, mut opacity) = (1.0, 1.0);
    match name {
        // Punch in out of the boot glitch.
        "Cover" => scale = 1.0 + 2.5 * (1.0 - progress(index, 22, 31)).powi(3),
        // Dive into the model before the views.
        "Model" => scale = 1.0 + progress(index, 447, 452).powi(3) * 8.0,
        "Brand4" => scale = 1.0 + progress(index, 812, 816).powi(3) * 1.3,
        "Outro" => opacity = progress(index, 822, 846).powi(2),
        _ => {}
    }
    let flash = if (150..160).contains(&index) {
        (-((index as f32 - 154.0) / 2.7).powi(2)).exp()
    } else if (449..456).contains(&index) {
        (-((index as f32 - 451.0) / 1.3).powi(2)).exp() * 0.45
    } else if (764..769).contains(&index) {
        (-((index as f32 - 766.0) / 1.2).powi(2)).exp() * 0.35
    } else {
        0.0
    };
    let glitch = if (609..614).contains(&index) { progress(index + 1, 609, 614) } else { 0.0 };
    let bars: Vec<_> = if glitch > 0.0 {
        (0..28)
            .map(|row| {
                let hash = ((row * 73 + index * 19) % 101) as f32 / 101.0;
                let y = row as f32 * 1080.0 / 28.0;
                let height = 1080.0 / 28.0 * (glitch * 1.5 - hash * 0.45).clamp(0.02, 1.0);
                fframes::svgr!(<rect x="0" y={y} width="1920" height={height} fill="#000" />)
            })
            .collect()
    } else {
        Vec::new()
    };
    fframes::svgr!(
        <g>
            <defs><clipPath id="shot-canvas"><rect width="1920" height="1080" /></clipPath></defs>
            <g clip-path="url(#shot-canvas)">
                <g opacity={opacity} transform={format!("translate(960 540) scale({scale}) translate(-960 -540)")}>
                    {composition}
                </g>
                {bars}
                <rect width="1920" height="1080" fill="#fffcf2" opacity={flash} />
            </g>
        </g>
    )
}
