//! SVG typography and panels drawn over each shot's shader.

use std::sync::OnceLock;

use fframes::{FFramesContext, FontQuery, FontStyle, Frame, Svgr};

const SANS: &str = "DM Sans";
const MONO: &str = "JetBrains Mono";
const SERIF: &str = "Instrument Serif";

const WHITE: &str = "#f6f4ff";
const SOFT: &str = "#c9c4df";
const LAVENDER: &str = "#bea4f1";
const TEAL: &str = "#afd8d7";
const CORAL: &str = "#ff7c51";
const DARK: &str = "#171922";
const PANEL: &str = "#08080d";
const PANEL_LINE: &str = "#45414f";

pub(crate) fn render(name: &str, frame: &mut Frame, ctx: &FFramesContext<'_, '_>) -> Svgr<'static> {
    let t = frame.seconds();
    match name {
        "Cover" => chroma("STRUCTURIZRX", 638, 68),
        "CoverAmber" => chroma("ARCHITECTURE, SPECIFIED.", 638, 68),
        "Fig1" => figure("FIG.1", "wiki/arch.png", "OUTDATED"),
        "Fig2" => figure("FIG.2", "slides_v7.key", "CONTRADICTS FIG.1"),
        "Fig3" => figure("FIG.3", "whiteboard.jpg", "MISSING API"),
        "Fig4" => figure("FIG.4", "final_FINAL.png", "WHICH ONE?"),
        "Drift" => drift(t),
        "ModelTitle" => center("ONE MODEL, AS TEXT.", 576, 64, "#2a2338"),
        "Model" => model(t),
        "ViewsTitle" => chroma("VIEWS ARE QUERIES.", 560, 72),
        "View1" => view("V-01", "auto context", view_context()),
        "View2" => view("V-02", "auto containers", view_containers()),
        "View3" => view("V-03", "auto focus api", view_focus()),
        "Agree" => fframes::svgr!(
            <g>
                <text x="960" y="381" text-anchor="middle" font-family={MONO} font-size="27" letter-spacing="4" fill={SOFT}>"ONE MODEL → EVERY VIEW"</text>
                <text x="960" y="554" text-anchor="middle" font-family={SANS} font-weight="500" font-size="132" letter-spacing="-5" fill={WHITE}>"they can’t disagree"</text>
                <text x="960" y="637" text-anchor="middle" font-family={SANS} font-size="32" letter-spacing="0.5" fill={SOFT}>"Views are generated, never redrawn."</text>
            </g>
        ),
        "Checked" => checked(t),
        "Outro" => outro(t, frame, ctx),
        _ => Svgr::empty(),
    }
}

// ---------------------------------------------------------------------------------------
// Building blocks

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// 0 to 1 over `duration` seconds from `start`.
fn fade(t: f32, start: f32, duration: f32) -> f32 {
    smooth((t - start) / duration)
}

fn center(text: &'static str, y: usize, size: usize, color: &'static str) -> Svgr<'static> {
    fframes::svgr!(
        <text x="960" y={y} text-anchor="middle" font-family={SANS} font-weight="500"
              font-size={size} letter-spacing="-1.8" fill={color}>{text}</text>
    )
}

/// White title with a red and a blue copy offset by a pixel, like a lens fringe.
fn chroma(text: &'static str, y: usize, size: usize) -> Svgr<'static> {
    fframes::svgr!(
        <g>
            <g transform="translate(1.2 1.4)">{center(text, y, size, "#ff7c51")}</g>
            <g transform="translate(-1 -1)">{center(text, y, size, "#6989ff")}</g>
            {center(text, y, size, "#f8f8f8")}
        </g>
    )
}

/// Corner labels: what this is, top left; where we are, top right.
fn hud(left: &'static str, right: &'static str) -> Svgr<'static> {
    fframes::svgr!(
        <g font-family={MONO} font-size="23">
            <text x="76" y="82" fill="#eeecf8">{left}</text>
            <text x="1844" y="82" text-anchor="end" fill="#c1becd">{right}</text>
        </g>
    )
}

