use crate::args::ProgramContext;
use crate::canvas::Canvas;
use crate::debug_console::DebugConsole;
use crate::generate_ship_sprites::*;
use crate::input::{FrameId, InputState, MouseButt};
use crate::names::*;
use crate::notifications::*;
use crate::onclick::OnClick;
use crate::scenes::{
    CursorMode, DockingContext, EditorContext, MainMenuContext, OrbitalContext, Render, Scene,
    SceneType, StaticSpriteDescriptor, SurfaceContext, TelescopeContext, TextLabel,
};
use crate::settings::*;
use crate::sim_rate::SimRate;
use crate::sounds::*;
use crate::ui::InteractionEvent;
use bevy::color::palettes::css::*;
use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::smaa::Smaa;
use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use bevy::window::WindowMode;
use clap::Parser;
use enum_iterator::next_cycle;
use image::DynamicImage;
use layout::layout::Tree;
use starling::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct GamePlugin;

fn gamepad_usage_system(gamepads: Query<(&Name, &Gamepad)>, mut state: ResMut<GameState>) {
    for (_name, gamepad) in &gamepads {
        for button in gamepad.get_just_pressed() {
            dbg!((button, state.cursor_position, true));
        }
        for button in gamepad.get_just_released() {
            dbg!((button, state.cursor_position, false));
        }

        if gamepad.just_pressed(GamepadButton::South) {
            let wb = state.input.screen_bounds.span;
            let n = state.ui.at(state.cursor_position, wb);
            if let Some(event) = n
                .map(|n| n.is_enabled().then(|| n.on_click()))
                .flatten()
                .flatten()
                .cloned()
            {
                state.on_button_event(event);
            }
        }

        let speed = state.settings.controller_cursor_speed;

        if let Some(left_stick_x) = gamepad.get(GamepadAxis::LeftStickX) {
            state.cursor_position += Vec2::X * left_stick_x * speed;
        }
        if let Some(left_stick_y) = gamepad.get(GamepadAxis::LeftStickY) {
            state.cursor_position += Vec2::Y * left_stick_y * speed;
        }
    }
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_system);

        app.insert_resource(Time::<Fixed>::from_duration(
            PHYSICS_CONSTANT_DELTA_TIME.to_duration(),
        ));

        app.add_systems(
            Update,
            (
                crate::keybindings::keyboard_input,
                crate::input::update_input_state,
                on_render_tick,
                crate::drawing::draw_game_state,
                crate::sprites::update_static_sprites,
                crate::sprites::update_background_color,
                gamepad_usage_system,
            )
                .chain(),
        );

        app.add_systems(
            FixedUpdate,
            (
                handle_interactions,
                // physics
                on_game_tick,
                // rendering
                crate::ui::do_text_labels,
                crate::sounds::sound_system,
            )
                .chain(),
        );
    }
}

#[derive(Component, Debug)]
pub struct BackgroundCamera;

fn init_system(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let args = match ProgramContext::try_parse() {
        Ok(args) => args,
        Err(e) => {
            _ = e.print();
            ProgramContext::default()
        }
    };

    let mut g = GameState::new(args);

    g.load_sprites(&mut images);

    commands.insert_resource(g);
    commands.spawn((
        Camera2d,
        Camera {
            hdr: true,
            order: 0,
            clear_color: ClearColorConfig::Custom(BLACK.with_alpha(0.0).into()),
            ..default()
        },
        Bloom {
            intensity: 0.2,
            ..Bloom::OLD_SCHOOL
        },
        BackgroundCamera,
        Smaa::default(),
        RenderLayers::layer(0),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            hdr: true,
            order: 1,
            clear_color: ClearColorConfig::Custom(BLACK.with_alpha(0.0).into()),
            ..default()
        },
        RenderLayers::layer(1),
    ));
}

#[derive(Resource)]
pub struct GameState {
    pub game_ticks: u64,
    pub render_ticks: u64,

    pub cursor_position: Vec2,

    pub settings: Settings,

    pub sounds: EnvironmentSounds,

    /// Contains all states related to window size, mouse clicks and positions,
    /// and button presses and holds.
    pub input: InputState,

    pub console: DebugConsole,

    /// Contains CLI arguments
    pub args: ProgramContext,

    /// All the game entities and logic therein. This should be able to run
    /// autonomously without any user input with on_sim_tick.
    pub universe: Universe,

    /// Stores information and provides an API for interacting with the simulation
    /// from the perspective of a global solar/planetary system view.
    ///
    /// Additional information allows the user to select spacecraft and
    /// direct them to particular orbits, or manually pilot them.
    pub orbital_context: OrbitalContext,

    pub telescope_context: TelescopeContext,

    pub docking_context: DockingContext,

    pub editor_context: EditorContext,

    pub surface_context: SurfaceContext,

    /// Wall clock, i.e. time since program began.
    pub wall_time: Nanotime,

    pub physics_duration: Nanotime,
    pub universe_ticks_per_game_tick: SimRate,
    pub paused: bool,
    pub exec_time: std::time::Duration,
    pub actual_universe_ticks_per_game_tick: u32,
    pub using_batch_mode: bool,

    /// Map of names to parts to their definitions. Loaded from
    /// the assets/parts directory
    pub part_database: HashMap<String, PartPrototype>,

    pub starfield: Vec<(Vec3, Srgba, f32, f32)>,
    pub pinned: HashSet<EntityId>,

    pub scenes: Vec<Scene>,
    pub current_scene_idx: usize,
    pub current_orbit: Option<usize>,

