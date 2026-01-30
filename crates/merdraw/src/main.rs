use std::env;
use std::fs;
use std::io::{self, Read};

use merdraw_ascii_render::{render_ascii, AsciiRenderOptions};
use merdraw_layout::{layout_flowchart, LayoutStyle};
use merdraw_parser::parse_flowchart;

fn main() {
    let input = match env::args().nth(1) {
        Some(path) => fs::read_to_string(path).expect("failed to read input file"),
        None => {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .expect("failed to read stdin");
            buffer
        }
    };

    let graph = parse_flowchart(&input).expect("failed to parse flowchart");
    let layout_style = LayoutStyle {
        min_width: 24.0,
        min_height: 16.0,
        char_width: 6.0,
        char_height: 10.0,
        node_padding_x: 6.0,
        node_padding_y: 4.0,
        node_gap: 8.0,
        layer_gap: 12.0,
    };
    let layout = layout_flowchart(&graph, &layout_style);
    let output = render_ascii(&layout, &AsciiRenderOptions::default());
    println!("{output}");
}
