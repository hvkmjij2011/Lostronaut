//! Sky and light model per planet: sky gradient with horizon glow and sunset band, sun and
//! moon light, fog and a hemisphere ambient (sky colour from above, ground bounce from below)
//! baked into a tiny generated environment cubemap. Pure functions; `world::update_sky`
//! applies them to the scene.
//!
//! Directions are in a local sky frame: +Y is the player's up, the sun lies in the X-Y plane
//! on the +X side. Colours are computed in sRGB space and returned linear.
use crate::galaxy::PlanetDef;
use crate::util::{mixc, smoothstep};
use bevy::math::Vec3;

type C3 = [f32; 3];

fn to_lin(c: C3) -> C3 {
    [c[0].max(0.0).powf(2.2), c[1].max(0.0).powf(2.2), c[2].max(0.0).powf(2.2)]
}

fn add(a: C3, b: C3, k: f32) -> C3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn mul(a: C3, k: f32) -> C3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub struct Atmos {
    zen: C3,
    hor: C3,
    sunset: C3,
    star: C3,
    ground: C3,
    /// atmosphere thickness as defined by the planet (0.05 barren .. 1.4 toxic)
    pub atm: f32,
    /// how much sky there is at all: ~0 on airless worlds (black sky by day)
    pub air: f32,
}

impl Atmos {
    pub fn new(def: &PlanetDef, star: C3) -> Atmos {
        let s = def.sky;
        let deep = [s[0] * s[0], s[1] * s[1], s[2] * s[2]];
        let atm = def.atmosphere;
        // thick atmospheres are hazy: the zenith is closer to the horizon colour
        let haze = smoothstep(0.9, 1.5, atm);
        let zen = mul(mixc(mixc(s, deep, 0.7), s, haze), 0.88);
        let hor = mixc(mixc(s, [1.0, 1.0, 1.0], 0.3), s, haze * 0.5);
        let sunset = mixc(mixc([1.0, 0.42, 0.16], star, 0.25), s, 0.22 + 0.3 * haze);
        Atmos { zen, hor, sunset, star, ground: def.top, atm, air: smoothstep(0.0, 0.65, atm) }
    }

    /// 0 at night, 1 in full day, from the sine of the sun elevation.
    pub fn day(se: f32) -> f32 {
        smoothstep(-0.15, 0.2, se)
    }

    fn twilight(se: f32) -> f32 {
        1.0 - smoothstep(0.0, 0.3, (se - 0.03).abs())
    }

    /// Sky colour (sRGB) looking in direction `d`; `sun` is the unit sun direction.
    pub fn sky_srgb(&self, d: Vec3, sun: Vec3) -> C3 {
        let se = sun.y;
        let day = Self::day(se);
        let twi = Self::twilight(se);
        let e = d.y.max(0.0);
        let hw = (1.0 - e).powi(5);
        let cs = d.dot(sun).max(0.0);
        let night = mixc([0.012, 0.016, 0.04], mixc([0.035, 0.045, 0.085], self.hor, 0.04), hw);
        let day_c = mul(mixc(self.zen, self.hor, hw), self.air);
        let mut c = mixc(night, day_c, day * (0.25 + 0.75 * self.air));
        // sunset band: low on the sky, strongest towards the sun
        let band = twi * hw * (0.3 + 0.7 * cs * cs) * self.air;
        c = mixc(c, self.sunset, band.min(1.0) * 0.85);
        // glow around the sun (wide haze + tight corona); thin air keeps only the corona
        let glow_c = mixc(mixc(self.star, [1.0, 1.0, 1.0], 0.3), self.sunset, twi * 0.7);
        let vis = smoothstep(-0.12, 0.05, se);
        let wide = cs.powi(6) * 0.32 * self.air * (0.4 + 0.6 * self.atm.min(1.2));
        let tight = cs.powi(48) * (0.25 + 0.75 * self.air);
        c = add(c, glow_c, (wide + tight) * vis);
        // below the horizon: ground haze, a little darker
        if d.y < 0.0 {
            c = mul(c, 1.0 - 0.3 * smoothstep(0.0, -0.4, d.y));
        }
        c
    }

