//! Power cables and oxygen tethers.
//!
//! Everything in this file is pure logic on plain save data (`Structure` lists), so bases on
//! planets the player is not on keep working without entities or voxels. The Bevy side lives
//! in `network::vis` (cable / tether meshes, status lights) and `network::hud` (egui parts).
//!
//! Rules:
//! - Power flows only along cables (`Structure::links`, kept symmetric). A network is a
//!   connected component of the cable graph; an unconnected structure is a network of its own.
//!   Generators, batteries and consumers interact only inside their network.
//! - A cable may be at most `cable_reach` long (longer when a Power Pylon is at either end).
//! - Oxygen tethers are not cabled: a post links to every post within `TETHER_REACH` and to
//!   every oxygen source whose bubble is within reach. A post is live when such a chain leads
//!   back to a live source. Oxygen Stations are live sources only while their network is powered.
pub mod hud;
pub mod vis;

use crate::items::SK;
use crate::state::Structure;
use bevy::math::Vec3;

/// Longest cable between two ordinary structures (m, centre to centre).
pub const CABLE_REACH: f32 = 12.0;
/// Longest cable when a Power Pylon is at one end (m).
pub const PYLON_REACH: f32 = 26.0;
/// Longest gap between two tether posts (m).
pub const TETHER_REACH: f32 = 14.0;
/// Radius around a live tether post in which the astronaut breathes (m).
pub const TETHER_BREATH: f32 = 7.5;
/// The oxygen link is shown as "stretched" beyond this fraction of `TETHER_BREATH`.
pub const STRETCH: f32 = 0.75;

/// Does this kind of structure take part in power networks?
pub fn carries_power(k: SK) -> bool {
    !matches!(k, SK::Storage | SK::Shuttle | SK::Shipyard | SK::Tether)
}

pub fn is_generator(k: SK) -> bool {
    matches!(k, SK::Pod | SK::Rtg | SK::Solar | SK::BioGen)
}

/// Structures that need power to do their job (shown with a status light).
pub fn is_consumer(k: SK) -> bool {
    k.def().power < 0.0
}

/// Maximum cable length between two kinds.
pub fn cable_reach(a: SK, b: SK) -> f32 {
    if a == SK::Pylon || b == SK::Pylon {
        PYLON_REACH
    } else {
        CABLE_REACH
    }
}

