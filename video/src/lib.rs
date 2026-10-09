//! structurizrx: a 30-second introduction, drawn as the six sheets of one specification.
//!
//! Every scene is a sheet of the same drawing: a gridded page, a sheet border, a title
//! block, a numbered section header. Blue marks the model, red marks corrections.
use fframes::{
    AnimateRuntimeInput, AudioMap, Color, Duration, FFramesContext, Frame, Overlap, Scene,
    Scenes, Svgr, Video,
    animation::{AnimationRuntime, Easing},
    include_media_dir,
};
use std::sync::LazyLock;

include_media_dir!(pub struct IntroMedia, "media");

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;

const PAPER: &str = "#f3f1ea";
const CARD: &str = "#fbfaf6";
const INK: &str = "#16191d";
const MUTED: &str = "#6a6e70";
const GRID: &str = "#e5e2d8";
const GRID_MAJOR: &str = "#d6d2c5";
const BLUE: &str = "#1e4fd8";
const BLUE_TINT: &str = "#e3e9fb";
const RED: &str = "#d23b1e";
const TERM_RED: &str = "#ff8466";
const TERM_BLUE: &str = "#9db7ff";
const MONO: &str = "IBM Plex Mono";
const COND: &str = "IBM Plex Sans Condensed";

/// Cross-fade between sheets.
const OVERLAP: f32 = 0.4;
/// Sheet durations (without overlap); they sum to 30 s.
const DURATIONS: [f32; 6] = [4.0, 5.5, 6.0, 5.0, 5.5, 4.0];
const SHEET_NAMES: [&str; 6] = ["COVER", "THE PROBLEM", "THE MODEL", "VIEWS", "VERIFICATION", "RELEASE"];

static EASE: LazyLock<AnimationRuntime> =
    LazyLock::new(|| AnimationRuntime::new(0.7, &Easing::CubicBezier(0.16, 1.0, 0.3, 1.0)));
static DRAW: LazyLock<AnimationRuntime> =
    LazyLock::new(|| AnimationRuntime::new(0.6, &Easing::CubicBezier(0.65, 0.0, 0.35, 1.0)));
static SPRING: LazyLock<AnimationRuntime> = LazyLock::new(|| {
    AnimationRuntime::new(3.0, &Easing::Spring { mass: 1.0, stiffness: 180.0, damping: 20.0 })
});
static STAMP: LazyLock<AnimationRuntime> = LazyLock::new(|| {
    AnimationRuntime::new(3.0, &Easing::Spring { mass: 1.0, stiffness: 320.0, damping: 18.0 })
});

pub struct IntroVideo<'a> {
    pub media: &'a IntroMedia,
}

impl<'a> IntroVideo<'a> {
    pub fn new(media: &'a IntroMedia) -> Self {
        Self { media }
    }
}

impl std::fmt::Debug for IntroVideo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntroVideo").finish()
    }
}