    /// Linear sky colour for the dome vertices.
    pub fn sky(&self, d: Vec3, sun: Vec3) -> C3 {
        // a little extra gain and saturation: the tonemapper flattens the unlit dome
        let c = to_lin(self.sky_srgb(d, sun));
        let l = c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11;
        [(l + (c[0] - l) * 1.25).max(0.0) * 1.2, (l + (c[1] - l) * 1.25).max(0.0) * 1.2, (l + (c[2] - l) * 1.25).max(0.0) * 1.2]
    }

    /// Linear ambient (irradiance-like) colour arriving from direction `d`: blurred sky from
    /// above, ground bounce from below, plus a moonlit floor at night so the night stays
    /// readable.
    pub fn ambient(&self, d: Vec3, sun: Vec3) -> C3 {
        let se = sun.y;
        let day = Self::day(se);
        let up = Vec3::new(d.x * 0.5, d.y.max(0.0) * 0.5 + 0.45, d.z * 0.5).normalize();
        let sky = to_lin(self.sky_srgb(up, sun));
        let horizon = to_lin(self.sky_srgb(Vec3::new(d.x, 0.05, d.z).normalize(), sun));
        let sky = mixc(horizon, sky, d.y.max(0.0));
        // keep the sky light from tinting everything blue: it is partly neutral
        let l = sky[0] * 0.3 + sky[1] * 0.59 + sky[2] * 0.11;
        let sky = mixc(sky, [l, l, l], 0.35);
        let sunlit = smoothstep(-0.05, 0.3, se) * 0.55;
        let g = to_lin(self.ground);
        let avg_sky = mixc(horizon, sky, 0.5);
        let ground = [g[0] * (sunlit + avg_sky[0]), g[1] * (sunlit + avg_sky[1]), g[2] * (sunlit + avg_sky[2])];
        let mut c = mixc(ground, sky, smoothstep(-0.45, 0.35, d.y));
        // airless worlds: no sky light, but bright ground bounce keeps shadows from going black
        c = add(c, mul(g, 0.25 * smoothstep(-0.05, 0.3, se)), 1.0 - self.air);
        let floor = mul([0.05, 0.065, 0.12], (1.0 - day) * (0.6 + 0.4 * smoothstep(-0.6, 0.6, d.y)));
        add(c, floor, 1.0)
    }

    /// Sun light: (linear colour, illuminance in lux).
    pub fn sun(&self, se: f32) -> (C3, f32) {
        let twi = Self::twilight(se);
        let low = 1.0 - smoothstep(0.05, 0.45, se);
        let col = mixc(self.star, mixc(self.sunset, [1.0, 0.7, 0.4], 0.3), (low * 0.75 * self.air).max(twi * 0.4 * self.air));
        let thick = 1.0 - 0.2 * (self.atm - 1.0).max(0.0);
        let lux = 10500.0 * smoothstep(-0.04, 0.16, se) * thick * (1.0 - 0.35 * low * self.air) + 1500.0 * (1.0 - self.air) * smoothstep(-0.04, 0.16, se);
        (to_lin(col), lux)
    }

    /// Moonlight from the anti-sun side: (linear colour, lux).
    pub fn moon(se: f32) -> (C3, f32) {
        (to_lin([0.62, 0.72, 1.0]), 1900.0 * smoothstep(-0.06, -0.28, se))
    }

    /// Fog colour (linear) and the extra in-scatter colour towards the sun.
    pub fn fog(&self, sun: Vec3) -> (C3, C3) {
        let side = Vec3::new(0.0, 0.03, 1.0).normalize();
        let base = self.sky(side, sun);
        let toward = self.sky(Vec3::new(1.0, 0.03, 0.0).normalize(), sun);
        let scatter = [(toward[0] - base[0]).max(0.0), (toward[1] - base[1]).max(0.0), (toward[2] - base[2]).max(0.0)];
        (base, scatter)
    }