    pub ui: Tree<OnClick>,

    pub notifications: Vec<Notification>,

    pub is_exit_prompt: bool,

    pub text_labels: Vec<TextLabel>,
    pub sprites: Vec<StaticSpriteDescriptor>,
    pub image_handles: HashMap<String, (Handle<Image>, UVec2)>,

    pub vehicle_names: Vec<String>,
}

fn generate_starfield() -> Vec<(Vec3, Srgba, f32, f32)> {
    (0..1000)
        .map(|_| {
            let s = rand(0.0, 2.0);
            let color = if s < 1.0 {
                RED.mix(&YELLOW, s)
            } else {
                WHITE.mix(&TEAL, s - 1.0)
            };
            (
                randvec3(1000.0, 12000.0),
                color,
                rand(3.0, 9.0),
                rand(700.0, 1900.0),
            )
        })
        .collect()
}

impl GameState {
    pub fn new(args: ProgramContext) -> Self {
        let planets = default_example();

        let part_database = match load_parts_from_dir(&args.parts_dir()) {
            Ok(d) => d,
            Err(s) => {
                error!("Failed to load parts: {s}");
                HashMap::new()
            }
        };

        let settings = match load_settings_from_file(&args.settings_path()) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to load settings: {e}");
                Settings::default()
            }
        };

        let mut sounds = EnvironmentSounds::new();
        sounds.play_loop("building.ogg", 0.1);

        let vehicle_names = match load_names_from_file(&args.names_path()) {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to load vehicle names: {e}");
                Vec::new()
            }
        };

        let mut g = GameState {
            render_ticks: 0,
            game_ticks: 0,
            cursor_position: Vec2::ZERO,
            settings,
            sounds,
            input: InputState::default(),
            args: args.clone(),
            universe: Universe::new(planets.clone()),
            console: DebugConsole::new(),
            orbital_context: OrbitalContext::new(EntityId(0)),
            telescope_context: TelescopeContext::new(),
            docking_context: DockingContext::new(),
            editor_context: EditorContext::new(),
            surface_context: SurfaceContext::default(),
            wall_time: Nanotime::zero(),
            physics_duration: Nanotime::days(7),
            universe_ticks_per_game_tick: SimRate::RealTime,
            actual_universe_ticks_per_game_tick: 0,
            using_batch_mode: false,
            paused: false,
            exec_time: std::time::Duration::new(0, 0),
            part_database,
            starfield: generate_starfield(),
            pinned: HashSet::new(),
            scenes: vec![
                Scene::main_menu(),
                Scene::orbital(),
                Scene::telescope(),
                Scene::editor(),
                Scene::surface(),
            ],
            current_scene_idx: 0,
            current_orbit: None,
            ui: Tree::new(),
            notifications: Vec::new(),
            is_exit_prompt: false,
            text_labels: Vec::new(),
            sprites: Vec::new(),
            image_handles: HashMap::new(),
            vehicle_names,
        };

        let surface_id = g
            .universe
            .landing_sites
            .iter()
            .next()
            .map(|(e, _)| *e)
            .unwrap_or(EntityId(0));

        g.surface_context.current_surface = surface_id;

        for model in ["remora", "icecream", "lander", "remora", "pollux"] {
            if let Some(v) = g.get_vehicle_by_model(model) {
                g.universe.add_surface_vehicle(
                    surface_id,
                    v,
                    Vec2::Y * 100.0 + randvec(20.0, 50.0),
                );
            }
        }

        let t = g.universe.stamp();

        let get_random_orbit = |pid: EntityId| {
            let r1 = rand(11000.0, 40000.0) as f64;
            let r2 = rand(11000.0, 40000.0) as f64;
            let argp = rand(0.0, 2.0 * PI) as f64;
            let body = planets.lookup(pid, t)?.0;
            let orbit = SparseOrbit::new(r1.max(r2), r1.min(r2), argp, body, t, false)?;
            Some(GlobalOrbit(pid, orbit))
        };

        for _ in 0..30 {
            let vehicle = g.get_random_vehicle();
            let orbit = get_random_orbit(EntityId(0));
            if let (Some(orbit), Some(vehicle)) = (orbit, vehicle) {
                g.spawn_with_random_perturbance(orbit, vehicle);
            }
        }

        for _ in 0..40 {
            let vehicle = g.get_random_vehicle();
            let orbit = get_random_orbit(EntityId(1));
            if let (Some(orbit), Some(vehicle)) = (orbit, vehicle) {
                g.spawn_with_random_perturbance(orbit, vehicle);
            }
        }

        for (id, _) in &g.universe.orbital_vehicles {
            if g.pinned.len() < 5 {
                g.pinned.insert(*id);
            }
        }

        g
    }

    pub fn load_sprites(&mut self, images: &mut Assets<Image>) {
        let mut handles = HashMap::new();

        for (name, _) in &self.part_database {
            let path = self.args.part_sprite_path(name);
            if let Some(img) = crate::generate_ship_sprites::read_image(Path::new(&path)) {
                let mut img = Image::from_dynamic(
                    DynamicImage::ImageRgba8(img),
                    true,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                );
                img.sampler = bevy::image::ImageSampler::nearest();
                let dims = img.size();
                let handle = images.add(img.clone());
                handles.insert(name.to_string(), (handle.clone(), dims));

                for pct in (0..=9).rev() {
                    for w in 0..img.width() {
                        for h in 0..img.height() {
                            if rand(0.0, 1.0) < 0.5 {
                                if let Some(pixel) = img.pixel_bytes_mut(UVec3::new(w, h, 0)) {
                                    pixel[3] = pixel[3].min(10);
                                    pixel[2] = 255;
                                }
                            }
                        }
                    }
                    let handle = images.add(img.clone());
                    handles.insert(format!("{}-building-{}", name, pct), (handle, dims));
                }
            } else {
                error!("Failed to load sprite for part {}", name);
            }
        }

        for name in [
            "cloud",
            "diamond",
            // items
            "item-bread",
            "item-corn",
            "item-h2",
            "item-ice",
            "item-methane",
            "item-o2",
            "item-potato",
            "item-wheat",
            "Earth",
            "Luna",
            "Asteroid",
            "conbot",
            "low-fuel",
            "low-fuel-dim",
            "radar",
            "radar-dim",
            "ctrl",
            "ctrl-dim",
            "shipscope",
        ] {
            let path = self.args.install_dir.join(format!("{}.png", name));
            if let Some(img) = crate::generate_ship_sprites::read_image(&path) {
                let mut img = Image::from_dynamic(
                    DynamicImage::ImageRgba8(img),
                    true,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                );
                img.sampler = bevy::image::ImageSampler::nearest();
                let dims = img.size();
                let handle = images.add(img);
                handles.insert(name.into(), (handle, dims));
            } else {
                error!("Failed to load sprite: {}", path.display());
            }
        }

        let image = generate_error_sprite();
        let dims = image.size();
        let handle = images.add(image);
        handles.insert("error".to_string(), (handle, dims));

        self.image_handles = handles;
    }
}