impl Video for IntroVideo<'_> {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::WHITE;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![&Cover as &dyn Scene, &Problem, &Model, &Views, &Checked, &Release])
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let total: f32 = DURATIONS.iter().sum();

        // The sheet number flips in the middle of each cross-fade.
        let mut sheet = 0;
        let mut end = 0.0;
        for (i, d) in DURATIONS.iter().enumerate() {
            end += d;
            if t < end - OVERLAP / 2.0 || i == DURATIONS.len() - 1 {
                sheet = i;
                break;
            }
        }

        let progress = (t / total * 980.0).clamp(0.5, 980.0);
        let section_ticks: Vec<Svgr> = (0..=DURATIONS.len())
            .map(|i| {
                let x = 160.0 + DURATIONS[..i].iter().sum::<f32>() / total * 980.0;
                fframes::svgr!(<line x1={x} y1="982" x2={x} y2="998" stroke={INK} stroke-width="1.5" />)
            })
            .collect();
        let zone_ticks: Vec<Svgr> = (1..8)
            .map(|i| {
                let x = 56.0 + i as f32 * 1808.0 / 8.0;
                let y = 56.0 + i as f32 * 968.0 / 8.0;
                fframes::svgr!(<g stroke={INK} stroke-width="1.5">
                    <line x1={x} y1="40" x2={x} y2="56" />
                    <line x1={x} y1="1024" x2={x} y2="1040" />
                    <line x1="40" y1={y} x2="56" y2={y} />
                    <line x1="1864" y1={y} x2="1880" y2={y} />
                </g>)
            })
            .collect();
        let timecode = format!("T+{:02}:{:04.1}", (t / 60.0) as u32, t % 60.0);

        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT} viewBox="0 0 1920 1080">
                <defs>
                    <pattern id="minor" width="40" height="40" patternUnits="userSpaceOnUse">
                        <path d="M40 0 H0 V40" fill="none" stroke={GRID} stroke-width="1" />
                    </pattern>
                    <pattern id="major" width="200" height="200" patternUnits="userSpaceOnUse">
                        <rect width="200" height="200" fill="url(#minor)" />
                        <path d="M200 0 H0 V200" fill="none" stroke={GRID_MAJOR} stroke-width="1.5" />
                    </pattern>
                </defs>
                <rect width="1920" height="1080" fill={PAPER} />
                <rect x="56" y="56" width="1808" height="968" fill="url(#major)" />
                // Sheet border: outer and inner frame, zone ticks.
                <rect x="40" y="40" width="1840" height="1000" fill="none" stroke={INK} stroke-width="2.5" />
                <rect x="56" y="56" width="1808" height="968" fill="none" stroke={INK} stroke-width="1" />
                {zone_ticks}

                {ctx.render_scenes(&frame)}

                // Progress along the document, one tick per section.
                <g font-family={MONO} font-weight="500">
                    <rect x="156" y="944" width="1004" height="72" fill={PAPER} />
                    <text x="160" y="970" font-size="18" letter-spacing="3" fill={MUTED}>"DWG SX-000  ·  SCALE 1:1  ·  UNITS: ELEMENTS"</text>
                    <line x1="160" y1="990" x2="1140" y2="990" stroke={GRID_MAJOR} stroke-width="2" />
                    <rect x="160" y="988" width={progress} height="4" fill={BLUE} />
                    {section_ticks}
                </g>
                // Title block.
                <g font-family={MONO}>
                    <rect x="1184" y="944" width="680" height="80" fill={PAPER} stroke={INK} stroke-width="1.5" />
                    <path d="M1424 944 V1024 M1584 944 V1024 M1734 944 V1024" stroke={INK} stroke-width="1" />
                    <g font-size="15" letter-spacing="2" fill={MUTED} font-weight="500">
                        <text x="1198" y="966">"TITLE"</text>
                        <text x="1438" y="966">"SECTION"</text>
                        <text x="1598" y="966">"SHEET"</text>
                        <text x="1748" y="966">"TIME"</text>
                    </g>
                    <g font-size="22" fill={INK} font-weight="600">
                        <text x="1198" y="1004">"STRUCTURIZRX"</text>
                        <text x="1438" y="1004" font-size="17">{SHEET_NAMES[sheet]}</text>
                        <text x="1598" y="1004">{format!("{} / {}", sheet + 1, DURATIONS.len())}</text>
                        <text x="1748" y="1004" font-size="19" font-weight="500">{timecode}</text>
                    </g>
                </g>
            </svg>
        )
    }
}

// ---------------------------------------------------------------------------------------
// Helpers

/// 0 to 1 over 0.7 s from `start` (seconds into the scene), ease-out.
fn ramp(frame: &Frame, start: f32) -> f32 {
    frame.animate_runtime(AnimateRuntimeInput { on_second: start, from: 0.0, to: 1.0, animation_runtime: &EASE })
}

/// 0 to 1 over 0.6 s, ease-in-out: for lines being drawn.
fn draw(frame: &Frame, start: f32) -> f32 {
    frame.animate_runtime(AnimateRuntimeInput { on_second: start, from: 0.0, to: 1.0, animation_runtime: &DRAW })
}

/// Vertical offset springing from 40 px to 0.
fn rise(frame: &Frame, start: f32) -> f32 {
    frame.animate_runtime(AnimateRuntimeInput { on_second: start, from: 40.0, to: 0.0, animation_runtime: &SPRING })
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Opacity of a whole sheet: fades in over the overlap, out over its last overlap.
fn sheet_opacity(frame: &Frame, index: usize) -> f32 {
    let t = frame.seconds();
    let len = DURATIONS[index] + if index > 0 { OVERLAP } else { 0.0 };
    let fade_in = if index > 0 { smooth(t / OVERLAP) } else { 1.0 };
    let fade_out = if index + 1 < DURATIONS.len() { smooth((len - t) / OVERLAP) } else { 1.0 };
    fade_in.min(fade_out)
}

/// The first `n` characters of `s`, where `n` grows at `cps` characters per second.
fn typed(s: &str, t: f32, start: f32, cps: f32) -> &str {
    let n = ((t - start) * cps).max(0.0) as usize;
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// An arrow from (x1, y1) to (x2, y2), drawn to `p` of its length; the head lands at the end.
fn arrow<'a>(x1: f32, y1: f32, x2: f32, y2: f32, p: f32, color: &'a str) -> Svgr<'a> {
    if p <= 0.001 {
        return Svgr::empty();
    }
    let ex = x1 + (x2 - x1) * p;
    let ey = y1 + (y2 - y1) * p;
    let angle = (y2 - y1).atan2(x2 - x1).to_degrees();
    let head = ((p - 0.8) / 0.2).clamp(0.0, 1.0);
    fframes::svgr!(<g>
        <line x1={x1} y1={y1} x2={ex} y2={ey} stroke={color} stroke-width="2" />
        <g transform={format!("translate({x2} {y2}) rotate({angle})")} opacity={head}>
            <path d="M0 0 L-16 -7 L-16 7 Z" fill={color} />
        </g>
    </g>)
}

