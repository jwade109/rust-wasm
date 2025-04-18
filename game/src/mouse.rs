use crate::planetary::GameState;
use crate::ui::InteractionEvent;
use bevy::prelude::*;
use core::time::Duration;
use starling::nanotime::Nanotime;
use starling::prelude::AABB;

const DOUBLE_CLICK_DURATION: Nanotime = Nanotime::millis(400);

#[derive(Debug, Clone, Copy)]
struct MouseFrame {
    frame_no: u32,
    screen_pos: Vec2,
    wall_time: Nanotime,
}

impl MouseFrame {
    fn age(&self, wall_time: Nanotime) -> Nanotime {
        wall_time - self.wall_time
    }
}

#[derive(Default, Debug, Clone, Copy)]
enum CursorTravel {
    #[default]
    None,
    Traveling(MouseFrame, MouseFrame),
    Finished(MouseFrame, MouseFrame),
}

impl CursorTravel {
    fn set_down(&mut self, current_frame: MouseFrame) {
        let next = match self {
            Self::None => Self::Traveling(current_frame, current_frame),
            Self::Traveling(down, _) => Self::Traveling(*down, current_frame),
            Self::Finished(_, _) => Self::Traveling(current_frame, current_frame),
        };

        *self = next;
    }

    fn set_up(&mut self) {
        let down = match self.down() {
            Some(d) => d,
            None => return,
        };
        let up = match self.current() {
            Some(d) => d,
            None => return,
        };

        *self = Self::Finished(*down, *up);
        println!("Mouse up: {:?}", &self);
    }

    fn down(&self) -> Option<&MouseFrame> {
        match &self {
            Self::None => None,
            Self::Traveling(f, _) | Self::Finished(f, _) => Some(f),
        }
    }

    fn current(&self) -> Option<&MouseFrame> {
        match &self {
            Self::Traveling(_, c) => Some(c),
            _ => None,
        }
    }

    fn up(&self) -> Option<&MouseFrame> {
        match &self {
            Self::Finished(_, f) => Some(f),
            _ => None,
        }
    }

    fn frame(&self, order: FrameId) -> Option<&MouseFrame> {
        match order {
            FrameId::Current => self.current(),
            FrameId::Down => self.down(),
            FrameId::Up => self.up(),
        }
    }
}

#[derive(Debug, Default)]
pub struct MouseState {
    hover: CursorTravel,
    left: CursorTravel,
    right: CursorTravel,
    middle: CursorTravel,

    pub viewport_bounds: AABB,
    pub world_bounds: AABB,
    pub scale: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButt {
    Hover,
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy)]
pub enum FrameId {
    Down,
    Current,
    Up,
}

impl MouseState {
    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn position(&self, button: MouseButt, order: FrameId) -> Option<Vec2> {
        let state = self.get_state(button);
        let frame = state.frame(order)?;
        Some(frame.screen_pos)
    }

    pub fn age(&self, button: MouseButt, order: FrameId, wall_time: Nanotime) -> Option<Nanotime> {
        let state = self.get_state(button);
        let frame = state.frame(order)?;
        Some(wall_time - frame.wall_time)
    }

    fn get_state(&self, button: MouseButt) -> &CursorTravel {
        match button {
            MouseButt::Hover => &self.hover,
            MouseButt::Left => &self.left,
            MouseButt::Right => &self.right,
            MouseButt::Middle => &self.middle,
        }
    }

    pub fn on_frame(&self, button: MouseButt, order: FrameId, frame_no: u32) -> bool {
        let state = self.get_state(button);
        let frame = state.frame(order);
        frame.map(|f| f.frame_no == frame_no).unwrap_or(false)
    }

    fn viewport_to_world(&self, p: Vec2) -> Vec2 {
        self.viewport_bounds.map(self.world_bounds, p)
    }

    pub fn world_position(&self, button: MouseButt, order: FrameId) -> Option<Vec2> {
        let p = self.position(button, order)?;
        Some(self.viewport_to_world(p))
    }

    pub fn ui_position(&self, button: MouseButt, order: FrameId) -> Option<Vec2> {
        let p = self.position(button, order)?;
        let p = Vec2::new(p.x, self.viewport_bounds.span.y - p.y);
        Some(p)
    }
}

pub fn update_mouse_state(
    win: Single<&Window>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera: Single<&Transform, With<crate::planetary::SoftController>>,
    mut state: ResMut<GameState>,
    mut events: EventWriter<InteractionEvent>,
) {
    let dims = Vec2::new(win.width(), win.height());
    let t = state.wall_time;
    let f = state.current_frame_no;

    state.mouse.viewport_bounds = AABB::new(dims / 2.0, dims);
    state.mouse.world_bounds = AABB::new(camera.translation.xy(), dims * camera.scale.z);
    state.mouse.scale = camera.scale.z;

    let current_frame = if let Some(p) = win.cursor_position() {
        let p = Vec2::new(p.x, dims.y - p.y);
        MouseFrame {
            frame_no: f,
            screen_pos: p,
            wall_time: t,
        }
    } else {
        state.mouse.hover.set_up();
        state.mouse.left.set_up();
        state.mouse.right.set_up();
        state.mouse.middle.set_up();
        return;
    };

    state.mouse.hover.set_down(current_frame);

    if buttons.pressed(MouseButton::Left) {
        let age = state.mouse.left.up().map(|f| f.age(t));
        if let Some(age) = age {
            if age < DOUBLE_CLICK_DURATION {
                events.send(InteractionEvent::DoubleClick(current_frame.screen_pos));
            }
        }
        state.mouse.left.set_down(current_frame);
    } else {
        state.mouse.left.set_up();
    }

    if buttons.pressed(MouseButton::Right) {
        state.mouse.right.set_down(current_frame);
    } else {
        state.mouse.right.set_up();
    }

    if buttons.pressed(MouseButton::Middle) {
        state.mouse.middle.set_down(current_frame);
    } else {
        state.mouse.middle.set_up();
    }
}
