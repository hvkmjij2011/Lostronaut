//! egui pieces for power networks and tethers: the HUD power / oxygen-link readout, the power
//! section of structure panels, and the tether drop button. Kept separate from `ui.rs` so a UI
//! rewrite can call them from wherever it wants.
use super::{Breath, Grid, NetStat, Status, STRETCH, TETHER_BREATH};
use crate::gameplay::PowerStat;
use crate::items::{tx, Lang, SK};
use crate::state::Game;
use bevy::prelude::Resource;
use bevy_egui::egui::{self, Color32, RichText};

const OK: Color32 = Color32::from_rgb(160, 235, 160);
const LOW: Color32 = Color32::from_rgb(255, 190, 90);
const BAD: Color32 = Color32::from_rgb(255, 110, 90);
const DIM: Color32 = Color32::from_rgb(150, 160, 175);
const ORANGE: Color32 = Color32::from_rgb(255, 160, 60);

/// Requests from the network UI to the game systems.
#[derive(Resource, Default)]
pub struct NetUi {
    /// drop a tether post at the astronaut's feet (touch button)
    pub drop_tether: bool,
}

fn t(l: Lang, tr: &'static str, en: &'static str) -> &'static str {
    tx(l, (tr, en))
}

fn status_color(s: Status) -> Color32 {
    match s {
        Status::Ok => OK,
        Status::Low => LOW,
        Status::NoPower => BAD,
    }
}

fn stat_of(p: &PowerStat) -> NetStat {
    NetStat { supply: p.supply, demand: p.demand, ratio: p.ratio, battery: p.battery, capacity: p.capacity, members: p.members, generators: 0 }
}

/// One-line power readout for the HUD: the network of the structure in focus.
pub fn power_hud(ui: &mut egui::Ui, power: &PowerStat, game: &Game, l: Lang) {
    let Some(id) = power.focus else {
        ui.label(RichText::new(t(l, "⚡ Yakında şebeke yok", "⚡ No grid nearby")).size(14.0).color(DIM));
        return;
    };
    let st = super::status_of(&stat_of(power));
    let name = game.structs().iter().find(|s| s.id == id).map(|s| s.kind.name(l)).unwrap_or("");
    let mut txt = format!("⚡ {:.0} / {:.0}", power.supply, power.demand);
    if power.capacity > 0.0 {
        txt += &format!("  🔋 {:.0}%", power.battery / power.capacity * 100.0);
    }
    ui.label(RichText::new(txt).size(14.0).color(status_color(st)));
    let state = match st {
        Status::Ok => t(l, "şebeke", "grid"),
        Status::Low => t(l, "güç yetersiz", "underpowered"),
        Status::NoPower => t(l, "güç yok", "no power"),
    };
    ui.label(RichText::new(format!("{name}: {state} ({} {})", power.members, t(l, "yapı", "parts"))).size(12.0).color(DIM));
}

/// Oxygen link line under the O₂ bar: tether distance, or a warning with the time left.
pub fn oxygen_hud(ui: &mut egui::Ui, grid: &Grid, game: &Game, drain: f32, l: Lang, touch: bool) {
    match grid.breath {
        Some(Breath::Post(_, d)) => {
            let col = if d > TETHER_BREATH * STRETCH { LOW } else { Color32::from_rgb(140, 220, 255) };
            ui.label(RichText::new(format!("{} {:.1} / {:.1} m", t(l, "Halat:", "Relay:"), d, TETHER_BREATH)).size(13.0).color(col));
        }
        Some(Breath::Source(_)) => {}
        None => {
            let secs = game.oxygen / drain.max(0.01);
            ui.label(RichText::new(format!("{} ~{:.0} s", t(l, "⚠ Oksijen bağlantısı yok", "⚠ No oxygen link"), secs)).size(13.5).strong().color(BAD));
        }
    }
    if !touch {
        let n = game.inv.count(crate::items::Item::Organic);
        ui.label(RichText::new(format!("[T] {} ({} {})", t(l, "Halat bırak", "Drop relay"), n, crate::items::Item::Organic.name(l))).size(12.0).color(DIM));
    }
}

/// Big touch button that drops a tether post.
pub fn tether_button(ui: &mut egui::Ui, net_ui: &mut NetUi, game: &Game, l: Lang) {
    let n = game.inv.count(crate::items::Item::Organic);
    let label = format!("{}\n({n})", t(l, "HALAT", "RELAY"));
    let b = egui::Button::new(RichText::new(label).size(15.0)).min_size(egui::vec2(78.0, 64.0)).corner_radius(12);
    if ui.add_enabled(n > 0, b).clicked() {
        net_ui.drop_tether = true;
    }
}

/// Short status text for hover prompts ("" when fine or not relevant).
pub fn hover_suffix(grid: &Grid, kind: SK, id: u32, l: Lang) -> &'static str {
    if !super::is_consumer(kind) {
        return "";
    }
    match grid.nets.status(id) {
        Status::Ok => "",
        Status::Low => t(l, "  ⚠ güç yetersiz", "  ⚠ low power"),
        Status::NoPower => t(l, "  ⚠ güç yok", "  ⚠ no power"),
    }
}