impl Render for GameState {
    fn background_color(state: &GameState) -> Srgba {
        match state.current_scene().kind() {
            SceneType::Orbital => OrbitalContext::background_color(state),
            SceneType::Editor => EditorContext::background_color(state),
            SceneType::Telescope => TelescopeContext::background_color(state),
            SceneType::DockingView => DockingContext::background_color(state),
            SceneType::MainMenu => BLACK,
            SceneType::Surface => SurfaceContext::background_color(state),
        }
    }

    fn ui(state: &GameState) -> Option<Tree<OnClick>> {
        match state.current_scene().kind() {
            SceneType::Surface => SurfaceContext::ui(state),
            _ => None,
        }
    }

    fn draw(canvas: &mut Canvas, state: &GameState) -> Option<()> {
        // BOOKMARK debug info

        crate::drawing::draw_x(&mut canvas.gizmos, state.cursor_position, 30.0, WHITE);

        #[allow(unused)]
        let debug_info: String = [
            format!("Wall time: {}", state.wall_time),
            format!("Universe time: {}", state.universe.stamp()),
            format!(
                "Ideal universe ticks per game tick: {}",
                state.universe_ticks_per_game_tick.as_ticks(),
            ),
            format!(
                "Actual universe ticks per game tick: {}",
                state.actual_universe_ticks_per_game_tick
            ),
            format!("Render ticks: {}", state.render_ticks),
            format!("Game ticks: {}", state.game_ticks),
            format!("Universe ticks: {}", state.universe.ticks()),
            format!("Execution time: {} us", state.exec_time.as_micros()),
        ]
        .iter()
        .map(|e| format!("{}\n", e))
        .collect();

        // canvas
        //     .text(debug_info, Vec2::splat(-300.0), 0.7)
        //     .anchor_left();

        match state.current_scene().kind() {
            SceneType::Orbital => OrbitalContext::draw(canvas, state),
            SceneType::Editor => EditorContext::draw(canvas, state),
            SceneType::Telescope => TelescopeContext::draw(canvas, state),
            SceneType::DockingView => DockingContext::draw(canvas, state),
            SceneType::MainMenu => MainMenuContext::draw(canvas, state),
            SceneType::Surface => SurfaceContext::draw(canvas, state),
        }
    }
}

fn keyboard_control_law(input: &InputState) -> Option<VehicleControl> {
    let mut ctrl = VehicleControl::NULLOPT;

    let docking_mode = input.is_pressed(KeyCode::ControlLeft);

    if docking_mode {
        ctrl.plus_x.throttle = input.is_pressed(KeyCode::ArrowUp) as u8 as f32;
        ctrl.plus_y.throttle = input.is_pressed(KeyCode::ArrowLeft) as u8 as f32;
        ctrl.neg_x.throttle = input.is_pressed(KeyCode::ArrowDown) as u8 as f32;
        ctrl.neg_y.throttle = input.is_pressed(KeyCode::ArrowRight) as u8 as f32;
    } else {
        ctrl.plus_x.throttle = input.is_pressed(KeyCode::ArrowUp) as u8 as f32;
        ctrl.neg_x.throttle = input.is_pressed(KeyCode::ArrowDown) as u8 as f32;

        ctrl.attitude = if input.is_pressed(KeyCode::ArrowLeft) {
            10.0
        } else if input.is_pressed(KeyCode::ArrowRight) {
            -10.0
        } else {
            0.0
        };
    }

    ctrl.plus_x.use_rcs = docking_mode;
    ctrl.plus_y.use_rcs = docking_mode;
    ctrl.neg_x.use_rcs = docking_mode;
    ctrl.neg_y.use_rcs = docking_mode;

    Some(ctrl)
}

impl GameState {
    pub fn reload(&mut self) {
        *self = GameState::new(self.args.clone());
    }

    pub fn set_piloting(&mut self, id: EntityId) {
        self.orbital_context.piloting = Some(id);
    }

    pub fn set_targeting(&mut self, id: EntityId) {
        self.orbital_context.targeting = Some(id);
    }

