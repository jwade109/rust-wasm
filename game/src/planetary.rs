use crate::util::{rand, randvec};
use bevy::color::palettes::basic::*;
use bevy::color::palettes::css::ORANGE;
use bevy::input::mouse::MouseWheel;
use bevy::math::VectorSpace;
use bevy::prelude::*;
use std::time::Duration;

// ORBIT PROPAGATION AND N-BODY SIMULATION

const GRAVITATIONAL_CONSTANT: f32 = 12000.0;

#[derive(Debug, Clone, Copy)]
struct Orbit {
    eccentricity: f32,
    semi_major_axis: f32,
    arg_periapsis: f32,
    true_anomaly: f32,
    mu: f32,
}

#[derive(Resource, Default)]
struct SimTime(Duration);

impl Orbit {
    fn from_pv(r: Vec2, v: Vec2, mass: f32) -> Self {
        let mu = mass * GRAVITATIONAL_CONSTANT;
        let r3 = r.extend(0.0);
        let v3 = v.extend(0.0);
        let h = r3.cross(v3);
        let e = v3.cross(h) / mu - r3 / r3.length();
        let arg_periapsis: f32 = f32::atan2(e.y, e.x);
        let semi_major_axis: f32 = h.length_squared() / (mu * (1.0 - e.length_squared()));
        let true_anomaly = f32::acos(e.dot(r3) / (e.length() * r3.length()));

        Orbit {
            eccentricity: e.length(),
            semi_major_axis,
            arg_periapsis,
            true_anomaly,
            mu,
        }
    }

    fn radius_at(&self, true_anomaly: f32) -> f32 {
        self.semi_major_axis * (1.0 - self.eccentricity.powi(2))
            / (1.0 + self.eccentricity * f32::cos(true_anomaly))
    }

    fn period(&self) -> Duration {
        let t = 2.0 * std::f32::consts::PI * (self.semi_major_axis.powi(3) / (self.mu)).sqrt();
        Duration::from_secs_f32(t)
    }

    fn pos(&self) -> Vec2 {
        self.position_at(self.true_anomaly)
    }

    fn vel(&self) -> Vec2 {
        self.velocity_at(self.true_anomaly)
    }

    fn position_at(&self, true_anomaly: f32) -> Vec2 {
        let r = self.radius_at(true_anomaly);
        Vec2::from_angle(true_anomaly + self.arg_periapsis) * r
    }

    fn velocity_at(&self, _true_anomaly: f32) -> Vec2 {
        todo!()
    }

    fn periapsis(&self) -> Vec2 {
        self.position_at(0.0)
    }

    fn apoapsis(&self) -> Vec2 {
        self.position_at(std::f32::consts::PI)
    }
}

#[derive(Component, Copy, Clone, Debug)]
#[require(Propagator)]
struct Body {
    radius: f32,
    mass: f32,
    soi: f32,
}

fn gravity_accel(body: Body, body_center: Vec2, sample: Vec2) -> Vec2 {
    let r: Vec2 = body_center - sample;
    let rsq = r.length_squared().clamp(body.radius.powi(2), std::f32::MAX);
    let a = GRAVITATIONAL_CONSTANT * body.mass / rsq;
    a * r.normalize()
}

#[derive(Component, Clone)]
#[require(Propagator)]
struct Orbiter {}

#[derive(Component, Clone, Copy)]
struct Collision {
    pos: Vec2,
    relative_to: Entity,
    expiry_time: f32,
}

// DRAWING STUFF

fn draw_orbit(orb: Orbit, gizmos: &mut Gizmos, alpha: f32) {
    if orb.eccentricity >= 1.0 {
        let n_points = 30;
        let theta_inf = f32::acos(-1.0 / orb.eccentricity);
        let points: Vec<_> = (-n_points..n_points)
            .map(|i| 0.98 * theta_inf * i as f32 / n_points as f32)
            .map(|t| orb.position_at(t))
            .collect();
        gizmos.linestrip_2d(points, Srgba { alpha, ..RED })
    }

    let color = Srgba { alpha, ..GRAY };

    let b = orb.semi_major_axis * (1.0 - orb.eccentricity.powi(2)).sqrt();
    let center = (orb.periapsis() + orb.apoapsis()) / 2.0;
    let iso = Isometry2d::new(center, orb.arg_periapsis.into());
    gizmos
        .ellipse_2d(iso, Vec2::new(orb.semi_major_axis, b), color)
        .resolution(orb.semi_major_axis.clamp(3.0, 200.0) as u32);
}

impl Body {
    fn earth() -> (Self, Propagator) {
        (
            Body {
                radius: 63.0,
                mass: 1000.0,
                soi: 15000.0,
            },
            Propagator::Fixed((0.0, 0.0).into()),
        )
    }

