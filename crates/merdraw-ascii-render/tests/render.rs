use merdraw_ascii_render::{render_ascii, AsciiRenderOptions};
use merdraw_layout::{layout_flowchart, LayoutStyle};
use merdraw_parser::parse_flowchart;

#[test]
fn renders_basic_ascii() {
    let graph = parse_flowchart("flowchart TB\nA-->B-->C\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let output = render_ascii(&layout, &AsciiRenderOptions::default());
    assert!(output.contains("A"));
    assert!(output.contains("B"));
    assert!(output.contains("C"));
    assert!(output.contains("-"));
}

#[test]
fn renders_arrowheads_for_edges() {
    let graph = parse_flowchart("flowchart TB\nA-->B-->C\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let output = render_ascii(&layout, &AsciiRenderOptions::default());
    let arrow_count = output.matches('v').count();
    assert!(arrow_count >= 2);
}

#[test]
fn keeps_box_borders_intact() {
    let graph = parse_flowchart("flowchart TB\nA-->B\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let output = render_ascii(&layout, &AsciiRenderOptions::default());
    let lines = output.lines().collect::<Vec<_>>();
    let (label_line, label_index) = find_label_line(&lines, "B").expect("B label not found");
    assert!(label_line.contains('B'));
    let top_border = lines.get(label_index.saturating_sub(1)).unwrap();
    assert!(!top_border.contains('v'));
    assert!(max_run(top_border, '-') >= 3);
}

#[test]
fn sizes_box_to_fit_label() {
    let graph = parse_flowchart("flowchart TB\nNODE_LONG_LABEL\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let output = render_ascii(&layout, &AsciiRenderOptions::default());
    let lines = output.lines().collect::<Vec<_>>();
    let (_label_line, label_index) =
        find_label_line(&lines, "NODE_LONG_LABEL").expect("label not found");
    let top_border = lines.get(label_index.saturating_sub(1)).unwrap();
    assert!(max_run(top_border, '-') >= "NODE_LONG_LABEL".len() + 2);
}

#[test]
fn renders_branching_edges() {
    let graph = parse_flowchart("flowchart TB\nA-->B\nA-->C\n").expect("parse failed");
    let layout = layout_flowchart(&graph, &LayoutStyle::default());
    let output = render_ascii(&layout, &AsciiRenderOptions::default());
    let arrow_count = output.matches('v').count();
    assert!(arrow_count >= 2);
}

fn find_label_line<'a>(lines: &'a [&str], label: &str) -> Option<(&'a str, usize)> {
    for (idx, line) in lines.iter().enumerate() {
        if line.contains(label) {
            return Some((*line, idx));
        }
    }
    None
}

fn max_run(line: &str, ch: char) -> usize {
    let mut best = 0usize;
    let mut current = 0usize;
    for c in line.chars() {
        if c == ch {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best
}
