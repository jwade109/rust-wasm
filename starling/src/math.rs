use crate::nanotime::Nanotime;
use glam::f32::Vec2;
use rand::Rng;

pub const PI: f32 = std::f32::consts::PI;

pub fn rand(min: f32, max: f32) -> f32 {
    rand::thread_rng().gen_range(min..max)
}

pub fn randvec(min: f32, max: f32) -> Vec2 {
    let rot = Vec2::from_angle(rand(0.0, std::f32::consts::PI * 2.0));
    let mag = rand(min, max);
    rot.rotate(Vec2::new(mag, 0.0))
}

pub fn rotate(v: Vec2, angle: f32) -> Vec2 {
    Vec2::from_angle(angle).rotate(v)
}

pub fn cross2d(a: Vec2, b: Vec2) -> f32 {
    a.extend(0.0).cross(b.extend(0.0)).z
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn apply<T: Copy, R>(x: &Vec<T>, func: impl Fn(T) -> R) -> Vec<R> {
    x.iter().map(|x| func(*x)).collect()
}

pub fn linspace(a: f32, b: f32, n: usize) -> Vec<f32> {
    if n < 2 {
        return vec![a];
    }
    if n == 2 {
        return vec![a, b];
    }
    (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            lerp(a, b, t)
        })
        .collect()
}

pub fn tspace(start: Nanotime, end: Nanotime, nsamples: u32) -> Vec<Nanotime> {
    let dt = (end - start) / nsamples as i64;
    (0..nsamples).map(|i| start + dt * i as i64).collect()
}

pub fn bhaskara_sin_approx(x: f32) -> f32 {
    let xp = x.abs();
    x.signum() * 16.0 * xp * (PI - xp) / (5.0 * PI.powi(2) - 4.0 * xp * (PI - xp))
}