    fn luna() -> (Self, Propagator) {
        (
            Body {
                radius: 22.0,
                mass: 10.0,
                soi: 800.0,
            },
            Propagator::NBody(NBodyPropagator {
                epoch: Duration::default(),
                pos: (-3800.0, 0.0).into(),
                vel: (0.0, -58.0).into(),
            }),
        )
    }
}

pub struct PlanetaryPlugin;

impl Plugin for PlanetaryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_planet);
        app.add_systems(
            Update,
            (
                draw_orbiters,
                draw_collisions,
                draw_planets,
                keyboard_input,
                scroll_events,
                rotate_camera,
                draw_all_propagators,
            ),
        );
        app.add_systems(
            FixedUpdate,
            (
                propagate_bodies,
                propagate_orbiters,
                collide_orbiters,
                update_collisions,
                breed_orbiters,
                despawn_escaped,
                update_sim_time,
            ),
        );
    }
}

const MAX_ORBITERS: usize = 10;

#[derive(Resource)]
struct PlanetaryConfig {
    sim_speed: f32,
    show_orbits: bool,
    paused: bool
}

impl Default for PlanetaryConfig {
    fn default() -> Self {
        PlanetaryConfig {
            sim_speed: 15.0,
            show_orbits: false,
            paused: false
        }
    }
}

fn spawn_planet(mut commands: Commands) {
    commands.spawn(Body::luna());
    commands.insert_resource(PlanetaryConfig::default());
    commands.insert_resource(SimTime::default());
    let e = commands.spawn(Body::earth()).id();

    commands.spawn((
        Orbiter {},
        Propagator::Kepler(KeplerPropagator {
            epoch: Duration::default(),
            primary: e,
            orbit: Orbit {
                eccentricity: 0.5,
                semi_major_axis: 2000.0,
                arg_periapsis: 0.2,
                true_anomaly: 0.4,
                mu: Body::earth().0.mass * GRAVITATIONAL_CONSTANT,
            },
        }),
    ));

    commands.spawn((
        Orbiter {},
        Propagator::NBody(NBodyPropagator {
            epoch: Duration::default(),
            pos: (400.0, 600.0).into(),
            vel: (-100.0, 30.0).into(),
        }),
    ));
}

fn draw_planets(mut gizmos: Gizmos, query: Query<(&Propagator, &Body)>) {
    for (prop, body) in query.iter() {
        let iso = Isometry2d::from_translation(prop.pos());
        gizmos.circle_2d(iso, body.radius, WHITE);
        gizmos.circle_2d(
            iso,
            body.soi,
            Srgba {
                alpha: 0.3,
                ..ORANGE
            },
        );

        let earth = Body::earth().0;
        let orb: Orbit = Orbit::from_pv(prop.pos(), prop.vel(), earth.mass);
        draw_orbit(orb, &mut gizmos, 0.6);
    }
}

fn draw_all_propagators(mut gizmos: Gizmos, query: Query<&Propagator>) {
    for prop in query.iter() {
        gizmos.circle_2d(
            Isometry2d::from_translation(prop.pos()),
            120.0,
            Srgba { alpha: 0.05, ..RED },
        );
    }
}