fn pos(s: &Structure) -> Vec3 {
    Vec3::from_array(s.pos)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkErr {
    Same,
    NoPower,
    TooFar,
    Already,
    Missing,
}

pub fn can_link(a: &Structure, b: &Structure) -> Result<(), LinkErr> {
    if a.id == b.id {
        return Err(LinkErr::Same);
    }
    if !carries_power(a.kind) || !carries_power(b.kind) {
        return Err(LinkErr::NoPower);
    }
    if a.links.contains(&b.id) {
        return Err(LinkErr::Already);
    }
    if pos(a).distance(pos(b)) > cable_reach(a.kind, b.kind) {
        return Err(LinkErr::TooFar);
    }
    Ok(())
}

/// Connects two structures with a cable (both directions).
pub fn link(structs: &mut [Structure], a: u32, b: u32) -> Result<(), LinkErr> {
    let ia = structs.iter().position(|s| s.id == a).ok_or(LinkErr::Missing)?;
    let ib = structs.iter().position(|s| s.id == b).ok_or(LinkErr::Missing)?;
    can_link(&structs[ia], &structs[ib])?;
    structs[ia].links.push(b);
    structs[ib].links.push(a);
    Ok(())
}

pub fn unlink(structs: &mut [Structure], a: u32, b: u32) {
    for s in structs.iter_mut() {
        if s.id == a {
            s.links.retain(|&x| x != b);
        } else if s.id == b {
            s.links.retain(|&x| x != a);
        }
    }
}

/// Removes every cable attached to `id` (dismantling, shuttle take-off).
pub fn unlink_all(structs: &mut [Structure], id: u32) {
    for s in structs.iter_mut() {
        if s.id == id {
            s.links.clear();
        } else {
            s.links.retain(|&x| x != id);
        }
    }
}

/// Drops links that point at missing structures and makes the rest symmetric.
pub fn sanitize(structs: &mut [Structure]) {
    let ids: Vec<u32> = structs.iter().map(|s| s.id).collect();
    let mut pairs = Vec::new();
    for s in structs.iter() {
        for &l in &s.links {
            if l != s.id && ids.contains(&l) {
                pairs.push((s.id, l));
            }
        }
    }
    for s in structs.iter_mut() {
        s.links.clear();
    }
    for (a, b) in pairs {
        for (x, y) in [(a, b), (b, a)] {
            if let Some(s) = structs.iter_mut().find(|s| s.id == x) {
                if !s.links.contains(&y) {
                    s.links.push(y);
                }
            }
        }
    }
}

/// The structure a newly placed `id` should be cabled to automatically: the nearest
/// structure that carries power and is within cable reach.
pub fn auto_link_target(structs: &[Structure], id: u32) -> Option<u32> {
    let me = structs.iter().find(|s| s.id == id)?;
    let others: Vec<Structure> = structs.iter().filter(|s| s.id != id).cloned().collect();
    auto_link_at(&others, me.kind, pos(me)).map(|c| c.0)
}

/// Structures `id` could be cabled to right now, nearest first: (id, distance).
pub fn link_candidates(structs: &[Structure], id: u32) -> Vec<(u32, f32)> {
    let Some(me) = structs.iter().find(|s| s.id == id) else { return Vec::new() };
    let mut v: Vec<(u32, f32)> = structs.iter().filter(|o| can_link(me, o).is_ok()).map(|o| (o.id, pos(o).distance(pos(me)))).collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    v
}

/// Connected components of the cable graph. Returns the network index of every structure
/// (same order as `structs`) and the number of networks.
pub fn components(structs: &[Structure]) -> (Vec<usize>, usize) {
    let n = structs.len();
    let mut net = vec![usize::MAX; n];
    let mut count = 0;
    let index = |id: u32| structs.iter().position(|s| s.id == id);
    for start in 0..n {
        if net[start] != usize::MAX {
            continue;
        }
        let mut stack = vec![start];
        net[start] = count;
        while let Some(i) = stack.pop() {
            for &l in &structs[i].links {
                if let Some(j) = index(l) {
                    if net[j] == usize::MAX {
                        net[j] = count;
                        stack.push(j);
                    }
                }
            }
        }
        count += 1;
    }
    (net, count)
}

/// Power balance of one network for one tick.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct NetStat {
    pub supply: f32,
    pub demand: f32,
    /// fraction of the demand that is met (1 when there is no demand)
    pub ratio: f32,
    pub battery: f32,
    pub capacity: f32,
    pub members: u32,
    pub generators: u32,
}

impl NetStat {
    /// Something on this network can deliver power right now.
    pub fn powered(&self) -> bool {
        self.supply > 0.01 || self.battery > 0.01
    }
    /// Every consumer on it gets full power.
    pub fn healthy(&self) -> bool {
        self.powered() && self.ratio >= 0.99
    }
}

/// How a single structure is doing, for lights, hover text and panels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// works at full speed (or is idle with power available)
    Ok,
    /// powered, but the network cannot cover every consumer
    Low,
    /// no generator or charged battery anywhere on its network
    NoPower,
}

pub fn status_of(stat: &NetStat) -> Status {
    if !stat.powered() {
        Status::NoPower
    } else if stat.ratio < 0.99 {
        Status::Low
    } else {
        Status::Ok
    }
}

