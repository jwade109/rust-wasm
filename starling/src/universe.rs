use crate::control_signals::ControlSignals;
use crate::prelude::*;
use std::collections::{HashMap, HashSet};

pub struct Universe {
    stamp: Nanotime,
    ticks: u128,
    next_entity_id: EntityId,
    pub orbiters: HashMap<EntityId, Orbiter>,
    pub vehicles: HashMap<EntityId, Vehicle>,
    pub surface_vehicles: HashMap<EntityId, (RigidBody, VehicleController, Vehicle)>,
    pub planets: PlanetarySystem,
    pub surface: Surface,
    pub landing_sites: HashMap<EntityId, Vec<(f32, String, Surface)>>,
    pub constellations: HashMap<EntityId, EntityId>,
}

fn generate_landing_sites(pids: &[EntityId]) -> HashMap<EntityId, Vec<(f32, String, Surface)>> {
    pids.iter()
        .map(|pid| {
            let n = randint(3, 12);
            let sites: Vec<_> = (0..n)
                .map(|_| {
                    let angle = rand(0.0, 2.0 * PI);
                    let name = get_random_name();
                    (angle, name, Surface::random())
                })
                .collect();
            (*pid, sites)
        })
        .collect()
}

impl Universe {
    pub fn new(planets: PlanetarySystem) -> Self {
        Self {
            stamp: Nanotime::zero(),
            ticks: 0,
            next_entity_id: EntityId(0),
            orbiters: HashMap::new(),
            vehicles: HashMap::new(),
            surface_vehicles: HashMap::new(),
            planets: planets.clone(),
            surface: Surface::random(),
            landing_sites: generate_landing_sites(&planets.planet_ids()),
            constellations: HashMap::new(),
        }
    }

    pub fn stamp(&self) -> Nanotime {
        self.stamp
    }

    pub fn ticks(&self) -> u128 {
        self.ticks
    }

    fn next_entity_id(&mut self) -> EntityId {
        let ret = self.next_entity_id;
        self.next_entity_id.0 += 1;
        ret
    }

    pub fn on_sim_ticks(&mut self, ticks: u32, signals: &ControlSignals) {
        (0..ticks).for_each(|_| self.on_sim_tick(signals));
    }

    pub fn on_sim_tick(&mut self, signals: &ControlSignals) {
        self.ticks += 1;
        self.stamp += PHYSICS_CONSTANT_DELTA_TIME;

        // for (_, orbiter) in &mut self.orbiters {
        //     orbiter.on_sim_tick();
        // }

        // for (_, vehicle) in &mut self.vehicles {
        //     vehicle.on_sim_tick();
        // }

        let gravity = self.surface.external_acceleration();

        for (_, (body, controller, vehicle)) in &mut self.surface_vehicles {
            let ext = signals.piloting.unwrap_or(VehicleControl::NULLOPT);

            let ctrl = match (controller.mode(), controller.get_target_pose()) {
                (VehicleControlPolicy::Idle, _) => VehicleControl::NULLOPT,
                (VehicleControlPolicy::External, _) => ext,
                (VehicleControlPolicy::PositionHold, Some(pose)) => {
                    position_hold_control_law(pose, body, vehicle, gravity)
                }
                (_, _) => VehicleControl::NULLOPT,
            };

            let elevation = self.surface.elevation(body.pv.pos_f32().x);

            if controller.mode() != VehicleControlPolicy::Idle {
                controller.check_target_achieved(body, gravity.length() > 0.0);
                vehicle.set_thrust_control(ctrl);
                vehicle.on_sim_tick();
            }

            if controller.mode() != VehicleControlPolicy::Idle || body.pv.pos_f32().y > elevation {
                let accel = vehicle.body_frame_accel();
                body.on_sim_tick(
                    accel,
                    self.surface.external_acceleration(),
                    PHYSICS_CONSTANT_DELTA_TIME,
                );
            } else {
                vehicle.set_thrust_control(VehicleControl::NULLOPT);
            }

            body.on_the_floor(elevation);
        }

        self.surface.on_sim_tick();

        self.constellations
            .retain(|id, _| self.orbiters.contains_key(id));
    }

    pub fn get_group_members(&mut self, gid: EntityId) -> Vec<EntityId> {
        self.constellations
            .iter()
            .filter_map(|(id, g)| (*g == gid).then(|| *id))
            .collect()
    }

    pub fn group_membership(&self, id: &EntityId) -> Option<EntityId> {
        self.constellations.get(id).cloned()
    }

    pub fn unique_groups(&self) -> Vec<EntityId> {
        let mut s: Vec<EntityId> = self
            .constellations
            .iter()
            .map(|(_, gid)| *gid)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        s.sort();
        s
    }

    pub fn orbiter_ids(&self) -> impl Iterator<Item = EntityId> + use<'_> {
        self.orbiters.keys().into_iter().map(|id| *id)
    }

    pub fn add_surface_vehicle(&mut self, vehicle: Vehicle, pos: Vec2) {
        let target = pos + randvec(10.0, 20.0);
        let vel = randvec(2.0, 7.0);

        let angle = rand(0.0, PI);

        let body = RigidBody {
            pv: PV::from_f64(pos, vel),
            angle: PI / 2.0,
            angular_velocity: 0.0,
        };

        let pose: Pose = (target, angle);

        let controller = VehicleController::position_hold(pose);
        let id = self.next_entity_id();
        self.surface_vehicles
            .insert(id, (body, controller, vehicle));
    }

    pub fn lup_orbiter(&self, id: EntityId, stamp: Nanotime) -> Option<ObjectLookup> {
        let orbiter = self.orbiters.get(&id)?;
        let prop = orbiter.propagator_at(stamp)?;
        let (_, frame_pv, _, _) = self.planets.lookup(prop.parent(), stamp)?;
        let local_pv = prop.pv(stamp)?;
        let pv = frame_pv + local_pv;
        Some(ObjectLookup(id, ScenarioObject::Orbiter(orbiter), pv))
    }

    pub fn lup_planet(&self, id: EntityId, stamp: Nanotime) -> Option<ObjectLookup> {
        let (body, pv, _, sys) = self.planets.lookup(id, stamp)?;
        Some(ObjectLookup(id, ScenarioObject::Body(&sys.name, body), pv))
    }
}