/// A C4-style element box centred on (cx, cy).
#[allow(clippy::too_many_arguments)]
fn node<'a>(cx: f32, cy: f32, w: f32, h: f32, name: &'a str, kind: &'a str, size: f32, focus: bool, opacity: f32, lift: f32) -> Svgr<'a> {
    let (fill, stroke, text, sub) = if focus { (BLUE, BLUE, CARD, BLUE_TINT) } else { (CARD, INK, INK, MUTED) };
    let rx = if kind == "person" { h / 2.0 } else { 4.0 };
    let has_kind = !kind.is_empty();
    let name_y = if has_kind { cy + size * 0.1 } else { cy + size * 0.35 };
    let kind_label = if has_kind { format!("[{kind}]") } else { String::new() };
    fframes::svgr!(<g opacity={opacity} transform={format!("translate(0 {lift})")}>
        <rect x={cx - w / 2.0} y={cy - h / 2.0} width={w} height={h} rx={rx} fill={fill} stroke={stroke} stroke-width="2" />
        <text x={cx} y={name_y} text-anchor="middle" font-family={COND} font-weight="600" font-size={size} fill={text}>{name}</text>
        <text x={cx} y={cy + size * 0.1 + size * 0.75} text-anchor="middle" font-family={MONO} font-weight="400" font-size={size * 0.6} fill={sub}>{kind_label}</text>
    </g>)
}

/// Section header: "§n  NAME" with a rule drawn across, and a reference on the right.
fn header<'a>(frame: &Frame, number: &'a str, name: &'a str, reference: &'a str) -> Svgr<'a> {
    let rule = (1600.0 * draw(frame, 0.05)).max(0.5);
    fframes::svgr!(<g font-family={MONO} opacity={ramp(frame, 0.0)}>
        <text x="160" y="160" font-size="30" font-weight="600" fill={BLUE}>{number}</text>
        <text x="236" y="160" font-size="26" font-weight="500" letter-spacing="5" fill={INK}>{name}</text>
        <text x="1760" y="160" text-anchor="end" font-size="24" font-weight="500" letter-spacing="3" fill={MUTED}>{reference}</text>
        <rect x="160" y="182" width={rule} height="2" fill={INK} />
    </g>)
}

/// Large condensed headline that rises in at `start`.
fn title<'a>(frame: &Frame, text: &'a str, start: f32) -> Svgr<'a> {
    fframes::svgr!(<g opacity={ramp(frame, start)} transform={format!("translate(0 {})", rise(frame, start))}>
        <text x="156" y="300" font-family={COND} font-weight="600" font-size="104" letter-spacing="-1" fill={INK}>{text}</text>
    </g>)
}

/// A line of DSL with string literals in blue, as `<tspan>`s.
fn dsl_spans(s: &str) -> Vec<Svgr<'_>> {
    let mut spans = Vec::new();
    let mut in_string = false;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if c == '"' {
            if in_string {
                spans.push(fframes::svgr!(<tspan fill={BLUE}>{&s[start..=i]}</tspan>));
                start = i + 1;
            } else {
                if i > start {
                    spans.push(fframes::svgr!(<tspan fill={INK}>{&s[start..i]}</tspan>));
                }
                start = i;
            }
            in_string = !in_string;
        }
    }
    if start < s.len() {
        let fill = if in_string { BLUE } else { INK };
        spans.push(fframes::svgr!(<tspan fill={fill}>{&s[start..]}</tspan>));
    }
    spans
}

// ---------------------------------------------------------------------------------------
// Sheet 1: cover

#[derive(Debug)]
struct Cover;