/// Supply / demand per network. `sun` gives the solar factor at a position (0..1).
/// Pure: does not change anything.
pub fn balance(structs: &[Structure], net: &[usize], count: usize, sun: &dyn Fn(Vec3) -> f32, dt: f32) -> Vec<NetStat> {
    let mut st = vec![NetStat { ratio: 1.0, ..Default::default() }; count];
    for (i, s) in structs.iter().enumerate() {
        let n = &mut st[net[i]];
        n.members += 1;
        let p = s.kind.def().power;
        match s.kind {
            SK::Pod | SK::Rtg => n.supply += p,
            SK::Solar => n.supply += p * sun(pos(s)),
            SK::BioGen => {
                if s.fuel > 0.0 {
                    n.supply += p
                }
            }
            SK::Battery => {
                n.battery += s.charge;
                n.capacity += crate::gameplay::BATTERY_CAP;
            }
            SK::Beacon => n.demand += -p,
            _ => {
                if s.job.is_some() && p < 0.0 {
                    n.demand += -p
                }
            }
        }
        if is_generator(s.kind) {
            n.generators += 1;
        }
    }
    for n in st.iter_mut() {
        let delta = (n.supply - n.demand) * dt;
        if delta < 0.0 && n.demand > 0.0 {
            let drawn = (-delta).min(n.battery);
            n.ratio = ((n.supply * dt + drawn) / (n.demand * dt)).clamp(0.0, 1.0);
        }
    }
    st
}

/// The networks of one planet after a tick.
#[derive(Clone, Default, Debug)]
pub struct PlanetNets {
    /// structure ids, same order as `net`
    pub ids: Vec<u32>,
    /// network index per structure
    pub net: Vec<usize>,
    pub stats: Vec<NetStat>,
}

impl PlanetNets {
    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.ids.iter().position(|&x| x == id).map(|i| self.net[i])
    }
    pub fn of(&self, id: u32) -> Option<NetStat> {
        self.index_of(id).map(|n| self.stats[n])
    }
    pub fn status(&self, id: u32) -> Status {
        self.of(id).map(|s| status_of(&s)).unwrap_or(Status::NoPower)
    }
    pub fn healthy(&self, id: u32) -> bool {
        self.of(id).map(|s| s.healthy()).unwrap_or(false)
    }
}

/// Live oxygen sources on a planet given its networks.
pub fn sources(structs: &[Structure], nets: &PlanetNets) -> Vec<Source> {
    structs
        .iter()
        .filter_map(|s| {
            let r = bubble(s, nets.healthy(s.id));
            (r > 0.0).then_some((s.id, pos(s), r))
        })
        .collect()
}

/// The structure the automatic cable of a `kind` placed at `at` would go to: (id, distance).
pub fn auto_link_at(structs: &[Structure], kind: SK, at: Vec3) -> Option<(u32, f32)> {
    if !carries_power(kind) {
        return None;
    }
    let probe = Structure::new(u32::MAX, kind, at.to_array(), [0.0, 0.0, 0.0, 1.0]);
    structs.iter().filter(|o| can_link(&probe, o).is_ok()).map(|o| (o.id, pos(o).distance(at))).min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Everything the game frame needs to know about networks and oxygen on the current planet.
#[derive(bevy::prelude::Resource, Default)]
pub struct Grid {
    pub nets: PlanetNets,
    pub sources: Vec<Source>,
    pub posts: Vec<Post>,
    pub breath: Option<Breath>,
    /// structure whose network the HUD readout describes
    pub focus: Option<u32>,
}

/// Oxygen bubble radius of a structure, 0 when it is not (currently) an oxygen source.
/// Oxygen Stations need a healthy power network.
pub fn bubble(s: &Structure, powered: bool) -> f32 {
    let r = s.kind.def().oxygen;
    if s.kind == SK::Beacon && !powered {
        0.0
    } else {
        r
    }
}

/// A tether post and how it hangs in the chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Post {
    pub id: u32,
    pub pos: Vec3,
    pub live: bool,
    /// next hop towards the oxygen source (a post or the source itself); for dead posts the
    /// nearest post in reach, so broken chains are still drawn
    pub parent: Option<u32>,
}

/// Oxygen sources: (id, position, bubble radius). Only live ones.
pub type Source = (u32, Vec3, f32);

fn reaches_source(p: Vec3, src: &Source) -> bool {
    p.distance(src.1) <= src.2 + TETHER_REACH * 0.5
}