/// The first `n` characters of `s`, where `n` grows at `cps` characters per second.
fn typed(s: &'static str, t: f32, start: f32, cps: f32) -> &'static str {
    let n = ((t - start) * cps).max(0.0) as usize;
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

fn arrow(x1: f32, y1: f32, x2: f32, y2: f32, p: f32) -> Svgr<'static> {
    if p <= 0.001 {
        return Svgr::empty();
    }
    let (ex, ey) = (x1 + (x2 - x1) * p, y1 + (y2 - y1) * p);
    let angle = (y2 - y1).atan2(x2 - x1).to_degrees();
    let head = ((p - 0.8) / 0.2).clamp(0.0, 1.0);
    fframes::svgr!(<g>
        <line x1={x1} y1={y1} x2={ex} y2={ey} stroke={SOFT} stroke-width="2.5" />
        <g transform={format!("translate({x2} {y2}) rotate({angle})")} opacity={head}>
            <path d="M0 0 L-18 -8 L-18 8 Z" fill={SOFT} />
        </g>
    </g>)
}

/// An element box centred on (cx, cy): frosted dark, or lavender when in focus.
#[allow(clippy::too_many_arguments)]
fn node(cx: f32, cy: f32, w: f32, h: f32, name: &'static str, kind: &'static str, size: f32, focus: bool, opacity: f32) -> Svgr<'static> {
    let (fill, fill_opacity, stroke, text, sub) =
        if focus { (LAVENDER, 1.0, LAVENDER, DARK, "#3b2f57") } else { (PANEL, 0.72, "#d9d4ea", WHITE, SOFT) };
    let rx = if kind == "person" { h / 2.0 } else { 10.0 };
    let name_y = if kind.is_empty() { cy + size * 0.35 } else { cy + size * 0.1 };
    let kind_label = if kind.is_empty() { String::new() } else { format!("[{kind}]") };
    fframes::svgr!(<g opacity={opacity}>
        <rect x={cx - w / 2.0} y={cy - h / 2.0} width={w} height={h} rx={rx} fill={fill} fill-opacity={fill_opacity} stroke={stroke} stroke-width="2" />
        <text x={cx} y={name_y} text-anchor="middle" font-family={SANS} font-weight="500" font-size={size} letter-spacing="-0.5" fill={text}>{name}</text>
        <text x={cx} y={cy + size * 0.85} text-anchor="middle" font-family={MONO} font-size={size * 0.5} fill={sub}>{kind_label}</text>
    </g>)
}

// ---------------------------------------------------------------------------------------
// The problem

fn figure(id: &'static str, file: &'static str, verdict: &'static str) -> Svgr<'static> {
    let stamp_w = verdict.chars().count() as f32 * 0.6 * 34.0 + 56.0;
    fframes::svgr!(
        <g>
            {hud("structurizrx / the problem", id)}
            <text x="960" y="250" text-anchor="middle" font-family={SANS} font-weight="500" font-size="110" letter-spacing="-4" fill={WHITE}>{file}</text>
            <g transform="translate(960 880) rotate(-4)">
                <rect x={-stamp_w / 2.0} y="-42" width={stamp_w} height="84" fill={PANEL} fill-opacity="0.6" stroke={CORAL} stroke-width="4" />
                <text x="0" y="12" text-anchor="middle" font-family={MONO} font-size="34" letter-spacing="4" fill={CORAL}>{verdict}</text>
            </g>
        </g>
    )
}

fn drift(t: f32) -> Svgr<'static> {
    fframes::svgr!(
        <g>
            <text x="152" y="278" font-family={MONO} font-size="26" letter-spacing="3" fill="#565967">"THE PROBLEM"</text>
            <text x="142" y="478" font-family={SANS} font-weight="500" font-size="174" letter-spacing="-7" fill={DARK}>"diagrams"</text>
            <text x="142" y="645" font-family={SANS} font-weight="500" font-size="174" letter-spacing="-7" fill={DARK}>"drift."</text>
            <path d="M154 722H222" stroke="#79758b" stroke-width="2" />
            <text x="152" y="785" font-family={SANS} font-size="32" fill="#565967" opacity={fade(t, 0.5, 0.4)}>"None of them match the code."</text>
        </g>
    )
}

// ---------------------------------------------------------------------------------------
// The model

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
const LINE_START: f32 = 0.5;
const LINE_STEP: f32 = 0.42;
const TYPE_CPS: f32 = 60.0;

fn line_start(i: usize) -> f32 {
    LINE_START + i as f32 * LINE_STEP
}

fn line_done(i: usize) -> f32 {
    line_start(i) + DSL[i].trim_start().len() as f32 / TYPE_CPS
}

