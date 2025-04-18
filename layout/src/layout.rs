#![allow(dead_code)]
#![allow(unused)]

use crate::svg::write_svg;
use starling::aabb::AABB;
use starling::prelude::Vec2;

#[derive(Debug, Clone, Copy)]
pub enum LayoutDir {
    LeftToRight,
    TopToBottom,
}

#[derive(Debug, Clone, Copy)]
pub enum Size {
    Grow,
    Fit,
    Fixed(f32),
}

impl Size {
    fn as_fixed(&self) -> Option<f32> {
        match self {
            Size::Fixed(s) => Some(*s),
            _ => None,
        }
    }

    fn is_grow(&self) -> bool {
        match self {
            Size::Grow => true,
            _ => false,
        }
    }

    fn is_fit(&self) -> bool {
        match self {
            Size::Fit => true,
            _ => false,
        }
    }

    fn is_fixed(&self) -> bool {
        match self {
            Size::Fixed(_) => true,
            _ => false,
        }
    }
}

impl Into<Size> for f32 {
    fn into(self) -> Size {
        Size::Fixed(self)
    }
}

impl Into<Size> for u32 {
    fn into(self) -> Size {
        Size::Fixed(self as f32)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NodeStyle {
    layout: LayoutDir,
    child_gap: f32,
    padding: f32,
    visible: bool,
    enabled_color: [f32; 4],
    disabled_color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct Node<IdType> {
    desired_width: Size,
    desired_height: Size,
    calculated_width: Option<f32>,
    calculated_height: Option<f32>,
    calculated_position: Option<Vec2>,
    layer: Option<u32>,
    children: Vec<Node<IdType>>,
    id: Option<IdType>,
    text_content: Option<String>,
    enabled: bool,
    style: NodeStyle,
}

impl<IdType> Node<IdType> {
    pub fn new(width: impl Into<Size>, height: impl Into<Size>) -> Self {
        let w = width.into();
        let h = height.into();
        Node {
            desired_width: w,
            desired_height: h,
            calculated_width: w.as_fixed(),
            calculated_height: h.as_fixed(),
            calculated_position: None,
            layer: None,
            children: Vec::new(),
            id: None,
            text_content: None,
            enabled: true,
            style: NodeStyle {
                layout: LayoutDir::LeftToRight,
                child_gap: 10.0,
                padding: 10.0,
                visible: true,
                enabled_color: [1.0, 0.6, 0.0, 0.2],
                disabled_color: [0.2, 0.2, 0.2, 0.8],
            },
        }
    }

    pub fn grow() -> Self {
        Node::new(Size::Grow, Size::Grow)
    }

    pub fn fit() -> Self {
        Node::new(Size::Fit, Size::Fit)
    }

    pub fn row(height: impl Into<Size>) -> Self {
        Node::new(Size::Grow, height).right()
    }

    pub fn button(
        s: impl Into<String>,
        id: impl Into<IdType>,
        width: impl Into<Size>,
        height: impl Into<Size>,
    ) -> Self {
        Node::<IdType>::new(width, height).with_text(s).with_id(id)
    }

    pub fn column(width: impl Into<Size>) -> Self {
        Node::new(width, Size::Grow).down()
    }

    pub fn hline() -> Self {
        Node::row(0).with_color([0.0, 0.0, 0.0, 0.5])
    }

    pub fn vline() -> Self {
        Node::column(0).with_color([0.0, 0.0, 0.0, 0.5])
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn text_content(&self) -> Option<&String> {
        self.text_content.as_ref()
    }

    pub fn with_text(mut self, s: impl Into<String>) -> Self {
        self.text_content = Some(s.into());
        self
    }

    pub fn grid(
        width: impl Into<Size>,
        height: impl Into<Size>,
        rows: u32,
        cols: u32,
        spacing: f32,
        func: impl Fn(u32) -> Option<Node<IdType>>,
    ) -> Self {
        let mut i = 0;
        let mut root = Node::new(width, height)
            .invisible()
            .with_padding(0.0)
            .with_child_gap(spacing)
            .down();

        for r in 0..rows {
            let mut node = Node::grow()
                .with_padding(0.0)
                .with_child_gap(spacing)
                .invisible();
            for c in 0..cols {
                let n = match func(i) {
                    Some(n) => n,
                    None => Node::grow().enabled(false).invisible(),
                };
                i += 1;
                node.add_child(n);
            }
            root.add_child(node);
        }

        root
    }

    pub fn id(&self) -> Option<&IdType> {
        self.id.as_ref()
    }

    pub fn with_id(mut self, id: impl Into<IdType>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_layout(mut self, layout: LayoutDir) -> Self {
        self.style.layout = layout;
        self
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.style.enabled_color = color;
        self
    }

    pub fn right(mut self) -> Self {
        self.style.layout = LayoutDir::LeftToRight;
        self
    }

    pub fn down(mut self) -> Self {
        self.style.layout = LayoutDir::TopToBottom;
        self
    }

    pub fn invisible(mut self) -> Self {
        self.style.visible = false;
        self
    }

    pub fn with_child_gap(mut self, child_gap: f32) -> Self {
        self.style.child_gap = child_gap;
        self
    }

    pub fn with_padding(mut self, padding: f32) -> Self {
        self.style.padding = padding;
        self
    }

    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.style.padding = spacing;
        self.style.child_gap = spacing;
        self
    }

    pub fn tight(mut self) -> Self {
        self.style.padding = 0.0;
        self.style.child_gap = 0.0;
        self
    }

    pub fn with_child(mut self, n: Node<IdType>) -> Self {
        self.add_child(n);
        self
    }

    pub fn with_children(mut self, nodes: impl Iterator<Item = Node<IdType>>) -> Self {
        nodes.for_each(|n| {
            self.add_child(n);
        });
        self
    }

    pub fn children(&self) -> impl Iterator<Item = &Node<IdType>> + use<'_, IdType> {
        self.children.iter()
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_visible(&self) -> bool {
        self.style.visible
    }

    pub fn add_child(&mut self, n: Node<IdType>) -> &mut Self {
        self.children.push(n);
        self
    }

    pub fn layer(&self) -> u32 {
        self.layer.unwrap_or(0)
    }

    pub fn color(&self) -> [f32; 4] {
        if self.enabled {
            self.style.enabled_color
        } else {
            self.style.disabled_color
        }
    }

    pub fn fixed_dims(&self) -> Vec2 {
        Vec2::new(
            self.desired_width.as_fixed().unwrap_or(0.0),
            self.desired_height.as_fixed().unwrap_or(0.0),
        )
    }

    pub fn calculated_dims(&self) -> Vec2 {
        Vec2::new(
            self.calculated_width.unwrap_or(0.0),
            self.calculated_height.unwrap_or(0.0),
        )
    }

    pub fn aabb(&self) -> AABB {
        let a = self.calculated_position.unwrap_or(Vec2::ZERO);
        let b = a + self.calculated_dims();
        AABB::from_arbitrary(a, b)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Node<IdType>> + use<'_, IdType> {
        let self_iter = [self].into_iter();
        let child_iters: Vec<&Node<IdType>> = self
            .children
            .iter()
            .flat_map(|n| n.iter())
            .collect::<Vec<_>>();
        self_iter.chain(child_iters)
    }
}

fn sum_fixed_dims<'a, IdType: 'a>(
    layout: LayoutDir,
    nodes: impl Iterator<Item = &'a Node<IdType>>,
    padding: f32,
    childgap: f32,
) -> Vec2 {
    let mut sx: f32 = 0.0;
    let mut sy: f32 = 0.0;

    for node in nodes {
        let dims = node.fixed_dims();
        match layout {
            LayoutDir::LeftToRight => {
                sx += dims.x + childgap;
                sy = sy.max(dims.y);
            }
            LayoutDir::TopToBottom => {
                sx = sx.max(dims.x);
                sy += dims.y + childgap;
            }
        };
    }

    if sx > 0.0 {
        match layout {
            LayoutDir::LeftToRight => sx -= childgap,
            _ => (),
        }
    }

    if sy > 0.0 {
        match layout {
            LayoutDir::TopToBottom => sy -= childgap,
            _ => (),
        }
    }

    sx += padding * 2.0;
    sy += padding * 2.0;

    Vec2::new(sx, sy)
}

fn populate_positions<'a, IdType: 'a>(
    mut root: &mut Node<IdType>,
    origin: impl Into<Option<Vec2>>,
) {
    let origin = origin.into().unwrap_or(Vec2::ZERO);
    root.calculated_position = Some(origin);

    let mut px = origin.x + root.style.padding;
    let mut py = origin.y + root.style.padding;

    root.children.iter_mut().for_each(|n| {
        let dim = n.calculated_dims();
        let o = Vec2::new(px, py);
        match root.style.layout {
            LayoutDir::LeftToRight => px += dim.x + root.style.child_gap,
            LayoutDir::TopToBottom => py += dim.y + root.style.child_gap,
        }
        populate_positions(n, o)
    });
}

fn assign_layers<IdType>(root: &mut Node<IdType>, layer: u32) {
    root.layer = Some(layer);

    for c in &mut root.children {
        assign_layers(c, layer + 1);
    }
}

pub fn populate_fit_sizes<IdType>(root: &mut Node<IdType>) {
    if root.is_leaf() {
        if root.desired_width.is_fit() {
            root.calculated_width = Some(0.0);
        }
        if root.desired_height.is_fit() {
            root.calculated_height = Some(0.0);
        }
        return;
    }

    root.children.iter_mut().for_each(|n| populate_fit_sizes(n));

    let dims = sum_fixed_dims(
        root.style.layout,
        root.children.iter(),
        root.style.padding,
        root.style.child_gap,
    );

    if root.desired_width.is_fit() {
        root.calculated_width = Some(dims.x);
    }

    if root.desired_height.is_fit() {
        root.calculated_height = Some(dims.y);
    }
}

pub fn populate_grow_sizes<IdType>(root: &mut Node<IdType>) {
    if root.is_leaf() {
        return;
    }

    let n_to_grow: u32 = root
        .children
        .iter()
        .map(|n| match root.style.layout {
            LayoutDir::LeftToRight => n.desired_width.is_grow(),
            LayoutDir::TopToBottom => n.desired_height.is_grow(),
        } as u32)
        .sum();

    let mut w = root.calculated_width.unwrap_or(0.0) - root.style.padding * 2.0;
    let mut h = root.calculated_height.unwrap_or(0.0) - root.style.padding * 2.0;

    for c in &root.children {
        match root.style.layout {
            LayoutDir::LeftToRight => {
                w -= (c.calculated_width.unwrap_or(0.0) + root.style.child_gap)
            }
            LayoutDir::TopToBottom => {
                h -= (c.calculated_height.unwrap_or(0.0) + root.style.child_gap)
            }
        }
    }

    let n_to_grow = n_to_grow.max(1);

    match root.style.layout {
        LayoutDir::LeftToRight => {
            w += root.style.child_gap;
            w /= n_to_grow as f32;
        }
        LayoutDir::TopToBottom => {
            h += root.style.child_gap;
            h /= n_to_grow as f32;
        }
    }

    root.children.iter_mut().for_each(|mut c| {
        if c.desired_width.is_grow() {
            c.calculated_width = Some(w);
        }
        if c.desired_height.is_grow() {
            c.calculated_height = Some(h);
        }
        populate_grow_sizes(c)
    });
}

pub struct Tree<IdType> {
    roots: Vec<Node<IdType>>,
}

impl<IdType> Tree<IdType> {
    pub fn new() -> Tree<IdType> {
        Tree { roots: Vec::new() }
    }

    pub fn add_layout(&mut self, mut node: Node<IdType>, origin: impl Into<Option<Vec2>>) {
        let origin = origin.into().unwrap_or(Vec2::ZERO);
        populate_fit_sizes(&mut node);
        populate_grow_sizes(&mut node);
        populate_positions(&mut node, origin);
        assign_layers(&mut node, 0);
        self.roots.push(node);
    }

    pub fn with_layout(mut self, node: Node<IdType>, origin: impl Into<Option<Vec2>>) -> Self {
        self.add_layout(node, origin);
        self
    }

    pub fn layouts(&self) -> &Vec<Node<IdType>> {
        &self.roots
    }

    pub fn at(&self, p: Vec2) -> Option<&Node<IdType>> {
        for layout in self.roots.iter().rev() {
            let mut candidates: Vec<&Node<IdType>> =
                layout.iter().filter(|n| n.aabb().contains(p)).collect();
            if candidates.is_empty() {
                continue;
            }
            candidates.sort_by_key(|n| n.layer());
            return candidates.last().map(|v| *v);
        }
        None
    }
}

pub fn write_layout_to_svg<T>(filepath: &str, tree: &Tree<T>) -> Result<(), std::io::Error> {
    let aabbs: Vec<(AABB, [f32; 4])> = tree
        .layouts()
        .iter()
        .flat_map(|r| r.iter().map(|n| n).collect::<Vec<_>>())
        .filter_map(|n| n.is_visible().then(|| (n.aabb(), n.color())))
        .collect();

    write_svg(filepath, &aabbs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::write_svg;

    #[test]
    fn write_layout() {
        let tree = crate::examples::example_layout(1700.0, 1200.0);
        write_layout_to_svg("example_layout.svg", &tree).unwrap();
    }

    #[test]
    fn fixed_dims() {
        let a = Node::new(300.0, 700.0);
        let b = Node::new(200.0, 400.0);
        let c = Node::new(550.0, 300.0);

        let nodes = [&a, &b, &c];

        let l2r = sum_fixed_dims(LayoutDir::LeftToRight, nodes.into_iter(), 0.0, 0.0);
        let t2b = sum_fixed_dims(LayoutDir::TopToBottom, nodes.into_iter(), 0.0, 0.0);

        assert_eq!(l2r.x, 1050.0);
        assert_eq!(l2r.y, 700.0);

        assert_eq!(t2b.x, 550.0);
        assert_eq!(t2b.y, 1400.0);

        let l2r = sum_fixed_dims(LayoutDir::LeftToRight, nodes.into_iter(), 12.0, 7.5);
        let t2b = sum_fixed_dims(LayoutDir::TopToBottom, nodes.into_iter(), 12.0, 7.5);

        assert_eq!(l2r.x, 1089.0);
        assert_eq!(l2r.y, 724.0);

        assert_eq!(t2b.x, 574.0);
        assert_eq!(t2b.y, 1439.0);

        let mut root = Node::<String>::new(Size::Fit, Size::Fit)
            .with_child(a)
            .with_child(b)
            .with_child(c);

        populate_fit_sizes(&mut root);
        populate_grow_sizes(&mut root);
        populate_positions(&mut root, None);
        assign_layers(&mut root, 0);

        let aabbs = root
            .iter()
            .map(|n| (n.aabb(), n.color()))
            .collect::<Vec<_>>();
        write_svg("boxes.svg", &aabbs).unwrap();

        let dims = root.calculated_dims();

        assert_eq!(dims.x, 1090.0);
        assert_eq!(dims.y, 720.0);
    }
}