    pub fn current_scene(&self) -> &Scene {
        &self.scenes[self.current_scene_idx]
    }

    pub fn is_tracked(&self, id: EntityId) -> bool {
        self.orbital_context.selected.contains(&id)
    }

    pub fn toggle_group(&mut self, gid: EntityId) {
        // - if any of the orbiters in the group are not selected,
        //   select all of them
        // - if all of them are already selected, deselect all of them

        let members = self.universe.get_group_members(gid);

        let all_selected = members.iter().all(|id| self.is_tracked(*id));

        for id in members {
            if all_selected {
                self.orbital_context.selected.remove(&id);
            } else {
                self.orbital_context.selected.insert(id);
            }
        }
    }

    pub fn disband_group(&mut self, gid: EntityId) {
        self.universe.constellations.retain(|_, g| *g != gid);
    }

    pub fn create_group(&mut self, gid: EntityId) {
        for id in &self.orbital_context.selected {
            self.universe.constellations.insert(*id, gid.clone());
        }
    }

    pub fn get_vehicle_by_model(&self, name: &str) -> Option<Vehicle> {
        let vehicles = crate::scenes::get_list_of_vehicles(self)?;

        if vehicles.is_empty() {
            return None;
        }

        let (_, path) = vehicles.iter().find(|(model, _)| model == name)?;

        let name = get_random_ship_name(&self.vehicle_names);

        let mut vehicle = load_vehicle(path, name, &self.part_database).ok()?;

        vehicle.build_all();

        Some(vehicle)
    }

    pub fn planned_maneuvers(&self, after: Nanotime) -> Vec<(EntityId, Nanotime, Vec2)> {
        let mut dvs = vec![];
        for (id, ov) in &self.universe.orbital_vehicles {
            if let Some(plan) = ov.controller.plan() {
                for (stamp, impulse) in plan.future_dvs(after) {
                    dvs.push((*id, stamp, impulse));
                }
            }
        }
        dvs.sort_by_key(|(_, t, _)| t.inner());
        dvs
    }

    pub fn measuring_tape(&self) -> Option<(Vec2, Vec2, Vec2)> {
        if self.orbital_context.cursor_mode != CursorMode::MeasuringTape {
            return None;
        }

        OrbitalContext::measuring_tape(self)
    }

    pub fn protractor(&self) -> Option<(Vec2, Vec2, Option<Vec2>)> {
        if self.orbital_context.cursor_mode != CursorMode::Protractor {
            return None;
        }

        OrbitalContext::protractor(self)
    }

    pub fn left_cursor_orbit(&self) -> Option<GlobalOrbit> {
        OrbitalContext::left_cursor_orbit(self)
    }

    pub fn cursor_orbit_if_mode(&self) -> Option<GlobalOrbit> {
        if self.orbital_context.cursor_mode == CursorMode::AddOrbit {
            self.left_cursor_orbit()
        } else {
            None
        }
    }

    pub fn piloting(&self) -> Option<EntityId> {
        self.orbital_context.piloting
    }

    pub fn targeting(&self) -> Option<EntityId> {
        self.orbital_context.targeting
    }

    pub fn get_orbit(&self, id: EntityId) -> Option<GlobalOrbit> {
        let lup = self.universe.lup_orbiter(id, self.universe.stamp())?;
        let orbiter = lup.orbiter()?;
        let prop = orbiter.propagator_at(self.universe.stamp())?;
        Some(prop.orbit)
    }

    pub fn spawn_with_random_perturbance(
        &mut self,
        global: GlobalOrbit,
        vehicle: Vehicle,
    ) -> Option<()> {
        let GlobalOrbit(parent, orbit) = global;
        let pv_local = orbit.pv(self.universe.stamp()).ok()?;
        let perturb = PV::from_f64(
            randvec(
                pv_local.pos_f32().length() * 0.005,
                pv_local.pos_f32().length() * 0.02,
            ),
            randvec(
                pv_local.vel_f32().length() * 0.005,
                pv_local.vel_f32().length() * 0.02,
            ),
        );
        let orbit = SparseOrbit::from_pv(pv_local + perturb, orbit.body, self.universe.stamp())?;
        self.universe
            .add_orbital_vehicle(vehicle, GlobalOrbit(parent, orbit));
        Some(())
    }

    pub fn spawn_new(&mut self) -> Option<()> {
        let orbit = self.cursor_orbit_if_mode()?;
        let vehicle = self.get_random_vehicle()?;
        self.spawn_with_random_perturbance(orbit, vehicle)
    }

    pub fn delete_orbiter(&mut self, id: EntityId) -> Option<()> {
        let lup = self.universe.lup_orbiter(id, self.universe.stamp())?;
        let _orbiter = lup.orbiter()?;
        let parent = lup.parent(self.universe.stamp())?;
        let pv = lup.pv().pos_f32();
        let plup = self.universe.lup_planet(parent, self.universe.stamp())?;
        let pvp = plup.pv().pos_f32();
        let pvl = pv - pvp;
        self.universe.orbital_vehicles.remove(&id)?;
        self.notify(
            ObjectId::Planet(parent),
            NotificationType::OrbiterDeleted(id),
            pvl,
        );
        Some(())
    }

    pub fn delete_objects(&mut self) {
        self.orbital_context
            .selected
            .clone()
            .into_iter()
            .for_each(|id| {
                self.delete_orbiter(id);
            });
    }

    pub fn current_orbit(&self) -> Option<&GlobalOrbit> {
        self.orbital_context.queued_orbits.get(self.current_orbit?)
    }

