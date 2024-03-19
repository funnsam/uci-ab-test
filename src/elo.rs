use std::sync::*;

pub fn update(r: &Mutex<(f32, f32)>, s_a: f32, s_b: f32) -> (f32, f32, f32, f32) {
    println!("plock");
    let mut r = r.lock().unwrap();
    println!("lock");

    let a = r.0;
    let b = r.1;

    let q_a = 10.0_f32.powf(a / 400.0);
    let q_b = 10.0_f32.powf(b / 400.0);
    let e_a = q_a / (q_a + q_b);
    let e_b = q_b / (q_a + q_b);

    let k_a = 2.0 + 36.0 / (1.0 + 2.0_f32.powf((a - 1500.0) / 63.0));
    let k_b = 2.0 + 36.0 / (1.0 + 2.0_f32.powf((b - 1500.0) / 63.0));

    println!("f");

    let new_a = a + k_a * (s_a - e_a);
    let new_b = b + k_b * (s_b - e_b);

    *r = (new_a, new_b);

    core::mem::drop(r);

    println!("a");
    (a, b, new_a, new_b)
}
