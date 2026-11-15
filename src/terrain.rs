//! Terrain streaming: chunks are generated and meshed around the player on worker threads,
//! far chunks are dropped again (edited ones are kept), and a coarse, faceted "far shell" of
//! the whole planet fills in everything beyond the streamed area. The far shell sits a little
//! below the real surface and is cut away wherever real chunk meshes are in place, so it never
//! shows up inside holes or caves.
use crate::meshgen;
use crate::player::Player;
use crate::util::any_perp;
use crate::voxel::{gen_chunk, mesh_block, ChunkData, MeshData, TerrainGen, VoxelWorld, CS};
use crate::world::{mobile, ChunkEnt, Gfx, PlanetScoped};
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct Budget {
    /// chunks whose centre is within this (radially stretched) distance get meshes
    pub mesh_r: f32,
    /// generated + meshed synchronously while the loading screen shows
    pub sync_r: f32,
    /// far shell resolution (quads per cube face edge)
    pub far_n: usize,
    pub max_gen: usize,
    pub max_mesh: usize,
    /// edited chunks remeshed on the main thread per frame
    pub max_sync: usize,
}

pub fn budget() -> Budget {
    if mobile() {
        Budget { mesh_r: 92.0, sync_r: 44.0, far_n: 30, max_gen: 6, max_mesh: 6, max_sync: 4 }
    } else {
        Budget { mesh_r: 132.0, sync_r: 60.0, far_n: 46, max_gen: 12, max_mesh: 12, max_sync: 8 }
    }
}

/// Distance used for streaming: radial offsets count three times, so the streamed region is a
/// wide, flat lens around the player (deep rock under a surface walker is not needed, and the
/// surface far above a caver is not visible).
pub fn metric(focus: Vec3, p: Vec3) -> f32 {
    let up = focus.normalize_or(Vec3::Y);
    let d = p - focus;
    let radial = p.length() - focus.length();
    let tang = (d - up * d.dot(up)).length();
    (tang * tang + 9.0 * radial * radial).sqrt()
}

fn chunk_centre(c: IVec3) -> Vec3 {
    (c * CS).as_vec3() + Vec3::splat(8.0)
}

fn chunk_of(p: Vec3) -> IVec3 {
    (p / CS as f32).floor().as_ivec3()
}

/// Coarse planet shell, precomputed once per planet.
pub struct FarGrid {
    verts: Vec<Vec3>,
    tris: Vec<[u32; 3]>,
    tri_col: Vec<[f32; 4]>,
    tri_nor: Vec<[f32; 3]>,
}

fn cube_dir(face: usize, u: f32, v: f32) -> Vec3 {
    // equal-angle cube map: much more uniform cells than a plain cube projection
    let (a, b) = ((u * std::f32::consts::FRAC_PI_4).tan(), (v * std::f32::consts::FRAC_PI_4).tan());
    let p = match face {
        0 => Vec3::new(1.0, b, -a),
        1 => Vec3::new(-1.0, b, a),
        2 => Vec3::new(a, 1.0, -b),
        3 => Vec3::new(a, -1.0, b),
        4 => Vec3::new(a, b, 1.0),
        _ => Vec3::new(-a, b, -1.0),
    };
    p.normalize()
}

