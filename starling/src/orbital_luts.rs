use crate::math::{apply, lerp_f64, linspace_f64, PI_64};
use crate::nanotime::Nanotime;
use crate::orbits::{Body, OrbitClass, SparseOrbit};
use lazy_static::lazy_static;
use splines::{Interpolation, Key, Spline};
use std::collections::HashMap;

const ECCENTRICITY_STEP: u8 = 1;
const N_SAMPLES: usize = 500;

#[derive(Debug, Clone)]
struct DenseOrbit(Spline<f64, f64>);

impl DenseOrbit {
    pub fn new(orbit: &SparseOrbit) -> Result<Self, &'static str> {
        let n_samples = match orbit.class() {
            OrbitClass::NearCircular => 1000,
            OrbitClass::Circular => 1000,
            OrbitClass::Elliptical => 1000,
            OrbitClass::HighlyElliptical => 1000,
            OrbitClass::Parabolic => return Err("Parabolic"),
            OrbitClass::Hyperbolic => return Err("Hyperbolic"),
            OrbitClass::VeryThin => return Err("Too thin"),
        };

        let sample_space = linspace_f64(-0.25, 1.25, n_samples);

        let period = orbit.period().ok_or("No period")?;
        let ta = orbit.t_next_p(orbit.epoch).ok_or("No next periapsis")?;
        let start = ta;
        let end = ta + period;
        let mut samples = vec![];
        let mut prev = None;

        let mut wrap_monotonic = |ta: f64| {
            let mut ta = ta;
            if let Some(prev) = prev {
                while prev > ta {
                    ta += PI_64 * 2.0;
                }
            }
            prev = Some(ta);
            ta
        };

        for s in sample_space {
            let t = start.lerp(end, s as f32);
            let ta = orbit.ta_at_time(t).ok_or("Bad true anomaly")?;
            let ta = wrap_monotonic(ta);
            samples.push(((t - start).to_secs_f64() / period.to_secs_f64(), ta));
        }

        let mut keys = vec![];
        for (i, (dt, ta)) in samples.iter().enumerate() {
            let interp = if i == 0 {
                Interpolation::Linear
            } else if i + 2 < samples.len() {
                Interpolation::CatmullRom
            } else {
                Interpolation::Linear
            };
            let key = Key::new(*dt, *ta, interp);
            keys.push(key);
        }

        let spline = Spline::<f64, f64>::from_vec(keys);

        Ok(Self(spline))
    }

    pub fn sample_normalized(&self, s: f64) -> f64 {
        self.0.sample(s).unwrap()
    }
}

fn get_orbit_with_ecc(ecc: f64) -> Vec<f64> {
    let a = 1000.0;
    let ra = a * (1.0 + ecc);
    let rp = a * (1.0 - ecc);
    let argp = 0.0;
    let body = Body {
        radius: 1.0,
        mu: 1000.0 * 12000.0,
        soi: 100000.0,
    };
    let epoch = Nanotime::zero();
    let retrograde = false;
    let orbit = SparseOrbit::new(ra, rp, argp, body, epoch, retrograde).unwrap();
    let dense = DenseOrbit::new(&orbit).unwrap();

    let s = linspace_f64(0.0, 1.0, N_SAMPLES);
    apply(&s, |s: f64| dense.sample_normalized(s) - s * 2.0 * PI_64)
}

lazy_static! {
    pub static ref BIG_ORBITS: HashMap<u8, Vec<f64>> = HashMap::from_iter(
        (0..=93)
            .step_by(ECCENTRICITY_STEP as usize)
            .map(|e| (e, get_orbit_with_ecc(e as f64 / 100.0)))
    );
}

fn fmod(a: f64, n: f64) -> f64 {
    a - n * (a / n).floor()
}

pub fn lookup_ta_from_ma(ma: f64, ecc: f64) -> Option<f64> {
    let ma = fmod(ma, 2.0 * PI_64);

    let ei = (ecc * 100.0) as u8;

    let el = ei - (ei % ECCENTRICITY_STEP);
    let eu = el + ECCENTRICITY_STEP;
    let sy = ((ecc * 100.0) - (el as f64)) / ECCENTRICITY_STEP as f64;

    let lower = BIG_ORBITS.get(&el)?;
    let upper = BIG_ORBITS.get(&eu)?;

    let x1 = ((ma / (2.0 * PI_64)) * (N_SAMPLES - 1) as f64) as usize;
    let x2 = x1 + 1;

    let ma_x1 = (x1 as f64 / (N_SAMPLES - 1) as f64) * 2.0 * PI_64;
    let ma_x2 = (x2 as f64 / (N_SAMPLES - 1) as f64) * 2.0 * PI_64;

    let sx = (ma - ma_x1) / (ma_x2 - ma_x1);

    let x1y1 = lower[x1 % N_SAMPLES];
    let x1y2 = upper[x1 % N_SAMPLES];

    let x2y1 = lower[x2 % N_SAMPLES];
    let x2y2 = upper[x2 % N_SAMPLES];

    let p1 = lerp_f64(x1y1, x1y2, sy);
    let p2 = lerp_f64(x2y1, x2y2, sy);

    Some(lerp_f64(p1, p2, sx) + ma)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_float_eq::assert_float_absolute_eq;

    #[test]
    fn lut_expected_values() {
        for ecc in linspace_f64(0.0, 0.9, 100) {
            assert_float_absolute_eq!(lookup_ta_from_ma(0.0, ecc).unwrap(), 0.0, 1E-2);
            assert_float_absolute_eq!(lookup_ta_from_ma(PI_64, ecc).unwrap(), PI_64, 1E-2);
            assert_float_absolute_eq!(lookup_ta_from_ma(2.0 * PI_64, ecc).unwrap(), 0.0, 1E-2);
        }

        for ma in linspace_f64(0.0, 1.95 * PI_64, 100) {
            assert_float_absolute_eq!(lookup_ta_from_ma(ma, 0.0).unwrap(), ma, 1E-2);
        }
    }
}