/// Power section of a structure panel: status with the reason, the cables of this structure
/// (each can be cut) and the structures in reach it can be cabled to.
pub fn power_section(ui: &mut egui::Ui, game: &mut Game, grid: &Grid, id: u32, l: Lang) {
    let Some(me) = game.structs().iter().find(|s| s.id == id).cloned() else { return };
    if !super::carries_power(me.kind) {
        return;
    }
    let net = grid.nets.of(id).unwrap_or_default();
    let st = super::status_of(&net);
    let consumer = super::is_consumer(me.kind);
    let (icon, text) = match (st, consumer) {
        (Status::NoPower, true) => ("⛔", t(l, "Güç yok: bu yapı bir jeneratöre kabloyla bağlı değil.", "No power: not cabled to any generator.")),
        (Status::NoPower, false) => ("⛔", t(l, "Bu şebekede güç üreten bir şey yok.", "Nothing on this grid produces power.")),
        (Status::Low, _) => ("⚠", t(l, "Şebeke tüketimi karşılayamıyor: makineler yavaş çalışır.", "The grid cannot cover the load: machines run slowly.")),
        (Status::Ok, _) => ("✔", t(l, "Şebekeye bağlı ve güçlü.", "Connected and powered.")),
    };
    ui.label(RichText::new(format!("{icon} {text}")).size(14.5).color(status_color(st)));
    let mut line = format!("⚡ {:.0} / {:.0}  •  {} {}", net.supply, net.demand, net.members, t(l, "yapı", "parts"));
    if net.capacity > 0.0 {
        line += &format!("  •  🔋 {:.0}/{:.0}", net.battery, net.capacity);
    }
    if net.demand > 0.0 && net.ratio < 0.99 {
        line += &format!("  •  {:.0}%", net.ratio * 100.0);
    }
    ui.label(RichText::new(line).size(13.0).color(DIM));

    let pos = bevy::math::Vec3::from_array(me.pos);
    let candidates = super::link_candidates(game.structs(), id);
    let header = format!("⚡ {} ({})", t(l, "Kablolar", "Cables"), me.links.len());
    let mut cut = None;
    let mut add = None;
    egui::CollapsingHeader::new(RichText::new(header).size(15.0)).id_salt(("cables", id)).default_open(st != Status::Ok || me.links.is_empty()).show(ui, |ui| {
        for &other in &me.links {
            let Some(o) = game.structs().iter().find(|s| s.id == other) else { continue };
            let d = bevy::math::Vec3::from_array(o.pos).distance(pos);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("— {} ({:.0} m)", o.kind.name(l), d)).size(14.0));
                if ui.add(egui::Button::new(RichText::new(format!("✂ {}", t(l, "Ayır", "Unplug"))).size(14.0)).min_size(egui::vec2(96.0, 34.0))).clicked() {
                    cut = Some(other);
                }
            });
        }
        if !candidates.is_empty() {
            ui.label(RichText::new(t(l, "Menzildeki yapılara bağla:", "Cable to something in reach:")).size(13.0).color(DIM));
            ui.horizontal_wrapped(|ui| {
                for &(other, d) in candidates.iter().take(6) {
                    let Some(o) = game.structs().iter().find(|s| s.id == other) else { continue };
                    let powered = grid.nets.of(other).map(|n| n.powered()).unwrap_or(false);
                    let mark = if powered { "⚡" } else { "○" };
                    let b = egui::Button::new(RichText::new(format!("{mark} {} {:.0} m", o.kind.name(l), d)).size(14.0).color(if powered { ORANGE } else { Color32::WHITE })).min_size(egui::vec2(0.0, 36.0));
                    if ui.add(b).clicked() {
                        add = Some(other);
                    }
                }
            });
        } else if me.links.is_empty() {
            let reach = if me.kind == SK::Pylon { super::PYLON_REACH } else { super::CABLE_REACH };
            ui.label(RichText::new(format!(
                "{} {:.0} m. {}",
                t(l, "Kablo menzilinde bağlanacak yapı yok:", "Nothing to cable to within"),
                reach,
                t(l, "Araya Güç Direği kur (26 m'ye kadar taşır) ya da yapıyı sök ve yakına yeniden kur (tam iade).", "Bridge the gap with Power Pylons (they carry 26 m) or dismantle and rebuild closer (full refund).")
            )).size(13.0).color(LOW));
        }
    });
    if let Some(o) = cut {
        super::unlink(game.structs_mut(), id, o);
    }
    if let Some(o) = add {
        let _ = super::link(game.structs_mut(), id, o);
    }
    if me.kind == SK::Beacon {
        let live = grid.sources.iter().any(|s| s.0 == id);
        let txt = if live {
            format!("{}: {:.0} m", t(l, "Oksijen alanı açık", "Oxygen zone on"), me.kind.def().oxygen)
        } else {
            t(l, "Oksijen alanı kapalı: istasyon tam güç ister.", "Oxygen zone off: the station needs full power.").to_string()
        };
        ui.label(RichText::new(txt).size(14.0).color(if live { Color32::from_rgb(140, 220, 255) } else { BAD }));
    }
}
