// ignore-tidy-linelength

use crate::camera_controls::*;
use crate::debug::*;
use crate::mouse::MouseState;
use crate::notifications::*;
use crate::ui::InteractionEvent;
use bevy::color::palettes::css::*;
use bevy::core_pipeline::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::window::WindowMode;
use layout::layout as ui;
use names::Generator;
use starling::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;

pub struct PlanetaryPlugin;

impl Plugin for PlanetaryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_system);
        app.add_systems(Update, log_system_info);

        app.add_systems(
            Update,
            (
                // egui
                crate::egui::ui_example_system,
                // physics
                step_system,
                // inputs
                crate::keybindings::keyboard_input,
                track_highlighted_objects,
                handle_interactions,
                handle_camera_interactions,
                crate::mouse::update_mouse_state,
                // update camera stuff
                move_camera_and_store,
                // rendering
                crate::sprites::make_new_sprites,
                crate::sprites::update_planet_sprites,
                crate::sprites::update_shadow_sprites,
                crate::sprites::update_background_sprite,
                crate::sprites::update_spacecraft_sprites,
                crate::drawing::draw_game_state,
            )
                .chain(),
        );
    }
}

#[derive(Component, Default)]
pub struct SoftController(pub Transform);

fn init_system(mut commands: Commands) {
    commands.insert_resource(GameState::default());
    commands.spawn((
        Camera2d,
        Camera {
            hdr: true,
            order: 0,
            clear_color: ClearColorConfig::Custom(BLACK.into()),
            ..default()
        },
        Bloom {
            intensity: 0.2,
            ..Bloom::OLD_SCHOOL
        },
        SoftController::default(),
        RenderLayers::layer(0),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(BLACK.with_alpha(0.0).into()),
            ..default()
        },
        RenderLayers::layer(1),
    ));
}

fn move_camera_and_store(
    mut query: Single<(&SoftController, &mut Transform)>,
    window: Single<&Window>,
    mut state: ResMut<GameState>,
) {
    let (ctrl, ref mut tf) = query.deref_mut();
    let target = ctrl.0;
    let current = tf.clone();
    tf.translation += (target.translation - current.translation) * 1.0;
    tf.scale += (target.scale - current.scale) * 1.0;

    state.camera.actual_scale = tf.scale.z;
    state.camera.world_center = tf.translation.xy();
    state.camera.window_dims = Vec2::new(window.width(), window.height());

    state.mouse.viewport_bounds = state.camera.viewport_bounds();
    state.mouse.world_bounds = state.camera.world_bounds();
    state.mouse.scale = tf.scale.z;
}

#[derive(Debug, Clone, Copy)]
pub enum ShowOrbitsState {
    None,
    Focus,
    All,
}

