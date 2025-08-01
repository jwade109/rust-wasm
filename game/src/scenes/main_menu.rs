#![allow(unused)]

use crate::canvas::Canvas;
use crate::game::GameState;
use crate::onclick::OnClick;
use crate::scenes::*;
use bevy::color::palettes::css::*;
use bevy::prelude::*;
use layout::layout::{Node, Size, Tree};
use starling::prelude::*;
use std::collections::HashMap;

pub struct MainMenuContext;

impl Default for MainMenuContext {
    fn default() -> Self {
        MainMenuContext {}
    }
}

impl Render for MainMenuContext {
    fn background_color(state: &GameState) -> Srgba {
        BLACK
    }

    fn draw(canvas: &mut Canvas, state: &GameState) -> Option<()> {
        let dims = state.input.screen_bounds.span;
        let time = compile_time::datetime_str!();
        let dir = match std::fs::canonicalize(state.args.install_dir.clone()) {
            Ok(dir) => dir.to_string_lossy().to_string(),
            Err(e) => format!("{} (\"{}\")", e, state.args.install_dir.clone().display()),
        };
        let n_vehicles = get_list_of_vehicles(state).map(|l| l.len()).unwrap_or(0);
        let s = format!(
            "Compiled on {}\nInstall directory: {}\n{} parts loaded\n{} vehicles loaded\n{} sprites loaded",
            time,
            dir,
            state.part_database.len(),
            n_vehicles,
            state.image_handles.len(),
        )
        .to_uppercase();
        let p = Vec2::new(-dims.x / 2.0 + 200.0, -dims.y / 2.0 + 140.0);

        let t = TextLabel::new(s, p, 0.6).with_anchor_left();
        canvas.label(t);

        crate::drawing::draw_cells(&mut canvas.gizmos, state);

        crate::drawing::draw_orbital_view(canvas, state);

        Some(())
    }

    fn ui(state: &GameState) -> Option<Tree<OnClick>> {
        let buttons = ["Load Save File", "Settings", "Exit"];
        let button_color = [0.2, 0.2, 0.2, 0.7];
        let bg_color = [0.0, 0.0, 0.0, 0.0];

        let wrapper = Node::structural(250, Size::Fit)
            .down()
            .with_color(bg_color)
            .with_children(buttons.iter().map(|s| {
                Node::button(
                    s.to_string(),
                    OnClick::Nullopt,
                    Size::Grow,
                    state.settings.ui_button_height,
                )
                .with_color(button_color)
            }))
            .with_children(state.scenes.iter().enumerate().map(|(i, s)| {
                Node::button(
                    s.name(),
                    OnClick::GoToScene(i),
                    Size::Grow,
                    state.settings.ui_button_height,
                )
                .with_color(button_color)
            }))
            .with_child({
                let s = "Reload";
                let onclick = OnClick::ReloadGame;
                Node::button(s, onclick, Size::Grow, state.settings.ui_button_height)
            });

        Some(Tree::new().with_layout(wrapper, Vec2::splat(300.0)))
    }
}
