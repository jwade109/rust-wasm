use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use starling::aabb::AABB;
use starling::core::*;
use starling::examples::*;
use starling::orbit::*;
use starling::orbiter::*;
use starling::planning::*;

use crate::debug::*;
use crate::drawing::*;

pub struct PlanetaryPlugin;

impl Plugin for PlanetaryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_system);
        app.add_systems(
            Update,
            (
                log_system_info,
                process_commands,
                keyboard_input,
                mouse_button_input,
                handle_zoom,
                scroll_events,
                update_cursor,
                propagate_system,
                manage_orbiter_labels,
                update_text,
                update_camera,
                draw,
            )
                .chain(),
        );
    }
}

fn init_system(mut commands: Commands) {
    commands.insert_resource(GameState::default());
    let s = 0.02;
    commands.insert_resource(ClearColor(Color::linear_rgb(s, s, s)));
}

fn manage_orbiter_labels(
    mut commands: Commands,
    state: Res<GameState>,
    text: Query<(Entity, &FollowObject)>,
) {
    for tid in &state.track_list {
        let has_txt = text.iter().any(|(_, f)| f.0 == *tid);
        if !has_txt {
            commands.spawn((
                Text2d::new(""),
                FollowObject(*tid),
                bevy::sprite::Anchor::TopLeft,
            ));
        }
    }

    for (e, f) in text.iter() {
        if !state.track_list.contains(&f.0) {
            commands.entity(e).despawn();
        }
    }
}

fn update_text(res: Res<GameState>, mut text: Query<(&mut Transform, &mut Text2d, &FollowObject)>) {
    let scale = res.actual_scale.min(1.0);
    let _ = text
        .iter_mut()
        .filter_map(|(mut tr, mut text, follow)| {
            let id = follow.0;
            let obj = res.system.objects.iter().find(|o| o.id == id)?;
            let pvl = obj.pvl(res.sim_time)?;
            let pv = obj.pv(res.sim_time, &res.system.system)?;
            let prop = obj.propagator_at(res.sim_time)?;
            let (_, _, _, parent) = res.system.system.lookup(prop.parent, res.sim_time)?;
            let warn_str = if obj.will_collide() && res.duty_cycle_high {
                " COLLISION IMMINENT"
            } else {
                ""
            };

            let event_lines = obj
                .props()
                .iter()
                .filter_map(|p| {
                    let dt = (p.end - res.sim_time).to_secs();
                    if let Some(e) = p.event {
                        Some(format!("\n{:?} in {:0.1}s", e, dt))
                    } else {
                        Some(format!("\nC {:0.1}s {:0.2}s", dt, p.dt.to_secs()))
                    }
                })
                .collect::<String>();

            let p_line = obj
                .propagator_at(res.sim_time)
                .map(|p| p.orbit.t_next_p(res.sim_time))
                .flatten()
                .map(|nt| format!("\nP {:0.1}s", (nt - res.sim_time).to_secs()))
                .unwrap_or("".into());

            let txt = format!(
                "{:?}{}\nOrbiting {}{}\nA {:0.2}\nV {:0.2}{}",
                id,
                warn_str,
                parent.name,
                p_line,
                pvl.pos.length(),
                pvl.vel.length(),
                event_lines,
            );

            tr.translation = (pv.pos + Vec2::new(40.0 * scale, 40.0 * scale)).extend(0.0);
            tr.scale = Vec3::new(scale, scale, scale);
            *text = txt.into();
            Some(())
        })
        .collect::<Vec<_>>();
}

fn draw(gizmos: Gizmos, res: Res<GameState>) {
    draw_game_state(gizmos, res)
}

#[derive(Debug, PartialEq, Eq)]
enum CameraTracking {
    TrackingTracks,
    TrackingCursor,
    Freewheeling,
}

#[derive(Component)]
struct FollowObject(ObjectId);

#[derive(Debug)]
pub struct CameraState {
    pub center: Vec2,
    easing_lpf: f32,
    state: CameraTracking,
}

impl CameraState {
    fn track(&mut self, pos: Vec2, state: CameraTracking) {
        if self.state != state {
            self.easing_lpf = 0.1;
        }

        self.center += (pos - self.center) * self.easing_lpf;
        self.easing_lpf += (1.0 - self.easing_lpf) * 0.01;
        self.state = state;
    }
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            easing_lpf: 0.1,
            state: CameraTracking::Freewheeling,
        }
    }
}

