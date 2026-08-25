//! In-app performance benchmark: drives the *live* renderer through a few
//! progressively heavier preset scenes and measures real per-frame time on
//! the user's own GPU. This is deliberately separate from the dev-only
//! Criterion benches in `pipes-core`/`pipes-render` (`cargo bench`, see
//! `docs/DEVELOPMENT.md`) — those never touch a `Renderer` at all, so they
//! can't answer "what settings can *my* machine actually run smoothly?"
//! the way this can. `main.rs` owns actually driving a `Run` forward one
//! real rendered frame at a time; this module is pure state plus the
//! text/PDF report writers, so both are usable/testable without a window.

use pipes_core::{GridBounds, SimConfig};

/// ~3s at 60fps, ~6s at 30fps — long enough to average out one-off hitches
/// (a scene reset, a dissolve transition) without making the user wait
/// too long per stage.
pub const FRAMES_PER_STAGE: u32 = 180;

pub struct Stage {
    pub name: &'static str,
    pub config: SimConfig,
}

/// Light matches the shipped defaults (so a user can compare "what I
/// actually run" against heavier scenes); Medium/Heavy stress the same
/// grid-size/pipe-count knobs exposed in the settings drawer's "Grid size
/// & reset" / "Pipe style & count" panels, at scales chosen to bracket the
/// point (found via `cargo bench -p pipes-render`, see
/// `docs/DEVELOPMENT.md`) where `build_instances` alone starts costing a
/// meaningful fraction of a 60fps frame budget.
pub fn stages() -> Vec<Stage> {
    vec![
        Stage {
            name: "Light (defaults)",
            config: SimConfig::default(),
        },
        Stage {
            name: "Medium",
            config: SimConfig {
                bounds: GridBounds::new(40, 28, 40),
                max_pipes: 30,
                ..SimConfig::default()
            },
        },
        Stage {
            name: "Heavy",
            config: SimConfig {
                bounds: GridBounds::new(64, 64, 64),
                max_pipes: 150,
                ..SimConfig::default()
            },
        },
    ]
}

#[derive(Debug, Clone)]
pub struct StageResult {
    pub name: String,
    pub bounds: (i32, i32, i32),
    pub max_pipes: usize,
    pub frame_count: usize,
    pub avg_ms: f32,
    pub min_ms: f32,
    pub max_ms: f32,
    pub avg_fps: f32,
}

