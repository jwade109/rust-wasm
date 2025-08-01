use crate::canvas::Canvas;
use crate::game::GameState;
use crate::onclick::OnClick;
use bevy::color::palettes::css::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use layout::layout::Tree;
use starling::math::Vec2;

#[derive(Debug, Clone)]
pub struct TextLabel {
    pub text: String,
    pub pos: Vec2,
    pub size: f32,
    pub color: Srgba,
    pub anchor: Anchor,
    pub z_index: f32,
}

impl TextLabel {
    pub fn new(text: String, pos: Vec2, size: f32) -> Self {
        Self {
            text,
            pos,
            size,
            color: WHITE,
            anchor: Anchor::Center,
            z_index: 100.0,
        }
    }

    pub fn with_color(mut self, color: Srgba) -> Self {
        self.color = color;
        self
    }

    pub fn anchor_left(&mut self) -> &mut Self {
        self.anchor = Anchor::CenterLeft;
        self
    }

    pub fn anchor_right(&mut self) -> &mut Self {
        self.anchor = Anchor::CenterRight;
        self
    }

    pub fn anchor_top_left(&mut self) -> &mut Self {
        self.anchor = Anchor::TopLeft;
        self
    }

    pub fn anchor_bottom_left(&mut self) -> &mut Self {
        self.anchor = Anchor::BottomLeft;
        self
    }

    pub fn with_anchor_left(mut self) -> Self {
        self.anchor = Anchor::CenterLeft;
        self
    }

    pub fn color(&self) -> Srgba {
        self.color
    }
}

#[derive(Debug, Clone)]
pub struct StaticSpriteDescriptor {
    pub position: Vec2,
    pub angle: f32,
    pub path: String,
    pub dims: Vec2,
    pub z_index: f32,
    pub color: Option<Srgba>,
}

impl StaticSpriteDescriptor {
    pub fn new(position: Vec2, angle: f32, path: String, dims: Vec2, z_index: f32) -> Self {
        Self {
            position,
            angle,
            path,
            dims,
            z_index,
            color: None,
        }
    }

    pub fn with_color(mut self, color: impl Into<Srgba>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn set_color(&mut self, color: impl Into<Srgba>) {
        self.color = Some(color.into());
    }
}

pub trait Render {
    fn background_color(state: &GameState) -> Srgba;

    fn ui(state: &GameState) -> Option<Tree<OnClick>>;

    fn draw(_canvas: &mut Canvas, _state: &GameState) -> Option<()> {
        None
    }
}
