use crate::inventory::InventoryItem;
use crate::orbits::SparseOrbit;
use crate::planning::*;
use crate::pv::PV;
use crate::scenario::*;
use crate::vehicle::Vehicle;
use crate::{nanotime::Nanotime, orbits::GlobalOrbit};
use serde::{Deserialize, Serialize};

use glam::f32::Vec2;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Hash)]
pub struct OrbiterId(pub i64);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Hash)]
pub struct PlanetId(pub i64);

#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Hash)]
pub struct GroupId(pub String);

impl std::fmt::Display for OrbiterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

impl std::fmt::Debug for OrbiterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

impl std::fmt::Display for PlanetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl std::fmt::Debug for PlanetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl std::fmt::Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, ":{}", self.0)
    }
}

impl std::fmt::Debug for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, ":{}", self.0)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Orbiter {
    id: OrbiterId,
    max_fuel_mass: f32,
    dry_mass: f32,
    exhaust_velocity: f32,
    props: Vec<Propagator>,

    pub vehicle: Vehicle,
}

fn rocket_equation(ve: f32, m0: f32, m1: f32) -> f32 {
    ve * (m0 / m1).ln()
}

fn mass_after_maneuver(ve: f32, m0: f32, dv: f32) -> f32 {
    m0 / (dv / ve).exp()
}

impl std::fmt::Display for Orbiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {:0.1}kg/{:0.1}kg, ve={:0.1}m/s, dv={:0.2}m/s, {} props",
            self.id,
            self.fuel_mass() + self.dry_mass,
            self.dry_mass,
            self.exhaust_velocity,
            self.remaining_dv(),
            self.props.len()
        )
    }
}

impl Orbiter {
    pub fn new(id: OrbiterId, orbit: GlobalOrbit, stamp: Nanotime) -> Self {
        Orbiter {
            id,
            max_fuel_mass: 800.0,
            dry_mass: 300.0,
            exhaust_velocity: 1700.0,
            props: vec![Propagator::new(orbit, stamp)],
            vehicle: Vehicle::random(stamp),
        }
    }

    pub fn id(&self) -> OrbiterId {
        self.id
    }

    pub fn fuel_mass(&self) -> f32 {
        self.vehicle.inventory.count(InventoryItem::LiquidFuel) as f32 / 1000.0
    }

    pub fn mass(&self) -> f32 {
        self.dry_mass + self.fuel_mass()
    }

    pub fn remaining_dv(&self) -> f32 {
        rocket_equation(self.exhaust_velocity, self.mass(), self.dry_mass)
    }

    pub fn fuel_percentage(&self) -> f32 {
        self.fuel_mass() / self.max_fuel_mass
    }

    pub fn low_fuel(&self) -> bool {
        self.vehicle.is_controllable() && self.remaining_dv() < 10.0
    }

    pub fn add_fuel(&mut self, kg: u64) {
        self.vehicle
            .inventory
            .add(InventoryItem::LiquidFuel, kg * 1000);
    }

    pub fn step(&mut self, stamp: Nanotime) {
        self.vehicle.step(stamp)
    }

    pub fn impulsive_burn(&mut self, stamp: Nanotime, dv: Vec2) -> Option<()> {
        if dv.length() > self.remaining_dv() {
            return None;
        }

        let fuel_mass_before_maneuver = self.fuel_mass();
        let m1 = mass_after_maneuver(self.exhaust_velocity, self.mass(), dv.length());
        let fuel_mass_after_maneuver = m1 - self.dry_mass;
        let spent_fuel = fuel_mass_before_maneuver - fuel_mass_after_maneuver;

        self.vehicle.inventory.take(
            InventoryItem::LiquidFuel,
            (spent_fuel * 1000.0).round() as u64,
        );

        let orbit: GlobalOrbit = {
            let prop = self.propagator_at(stamp)?;
            let pv = prop.pv_universal(stamp)? + PV::vel(dv);
            let orbit = SparseOrbit::from_pv(pv, prop.orbit.1.body, stamp)?;
            GlobalOrbit(prop.parent(), orbit)
        };
        self.props.clear();
        let new_prop = Propagator::new(orbit, stamp);
        self.props.push(new_prop);
        Some(())
    }

