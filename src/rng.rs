//! Tiny, fast xorshift64* RNG — no extra dependency needed for gameplay randomness.

use bevy::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Rng(u64);

impl Rng {
    pub fn seeded() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let mut rng = Self(seed | 1);
        // Warm up so close seeds diverge.
        for _ in 0..8 {
            rng.next_u64();
        }
        rng
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    #[inline]
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    #[inline]
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }

    /// Uniform integer in `[0, n)`.
    #[allow(dead_code)]
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next_u64() >> 32) % n.max(1) as u64) as u32
    }

    #[allow(dead_code)]
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    pub fn unit_vec3(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            );
            let l2 = v.length_squared();
            if l2 > 0.01 && l2 <= 1.0 {
                return v / l2.sqrt();
            }
        }
    }

    pub fn rotation(&mut self) -> Quat {
        let axis = self.unit_vec3();
        Quat::from_axis_angle(axis, self.range(0.0, std::f32::consts::TAU))
    }
}