#[derive(Resource)]
pub struct GameState {
    pub sim_time: Nanotime,
    pub physics_duration: Nanotime,
    pub sim_speed: i32,
    pub show_orbits: bool,
    pub show_potential_field: bool,
    pub paused: bool,
    pub system: OrbitalTree,
    pub ids: ObjectIdTracker,
    pub backup: Option<(OrbitalTree, ObjectIdTracker, Nanotime)>,
    pub track_list: Vec<ObjectId>,
    pub highlighted_list: Vec<ObjectId>,
    pub target_scale: f32,
    pub actual_scale: f32,
    pub draw_levels: Vec<i32>,
    pub cursor: Vec2,
    pub camera: CameraState,
    pub follow: bool,
    pub mouse_screen_pos: Option<Vec2>,
    pub mouse_down_pos: Option<Vec2>,
    pub window_dims: Vec2,
    pub control_points: Vec<Vec2>,
    pub hide_debug: bool,
    pub duty_cycle_high: bool,
}

impl GameState {
    pub fn game_bounds(&self) -> AABB {
        AABB::new(self.camera.center, self.window_dims * self.actual_scale)
    }

    pub fn window_bounds(&self) -> AABB {
        AABB::new(self.window_dims / 2.0, self.window_dims)
    }

    pub fn primary(&self) -> ObjectId {
        *self.track_list.first().unwrap_or(&ObjectId(-1))
    }

    pub fn mouse_pos(&self) -> Option<Vec2> {
        let gb = self.game_bounds();
        let wb = self.window_bounds();
        Some(AABB::map(wb, gb, self.mouse_screen_pos?))
    }

    pub fn mouse_down_pos(&self) -> Option<Vec2> {
        let p = self.mouse_down_pos?;
        let gb = self.game_bounds();
        let wb = self.window_bounds();
        Some(AABB::map(wb, gb, p))
    }

    pub fn selection_region(&self) -> Option<AABB> {
        Some(AABB::from_arbitrary(
            self.mouse_pos()?,
            self.mouse_down_pos()?,
        ))
    }

    pub fn toggle_track(&mut self, id: ObjectId) {
        if self.track_list.contains(&id) {
            self.track_list.retain(|e| *e != id);
        } else {
            self.track_list.insert(0, id);
        }
    }

    pub fn tracked_aabb(&self) -> Option<AABB> {
        let pos = self
            .track_list
            .iter()
            .filter_map(|id| Some(self.system.orbiter_lookup(*id, self.sim_time)?.pv().pos))
            .collect::<Vec<_>>();
        AABB::from_list(&pos).map(|aabb| aabb.padded(60.0))
    }

    pub fn target_orbit(&self) -> Option<Orbit> {
        let p1 = self.control_points.get(0);
        let p2 = self.control_points.get(1).map(|e| *e).or(self.mouse_pos());

        if let Some((p1, p2)) = p1.zip(p2) {
            if p1.distance(p2) < 10.0 {
                return None;
            }

            let v = (self.system.system.primary.mass * GRAVITATIONAL_CONSTANT / p1.length()).sqrt();

            Some(Orbit::from_pv(
                (*p1, (p2 - p1) * v / p1.length()),
                self.system.system.primary.mass,
                self.sim_time,
            ))
        } else {
            None
        }
    }

    pub fn spawn_new(&mut self) {
        let t = self.target_orbit().or_else(|| {
            let lup = self.system.orbiter_lookup(self.primary(), self.sim_time)?;
            if lup.level == 0 {
                Some(lup.object.propagator_at(self.sim_time)?.orbit)
            } else {
                None
            }
        });

        if let Some(orbit) = t {
            let id = self.ids.next();
            self.toggle_track(id);
            self.system.add_object(
                id,
                self.system.system.id,
                orbit.random_nudge(self.sim_time, 1.0),
                self.sim_time,
            );
        }
    }

    pub fn delete_objects(&mut self) {
        self.track_list.iter().for_each(|i| {
            self.system.remove_object(*i);
        });
    }

    pub fn primary_object_mut(&mut self) -> Option<&mut Orbiter> {
        let pri = self.primary();
        self.system.objects.iter_mut().find(|o| o.id == pri)
    }