impl Scene for Cover {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATIONS[0])
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let word = "structurizrx";
        let shown = typed(word, t, 0.25, 22.0);
        // Plex Mono advances 0.6 em per glyph.
        let cursor_x = 160.0 + shown.len() as f32 * 0.6 * 168.0;
        let cursor_on = shown.len() < word.len() || (t * 2.0) as u32 % 2 == 0;
        let cursor_opacity = if cursor_on && t < 2.6 { 1.0 } else { 0.0 };

        let dim = draw(&frame, 1.0);
        let dim_w = 1210.0;
        let half = dim_w / 2.0 * dim;
        let mid = 160.0 + dim_w / 2.0;
        let ticks = ramp(&frame, 1.4);

        let spin = t * 12.0;
        let target = ramp(&frame, 0.4);

        fframes::svgr!(<g opacity={sheet_opacity(&frame, 0)}>
            <g font-family={MONO} opacity={ramp(&frame, 0.0)}>
                <text x="160" y="330" font-size="28" font-weight="500" letter-spacing="5" fill={MUTED}>"DWG SX-000 — SYSTEM SPECIFICATION"</text>
                <rect x="160" y="352" width="120" height="3" fill={BLUE} />
            </g>
            <text x="156" y="540" font-family={MONO} font-weight="600" font-size="168" letter-spacing="0" fill={INK}>{shown}</text>
            <rect x={cursor_x + 6.0} y="420" width="84" height="140" fill={BLUE} opacity={cursor_opacity} />

            // Dimension line under the wordmark.
            <g stroke={BLUE} stroke-width="2">
                <line x1={mid - half} y1="610" x2={mid + half} y2="610" />
                <g opacity={ticks}>
                    <line x1="160" y1="590" x2="160" y2="630" />
                    <line x1={160.0 + dim_w} y1="590" x2={160.0 + dim_w} y2="630" />
                    <path d="M160 610 L178 603 L178 617 Z" fill={BLUE} stroke="none" />
                    <path d={format!("M{e} 610 L{a} 603 L{a} 617 Z", e = 160.0 + dim_w, a = 142.0 + dim_w)} fill={BLUE} stroke="none" />
                </g>
            </g>
            <g opacity={ticks}>
                <rect x={mid - 210.0} y="592" width="420" height="36" fill={PAPER} />
                <text x={mid} y="620" text-anchor="middle" font-family={MONO} font-weight="600" font-size="28" letter-spacing="3" fill={BLUE}>"1 MODEL → N VIEWS"</text>
            </g>

            <g opacity={ramp(&frame, 1.7)} transform={format!("translate(0 {})", rise(&frame, 1.7))}>
                <text x="158" y="790" font-family={COND} font-weight="600" font-size="96" fill={INK}>"Architecture, specified."</text>
            </g>
            <g opacity={ramp(&frame, 2.1)}>
                <text x="160" y="860" font-family={MONO} font-weight="400" font-size="32" fill={MUTED}>"C4 models as code — for people and for LLM agents."</text>
            </g>

            // Registration target, slowly turning.
            <g transform="translate(1600 340)" opacity={target} stroke={INK} fill="none" stroke-width="1.5">
                <circle r="70" />
                <circle r="34" stroke={BLUE} />
                <line x1="-100" y1="0" x2="100" y2="0" />
                <line x1="0" y1="-100" x2="0" y2="100" />
                <g transform={format!("rotate({spin})")} stroke={BLUE} stroke-width="3">
                    <path d="M0 -70 A70 70 0 0 1 70 0" />
                </g>
            </g>
        </g>)
    }
}

// ---------------------------------------------------------------------------------------
// Sheet 2: the problem

#[derive(Debug)]
struct Problem;

