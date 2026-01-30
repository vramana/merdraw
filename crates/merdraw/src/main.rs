use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use merdraw_ascii_render::{render_ascii, AsciiRenderOptions};
use merdraw_layout::{layout_flowchart, suggest_canvas_size, LayoutStyle};
use merdraw_parser::parse_flowchart;
use merdraw_skia_render::{render_to_file, ImageFormat, SkiaRenderOptions};

fn main() {
    let options = parse_args(env::args().skip(1).collect());
    let input = read_input(options.input.as_deref());

    let graph = parse_flowchart(&input).expect("failed to parse flowchart");

    let ascii_layout_style = LayoutStyle {
        min_width: 24.0,
        min_height: 16.0,
        char_width: 6.0,
        char_height: 10.0,
        node_padding_x: 6.0,
        node_padding_y: 4.0,
        node_gap: 8.0,
        layer_gap: 12.0,
    };

    let layout_style = if options.out.is_some() {
        LayoutStyle::default()
    } else {
        ascii_layout_style
    };

    let layout = layout_flowchart(&graph, &layout_style);

    if let Some(out_path) = options.out {
        let format = match options.format.as_deref() {
            Some("png") => ImageFormat::Png,
            Some("jpg") | Some("jpeg") => ImageFormat::Jpeg {
                quality: options.quality,
            },
            Some(other) => {
                eprintln!("unsupported format: {other}");
                std::process::exit(1);
            }
            None => infer_format_from_path(&out_path).unwrap_or(ImageFormat::Png),
        };

    let mut render_options = SkiaRenderOptions {
        width: 0,
        height: 0,
        jpeg_quality: options.quality,
        font_path: options.font,
        ..SkiaRenderOptions::default()
    };
    let padding = render_options.padding;
    let (width, height) = match (options.width, options.height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => {
            let scale = ((w as f32 - padding * 2.0) / layout.width.max(1.0)).max(0.1);
            let h = (layout.height.max(1.0) * scale + padding * 2.0).ceil().max(1.0) as u32;
            (w, h)
        }
        (None, Some(h)) => {
            let scale = ((h as f32 - padding * 2.0) / layout.height.max(1.0)).max(0.1);
            let w = (layout.width.max(1.0) * scale + padding * 2.0).ceil().max(1.0) as u32;
            (w, h)
        }
        (None, None) => suggest_canvas_size(&layout, padding, 1.0),
    };
    render_options.width = width;
    render_options.height = height;
        if let Err(err) = render_to_file(&layout, format, &render_options, &out_path) {
            eprintln!("render failed: {err:?}");
            std::process::exit(1);
        }
        return;
    }

    let output = render_ascii(&layout, &AsciiRenderOptions::default());
    println!("{output}");
}

struct CliOptions {
    input: Option<String>,
    out: Option<PathBuf>,
    format: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    quality: u8,
    font: Option<PathBuf>,
}

fn parse_args(args: Vec<String>) -> CliOptions {
    let mut input = None;
    let mut out = None;
    let mut format = None;
    let mut width = None;
    let mut height = None;
    let mut quality = 85;
    let mut font = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                if let Some(path) = iter.next() {
                    out = Some(PathBuf::from(path));
                }
            }
            "--format" => {
                if let Some(value) = iter.next() {
                    format = Some(value.to_lowercase());
                }
            }
            "--width" => {
                if let Some(value) = iter.next() {
                    if let Ok(parsed) = value.parse() {
                        width = Some(parsed);
                    }
                }
            }
            "--height" => {
                if let Some(value) = iter.next() {
                    if let Ok(parsed) = value.parse() {
                        height = Some(parsed);
                    }
                }
            }
            "--quality" => {
                if let Some(value) = iter.next() {
                    if let Ok(parsed) = value.parse() {
                        quality = parsed;
                    }
                }
            }
            "--font" => {
                if let Some(value) = iter.next() {
                    font = Some(PathBuf::from(value));
                }
            }
            _ => {
                if input.is_none() {
                    input = Some(arg);
                }
            }
        }
    }

    CliOptions {
        input,
        out,
        format,
        width,
        height,
        quality,
        font,
    }
}

fn read_input(path: Option<&str>) -> String {
    match path {
        Some("-") | None => {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .expect("failed to read stdin");
            buffer
        }
        Some(path) => fs::read_to_string(path).expect("failed to read input file"),
    }
}

fn infer_format_from_path(path: &PathBuf) -> Option<ImageFormat> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    match ext.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg { quality: 85 }),
        _ => None,
    }
}