    pub fn do_maneuver(&mut self, dv: Vec2) -> Option<()> {
        if self.paused {
            return Some(());
        }
        let s = self.sim_time;
        let d = self.physics_duration;
        let p = self.system.system.clone();
        let obj = self.primary_object_mut()?;
        obj.dv(s, dv);
        let res = obj.propagate_to(s, d, &p);
        match res {
            Ok(_) => Some(()),
            Err(p) => {
                dbg!(p);
                None
            }
        }
    }
}

impl Default for GameState {
    fn default() -> Self {
        let (system, ids) = default_example();
        GameState {
            sim_time: Nanotime(0),
            physics_duration: Nanotime::secs(500),
            sim_speed: 0,
            show_orbits: true,
            show_potential_field: false,
            paused: false,
            system: system.clone(),
            ids,
            track_list: Vec::new(),
            highlighted_list: Vec::new(),
            backup: Some((system, ids, Nanotime(0))),
            target_scale: 4.0,
            actual_scale: 4.0,
            draw_levels: (-70000..=-10000)
                .step_by(10000)
                .chain((-5000..-3000).step_by(250))
                .collect(),
            cursor: Vec2::ZERO,
            camera: CameraState::default(),
            follow: false,
            mouse_screen_pos: None,
            mouse_down_pos: None,
            window_dims: Vec2::ZERO,
            control_points: Vec::new(),
            hide_debug: false,
            duty_cycle_high: false,
        }
    }
}

fn propagate_system(time: Res<Time>, mut state: ResMut<GameState>) {
    if !state.paused {
        let sp = 10.0f32.powi(state.sim_speed);
        state.sim_time += Nanotime((time.delta().as_nanos() as f32 * sp) as i64);
    }

    state.duty_cycle_high = time.elapsed().as_millis() % 1000 < 500;

    let s = state.sim_time;
    let d = state.physics_duration;
    state.system.propagate_to(s, d);

    if let Some(a) = state.selection_region() {
        state.highlighted_list = state
            .system
            .objects
            .iter()
            .filter_map(|o| {
                let pv = state.system.orbiter_lookup(o.id, state.sim_time)?.pv();
                a.contains(pv.pos).then(|| o.id)
            })
            .collect();
    } else {
        state.highlighted_list.clear();
    }

    let mut track_list = state.track_list.clone();
    track_list.retain(|o| state.system.orbiter_lookup(*o, state.sim_time).is_some());
    state.track_list = track_list;
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
    if state.hide_debug {
        return;
    }

    send_log(&mut evt, &format!("Camera: {:?}", state.camera));
    if state.track_list.len() > 15 {
        send_log(&mut evt, &format!("Tracks: lots of em"));
    } else {
        send_log(&mut evt, &format!("Tracks: {:?}", state.track_list));
    }
    send_log(&mut evt, &format!("Epoch: {:?}", state.sim_time));
    send_log(&mut evt, &format!("Physics: {:?}", state.physics_duration));
    send_log(&mut evt, &format!("Scale: {:0.3}", state.actual_scale));
    send_log(&mut evt, &format!("{} objects", state.system.objects.len()));
    if state.paused {
        send_log(&mut evt, "Paused");
    }
    send_log(
        &mut evt,
        &format!(
            "Sim speed: 10^{} [{}]",
            state.sim_speed,
            sim_speed_str(state.sim_speed)
        ),
    );

    let prop_count: usize = state.system.objects.iter().map(|o| o.props().len()).sum();
    send_log(&mut evt, &format!("Propagators: {}", prop_count));

    if let Some(lup) = state.system.orbiter_lookup(state.primary(), state.sim_time) {
        if let Some(b) = lup.body {
            send_log(&mut evt, &format!("BD: {:?}", b));
        }

        for prop in lup.object.props() {
            send_log(
                &mut evt,
                &format!(
                    "- [{:?}, {:?}, {}, {:?}]",
                    prop.start, prop.end, prop.finished, prop.event
                ),
            );
        }

        if let Some(prop) = lup.object.propagator_at(state.sim_time) {
            send_log(&mut evt, &format!("{:#?}", prop));
            send_log(
                &mut evt,
                &format!("Consistent: {}", prop.orbit.is_consistent(state.sim_time)),
            );
            send_log(
                &mut evt,
                &format!("Next p: {:?}", prop.orbit.t_next_p(state.sim_time)),
            );
            send_log(&mut evt, &format!("Period: {:?}", prop.orbit.period()));
            send_log(
                &mut evt,
                &format!("Orbit count: {:?}", prop.orbit.orbit_number(state.sim_time)),
            );
        }
    }
}

fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<GameState>,
    mut exit: ResMut<Events<bevy::app::AppExit>>,
    cstate: Res<CommandsState>,
    time: Res<Time>,
) {
    if cstate.active {
        return;
    }

    for key in keys.get_just_pressed() {
        match key {
            KeyCode::Period => {
                state.sim_speed = i32::clamp(state.sim_speed + 1, -10, 4);
            }
            KeyCode::Comma => {
                state.sim_speed = i32::clamp(state.sim_speed - 1, -10, 4);
            }
            KeyCode::KeyF => {
                state.follow = !state.follow;
                if state.follow {
                    state.camera.easing_lpf = 0.1;
                }
            }
            KeyCode::Delete => {
                state.delete_objects();
            }
            KeyCode::KeyH => {
                state.hide_debug = !state.hide_debug;
            }
            KeyCode::KeyK => {
                state.spawn_new();
            }
            KeyCode::Equal => {
                state.target_scale /= 1.5;
            }
            KeyCode::Minus => {
                state.target_scale *= 1.5;
            }
            _ => (),
        }
    }

    let dt = time.delta().as_secs_f32();
    let cursor_rate = 1400.0 * state.actual_scale;

    let dv = if keys.pressed(KeyCode::ControlLeft) {
        0.002
    } else {
        0.03
    };

    if keys.pressed(KeyCode::KeyW) {
        state.cursor.y += cursor_rate * dt;
        state.follow = false;
    }
    if keys.pressed(KeyCode::KeyA) {
        state.cursor.x -= cursor_rate * dt;
        state.follow = false;
    }
    if keys.pressed(KeyCode::KeyD) {
        state.cursor.x += cursor_rate * dt;
        state.follow = false;
    }
    if keys.pressed(KeyCode::KeyS) {
        state.cursor.y -= cursor_rate * dt;
        state.follow = false;
    }

    if keys.pressed(KeyCode::ArrowUp) {
        state.do_maneuver(Vec2::Y * dv);
    }

    if keys.pressed(KeyCode::ArrowDown) {
        state.do_maneuver(-Vec2::Y * dv);
    }

    if keys.pressed(KeyCode::ArrowLeft) {
        state.do_maneuver(-Vec2::X * dv);
    }

    if keys.pressed(KeyCode::ArrowRight) {
        state.do_maneuver(Vec2::X * dv);
    }

    if keys.just_pressed(KeyCode::Space) {
        state.paused = !state.paused;
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.send(bevy::app::AppExit::Success);
    }
}

fn mouse_button_input(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<GameState>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        state.mouse_down_pos = state.mouse_screen_pos;
    }
    if buttons.just_pressed(MouseButton::Right) {
        state.control_points.clear();
        if let Some(p) = state.mouse_pos() {
            state.control_points.push(p);
        }
    }
    if buttons.just_released(MouseButton::Right) {
        if let Some(p) = state.mouse_pos() {
            state.control_points.push(p);
        }
    }
    if buttons.just_released(MouseButton::Left) {
        state.mouse_down_pos = None;
        let hl = state.highlighted_list.clone();
        if keys.pressed(KeyCode::ShiftLeft) {
            // add to track list
            for h in hl {
                if !state.track_list.contains(&h) {
                    state.track_list.push(h);
                }
            }
        } else if keys.pressed(KeyCode::KeyX) {
            // remove from track list
            state.track_list.retain(|id| !hl.contains(id))
        } else {
            // start from scratch
            state.track_list.clear();
            state.track_list = hl;
        }
        state.camera.state = CameraTracking::Freewheeling;
    }
    // we can check multiple at once with `.any_*`
    if buttons.any_just_pressed([MouseButton::Left, MouseButton::Middle]) {
        // Either the left or the middle (wheel) button was just pressed
    }
}

fn update_cursor(
    mut state: ResMut<GameState>,
    q: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    let (w, p) = match q.get_single() {
        Ok(w) => (w, w.cursor_position()),
        Err(_) => {
            state.mouse_screen_pos = None;
            return;
        }
    };
    state.mouse_screen_pos = p.map(|p| Vec2::new(p.x, w.height() - p.y));
    state.window_dims = Vec2::new(w.width(), w.height());
}

fn load_new_scenario(state: &mut GameState, tree: OrbitalTree, ids: ObjectIdTracker) {
    state.backup = Some((tree.clone(), ids, Nanotime(0)));
    state.target_scale = 0.001 * tree.system.primary.soi;
    state.system = tree;
    state.sim_time = Nanotime(0);
}