/// A loose, out-of-date diagram found somewhere: three boxes and two arrows.
#[allow(clippy::too_many_arguments)]
fn stale_card<'a>(frame: &Frame, x: f32, y: f32, rot: f32, caption: &'a str, labels: [&'a str; 3], start: f32, stamp: &'a str, stamp_at: f32) -> Svgr<'a> {
    let (w, h) = (380.0, 240.0);
    let o = ramp(frame, start);
    let lift = rise(frame, start);
    let s = frame.animate_runtime(AnimateRuntimeInput { on_second: stamp_at, from: 1.6, to: 1.0, animation_runtime: &STAMP });
    let so = smooth((frame.seconds() - stamp_at) / 0.12);
    let sw = stamp.chars().count() as f32 * 0.6 * 26.0 + 36.0;
    let (sx, sy) = (x + w - sw / 2.0 + 14.0, y + h - 4.0);
    fframes::svgr!(<g>
        <g opacity={o} transform={format!("translate(0 {lift}) rotate({rot} {} {})", x + w / 2.0, y + h / 2.0)}>
            <rect x={x} y={y} width={w} height={h} fill={CARD} stroke={INK} stroke-width="1.5" />
            <text x={x + 18.0} y={y + 36.0} font-family={MONO} font-size="20" font-weight="500" fill={MUTED}>{caption}</text>
            {node(x + 90.0, y + 120.0, 120.0, 54.0, labels[0], "", 24.0, false, 1.0, 0.0)}
            {node(x + 290.0, y + 120.0, 120.0, 54.0, labels[1], "", 24.0, false, 1.0, 0.0)}
            {node(x + 190.0, y + 196.0, 120.0, 54.0, labels[2], "", 24.0, false, 1.0, 0.0)}
            {arrow(x + 150.0, y + 120.0, x + 228.0, y + 120.0, 1.0, MUTED)}
            {arrow(x + 270.0, y + 147.0, x + 225.0, y + 168.0, 1.0, MUTED)}
        </g>
        <g opacity={so} transform={format!("translate({sx} {sy}) scale({s}) rotate(-6)")}>
            <rect x={-sw / 2.0} y="-24" width={sw} height="48" fill={PAPER} stroke={RED} stroke-width="3" />
            <text x="0" y="9" text-anchor="middle" font-family={MONO} font-weight="600" font-size="26" letter-spacing="1" fill={RED}>{stamp}</text>
        </g>
    </g>)
}

impl Scene for Problem {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATIONS[1])
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(OVERLAP)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let body = ["Slides, wikis, whiteboards:", "every copy tells a", "different story —", "and none match the code."];
        let lines: Vec<Svgr> = body
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let start = 0.8 + i as f32 * 0.12;
                let fill = if i == 3 { RED } else { INK };
                fframes::svgr!(<g opacity={ramp(&frame, start)} transform={format!("translate(0 {})", rise(&frame, start) * 0.5)}>
                    <text x="160" y={440.0 + i as f32 * 60.0} font-family={MONO} font-size="36" font-weight="400" fill={fill}>{*line}</text>
                </g>)
            })
            .collect();

        fframes::svgr!(<g opacity={sheet_opacity(&frame, 1)}>
            {header(&frame, "§1", "THE PROBLEM", "REQ SX-1.0")}
            {title(&frame, "Diagrams drift.", 0.2)}
            {lines}
            {stale_card(&frame, 940.0, 360.0, -2.0, "FIG.1  wiki/arch.png", ["Web", "API", "DB"], 0.6, "OUTDATED", 2.3)}
            {stale_card(&frame, 1370.0, 400.0, 1.5, "FIG.2  slides_v7.key", ["Web", "Auth", "DB"], 0.75, "NO API?", 2.6)}
            {stale_card(&frame, 1140.0, 640.0, -1.0, "FIG.3  whiteboard.jpg", ["Shop", "Pay", "Store"], 0.9, "CONTRADICTS", 2.9)}
        </g>)
    }
}

// ---------------------------------------------------------------------------------------
// Sheet 3: one model as text

#[derive(Debug)]
struct Model;

const DSL: [&str; 9] = [
    "customer = person \"Customer\"",
    "shop = softwareSystem \"Shop\" {",
    "    web = container \"Web App\"",
    "    api = container \"API\" \"Rust\"",
    "    db  = container \"Database\"",
    "}",
    "customer -> web \"shops on\"",
    "web -> api \"calls\"",
    "api -> db \"reads & writes\"",
];
const LINE_START: f32 = 0.7;
const LINE_STEP: f32 = 0.36;
const TYPE_CPS: f32 = 70.0;

/// When line `i` of the DSL has finished typing.
fn line_done(i: usize) -> f32 {
    LINE_START + i as f32 * LINE_STEP + DSL[i].len() as f32 / TYPE_CPS
}

