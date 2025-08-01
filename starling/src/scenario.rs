use crate::entities::*;
use crate::id::*;
use crate::nanotime::Nanotime;
use crate::orbiter::*;
use crate::orbits::{Body, SparseOrbit};
use crate::propagator::EventType;
use crate::pv::PV;
use glam::f32::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct ObjectIdTracker(EntityId, EntityId);

impl ObjectIdTracker {
    pub fn new() -> Self {
        ObjectIdTracker(EntityId(0), EntityId(0))
    }

    pub fn next(&mut self) -> EntityId {
        let ret = self.0;
        self.0 .0 += 1;
        ret
    }

    pub fn next_planet(&mut self) -> EntityId {
        let ret = self.1;
        self.1 .0 += 1;
        ret
    }
}

#[derive(Debug, Clone)]
pub enum ScenarioObject<'a> {
    Orbiter(&'a Orbiter),
    Body(&'a String, Body),
}

#[derive(Debug, Clone)]
pub struct ObjectLookup<'a>(pub EntityId, pub ScenarioObject<'a>, pub PV);

impl<'a> ObjectLookup<'a> {
    pub fn id(&self) -> EntityId {
        self.0
    }

    pub fn pv(&self) -> PV {
        self.2
    }

    pub fn orbiter(&self) -> Option<&'a Orbiter> {
        match self.1 {
            ScenarioObject::Orbiter(o) => Some(o),
            _ => None,
        }
    }

    pub fn parent(&self, stamp: Nanotime) -> Option<EntityId> {
        let orbiter = self.orbiter()?;
        let prop = orbiter.propagator_at(stamp)?;
        Some(prop.parent())
    }

    pub fn body(&self) -> Option<Body> {
        match self.1 {
            ScenarioObject::Body(_, b) => Some(b),
            _ => None,
        }
    }

    pub fn named_body(&self) -> Option<(&'a String, Body)> {
        match self.1 {
            ScenarioObject::Body(s, b) => Some((s, b)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanetarySystem {
    pub id: EntityId,
    pub name: String,
    pub body: Body,
    pub subsystems: Vec<(SparseOrbit, PlanetarySystem)>,
}

impl PlanetarySystem {
    pub fn new(id: EntityId, name: impl Into<String>, body: Body) -> Self {
        PlanetarySystem {
            id,
            name: name.into(),
            body,
            subsystems: vec![],
        }
    }

    pub fn orbit(&mut self, orbit: SparseOrbit, planets: PlanetarySystem) {
        self.subsystems.push((orbit, planets));
    }

    pub fn planet_ids(&self) -> Vec<EntityId> {
        let mut ret = vec![self.id];
        for (_, sub) in &self.subsystems {
            ret.extend_from_slice(&sub.planet_ids())
        }
        ret
    }

    pub fn bodies<T: Into<Option<PV>>>(
        &self,
        stamp: Nanotime,
        origin: T,
    ) -> impl Iterator<Item = (PV, Body)> + use<'_, T> {
        let origin = origin.into().unwrap_or(PV::ZERO);
        let mut ret = vec![(origin, self.body)];
        for (orbit, sys) in &self.subsystems {
            if let Ok(pv) = orbit.pv(stamp) {
                let r = sys.bodies(stamp, pv);
                ret.extend(r);
            }
        }
        ret.into_iter()
    }

    fn lookup_inner(
        &self,
        id: EntityId,
        stamp: Nanotime,
        wrt: PV,
        parent_id: Option<EntityId>,
    ) -> Option<(Body, PV, Option<EntityId>, &PlanetarySystem)> {
        if self.id == id {
            return Some((self.body, wrt, parent_id, self));
        }

        for (orbit, pl) in &self.subsystems {
            if let Some(pv) = orbit.pv(stamp).ok() {
                let ret = pl.lookup_inner(id, stamp, wrt + pv, Some(self.id));
                if let Some(r) = ret {
                    return Some(r);
                }
            }
        }

        None
    }

    pub fn lookup(
        &self,
        id: EntityId,
        stamp: Nanotime,
    ) -> Option<(Body, PV, Option<EntityId>, &PlanetarySystem)> {
        self.lookup_inner(id, stamp, PV::ZERO, None)
    }

    pub fn potential_at(&self, pos: Vec2, stamp: Nanotime) -> f32 {
        let r = pos.length().clamp(10.0, std::f32::MAX);
        let mut ret = -self.body.mu() / r;
        for (orbit, pl) in &self.subsystems {
            if let Some(pv) = orbit.pv(stamp).ok() {
                ret += pl.potential_at(pos - pv.pos_f32(), stamp);
            }
        }
        ret
    }
}

#[derive(Debug, Clone)]
pub struct RemovalInfo {
    pub stamp: Nanotime,
    pub reason: EventType,
    pub parent: EntityId,
    pub orbit: SparseOrbit,
}

impl RemovalInfo {
    pub fn pv(&self) -> Option<PV> {
        self.orbit.pv(self.stamp).ok()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Scenario {
    pub orbiters: HashMap<EntityId, Orbiter>,
    pub planets: PlanetarySystem,
}

pub fn simulate(
    orbiters: &mut HashMap<EntityId, OrbitalSpacecraftEntity>,
    planets: &PlanetarySystem,
    stamp: Nanotime,
    future_dur: Nanotime,
) -> Vec<(EntityId, RemovalInfo)> {
    for (_, ov) in orbiters.iter_mut() {
        let e = ov.orbiter.propagate_to(stamp, future_dur, planets);
        if let Err(_e) = e {
            // dbg!(e);
        }
    }

    let mut info = vec![];

    orbiters.retain(|id, ov| {
        if ov.orbiter.propagator_at(stamp).is_none() {
            let reason = ov.orbiter.props().last().map(|p| RemovalInfo {
                stamp: p.end().unwrap_or(stamp),
                reason: p.event().unwrap_or(EventType::NumericalError),
                parent: p.parent(),
                orbit: p.orbit.1,
            });
            if let Some(reason) = reason {
                info.push((*id, reason));
            }
            false
        } else {
            true
        }
    });

    info
}
