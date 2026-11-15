//! Blender-made models, baked into the binary.
//!
//! `assets/models/models.lrm` is produced by `tools/blender/build.sh` (see
//! `tools/blender/build_assets.py`) and embedded with `include_bytes!`, so the game stays a
//! single executable / APK. A model is a list of named parts; each part has a pivot offset
//! relative to its parent part, an optional lit mesh (vertex colours, `Gfx::vc`) and an
//! optional glowing mesh (`Gfx::glow`, unlit), plus an animation tag.
use crate::state::Game;
use crate::world::Sky;
use bevy::asset::RenderAssetUsages;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::collections::HashMap;

static PACK: &[u8] = include_bytes!("../assets/models/models.lrm");

/// Height of one starship hull section; must match `ship_hull` in tools/blender/structures.py.
pub const HULL_H: f32 = 2.3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cond {
    Always,
    /// the owning structure has a job running
    Job,
    /// the owning structure has fuel
    Fuel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anim {
    None,
    Spin { axis: Vec3, speed: f32, cond: Cond },
    Swing { axis: Vec3, amp: f32, hz: f32, cond: Cond },
    Slide { axis: Vec3, amp: f32, hz: f32, cond: Cond },
    Pulse { amp: f32, hz: f32, cond: Cond },
    /// yaw the part so its front (-Z) faces the sun
    Sun,
}

impl Anim {
    pub fn parse(s: &str) -> Anim {
        let w: Vec<&str> = s.split_whitespace().collect();
        let axis = |a: &str| match a.trim_start_matches(['-', '+']) {
            "x" => Vec3::X,
            "y" => Vec3::Y,
            _ => Vec3::Z,
        };
        let num = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
        let cond = |i: usize| match w.get(i).copied() {
            Some("job") => Cond::Job,
            Some("fuel") => Cond::Fuel,
            _ => Cond::Always,
        };
        match w.first().copied() {
            Some("spin") => Anim::Spin { axis: axis(w.get(1).unwrap_or(&"y")), speed: num(2), cond: cond(3) },
            Some("swing") => Anim::Swing { axis: axis(w.get(1).unwrap_or(&"y")), amp: num(2), hz: num(3), cond: cond(4) },
            Some("slide") => Anim::Slide { axis: axis(w.get(1).unwrap_or(&"y")), amp: num(2), hz: num(3), cond: cond(4) },
            Some("pulse") => Anim::Pulse { amp: num(1), hz: num(2), cond: cond(3) },
            Some("sun") => Anim::Sun,
            _ => Anim::None,
        }
    }
}

#[derive(Default, Clone)]
pub struct MeshSrc {
    pub pos: Vec<[f32; 3]>,
    pub nor: Vec<[f32; 3]>,
    pub col: Vec<[f32; 4]>,
    pub idx: Vec<u16>,
}

impl MeshSrc {
    fn mesh(&self) -> Mesh {
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos.clone());
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor.clone());
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col.clone());
        m.insert_indices(Indices::U16(self.idx.clone()));
        m
    }
}

#[derive(Clone)]
pub struct PartSrc {
    pub name: String,
    pub parent: Option<usize>,
    pub offset: Vec3,
    pub anim: Anim,
    pub lit: Option<MeshSrc>,
    pub glow: Option<MeshSrc>,
}

#[derive(Clone)]
pub struct ModelSrc {
    pub name: String,
    pub parts: Vec<PartSrc>,
}

impl ModelSrc {
    /// Pivot of part `i` in model space (sum of the offsets up the parent chain).
    pub fn pivot(&self, i: usize) -> Vec3 {
        let p = &self.parts[i];
        p.offset + p.parent.map(|j| self.pivot(j)).unwrap_or(Vec3::ZERO)
    }

    /// All parts (lit and glowing) merged in their rest pose, for ghosts, decor and icons.
    pub fn merged(&self) -> MeshSrc {
        let mut out = MeshSrc::default();
        for (i, p) in self.parts.iter().enumerate() {
            let o = self.pivot(i);
            for m in [&p.lit, &p.glow].into_iter().flatten() {
                let base = out.pos.len() as u32;
                out.pos.extend(m.pos.iter().map(|v| (Vec3::from_array(*v) + o).to_array()));
                out.nor.extend_from_slice(&m.nor);
                out.col.extend_from_slice(&m.col);
                // merged meshes can exceed u16 range in theory; models are far below it
                out.idx.extend(m.idx.iter().map(|&k| (base + k as u32) as u16));
            }
        }
        out
    }

    pub fn tris(&self) -> usize {
        self.parts.iter().flat_map(|p| [&p.lit, &p.glow]).flatten().map(|m| m.idx.len() / 3).sum()
    }
}