    pub fn commit_mission(&mut self) -> Option<()> {
        let orbit = self.current_orbit()?.clone();
        self.command_selected(&orbit);
        Some(())
    }

    pub fn impulsive_burn(&mut self, id: EntityId, stamp: Nanotime, dv: Vec2) -> Option<()> {
        let orbiter = &mut self.universe.orbital_vehicles.get_mut(&id)?.orbiter;
        orbiter.try_impulsive_burn(stamp, dv)?;
        Some(())
    }

    pub fn swap_ownship_target(&mut self) {
        let tmp = self.orbital_context.targeting;
        self.orbital_context.targeting = self.orbital_context.piloting;
        self.orbital_context.piloting = tmp;
    }

    pub fn write_editor_to_ownship(&mut self) -> Option<()> {
        let id = match self.piloting() {
            Some(p) => p,
            None => {
                self.notice("No ownship to write to");
                return None;
            }
        };

        let vehicle = match self.universe.orbital_vehicles.get_mut(&id) {
            Some(v) => &mut v.vehicle,
            None => {
                self.notice(format!("Failed to find vehicle for id {}", id));
                return None;
            }
        };

        let new_vehicle = self.editor_context.vehicle.clone();

        let old_title = vehicle.name().to_string();
        let new_title = new_vehicle.name().to_string();

        *vehicle = new_vehicle;

        self.notice(format!(
            "Successfully overwrite vehicle {}, \"{}\" -> \"{}\"",
            id, old_title, new_title
        ));

        Some(())
    }

    pub fn command_selected(&mut self, next: &GlobalOrbit) {
        if self.orbital_context.selected.is_empty() {
            return;
        }
        self.notice(format!(
            "Commanding {} orbiters to {}",
            self.orbital_context.selected.len(),
            next,
        ));
        for id in self.orbital_context.selected.clone() {
            self.command(id, next);
        }
    }

    pub fn release_selected(&mut self) {
        let tracks = self.orbital_context.selected.clone();
        for id in tracks {
            if let Some(ov) = self.universe.orbital_vehicles.get_mut(&id) {
                ov.controller.clear();
            }
        }
    }

    pub fn command(&mut self, id: EntityId, next: &GlobalOrbit) -> Option<()> {
        let t = self.universe.stamp();
        let ov = self.universe.orbital_vehicles.get_mut(&id)?;
        if !ov.vehicle.is_controllable() {
            self.notify(
                ObjectId::Orbiter(id),
                NotificationType::NotControllable(id),
                None,
            );
            return None;
        }

        match ov.controller.set_destination(*next, t) {
            Err(e) => {
                dbg!(e);
            }
            _ => (),
        }

        Some(())
    }

    pub fn notice(&mut self, s: impl Into<String>) {
        let s = s.into();
        info!("Notice: {s}");
        self.console.log(s);
    }

    pub fn notify(
        &mut self,
        parent: impl Into<Option<ObjectId>>,
        kind: NotificationType,
        offset: impl Into<Option<Vec2>>,
    ) {
        let notif = Notification {
            parent: parent.into(),
            offset: offset.into().unwrap_or(Vec2::ZERO),
            jitter: Vec2::ZERO,
            sim_time: self.universe.stamp(),
            wall_time: self.wall_time,
            extra_time: Nanotime::secs_f32(rand(0.0, 1.0)),
            kind,
        };

        if self.notifications.iter().any(|e| notif.is_duplicate(e)) {
            return;
        }

        self.notifications.push(notif);
    }

    pub fn light_source(&self) -> Vec2 {
        let angle = 2.0 * PI * self.universe.stamp().to_secs() / Nanotime::days(365).to_secs();
        rotate(Vec2::X, angle + PI) * 1000000.0
    }

    pub fn save(&mut self) -> Option<()> {
        match self.current_scene().kind() {
            SceneType::Editor => EditorContext::save_to_file(self),
            _ => None,
        }
    }

    pub fn load(&mut self) -> Option<()> {
        match self.current_scene().kind() {
            SceneType::Editor => EditorContext::load_from_file(self),
            _ => None,
        }
    }