    pub fn pv(&self, stamp: Nanotime, planets: &PlanetarySystem) -> Option<PV> {
        let prop = self.propagator_at(stamp)?;
        let (_, pv, _, _) = planets.lookup(prop.parent(), stamp)?;
        Some(prop.pv(stamp)? + pv)
    }

    pub fn pvl(&self, stamp: Nanotime) -> Option<PV> {
        let prop = self.propagator_at(stamp)?;
        Some(prop.pv(stamp)?)
    }

    pub fn propagator_at(&self, stamp: Nanotime) -> Option<&Propagator> {
        self.props.iter().find(|p| p.is_active(stamp))
    }

    pub fn props(&self) -> &Vec<Propagator> {
        &self.props
    }

    pub fn orbit(&self, stamp: Nanotime) -> Option<&GlobalOrbit> {
        let prop = self.propagator_at(stamp)?;
        Some(&prop.orbit)
    }

    pub fn will_collide(&self) -> bool {
        self.props.iter().any(|p| match p.horizon {
            HorizonState::Terminating(_, EventType::Collide(_)) => true,
            _ => false,
        })
    }

    pub fn will_change(&self) -> bool {
        self.props
            .first()
            .map(|p| p.horizon.is_change())
            .unwrap_or(false)
    }

    pub fn is_indefinitely_stable(&self) -> bool {
        self.props.iter().any(|p| p.is_indefinite())
    }

    pub fn has_error(&self) -> bool {
        self.props.iter().any(|p| p.is_err())
    }

    pub fn propagate_to(
        &mut self,
        stamp: Nanotime,
        future_dur: Nanotime,
        planets: &PlanetarySystem,
    ) -> Result<(), PredictError<Nanotime>> {
        while self.props.len() > 1 && self.props[0].end().unwrap_or(stamp) < stamp {
            self.props.remove(0);
        }

        let t = stamp + future_dur;

        let max_iters = 10;

        for _ in 0..max_iters {
            let prop = self.props.iter_mut().last().ok_or(PredictError::Lookup)?;

            let (_, _, _, pl) = planets
                .lookup(prop.parent(), stamp)
                .ok_or(PredictError::Lookup)?;
            let bodies = pl
                .subsystems
                .iter()
                .map(|(orbit, pl)| (pl.id, orbit, pl.body.soi))
                .collect::<Vec<_>>();

            prop.finish_or_compute_until(t, &bodies)?;

            let (end, prop_finished) = match prop.horizon {
                HorizonState::Continuing(end) => (end, false),
                HorizonState::Indefinite => return Ok(()),
                HorizonState::Terminating(_, _) => return Ok(()),
                HorizonState::Transition(end, _) => (end, true),
            };

            if end > t {
                return Ok(());
            }

            if prop_finished {
                match prop.next_prop(planets) {
                    Ok(None) => {
                        return Ok(());
                    }
                    Ok(Some(next)) => {
                        self.props.push(next);
                    }
                    Err(_) => {
                        let mut p = prop.clone();
                        p.start = end;
                        p.horizon = HorizonState::Terminating(end, EventType::NumericalError);
                        self.props.push(p);
                        return Ok(());
                    }
                }
            }
        }

        Err(PredictError::TooManyIterations)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectId {
    Planet(PlanetId),
    Orbiter(OrbiterId),
}

impl ObjectId {
    pub fn orbiter(&self) -> Option<OrbiterId> {
        match self {
            Self::Orbiter(id) => Some(*id),
            _ => None,
        }
    }

    pub fn planet(&self) -> Option<PlanetId> {
        match self {
            Self::Planet(id) => Some(*id),
            _ => None,
        }
    }
}

impl From<OrbiterId> for ObjectId {
    fn from(value: OrbiterId) -> Self {
        ObjectId::Orbiter(value)
    }
}

impl From<PlanetId> for ObjectId {
    fn from(value: PlanetId) -> Self {
        ObjectId::Planet(value)
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