impl Scene for Model {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATIONS[2])
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(OVERLAP)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let panel = ramp(&frame, 0.35);

        // The line being typed gets a blue marker in the gutter.
        let typing = (0..DSL.len()).rev().find(|&i| t >= LINE_START + i as f32 * LINE_STEP);
        let code: Vec<Svgr> = DSL
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let y = 482.0 + i as f32 * 44.0;
                // Indentation becomes an x offset: SVG collapses leading spaces.
                let indent = line.len() - line.trim_start().len();
                let shown = typed(line.trim_start(), t, LINE_START + i as f32 * LINE_STEP, TYPE_CPS);
                let code_x = 240.0 + indent as f32 * 0.6 * 28.0;
                let visible = if t >= LINE_START + i as f32 * LINE_STEP { 1.0 } else { 0.0 };
                let marker = if typing == Some(i) && t < line_done(DSL.len() - 1) + 0.4 { 1.0 } else { 0.0 };
                fframes::svgr!(<g opacity={visible}>
                    <rect x="161" y={y - 30.0} width="5" height="40" fill={BLUE} opacity={marker} />
                    <text x="214" y={y} text-anchor="end" font-family={MONO} font-size="24" fill={MUTED}>{format!("{}", i + 1)}</text>
                    <text x={code_x} y={y} font-family={MONO} font-size="28" font-weight="400">{dsl_spans(shown)}</text>
                </g>)
            })
            .collect();

        let appear = |i: usize| ramp(&frame, line_done(i));
        let lift = |i: usize| rise(&frame, line_done(i)) * 0.5;
        let boundary = appear(1);

        fframes::svgr!(<g opacity={sheet_opacity(&frame, 2)}>
            {header(&frame, "§2", "THE MODEL", "REQ SX-2.0")}
            {title(&frame, "One model, as text.", 0.2)}

            <g opacity={panel}>
                <rect x="160" y="370" width="800" height="500" fill={CARD} stroke={INK} stroke-width="1.5" />
                <line x1="160" y1="414" x2="960" y2="414" stroke={INK} stroke-width="1" />
                <line x1="226" y1="414" x2="226" y2="870" stroke={GRID_MAJOR} stroke-width="1" />
                <text x="180" y="400" font-family={MONO} font-size="22" font-weight="500" fill={MUTED}>"ws.dsl"</text>
                <text x="940" y="400" text-anchor="end" font-family={MONO} font-size="22" font-weight="500" fill={MUTED}>"SOURCE OF TRUTH"</text>
            </g>
            {code}

            // The diagram assembles from the lines above.
            <g opacity={boundary}>
                <rect x="1060" y="530" width="700" height="370" fill="none" stroke={MUTED} stroke-width="2" stroke-dasharray="10 8" />
                <text x="1080" y="884" font-family={MONO} font-size="22" font-weight="500" fill={MUTED}>"Shop  [software system]"</text>
            </g>
            {node(1240.0, 410.0, 280.0, 96.0, "Customer", "person", 36.0, false, appear(0), lift(0))}
            {node(1240.0, 640.0, 280.0, 96.0, "Web App", "container", 36.0, false, appear(2), lift(2))}
            {node(1600.0, 640.0, 260.0, 96.0, "API", "container: Rust", 36.0, true, appear(3), lift(3))}
            {node(1600.0, 800.0, 260.0, 76.0, "Database", "container", 32.0, false, appear(4), lift(4))}
            {arrow(1240.0, 458.0, 1240.0, 590.0, draw(&frame, line_done(6)), INK)}
            {arrow(1380.0, 640.0, 1468.0, 640.0, draw(&frame, line_done(7)), INK)}
            {arrow(1600.0, 688.0, 1600.0, 760.0, draw(&frame, line_done(8)), INK)}
            <g font-family={MONO} font-size="22" fill={INK}>
                <text x="1256" y="505" opacity={ramp(&frame, line_done(6) + 0.3)}>"shops on"</text>
                <text x="1424" y="624" text-anchor="middle" opacity={ramp(&frame, line_done(7) + 0.3)}>"calls"</text>
                <text x="1584" y="732" text-anchor="end" opacity={ramp(&frame, line_done(8) + 0.3)}>"reads & writes"</text>
            </g>
        </g>)
    }
}

// ---------------------------------------------------------------------------------------
// Sheet 4: views are queries

#[derive(Debug)]
struct Views;