    pub fn on_button_event(&mut self, id: OnClick) -> Option<()> {
        self.sounds.play_once("button-up.ogg", 1.0);

        match id {
            OnClick::CurrentBody(id) => self.orbital_context.following = Some(ObjectId::Planet(id)),
            OnClick::Orbiter(id) => self.orbital_context.following = Some(ObjectId::Orbiter(id)),
            OnClick::ToggleDrawMode => {
                self.orbital_context.draw_mode = next_cycle(&self.orbital_context.draw_mode)
            }
            OnClick::ClearTracks => self.orbital_context.selected.clear(),
            OnClick::ClearOrbits => self.orbital_context.queued_orbits.clear(),
            OnClick::Group(gid) => self.toggle_group(gid),
            OnClick::CreateGroup => {
                // let id = self.ids.next();
                // self.create_group(id);
                println!("todo!");
            }
            OnClick::DisbandGroup(gid) => self.disband_group(gid),
            OnClick::CommitMission => {
                self.commit_mission();
            }
            OnClick::Exit => self.shutdown_with_prompt(),
            OnClick::SimSpeed(r) => {
                self.universe_ticks_per_game_tick = r;
            }
            OnClick::DeleteOrbit(i) => {
                self.orbital_context.queued_orbits.remove(i);
            }
            OnClick::TogglePause => self.paused = !self.paused,
            OnClick::GlobalOrbit(i) => {
                let orbit = self.orbital_context.queued_orbits.get(i)?;
                self.orbital_context.following = Some(ObjectId::Planet(orbit.0));
                self.current_orbit = Some(i);
            }
            OnClick::Nullopt => (),
            OnClick::Save => {
                self.save();
            }
            OnClick::Load => {
                self.load();
            }
            OnClick::CursorMode(c) => self.orbital_context.cursor_mode = c,
            OnClick::AutopilotingCount => {
                self.orbital_context.selected = self
                    .universe
                    .orbital_vehicles
                    .iter()
                    .filter_map(|(id, ov)| (!ov.controller.is_idle()).then(|| *id))
                    .collect();
            }
            OnClick::GoToScene(i) => {
                self.set_current_scene(i);
            }
            OnClick::ThrottleLevel(throttle) => {
                self.orbital_context.throttle = throttle;
                self.notice(format!("Throttle set to {:?}", throttle));
            }
            OnClick::ClearPilot => self.orbital_context.piloting = None,
            OnClick::ClearTarget => self.orbital_context.targeting = None,
            OnClick::SetPilot(p) => self.orbital_context.piloting = Some(p),
            OnClick::SetTarget(p) => self.orbital_context.targeting = Some(p),
            OnClick::SelectPart(name) => EditorContext::set_current_part(self, &name),
            OnClick::ToggleLayer(layer) => self.editor_context.toggle_layer(layer),
            OnClick::LoadVehicle(path) => _ = EditorContext::load_vehicle(&path, self),
            OnClick::ConfirmExitDialog => self.shutdown(),
            OnClick::DismissExitDialog => self.is_exit_prompt = false,
            OnClick::TogglePartsMenuCollapsed => {
                self.editor_context.parts_menu_collapsed = !self.editor_context.parts_menu_collapsed
            }
            OnClick::ToggleVehiclesMenuCollapsed => {
                self.editor_context.vehicles_menu_collapsed =
                    !self.editor_context.vehicles_menu_collapsed
            }
            OnClick::ToggleLayersMenuCollapsed => {
                self.editor_context.layers_menu_collapsed =
                    !self.editor_context.layers_menu_collapsed
            }
            OnClick::IncrementThrottle(d) => {
                self.orbital_context.throttle.increment(d);
            }
            OnClick::OpenNewCraft => {
                self.editor_context.new_craft();
            }
            OnClick::WriteVehicleToImage => {
                self.editor_context.write_image_to_file(&self.args);
            }
            OnClick::RotateCraft => {
                self.editor_context.rotate_craft();
            }
            OnClick::ToggleVehicleInfo => {
                self.editor_context.show_vehicle_info = !self.editor_context.show_vehicle_info;
            }
            OnClick::SendToSurface => {
                let mut vehicle = self.editor_context.vehicle.clone();
                vehicle.build_all();
                self.universe.add_surface_vehicle(
                    self.surface_context.current_surface,
                    vehicle,
                    Vec2::Y * 100.0,
                );
            }
            OnClick::NormalizeCraft => self.editor_context.normalize_coordinates(),
            OnClick::SwapOwnshipTarget => _ = self.swap_ownship_target(),
            OnClick::PinObject(id) => _ = self.pinned.insert(id),
            OnClick::UnpinObject(id) => _ = self.pinned.remove(&id),
            OnClick::ReloadGame => _ = self.reload(),
            OnClick::IncreaseGravity => {
                self.universe
                    .increase_gravity(self.surface_context.current_surface);
            }
            OnClick::DecreaseGravity => {
                self.universe
                    .decrease_gravity(self.surface_context.current_surface);
            }
            OnClick::IncreaseWind => {
                self.universe
                    .increase_wind(self.surface_context.current_surface);
            }
            OnClick::DecreaseWind => {
                self.universe
                    .decrease_wind(self.surface_context.current_surface);
            }
            OnClick::ToggleSurfaceSleep => {
                self.universe
                    .toggle_sleep(self.surface_context.current_surface);
            }
            OnClick::SetRecipe(id, recipe) => {
                if self.editor_context.vehicle.set_recipe(id, recipe) {
                    self.notice(format!("Set recipe for part {:?} to {:?}", id, recipe));
                } else {
                    self.notice(format!(
                        "Failed to set recipe for part {:?} to {:?}",
                        id, recipe
                    ));
                }
            }
            OnClick::ClearContents(id) => {
                if self.editor_context.vehicle.clear_contents(id) {
                    self.notice(format!("Cleared inventory for part {:?}", id));
                } else {
                    self.notice(format!("Failed to clear inventory for part {:?}", id));
                }
            }
            OnClick::GoToSurface(id) => {
                self.surface_context.current_surface = id;
                if let Some(idx) = self
                    .scenes
                    .iter()
                    .position(|s| *s.kind() == SceneType::Surface)
                {
                    self.current_scene_idx = idx;
                }
            }

            // BOOKMARK unhandled event
            _ => info!("Unhandled button event: {id:?}"),
        };

        Some(())
    }

    pub fn shutdown_with_prompt(&mut self) {
        if self.is_exit_prompt {
            self.shutdown()
        } else {
            self.is_exit_prompt = true;
        }
    }

    pub fn shutdown(&self) {
        // for a sensation of weightiness
        std::thread::sleep(core::time::Duration::from_millis(50));
        std::process::exit(0)
    }

    pub fn set_current_scene(&mut self, i: usize) -> Option<()> {
        if i == self.current_scene_idx {
            return Some(());
        }
        self.scenes.get(i)?;
        self.current_scene_idx = i;
        Some(())
    }

