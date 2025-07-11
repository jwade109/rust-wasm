use crate::args::ProgramContext;
use crate::camera_controller::LinearCameraController;
use crate::canvas::Canvas;
use crate::craft_editor::*;
use crate::drawing::*;
use crate::game::GameState;
use crate::input::InputState;
use crate::input::{FrameId, MouseButt};
use crate::onclick::OnClick;
use crate::scenes::{CameraProjection, Render, TextLabel};
use crate::ui::*;
use bevy::color::palettes::css::*;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use layout::layout::{Node, Size, Tree};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use starling::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleFileStorage {
    pub name: String,
    pub parts: Vec<VehiclePartFileStorage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehiclePartFileStorage {
    pub partname: String,
    pub pos: IVec2,
    pub rot: Rotation,
}

#[derive(Debug)]
pub struct EditorContext {
    camera: LinearCameraController,
    cursor_state: CursorState,
    rotation: Rotation,
    filepath: Option<PathBuf>,
    focus_layer: Option<PartLayer>,
    selected_part: Option<usize>,
    occupied: HashMap<PartLayer, HashMap<IVec2, usize>>,
    vehicle: Vehicle,

    // menus
    pub show_vehicle_info: bool,
    pub parts_menu_collapsed: bool,
    pub vehicles_menu_collapsed: bool,
    pub layers_menu_collapsed: bool,
}

impl EditorContext {
    pub fn new() -> Self {
        EditorContext {
            camera: LinearCameraController::new(Vec2::ZERO, 18.0),
            cursor_state: CursorState::None,
            rotation: Rotation::East,
            filepath: None,
            focus_layer: None,
            selected_part: None,
            occupied: HashMap::new(),
            vehicle: Vehicle::from_parts("".into(), Nanotime::zero(), Vec::new()),
            show_vehicle_info: false,
            parts_menu_collapsed: false,
            vehicles_menu_collapsed: true,
            layers_menu_collapsed: false,
        }
    }

    pub fn vehicle(&self) -> &Vehicle {
        &self.vehicle
    }

    pub fn selected_part(&self) -> Option<&PartInstance> {
        self.vehicle.get_part_by_index(self.selected_part?)
    }

    pub fn cursor_box(&self, input: &InputState) -> Option<AABB> {
        let p1 = input.position(MouseButt::Left, FrameId::Down)?;
        let p2 = input.position(MouseButt::Left, FrameId::Current)?;
        Some(AABB::from_arbitrary(
            vround(self.c2w(p1)).as_vec2(),
            vround(self.c2w(p2)).as_vec2(),
        ))
    }

    pub fn new_craft(&mut self) {
        self.filepath = None;
        self.vehicle.clear();
        self.cursor_state = CursorState::None;
        self.update();
    }

    pub fn write_image_to_file(&self, args: &ProgramContext) {
        write_image_to_file(&self.vehicle, args, "vehicle");
    }

    pub fn rotate_craft(&mut self) {
        let new_instances: Vec<_> = self
            .vehicle
            .parts()
            .map(|instance| instance.rotated())
            .collect();
        self.vehicle.clear();
        for instance in new_instances {
            self.vehicle.add_part(instance);
        }
        self.update();
    }

    pub fn normalize_coordinates(&mut self) {
        if self.vehicle.parts().count() == 0 {
            return;
        }

        let mut min: IVec2 = IVec2::ZERO;
        let mut max: IVec2 = IVec2::ZERO;

        self.vehicle.parts().for_each(|instance| {
            let dims = instance.dims_grid();
            let p = instance.origin();
            let q = p + dims.as_ivec2();
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(q.x);
            max.y = max.y.max(q.y);
        });

        let avg = min + (max - min) / 2;

        let new_parts: Vec<_> = self
            .vehicle
            .parts()
            .map(|instance| instance.with_origin(instance.origin() - avg))
            .collect();

        self.vehicle.clear();

        for part in new_parts {
            self.vehicle.add_part(part);
        }

        self.update();
    }

    pub fn set_current_part(state: &mut GameState, name: &String) {
        if let Some(part) = state.part_database.get(name).cloned() {
            state.editor_context.cursor_state = CursorState::Part(part);
        }
    }

    fn open_existing_file(&mut self) -> Option<PathBuf> {
        if let Some(p) = FileDialog::new().set_directory("/").pick_file() {
            self.filepath = Some(p);
        }
        self.filepath.clone()
    }

    fn open_file_to_save(&mut self) -> Option<PathBuf> {
        if self.filepath.is_none() {
            self.filepath = FileDialog::new().set_directory("/").save_file()
        };
        self.filepath.clone()
    }

    pub fn is_layer_visible(&self, layer: PartLayer) -> bool {
        if let Some(focus) = self.focus_layer {
            focus == layer
        } else {
            true
        }
    }

    pub fn toggle_layer(&mut self, layer: PartLayer) {
        self.focus_layer = if self.focus_layer == Some(layer) {
            None
        } else {
            Some(layer)
        };
    }

    pub fn save_to_file(state: &mut GameState) -> Option<()> {
        let choice: PathBuf = state.editor_context.open_file_to_save()?;
        state.notice(format!("Saving to {}", choice.display()));

        let parts = state
            .editor_context
            .vehicle
            .parts()
            .map(|instance| VehiclePartFileStorage {
                partname: instance.part().sprite_path().to_string(),
                pos: instance.origin(),
                rot: instance.rotation(),
            })
            .collect();

        let storage = VehicleFileStorage {
            name: "".into(),
            parts,
        };

        let s = serde_yaml::to_string(&storage).ok()?;
        std::fs::write(choice, s).ok()
    }

    pub fn load_from_file(state: &mut GameState) -> Option<()> {
        let choice = state.editor_context.open_existing_file()?;
        EditorContext::load_vehicle(&choice, state)
    }

    pub fn load_from_vehicle_file(path: &Path) -> Option<VehicleFileStorage> {
        let s = std::fs::read_to_string(path).ok()?;
        serde_yaml::from_str(&s).ok()
    }

    pub fn load_vehicle(path: &Path, state: &mut GameState) -> Option<()> {
        state.notice(format!("Loading vehicle from {}", path.display()));
        let s = std::fs::read_to_string(path).ok()?;
        let storage: VehicleFileStorage = serde_yaml::from_str(&s).ok()?;
        state.notice(format!("Loaded vehicle \"{}\"", storage.name));

        state.editor_context.vehicle.clear();
        for ps in storage.parts {
            if let Some(part) = state.part_database.get(&ps.partname) {
                let instance = PartInstance::new(ps.pos, ps.rot, part.clone());
                state.editor_context.vehicle.add_part(instance);
            } else {
                error!("Failed to load part: {}", ps.partname);
            }
        }
        state.editor_context.filepath = Some(path.to_path_buf());
        state.editor_context.update();
        state.editor_context.vehicles_menu_collapsed = true;
        Some(())
    }

    fn get_part_at(&self, p: IVec2) -> Option<&PartInstance> {
        for layer in [
            PartLayer::Exterior,
            PartLayer::Structural,
            PartLayer::Internal,
        ] {
            if !self.is_layer_visible(layer) {
                continue;
            }

            if let Some(occ) = self.occupied.get(&layer) {
                if let Some(idx) = occ.get(&p) {
                    return Some(self.vehicle.get_part_by_index(*idx)?);
                }
            }
        }

        None
    }

    fn update(&mut self) {
        self.occupied.clear();
        for (i, instance) in self.vehicle.parts().enumerate() {
            let pixels = occupied_pixels(instance.origin(), instance.rotation(), instance.part());
            if let Some(occ) = self.occupied.get_mut(&instance.part().layer()) {
                for p in pixels {
                    occ.insert(p, i);
                }
            } else {
                let mut occ = HashMap::new();
                for p in pixels {
                    occ.insert(p, i);
                }
                self.occupied.insert(instance.part().layer(), occ);
            }
        }
    }

    fn try_place_part(&mut self, p: IVec2, new_part: Part) -> Option<()> {
        let new_pixels = occupied_pixels(p, self.rotation, &new_part);
        if let Some(occ) = self.occupied.get(&new_part.layer()) {
            for p in &new_pixels {
                if occ.contains_key(p) {
                    return None;
                }
            }
        }

        let instance = PartInstance::new(p, self.rotation, new_part);
        self.vehicle.add_part(instance);
        self.update();
        Some(())
    }

    fn remove_part_at(&mut self, p: IVec2) {
        self.vehicle.remove_part_at(p, self.focus_layer);
        self.update();
    }

    fn current_part_and_cursor_position(state: &GameState) -> Option<(IVec2, Part)> {
        let ctx = &state.editor_context;
        let part = state.editor_context.cursor_state.current_part()?;
        let wh = pixel_dims_with_rotation(ctx.rotation, &part).as_ivec2();
        let pos = state.input.position(MouseButt::Hover, FrameId::Current)?;
        let pos = vround(state.editor_context.c2w(pos));
        Some((pos - wh / 2, part))
    }
}

pub fn vehicle_info(vehicle: &Vehicle) -> String {
    let bounds = vehicle.aabb();
    let fuel_economy = if vehicle.remaining_dv() > 0.0 {
        vehicle.fuel_mass().to_kg_f32() / vehicle.remaining_dv()
    } else {
        0.0
    };

    let fuel_mass = vehicle.fuel_mass();
    let rate = vehicle.fuel_consumption_rate();
    let accel = vehicle.body_frame_acceleration();
    let pct = vehicle.fuel_percentage() * 100.0;
    let burn_time = if rate > 0.0 {
        format!("{:0.1} s", fuel_mass.to_kg_f32() / rate)
    } else {
        "N/A".to_string()
    };

    [
        format!("Dry mass: {}", vehicle.dry_mass()),
        format!("Fuel: {} ({:0.0}%)", fuel_mass, pct),
        format!("Burn time: {}", burn_time),
        format!("Current mass: {}", vehicle.current_mass()),
        format!("Thrusters: {}", vehicle.thruster_count()),
        format!("Thrust: {:0.2} kN", vehicle.thrust() / 1000.0),
        format!("Tanks: {}", vehicle.tank_count()),
        format!("Accel: {:0.2} g", vehicle.accel() / 9.81),
        format!("Ve: {:0.1} s", vehicle.average_linear_exhaust_velocity()),
        format!("DV: {:0.1} m/s", vehicle.remaining_dv()),
        format!("WH: {:0.2}x{:0.2}", bounds.span.x, bounds.span.y),
        format!("Econ: {:0.2} kg-s/m", fuel_economy),
        format!("Fuel: {:0.1}/s", rate),
        format!("Accel: ({:0.2}, {:0.2}) m/s^2", accel.x, accel.y),
    ]
    .into_iter()
    .map(|s| format!("{s}\n"))
    .collect()
}

fn draw_highlight_box(canvas: &mut Canvas, aabb: AABB, ctx: &impl CameraProjection, color: Srgba) {
    let w1 = 2.0;
    let w2 = 4.0;

    let x1 = Vec2::X * w1;
    let x2 = Vec2::X * w2;

    let y1 = Vec2::Y * w1;
    let y2 = Vec2::Y * w2;

    let left = AABB::from_arbitrary(aabb.lower() - x1, aabb.top_left() - x2);
    let right = AABB::from_arbitrary(aabb.bottom_right() + x1, aabb.upper() + x2);

    let upper = AABB::from_arbitrary(aabb.top_left() + y1, aabb.upper() + y2);
    let lower = AABB::from_arbitrary(aabb.lower() - y1, aabb.bottom_right() - y2);

    for aabb in [upper, lower, left, right] {
        canvas.rect(ctx.w2c_aabb(aabb), color).z_index = 100.0;
    }
}

fn highlight_part(
    canvas: &mut Canvas,
    instance: &PartInstance,
    ctx: &impl CameraProjection,
    color: Srgba,
) {
    let wh = instance.dims_grid().as_ivec2();
    let p = instance.origin();
    let q = p + wh;
    let r = p + IVec2::X * wh.x;
    let s = p + IVec2::Y * wh.y;
    let aabb = aabb_for_part(p, instance.rotation(), instance.part());

    draw_highlight_box(canvas, aabb, ctx, color);

    for p in [p, q, r, s] {
        let p = p.as_vec2();
        draw_cross(&mut canvas.gizmos, ctx.w2c(p), 0.5 * ctx.scale(), color);
    }
}

impl Render for EditorContext {
    fn background_color(_state: &GameState) -> bevy::color::Srgba {
        GRAY.with_luminance(0.12)
    }

    fn ui(state: &GameState) -> Option<Tree<OnClick>> {
        use crate::ui::*;

        let vb = state.input.screen_bounds;
        if vb.span.x == 0.0 || vb.span.y == 0.0 {
            return None;
        }

        let top_bar = top_bar(state);
        let parts = part_selection(state);
        let layers = layer_selection(state);
        let vehicles = vehicle_selection(state);

        let other_buttons = other_buttons();
        let part_buttons = state
            .editor_context
            .selected_part()
            .map(|p| part_ui_layout(p));

        let right_column = Node::column(400)
            .invisible()
            .with_child(other_buttons)
            .with_child(part_buttons);

        let main_area = Node::grow()
            .invisible()
            .with_child(parts)
            .with_child(
                Node::fit()
                    .down()
                    .with_padding(0.0)
                    .invisible()
                    .with_child(layers),
            )
            .with_child(vehicles)
            .with_child(Node::grow().invisible())
            .with_child(right_column);

        let layout = Node::structural(vb.span.x, vb.span.y)
            .tight()
            .invisible()
            .down()
            .with_child(top_bar)
            .with_child(main_area);

        Some(Tree::new().with_layout(layout, Vec2::ZERO))
    }

    fn draw(canvas: &mut Canvas, state: &GameState) -> Option<()> {
        let ctx = &state.editor_context;
        draw_cross(&mut canvas.gizmos, ctx.w2c(Vec2::ZERO), 10.0, GRAY);

        match &ctx.cursor_state {
            CursorState::None | CursorState::Part(_) => {
                if let Some(p) = state.input.current() {
                    canvas.circle(p, 4.0, WHITE);
                }
            }
            CursorState::Pipes => {
                if let Some(p) = state.input.current() {
                    canvas.square(p, 6.0, PURPLE);
                }
            }
        }

        let radius = ctx.vehicle.bounding_radius();
        let bounds = ctx.vehicle.aabb();

        let filename = match &state.editor_context.filepath {
            Some(p) => format!("[{}]", p.display()),
            None => "[No file open]".to_string(),
        };

        let vehicle_info = vehicle_info(&ctx.vehicle);

        let info: String = [
            filename,
            format!("{} parts", state.editor_context.vehicle.parts().count()),
            format!("Rotation: {:?}", state.editor_context.rotation),
        ]
        .into_iter()
        .map(|s| format!("{s}\n"))
        .collect();

        let info = format!("{}{}", info, vehicle_info);

        let half_span = state.input.screen_bounds.span * 0.5;

        canvas.label(
            TextLabel::new(
                info.to_uppercase(),
                Vec2::new(half_span.x - 600.0, half_span.y - 400.0),
                0.7,
            )
            .with_anchor_left(),
        );

        // axes
        {
            let length = bounds.span.x * PIXELS_PER_METER * 1.5;
            let width = bounds.span.y * PIXELS_PER_METER * 1.5;
            let o = ctx.w2c(Vec2::ZERO);
            let p = ctx.w2c(Vec2::X * length);
            let q = ctx.w2c(Vec2::Y * width);
            canvas.gizmos.line_2d(o, p, RED.with_alpha(0.3));
            canvas.gizmos.line_2d(o, q, GREEN.with_alpha(0.3));
        }

        if let Some((p, current_part)) = Self::current_part_and_cursor_position(state) {
            let current_pixels = occupied_pixels(p, ctx.rotation, &current_part);

            let mut visited_parts = HashSet::new();

            if let Some(occ) = ctx.occupied.get(&current_part.layer()) {
                for q in &current_pixels {
                    if let Some(idx) = occ.get(q) {
                        if visited_parts.contains(idx) {
                            continue;
                        }
                        visited_parts.insert(*idx);
                        if let Some(instance) = ctx.vehicle.get_part_by_index(*idx) {
                            highlight_part(canvas, instance, ctx, RED.with_alpha(0.6));
                        }
                    }
                }
            }
        }

        if ctx.show_vehicle_info {
            draw_aabb(
                &mut canvas.gizmos,
                ctx.w2c_aabb(bounds.scale(PIXELS_PER_METER)),
                TEAL.with_alpha(0.1),
            );

            draw_circle(
                &mut canvas.gizmos,
                ctx.w2c(Vec2::ZERO),
                radius * ctx.scale() * PIXELS_PER_METER,
                RED.with_alpha(0.1),
            );

            draw_vehicle(
                canvas,
                &ctx.vehicle,
                ctx.w2c(Vec2::ZERO),
                ctx.scale() * PIXELS_PER_METER,
                0.0,
            );

            // COM
            let com = ctx.vehicle.center_of_mass() * PIXELS_PER_METER;
            draw_circle(&mut canvas.gizmos, ctx.w2c(com), 7.0, ORANGE);
            draw_x(&mut canvas.gizmos, ctx.w2c(com), 7.0, WHITE);

            // thrust envelope
            for (rcs, color) in [(false, RED), (true, BLUE)] {
                let positions: Vec<_> = linspace(0.0, 2.0 * PI, 200)
                    .into_iter()
                    .map(|a| {
                        let thrust: f32 = ctx.vehicle.max_thrust_along_heading(a, rcs);
                        let r = (1.0 + thrust.abs().sqrt() / 100.0)
                            * ctx.vehicle.bounding_radius()
                            * PIXELS_PER_METER;
                        ctx.w2c(rotate(Vec2::X * r, a))
                    })
                    .collect();
                canvas.gizmos.linestrip_2d(positions, color.with_alpha(0.6));
            }
        }

        for layer in enum_iterator::all::<PartLayer>() {
            for instance in ctx.vehicle.parts().filter(|p| p.part().layer() == layer) {
                let alpha = if ctx.is_layer_visible(instance.part().layer()) {
                    1.0
                } else {
                    0.02
                };
                let dims = instance.dims_grid();
                let sprite_dims = instance.part().dims();
                let center = ctx.w2c(instance.origin().as_vec2() + dims.as_vec2() / 2.0);
                let p = instance.percent_built();
                let sprite_name = instance.part().sprite_path();
                let sprite_name = if p == 1.0 {
                    sprite_name.to_string()
                } else {
                    let idx = (p * 10.0).floor() as i32;
                    format!("{}-building-{}", sprite_name, idx)
                };

                canvas
                    .sprite(
                        center,
                        instance.rotation().to_angle(),
                        sprite_name,
                        None,
                        sprite_dims.as_vec2() * ctx.scale(),
                    )
                    .set_color(WHITE.with_alpha(alpha));

                // if let Part::Tank(tank) = instance.part() {
                //     let name = tank.item().to_sprite_name();
                //     canvas.sprite(center, 0.0, name, None, Vec2::splat(100.0));
                // }

                // if p < 1.0 {
                //     let color = crate::generate_ship_sprites::diagram_color(instance.part());
                //     let aabb = AABB::new(center, dims.as_vec2() * ctx.scale() * (1.0 - p));
                //     canvas.rect(aabb, color /* .with_alpha(1.0 - p * 0.8) */);
                // }
            }
        }

        if let Some(cursor) = state.input.position(MouseButt::Hover, FrameId::Current) {
            let c = ctx.c2w(cursor);

            let discrete = IVec2::new(
                (c.x / 10.0).round() as i32 * 10,
                (c.y / 10.0).round() as i32 * 10,
            );

            for dx in (-100..=100).step_by(10) {
                for dy in (-100..=100).step_by(10) {
                    let s = IVec2::new(dx, dy);
                    let p = discrete - s;
                    let d = (s.length_squared() as f32).sqrt();
                    let alpha = 0.2 * (1.0 - d / 100.0);
                    if alpha > 0.01 {
                        draw_diamond(
                            &mut canvas.gizmos,
                            ctx.w2c(p.as_vec2()),
                            7.0,
                            GRAY.with_alpha(alpha),
                        );
                    }
                }
            }

            if Self::current_part_and_cursor_position(state).is_none() {
                if let Some((idx, instance)) = ctx.vehicle.get_part_at(vfloor(c), ctx.focus_layer) {
                    highlight_part(canvas, instance, ctx, TEAL.with_alpha(0.6));
                    for (other, other_instance) in ctx.vehicle.parts().enumerate() {
                        if ctx.vehicle.is_connected(idx, other) {
                            highlight_part(canvas, other_instance, ctx, YELLOW.with_alpha(0.4))
                        }
                    }
                }
            }
        }

        if let Some(instance) = ctx.selected_part() {
            highlight_part(canvas, instance, ctx, GREEN.with_alpha(0.4));
            canvas.text(format!("{:#?}", instance), Vec2::new(300.0, 400.0), 0.6);
        }

        for pipe in ctx.vehicle.pipes() {
            let p = pipe.as_vec2();
            let q = (pipe + IVec2::ONE).as_vec2();
            let aabb = AABB::from_arbitrary(p, q);
            canvas.rect(ctx.w2c_aabb(aabb), PURPLE);
        }

        if let Some((p, current_part)) = Self::current_part_and_cursor_position(state) {
            let dims = pixel_dims_with_rotation(ctx.rotation, &current_part);
            let sprite_dims = current_part.dims();
            canvas.sprite(
                ctx.w2c(p.as_vec2() + dims.as_vec2() / 2.0),
                ctx.rotation.to_angle(),
                current_part.sprite_path().to_string(),
                None,
                sprite_dims.as_vec2() * ctx.scale(),
            );
        }

        for (group_id, group) in ctx.vehicle.conn_groups().enumerate() {
            let color = crate::sprites::hashable_to_color(&group_id);
            let mut points = Vec::new();
            for p in group.points() {
                points.push(ctx.w2c(p.as_vec2()));
            }
            let color: Srgba = color.into();
            for p in &points {
                for q in &points {
                    canvas.gizmos.line_2d(*p, *q, color);
                }
            }
            if let Some(bounds) = group.bounds() {
                draw_aabb(
                    &mut canvas.gizmos,
                    ctx.w2c_aabb(bounds),
                    color.with_alpha(0.5),
                );
            }
        }

        Some(())
    }
}

fn aabb_for_part(p: IVec2, rot: Rotation, part: &Part) -> AABB {
    let wh = pixel_dims_with_rotation(rot, part).as_ivec2();
    let q = p + wh;
    AABB::from_arbitrary(p.as_vec2(), q.as_vec2())
}

fn expandable_menu(text: &str, onclick: OnClick) -> Node<OnClick> {
    Node::structural(300, Size::Fit)
        .down()
        .with_color(UI_BACKGROUND_COLOR)
        .with_child(Node::button(text, onclick, Size::Grow, BUTTON_HEIGHT))
}

fn part_selection(state: &GameState) -> Node<OnClick> {
    let mut part_names: Vec<_> = state.part_database.keys().collect();
    part_names.sort();

    let mut n = expandable_menu("Parts", OnClick::TogglePartsMenuCollapsed);

    if !state.editor_context.parts_menu_collapsed {
        n.add_child(Node::hline());
        n.add_children(part_names.into_iter().map(|s| {
            let onclick = OnClick::SelectPart(s.clone());
            Node::button(s, onclick, Size::Grow, BUTTON_HEIGHT)
        }));
    }

    n
}

pub fn get_list_of_vehicles(state: &GameState) -> Option<Vec<(String, PathBuf)>> {
    let mut ret = vec![];
    if let Ok(paths) = std::fs::read_dir(&state.args.vehicle_dir()) {
        for path in paths {
            if let Ok(path) = path {
                let s = path.path().file_stem()?.to_string_lossy().to_string();
                ret.push((s, path.path()));
            }
        }
    }
    Some(ret)
}

fn vehicle_selection(state: &GameState) -> Node<OnClick> {
    let vehicles = get_list_of_vehicles(state).unwrap_or(vec![]);

    let mut n = expandable_menu("Vehicles", OnClick::ToggleVehiclesMenuCollapsed);

    if !state.editor_context.vehicles_menu_collapsed {
        n.add_child(Node::hline());
        n.add_children(vehicles.into_iter().map(|(name, path)| {
            let onclick = OnClick::LoadVehicle(path);
            Node::button(name, onclick, Size::Grow, BUTTON_HEIGHT)
        }));
    }

    n
}

fn other_buttons() -> Node<OnClick> {
    let rotate = Node::button("Rotate", OnClick::RotateCraft, Size::Grow, BUTTON_HEIGHT);
    let normalize = Node::button(
        "Normalize",
        OnClick::NormalizeCraft,
        Size::Grow,
        BUTTON_HEIGHT,
    );
    let write = Node::button(
        "To Image",
        OnClick::WriteVehicleToImage,
        Size::Grow,
        BUTTON_HEIGHT,
    );
    let new_button = Node::button("New", OnClick::OpenNewCraft, Size::Grow, BUTTON_HEIGHT);

    let toggle_info = Node::button(
        "Info",
        OnClick::ToggleVehicleInfo,
        Size::Grow,
        BUTTON_HEIGHT,
    );

    let write_to_ownship = Node::button(
        "Modify Ownship",
        OnClick::WriteToOwnship,
        Size::Grow,
        BUTTON_HEIGHT,
    );

    Node::structural(Size::Grow, Size::Fit)
        .with_color(UI_BACKGROUND_COLOR)
        .down()
        .with_child(new_button)
        .with_child(rotate)
        .with_child(normalize)
        .with_child(write)
        .with_child(toggle_info)
        .with_child(write_to_ownship)
}

fn layer_selection(state: &GameState) -> Node<OnClick> {
    let mut n = expandable_menu("Layers", OnClick::ToggleLayersMenuCollapsed);

    if !state.editor_context.layers_menu_collapsed {
        n.add_child(Node::hline());
        n.add_children(enum_iterator::all::<PartLayer>().into_iter().map(|p| {
            let s = format!("{:?}", p);
            let onclick = OnClick::ToggleLayer(p);
            let mut n = Node::button(s, onclick, Size::Grow, BUTTON_HEIGHT);
            if !state.editor_context.is_layer_visible(p) {
                n = n.with_color(GRAY.to_f32_array());
            }
            n
        }));
    }

    n
}

impl CameraProjection for EditorContext {
    fn origin(&self) -> Vec2 {
        self.camera.origin()
    }

    fn scale(&self) -> f32 {
        self.camera.scale()
    }
}

fn process_part_mode(state: &mut GameState) {
    if let Some(p) = state.input.on_frame(MouseButt::Left, FrameId::Down) {
        let p = state.editor_context.c2w(p);
        if let Some((index, ..)) = state
            .editor_context
            .vehicle
            .get_part_at(vfloor(p), state.editor_context.focus_layer)
        {
            state.editor_context.selected_part = Some(index)
        } else {
            state.editor_context.selected_part = None;
        }
    }

    if let Some(_) = state.input.position(MouseButt::Left, FrameId::Current) {
        if let Some((p, part)) = EditorContext::current_part_and_cursor_position(state) {
            state.editor_context.try_place_part(p, part);
        }
    } else if let Some(p) = state.input.position(MouseButt::Right, FrameId::Current) {
        let p = vfloor(state.editor_context.c2w(p));
        state.editor_context.remove_part_at(p);
    } else if state.input.just_pressed(KeyCode::KeyQ) {
        if state.editor_context.cursor_state.current_part().is_some() {
            state.editor_context.cursor_state = CursorState::None;
        } else if let Some(p) = state.input.position(MouseButt::Hover, FrameId::Current) {
            let p = vfloor(state.editor_context.c2w(p));
            if let Some(instance) = state.editor_context.get_part_at(p).cloned() {
                state.editor_context.rotation = instance.rotation();
                state.editor_context.cursor_state = CursorState::Part(instance.part().clone());
            } else {
                state.editor_context.cursor_state = CursorState::None;
            }
        }
    }

    if state.input.just_pressed(KeyCode::KeyR) {
        state.editor_context.rotation = enum_iterator::next_cycle(&state.editor_context.rotation);
    }
}

impl EditorContext {
    pub fn step(state: &mut GameState, dt: f32) {
        let is_hovering = state.is_hovering_over_ui();

        let ctx = &mut state.editor_context;

        ctx.camera.update(dt, &state.input);

        ctx.vehicle.build_once();

        for tank in ctx.vehicle.tanks_mut() {
            tank.put(Mass::kilograms(10));
        }

        if is_hovering {
            return;
        }

        if state.input.just_pressed(KeyCode::KeyP) {
            ctx.cursor_state.toggle_logistics();
        }

        match ctx.cursor_state {
            CursorState::Pipes => {
                if let Some(p) = state.input.on_frame(MouseButt::Left, FrameId::Current) {
                    let p = ctx.c2w(p);
                    let p = vfloor(p);
                    ctx.vehicle.add_pipe(p);
                } else if let Some(p) = state.input.on_frame(MouseButt::Right, FrameId::Current) {
                    let p = ctx.c2w(p);
                    let p = vfloor(p);
                    ctx.vehicle.remove_pipe(p);
                }
            }
            _ => {
                process_part_mode(state);
            }
        }
    }
}

pub fn write_image_to_file(vehicle: &Vehicle, ctx: &ProgramContext, name: &str) -> Option<()> {
    let outpath: String = format!("/tmp/{}.png", name);
    println!("Writing vehicle {} to path {}", vehicle.name(), outpath);
    let img = crate::generate_ship_sprites::generate_image(vehicle, &ctx.parts_dir(), false)?;
    img.save(outpath).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_vehicle_to_image() {
        let dir = project_root::get_project_root()
            .expect("Expected project root to be discoverable")
            .join("assets");

        dbg!(&dir);

        let args = ProgramContext::new(dir);

        let g = GameState::new(args.clone());

        let vehicles = get_list_of_vehicles(&g).expect("Expected list of vehicles");
        dbg!(vehicles);

        for name in ["remora", "lander", "pollux", "manta", "spacestation"] {
            let vehicle = g.get_vehicle_by_model(name).expect("Expected a vehicle");
            write_image_to_file(&vehicle, &args, name);
        }
    }
}