    /// Fog visibility in metres: barely any haze on airless worlds, thick on toxic ones.
    pub fn visibility(&self) -> f32 {
        1600.0 / (0.4 + 2.2 * self.atm.max(0.0).powf(2.5))
    }
}

/// Cubemap texel direction (wgpu face order +X -X +Y -Y +Z -Z, u right, v down) turned
/// into the local sky direction Bevy uses when it samples the map (it negates z).
pub fn cube_dir(face: usize, u: f32, v: f32) -> Vec3 {
    let c = match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    };
    Vec3::new(c.x, c.y, -c.z).normalize()
}

/// Fills an RGBA8 (linear) cubemap of `size`² per face with the ambient light. Returns the
/// scale the texels were divided by (to be used as the environment map intensity factor).
pub fn fill_ambient_cube(atmos: &Atmos, sun: Vec3, size: u32, out: &mut Vec<u8>) -> f32 {
    let mut cols = Vec::with_capacity((size * size * 6) as usize);
    let mut max = 1e-4f32;
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let c = atmos.ambient(cube_dir(face, u, v), sun);
                max = max.max(c[0]).max(c[1]).max(c[2]);
                cols.push(c);
            }
        }
    }
    out.clear();
    for c in cols {
        for k in 0..3 {
            out.push((c[k] / max * 255.0).round().clamp(0.0, 255.0) as u8);
        }
        out.push(255);
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::{PlanetKind, StarSystem};

    fn lum(c: C3) -> f32 {
        c[0] * 0.3 + c[1] * 0.6 + c[2] * 0.1
    }

    #[test]
    fn day_night_and_atmospheres_behave() {
        let sys = StarSystem::generate(3, 0);
        let noon = Vec3::new(0.2, 0.98, 0.0).normalize();
        let night = Vec3::new(0.2, -0.98, 0.0).normalize();
        let dusk = Vec3::new(0.995, 0.02, 0.0).normalize();
        let mut barren_day = None;
        let mut toxic_vis = None;
        for p in &sys.planets {
            let a = Atmos::new(p, sys.star_color);
            let zen = Vec3::Y;
            assert!(lum(a.sky(zen, noon)) > lum(a.sky(zen, night)) * 3.0 || p.kind == PlanetKind::Barren, "{:?}", p.kind);
            // the sunset band makes the sky towards the setting sun warmer than away from it
            let to_sun = a.sky_srgb(Vec3::new(1.0, 0.05, 0.0).normalize(), dusk);
            let away = a.sky_srgb(Vec3::new(-1.0, 0.05, 0.0).normalize(), dusk);
            assert!(to_sun[0] > away[0], "{:?}: no glow towards the sun", p.kind);
            // ambient: sky above is brighter than the ground below by day, never zero at night
            assert!(lum(a.ambient(Vec3::Y, noon)) > lum(a.ambient(-Vec3::Y, noon)) || p.kind == PlanetKind::Barren);
            assert!(lum(a.ambient(Vec3::Y, night)) > 0.02, "{:?} night too dark", p.kind);
            if p.kind == PlanetKind::Barren {
                barren_day = Some(lum(a.sky(zen, noon)));
            }
            if p.kind == PlanetKind::Toxic {
                toxic_vis = Some(a.visibility());
            }
        }
        assert!(barren_day.unwrap() < 0.02, "airless sky should be black by day");
        assert!(toxic_vis.unwrap() < 300.0);
    }

    #[test]
    fn cube_faces_point_the_right_way() {
        // centre texel of the +Y face looks up, of -Y down
        assert!(cube_dir(2, 0.0, 0.0).y > 0.99);
        assert!(cube_dir(3, 0.0, 0.0).y < -0.99);
        assert!(cube_dir(0, 0.0, 0.0).x > 0.99);
    }
}