/// A line of DSL with keywords in lavender and string literals in teal.
fn dsl_spans(s: &'static str) -> Vec<Svgr<'static>> {
    let mut spans = Vec::new();
    let mut in_string = false;
    let mut start = 0;
    let mut push = |text: &'static str, string: bool| {
        if text.is_empty() {
            return;
        }
        if string {
            spans.push(fframes::svgr!(<tspan fill={TEAL}>{text}</tspan>));
            return;
        }
        // Split the plain part on spaces so element keywords can be coloured.
        let mut rest = text;
        while !rest.is_empty() {
            let end = rest.find(' ').map_or(rest.len(), |i| i + 1);
            let (word, tail) = rest.split_at(end);
            let fill = if matches!(word.trim(), "person" | "softwareSystem" | "container") { LAVENDER } else { "#e5e1ed" };
            spans.push(fframes::svgr!(<tspan fill={fill}>{word}</tspan>));
            rest = tail;
        }
    };
    for (i, c) in s.char_indices() {
        if c == '"' {
            if in_string {
                push(&s[start..=i], true);
                start = i + 1;
            } else {
                push(&s[start..i], false);
                start = i;
            }
            in_string = !in_string;
        }
    }
    push(&s[start..], in_string);
    spans
}

fn model(t: f32) -> Svgr<'static> {
    let code: Vec<Svgr> = DSL
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let y = 410.0 + i as f32 * 64.0;
            let indent = line.len() - line.trim_start().len();
            let shown = typed(line.trim_start(), t, line_start(i), TYPE_CPS);
            let visible = if t >= line_start(i) { 1.0 } else { 0.0 };
            fframes::svgr!(<g opacity={visible}>
                <text x="122" y={y} text-anchor="end" font-family={MONO} font-size="24" fill="#5d5868">{format!("{}", i + 1)}</text>
                <text x={150.0 + indent as f32 * 0.6 * 30.0} y={y} font-family={MONO} font-size="30">{dsl_spans(shown)}</text>
            </g>)
        })
        .collect();
    let appear = |i: usize| fade(t, line_done(i), 0.35);
    let draw = |i: usize| fade(t, line_done(i), 0.5);

    fframes::svgr!(
        <g>
            {hud("structurizrx / the model", "ws.dsl → views")}
            <text x="76" y="210" font-family={SANS} font-weight="500" font-size="96" letter-spacing="-3" fill={WHITE}>"One model, as text."</text>
            <rect x="64" y="280" width="1792" height="736" rx="10" fill={PANEL} fill-opacity="0.88" stroke={PANEL_LINE} />
            <path d="M64 340H1856 M960 340V1016" stroke="#39353e" />
            <text x="95" y="320" font-family={MONO} font-size="20" fill="#cfc9da">"ws.dsl"</text>
            <text x="991" y="320" font-family={MONO} font-size="20" fill="#cfc9da">"view: containers"</text>
            {code}

            <g opacity={appear(1)}>
                <rect x="1000" y="530" width="816" height="456" rx="12" fill="none" stroke="#5d5868" stroke-width="2" stroke-dasharray="10 8" />
                <text x="1024" y="966" font-family={MONO} font-size="20" fill="#8d88a0">"Shop [software system]"</text>
            </g>
            {node(1190.0, 430.0, 300.0, 100.0, "Customer", "person", 36.0, false, appear(0))}
            {node(1190.0, 660.0, 300.0, 100.0, "Web App", "container", 36.0, false, appear(2))}
            {node(1620.0, 660.0, 280.0, 100.0, "API", "container: Rust", 36.0, true, appear(3))}
            {node(1620.0, 870.0, 280.0, 90.0, "Database", "container", 34.0, false, appear(4))}
            {arrow(1190.0, 480.0, 1190.0, 606.0, draw(6))}
            {arrow(1340.0, 660.0, 1476.0, 660.0, draw(7))}
            {arrow(1620.0, 710.0, 1620.0, 821.0, draw(8))}
            <g font-family={MONO} font-size="22" fill={SOFT}>
                <text x="1206" y="512" opacity={fade(t, line_done(6) + 0.3, 0.3)}>"shops on"</text>
                <text x="1408" y="642" text-anchor="middle" opacity={fade(t, line_done(7) + 0.3, 0.3)}>"calls"</text>
                <text x="1604" y="772" text-anchor="end" opacity={fade(t, line_done(8) + 0.3, 0.3)}>"reads & writes"</text>
            </g>
        </g>
    )
}

