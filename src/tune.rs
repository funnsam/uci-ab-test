use std::ops::*;

const ALPHA: f32 = 0.602;
const GAMMA: f32 = 0.101;
const C: f32 = 0.5;
const MAGNITUDE: f32 = 1.0;

pub fn tune(iterations: usize, mut theta: FeautureVector<f32>) {
    let mut seed = 1;

    let ua = iterations as f32 * 0.08;
    let la = 0.1 * (ua + 1.0).powf(ALPHA) / MAGNITUDE;

    for k in 0..iterations {
        let k = k as f32;

        let ak = la / (k + 1.0 + ua).powf(ALPHA);
        let ck = C / (k + 1.0).powf(GAMMA);

        let mut delta = FeautureVector::empty_with_capacity(theta.len());
        for _ in 0..theta.len() {
            delta.push((2 * (rand(&mut seed) % 2) - 1) as f32);
        }

        let ckd = delta * ck;
        let theta_p = theta.clone() + ckd.clone();
        let theta_m = theta.clone() - ckd.clone();

        theta = theta + get_result(theta_p, theta_m)._div(ckd) * ak;
    }
}

fn get_result(a: FeautureVector<f32>, b: FeautureVector<f32>) -> f32 {
    0.0
}

#[derive(Clone)]
struct FeautureVector<T> {
    features: Vec<T>,
}

impl<T> FeautureVector<T> {
    fn empty_with_capacity(cap: usize) -> Self {
        Self {
            features: Vec::with_capacity(cap),
        }
    }
}

impl<T: Clone, R: Clone + Add<T, Output = T>> Add<FeautureVector<R>> for FeautureVector<T> {
    type Output = Self;

    fn add(mut self, rhs: FeautureVector<R>) -> Self {
        for (i, e) in self.iter_mut().enumerate() {
            *e = rhs[i].clone() + e.clone();
        }

        self
    }
}

impl<T: Clone + Sub<R, Output = T>, R: Clone> Sub<FeautureVector<R>> for FeautureVector<T> {
    type Output = Self;

    fn sub(mut self, rhs: FeautureVector<R>) -> Self {
        for (i, e) in self.iter_mut().enumerate() {
            *e = e.clone() - rhs[i].clone();
        }

        self
    }
}

impl<T: Clone, R: Clone + Mul<T, Output = T>> Mul<R> for FeautureVector<T> {
    type Output = Self;

    fn mul(mut self, rhs: R) -> Self {
        for i in self.iter_mut() {
            *i = rhs.clone() * i.clone();
        }

        self
    }
}

trait FeautureVectorOps<T> where
    Self: Clone + Sized + Div<T, Output = T>
{
    fn _div(self, mut rhs: FeautureVector<T>) -> FeautureVector<T> {
        for i in rhs.iter_mut() {
            *i = self.clone() / *i;
        }

        rhs
    }
}

impl<T: Clone + Sized + Div<I, Output = I>, I> FeautureVectorOps<I> for T {}

impl<T> Deref for FeautureVector<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Vec<T> {
        &self.features
    }
}

impl<T> DerefMut for FeautureVector<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.features
    }
}

fn rand(seed: &mut i32) -> i32 {
    let mut p = *seed as u32;
    p ^= p << 13;
    p ^= p >> 17;
    p ^= p << 5;
    *seed = p as i32;
    return *seed;
}
