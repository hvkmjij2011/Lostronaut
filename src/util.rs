//! Deterministic RNG + gradient noise. No external deps so results are identical on every platform.
use bevy::math::Vec3;

#[derive(Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// [0,1)
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
    pub fn int(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as u32
        }
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
    pub fn pick<'a, T>(&mut self, s: &'a [T]) -> &'a T {
        &s[self.int(s.len() as u32) as usize]
    }
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u64())
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.int(i as u32 + 1) as usize;
            v.swap(i, j);
        }
    }
    /// Uniform direction on the unit sphere.
    pub fn dir(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, std::f32::consts::TAU);
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(r * a.cos(), z, r * a.sin())
    }
}

pub fn mix64(a: u64, b: u64) -> u64 {
    let mut r = Rng::new(a ^ b.wrapping_mul(0xD6E8_FEB8_6659_FD93));
    r.next_u64()
}

#[inline]
fn hash3(x: i32, y: i32, z: i32, s: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f)
        ^ s.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    h
}

#[inline]
fn grad(h: u32, x: f32, y: f32, z: f32) -> f32 {
    match h & 15 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        11 => -y - z,
        12 => x + y,
        13 => -x + y,
        14 => -y + z,
        _ => -y - z,
    }
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Perlin gradient noise, roughly in [-1, 1].
pub fn noise3(p: Vec3, seed: u32) -> f32 {
    let xi = p.x.floor();
    let yi = p.y.floor();
    let zi = p.z.floor();
    let (x, y, z) = (p.x - xi, p.y - yi, p.z - zi);
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);
    let (u, v, w) = (fade(x), fade(y), fade(z));
    let g = |dx: i32, dy: i32, dz: i32| {
        grad(
            hash3(xi + dx, yi + dy, zi + dz, seed),
            x - dx as f32,
            y - dy as f32,
            z - dz as f32,
        )
    };
    let x00 = lerp(g(0, 0, 0), g(1, 0, 0), u);
    let x10 = lerp(g(0, 1, 0), g(1, 1, 0), u);
    let x01 = lerp(g(0, 0, 1), g(1, 0, 1), u);
    let x11 = lerp(g(0, 1, 1), g(1, 1, 1), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

pub fn fbm(p: Vec3, seed: u32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut f = 1.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        sum += amp * noise3(p * f, seed.wrapping_add(i * 131));
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn mixc(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}

pub fn scalec(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// Any unit vector perpendicular to `n`.
pub fn any_perp(n: Vec3) -> Vec3 {
    let a = if n.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    n.cross(a).normalize()
}
