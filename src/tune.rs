use std::ops::*;

const ALPHA: f32 = 0.602;
const GAMMA: f32 = 0.101;
const C: f32 = 0.5;
const MAGNITUDE: f32 = 100.0;

pub fn tune(iterations: usize, engine: &str, mut theta: FeatureVector<f32>, fen: &[String], mut seed: i32) {
    let ua = iterations as f32 * 0.08;
    let la = 0.1 * (ua + 1.0).powf(ALPHA) / MAGNITUDE;

    for k in 0..iterations {
        let k = k as f32;

        let ak = la / (k + 1.0 + ua).powf(ALPHA);
        let ck = C / (k + 1.0).powf(GAMMA);

        let mut delta = FeatureVector::empty_with_capacity(theta.len());
        for _ in 0..theta.len() {
            delta.push((2 * (rand(&mut seed) % 2) - 1) as f32);
        }

        let ckd = delta * ck;
        let theta_p = theta.clone() + ckd.clone();
        let theta_m = theta.clone() - ckd.clone();

        theta = theta + get_result(engine, &theta_p, &theta_m, fen)._div(ckd) * ak;

        std::fs::write(format!("tune_iter_{k}.flt"), theta.to_binary()).unwrap();
        std::fs::write(format!("tune_iter_{k}.int"), Into::<FeatureVector<i32>>::into(&theta).to_binary()).unwrap();
    }
}

fn get_result(engine: &str, a: &FeatureVector<f32>, b: &FeatureVector<f32>, fen: &[String]) -> f32 {
    let a = a.into();
    let b = b.into();

    let result = 0.0;

    for f in fen.iter() {
        let mut a_engine = crate::engine::Engine::new(engine, &f);
        a_engine.send_features(&a);

        let mut b_engine = crate::engine::Engine::new(engine, &f);
        b_engine.send_features(&b);

        todo!()
    }

    result / fen.len() as f32
}

#[derive(Clone)]
pub struct FeatureVector<T> {
    pub features: Vec<T>,
}

impl<T> FeatureVector<T> {
    fn empty_with_capacity(cap: usize) -> Self {
        Self {
            features: Vec::with_capacity(cap),
        }
    }
}

macro_rules! binaries {
    ($t: ty) => {
        impl FeatureVector<$t> {
            pub fn from_binary(b: &[u8]) -> Self {
                let mut f = Vec::with_capacity(b.len() / core::mem::size_of::<$t>());

                for i in b.chunks_exact(4) {
                    f.push(<$t>::from_le_bytes(i.try_into().unwrap()));
                }

                Self { features: f }
            }

            pub fn to_binary(&self) -> Vec<u8> {
                let mut buf = Vec::with_capacity(self.len() * core::mem::size_of::<$t>());

                for f in self.iter() {
                    buf.extend(f.to_le_bytes());
                }

                buf
            }
        }
    };
}

binaries!(f32);
binaries!(i32);

impl Into<FeatureVector<i32>> for &FeatureVector<f32> {
    fn into(self) -> FeatureVector<i32> {
        let mut new = FeatureVector::empty_with_capacity(self.len());

        for i in self.iter() {
            new.push(*i as i32);
        }

        new
    }
}

impl Into<FeatureVector<f32>> for &FeatureVector<i32> {
    fn into(self) -> FeatureVector<f32> {
        let mut new = FeatureVector::empty_with_capacity(self.len());

        for i in self.iter() {
            new.push(*i as f32);
        }

        new
    }
}

impl<T: Clone, R: Clone + Add<T, Output = T>> Add<FeatureVector<R>> for FeatureVector<T> {
    type Output = Self;

    fn add(mut self, rhs: FeatureVector<R>) -> Self {
        for (i, e) in self.iter_mut().enumerate() {
            *e = rhs[i].clone() + e.clone();
        }

        self
    }
}

impl<T: Clone + Sub<R, Output = T>, R: Clone> Sub<FeatureVector<R>> for FeatureVector<T> {
    type Output = Self;

    fn sub(mut self, rhs: FeatureVector<R>) -> Self {
        for (i, e) in self.iter_mut().enumerate() {
            *e = e.clone() - rhs[i].clone();
        }

        self
    }
}

impl<T: Clone, R: Clone + Mul<T, Output = T>> Mul<R> for FeatureVector<T> {
    type Output = Self;

    fn mul(mut self, rhs: R) -> Self {
        for i in self.iter_mut() {
            *i = rhs.clone() * i.clone();
        }

        self
    }
}

trait FeautureVectorOps<T> where
    Self: Clone + Sized + Div<T, Output = T>,
    T: Clone
{
    fn _div(self, mut rhs: FeatureVector<T>) -> FeatureVector<T> {
        for i in rhs.iter_mut() {
            *i = self.clone() / i.clone();
        }

        rhs
    }
}

impl<T: Clone + Sized + Div<I, Output = I>, I: Clone> FeautureVectorOps<I> for T {}

impl<T> Deref for FeatureVector<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Vec<T> {
        &self.features
    }
}

impl<T> DerefMut for FeatureVector<T> {
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