impl FarGrid {
    pub fn build(gen: &TerrainGen, n: usize) -> FarGrid {
        let w = n + 1;
        let step = std::f32::consts::FRAC_PI_2 / n as f32;
        let faces: Vec<(Vec<Vec3>, Vec<[f32; 4]>, Vec<[f32; 3]>)> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..6)
                .map(|f| {
                    s.spawn(move || {
                        let mut verts = Vec::with_capacity(w * w);
                        for j in 0..w {
                            for i in 0..w {
                                let d = cube_dir(f, i as f32 / n as f32 * 2.0 - 1.0, j as f32 / n as f32 * 2.0 - 1.0);
                                // eroded height: the minimum around the vertex, sunk a little, so
                                // the shell stays under the real surface between its samples
                                let t1 = any_perp(d);
                                let t2 = d.cross(t1);
                                let mut h = gen.height(d);
                                for o in [t1, -t1, t2, -t2] {
                                    h = h.min(gen.height((d + o * step * 0.8).normalize()));
                                }
                                verts.push(d * (h - 1.2));
                            }
                        }
                        let mut cols = Vec::with_capacity(n * n * 2);
                        let mut nors = Vec::with_capacity(n * n * 2);
                        for j in 0..n {
                            for i in 0..n {
                                let a = j * w + i;
                                for t in [[a, a + 1, a + w + 1], [a, a + w + 1, a + w]] {
                                    let (p0, p1, p2) = (verts[t[0]], verts[t[1]], verts[t[2]]);
                                    let nn = (p1 - p0).cross(p2 - p0).normalize_or_zero();
                                    let c = (p0 + p1 + p2) / 3.0;
                                    let col = gen.color_at(c, nn, 0.2);
                                    cols.push([col[0].powf(2.2) * 0.92, col[1].powf(2.2) * 0.92, col[2].powf(2.2) * 0.92, 1.0]);
                                    nors.push(nn.to_array());
                                }
                            }
                        }
                        (verts, cols, nors)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut g = FarGrid { verts: Vec::new(), tris: Vec::new(), tri_col: Vec::new(), tri_nor: Vec::new() };
        for (verts, cols, nors) in faces {
            let o = g.verts.len() as u32;
            for j in 0..n {
                for i in 0..n {
                    let a = (j * w + i) as u32 + o;
                    let w = w as u32;
                    g.tris.push([a, a + 1, a + w + 1]);
                    g.tris.push([a, a + w + 1, a + w]);
                }
            }
            g.verts.extend(verts);
            g.tri_col.extend(cols);
            g.tri_nor.extend(nors);
        }
        g
    }

    /// Mesh of the shell without the triangles for which `cut` holds at all three corners.
    fn mesh(&self, cut: &[bool]) -> MeshData {
        let mut m = MeshData { pos: Vec::new(), nor: Vec::new(), col: Vec::new(), idx: Vec::new() };
        for (k, t) in self.tris.iter().enumerate() {
            if cut[t[0] as usize] && cut[t[1] as usize] && cut[t[2] as usize] {
                continue;
            }
            let i0 = m.pos.len() as u32;
            for v in t {
                m.pos.push(self.verts[*v as usize].to_array());
                m.nor.push(self.tri_nor[k]);
                m.col.push(self.tri_col[k]);
            }
            m.idx.extend_from_slice(&[i0, i0 + 1, i0 + 2]);
        }
        m
    }
}

#[derive(Resource)]
pub struct Streamer {
    pub budget: Budget,
    want: Vec<IVec3>,
    want_at: Vec3,
    gen_tasks: HashMap<IVec3, Task<ChunkData>>,
    mesh_tasks: HashMap<IVec3, (u32, Task<Option<MeshData>>)>,
    /// chunks whose mesh is current (None = nothing to draw)
    pub meshed: HashMap<IVec3, Option<Entity>>,
    ver: HashMap<IVec3, u32>,
    far: Arc<FarGrid>,
    far_ent: Option<Entity>,
    far_mat: Handle<StandardMaterial>,
    far_at: Vec3,
    far_dirty: bool,
    far_timer: f32,
    far_cut: HashSet<IVec3>,
    frame: u32,
    pub focus: Vec3,
    /// (worst stream time this period in ms, far rebuilds, timer) for the bot log
    stats: (f32, u32, f32, f32, f32),
}

fn want_list(vw: &VoxelWorld, focus: Vec3, r: f32) -> Vec<IVec3> {
    let fc = chunk_of(focus);
    let k = (r / CS as f32).ceil() as i32 + 1;
    let mut out: Vec<(f32, IVec3)> = Vec::new();
    for z in -k..=k {
        for y in -k..=k {
            for x in -k..=k {
                let c = fc + IVec3::new(x, y, z);
                let d = metric(focus, chunk_centre(c));
                if d <= r && vw.gen.trivial_chunk(c).is_none() {
                    out.push((d, c));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|x| x.1).collect()
}

fn spawn_chunk(commands: &mut Commands, meshes: &mut Assets<Mesh>, gfx: &Gfx, m: MeshData) -> Entity {
    commands.spawn((Mesh3d(meshes.add(meshgen::terrain_mesh(m))), MeshMaterial3d(gfx.terrain.clone()), Transform::IDENTITY, ChunkEnt, PlanetScoped)).id()
}

impl Streamer {
    /// Loads and meshes the neighbourhood of `focus` right away and builds the far shell.
    pub fn prime(vw: &mut VoxelWorld, focus: Vec3, commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, gfx: &Gfx) -> Streamer {
        let budget = budget();
        let near = want_list(vw, focus, budget.sync_r);
        let mut need: HashSet<IVec3> = HashSet::new();
        for c in &near {
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        need.insert(*c + IVec3::new(dx, dy, dz));
                    }
                }
            }
        }
        let need: Vec<IVec3> = need.into_iter().filter(|c| vw.gen.trivial_chunk(*c).is_none()).collect();
        vw.load_now(&need);
        let blocks: Vec<(IVec3, Option<crate::voxel::Block>)> = near.iter().map(|c| (*c, vw.block(*c))).collect();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 12);
        let gen = &vw.gen;
        let results: Vec<Vec<(IVec3, Option<MeshData>)>> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..threads)
                .map(|t| {
                    let blocks = &blocks;
                    s.spawn(move || blocks.iter().skip(t).step_by(threads).map(|(c, b)| (*c, b.as_ref().and_then(|b| mesh_block(gen, *c, b)))).collect::<Vec<_>>())
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut meshed = HashMap::new();
        for r in results {
            for (c, m) in r {
                meshed.insert(c, m.map(|m| spawn_chunk(commands, meshes, gfx, m)));
            }
        }
        vw.dirty.clear();
        let far = Arc::new(FarGrid::build(&vw.gen, budget.far_n));
        let far_mat = mats.add(StandardMaterial { perceptual_roughness: 0.95, reflectance: 0.12, ..default() });
        let mut s = Streamer {
            budget,
            want: Vec::new(),
            want_at: Vec3::splat(1e9),
            gen_tasks: HashMap::new(),
            mesh_tasks: HashMap::new(),
            meshed,
            ver: HashMap::new(),
            far,
            far_ent: None,
            far_mat,
            far_at: focus,
            far_dirty: true,
            far_timer: 0.0,
            far_cut: HashSet::new(),
            frame: 0,
            focus,
            stats: (0.0, 0, 0.0, 0.0, 0.0),
        };
        s.rebuild_far(vw, focus, commands, meshes);
        s
    }

    fn settled(&self, vw: &VoxelWorld, c: IVec3) -> bool {
        self.meshed.contains_key(&c) || vw.gen.trivial_chunk(c).is_some()
    }

    fn rebuild_far(&mut self, vw: &VoxelWorld, focus: Vec3, commands: &mut Commands, meshes: &mut Assets<Mesh>) {
        let lim = self.budget.mesh_r - 6.0;
        let mut cut_set = HashSet::new();
        let cut: Vec<bool> = self
            .far
            .verts
            .iter()
            .map(|v| {
                if metric(focus, *v) > lim {
                    return false;
                }
                let up = v.normalize();
                let a = chunk_of(*v);
                let b = chunk_of(*v + up * 6.0);
                let c = chunk_of(*v - up * 2.0);
                if self.settled(vw, a) && self.settled(vw, b) && self.settled(vw, c) {
                    cut_set.insert(a);
                    cut_set.insert(b);
                    cut_set.insert(c);
                    true
                } else {
                    false
                }
            })
            .collect();
        let m = self.far.mesh(&cut);
        let mesh = meshes.add(meshgen::terrain_mesh(m));
        match self.far_ent {
            Some(e) => {
                // bounds are only computed once per entity: drop them so they follow the new mesh
                commands.entity(e).insert(Mesh3d(mesh)).remove::<bevy::render::primitives::Aabb>();
            }
            None => {
                self.far_ent = Some(commands.spawn((Mesh3d(mesh), MeshMaterial3d(self.far_mat.clone()), Transform::IDENTITY, NotShadowCaster, NotShadowReceiver, PlanetScoped)).id());
            }
        }
        self.far_cut = cut_set;
        self.far_at = focus;
        self.far_dirty = false;
        self.far_timer = 0.0;
    }

    /// Chunks around `focus` whose data and mesh are in place (for tests and the loading screen).
    pub fn ready_near(&self, vw: &VoxelWorld, focus: Vec3, r: f32) -> bool {
        want_list(vw, focus, r).iter().all(|c| self.meshed.contains_key(c))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn stream(
    mut commands: Commands,
    time: Res<Time>,
    mut vw: ResMut<VoxelWorld>,
    mut st: ResMut<Streamer>,
    gfx: Res<Gfx>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<&Mesh3d, With<ChunkEnt>>,
    player: Query<&Transform, With<Player>>,
) {
    let st = &mut *st;
    let Ok(ptf) = player.single() else { return };
    let t_start = std::time::Instant::now();
    let focus = ptf.translation;
    st.focus = focus;
    st.frame = st.frame.wrapping_add(1);
    st.far_timer += time.delta_secs();
    let pool = AsyncComputeTaskPool::get();

    let apply = |commands: &mut Commands, meshes: &mut Assets<Mesh>, st: &mut Streamer, c: IVec3, m: Option<MeshData>| {
        let old = st.meshed.get(&c).copied().flatten();
        let new = match (m, old) {
            (Some(m), Some(e)) => {
                if let Ok(h) = q.get(e) {
                    meshes.remove(&h.0);
                }
                // Bevy computes bounds once per entity; a dug chunk's old (shallow) bounds would
                // get it frustum-culled while looking down into the pit, so recompute them
                commands.entity(e).insert(Mesh3d(meshes.add(meshgen::terrain_mesh(m)))).remove::<bevy::render::primitives::Aabb>();
                Some(e)
            }
            (Some(m), None) => Some(spawn_chunk(commands, meshes, &gfx, m)),
            (None, Some(e)) => {
                if let Ok(h) = q.get(e) {
                    meshes.remove(&h.0);
                }
                commands.entity(e).despawn();
                None
            }
            (None, None) => None,
        };
        if !st.meshed.contains_key(&c) {
            st.far_dirty = true;
        }
        st.meshed.insert(c, new);
    };

    // finished generation tasks
    let done: Vec<IVec3> = st.gen_tasks.iter_mut().filter_map(|(c, t)| block_on(future::poll_once(t)).map(|d| (*c, d))).map(|(c, d)| {
        vw.chunks.entry(c).or_insert(d);
        c
    }).collect();
    for c in done {
        st.gen_tasks.remove(&c);
    }
    // finished mesh tasks
    let mut finished = Vec::new();
    for (c, (v, t)) in st.mesh_tasks.iter_mut() {
        if let Some(m) = block_on(future::poll_once(t)) {
            finished.push((*c, *v, m));
        }
    }
    for (c, v, m) in finished {
        st.mesh_tasks.remove(&c);
        if st.ver.get(&c).copied().unwrap_or(0) == v {
            apply(&mut commands, &mut meshes, st, c, m);
        }
    }

    // edits: remesh right away, nearest first
    if !vw.dirty.is_empty() {
        let mut list: Vec<IVec3> = vw.dirty.iter().copied().collect();
        list.sort_by(|a, b| chunk_centre(*a).distance_squared(focus).total_cmp(&chunk_centre(*b).distance_squared(focus)));
        for c in list.into_iter().take(st.budget.max_sync) {
            vw.dirty.remove(&c);
            *st.ver.entry(c).or_insert(0) += 1;
            st.mesh_tasks.remove(&c);
            if st.meshed.contains_key(&c) || metric(focus, chunk_centre(c)) < st.budget.mesh_r {
                let m = vw.mesh_chunk(c);
                apply(&mut commands, &mut meshes, st, c, m);
            }
        }
    }

    // what should be meshed
    if st.want_at.distance(focus) > 6.0 {
        let t = std::time::Instant::now();
        st.want = want_list(&vw, focus, st.budget.mesh_r);
        st.want_at = focus;
        st.stats.4 = st.stats.4.max(t.elapsed().as_secs_f32() * 1000.0);
    }
    let mut blocks_left = 24;
    for i in 0..st.want.len() {
        if st.gen_tasks.len() >= st.budget.max_gen && st.mesh_tasks.len() >= st.budget.max_mesh {
            break;
        }
        let c = st.want[i];
        if st.meshed.contains_key(&c) || st.mesh_tasks.contains_key(&c) {
            continue;
        }
        let mut ready = true;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let k = c + IVec3::new(dx, dy, dz);
                    if !vw.ready(k) {
                        ready = false;
                        if !st.gen_tasks.contains_key(&k) && st.gen_tasks.len() < st.budget.max_gen {
                            let gen = vw.gen.clone();
                            st.gen_tasks.insert(k, pool.spawn(async move { gen_chunk(&gen, k) }));
                        }
                    }
                }
            }
        }
        if !ready || st.mesh_tasks.len() >= st.budget.max_mesh || blocks_left == 0 {
            continue;
        }
        blocks_left -= 1;
        match vw.block(c) {
            None => apply(&mut commands, &mut meshes, st, c, None),
            Some(b) => {
                let gen = vw.gen.clone();
                let v = st.ver.get(&c).copied().unwrap_or(0);
                st.mesh_tasks.insert(c, (v, pool.spawn(async move { mesh_block(&gen, c, &b) })));
            }
        }
    }

    // far shell follows the meshed area
    // (deep underground the shell is out of sight: only follow large moves there, so that
    // the stream of freshly meshed cave chunks does not rebuild it every few frames)
    let deep = vw.gen.depth(focus) > 45.0;
    let moved = st.far_at.distance(focus) > if deep { 40.0 } else { 6.0 };
    if (moved || (st.far_dirty && !deep)) && st.far_timer > 0.35 {
        let t = std::time::Instant::now();
        st.rebuild_far(&vw, focus, &mut commands, &mut meshes);
        st.stats.1 += 1;
        st.stats.3 = st.stats.3.max(t.elapsed().as_secs_f32() * 1000.0);
    }

    // drop what is far away
    if st.frame % 30 == 0 {
        let r = st.budget.mesh_r;
        let drop: Vec<IVec3> = st.meshed.keys().copied().filter(|c| !st.far_cut.contains(c) && metric(focus, chunk_centre(*c)) > r + 32.0).collect();
        for c in drop {
            if let Some(Some(e)) = st.meshed.remove(&c) {
                if let Ok(h) = q.get(e) {
                    meshes.remove(&h.0);
                }
                commands.entity(e).despawn();
            }
        }
        let modified = &vw.modified;
        let evict: Vec<IVec3> = vw.chunks.keys().copied().filter(|c| !modified.contains(c) && metric(focus, chunk_centre(*c)) > r + 56.0).collect();
        for c in evict {
            vw.chunks.remove(&c);
        }
    }
    st.stats.0 = st.stats.0.max(t_start.elapsed().as_secs_f32() * 1000.0);
    st.stats.2 += time.delta_secs();
    if st.stats.2 > 2.0 {
        if std::env::var("LOSTRANOUT_BOT").is_ok() {
            let ents = st.meshed.values().filter(|e| e.is_some()).count();
            eprintln!("[stream] worst {:.1} ms (far {:.1}, want {:.1}), far rebuilds {}, chunks {} loaded / {} meshed / {} drawn, tasks {}+{}", st.stats.0, st.stats.3, st.stats.4, st.stats.1, vw.chunks.len(), st.meshed.len(), ents, st.gen_tasks.len(), st.mesh_tasks.len());
        }
        st.stats = (0.0, 0, 0.0, 0.0, 0.0);
    }
}

/// Generates a planet's chunks on the calling thread (tests).
#[cfg(test)]
pub fn gen_all(gen: &TerrainGen, coords: &[IVec3]) -> Vec<(IVec3, ChunkData)> {
    coords.iter().map(|c| (*c, gen_chunk(gen, *c))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::StarSystem;

    #[test]
    fn far_shell_stays_below_the_real_surface() {
        let sys = StarSystem::generate(21, 0);
        let gen = TerrainGen::new(&sys.planets[sys.home]);
        let g = FarGrid::build(&gen, 24);
        assert_eq!(g.tris.len(), 6 * 24 * 24 * 2);
        let mut above = 0;
        for t in g.tris.iter().step_by(7) {
            let c = (g.verts[t[0] as usize] + g.verts[t[1] as usize] + g.verts[t[2] as usize]) / 3.0;
            if c.length() > gen.height(c.normalize()) + 0.5 {
                above += 1;
            }
        }
        assert!(above * 50 < g.tris.len() / 7, "the far shell pokes through the terrain too often: {above}");
        // the shell is closed: every edge is shared by exactly two triangles (by position)
        let key = |v: Vec3| ((v.x * 64.0).round() as i64, (v.y * 64.0).round() as i64, (v.z * 64.0).round() as i64);
        let mut edges: HashMap<((i64, i64, i64), (i64, i64, i64)), i32> = HashMap::new();
        for t in &g.tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (ka, kb) = (key(g.verts[a as usize]), key(g.verts[b as usize]));
                let e = if ka < kb { (ka, kb) } else { (kb, ka) };
                *edges.entry(e).or_insert(0) += 1;
            }
        }
        let open = edges.values().filter(|n| **n != 2).count();
        assert!(open * 200 < edges.len(), "far shell has {open} open edges of {}", edges.len());
    }

    #[test]
    fn streaming_metric_is_flat() {
        let f = Vec3::new(0.0, 300.0, 0.0);
        assert!((metric(f, f + Vec3::X * 20.0) - 20.0).abs() < 0.5);
        assert!((metric(f, f - Vec3::Y * 20.0) - 60.0).abs() < 0.5);
        let _ = gen_all;
    }
}