fn draw_orbiters(
    mut gizmos: Gizmos,
    query: Query<&Propagator, With<Orbiter>>,
    pq: Query<&Body>,
    config: Res<PlanetaryConfig>,
) {
    let planets: Vec<&Body> = pq.iter().collect();

    for prop in query.iter() {
        match prop {
            Propagator::Fixed(p) => {
                let color: Srgba = RED;
                let iso: Isometry2d = Isometry2d::from_translation(*p);
                gizmos.circle_2d(iso, 20.0, color);
            }
            Propagator::NBody(nb) => {
                let color: Srgba = WHITE;
                let iso: Isometry2d = Isometry2d::from_translation(nb.pos);
                gizmos.circle_2d(iso, 12.0, color);
                if config.show_orbits {
                    let orb = Orbit::from_pv(nb.pos, nb.vel, Body::earth().0.mass);
                    draw_orbit(orb, &mut gizmos, 0.03);
                }
            }
            Propagator::Kepler(k) => {
                let color: Srgba = ORANGE;
                let iso: Isometry2d = Isometry2d::from_translation(prop.pos());
                gizmos.circle_2d(iso, 12.0, color);
                if config.show_orbits {
                    draw_orbit(k.orbit, &mut gizmos, 0.2);
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct KeplerPropagator {
    epoch: Duration,
    primary: Entity,
    orbit: Orbit,
}

impl KeplerPropagator {
    fn from_pv(epoch: Duration, pos: Vec2, vel: Vec2, body: Body, e: Entity) -> Self {
        let orbit = Orbit::from_pv(pos, vel, body.mass);
        KeplerPropagator {
            epoch,
            primary: e,
            orbit,
        }
    }

    fn propagate_to(&mut self, epoch: Duration)
    {
        let delta = epoch - self.epoch;

        let period = self.orbit.period();

        let revs = delta.as_secs_f32() / period.as_secs_f32();
        let du = revs * 2.0 * std::f32::consts::PI;
        self.orbit.true_anomaly += du;
        self.epoch = epoch;
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct NBodyPropagator {
    epoch: Duration,
    pos: Vec2,
    vel: Vec2,
}

impl NBodyPropagator {
    fn propagate_to(&mut self, bodies: &[(Vec2, Body)], epoch: Duration) {
        let delta_time = epoch - self.epoch;
        let dt = delta_time.as_secs_f32();

        let steps_per_minute = self.vel.length().clamp(2.0, 2000.0);
        let steps = (steps_per_minute * dt).clamp(5.0, 10000.0) as u32;

        (0..steps).for_each(|_| {
            let a: Vec2 = bodies
                .iter()
                .map(|(c, b)| -> Vec2 { gravity_accel(*b, *c, self.pos) })
                .sum();

            self.vel += a * dt / steps as f32;
            self.pos += self.vel * dt / steps as f32;
        });

        self.epoch = epoch;
    }
}

#[derive(Component, Debug, Copy, Clone)]
enum Propagator {
    Fixed(Vec2),
    NBody(NBodyPropagator),
    Kepler(KeplerPropagator),
}

impl Default for Propagator {
    fn default() -> Self {
        Propagator::Fixed(Vec2::ZERO)
    }
}

impl Propagator {
    fn fixed_at(pos: Vec2) -> Self {
        Propagator::Fixed(pos)
    }

    fn epoch(&self) -> Duration {
        match self {
            Propagator::NBody(nb) => nb.epoch,
            Propagator::Kepler(_) => todo!(),
            Propagator::Fixed(_) => Duration::default(),
        }
    }

    fn pos(&self) -> Vec2 {
        match self {
            Propagator::NBody(nb) => nb.pos,
            Propagator::Kepler(k) => k.orbit.pos(),
            Propagator::Fixed(p) => *p,
        }
    }

    fn vel(&self) -> Vec2 {
        match self {
            Propagator::NBody(nb) => nb.vel,
            Propagator::Kepler(k) => k.orbit.vel(),
            Propagator::Fixed(_) => Vec2::ZERO,
        }
    }
}

fn update_sim_time(time: Res<Time>, mut simtime: ResMut<SimTime>, config: Res<PlanetaryConfig>) {
    if config.paused {
        return;
    }
    let SimTime(dur) = simtime.as_mut();
    *dur = *dur + Duration::from_nanos((time.delta().as_nanos() as f32 * config.sim_speed) as u64);
}

fn propagate_orbiters(
    time: Res<SimTime>,
    mut pq: Query<&mut Propagator, Without<Body>>,
    bq: Query<(&Propagator, &Body)>,
) {
    let SimTime(t) = *time;
    let bodies: Vec<(Vec2, Body)> = bq.into_iter().map(|(p, b)| (p.pos(), *b)).collect();

    pq.iter_mut()
        .for_each(|mut p: Mut<'_, Propagator>| match p.as_mut() {
            Propagator::Kepler(k) => {
                k.propagate_to(t);
            }
            Propagator::NBody(nb) => {
                nb.propagate_to(&bodies, t);
            }
            &mut Propagator::Fixed(_) => (),
        });
}

fn propagate_bodies(time: Res<SimTime>, mut query: Query<(&mut Propagator, &Body)>) {
    let SimTime(t) = *time;

    let bodies: Vec<(Vec2, Body)> = query.into_iter().map(|(p, b)| (p.pos(), *b)).collect();

    query.iter_mut().for_each(|(mut current_prop, _)| {
        let other_bodies = bodies
            .clone()
            .into_iter()
            .filter(|(p, _)| p.distance(current_prop.pos()) > 0.0)
            .collect::<Vec<_>>();

        match current_prop.as_mut() {
            Propagator::NBody(nb) => nb.propagate_to(&other_bodies, t),
            Propagator::Fixed(_) => (),
            Propagator::Kepler(k) => k.propagate_to(t),
        }
    })
}

fn collide_orbiters(
    time: Res<Time>,
    mut commands: Commands,
    oq: Query<(Entity, &Propagator), With<Orbiter>>,
    pq: Query<(Entity, &Propagator, &Body), Without<Orbiter>>,
) {
    let bodies: Vec<(Entity, Vec2, Body)> =
        pq.into_iter().map(|(e, p, b)| (e, p.pos(), *b)).collect();

    for (oe, prop) in oq.iter() {
        if let Some((hit_entity, hit_entity_pos)) = bodies
            .iter()
            .map(|(pe, c, b)| {
                let r = prop.pos() - c;
                if r.length_squared() < b.radius * b.radius {
                    return Some((pe, c));
                }
                None
            })
            .filter(|e| e.is_some())
            .map(|e| e.unwrap())
            .next()
        {
            commands.entity(oe).despawn();
            commands.spawn(Collision {
                pos: prop.pos() - hit_entity_pos,
                relative_to: *hit_entity,
                expiry_time: time.elapsed_secs() + 3.0,
            });
        }
    }
}

fn breed_orbiters(mut commands: Commands, query: Query<&Propagator, With<Orbiter>>) {
    // if query.iter().count() > MAX_ORBITERS {
    //     return;
    // }

    // for prop in query.iter() {
    //     if rand(0.0, 1.0) < 0.1 {
    //         let new_prop = Propagator::NBody(NBodyPropagator {
    //             epoch: Duration::default(),
    //             pos: prop.pos(),
    //             vel: Vec2::from_angle(rand(-0.12, 0.12)).rotate(prop.vel()),
    //         });
    //         commands.spawn((new_prop, Orbiter {}));
    //     }
    // }
}

fn draw_collisions(
    mut gizmos: Gizmos,
    query: Query<&Collision>,
    planets: Query<&Propagator, With<Body>>,
) {
    for col in query.iter() {
        if let Some(p) = planets.get(col.relative_to).ok() {
            let s = 9.0;
            let ne = Vec2::new(s, s);
            let nw = Vec2::new(-s, s);
            let p = p.pos() + col.pos;
            gizmos.line_2d(p - ne, p + ne, RED);
            gizmos.line_2d(p - nw, p + nw, RED);
        }
    }
}

fn update_collisions(mut commands: Commands, time: Res<Time>, query: Query<(Entity, &Collision)>) {
    let t = time.elapsed_secs();
    for (e, col) in query.iter() {
        if col.expiry_time < t {
            commands.entity(e).despawn();
        }
    }
}

fn despawn_escaped(mut commands: Commands, query: Query<(Entity, &Propagator), With<Orbiter>>) {
    for (e, prop) in query.iter() {
        if prop.pos().length() > 15000.0 {
            commands.entity(e).despawn();
        }
    }
}

fn keyboard_input(keys: Res<ButtonInput<KeyCode>>, mut config: ResMut<PlanetaryConfig>) {
    for key in keys.get_pressed() {
        match key {
            KeyCode::ArrowDown | KeyCode::KeyS => {
                config.sim_speed = f32::clamp(config.sim_speed - 0.01, 0.0, 200.0);
                dbg!(config.sim_speed);
            }
            KeyCode::ArrowUp | KeyCode::KeyW => {
                config.sim_speed = f32::clamp(config.sim_speed + 0.01, 0.0, 200.0);
                dbg!(config.sim_speed);
            }
            KeyCode::ArrowLeft | KeyCode::KeyA => {
                config.sim_speed = f32::clamp(config.sim_speed - 1.0, 0.0, 200.0);
                dbg!(config.sim_speed);
            }
            KeyCode::ArrowRight | KeyCode::KeyD => {
                config.sim_speed = f32::clamp(config.sim_speed + 1.0, 0.0, 200.0);
                dbg!(config.sim_speed);
            }
            _ => {
                dbg!(key);
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyO) {
        config.show_orbits = !config.show_orbits;
    }
    if keys.just_pressed(KeyCode::Space) {
        config.paused = !config.paused;
    }
}

fn scroll_events(
    mut evr_scroll: EventReader<MouseWheel>,
    mut transforms: Query<&mut Transform, With<Camera>>,
) {
    use bevy::input::mouse::MouseScrollUnit;

    let mut transform = transforms.single_mut();

    for ev in evr_scroll.read() {
        match ev.unit {
            MouseScrollUnit::Line => {
                if ev.y > 0.0 {
                    transform.scale /= 1.1;
                } else {
                    transform.scale *= 1.1;
                }
            }
            _ => (),
        }
    }
}

fn rotate_camera(
    query: Query<(&Body, &Propagator), With<Body>>,
    mut transforms: Query<&mut Transform, With<Camera>>,
) {
    // let mut transform = transforms.single_mut();

    // if let Some(moon) = query
    //     .iter()
    //     .filter(|(b, _)| b.mass == Body::luna().0.mass)
    //     .next()
    // {
    //     let angle: f32 = moon.1.pos().to_angle();
    //     let quat = Quat::from_rotation_z(angle);
    //     *transform = transform.with_rotation(quat)
    // }
}
