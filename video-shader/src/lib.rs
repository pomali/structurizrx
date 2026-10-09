//! structurizrx in the style of fframes' `shader-mode` example: every shot is a GPU
//! shader under SVG typography, cut hard on the frame.

mod edit;
mod overlays;
mod transitions;

use edit::{SHOTS, ShotSpec};
use fframes::{
    AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Shader, ShaderUniforms, Svgr,
    Video, include_media_dir,
};

include_media_dir!(pub struct IntroShaderMedia, "media");

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
pub const FPS: usize = 30;

const COMMON: &str = include_str!("shaders/common.sksl");
const SOURCE_EFFECTS: &str = include_str!("shaders/source-effects.sksl");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    Binary,
    Glass,
    Aurora,
    LiquidChrome,
    Gradient,
    Flow,
    Rays,
    Obsidian,
    Particles,
    Spectral,
    Emboss,
    Eclipse,
}

impl Effect {
    const ALL: [Self; 12] = [
        Self::Binary,
        Self::Glass,
        Self::Aurora,
        Self::LiquidChrome,
        Self::Gradient,
        Self::Flow,
        Self::Rays,
        Self::Obsidian,
        Self::Particles,
        Self::Spectral,
        Self::Emboss,
        Self::Eclipse,
    ];

    fn source(self) -> &'static str {
        match self {
            Self::Binary => include_str!("shaders/binary-intro.sksl"),
            Self::Glass => include_str!("shaders/glass.sksl"),
            Self::Aurora => include_str!("shaders/aurora.sksl"),
            Self::LiquidChrome => include_str!("shaders/liquid-chrome.sksl"),
            Self::Gradient => "half4 main(float2 coord) { return finish(gradientColor(coord,iTime),coord); }",
            Self::Flow => include_str!("shaders/flow.sksl"),
            Self::Rays => include_str!("shaders/rays.sksl"),
            Self::Obsidian => include_str!("shaders/obsidian.sksl"),
            Self::Particles => include_str!("shaders/particles.sksl"),
            Self::Spectral => include_str!("shaders/spectral.sksl"),
            Self::Emboss => include_str!("shaders/emboss.sksl"),
            Self::Eclipse => include_str!("shaders/eclipse.sksl"),
        }
    }

    fn mask(self) -> Option<&'static str> {
        match self {
            Self::Binary => Some("binary-atlas.png"),
            Self::Emboss => Some("wordmark-sdf.png"),
            _ => None,
        }
    }

    /// The shader's own clock is `uClock`, so cuts can keep or restart it.
    fn shader(self) -> Shader {
        let sculpture = if matches!(self, Self::LiquidChrome | Self::Obsidian) {
            include_str!("shaders/sculpture.sksl")
        } else {
            ""
        };
        let gradient = if self == Self::Gradient { include_str!("shaders/gradient-shared.sksl") } else { "" };
        Shader::sksl(format!("{COMMON}\n{SOURCE_EFFECTS}\n{sculpture}\n{gradient}\n{}", self.source()).replace("iTime", "uClock"))
    }
}

#[derive(Debug)]
struct Shot {
    spec: ShotSpec,
    shader: Option<Shader>,
}

impl Scene for Shot {
    fn name(&self) -> &'static str {
        self.spec.name
    }

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(self.spec.frames())
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let index = frame.global_index;
        let overlay = overlays::render(self.spec.name, &mut frame, ctx);
        let Some(shader) = &self.shader else {
            return transitions::apply(index, self.spec.name, overlay);
        };
        let clock = (index - self.spec.clock_start) as f32 / FPS as f32;
        let mut uniforms = ShaderUniforms::new()
            .float("uClock", clock)
            .float("uProgress", frame.index as f32 / self.spec.frames() as f32)
            .float("uVariant", f32::from(self.spec.variant))
            .float("uBeat", (-(clock % 0.5) * 12.0).exp());
        if let Some(name) = self.spec.effect.and_then(Effect::mask) {
            let Some(mask) = ctx.get_image(name) else {
                return Svgr::empty();
            };
            uniforms = uniforms.image("uMask", mask);
        }
        let layer = shader.draw(&frame, uniforms);
        let composition = fframes::svgr!(
            <g>
                <image href={layer.href()} x="0" y="0" width="1920" height="1080" />
                {overlay}
            </g>
        );
        transitions::apply(index, self.spec.name, composition)
    }
}

pub struct IntroShaderVideo<'a> {
    pub media: &'a IntroShaderMedia,
    shots: Vec<Shot>,
}

impl<'a> IntroShaderVideo<'a> {
    /// Shaders are defined once; Skia compiles and caches them on first use.
    pub fn new(media: &'a IntroShaderMedia) -> Self {
        let shaders = Effect::ALL.map(Effect::shader);
        let shots = SHOTS
            .iter()
            .copied()
            .map(|spec| Shot { shader: spec.effect.map(|effect| shaders[effect as usize].clone()), spec })
            .collect();
        Self { media, shots }
    }
}

impl std::fmt::Debug for IntroShaderVideo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntroShaderVideo").finish()
    }
}

impl Video for IntroShaderVideo<'_> {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(edit::FRAMES)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(self.shots.iter().map(|shot| shot as &dyn Scene).collect::<Vec<_>>())
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT} viewBox="0 0 1920 1080">
                <rect width="1920" height="1080" fill="#000" />
                {ctx.render_scenes(&frame)}
            </svg>
        )
    }
}