impl ShowOrbitsState {
    fn next(&mut self) {
        let n = match self {
            ShowOrbitsState::None => ShowOrbitsState::Focus,
            ShowOrbitsState::Focus => ShowOrbitsState::All,
            ShowOrbitsState::All => ShowOrbitsState::None,
        };
        *self = n;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Default,
    Constellations,
    Stability,
}

impl GameMode {
    fn next(&self) -> Self {
        match self {
            GameMode::Default => GameMode::Constellations,
            GameMode::Constellations => GameMode::Stability,
            GameMode::Stability => GameMode::Default,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SelectionMode {
    Rect,
    Altitude,
    NearOrbit,
}

impl SelectionMode {
    fn next(&self) -> Self {
        match self {
            SelectionMode::Rect => SelectionMode::Altitude,
            SelectionMode::Altitude => SelectionMode::NearOrbit,
            SelectionMode::NearOrbit => SelectionMode::Rect,
        }
    }
}

#[derive(Resource)]
pub struct GameState {
    pub current_frame_no: u32,

    pub mouse: MouseState,
    pub camera: CameraState,

    pub sim_time: Nanotime,
    pub actual_time: Nanotime,
    pub physics_duration: Nanotime,
    pub sim_speed: i32,
    pub paused: bool,
    pub scenario: Scenario,
    pub ids: ObjectIdTracker,
    pub backup: Option<(Scenario, ObjectIdTracker, Nanotime)>,
    pub track_list: HashSet<ObjectId>,
    pub hide_debug: bool,
    pub show_graph: bool,
    pub duty_cycle_high: bool,
    pub controllers: Vec<Controller>,
    pub follow: Option<ObjectId>,
    pub show_orbits: ShowOrbitsState,
    pub show_animations: bool,
    pub queued_orbits: Vec<GlobalOrbit>,
    pub constellations: HashMap<ObjectId, GroupId>,
    pub selection_mode: SelectionMode,

    pub ui: ui::Tree<crate::ui::GuiNodeId>,
    pub context_menu_origin: Option<Vec2>,

    pub last_redraw: Nanotime,

    pub game_mode: GameMode,

    pub notifications: Vec<Notification>,
}

impl Default for GameState {
    fn default() -> Self {
        let (scenario, ids) = default_example();

        GameState {
            current_frame_no: 0,
            mouse: MouseState::default(),
            sim_time: Nanotime::zero(),
            actual_time: Nanotime::zero(),
            physics_duration: Nanotime::secs(120),
            sim_speed: 0,
            paused: false,
            scenario: scenario.clone(),
            ids,
            track_list: HashSet::new(),
            backup: Some((scenario, ids, Nanotime::zero())),
            camera: CameraState::default(),
            hide_debug: true,
            show_graph: false,
            duty_cycle_high: false,
            controllers: vec![],
            follow: None,
            show_orbits: ShowOrbitsState::Focus,
            show_animations: false,
            queued_orbits: Vec::new(),
            constellations: HashMap::new(),
            selection_mode: SelectionMode::Rect,
            ui: ui::Tree::new(),
            context_menu_origin: None,
            last_redraw: Nanotime::zero(),
            game_mode: GameMode::Default,
            notifications: Vec::new(),
        }
    }
}

impl GameState {
    pub fn redraw(&mut self) {
        // self.last_redraw = Nanotime::zero()
    }

    pub fn primary(&self) -> Option<ObjectId> {
        self.track_list.iter().next().cloned()
    }

    pub fn toggle_track(&mut self, id: ObjectId) {
        if self.track_list.contains(&id) {
            self.track_list.retain(|e| *e != id);
        } else {
            self.track_list.insert(id);
        }
    }

    pub fn is_tracked(&self, id: ObjectId) -> bool {
        self.track_list.contains(&id)
    }

    pub fn get_group_members(&mut self, gid: &GroupId) -> Vec<ObjectId> {
        self.constellations
            .iter()
            .filter_map(|(id, g)| (g == gid).then(|| *id))
            .collect()
    }

    pub fn group_membership(&self, id: &ObjectId) -> Option<&GroupId> {
        self.constellations.get(id)
    }

    pub fn unique_groups(&self) -> Vec<&GroupId> {
        let mut s: Vec<&GroupId> = self
            .constellations
            .iter()
            .map(|(_, gid)| gid)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        s.sort();
        s
    }

    pub fn toggle_group(&mut self, gid: &GroupId) -> Option<()> {
        // - if any of the orbiters in the group are not selected,
        //   select all of them
        // - if all of them are already selected, deselect all of them

        let members = self.get_group_members(gid);

        let all_selected = members.iter().all(|id| self.is_tracked(*id));

        for id in members {
            if all_selected {
                self.track_list.remove(&id);
            } else {
                self.track_list.insert(id);
            }
        }

        Some(())
    }

    pub fn disband_group(&mut self, gid: &GroupId) {
        self.constellations.retain(|_, g| g != gid);
    }

    pub fn create_group(&mut self, gid: GroupId) {
        for id in &self.track_list {
            self.constellations.insert(*id, gid.clone());
        }
    }

    pub fn planned_maneuvers(&self, after: Nanotime) -> Vec<(ObjectId, Nanotime, Vec2)> {
        let mut dvs = vec![];
        for ctrl in &self.controllers {
            if let Some(plan) = ctrl.plan() {
                for (stamp, impulse) in plan.future_dvs(after) {
                    dvs.push((ctrl.target(), stamp, impulse));
                }
            }
        }
        dvs.sort_by_key(|(_, t, _)| t.inner());
        dvs
    }

    pub fn control_points(&self) -> Vec<Vec2> {
        self.mouse
            .right_world()
            .into_iter()
            .chain(self.mouse.current_world().into_iter())
            .collect()
    }

    pub fn selection_region(&self) -> Option<Region> {
        match self.selection_mode {
            SelectionMode::Rect => self
                .mouse
                .left_world()
                .zip(self.mouse.current_world())
                .map(|(a, b)| Region::aabb(a, b)),
            SelectionMode::Altitude => self
                .mouse
                .left_world()
                .zip(self.mouse.current_world())
                .map(|(a, b)| Region::altitude(a, b)),
            SelectionMode::NearOrbit => self
                .left_cursor_orbit()
                .map(|GlobalOrbit(_, orbit)| Region::NearOrbit(orbit, 50.0)),
        }
    }

    pub fn cursor_pv(&self, p1: Vec2, p2: Vec2) -> Option<PV> {
        if p1.distance(p2) < 20.0 {
            return None;
        }

        let wrt_id = self.scenario.relevant_body(p1, self.sim_time)?;
        let parent = self.scenario.lup(wrt_id, self.sim_time)?;

        let r = p1.distance(parent.pv().pos);
        let v = (parent.body()?.mu() / r).sqrt();

        Some(PV::new(p1, (p2 - p1) * v / r))
    }

    pub fn cursor_orbit(&self, p1: Vec2, p2: Vec2) -> Option<GlobalOrbit> {
        let pv = self.cursor_pv(p1, p2)?;
        let parent_id = self.scenario.relevant_body(pv.pos, self.sim_time)?;
        let parent = self.scenario.lup(parent_id, self.sim_time)?;
        let parent_pv = parent.pv();
        let pv = pv - PV::pos(parent_pv.pos);
        let body = parent.body()?;
        Some(GlobalOrbit(
            parent_id,
            SparseOrbit::from_pv(pv, body, self.sim_time)?,
        ))
    }

    pub fn left_cursor_orbit(&self) -> Option<GlobalOrbit> {
        self.cursor_orbit(self.mouse.left_world()?, self.mouse.current_world()?)
    }

    pub fn right_cursor_orbit(&self) -> Option<GlobalOrbit> {
        self.cursor_orbit(self.mouse.right_world()?, self.mouse.current_world()?)
    }

    pub fn follow_position(&self) -> Option<Vec2> {
        let id = self.follow?;
        let lup = self.scenario.lup(id, self.sim_time)?;
        Some(lup.pv().pos)
    }

    pub fn spawn_at(&mut self, global: &GlobalOrbit) -> Option<()> {
        let GlobalOrbit(parent, orbit) = global;
        let pv_local = orbit.pv(self.sim_time).ok()?;
        let perturb = PV::new(
            randvec(pv_local.pos.length() * 0.005, pv_local.pos.length() * 0.02),
            randvec(pv_local.vel.length() * 0.005, pv_local.vel.length() * 0.02),
        );
        let orbit = SparseOrbit::from_pv(pv_local + perturb, orbit.body, self.sim_time)?;
        let id = self.ids.next();
        self.scenario.add_object(id, *parent, orbit, self.sim_time);
        Some(())
    }

    pub fn spawn_new(&mut self) -> Option<()> {
        let orbit = self.right_cursor_orbit()?;
        self.spawn_at(&orbit)
    }

    pub fn delete_orbiter(&mut self, id: ObjectId) -> Option<()> {
        let lup = self.scenario.lup(id, self.sim_time)?;
        let _orbiter = lup.orbiter()?;
        let parent = lup.parent(self.sim_time)?;
        let pv = lup.pv().pos;
        let plup = self.scenario.lup(parent, self.sim_time)?;
        let pvp = plup.pv().pos;
        let pvl = pv - pvp;
        self.scenario.remove_object(id)?;
        self.notify(parent, NotificationType::OrbiterDeleted(id), pvl);
        Some(())
    }

    pub fn delete_objects(&mut self) {
        self.track_list.clone().into_iter().for_each(|id| {
            self.delete_orbiter(id);
        });
    }

    pub fn highlighted(&self) -> HashSet<ObjectId> {
        if let Some(a) = self.selection_region() {
            self.scenario
                .all_ids()
                .into_iter()
                .filter_map(|id| {
                    let pv = self.scenario.lup(id, self.sim_time)?.pv();
                    a.contains(pv.pos).then(|| id)
                })
                .collect()
        } else {
            HashSet::new()
        }
    }

    pub fn do_maneuver(&mut self, dv: Vec2) -> Option<()> {
        let id = self.follow?;

        if !self.track_list.contains(&id) {
            return None;
        }

        if self
            .scenario
            .impulsive_burn(id, self.sim_time, dv, 10)
            .is_none()
        {
            self.notify(id, NotificationType::ManeuverFailed(id), None)
        } else {
            self.notify(id, NotificationType::OrbitChanged(id), None);
        }

        self.scenario.simulate(self.sim_time, self.physics_duration);
        Some(())
    }

    pub fn command_selected(&mut self, next: &GlobalOrbit) {
        if self.track_list.is_empty() {
            return;
        }
        info!("Commanding {} orbiters to {}", self.track_list.len(), next,);
        for id in self.track_list.clone() {
            self.command(id, next);
        }
    }

    pub fn release_selected(&mut self) {
        let tracks = self.track_list.clone();
        self.controllers.retain(|c| !tracks.contains(&c.target()));
    }

    pub fn command(&mut self, id: ObjectId, next: &GlobalOrbit) -> Option<()> {
        self.scenario.lup(id, self.sim_time)?.orbiter()?;

        if self.controllers.iter().find(|c| c.target() == id).is_none() {
            self.controllers.push(Controller::idle(id));
        }

        self.controllers.iter_mut().for_each(|c| {
            let ret = c.set_destination(*next, self.sim_time);
            if let Err(_e) = ret {
                // dbg!(e);
            }
        });

        Some(())
    }

    pub fn notify(
        &mut self,
        parent: ObjectId,
        kind: NotificationType,
        offset: impl Into<Option<Vec2>>,
    ) {
        let notif = Notification {
            parent,
            offset: offset.into().unwrap_or(Vec2::ZERO),
            jitter: Vec2::ZERO,
            wall_time: self.actual_time,
            extra_time: Nanotime::secs_f32(rand(0.0, 1.0)),
            kind,
        };

        if self.notifications.iter().any(|e| notif.is_duplicate(e)) {
            return;
        }

        self.notifications.push(notif);
    }

    pub fn on_button_event(&mut self, id: crate::ui::GuiNodeId) -> Option<()> {
        use crate::ui::GuiNodeId;

        match id {
            GuiNodeId::Orbiter(id) => self.follow = Some(id),
            GuiNodeId::ToggleDrawMode => self.game_mode = self.game_mode.next(),
            GuiNodeId::ClearTracks => self.track_list.clear(),
            GuiNodeId::ClearOrbits => self.queued_orbits.clear(),
            GuiNodeId::Group(gid) => self.toggle_group(&gid).unwrap(),
            GuiNodeId::CreateGroup => self.create_group(get_group_id()),
            GuiNodeId::Exit => std::process::exit(0),
            _ => (),
        };

        Some(())
    }

    pub fn step(&mut self, time: &Time) {
        self.current_frame_no += 1;
        let old_sim_time = self.sim_time;
        self.actual_time += Nanotime::nanos(time.delta().as_nanos() as i64);
        if !self.paused {
            let sp = 10.0f32.powi(self.sim_speed);
            self.sim_time += Nanotime::nanos((time.delta().as_nanos() as f32 * sp) as i64);
        }

        self.duty_cycle_high = time.elapsed().as_millis() % 1000 < 500;

        if let Some(p) = self.mouse.just_left_clicked(self.current_frame_no - 1) {
            let q = Vec2::new(p.x, self.camera.viewport_bounds().span.y - p.y);
            if let Some(n) = self.ui.at(q).map(|n| n.id()).flatten() {
                self.on_button_event(n.clone());
            }
            self.context_menu_origin = None;
            self.redraw();
        }

        if let Some(p) = self.mouse.just_right_clicked(self.current_frame_no - 1) {
            self.context_menu_origin = Some(p);
            self.redraw();
        }

        let s = self.sim_time;
        let d = self.physics_duration;

        let mut man = self.planned_maneuvers(old_sim_time);
        while let Some((id, t, dv)) = man.first() {
            if s > *t {
                let perturb = randvec(0.01, 0.05);
                self.scenario.simulate(*t, d);
                self.scenario.impulsive_burn(*id, *t, dv + perturb, 50);
                self.notify(*id, NotificationType::OrbitChanged(*id), None);
            } else {
                break;
            }
            man.remove(0);
        }

        for (id, ri) in self.scenario.simulate(s, d) {
            if let Some(pv) = ri.orbit.pv(ri.stamp).ok() {
                self.notify(ri.parent, NotificationType::OrbiterCrashed(id), pv.pos);
            }
        }

        let mut track_list = self.track_list.clone();
        track_list.retain(|o| self.scenario.lup(*o, self.sim_time).is_some());
        self.track_list = track_list;

        let ids: Vec<_> = self.scenario.orbiter_ids().collect();

        self.constellations.retain(|id, _| ids.contains(id));

        let mut notifs = vec![];

        self.controllers.iter_mut().for_each(|c| {
            if !c.needs_update(s) {
                return;
            }

            let lup = self.scenario.lup(c.target(), s);
            let orbiter = lup.map(|lup| lup.orbiter()).flatten();
            let prop = orbiter.map(|orb| orb.propagator_at(s)).flatten();

            if let Some(prop) = prop {
                let res = c.update(s, prop.orbit);
                if let Err(_) = res {
                    notifs.push((c.target(), NotificationType::ManeuverFailed(c.target())));
                }
            }
        });

        notifs
            .into_iter()
            .for_each(|(t, n)| self.notify(t, n, None));

        let mut finished_ids = Vec::<ObjectId>::new();

        self.controllers.retain(|c| {
            if c.is_idle() {
                finished_ids.push(c.target());
                false
            } else {
                true
            }
        });

        finished_ids
            .into_iter()
            .for_each(|id| self.notify(id, NotificationType::ManeuverComplete(id), None));

        self.notifications.iter_mut().for_each(|n| n.jitter());

        self.notifications
            .retain(|n| n.wall_time + n.duration() > self.actual_time);
    }
}

fn step_system(time: Res<Time>, mut state: ResMut<GameState>) {
    state.step(&time);
}

fn sim_speed_str(speed: i32) -> String {
    if speed == 0 {
        ">".to_owned()
    } else if speed > 0 {
        (0..speed.abs() * 2).map(|_| '>').collect()
    } else {
        (0..speed.abs() * 2).map(|_| '<').collect()
    }
}

fn log_system_info(state: Res<GameState>, mut evt: EventWriter<DebugLog>) {
    let mut log = |str: &str| {
        send_log(&mut evt, str);
    };

    if state.hide_debug {
        return;
    }

    let logs = [
        "",
        "Look around - [W][A][S][D]",
        "Control orbiter - Arrow Keys",
        "  Increase thrust - hold [LSHIFT]",
        "  Decrease thrust - hold [LCTRL]",
        "Zoom in/out - +/-, [Scroll]",
        "Select spacecraft - Left click and drag",
        "Set target orbit - Right click and drag",
        "Send spacecraft to orbit - [ENTER]",
        "Toggle orbit draw modes - [TAB]",
        "Increase sim speed - [.]",
        "Decrease sim speed - [,]",
        "Pause - [SPACE]",
        "",
    ];

    for s in logs {
        log(s);
    }

    log(&format!("Epoch: {:?}", state.sim_time));

    if state.paused {
        log("Paused");
    }
    log(&format!(
        "Sim speed: 10^{} [{}]",
        state.sim_speed,
        sim_speed_str(state.sim_speed)
    ));

    let mut show_id_list = |ids: &HashSet<ObjectId>, name: &str| {
        if ids.len() > 15 {
            log(&format!("{}: {} ...", name, ids.len()));
        } else {
            log(&format!("{}: {} {:?}", name, ids.len(), ids));
        }
    };

    show_id_list(&state.track_list, "Tracks");
    show_id_list(&state.highlighted(), "Select");

    log(&format!("Physics: {:?}", state.physics_duration));
    log(&format!("Scale: {:0.3}", state.camera.actual_scale));
    log(&format!("Ctlrs: {}", state.controllers.len()));

    {
        for (id, t, dv) in state.planned_maneuvers(state.sim_time) {
            log(&format!("- {} {:?} {}", id, t, dv))
        }
    }

    log(&format!("Orbiters: {}", state.scenario.orbiter_count()));
    log(&format!("Propagators: {}", state.scenario.prop_count()));

    if let Some(lup) = state
        .primary()
        .map(|id| state.scenario.lup(id, state.sim_time))
        .flatten()
    {
        if let Some(ctrl) = state.controllers.get(0) {
            log(&format!("{}", ctrl));
        }

        if let Some(o) = lup.orbiter() {
            log(&format!("{}", o));
            for prop in o.props() {
                log(&format!("- [{}]", prop));
            }
            if let Some(prop) = o.propagator_at(state.sim_time) {
                log(&format!(
                    "Next p: {:?}",
                    prop.orbit.1.t_next_p(state.sim_time)
                ));
                log(&format!("Period: {:?}", prop.orbit.1.period()));
                log(&format!(
                    "Orbit count: {:?}",
                    prop.orbit.1.orbit_number(state.sim_time)
                ));
            }
        } else if let Some(b) = lup.body() {
            log(&format!("BD: {:?}", b));
        }
    }
}

fn get_group_id() -> GroupId {
    let mut generator = Generator::default();
    let s = generator.next().unwrap();
    GroupId(s)
}

fn process_interaction(
    inter: &InteractionEvent,
    state: &mut GameState,
    exit: &mut EventWriter<bevy::app::AppExit>,
    window: &mut Window,
) -> Option<()> {
    match inter {
        InteractionEvent::Delete => state.delete_objects(),
        InteractionEvent::CommitMission => {
            for orbit in state.queued_orbits.clone() {
                state.command_selected(&orbit);
                break;
            }
        }
        InteractionEvent::ClearMissions => {
            state.release_selected();
        }
        InteractionEvent::ToggleDebugMode => {
            state.hide_debug = !state.hide_debug;
        }
        InteractionEvent::ToggleGraph => {
            state.show_graph = !state.show_graph;
        }
        InteractionEvent::ClearSelection => {
            state.track_list.clear();
        }
        InteractionEvent::ClearOrbitQueue => {
            state.queued_orbits.clear();
        }
        InteractionEvent::SimSlower => {
            state.sim_speed = i32::clamp(state.sim_speed - 1, -2, 2);
            state.redraw();
        }
        InteractionEvent::SimFaster => {
            state.sim_speed = i32::clamp(state.sim_speed + 1, -2, 2);
            state.redraw();
        }
        InteractionEvent::SimPause => {
            state.paused = !state.paused;
        }
        InteractionEvent::SelectionMode => {
            state.selection_mode = state.selection_mode.next();
        }
        InteractionEvent::GameMode => {
            state.game_mode = state.game_mode.next();
        }
        InteractionEvent::RedrawGui => {
            state.redraw();
        }
        InteractionEvent::Orbits => {
            state.show_orbits.next();
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
        InteractionEvent::DoubleClick(p) => {
            // check to see if we're in the main area
            let n = state
                .ui
                .at(Vec2::new(p.x, state.camera.viewport_bounds().span.y - p.y))?;
            (n.id() == Some(&crate::ui::GuiNodeId::World)).then(|| ())?;

            let w = state
                .camera
                .viewport_bounds()
                .map(state.camera.world_bounds(), *p);
            let id = state.scenario.nearest(w, state.sim_time);
            if let Some(id) = id {
                state.follow = Some(id);
                state.notify(id, NotificationType::Following(id), None);
            }
        }
        InteractionEvent::ExitApp => {
            exit.send(bevy::app::AppExit::Success);
        }
        InteractionEvent::Save => {
            state.backup = Some((state.scenario.clone(), state.ids, state.sim_time));
        }
        InteractionEvent::ContextDependent => {
            if let Some(o) = state.right_cursor_orbit() {
                info!("Enqueued orbit {}", &o);
                state.queued_orbits.push(o);
            } else if !state.track_list.is_empty() {
                state.track_list.clear();
            } else if !state.queued_orbits.is_empty() {
                state.queued_orbits.clear();
            }
        }
        InteractionEvent::Restore => {
            if let Some((sys, ids, time)) = &state.backup {
                state.scenario = sys.clone();
                state.sim_time = *time;
                state.ids = *ids;
            }
        }
        InteractionEvent::Load(name) => {
            let (system, ids) = match name.as_str() {
                "grid" => Some(consistency_example()),
                "earth" => Some(earth_moon_example_one()),
                "earth2" => Some(earth_moon_example_two()),
                "moon" => Some(just_the_moon()),
                "jupiter" => Some(sun_jupiter_lagrange()),
                _ => {
                    error!("No scenario named {}", name);
                    None
                }
            }?;
            load_new_scenario(state, system, ids);
        }
        InteractionEvent::ToggleObject(id) => {
            state.toggle_track(*id);
        }
        InteractionEvent::ToggleGroup(gid) => {
            state.toggle_group(gid);
        }
        InteractionEvent::DisbandGroup(gid) => {
            state.disband_group(gid);
        }
        InteractionEvent::CreateGroup => {
            let gid = get_group_id();
            state.create_group(gid.clone());
        }
        InteractionEvent::Thrust(dx, dy) => {
            let s = 0.1;
            let dv = (Vec2::X * *dx as f32 + Vec2::Y * *dy as f32) * s;
            state.do_maneuver(dv);
        }
        InteractionEvent::Reset
        | InteractionEvent::MoveLeft
        | InteractionEvent::MoveRight
        | InteractionEvent::MoveUp
        | InteractionEvent::MoveDown => state.follow = None,
        _ => (),
    };
    state.redraw();
    Some(())
}

fn handle_interactions(
    mut events: EventReader<InteractionEvent>,
    mut state: ResMut<GameState>,
    mut exit: EventWriter<bevy::app::AppExit>,
    mut window: Single<&mut Window>,
) {
    for e in events.read() {
        debug!("Interaction event: {e:?}");
        process_interaction(e, &mut state, &mut exit, &mut window);
    }
}

fn handle_camera_interactions(
    mut events: EventReader<InteractionEvent>,
    mut query: Query<&mut SoftController>,
    state: Res<GameState>,
    time: Res<Time>,
) {
    let mut ctrl = match query.get_single_mut() {
        Ok(c) => c,
        Err(e) => {
            error!("{:?}", e);
            return;
        }
    };

    let cursor_delta = 1400.0 * time.delta_secs() * ctrl.0.scale.z;
    let scale_scalar = 1.5;

    if let Some(p) = state.follow_position() {
        ctrl.0.translation = p.extend(0.0);
    }

    for e in events.read() {
        match e {
            InteractionEvent::MoveLeft => ctrl.0.translation.x -= cursor_delta,
            InteractionEvent::MoveRight => ctrl.0.translation.x += cursor_delta,
            InteractionEvent::MoveUp => ctrl.0.translation.y += cursor_delta,
            InteractionEvent::MoveDown => ctrl.0.translation.y -= cursor_delta,
            InteractionEvent::ZoomIn => ctrl.0.scale /= scale_scalar,
            InteractionEvent::ZoomOut => ctrl.0.scale *= scale_scalar,
            InteractionEvent::Reset => ctrl.0 = Transform::IDENTITY,
            _ => (),
        }
    }
}

// TODO get rid of this
fn track_highlighted_objects(buttons: Res<ButtonInput<MouseButton>>, mut state: ResMut<GameState>) {
    if buttons.just_released(MouseButton::Left) || buttons.just_released(MouseButton::Middle) {
        let h = state.highlighted();
        state.track_list.extend(h.into_iter());
    }
}

fn load_new_scenario(state: &mut GameState, scen: Scenario, ids: ObjectIdTracker) {
    state.backup = Some((scen.clone(), ids, Nanotime::zero()));
    state.scenario = scen;
    state.ids = ids;
    state.sim_time = Nanotime::zero();
    state.track_list.clear();
}
