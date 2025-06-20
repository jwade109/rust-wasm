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
    color: Srgba,
    pub anchor: Anchor,
}

impl TextLabel {
    pub fn new(text: String, pos: Vec2, size: f32) -> Self {
        Self {
            text,
            pos,
            size,
            color: WHITE,
            anchor: Anchor::Center,
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

    pub fn with_anchor_left(mut self) -> Self {
        self.anchor = Anchor::CenterLeft;
        self
    }

    pub fn color(&self) -> Srgba {
        self.color
    }
}

#[derive(Debug, Clone)]
pub enum SpritePath {
    Procedural(String),
    Filesystem(String),
}

#[derive(Debug, Clone)]
pub struct StaticSpriteDescriptor {
    pub position: Vec2,
    pub angle: f32,
    pub path: SpritePath,
    pub scale: f32,
    pub z_index: f32,
    pub color: Option<Srgba>,
}

impl StaticSpriteDescriptor {
    pub fn filesystem(position: Vec2, angle: f32, path: String, scale: f32, z_index: f32) -> Self {
        Self {
            position,
            angle,
            path: SpritePath::Filesystem(path),
            scale,
            z_index,
            color: None,
        }
    }

    pub fn procedural(position: Vec2, angle: f32, path: String, scale: f32, z_index: f32) -> Self {
        Self {
            position,
            angle,
            path: SpritePath::Procedural(path),
            scale,
            z_index,
            color: None,
        }
    }

    pub fn with_color(mut self, color: Srgba) -> Self {
        self.color = Some(color);
        self
    }
}

pub trait Render {
    fn text_labels(_state: &GameState) -> Option<Vec<TextLabel>> {
        None
    }

    fn sprites(_state: &GameState) -> Option<Vec<StaticSpriteDescriptor>> {
        None
    }

    fn background_color(state: &GameState) -> Srgba;

    fn ui(state: &GameState) -> Option<Tree<OnClick>>;

    fn draw(_canvas: &mut Canvas, _state: &GameState) -> Option<()> {
        None
    }
}