    pub fn get_random_vehicle(&self) -> Option<Vehicle> {
        let vehicles = crate::scenes::get_list_of_vehicles(self).unwrap_or(vec![]);

        if vehicles.is_empty() {
            return None;
        }

        let choice = randint(0, vehicles.len() as i32);
        let (_, path) = vehicles.get(choice as usize)?;

        let name = get_random_ship_name(&self.vehicle_names);

        let mut vehicle = load_vehicle(path, name, &self.part_database).ok()?;

        vehicle.build_all();

        Some(vehicle)
    }

    pub fn current_hover_ui(&self) -> Option<&OnClick> {
        let wb = self.input.screen_bounds.span;
        let p = self.input.position(MouseButt::Hover, FrameId::Current)?;
        self.ui.at(p, wb).map(|n| n.on_click()).flatten()
    }

    pub fn is_hovering_over_ui(&self) -> bool {
        let wb = self.input.screen_bounds.span;
        let p = match self.input.position(MouseButt::Hover, FrameId::Current) {
            Some(p) => p,
            None => return false,
        };
        self.ui.at(p, wb).map(|n| n.is_visible()).unwrap_or(false)
    }

    pub fn is_currently_left_clicked_on_ui(&self) -> bool {
        let wb = self.input.screen_bounds.span;
        if self
            .input
            .position(MouseButt::Left, FrameId::Current)
            .is_none()
        {
            return false;
        }
        let p = match self.input.position(MouseButt::Left, FrameId::Down) {
            Some(p) => p,
            None => return false,
        };
        self.ui.at(p, wb).map(|n| n.is_visible()).unwrap_or(false)
    }

    fn maybe_trigger_click_event(&mut self) -> Option<()> {
        use FrameId::*;
        use MouseButt::*;

        let wb = self.input.screen_bounds.span;

        let p = self.input.position(Left, Down)?;
        let q = self.input.position(Left, Up)?;
        let n = self.ui.at(p, wb)?;
        let m = self.ui.at(q, wb)?;
        if !n.is_enabled() || !m.is_enabled() {
            return None;
        }
        let n = n.on_click()?;
        let m = m.on_click()?;
        if n == m {
            self.on_button_event(n.clone());
        }
        return Some(());
    }

    fn handle_click_events(&mut self) {
        use FrameId::*;
        use MouseButt::*;

        if self.input.on_frame(Left, Up).is_some() {
            self.maybe_trigger_click_event();
        }
    }

    pub fn on_render_tick(&mut self) {
        self.render_ticks += 1;

        if self.console.is_active() {
            if let Some((decl, args)) = self.console.process_input(&mut self.input) {
                decl.execute(self, args);
            }
            return;
        }

        if self.input.is_pressed(KeyCode::ShiftLeft) && self.input.is_pressed(KeyCode::ControlLeft)
        {
            let delta = if self.input.just_pressed(KeyCode::Minus) {
                -1.0
            } else if self.input.just_pressed(KeyCode::Equal) {
                1.0
            } else {
                0.0
            };

            self.settings.ui_button_height =
                (self.settings.ui_button_height + delta).clamp(3.0, 40.0);
        }

        self.handle_click_events();

        let on_ui = self.is_hovering_over_ui();

        match self.current_scene().kind() {
            SceneType::DockingView => {
                self.docking_context.on_render_tick(&self.input);
            }
            SceneType::Editor => {
                EditorContext::on_render_tick(self);
            }
            SceneType::MainMenu => (),
            SceneType::Orbital => {
                self.orbital_context
                    .on_render_tick(on_ui, &self.input, &self.universe);
            }
            SceneType::Surface => {
                self.surface_context.on_render_tick(
                    &self.input,
                    &mut self.universe,
                    &mut self.sounds,
                );
            }
            SceneType::Telescope => {
                self.telescope_context.on_render_tick(&self.input);
            }
        }
    }

    pub fn on_game_tick(&mut self) {
        self.game_ticks += 1;

        let mut signals = ControlSignals::new();

        if let Some(id) = self.piloting() {
            if let Some(cmd) = keyboard_control_law(&self.input) {
                if cmd != VehicleControl::NULLOPT {
                    signals.piloting_commands.insert(id, cmd);
                }
            }
        }

        // BOOKMARK gameloop
        self.actual_universe_ticks_per_game_tick = 0;
        self.exec_time = std::time::Duration::ZERO;
        if !self.paused {
            (
                self.actual_universe_ticks_per_game_tick,
                self.exec_time,
                self.using_batch_mode,
            ) = self.universe.on_sim_ticks(
                self.universe_ticks_per_game_tick.as_ticks(),
                &signals,
                std::time::Duration::from_millis(10),
            )
        }

        let old_sim_time = self.universe.stamp();

        self.wall_time += PHYSICS_CONSTANT_DELTA_TIME;

        let s = self.universe.stamp();
        let d = self.physics_duration;

        let planets = self.universe.planets.clone();

        let mut man = self.planned_maneuvers(old_sim_time);
        while let Some((id, t, dv)) = man.first() {
            if s > *t {
                let perturb = 0.0 * randvec(0.01, 0.05);
                simulate(&mut self.universe.orbital_vehicles, &planets, *t, d);
                self.impulsive_burn(*id, *t, dv + perturb);
                self.notify(
                    ObjectId::Orbiter(*id),
                    NotificationType::OrbitChanged(*id),
                    None,
                );
            } else {
                break;
            }
            man.remove(0);
        }

        for (id, ri) in simulate(&mut self.universe.orbital_vehicles, &planets, s, d) {
            info!("{} {:?}", id, &ri);
            if let Some(pv) = ri.orbit.pv(ri.stamp).ok() {
                let notif = match ri.reason {
                    EventType::Collide(_) => NotificationType::OrbiterCrashed(id),
                    EventType::Encounter(_) => continue,
                    EventType::Escape(_) => NotificationType::OrbiterEscaped(id),
                    EventType::Impulse(_) => continue,
                    EventType::NumericalError => NotificationType::NumericalError(id),
                };
                self.notify(ObjectId::Planet(ri.parent), notif, pv.pos_f32());
            }
        }

        let mut track_list = self.orbital_context.selected.clone();
        track_list.retain(|o| {
            self.universe
                .lup_orbiter(*o, self.universe.stamp())
                .is_some()
        });
        self.orbital_context.selected = track_list;

        self.notifications.iter_mut().for_each(|n| n.jitter());

        self.notifications
            .retain(|n| n.wall_time + n.duration() > self.wall_time);

        match self.current_scene().kind() {
            SceneType::Orbital => {
                self.orbital_context.on_game_tick();
            }
            SceneType::Telescope => {
                self.telescope_context.on_game_tick();
            }
            SceneType::DockingView => {
                self.docking_context.on_game_tick();
            }
            SceneType::Editor => {
                EditorContext::on_game_tick(self);
            }
            SceneType::Surface => {
                SurfaceContext::on_game_tick(self);
            }
            _ => (),
        }
    }
}