impl StageResult {
    fn from_samples(
        name: &str,
        bounds: (i32, i32, i32),
        max_pipes: usize,
        samples: &[f32],
    ) -> Self {
        let sum: f32 = samples.iter().sum();
        let avg_ms = sum / samples.len() as f32;
        let min_ms = samples.iter().copied().fold(f32::INFINITY, f32::min);
        let max_ms = samples.iter().copied().fold(0.0f32, f32::max);
        Self {
            name: name.to_string(),
            bounds,
            max_pipes,
            frame_count: samples.len(),
            avg_ms,
            min_ms,
            max_ms,
            avg_fps: if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Report {
    pub app_version: String,
    pub os: String,
    pub gpu_name: String,
    pub generated_at: String,
    pub stages: Vec<StageResult>,
}

/// State machine for one benchmark pass. Owns no GPU/window handles at
/// all — `main.rs` reads `current_config()` to know which `Scene`/config
/// should be active right now, and calls `record_frame` once per real
/// render with that frame's wall-clock cost.
pub struct Run {
    stages: Vec<Stage>,
    stage_index: usize,
    samples: Vec<f32>,
    results: Vec<StageResult>,
    /// Discarded (not recorded, not counted toward `FRAMES_PER_STAGE`)
    /// before the very first sample: the first frame or two rendered right
    /// after a `Renderer` is created pays for one-off GPU pipeline/shader
    /// warm-up that has nothing to do with steady-state scene cost — found
    /// by actually running a benchmark and seeing an ~77ms outlier
    /// (vs. ~2-7ms otherwise) skew the *first* stage's average alone, since
    /// that's the only stage transition that follows fresh renderer
    /// creation rather than just a config/`Scene` swap.
    warmup_remaining: u32,
}

const WARMUP_FRAMES: u32 = 15;

impl Run {
    pub fn new() -> Self {
        Self {
            stages: stages(),
            stage_index: 0,
            samples: Vec::with_capacity(FRAMES_PER_STAGE as usize),
            results: Vec::new(),
            warmup_remaining: WARMUP_FRAMES,
        }
    }

    pub fn current_config(&self) -> &SimConfig {
        &self.stages[self.stage_index].config
    }

    pub fn current_stage_name(&self) -> &str {
        self.stages[self.stage_index].name
    }

    /// `(stage_number, stage_count, frames_recorded, frames_needed)`, all
    /// 1-based/absolute for direct display (e.g. "Stage 2/3 — 64/180").
    pub fn progress(&self) -> (usize, usize, u32, u32) {
        (
            self.stage_index + 1,
            self.stages.len(),
            self.samples.len() as u32,
            FRAMES_PER_STAGE,
        )
    }

    /// Records one real frame's cost in milliseconds. Returns `true` once
    /// every stage has finished (the caller should stop calling this and
    /// switch to `into_report`) — `false` means keep rendering
    /// `current_config()`'s scene and keep calling this.
    pub fn record_frame(&mut self, frame_ms: f32) -> bool {
        if self.warmup_remaining > 0 {
            self.warmup_remaining -= 1;
            return false;
        }
        self.samples.push(frame_ms);
        if self.samples.len() as u32 >= FRAMES_PER_STAGE {
            let stage = &self.stages[self.stage_index];
            let bounds = (
                stage.config.bounds.width,
                stage.config.bounds.height,
                stage.config.bounds.depth,
            );
            self.results.push(StageResult::from_samples(
                stage.name,
                bounds,
                stage.config.max_pipes,
                &self.samples,
            ));
            self.samples.clear();
            self.stage_index += 1;
        }
        self.stage_index >= self.stages.len()
    }

    pub fn into_report(self, app_version: String, gpu_name: String) -> Report {
        Report {
            app_version,
            os: std::env::consts::OS.to_string(),
            gpu_name,
            generated_at: humantime::format_rfc3339_seconds(std::time::SystemTime::now())
                .to_string(),
            stages: self.results,
        }
    }
}

impl Default for Run {
    fn default() -> Self {
        Self::new()
    }
}

/// Plain-text report — no branding needed here (a text file has no visual
/// identity to carry), just clearly labeled so a pasted-in bug report or
/// support email is unambiguous about what it is and where it came from.
pub fn to_text(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("neo_win_pipes — Performance Report\n");
    out.push_str(&format!("Generated: {}\n", report.generated_at));
    out.push_str(&format!("App version: {}\n", report.app_version));
    out.push_str(&format!("OS: {}\n", report.os));
    out.push_str(&format!("GPU: {}\n", report.gpu_name));
    out.push('\n');
    out.push_str(&format!(
        "{:<16} {:>16} {:>10} {:>8} {:>9} {:>9} {:>9} {:>9}\n",
        "Stage", "Grid (WxHxD)", "Max pipes", "Frames", "Avg FPS", "Avg ms", "Min ms", "Max ms"
    ));
    for s in &report.stages {
        out.push_str(&format!(
            "{:<16} {:>5}x{:<4}x{:<5} {:>10} {:>8} {:>9.1} {:>9.2} {:>9.2} {:>9.2}\n",
            s.name,
            s.bounds.0,
            s.bounds.1,
            s.bounds.2,
            s.max_pipes,
            s.frame_count,
            s.avg_fps,
            s.avg_ms,
            s.min_ms,
            s.max_ms
        ));
    }
    out
}

/// A simple branded PDF: title/wordmark, run metadata, and a results
/// table — built directly with `printpdf`'s low-level text/line drawing
/// (no layout engine dependency) since the content is short and fixed in
/// shape. Returns the finished PDF's bytes, ready to write to a file the
/// caller picked via a save dialog.
pub fn to_pdf_bytes(report: &Report) -> Vec<u8> {
    use printpdf::{Color as PdfColor, Line, Mm, PdfDocument, Point, Rgb};

    const PAGE_W_MM: f32 = 210.0; // A4
    const PAGE_H_MM: f32 = 297.0;
    const MARGIN_MM: f32 = 20.0;
    const ACCENT: (f32, f32, f32) = (0.35, 0.65, 0.95); // the site's blue accent

    let (doc, page1, layer1) = PdfDocument::new(
        "neo_win_pipes Performance Report",
        Mm(PAGE_W_MM),
        Mm(PAGE_H_MM),
        "Layer 1",
    );
    let layer = doc.get_page(page1).get_layer(layer1);

    let font = doc
        .add_builtin_font(printpdf::BuiltinFont::HelveticaBold)
        .expect("built-in font is always available");
    let font_regular = doc
        .add_builtin_font(printpdf::BuiltinFont::Helvetica)
        .expect("built-in font is always available");

    let mut y = PAGE_H_MM - MARGIN_MM;

    // Branding header: an accent-colored rule under the wordmark, echoing
    // the splash site's own header treatment rather than a generic black
    // title on white.
    layer.use_text("neo_win_pipes", 22.0, Mm(MARGIN_MM), Mm(y), &font);
    y -= 8.0;
    layer.set_outline_color(PdfColor::Rgb(Rgb::new(ACCENT.0, ACCENT.1, ACCENT.2, None)));
    layer.set_outline_thickness(1.5);
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(MARGIN_MM), Mm(y)), false),
            (Point::new(Mm(PAGE_W_MM - MARGIN_MM), Mm(y)), false),
        ],
        is_closed: false,
    });
    y -= 10.0;

    layer.use_text("Performance Report", 15.0, Mm(MARGIN_MM), Mm(y), &font);
    y -= 10.0;

    layer.set_fill_color(PdfColor::Rgb(Rgb::new(0.2, 0.2, 0.2, None)));
    for line in [
        format!("Generated: {}", report.generated_at),
        format!("App version: {}", report.app_version),
        format!("OS: {}", report.os),
        format!("GPU: {}", report.gpu_name),
    ] {
        layer.use_text(&line, 11.0, Mm(MARGIN_MM), Mm(y), &font_regular);
        y -= 6.0;
    }
    y -= 6.0;

    // Results table: hand-drawn columns rather than a table-layout crate —
    // three fixed rows (one per stage), not worth the extra dependency.
    let col_x = [
        MARGIN_MM,
        MARGIN_MM + 32.0,
        MARGIN_MM + 70.0,
        MARGIN_MM + 100.0,
        MARGIN_MM + 125.0,
        MARGIN_MM + 150.0,
    ];
    let headers = [
        "Stage",
        "Grid",
        "Max pipes",
        "Avg FPS",
        "Avg ms",
        "Min/Max ms",
    ];
    for (x, h) in col_x.iter().zip(headers.iter()) {
        layer.use_text(*h, 10.0, Mm(*x), Mm(y), &font);
    }
    y -= 5.0;
    layer.set_outline_color(PdfColor::Rgb(Rgb::new(0.7, 0.7, 0.7, None)));
    layer.set_outline_thickness(0.5);
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(MARGIN_MM), Mm(y)), false),
            (Point::new(Mm(PAGE_W_MM - MARGIN_MM), Mm(y)), false),
        ],
        is_closed: false,
    });
    y -= 6.0;

    layer.set_fill_color(PdfColor::Rgb(Rgb::new(0.1, 0.1, 0.1, None)));
    for s in &report.stages {
        let cells = [
            s.name.clone(),
            format!("{}x{}x{}", s.bounds.0, s.bounds.1, s.bounds.2),
            s.max_pipes.to_string(),
            format!("{:.1}", s.avg_fps),
            format!("{:.2}", s.avg_ms),
            format!("{:.2} / {:.2}", s.min_ms, s.max_ms),
        ];
        for (x, cell) in col_x.iter().zip(cells.iter()) {
            layer.use_text(cell, 10.0, Mm(*x), Mm(y), &font_regular);
        }
        y -= 7.0;
    }

    y -= 8.0;
    layer.set_fill_color(PdfColor::Rgb(Rgb::new(0.5, 0.5, 0.5, None)));
    layer.use_text(
        "neowinpipes.com — a cross-platform recreation of the classic Windows 3D Pipes screensaver",
        8.5,
        Mm(MARGIN_MM),
        Mm(y),
        &font_regular,
    );

    doc.save_to_bytes()
        .expect("in-memory PDF write cannot fail")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> Report {
        Report {
            app_version: "0.6.0".to_string(),
            os: "windows".to_string(),
            gpu_name: "Test GPU (Vulkan)".to_string(),
            generated_at: "2026-08-25T00:00:00Z".to_string(),
            stages: vec![StageResult::from_samples(
                "Light",
                (24, 16, 24),
                6,
                &[10.0, 20.0, 30.0],
            )],
        }
    }

    #[test]
    fn stage_result_computes_avg_min_max_fps_correctly() {
        let result = StageResult::from_samples("Light", (24, 16, 24), 6, &[10.0, 20.0, 30.0]);
        assert_eq!(result.frame_count, 3);
        assert!((result.avg_ms - 20.0).abs() < 1e-6);
        assert!((result.min_ms - 10.0).abs() < 1e-6);
        assert!((result.max_ms - 30.0).abs() < 1e-6);
        // 1000ms / 20ms average frame time = 50 fps.
        assert!((result.avg_fps - 50.0).abs() < 1e-4);
    }

    #[test]
    fn run_discards_warmup_frames_before_recording_any_sample() {
        let mut run = Run::new();
        for _ in 0..WARMUP_FRAMES {
            assert!(
                !run.record_frame(999.0),
                "warmup frames must never finish a run"
            );
        }
        // The huge 999ms warmup samples must not have been recorded.
        assert_eq!(
            run.progress().2,
            0,
            "warmup frames must not count toward a stage"
        );
    }

    #[test]
    fn run_advances_through_every_stage_and_reports_all_of_them() {
        let mut run = Run::new();
        let stage_count = stages().len();
        let mut finished = false;
        // Warmup, then every stage's full frame quota.
        for _ in 0..(WARMUP_FRAMES + stage_count as u32 * FRAMES_PER_STAGE) {
            finished = run.record_frame(5.0);
        }
        assert!(
            finished,
            "recording every stage's full quota must finish the run"
        );
        let report = run.into_report("0.6.0".to_string(), "Test GPU".to_string());
        assert_eq!(report.stages.len(), stage_count);
        for stage in &report.stages {
            assert_eq!(stage.frame_count, FRAMES_PER_STAGE as usize);
        }
    }

    #[test]
    fn progress_reports_the_current_stage_one_indexed() {
        let mut run = Run::new();
        for _ in 0..WARMUP_FRAMES {
            run.record_frame(5.0);
        }
        assert_eq!(
            run.progress().0,
            1,
            "first stage must report as stage 1, not 0"
        );
        for _ in 0..FRAMES_PER_STAGE {
            run.record_frame(5.0);
        }
        assert_eq!(
            run.progress().0,
            2,
            "must advance to stage 2 after stage 1's quota"
        );
    }

    #[test]
    fn to_text_includes_every_field_a_reader_would_need() {
        let text = to_text(&sample_report());
        assert!(text.contains("0.6.0"));
        assert!(text.contains("windows"));
        assert!(text.contains("Test GPU"));
        assert!(text.contains("Light"));
        assert!(text.contains("24"), "grid dimensions must appear");
    }

    #[test]
    fn to_pdf_bytes_produces_a_well_formed_pdf() {
        let bytes = to_pdf_bytes(&sample_report());
        assert!(!bytes.is_empty());
        assert_eq!(
            &bytes[0..5],
            b"%PDF-",
            "must start with the PDF file signature"
        );
        assert!(
            bytes.len() > 200,
            "a real multi-page-capable PDF is never this small if generation actually ran"
        );
    }
}