impl Scene for Views {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATIONS[3])
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(OVERLAP)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let (w, h, top) = (480.0, 400.0, 380.0);
        let views = [("V-01", "auto context"), ("V-02", "auto containers"), ("V-03", "auto focus api")];
        let frames: Vec<Svgr> = views
            .iter()
            .enumerate()
            .map(|(i, (id, query))| {
                let x = 160.0 + i as f32 * 560.0;
                let start = 0.6 + i as f32 * 0.25;
                let inner = start + 0.35;
                let a = ramp(&frame, inner);
                let d = draw(&frame, inner + 0.2);
                let cx = x + w / 2.0;
                let body = match i {
                    0 => fframes::svgr!(<g>
                        {node(cx, top + 150.0, 220.0, 80.0, "Customer", "person", 30.0, false, a, 0.0)}
                        {node(cx, top + 310.0, 220.0, 80.0, "Shop", "software system", 30.0, false, a, 0.0)}
                        {arrow(cx, top + 190.0, cx, top + 268.0, d, INK)}
                    </g>),
                    1 => fframes::svgr!(<g>
                        <rect x={x + 24.0} y={top + 90.0} width={w - 48.0} height={h - 120.0} fill="none" stroke={MUTED} stroke-width="1.5" stroke-dasharray="8 6" opacity={a} />
                        {node(x + 100.0, top + 180.0, 116.0, 64.0, "Web", "", 26.0, false, a, 0.0)}
                        {node(cx, top + 300.0, 116.0, 64.0, "API", "", 26.0, false, a, 0.0)}
                        {node(x + w - 100.0, top + 180.0, 116.0, 64.0, "DB", "", 26.0, false, a, 0.0)}
                        {arrow(x + 130.0, top + 212.0, cx - 40.0, top + 268.0, d, INK)}
                        {arrow(cx + 40.0, top + 268.0, x + w - 130.0, top + 212.0, d, INK)}
                    </g>),
                    _ => fframes::svgr!(<g>
                        {node(cx, top + 230.0, 170.0, 84.0, "API", "focus", 32.0, true, a, 0.0)}
                        {node(x + 90.0, top + 130.0, 116.0, 60.0, "Web", "", 24.0, false, a * 0.75, 0.0)}
                        {node(x + w - 90.0, top + 330.0, 116.0, 60.0, "DB", "", 24.0, false, a * 0.75, 0.0)}
                        {arrow(x + 120.0, top + 160.0, cx - 50.0, top + 188.0, d, INK)}
                        {arrow(cx + 50.0, top + 272.0, x + w - 120.0, top + 300.0, d, INK)}
                    </g>),
                };
                fframes::svgr!(<g opacity={ramp(&frame, start)} transform={format!("translate(0 {})", rise(&frame, start))}>
                    <rect x={x} y={top} width={w} height={h} fill={CARD} stroke={INK} stroke-width="1.5" />
                    <line x1={x} y1={top + 52.0} x2={x + w} y2={top + 52.0} stroke={INK} stroke-width="1" />
                    <text x={x + 18.0} y={top + 35.0} font-family={MONO} font-size="24" font-weight="600" fill={BLUE}>{*id}</text>
                    <text x={x + 92.0} y={top + 35.0} font-family={MONO} font-size="24" font-weight="400" fill={INK}>{*query}</text>
                    {body}
                </g>)
            })
            .collect();

        fframes::svgr!(<g opacity={sheet_opacity(&frame, 3)}>
            {header(&frame, "§3", "VIEWS", "REQ SX-3.0")}
            {title(&frame, "Views are queries.", 0.2)}
            {frames}
            <g opacity={ramp(&frame, 1.9)} transform={format!("translate(0 {})", rise(&frame, 1.9) * 0.5)}>
                <text x="160" y="880" font-family={COND} font-weight="600" font-size="52" fill={INK}>
                    "Generated from one model. "
                    <tspan fill={BLUE}>"They can’t disagree."</tspan>
                </text>
            </g>
        </g>)
    }
}

// ---------------------------------------------------------------------------------------
// Sheet 5: verification

#[derive(Debug)]
struct Checked;