// ---------------------------------------------------------------------------------------
// Views

fn view(id: &'static str, query: &'static str, diagram: Svgr<'static>) -> Svgr<'static> {
    fframes::svgr!(
        <g>
            <g font-family={MONO} font-size="30">
                <text x="76" y="96" fill={LAVENDER}>{id}</text>
                <text x="176" y="96" fill={WHITE}>{query}</text>
            </g>
            <text x="1844" y="96" text-anchor="end" font-family={MONO} font-size="23" fill="#c1becd">"generated"</text>
            {diagram}
        </g>
    )
}

fn view_context() -> Svgr<'static> {
    fframes::svgr!(<g>
        {node(960.0, 370.0, 420.0, 130.0, "Customer", "person", 52.0, false, 1.0)}
        {node(960.0, 760.0, 420.0, 130.0, "Shop", "software system", 52.0, false, 1.0)}
        {arrow(960.0, 435.0, 960.0, 692.0, 1.0)}
    </g>)
}

fn view_containers() -> Svgr<'static> {
    fframes::svgr!(<g>
        <rect x="360" y="270" width="1200" height="620" rx="16" fill="none" stroke={SOFT} stroke-width="2" stroke-dasharray="12 10" />
        <text x="390" y="866" font-family={MONO} font-size="24" fill={SOFT}>"Shop [software system]"</text>
        {node(620.0, 440.0, 300.0, 110.0, "Web App", "container", 44.0, false, 1.0)}
        {node(1300.0, 440.0, 300.0, 110.0, "Database", "container", 44.0, false, 1.0)}
        {node(960.0, 700.0, 300.0, 110.0, "API", "container", 44.0, false, 1.0)}
        {arrow(700.0, 495.0, 870.0, 643.0, 1.0)}
        {arrow(1050.0, 645.0, 1220.0, 497.0, 1.0)}
    </g>)
}

fn view_focus() -> Svgr<'static> {
    fframes::svgr!(<g>
        {node(960.0, 560.0, 360.0, 140.0, "API", "focus", 60.0, true, 1.0)}
        {node(500.0, 330.0, 280.0, 100.0, "Web App", "", 40.0, false, 0.8)}
        {node(1420.0, 790.0, 280.0, 100.0, "Database", "", 40.0, false, 0.8)}
        {arrow(600.0, 380.0, 800.0, 490.0, 1.0)}
        {arrow(1120.0, 630.0, 1320.0, 740.0, 1.0)}
    </g>)
}

// ---------------------------------------------------------------------------------------
// Verification

fn checked(t: f32) -> Svgr<'static> {
    let cmd = "$ structurizrx validate ws.dsl";
    let show = |at: f32| if t >= at { 1.0 } else { 0.0 };
    fframes::svgr!(
        <g>
            // A soft dark pool under the text; the satin stays visible at the edges.
            <defs>
                <radialGradient id="checked-scrim" cx="960" cy="600" r="980" gradientUnits="userSpaceOnUse">
                    <stop offset="0" stop-color="#000" stop-opacity="0.9" />
                    <stop offset="0.55" stop-color="#000" stop-opacity="0.75" />
                    <stop offset="1" stop-color="#000" stop-opacity="0" />
                </radialGradient>
            </defs>
            <rect width="1920" height="1080" fill="url(#checked-scrim)" />
            {hud("structurizrx / verification", "validate · lint · diff · mcp")}
            <text x="960" y="230" text-anchor="middle" font-family={SANS} font-weight="500" font-size="104" letter-spacing="-4" fill={WHITE}>"Checked like code."</text>
            <rect x="380" y="320" width="1160" height="400" rx="10" fill={PANEL} fill-opacity="0.9" stroke={PANEL_LINE} />
            <path d="M380 370H1540" stroke="#39353e" />
            <text x="411" y="353" font-family={MONO} font-size="20" fill="#cfc9da">"terminal"</text>
            <g font-family={MONO} font-size="32">
                <text x="420" y="440" fill="#e5e1ed">{typed(cmd, t, 0.4, 45.0)}</text>
                <g opacity={show(1.2)}>
                    <text x="420" y="492" fill={CORAL}>"error 9:12  unknown element 'dbb'"</text>
                    <text x={420.0 + 12.0 * 0.6 * 32.0} y="540" fill={TEAL}>"did you mean 'db'?"</text>
                </g>
                <text x="420" y="616" fill="#e5e1ed" opacity={show(1.9)}>{typed(cmd, t, 1.9, 45.0)}</text>
                <text x="420" y="668" fill={LAVENDER} opacity={show(2.7)}>"OK  5 elements · 3 views · 0 findings"</text>
            </g>
            <text x="960" y="860" text-anchor="middle" font-family={SERIF} font-style="italic" font-size="64" fill="#ece8e1" opacity={fade(t, 1.3, 0.5)}>"Errors precise enough for an LLM agent"</text>
            <text x="960" y="936" text-anchor="middle" font-family={SERIF} font-style="italic" font-size="64" fill={LAVENDER} opacity={fade(t, 1.5, 0.5)}>"to fix its own mistakes."</text>
        </g>
    )
}