fn on_game_tick(mut state: ResMut<GameState>, mut images: ResMut<Assets<Image>>) {
    state.on_game_tick();

    if state.image_handles.is_empty() {
        state.load_sprites(&mut images)
    }

    crate::generate_ship_sprites::proc_gen_ship_sprites(&mut state, &mut images);
    crate::generate_ship_sprites::proc_gen_terrain_sprites(&mut state, &mut images);
}

fn on_render_tick(mut state: ResMut<GameState>) {
    state.on_render_tick();
}

pub const MIN_SIM_SPEED: u32 = 0;
pub const MAX_SIM_SPEED: u32 = 1000000;

fn process_interaction(
    inter: &InteractionEvent,
    state: &mut GameState,
    window: &mut Window,
) -> Option<()> {
    match inter {
        InteractionEvent::Delete => state.delete_objects(),
        InteractionEvent::CommitMission => {
            state.commit_mission();
        }
        InteractionEvent::ClearMissions => {
            state.release_selected();
        }
        InteractionEvent::ClearSelection => {
            state.orbital_context.selected.clear();
        }
        InteractionEvent::ClearOrbitQueue => {
            state.orbital_context.queued_orbits.clear();
        }
        InteractionEvent::SimSlower => {
            if let Some(t) = enum_iterator::previous(&state.universe_ticks_per_game_tick) {
                state.universe_ticks_per_game_tick = t;
            }
        }
        InteractionEvent::SetSim(r) => {
            state.universe_ticks_per_game_tick = *r;
        }
        InteractionEvent::SimFaster => {
            if let Some(t) = enum_iterator::next(&state.universe_ticks_per_game_tick) {
                state.universe_ticks_per_game_tick = t;
            }
        }
        InteractionEvent::SimPause => {
            state.paused = !state.paused;
        }
        InteractionEvent::CursorMode => {
            state.orbital_context.cursor_mode = next_cycle(&state.orbital_context.cursor_mode);
        }
        InteractionEvent::DrawMode => {
            state.orbital_context.draw_mode = next_cycle(&state.orbital_context.draw_mode);
        }
        InteractionEvent::Orbits => {
            state.orbital_context.show_orbits = next_cycle(&state.orbital_context.show_orbits);
        }
        InteractionEvent::Spawn => {
            state.spawn_new();
        }
        InteractionEvent::ToggleFullscreen => {
            let fs = WindowMode::BorderlessFullscreen(MonitorSelection::Current);
            window.mode = if window.mode == fs {
                WindowMode::Windowed
            } else {
                fs
            };
        }
        InteractionEvent::ToggleDebugConsole => {
            state.console.toggle();
        }
        InteractionEvent::Escape => {
            if state.console.is_active() {
                state.console.hide()
            } else if !state.is_exit_prompt {
                state.is_exit_prompt = true;
            } else {
                state.shutdown()
            }
        }
        InteractionEvent::ContextDependent => {
            if let Some(o) = state.cursor_orbit_if_mode() {
                state.notice(format!("Enqueued orbit {}", &o));
                state.orbital_context.queued_orbits.push(o);
            } else if state.orbital_context.following.is_some() {
                state.orbital_context.following = None;
            } else if !state.orbital_context.selected.is_empty() {
                state.orbital_context.selected.clear();
            }
        }
        InteractionEvent::ToggleObject(id) => {
            state.orbital_context.toggle_track(*id);
        }
        InteractionEvent::ToggleGroup(gid) => {
            state.toggle_group(*gid);
        }
        InteractionEvent::DisbandGroup(gid) => {
            state.disband_group(*gid);
        }
        InteractionEvent::CreateGroup => {
            // let gid = state.ids.next();
            // state.create_group(gid);
            println!("todo!");
        }
        _ => (),
    };
    Some(())
}

fn handle_interactions(
    mut events: EventReader<InteractionEvent>,
    mut state: ResMut<GameState>,
    mut window: Single<&mut Window>,
) {
    for e in events.read() {
        debug!("Interaction event: {e:?}");
        process_interaction(e, &mut state, &mut window);
    }
}