impl Scene for Checked {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATIONS[4])
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(OVERLAP)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let cmd = "$ structurizrx validate ws.dsl";
        let show = |at: f32| if t >= at { 1.0 } else { 0.0 };
        let cmd1 = typed(cmd, t, 0.5, 45.0);
        let cmd2 = typed(cmd, t, 2.0, 45.0);

        let clauses = [("4.1", "validate --strict"), ("4.2", "lint · review"), ("4.3", "diff git revisions"), ("4.4", "MCP server for agents")];
        let list: Vec<Svgr> = clauses
            .iter()
            .enumerate()
            .map(|(i, (n, text))| {
                let start = 0.7 + i as f32 * 0.12;
                let y = 420.0 + i as f32 * 66.0;
                fframes::svgr!(<g opacity={ramp(&frame, start)} transform={format!("translate(0 {})", rise(&frame, start) * 0.5)} font-family={MONO} font-size="32">
                    <text x="1240" y={y} font-weight="600" fill={BLUE}>{*n}</text>
                    <text x="1330" y={y} font-weight="400" fill={INK}>{*text}</text>
                    <line x1="1240" y1={y + 22.0} x2="1760" y2={y + 22.0} stroke={GRID_MAJOR} stroke-width="1.5" />
                </g>)
            })
            .collect();

        fframes::svgr!(<g opacity={sheet_opacity(&frame, 4)}>
            {header(&frame, "§4", "VERIFICATION", "REQ SX-4.0")}
            {title(&frame, "Checked like code.", 0.2)}

            <g opacity={ramp(&frame, 0.3)}>
                <rect x="160" y="370" width="980" height="340" rx="6" fill={INK} />
                <circle cx="190" cy="396" r="7" fill="#3a3f45" />
                <circle cx="214" cy="396" r="7" fill="#3a3f45" />
                <circle cx="238" cy="396" r="7" fill="#3a3f45" />
            </g>
            <g font-family={MONO} font-size="30" font-weight="400">
                <text x="190" y="460" fill={CARD}>{cmd1}</text>
                <g opacity={show(1.25)}>
                    <text x="190" y="508" fill={TERM_RED}>"error 9:12  unknown element 'dbb'"</text>
                    <text x={190.0 + 12.0 * 0.6 * 30.0} y="552" fill={TERM_BLUE}>"did you mean 'db'?"</text>
                </g>
                <text x="190" y="616" fill={CARD} opacity={show(2.0)}>{cmd2}</text>
                <text x="190" y="664" fill={TERM_BLUE} font-weight="600" opacity={show(2.85)}>"OK  5 elements · 3 views · 0 findings"</text>
            </g>

            {list}

            <g opacity={ramp(&frame, 3.0)} transform={format!("translate(0 {})", rise(&frame, 3.0) * 0.5)}>
                <text x="160" y="810" font-family={COND} font-weight="600" font-size="52" fill={INK}>"Errors precise enough for an LLM agent"</text>
                <text x="160" y="872" font-family={COND} font-weight="600" font-size="52" fill={BLUE}>"to fix its own mistakes."</text>
            </g>
        </g>)
    }
}

// ---------------------------------------------------------------------------------------
// Sheet 6: release

#[derive(Debug)]
struct Release;

impl Scene for Release {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DURATIONS[5])
    }

    fn overlap(&self) -> Overlap {
        Overlap::Previous(OVERLAP)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let stamp_at = 1.5;
        let s = frame.animate_runtime(AnimateRuntimeInput { on_second: stamp_at, from: 1.5, to: 1.0, animation_runtime: &STAMP });
        let so = smooth((frame.seconds() - stamp_at) / 0.12);

        fframes::svgr!(<g opacity={sheet_opacity(&frame, 5)}>
            {header(&frame, "§5", "RELEASE", "REQ SX-5.0")}
            <g opacity={ramp(&frame, 0.2)} transform={format!("translate(0 {})", rise(&frame, 0.2))}>
                <text x="156" y="420" font-family={MONO} font-weight="600" font-size="140" fill={INK}>"structurizrx"</text>
            </g>
            <g opacity={ramp(&frame, 0.45)} transform={format!("translate(0 {})", rise(&frame, 0.45))}>
                <text x="158" y="536" font-family={COND} font-weight="600" font-size="72" fill={INK}>"One model. Every view. Verified."</text>
            </g>
            <g opacity={ramp(&frame, 0.75)} font-family={MONO}>
                <rect x="160" y="596" width="740" height="80" fill={CARD} stroke={INK} stroke-width="1.5" />
                <text x="190" y="648" font-size="32" font-weight="500" fill={INK}>
                    <tspan fill={BLUE}>"$ "</tspan>
                    "structurizrx serve ws.dsl --open"
                </text>
            </g>
            <g opacity={ramp(&frame, 0.95)}>
                <text x="160" y="760" font-family={MONO} font-size="28" font-weight="500" letter-spacing="4" fill={MUTED}>"SVG · PNG · MERMAID · PLANTUML · DOT · JSON"</text>
            </g>

            <g opacity={so} transform={format!("translate(1480 560) scale({s}) rotate(-8)")}>
                <rect x="-200" y="-90" width="400" height="180" fill="none" stroke={RED} stroke-width="5" />
                <rect x="-186" y="-76" width="372" height="152" fill="none" stroke={RED} stroke-width="2" />
                <text x="0" y="14" text-anchor="middle" font-family={COND} font-weight="600" font-size="64" letter-spacing="3" fill={RED}>"APPROVED"</text>
                <text x="0" y="56" text-anchor="middle" font-family={MONO} font-weight="600" font-size="22" letter-spacing="4" fill={RED}>"REV A · SX-000"</text>
            </g>
        </g>)
    }
}