fn lin(c: u8) -> f32 {
    (c as f32 / 255.0).powf(2.2)
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let s = self.b.get(self.i..self.i + n).ok_or("model pack truncated")?;
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.u8()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn mesh(&mut self) -> Result<Option<MeshSrc>, String> {
        let nv = self.u32()? as usize;
        let ni = self.u32()? as usize;
        let mut m = MeshSrc::default();
        for _ in 0..nv {
            m.pos.push([self.f32()?, self.f32()?, self.f32()?]);
            let n = self.take(4)?;
            let n = Vec3::new(n[0] as i8 as f32, n[1] as i8 as f32, n[2] as i8 as f32).normalize_or(Vec3::Y);
            m.nor.push(n.to_array());
            let c = self.take(4)?;
            m.col.push([lin(c[0]), lin(c[1]), lin(c[2]), 1.0]);
        }
        for _ in 0..ni {
            let k = self.u16()?;
            if k as usize >= nv {
                return Err("index out of range".into());
            }
            m.idx.push(k);
        }
        Ok((ni > 0).then_some(m))
    }
}

pub fn parse(bytes: &[u8]) -> Result<Vec<ModelSrc>, String> {
    let mut r = Rd { b: bytes, i: 0 };
    if r.take(4)? != b"LRM1" {
        return Err("not an LRM1 model pack".into());
    }
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n {
        let name = r.str()?;
        let np = r.u16()? as usize;
        let mut parts = Vec::new();
        for i in 0..np {
            let pname = r.str()?;
            let parent = r.i16()?;
            let parent = if parent < 0 { None } else { Some(parent as usize) };
            if parent.is_some_and(|p| p >= i) {
                return Err(format!("{name}.{pname}: parent must come first"));
            }
            let offset = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
            let anim = Anim::parse(&r.str()?);
            let lit = r.mesh()?;
            let glow = r.mesh()?;
            parts.push(PartSrc { name: pname, parent, offset, anim, lit, glow });
        }
        out.push(ModelSrc { name, parts });
    }
    Ok(out)
}

/// Parsed embedded pack (no GPU handles); cheap enough to parse once per lookup in tests.
pub fn pack() -> Vec<ModelSrc> {
    parse(PACK).expect("embedded model pack is valid")
}

pub struct PartH {
    pub name: String,
    pub parent: Option<usize>,
    pub offset: Vec3,
    pub anim: Anim,
    pub lit: Option<Handle<Mesh>>,
    pub glow: Option<Handle<Mesh>>,
}

pub struct ModelH {
    pub src_pivots: Vec<Vec3>,
    pub parts: Vec<PartH>,
    pub merged: Handle<Mesh>,
}

/// GPU-side model set, kept in `Gfx::models`.
pub struct ModelLib {
    pub models: HashMap<String, ModelH>,
}

/// Animated model part. `owner` is the structure id whose state drives the condition.
#[derive(Component)]
pub struct PartAnim {
    pub anim: Anim,
    pub base: Transform,
    pub owner: Option<u32>,
    pub phase: f32,
    pub weight: f32,
}

impl ModelLib {
    pub fn load(meshes: &mut Assets<Mesh>) -> Self {
        let mut models = HashMap::new();
        for m in pack() {
            let parts = m
                .parts
                .iter()
                .map(|p| PartH {
                    name: p.name.clone(),
                    parent: p.parent,
                    offset: p.offset,
                    anim: p.anim,
                    lit: p.lit.as_ref().map(|s| meshes.add(s.mesh())),
                    glow: p.glow.as_ref().map(|s| meshes.add(s.mesh())),
                })
                .collect();
            let src_pivots = (0..m.parts.len()).map(|i| m.pivot(i)).collect();
            let merged = meshes.add(m.merged().mesh());
            models.insert(m.name.clone(), ModelH { src_pivots, parts, merged });
        }
        ModelLib { models }
    }

    pub fn get(&self, name: &str) -> &ModelH {
        self.models.get(name).unwrap_or_else(|| panic!("model {name} missing from the pack"))
    }

    /// The whole model as one mesh (rest pose).
    pub fn merged(&self, name: &str) -> Handle<Mesh> {
        self.get(name).merged.clone()
    }

    /// Model-space pivot of a named part, e.g. ("astronaut", "backpack").
    pub fn pivot(&self, model: &str, part: &str) -> Option<Vec3> {
        let m = self.models.get(model)?;
        m.parts.iter().position(|p| p.name == part).map(|i| m.src_pivots[i])
    }