/// Evaluates every tether post on a planet: which ones are live and how the chain runs.
pub fn tethers(structs: &[Structure], sources: &[Source]) -> Vec<Post> {
    let mut posts: Vec<Post> = structs.iter().filter(|s| s.kind == SK::Tether).map(|s| Post { id: s.id, pos: pos(s), live: false, parent: None }).collect();
    // breadth-first from the sources so every live post points along a shortest hop chain
    let mut frontier: Vec<usize> = Vec::new();
    for (i, p) in posts.iter_mut().enumerate() {
        if let Some(src) = sources.iter().filter(|s| reaches_source(p.pos, s)).min_by(|a, b| a.1.distance(p.pos).total_cmp(&b.1.distance(p.pos))) {
            p.live = true;
            p.parent = Some(src.0);
            frontier.push(i);
        }
    }
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for &i in &frontier {
            let (pid, ppos) = (posts[i].id, posts[i].pos);
            for (j, q) in posts.iter_mut().enumerate() {
                if !q.live && q.pos.distance(ppos) <= TETHER_REACH {
                    q.live = true;
                    q.parent = Some(pid);
                    next.push(j);
                }
            }
        }
        frontier = next;
    }
    // dead posts: hang them on their nearest neighbour so the broken chain stays visible
    let snapshot = posts.clone();
    for p in posts.iter_mut().filter(|p| !p.live) {
        p.parent = snapshot
            .iter()
            .filter(|q| q.id != p.id && q.pos.distance(p.pos) <= TETHER_REACH)
            .min_by(|a, b| a.pos.distance(p.pos).total_cmp(&b.pos.distance(p.pos)))
            .map(|q| q.id);
    }
    posts
}

/// Where the astronaut's oxygen comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Breath {
    /// inside a source bubble
    Source(u32),
    /// tethered to a live post: (post id, distance to it)
    Post(u32, f32),
}

