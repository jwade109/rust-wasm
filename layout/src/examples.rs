use crate::layout::*;

fn box_with_corners(w: f32) -> Node<String> {
    let banner = || {
        Node::row(Size::Fit)
            .invisible()
            .with_child(Node::structural(w, w))
            .with_child(Node::grow().invisible())
            .with_child(Node::structural(w, w))
            .with_child(Node::grow().invisible())
            .with_child(Node::structural(w, w))
    };

    Node::grow()
        .invisible()
        .tight()
        .down()
        .with_child(banner())
        .with_child(Node::grow().invisible())
        .with_child(banner())
}

#[allow(unused)]
fn text_dims(s: &str) -> (usize, usize) {
    let max_line = s.lines().map(|l| l.len()).max().unwrap_or(0);
    let lines = s.lines().count();
    (lines, max_line)
}

#[allow(unused)]
fn text_node(s: &str, width: impl Into<Size>, height: impl Into<Size>) -> Node<String> {
    let chr_width = 15.0;
    let chr_height = 30.0;
    let (lines, max_line) = text_dims(&s);
    let twidth = max_line as f32 * chr_width;
    let theight = lines as f32 * chr_height;
    Node::structural(width, height).tight().down().with_child(
        Node::grow()
            .tight()
            .with_child(Node::grow())
            .with_child(
                Node::column(Size::Fit)
                    .tight()
                    .with_child(Node::grow())
                    .with_child(Node::text(twidth, theight, s))
                    .with_child(Node::grow()),
            )
            .with_child(Node::grow()),
    )
}

pub fn example_layout(width: f32, height: f32) -> Tree<String> {
    let spacing = 8.0;

    let sidebar = Node::column(300.0)
        .with_spacing(spacing)
        .with_child(Node::button(
            "wow this is\na fair amount\nof text",
            "dingus",
            Size::Grow,
            Size::Grow,
        ))
        .with_child(Node::grid(Size::Grow, Size::Grow, 2, 4, spacing, |_| {
            Some(Node::grow())
        }))
        .with_child(Node::hline())
        .with_children((0..6).map(|i| {
            Node::row(Size::Fit)
                .invisible()
                .with_padding(0.0)
                .with_child(Node::structural(40 + i * 6, 10))
                .with_child(Node::vline())
                .with_child(Node::grow())
        }))
        .with_child(Node::row(30))
        .with_child(
            Node::grid(Size::Grow, 100, 4, 5, spacing, |_| Some(Node::grow()))
                .with_child(Node::vline())
                .with_child(Node::grow().with_text("hello\ndingus")),
        )
        .with_children((0..4).map(|_| Node::row(25)));

    let topbar = Node::row(Size::Fit)
        .with_spacing(spacing)
        .with_children((0..10).map(|i| Node::text(120, 40, &format!("thing {}", i))))
        .with_children((0..5).map(|_| Node::grow().invisible()))
        .with_child(Node::column(70).with_text("Exit").with_on_click("exit"));

    let main = Node::grow()
        .tight()
        .invisible()
        .with_child(sidebar)
        .with_child(
            Node::grow()
                .invisible()
                .down()
                .tight()
                .with_children([box_with_corners(40.0), Node::row(30)].into_iter()),
        );

    let root = Node::structural(width, height)
        .invisible()
        .tight()
        .down()
        .with_child(topbar)
        .with_child(main);

    Tree::new().with_layout(root, None)
}