fn on_command(state: &mut GameState, cmd: &Vec<String>) {
    let starts_with = |s: &'static str| -> bool { cmd.first() == Some(&s.to_string()) };

    if starts_with("load") {
        let (system, ids) = match cmd.get(1).map(|s| s.as_str()) {
            Some("grid") => consistency_example(),
            Some("earth") => earth_moon_example_one(),
            Some("earth2") => earth_moon_example_two(),
            Some("moon") => just_the_moon(),
            Some("jupiter") => sun_jupiter_lagrange(),
            _ => {
                return;
            }
        };
        load_new_scenario(state, system, ids);
    } else if starts_with("toggle") {
        match cmd.get(1).map(|s| s.as_str()) {
            Some("potential") => {
                state.show_potential_field = !state.show_potential_field;
            }
            Some("orbit") => {
                state.show_orbits = !state.show_orbits;
            }
            _ => {
                return;
            }
        }
    } else if starts_with("restore") {
        if let Some((sys, ids, time)) = &state.backup {
            state.system = sys.clone();
            state.sim_time = *time;
            state.ids = *ids;
        }
    } else if starts_with("save") {
        state.backup = Some((state.system.clone(), state.ids, state.sim_time));
    } else if starts_with("track") {
        for n in cmd.iter().skip(1).filter_map(|s| s.parse::<i64>().ok()) {
            let id = ObjectId(n);
            state.toggle_track(id);
        }
    } else if starts_with("untrack") {
        state.track_list.clear();
    } else if starts_with("level") {
        if Some(&"clear".to_string()) == cmd.get(1) {
            state.draw_levels.clear();
        } else {
            state.draw_levels.extend(
                cmd.iter()
                    .skip(1)
                    .filter_map(|s| Some(-(s.parse::<i32>().ok()?))),
            );
        }
    } else if starts_with("rm") {
        state.delete_objects();
    } else if starts_with("spawn") {
        state.spawn_new();
        // } else if starts_with("maneuver") {
        //     let tl = state.track_list.clone();
        //     _ = tl
        //         .iter()
        //         .filter_map(|id| {
        //             let dt = Nanotime::secs_f32(cmd.get(1)?.parse().ok()?);
        //             let dx = cmd.get(2)?.parse::<f32>().ok()?;
        //             let dy = cmd.get(3)?.parse::<f32>().ok()?;
        //             let t = state.sim_time + dt;
        //             state.register_maneuver(*id, Vec2::new(dx, dy), t);
        //             Some(())
        //         })
        //         .collect::<Vec<_>>();
    }
}

fn process_commands(mut evts: EventReader<DebugCommand>, mut state: ResMut<GameState>) {
    for DebugCommand(cmd) in evts.read() {
        on_command(&mut state, cmd);
    }
}

fn handle_zoom(mut state: ResMut<GameState>, mut tf: Query<&mut Transform, With<Camera>>) {
    let mut transform = tf.single_mut();
    let ds = (state.target_scale - transform.scale) * 0.5;
    transform.scale += ds;
    state.actual_scale = transform.scale.x;
}

fn update_camera(mut query: Query<&mut Transform, With<Camera>>, mut state: ResMut<GameState>) {
    let mut tf = query.single_mut();

    if state.follow {
        if let Some(a) = state.tracked_aabb() {
            state
                .camera
                .track(a.center, CameraTracking::TrackingTracks);
            state.cursor = state.camera.center;
        } else {
            let s = state.cursor;
            state.camera.track(s, CameraTracking::Freewheeling);
        }
    } else {
        let s = state.cursor;
        state.camera.track(s, CameraTracking::TrackingCursor)
    }

    *tf = tf.with_translation(state.camera.center.extend(0.0));
}

fn scroll_events(
    mut evr_scroll: EventReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<GameState>,
) {
    if keys.pressed(KeyCode::ShiftLeft) {
        for ev in evr_scroll.read() {
            if ev.y > 0.0 {
                state.sim_speed = i32::clamp(state.sim_speed + 1, -10, 4);
            } else {
                state.sim_speed = i32::clamp(state.sim_speed - 1, -10, 4);
            }
        }
    } else {
        for ev in evr_scroll.read() {
            if ev.y > 0.0 {
                state.target_scale /= 1.3;
            } else {
                state.target_scale *= 1.3;
            }
        }
    }
}