/// Oxygen supply at the astronaut position `p`.
pub fn breath(p: Vec3, sources: &[Source], posts: &[Post]) -> Option<Breath> {
    if let Some(s) = sources.iter().find(|s| s.1.distance(p) < s.2) {
        return Some(Breath::Source(s.0));
    }
    posts
        .iter()
        .filter(|q| q.live)
        .map(|q| (q.id, q.pos.distance(p)))
        .filter(|(_, d)| *d < TETHER_BREATH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, d)| Breath::Post(id, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(id: u32, kind: SK, x: f32) -> Structure {
        Structure::new(id, kind, [x, 100.0, 0.0], [0.0, 0.0, 0.0, 1.0])
    }

    #[test]
    fn links_are_symmetric_and_limited_by_reach() {
        let mut v = vec![st(1, SK::Pod, 0.0), st(2, SK::Smelter, 8.0), st(3, SK::Smelter, 30.0), st(4, SK::Storage, 3.0)];
        assert_eq!(link(&mut v, 1, 2), Ok(()));
        assert_eq!(v[0].links, vec![2]);
        assert_eq!(v[1].links, vec![1]);
        assert_eq!(link(&mut v, 2, 1), Err(LinkErr::Already));
        assert_eq!(link(&mut v, 1, 3), Err(LinkErr::TooFar));
        assert_eq!(link(&mut v, 1, 4), Err(LinkErr::NoPower), "crates do not take cables");
        assert_eq!(link(&mut v, 1, 1), Err(LinkErr::Same));
        assert_eq!(link(&mut v, 1, 99), Err(LinkErr::Missing));
        unlink(&mut v, 2, 1);
        assert!(v[0].links.is_empty() && v[1].links.is_empty());
    }

    #[test]
    fn pylons_carry_cables_further() {
        // pod at 0, pylon at 20, smelter at 40: too far directly, fine through the pylon
        let mut v = vec![st(1, SK::Pod, 0.0), st(2, SK::Pylon, 20.0), st(3, SK::Smelter, 40.0)];
        assert_eq!(link(&mut v, 1, 3), Err(LinkErr::TooFar));
        link(&mut v, 1, 2).unwrap();
        link(&mut v, 2, 3).unwrap();
        let (net, n) = components(&v);
        assert_eq!(n, 1);
        assert!(net.iter().all(|&x| x == net[0]));
    }

    #[test]
    fn components_and_unlink_all() {
        let mut v = vec![st(1, SK::Pod, 0.0), st(2, SK::Smelter, 5.0), st(3, SK::Solar, 50.0), st(4, SK::Battery, 55.0), st(5, SK::Research, 100.0)];
        link(&mut v, 1, 2).unwrap();
        link(&mut v, 3, 4).unwrap();
        let (net, n) = components(&v);
        assert_eq!(n, 3);
        assert_eq!(net[0], net[1]);
        assert_eq!(net[2], net[3]);
        assert_ne!(net[0], net[2]);
        assert_ne!(net[4], net[0]);
        unlink_all(&mut v, 1);
        assert!(v.iter().all(|s| !s.links.contains(&1)) && v[0].links.is_empty());
        assert_eq!(components(&v).1, 4);
    }

    #[test]
    fn sanitize_repairs_dangling_and_one_sided_links() {
        let mut v = vec![st(1, SK::Pod, 0.0), st(2, SK::Smelter, 5.0)];
        v[0].links = vec![2, 77, 1];
        sanitize(&mut v);
        assert_eq!(v[0].links, vec![2]);
        assert_eq!(v[1].links, vec![1]);
    }

    #[test]
    fn auto_link_picks_the_nearest_powerable_structure_in_reach() {
        let v = vec![st(1, SK::Pod, 0.0), st(2, SK::Storage, 9.0), st(3, SK::Solar, 13.0), st(4, SK::Smelter, 11.0)];
        // smelter at 11: storage (2 m) cannot take cables, solar is 2 m away, pod 11 m
        assert_eq!(auto_link_target(&v, 4), Some(3));
        let far = vec![st(1, SK::Pod, 0.0), st(2, SK::Smelter, 20.0)];
        assert_eq!(auto_link_target(&far, 2), None);
        let crate_only = vec![st(1, SK::Pod, 0.0), st(2, SK::Storage, 4.0)];
        assert_eq!(auto_link_target(&crate_only, 2), None);
        let c = link_candidates(&v, 4);
        assert_eq!(c.iter().map(|c| c.0).collect::<Vec<_>>(), vec![3, 1]);
    }

    #[test]
    fn balance_is_per_network() {
        let mut v = vec![st(1, SK::Pod, 0.0), st(2, SK::Smelter, 5.0), st(3, SK::Smelter, 50.0)];
        v[1].job = Some(crate::state::Job { recipe: 0, count: 1, progress: 0.0 });
        v[2].job = Some(crate::state::Job { recipe: 0, count: 1, progress: 0.0 });
        link(&mut v, 1, 2).unwrap();
        let (net, n) = components(&v);
        let st = balance(&v, &net, n, &|_| 1.0, 0.1);
        let a = st[net[1]];
        let b = st[net[2]];
        assert_eq!((a.supply, a.demand, a.ratio), (4.0, 4.0, 1.0));
        assert_eq!(status_of(&a), Status::Ok);
        assert_eq!((b.supply, b.demand, b.ratio), (0.0, 4.0, 0.0));
        assert_eq!(status_of(&b), Status::NoPower);
    }

    #[test]
    fn battery_on_the_network_covers_a_deficit() {
        let mut v = vec![st(1, SK::Battery, 0.0), st(2, SK::Smelter, 5.0)];
        v[0].charge = 100.0;
        v[1].job = Some(crate::state::Job { recipe: 0, count: 1, progress: 0.0 });
        link(&mut v, 1, 2).unwrap();
        let (net, n) = components(&v);
        let s = balance(&v, &net, n, &|_| 1.0, 0.1)[0];
        assert!(s.powered() && (s.ratio - 1.0).abs() < 1e-4);
        // the same battery not cabled does nothing for the smelter
        unlink(&mut v, 1, 2);
        let (net, n) = components(&v);
        let s = balance(&v, &net, n, &|_| 1.0, 0.1);
        assert_eq!(status_of(&s[net[1]]), Status::NoPower);
    }

    #[test]
    fn solar_follows_the_sun_and_oxygen_station_needs_power() {
        let mut v = vec![st(1, SK::Solar, 0.0), st(2, SK::Beacon, 5.0)];
        link(&mut v, 1, 2).unwrap();
        let (net, n) = components(&v);
        let day = balance(&v, &net, n, &|_| 1.0, 0.1)[0];
        let night = balance(&v, &net, n, &|_| 0.0, 0.1)[0];
        assert!(day.healthy());
        assert!(!night.powered());
        assert_eq!(bubble(&v[1], day.healthy()), SK::Beacon.def().oxygen);
        assert_eq!(bubble(&v[1], night.healthy()), 0.0);
        assert_eq!(bubble(&st(9, SK::Pod, 0.0), false), SK::Pod.def().oxygen, "the pod is always a source");
    }

    fn pod_sources() -> Vec<Source> {
        vec![(1, Vec3::new(0.0, 100.0, 0.0), SK::Pod.def().oxygen)]
    }

    #[test]
    fn tether_chain_leads_back_to_the_source() {
        let r = SK::Pod.def().oxygen;
        let step = TETHER_REACH - 1.0;
        let x0 = r + TETHER_REACH * 0.5 - 1.0;
        let v = vec![st(1, SK::Pod, 0.0), st(10, SK::Tether, x0), st(11, SK::Tether, x0 + step), st(12, SK::Tether, x0 + 2.0 * step), st(13, SK::Tether, x0 + 4.0 * step)];
        let posts = tethers(&v, &pod_sources());
        let get = |id| posts.iter().find(|p| p.id == id).copied().unwrap();
        assert!(get(10).live && get(11).live && get(12).live);
        assert_eq!(get(10).parent, Some(1));
        assert_eq!(get(11).parent, Some(10));
        assert_eq!(get(12).parent, Some(11));
        assert!(!get(13).live, "a post beyond the gap is dead");
        assert_eq!(get(13).parent, None);

        // breaking the chain in the middle kills everything behind it
        let broken: Vec<Structure> = v.iter().filter(|s| s.id != 11).cloned().collect();
        let posts = tethers(&broken, &pod_sources());
        assert!(posts.iter().find(|p| p.id == 10).unwrap().live);
        assert!(!posts.iter().find(|p| p.id == 12).unwrap().live);

        // without a live source nothing is live
        assert!(tethers(&v, &[]).iter().all(|p| !p.live));
    }

    #[test]
    fn breathing_zones_of_a_chain_overlap() {
        // any two linked things leave no gap to walk through
        assert!(2.0 * TETHER_BREATH > TETHER_REACH);
        let r = SK::Pod.def().oxygen;
        assert!(r + TETHER_BREATH > r + TETHER_REACH * 0.5);
        // walk out along a maximal chain, nowhere on the way is without air
        let x0 = r + TETHER_REACH * 0.5;
        let v = vec![st(1, SK::Pod, 0.0), st(10, SK::Tether, x0), st(11, SK::Tether, x0 + TETHER_REACH), st(12, SK::Tether, x0 + 2.0 * TETHER_REACH)];
        let posts = tethers(&v, &pod_sources());
        assert!(posts.iter().all(|p| p.live));
        let mut x = 0.0;
        while x < x0 + 2.0 * TETHER_REACH + TETHER_BREATH - 0.1 {
            assert!(breath(Vec3::new(x, 100.0, 0.0), &pod_sources(), &posts).is_some(), "no air at x={x}");
            x += 0.25;
        }
        let far = breath(Vec3::new(x0 + 2.0 * TETHER_REACH + TETHER_BREATH + 0.5, 100.0, 0.0), &pod_sources(), &posts);
        assert_eq!(far, None);
        match breath(Vec3::new(x0 + TETHER_REACH + 2.0, 100.0, 0.0), &pod_sources(), &posts) {
            Some(Breath::Post(id, d)) => assert_eq!((id, (d * 10.0).round()), (11, 20.0)),
            other => panic!("expected the middle post, got {other:?}"),
        }
        assert_eq!(breath(Vec3::new(2.0, 100.0, 0.0), &pod_sources(), &posts), Some(Breath::Source(1)));
    }

    #[test]
    fn unpowered_oxygen_station_does_not_feed_tethers() {
        let v = vec![st(2, SK::Beacon, 0.0), st(10, SK::Tether, 8.0)];
        let sources: Vec<Source> = v.iter().filter_map(|s| {
            let r = bubble(s, false);
            (r > 0.0).then_some((s.id, pos(s), r))
        }).collect();
        assert!(sources.is_empty());
        assert!(!tethers(&v, &sources)[0].live);
    }
}