    /// Spawns the parts of `name` as children of `root`. Returns one entity per part, in
    /// pack order (parents before children). Every part entity carries a `Name`.
    pub fn spawn(&self, commands: &mut Commands, root: Entity, name: &str, vc: &Handle<StandardMaterial>, glow: &Handle<StandardMaterial>, owner: Option<u32>) -> Vec<Entity> {
        let m = self.get(name);
        let mut ents: Vec<Entity> = Vec::with_capacity(m.parts.len());
        for p in &m.parts {
            let tf = Transform::from_translation(p.offset);
            let mut e = commands.spawn((tf, Visibility::Inherited, Name::new(p.name.clone())));
            match (&p.lit, &p.glow) {
                (Some(l), g) => {
                    e.insert((Mesh3d(l.clone()), MeshMaterial3d(vc.clone())));
                    if let Some(g) = g {
                        e.with_children(|c| {
                            c.spawn((Mesh3d(g.clone()), MeshMaterial3d(glow.clone()), Transform::IDENTITY, NotShadowCaster));
                        });
                    }
                }
                (None, Some(g)) => {
                    e.insert((Mesh3d(g.clone()), MeshMaterial3d(glow.clone()), NotShadowCaster));
                }
                (None, None) => {}
            }
            if p.anim != Anim::None {
                e.insert(PartAnim { anim: p.anim, base: tf, owner, phase: 0.0, weight: 0.0 });
            }
            let id = e.id();
            let parent = p.parent.map(|i| ents[i]).unwrap_or(root);
            commands.entity(parent).add_child(id);
            ents.push(id);
        }
        ents
    }
}

/// Drives `PartAnim` parts: spinning fans, swinging arms, the research dish, solar panels.
pub fn animate_parts(time: Res<Time>, game: Res<Game>, sky: Option<Res<Sky>>, mut q: Query<(&mut PartAnim, &mut Transform, &ChildOf)>, globals: Query<&GlobalTransform>) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let structs = game.structs();
    let active = |owner: Option<u32>, cond: Cond| -> bool {
        match cond {
            Cond::Always => true,
            Cond::Job => owner.and_then(|id| structs.iter().find(|s| s.id == id)).is_some_and(|s| s.job.is_some()),
            Cond::Fuel => owner.and_then(|id| structs.iter().find(|s| s.id == id)).is_some_and(|s| s.fuel > 0.0),
        }
    };
    for (mut pa, mut tf, child_of) in &mut q {
        let base = pa.base;
        match pa.anim {
            Anim::None => {}
            Anim::Spin { axis, speed, cond } => {
                let target = if active(pa.owner, cond) { 1.0 } else { 0.0 };
                pa.weight += (target - pa.weight) * (dt * 1.5).min(1.0);
                pa.phase = (pa.phase + speed * pa.weight * dt) % std::f32::consts::TAU;
                tf.rotation = base.rotation * Quat::from_axis_angle(axis, pa.phase);
            }
            Anim::Swing { axis, amp, hz, cond } | Anim::Slide { axis, amp, hz, cond } => {
                let target = if active(pa.owner, cond) { 1.0 } else { 0.0 };
                pa.weight += (target - pa.weight) * (dt * 2.0).min(1.0);
                pa.phase += hz * std::f32::consts::TAU * dt * pa.weight.max(0.05);
                let s = pa.phase.sin() * amp * pa.weight;
                if matches!(pa.anim, Anim::Swing { .. }) {
                    tf.rotation = base.rotation * Quat::from_axis_angle(axis, s);
                } else {
                    tf.translation = base.translation + axis * s;
                }
            }
            Anim::Pulse { amp, hz, cond } => {
                let target = if active(pa.owner, cond) { 1.0 } else { 0.0 };
                pa.weight += (target - pa.weight) * (dt * 2.0).min(1.0);
                tf.scale = base.scale * (1.0 + (t * hz * std::f32::consts::TAU).sin() * amp * pa.weight);
            }
            Anim::Sun => {
                let (Some(sky), Ok(parent)) = (sky.as_ref(), globals.get(child_of.parent())) else { continue };
                let local = parent.rotation().inverse() * sky.sun_dir;
                if local.y > -0.05 && local.xz().length() > 1e-3 {
                    let yaw = (-local.x).atan2(-local.z);
                    let target = base.rotation * Quat::from_rotation_y(yaw);
                    tf.rotation = tf.rotation.slerp(target, (dt * 0.6).min(1.0));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_parses_and_is_sane() {
        let pack = pack();
        assert!(!pack.is_empty());
        for m in &pack {
            assert!(!m.parts.is_empty(), "{} has no parts", m.name);
            for p in &m.parts {
                for s in [&p.lit, &p.glow].into_iter().flatten() {
                    assert_eq!(s.idx.len() % 3, 0);
                    assert!(s.pos.iter().all(|v| v.iter().all(|c| c.is_finite() && c.abs() < 40.0)), "{}.{}", m.name, p.name);
                }
            }
            assert!(m.tris() < 12_000, "{} has {} triangles", m.name, m.tris());
        }
    }

    #[test]
    fn anim_tags_parse() {
        assert_eq!(Anim::parse("spin y 6 job"), Anim::Spin { axis: Vec3::Y, speed: 6.0, cond: Cond::Job });
        assert_eq!(Anim::parse("sun"), Anim::Sun);
        assert_eq!(Anim::parse(""), Anim::None);
        assert!(matches!(Anim::parse("pulse 0.1 2 always"), Anim::Pulse { cond: Cond::Always, .. }));
    }
}