// ---------------------------------------------------------------------------------------
// Outro: the wordmark with echoes of its first letter, as in shader-mode's outro.

const WORD: &str = "structurizrx";
const WORD_SIZE: usize = 220;
static WIDTHS: OnceLock<(f32, f32)> = OnceLock::new();

fn measure(frame: &mut Frame, ctx: &FFramesContext<'_, '_>) -> (f32, f32) {
    if let Some(widths) = WIDTHS.get() {
        return *widths;
    }
    let font = FontQuery { family: SERIF, size: WORD_SIZE, weight: 400, style: FontStyle::Italic, ..Default::default() };
    match (frame.text_width(ctx, font, WORD), frame.text_width(ctx, font, "s")) {
        (Some(word), Some(letter)) => *WIDTHS.get_or_init(|| (word as f32, letter as f32)),
        _ => (1100.0, 80.0),
    }
}

fn spring(seconds: f32, stiffness: f32, damping: f32) -> f32 {
    if seconds <= 0.0 {
        return 0.0;
    }
    let natural = stiffness.sqrt();
    let ratio = damping / (2.0 * natural);
    let damped = natural * (1.0 - ratio * ratio).sqrt();
    1.0 - (-ratio * natural * seconds).exp() * ((damped * seconds).cos() + ratio * natural / damped * (damped * seconds).sin())
}

fn expo_out(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    if progress >= 1.0 { 1.0 } else { 1.0 - 2.0_f32.powf(-10.0 * progress) }
}

fn outro(t: f32, frame: &mut Frame, ctx: &FFramesContext<'_, '_>) -> Svgr<'static> {
    const BEAT: f32 = 0.25;
    let beats = t / BEAT;
    let (width, letter) = measure(frame, ctx);
    let step = letter * 0.9;
    let total = width + step * 3.0;
    let x = 960.0 - total / 2.0 + step * 3.0;
    let y = 560.0;
    let word_opacity = spring(t, 260.0, 20.0).min(1.0);
    let word_scale = 1.0 + (1.0 - expo_out(beats / 0.5)) * 0.25;
    let echoes: Vec<Svgr> = ["#bea4f1", "#8f78c9", "#5d4b8f"]
        .into_iter()
        .enumerate()
        .rev()
        .map(|(index, color)| {
            let index = index as f32;
            let amount = spring((beats - 0.5 - index * 0.5) * BEAT, 170.0, 15.0);
            let dx = -step * (index + 1.0) * amount;
            let opacity = amount.clamp(0.0, 1.0) * (1.0 - index * 0.18);
            fframes::svgr!(
                <text x={x + dx} y={y} font-family={SERIF} font-style="italic" font-weight="400"
                      font-size={WORD_SIZE} fill={color} opacity={opacity}>"s"</text>
            )
        })
        .collect();
    let tag = fade(t, 0.9, 0.35);
    let command = fade(t, 1.2, 0.35);
    fframes::svgr!(
        <g>
            {echoes}
            <g transform={format!("translate(960 {y}) scale({word_scale}) translate(-960 -{y})")} opacity={word_opacity}>
                <text x={x} y={y} font-family={SERIF} font-style="italic" font-weight="400" font-size={WORD_SIZE} fill="#ece8e1">{WORD}</text>
            </g>
            <text x="960" y="666" text-anchor="middle" font-family={MONO} font-size="30" letter-spacing="3" fill="#c6b4ef" opacity={tag}>"ONE MODEL · EVERY VIEW · VERIFIED"</text>
            <text x="960" y="730" text-anchor="middle" font-family={MONO} font-size="24" fill="#8d88a0" opacity={command}>"$ structurizrx serve ws.dsl --open"</text>
        </g>
    )
}
