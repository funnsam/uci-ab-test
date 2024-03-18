use std::sync::*;

pub fn update(r_a: &Mutex<f32>, r_b: &Mutex<f32>, s_a: f32, s_b: f32) -> (f32, f32, f32, f32) {
    println!("plock");
    let mut r_a = r_a.lock().unwrap();
    println!("alock");
    let mut r_b = r_b.try_lock().unwrap();

    println!("block");

    let a = *r_a;
    let b = *r_b;

    let q_a = 10.0_f32.powf(a / 400.0);
    let q_b = 10.0_f32.powf(b / 400.0);
    let e_a = q_a / (q_a + q_b);
    let e_b = q_b / (q_a + q_b);

    let k_a = 2.0 + 36.0 / (1.0 + 2.0_f32.powf((a - 1500.0) / 63.0));
    let k_b = 2.0 + 36.0 / (1.0 + 2.0_f32.powf((b - 1500.0) / 63.0));

    println!("f");

    let new_a = a + k_a * (s_a - e_a);
    let new_b = b + k_b * (s_b - e_b);

    *r_a = new_a;
    *r_b = new_b;

    core::mem::drop(r_a);
    core::mem::drop(r_b);

    println!("a");
    (a, b, new_a, new_b)
}
