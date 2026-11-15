//! All user interface, drawn with egui: menus, HUD, tablet (inventory / build / research /
//! system map / journal), structure panels and touch controls.
use crate::galaxy::StarSystem;
use crate::gameplay::{give, PowerStat, BATTERY_CAP, BIO_BURN};
use crate::items::{tx, Item, Lang, Station, Tech, RECIPES, SHIP_PARTS, SK};
use crate::player::{Controls, Player};
use crate::state::{self, Game, Job};
use crate::story;
use crate::world::{DecorEnt, DecorKind, PlanetRt, Sky};
use crate::{Action, AppState, Cine, MainCam, Sys, Tab, ToolMode, Ui};
use bevy::prelude::*;
use bevy_egui::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke};
use bevy_egui::EguiContexts;

fn t(l: Lang, tr: &'static str, en: &'static str) -> &'static str {
    match l {
        Lang::Tr => tr,
        Lang::En => en,
    }
}

fn c32(c: [f32; 3]) -> Color32 {
    Color32::from_rgb((c[0].clamp(0.0, 1.0) * 255.0) as u8, (c[1].clamp(0.0, 1.0) * 255.0) as u8, (c[2].clamp(0.0, 1.0) * 255.0) as u8)
}

const ACCENT: Color32 = Color32::from_rgb(255, 150, 50);
const CYAN: Color32 = Color32::from_rgb(110, 220, 255);
const PANEL: Color32 = Color32::from_rgba_premultiplied(14, 18, 28, 235);
const DIM: Color32 = Color32::from_rgb(150, 160, 175);

fn panel() -> egui::Frame {
    egui::Frame::new().fill(PANEL).corner_radius(10).inner_margin(egui::Margin::same(12)).stroke(Stroke::new(1.0_f32, Color32::from_rgb(50, 62, 84)))
}

fn hud_frame() -> egui::Frame {
    egui::Frame::new().fill(Color32::from_rgba_premultiplied(8, 11, 18, 170)).corner_radius(8).inner_margin(egui::Margin::same(8))
}

pub fn style(mut contexts: EguiContexts, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    *done = true;
    let mut v = egui::Visuals::dark();
    v.window_fill = PANEL;
    v.panel_fill = PANEL;
    v.window_corner_radius = 12.into();
    v.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(50, 62, 84));
    v.widgets.inactive.corner_radius = 6.into();
    v.widgets.hovered.corner_radius = 6.into();
    v.widgets.active.corner_radius = 6.into();
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(36, 44, 62);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(58, 72, 100);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(200, 110, 40);
    v.selection.bg_fill = Color32::from_rgb(190, 100, 30);
    v.override_text_color = Some(Color32::from_rgb(228, 232, 240));
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        for (_, f) in s.text_styles.iter_mut() {
            f.size *= 1.2;
        }
        s.spacing.item_spacing = egui::vec2(8.0, 7.0);
        s.spacing.button_padding = egui::vec2(10.0, 6.0);
        s.interaction.selectable_labels = false;
    });
}

fn big_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(RichText::new(text).size(20.0)).min_size(egui::vec2(280.0, 44.0))).clicked()
}

fn backdrop(ctx: &egui::Context, alpha: u8) {
    let r = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background()).rect_filled(r, 0.0, Color32::from_black_alpha(alpha));
}

#[allow(clippy::too_many_arguments)]
pub fn menu_ui(mut commands: Commands, mut contexts: EguiContexts, mut ui_s: ResMut<Ui>, mut next: ResMut<NextState<AppState>>, mut exit: EventWriter<AppExit>, time: Res<Time>) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    let ctx = ctx.clone();
    let l = ui_s.lang;
    let has_save = state::save_exists();
    egui::Area::new("menu".into()).anchor(Align2::LEFT_CENTER, [70.0, 0.0]).show(&ctx, |ui| {
        ui.label(RichText::new("LOSTRANOUT").size(64.0).strong().color(Color32::WHITE));
        ui.label(RichText::new(t(l, "Yakıt bitti. Yol bitmedi.", "The fuel ran out. The road did not.")).size(20.0).color(ACCENT));
        ui.add_space(28.0);
        if big_button(ui, t(l, "Devam Et", "Continue"), has_save) {
            if let Some(game) = state::load_game() {
                ui_s.lang = game.lang;
                commands.insert_resource(Sys(StarSystem::generate(game.seed, game.system_index)));
                let finished = game.finished;
                commands.insert_resource(game);
                next.set(if finished { AppState::Ending } else { AppState::Loading });
            }
        }
        if ui_s.confirm_new {
            ui.label(RichText::new(t(l, "Mevcut kayıt silinecek. Emin misin?", "The current save will be overwritten. Are you sure?")).color(ACCENT));
            ui.horizontal(|ui| {
                if ui.button(t(l, "Evet, yeni oyun", "Yes, new game")).clicked() {
                    ui_s.confirm_new = false;
                    start_new(&mut commands, &mut ui_s, &mut next, &time);
                }
                if ui.button(t(l, "Vazgeç", "Cancel")).clicked() {
                    ui_s.confirm_new = false;
                }
            });
        } else if big_button(ui, t(l, "Yeni Oyun", "New Game"), true) {
            if has_save {
                ui_s.confirm_new = true;
            } else {
                start_new(&mut commands, &mut ui_s, &mut next, &time);
            }
        }
        if big_button(ui, t(l, "Dil: Türkçe", "Language: English"), true) {
            ui_s.lang = if l == Lang::Tr { Lang::En } else { Lang::Tr };
            state::save_lang(ui_s.lang);
        }
        if !cfg!(target_os = "android") && big_button(ui, t(l, "Çıkış", "Quit"), true) {
            exit.write(AppExit::Success);
        }
        ui.add_space(20.0);
        let hint = if cfg!(target_os = "android") {
            t(l, "Sol alt: yürü  •  Ekranı sürükle: kamera\n\"ALET: AÇIK\" iken araziye dokun: kaz / doldur / düzle", "Bottom-left: walk  •  Drag the screen: camera\nWith \"TOOL: ON\", touch the ground: dig / fill / flatten")
        } else {
            t(l, "WASD: yürü  •  Sağ tık sürükle: kamera  •  Sol tık: kazı aleti\nE: etkileşim  •  Tab: tablet  •  B: inşa  •  1/2/3: alet modu", "WASD: walk  •  Right-drag: camera  •  Left click: excavator\nE: interact  •  Tab: tablet  •  B: build  •  1/2/3: tool mode")
        };
        ui.label(RichText::new(hint).size(14.0).color(DIM));
    });
}

fn start_new(commands: &mut Commands, ui_s: &mut Ui, next: &mut NextState<AppState>, time: &Time) {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7) ^ (time.elapsed_secs_f64() * 1e6) as u64;
    let game = Game::new(seed, ui_s.lang);
    commands.insert_resource(Sys(StarSystem::generate(game.seed, 0)));
    commands.insert_resource(game);
    ui_s.intro_page = 0;
    next.set(AppState::Intro);
}

pub fn intro_ui(mut contexts: EguiContexts, mut ui_s: ResMut<Ui>, sys: Res<Sys>, mut next: ResMut<NextState<AppState>>) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    let ctx = ctx.clone();
    let l = ui_s.lang;
    backdrop(&ctx, 235);
    let pages = story::intro_pages(sys.0.index);
    let page = ui_s.intro_page.min(pages.len() - 1);
    let text = story::fill(tx(l, pages[page]), &sys.0.name, &sys.0.planets[sys.0.home].name);
    egui::Area::new("intro".into()).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(&ctx, |ui| {
        ui.set_max_width(680.0);
        ui.label(RichText::new(text).size(24.0).color(Color32::from_rgb(235, 235, 240)));
        ui.add_space(30.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new(t(l, "Devam  ▶", "Continue  ▶")).size(20.0)).min_size(egui::vec2(180.0, 42.0))).clicked() {
                if page + 1 >= pages.len() {
                    next.set(AppState::Loading);
                } else {
                    ui_s.intro_page = page + 1;
                }
            }
            if ui.button(t(l, "Atla", "Skip")).clicked() {
                next.set(AppState::Loading);
            }
            ui.label(RichText::new(format!("{}/{}", page + 1, pages.len())).color(DIM));
        });
    });
}

pub fn loading_ui(mut contexts: EguiContexts, ui_s: Res<Ui>, sys: Res<Sys>, game: Res<Game>) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    let ctx = ctx.clone();
    backdrop(&ctx, 255);
    let l = ui_s.lang;
    let p = &sys.0.planets[game.planet.min(sys.0.planets.len() - 1)];
    egui::Area::new("loading".into()).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(&ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(&p.name).size(48.0).strong());
            ui.label(RichText::new(format!("{}  •  {} {}", tx(l, p.kind.name()), sys.0.name, t(l, "sistemi", "system"))).size(20.0).color(ACCENT));
            ui.add_space(16.0);
            ui.label(RichText::new(t(l, "Gezegen oluşturuluyor…", "Generating planet…")).size(18.0).color(DIM));
        });
    });
}

pub fn ending_ui(mut commands: Commands, mut contexts: EguiContexts, mut ui_s: ResMut<Ui>, mut game: ResMut<Game>, sys: Res<Sys>, mut next: ResMut<NextState<AppState>>) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    let ctx = ctx.clone();
    backdrop(&ctx, 245);
    let l = ui_s.lang;
    let text = story::fill(tx(l, story::ending(sys.0.index)), &sys.0.name, &sys.0.planets[sys.0.home].name);
    egui::Area::new("ending".into()).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(&ctx, |ui| {
        ui.set_max_width(680.0);
        ui.label(RichText::new(text).size(22.0));
        ui.add_space(18.0);
        let mins = (game.time / 60.0) as u32;
        ui.label(RichText::new(format!("{}: {} {}  •  {}: {}  •  {}: {}", t(l, "Süre", "Time"), mins, t(l, "dk", "min"), t(l, "Ziyaret edilen sistem", "Systems visited"), game.system_index + 1, t(l, "Kara kutu kayıtları", "Black box logs"), game.fragments)).color(DIM));
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new(t(l, "Sonraki yıldıza devam et", "Continue to the next star")).size(20.0)).min_size(egui::vec2(300.0, 44.0))).clicked() {
                game.advance_system();
                let _ = state::save_game(&game);
                commands.insert_resource(Sys(StarSystem::generate(game.seed, game.system_index)));
                // the next chapter opens with its own arrival pages
                ui_s.intro_page = 0;
                next.set(AppState::Intro);
            }
            if ui.button(t(l, "Ana menü", "Main menu")).clicked() {
                next.set(AppState::Menu);
            }
        });
    });
}

// ------------------------------------------------------------------------------------------

fn item_icon(painter: &egui::Painter, c: egui::Pos2, r: f32, item: Item) {
    let col = c32(item.def().color);
    let dark = Color32::from_rgb((col.r() as f32 * 0.55) as u8, (col.g() as f32 * 0.55) as u8, (col.b() as f32 * 0.55) as u8);
    let idx = Item::ALL.iter().position(|i| *i == item).unwrap_or(0);
    let poly = |n: usize, rot: f32, rr: f32| -> Vec<egui::Pos2> {
        (0..n).map(|i| { let a = rot + i as f32 / n as f32 * std::f32::consts::TAU; egui::pos2(c.x + a.cos() * rr, c.y + a.sin() * rr) }).collect()
    };
    let stroke = Stroke::new(1.5_f32, dark);
    match idx {
        0..=9 => {
            painter.add(egui::Shape::convex_polygon(poly(6, 0.3, r), col, stroke));
            painter.add(egui::Shape::convex_polygon(poly(3, 0.9, r * 0.45), dark, Stroke::NONE));
        }
        10..=19 => {
            let rect = egui::Rect::from_center_size(c, egui::vec2(r * 1.9, r * 1.1));
            painter.rect_filled(rect, 3.0, col);
            painter.rect_filled(rect.shrink2(egui::vec2(r * 0.3, r * 0.3)), 2.0, dark);
        }
        20..=22 => {
            painter.add(egui::Shape::convex_polygon(poly(8, 0.0, r), col, stroke));
            painter.circle_filled(c, r * 0.35, dark);
        }
        23..=26 => {
            painter.add(egui::Shape::convex_polygon(poly(4, 0.0, r * 1.1), col, Stroke::new(2.0_f32, Color32::WHITE)));
        }
        _ => {
            painter.circle_filled(c, r * 0.9, col);
            painter.circle_stroke(c, r * 0.9, Stroke::new(2.0_f32, Color32::WHITE));
        }
    }
}

fn slot(ui: &mut egui::Ui, l: Lang, content: Option<(Item, u32)>, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(74.0, 78.0), Sense::click());
    let p = ui.painter();
    let bg = if selected { Color32::from_rgb(90, 60, 30) } else if resp.hovered() { Color32::from_rgb(48, 58, 80) } else { Color32::from_rgb(28, 34, 48) };
    p.rect_filled(rect, 6.0, bg);
    if let Some((item, n)) = content {
        item_icon(p, rect.center() - egui::vec2(0.0, 12.0), 17.0, item);
        p.text(rect.right_top() + egui::vec2(-5.0, 4.0), Align2::RIGHT_TOP, n.to_string(), FontId::proportional(15.0), Color32::WHITE);
        let name = item.name(l);
        let short: String = if name.chars().count() > 11 { name.chars().take(10).collect::<String>() + "…" } else { name.to_string() };
        p.text(rect.center_bottom() - egui::vec2(0.0, 5.0), Align2::CENTER_BOTTOM, short, FontId::proportional(11.5), Color32::from_rgb(200, 205, 215));
    }
    resp
}

fn cost_row(ui: &mut egui::Ui, l: Lang, game: &Game, cost: &[(Item, u32)]) {
    ui.horizontal_wrapped(|ui| {
        for &(item, n) in cost {
            let have = game.inv.count(item);
            let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
            item_icon(ui.painter(), r.center(), 7.5, item);
            let col = if have >= n { Color32::from_rgb(150, 230, 150) } else { Color32::from_rgb(255, 130, 110) };
            ui.label(RichText::new(format!("{} {}/{}", item.name(l), have, n)).color(col).size(14.0));
        }
    });
}

fn bar(ui: &mut egui::Ui, label: &str, frac: f32, color: Color32, text: String) {
    ui.horizontal(|ui| {
        ui.add_sized([58.0, 18.0], egui::Label::new(RichText::new(label).size(14.0)));
        ui.add(egui::ProgressBar::new(frac.clamp(0.0, 1.0)).desired_width(150.0).fill(color).text(RichText::new(text).size(13.0)));
    });
}

#[allow(clippy::too_many_arguments)]
pub fn play_ui(
    mut contexts: EguiContexts,
    time: Res<Time>,
    mut ui_s: ResMut<Ui>,
    mut game: ResMut<Game>,
    sys: Res<Sys>,
    rt: Res<PlanetRt>,
    sky: Res<Sky>,
    power: Res<PowerStat>,
    cine: Res<Cine>,
    controls: Res<Controls>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    player: Query<&Transform, With<Player>>,
    decor: Query<(&DecorEnt, &Transform)>,
    (grid, mut net_ui): (Res<crate::network::Grid>, ResMut<crate::network::hud::NetUi>),
    mut fps: Local<(f32, u32, f32)>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };
    let ctx = ctx.clone();
    let ui_s = &mut *ui_s;
    let game = &mut *game;
    fps.0 += time.delta_secs();
    fps.1 += 1;
    if fps.0 >= 0.5 {
        fps.2 = fps.1 as f32 / fps.0;
        fps.0 = 0.0;
        fps.1 = 0;
    }
    game.lang = ui_s.lang;
    let l = ui_s.lang;
    let dt = time.delta_secs();
    let screen = ctx.screen_rect();
    let narrow = screen.width() < 900.0;
    // On a phone the corners are needed by the tablet / machine panels: the HUD boxes sit below
    // the top button row, and step aside completely while a panel is open.
    let hide_hud = narrow && (ui_s.open || ui_s.machine.is_some());
    let hud_y = if narrow { 70.0 } else { 10.0 };

    if cine.0.is_some() {
        egui::Area::new("cine".into()).anchor(Align2::CENTER_BOTTOM, [0.0, -60.0]).show(&ctx, |ui| {
            hud_frame().show(ui, |ui| {
                let text = match cine.0.unwrap().0 {
                    crate::CineKind::Launch => t(l, "Fırlatma…", "Lift-off…"),
                    crate::CineKind::Land => t(l, "İniş…", "Landing…"),
                    crate::CineKind::Finale => t(l, "Öncü II fırlatılıyor…", "Pioneer II launching…"),
                };
                ui.label(RichText::new(text).size(22.0).color(ACCENT));
            });
        });
        return;
    }

    // ---------------- world-space markers
    if let (Ok((camera, cgt)), Ok(ptf)) = (cam.single(), player.single()) {
        let painter = ctx.layer_painter(egui::LayerId::background());
        let ppos = ptf.translation;
        let mark = |pos: Vec3, col: Color32, label: String| {
            if let Ok(sp) = camera.world_to_viewport(cgt, pos) {
                let p = egui::pos2(sp.x, sp.y);
                painter.circle_filled(p, 5.0, col);
                painter.circle_stroke(p, 7.0, Stroke::new(1.5_f32, Color32::from_black_alpha(160)));
                painter.text(p + egui::vec2(0.0, -10.0), Align2::CENTER_BOTTOM, label, FontId::proportional(13.0), col);
            }
        };
        for s in game.structs() {
            let sp = Vec3::from_array(s.pos);
            let d = sp.distance(ppos);
            if matches!(s.kind, SK::Pod | SK::Shuttle | SK::Shipyard) && d > 24.0 {
                mark(sp + sp.normalize() * 3.0, ACCENT, format!("{} {:.0}m", s.kind.name(l), d));
            }
        }
        if game.has_tech(Tech::Scanner) {
            let mut nearest: Vec<(Item, f32, Vec3)> = Vec::new();
            for (d, tf) in &decor {
                if let DecorKind::Crystal(item) = d.kind {
                    let dist = tf.translation.distance(ppos);
                    if dist > 90.0 || dist < 6.0 {
                        continue;
                    }
                    match nearest.iter_mut().find(|n| n.0 == item) {
                        Some(n) => {
                            if dist < n.1 {
                                *n = (item, dist, tf.translation)
                            }
                        }
                        None => nearest.push((item, dist, tf.translation)),
                    }
                }
            }
            for (item, dist, pos) in nearest {
                mark(pos + pos.normalize(), c32(item.def().color), format!("{} {:.0}m", item.name(l), dist));
            }
        }
    }

    // ---------------- touch joystick
    if let Some(o) = controls.joy_origin {
        let p = ctx.layer_painter(egui::LayerId::background());
        let r = screen.height() * 0.11;
        p.circle_filled(egui::pos2(o.x, o.y), r, Color32::from_white_alpha(18));
        p.circle_stroke(egui::pos2(o.x, o.y), r, Stroke::new(2.0_f32, Color32::from_white_alpha(70)));
        p.circle_filled(egui::pos2(controls.joy_pos.x, controls.joy_pos.y), r * 0.42, Color32::from_white_alpha(90));
    }

    // ---------------- status (top-left)
    if !hide_hud { egui::Area::new("status".into()).anchor(Align2::LEFT_TOP, [10.0, hud_y]).show(&ctx, |ui| {
        hud_frame().show(ui, |ui| {
            let o2c = if game.oxygen < 25.0 { Color32::from_rgb(230, 70, 60) } else { Color32::from_rgb(70, 170, 235) };
            bar(ui, "O₂", game.oxygen / 100.0, o2c, format!("{:.0}%{}", game.oxygen, if ui_s.in_oxygen { " +" } else { "" }));
            crate::network::hud::oxygen_hud(ui, &grid, game, crate::gameplay::o2_drain(rt.def.atmosphere), l, controls.touch);
            bar(ui, t(l, "Sağlık", "Health"), game.health / 100.0, Color32::from_rgb(210, 70, 80), format!("{:.0}", game.health));
            if game.has_tech(Tech::Jetpack) {
                bar(ui, "Jet", game.jet / 100.0, Color32::from_rgb(240, 170, 60), String::new());
            }
            bar(ui, t(l, "Toprak", "Soil"), game.soil / game.max_soil(), Color32::from_rgb(150, 110, 70), format!("{:.0}", game.soil));
            crate::network::hud::power_hud(ui, &power, game, l);
            ui.label(RichText::new(format!("{}: {}", t(l, "Veri", "Data"), game.data)).size(14.0).color(CYAN));
        });
    }); }

    // ---------------- title + menu buttons (top-centre)
    // Every HUD area is kept tight around its widgets: whatever is not covered by a widget
    // belongs to the world (camera drag, terrain tool, touch joystick).
    egui::Area::new("title".into()).anchor(Align2::CENTER_TOP, [0.0, 6.0]).interactable(false).show(&ctx, |ui| {
        let tod = if sky.day > 0.6 { t(l, "gündüz", "day") } else if sky.day > 0.1 { t(l, "alacakaranlık", "twilight") } else { t(l, "gece", "night") };
        ui.add(egui::Label::new(RichText::new(format!("{}  •  {}", rt.def.name, tod)).size(17.0).strong()).extend());
    });
    egui::Area::new("top".into()).anchor(Align2::CENTER_TOP, [0.0, 32.0]).show(&ctx, |ui| {
        {
            ui.horizontal(|ui| {
                let journal_label = if game.journal_unread { t(l, "Günlük (!)", "Journal (!)") } else { t(l, "Günlük", "Journal") };
                for (tab, label) in [(Tab::Inventory, t(l, "Envanter", "Inventory")), (Tab::Build, t(l, "İnşa", "Build")), (Tab::Research, t(l, "Araştırma", "Research")), (Tab::Map, t(l, "Harita", "Map")), (Tab::Journal, journal_label)] {
                    let sel = ui_s.open && ui_s.tab == tab;
                    if ui.add(egui::Button::new(RichText::new(label).size(15.0)).selected(sel)).clicked() {
                        if sel {
                            ui_s.open = false;
                        } else {
                            ui_s.open = true;
                            ui_s.tab = tab;
                            ui_s.machine = None;
                        }
                    }
                }
                if ui.add(egui::Button::new(RichText::new(t(l, "Menü", "Menu")).size(15.0))).clicked() {
                    ui_s.paused = !ui_s.paused;
                }
            });
        }
    });

    // ---------------- objective (top-right)
    if !hide_hud { egui::Area::new("quest".into()).anchor(Align2::RIGHT_TOP, [-10.0, hud_y]).show(&ctx, |ui| {
        hud_frame().show(ui, |ui| {
            ui.set_max_width(if narrow { 200.0 } else { 290.0 });
            ui.label(RichText::new(t(l, "GÖREV", "OBJECTIVE")).size(12.0).color(ACCENT));
            if game.quest < story::QUESTS.len() {
                ui.label(RichText::new(tx(l, story::QUESTS[game.quest].text)).size(15.0));
                if game.quest == story::Q_DELIVER {
                    if let Some(s) = game.planets.iter().flat_map(|p| p.structs.iter()).find(|s| s.kind == SK::Shipyard) {
                        for (item, n) in SHIP_PARTS {
                            ui.label(RichText::new(format!("  {} {}/{}", item.name(l), s.delivered_count(item), n)).size(13.0).color(DIM));
                        }
                    }
                }
            } else {
                ui.label(RichText::new(t(l, "İşaretleri izle.", "Follow the beacons.")).size(15.0));
            }
        });
        // pickups feed
        ui_s.pickups.retain_mut(|p| {
            p.2 -= dt;
            p.2 > 0.0
        });
        for (item, n, ttl) in &ui_s.pickups {
            let a = (ttl.min(1.0) * 255.0) as u8;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::hover());
                item_icon(ui.painter(), r.center(), 8.0, *item);
                ui.label(RichText::new(format!("+{} {}", n, item.name(l))).size(15.0).color(Color32::from_white_alpha(a)));
            });
        }
    }); }

    // ---------------- toasts
    ui_s.toasts.retain_mut(|x| {
        x.1 -= dt;
        x.1 > 0.0
    });
    egui::Area::new("toasts".into()).anchor(Align2::CENTER_TOP, [0.0, 86.0]).interactable(false).show(&ctx, |ui| {
        ui.set_width(screen.width().min(700.0));
        ui.vertical_centered(|ui| {
            for (text, _, col) in &ui_s.toasts {
                hud_frame().show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(text).size(16.0).color(c32(*col))).extend());
                });
            }
        });
    });

    // ---------------- bottom bar: tool / placement
    let bottom = if cfg!(target_os = "android") { -22.0 } else { -10.0 };
    if let (Some(text), false, None, None) = (ui_s.hover_text.clone(), ui_s.open, ui_s.machine, ui_s.placing) {
        egui::Area::new("prompt".into()).anchor(Align2::CENTER_BOTTOM, [0.0, bottom - 56.0]).show(&ctx, |ui| {
            let label = if controls.touch { text } else { format!("[E]  {text}") };
            if ui.add(egui::Button::new(RichText::new(label).size(18.0).color(Color32::WHITE)).fill(Color32::from_rgb(170, 90, 25)).min_size(egui::vec2(0.0, 34.0))).clicked() {
                ui_s.v_interact = true;
            }
        });
    }
    if !hide_hud { egui::Area::new("tool".into()).anchor(Align2::CENTER_BOTTOM, [0.0, bottom]).show(&ctx, |ui| {
        {
            hud_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(kind) = ui_s.placing {
                        ui.label(RichText::new(format!("{}: {}", t(l, "Yerleştir", "Place"), kind.name(l))).size(16.0).color(ACCENT));
                        if let Some(p) = &ui_s.place_problem {
                            ui.label(RichText::new(p).color(Color32::from_rgb(255, 120, 100)));
                        }
                        if ui.button(t(l, "✔ Onayla", "✔ Confirm")).clicked() {
                            ui_s.place_confirm = true;
                        }
                        if ui.button(t(l, "✖ İptal", "✖ Cancel")).clicked() {
                            ui_s.placing = None;
                        }
                    } else {
                        if controls.touch {
                            let label = if ui_s.tool_armed { t(l, "ALET: AÇIK", "TOOL: ON") } else { t(l, "ALET: KAPALI", "TOOL: OFF") };
                            if ui.add(egui::Button::new(RichText::new(label).size(15.0)).selected(ui_s.tool_armed)).clicked() {
                                ui_s.tool_armed = !ui_s.tool_armed;
                            }
                        }
                        for (mode, label) in [(ToolMode::Dig, t(l, "1 Kaz", "1 Dig")), (ToolMode::Fill, t(l, "2 Doldur", "2 Fill")), (ToolMode::Flatten, t(l, "3 Düzle", "3 Flatten"))] {
                            if ui.add(egui::Button::new(RichText::new(label).size(15.0)).selected(ui_s.tool_mode == mode)).clicked() {
                                ui_s.tool_mode = mode;
                            }
                        }
                        if ui.button("−").clicked() {
                            ui_s.brush = (ui_s.brush - 0.5).max(1.5);
                        }
                        ui.label(RichText::new(format!("{} {:.1}", t(l, "Yarıçap", "Radius"), ui_s.brush)).size(14.0));
                        if ui.button("+").clicked() {
                            ui_s.brush = (ui_s.brush + 0.5).min(ui_s.max_brush);
                        }
                    }
                });
            });
        }
    }); }

    // ---------------- touch buttons
    ui_s.v_jump = false;
    if controls.touch && !hide_hud {
        egui::Area::new("touch".into()).anchor(Align2::RIGHT_BOTTOM, [-14.0, -14.0]).show(&ctx, |ui| {
            ui.horizontal(|ui| {
                crate::network::hud::tether_button(ui, &mut net_ui, game, l);
                ui.vertical(|ui| {
                    if ui.add(egui::Button::new(RichText::new("+").size(22.0)).min_size(egui::vec2(48.0, 44.0))).clicked() {
                        ui_s.v_zoom = 2.0;
                    }
                    if ui.add(egui::Button::new(RichText::new("-").size(22.0)).min_size(egui::vec2(48.0, 44.0))).clicked() {
                        ui_s.v_zoom = -2.0;
                    }
                });
                let jump = ui.add(egui::Button::new(RichText::new(t(l, "ZIPLA", "JUMP")).size(17.0)).min_size(egui::vec2(96.0, 96.0)).corner_radius(48).sense(Sense::drag()));
                if jump.is_pointer_button_down_on() || jump.dragged() {
                    ui_s.v_jump = true;
                }
            });
        });
    }

    // ---------------- tablet
    if ui_s.open {
        let w = (screen.width() - 30.0).min(820.0);
        let h = (screen.height() - if narrow { 112.0 } else { 150.0 }).clamp(200.0, 520.0);
        egui::Window::new("tablet").title_bar(false).resizable(false).collapsible(false).anchor(Align2::CENTER_CENTER, [0.0, 20.0]).fixed_size([w, h]).frame(panel()).show(&ctx, |ui| {
            ui.set_min_size(egui::vec2(w, h));
            match ui_s.tab {
                Tab::Inventory => inventory_tab(ui, ui_s, game, l),
                Tab::Build => build_tab(ui, ui_s, game, l),
                Tab::Research => research_tab(ui, ui_s, game, l),
                Tab::Map => map_tab(ui, ui_s, game, &sys.0, l, h),
                Tab::Journal => journal_tab(ui, ui_s, game, &sys.0, l, h),
            }
        });
    }

    // ---------------- structure panel
    if let Some(id) = ui_s.machine {
        if game.structs().iter().any(|s| s.id == id) {
            let mut keep = true;
            egui::Window::new("machine").title_bar(false).resizable(false).collapsible(false).anchor(Align2::CENTER_CENTER, [0.0, 20.0]).frame(panel()).show(&ctx, |ui| {
                ui.set_width((screen.width() - 40.0).min(560.0));
                keep = structure_panel(ui, ui_s, game, &sys.0, &grid, id, l, (screen.height() - 340.0).max(110.0));
            });
            if !keep {
                ui_s.machine = None;
            }
        } else {
            ui_s.machine = None;
        }
    }

    // ---------------- pause
    if ui_s.paused {
        backdrop(&ctx, 150);
        egui::Window::new("pause").title_bar(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).frame(panel()).show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t(l, "DURAKLATILDI", "PAUSED")).size(26.0).strong());
                ui.label(RichText::new(format!("{} • {} {} • {:.0} FPS", sys.0.name, sys.0.planets.len(), t(l, "gezegen", "planets"), fps.2)).color(DIM));
                ui.add_space(8.0);
                if big_button(ui, t(l, "Devam", "Resume"), true) {
                    ui_s.paused = false;
                }
                if big_button(ui, t(l, "Kaydet", "Save"), true) {
                    ui_s.actions.push(Action::Save);
                    ui_s.paused = false;
                }
                if big_button(ui, t(l, "Dil: Türkçe", "Language: English"), true) {
                    ui_s.lang = if l == Lang::Tr { Lang::En } else { Lang::Tr };
                    state::save_lang(ui_s.lang);
                }
                if big_button(ui, t(l, "Kaydet ve ana menüye dön", "Save and exit to menu"), true) {
                    ui_s.actions.push(Action::ToMenu);
                }
                ui.add_space(8.0);
if !cfg!(target_os = "android") {
                    ui.label(RichText::new(t(l, "WASD yürü • Shift koş • Boşluk zıpla / jetpack\nSağ tık sürükle: kamera • Tekerlek: yakınlaş\nSol tık: kazı aleti • 1/2/3: kaz / doldur / düzle • Z/X: çap\nE: etkileşim • Tab: envanter • B: inşa • R: araştırma\nM: harita • J: günlük • Esc: kapat / duraklat", "WASD walk • Shift run • Space jump / jetpack\nRight-drag: camera • Wheel: zoom\nLeft click: excavator • 1/2/3: dig / fill / flatten • Z/X: size\nE: interact • Tab: inventory • B: build • R: research\nM: map • J: journal • Esc: close / pause")).size(13.5).color(DIM));
                }
            });
        });
    }
}

fn tab_title(ui: &mut egui::Ui, ui_s: &mut Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(22.0).strong().color(ACCENT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("✖").clicked() {
                ui_s.open = false;
                ui_s.travel_from = None;
            }
        });
    });
    ui.separator();
}

fn inventory_tab(ui: &mut egui::Ui, ui_s: &mut Ui, game: &mut Game, l: Lang) {
    tab_title(ui, ui_s, t(l, "Sırt Çantası", "Backpack"));
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for i in 0..game.inv.slots.len() {
                if slot(ui, l, game.inv.slots[i], ui_s.inv_sel == Some(i)).clicked() {
                    ui_s.inv_sel = if ui_s.inv_sel == Some(i) { None } else { Some(i) };
                }
            }
        });
        ui.add_space(6.0);
        if let Some((item, n)) = ui_s.inv_sel.and_then(|i| game.inv.slots.get(i).copied().flatten()) {
            ui.label(RichText::new(format!("{} ×{}", item.name(l), n)).size(18.0).strong());
            ui.label(RichText::new(tx(l, item.def().desc)).color(DIM));
            if ui.button(t(l, "Yuvayı boşalt (at)", "Empty slot (discard)")).clicked() {
                if let Some(i) = ui_s.inv_sel {
                    game.inv.slots[i] = None;
                }
            }
        } else {
            ui.label(RichText::new(t(l, "Bir eşyaya dokunarak ayrıntısını gör. Madenleri Eritme Ocağında, parçaları Üretim Tezgahında işle.", "Tap an item for details. Process ores at the Smelter and parts at the Assembler.")).color(DIM));
        }
    });
}

fn build_tab(ui: &mut egui::Ui, ui_s: &mut Ui, game: &mut Game, l: Lang) {
    tab_title(ui, ui_s, t(l, "İnşa", "Build"));
    egui::ScrollArea::vertical().show(ui, |ui| {
        for kind in SK::BUILDABLE {
            let def = kind.def();
            let unlocked = game.tech_ok(def.tech);
            egui::Frame::new().fill(Color32::from_rgb(24, 30, 44)).corner_radius(6).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() - 140.0);
                        ui.label(RichText::new(tx(l, def.name)).size(17.0).strong().color(if unlocked { Color32::WHITE } else { DIM }));
                        ui.label(RichText::new(tx(l, def.desc)).size(13.5).color(DIM));
                        if unlocked {
                            cost_row(ui, l, game, def.cost);
                            if def.power != 0.0 {
                                ui.label(RichText::new(format!("⚡ {:+.0}", def.power)).size(13.0).color(if def.power > 0.0 { Color32::from_rgb(160, 235, 160) } else { Color32::from_rgb(240, 190, 120) }));
                            }
                        } else if let Some(tech) = def.tech {
                            ui.label(RichText::new(format!("🔒 {}: {}", t(l, "Araştırma gerekli", "Requires research"), tx(l, tech.def().name))).size(13.5).color(Color32::from_rgb(255, 170, 100)));
                        }
                    });
                    let can = unlocked && game.inv.has_all(def.cost);
                    if ui.add_enabled(can, egui::Button::new(RichText::new(t(l, "Yerleştir", "Place")).size(16.0)).min_size(egui::vec2(120.0, 40.0))).clicked() {
                        ui_s.placing = Some(kind);
                        ui_s.open = false;
                    }
                });
            });
        }
    });
}

fn research_tab(ui: &mut egui::Ui, ui_s: &mut Ui, game: &mut Game, l: Lang) {
    tab_title(ui, ui_s, t(l, "Araştırma", "Research"));
    ui.label(RichText::new(format!("{}: {}", t(l, "Veri", "Data"), game.data)).size(18.0).color(CYAN));
    ui.label(RichText::new(t(l, "Veri; numuneleri Araştırma İstasyonunda inceleyerek, yeni şeyler keşfederek ve görevleri tamamlayarak kazanılır.", "Data is earned by analysing specimens at a Research Station, discovering new things and completing objectives.")).size(13.5).color(DIM));
    egui::ScrollArea::vertical().show(ui, |ui| {
        for tech in Tech::ALL {
            let def = tech.def();
            let owned = game.has_tech(tech);
            let prereq_ok = def.prereq.map(|p| game.has_tech(p)).unwrap_or(true);
            egui::Frame::new().fill(Color32::from_rgb(24, 30, 44)).corner_radius(6).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() - 150.0);
                        ui.label(RichText::new(tx(l, def.name)).size(17.0).strong().color(if owned { Color32::from_rgb(150, 230, 150) } else { Color32::WHITE }));
                        ui.label(RichText::new(tx(l, def.desc)).size(13.5).color(DIM));
                        if let (Some(p), false) = (def.prereq, prereq_ok) {
                            ui.label(RichText::new(format!("🔒 {}: {}", t(l, "Önce", "First"), tx(l, p.def().name))).size(13.5).color(Color32::from_rgb(255, 170, 100)));
                        }
                    });
                    if owned {
                        ui.label(RichText::new("✔").size(22.0).color(Color32::from_rgb(150, 230, 150)));
                    } else if ui.add_enabled(prereq_ok && game.data >= def.cost, egui::Button::new(RichText::new(format!("{} ({})", t(l, "Araştır", "Unlock"), def.cost)).size(15.0)).min_size(egui::vec2(130.0, 40.0))).clicked() {
                        game.data -= def.cost;
                        game.techs.push(tech);
                        if tech == Tech::Backpack {
                            let n = game.inv.slots.len() + 8;
                            game.inv.slots.resize(n, None);
                        }
                        ui_s.toast(format!("{}: {}", t(l, "Araştırıldı", "Unlocked"), tx(l, def.name)), [0.5, 0.9, 1.0]);
                    }
                });
            });
        }
    });
}

fn map_tab(ui: &mut egui::Ui, ui_s: &mut Ui, game: &mut Game, sys: &StarSystem, l: Lang, h: f32) {
    tab_title(ui, ui_s, &format!("{}: {}  ({})", t(l, "Sistem Haritası", "System Map"), sys.name, tx(l, sys.class)));
    let sel = ui_s.map_sel.unwrap_or(game.planet).min(sys.planets.len() - 1);
    let side = (h - 70.0).min(ui.available_width() * 0.5).max(180.0);
    ui.horizontal_top(|ui| {
        let (resp, painter) = ui.allocate_painter(egui::vec2(side, side), Sense::click());
        let c = resp.rect.center();
        painter.rect_filled(resp.rect, 8.0, Color32::from_rgb(6, 8, 16));
        // everything scales with the map square so the system keeps its proportions on a phone
        painter.circle_filled(c, side * 0.045, c32(sys.star_color));
        painter.circle_filled(c, side * 0.068, Color32::from_rgba_unmultiplied(255, 230, 160, 40));
        let n = sys.planets.len();
        let (inner, outer) = (side * 0.15, side * 0.41);
        let font = (side * 0.045).clamp(9.5, 13.0);
        let mut hits = Vec::new();
        for (i, p) in sys.planets.iter().enumerate() {
            let r = inner + (outer - inner) * i as f32 / (n - 1).max(1) as f32;
            painter.circle_stroke(c, r, Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 70)));
            let a = p.phase + game.time as f32 * 0.02 / (1.0 + i as f32);
            let pos = c + egui::vec2(a.cos() * r, a.sin() * r);
            // planet radii are 220-380 m: map that range onto a small dot relative to the map
            let pr = side * (0.016 + 0.018 * ((p.radius - 200.0) / 200.0).clamp(0.0, 1.0));
            painter.circle_filled(pos, pr, c32(p.top));
            painter.circle_stroke(pos, pr, Stroke::new(1.0_f32, c32(p.rock)));
            if i == game.planet {
                painter.circle_stroke(pos, pr + 3.0, Stroke::new(2.0_f32, ACCENT));
            }
            if i == sel {
                painter.circle_stroke(pos, pr + 5.5, Stroke::new(1.5_f32, Color32::WHITE));
            }
            painter.text(pos + egui::vec2(0.0, pr + 4.0), Align2::CENTER_TOP, &p.name, FontId::proportional(font), Color32::from_rgb(220, 225, 235));
            hits.push((i, pos));
        }
        if resp.clicked() {
            if let Some(m) = resp.interact_pointer_pos() {
                if let Some((i, _)) = hits.iter().min_by(|a, b| a.1.distance(m).partial_cmp(&b.1.distance(m)).unwrap()) {
                    ui_s.map_sel = Some(*i);
                }
            }
        }
        ui.vertical(|ui| {
            egui::ScrollArea::vertical().max_height(side).show(ui, |ui| {
                let p = &sys.planets[sel];
                ui.label(RichText::new(&p.name).size(24.0).strong());
                ui.label(RichText::new(tx(l, p.kind.name())).color(ACCENT));
                ui.label(format!("{}: {:.0} m   •   {}: {:.1} m/s²", t(l, "Yarıçap", "Radius"), p.radius, t(l, "Yerçekimi", "Gravity"), p.gravity));
                ui.label(format!("{}: {:.0} {}", t(l, "Gün uzunluğu", "Day length"), p.day_len / 60.0, t(l, "dk", "min")));
                let save = &game.planets[sel];
                ui.label(RichText::new(if sel == game.planet { t(l, "Şu an buradasın", "You are here") } else if save.visited { t(l, "Ziyaret edildi", "Visited") } else { t(l, "Keşfedilmedi", "Unexplored") }).color(DIM));
                if !save.structs.is_empty() {
                    ui.label(RichText::new(format!("{}: {}", t(l, "Yapılar", "Structures"), save.structs.len())).color(DIM));
                }
                ui.add_space(4.0);
                ui.label(RichText::new(t(l, "Yörünge taraması: kaynaklar", "Orbital scan: resources")).strong());
                for (item, rich) in &p.resources {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
                        item_icon(ui.painter(), r.center(), 7.5, *item);
                        let amount = if *rich > 0.75 { t(l, "bol", "rich") } else if *rich > 0.45 { t(l, "orta", "moderate") } else { t(l, "az", "sparse") };
                        ui.label(format!("{} ({})", item.name(l), amount));
                    });
                }
                if !p.flora.is_empty() {
                    ui.label(RichText::new(format!("{}: {} {}", t(l, "Bitki örtüsü", "Flora"), p.flora.len(), t(l, "tür", "species"))).color(DIM));
                }
                ui.add_space(8.0);
                if let Some(shuttle_id) = ui_s.travel_from {
                    let fuel = game.structs().iter().find(|s| s.id == shuttle_id).map(|s| s.fuel).unwrap_or(0.0);
                    if sel != game.planet {
                        let cost = sys.fuel_cost(game.planet, sel);
                        ui.label(format!("{}: {} / {:.0}", t(l, "Gereken yakıt", "Fuel needed"), cost, fuel));
                        if ui.add_enabled(fuel + 0.01 >= cost as f32, egui::Button::new(RichText::new(format!("🚀 {} {}", t(l, "Uç:", "Fly to"), p.name)).size(17.0)).min_size(egui::vec2(200.0, 42.0))).clicked() {
                            ui_s.actions.push(Action::Travel(sel));
                        }
                    }
                } else if sel != game.planet {
                    ui.label(RichText::new(t(l, "Yolculuk için bir Mekiğe bin.", "Board a Shuttle to travel.")).color(DIM));
                }
            });
        });
    });
}

fn journal_tab(ui: &mut egui::Ui, ui_s: &mut Ui, game: &mut Game, sys: &StarSystem, l: Lang, h: f32) {
    tab_title(ui, ui_s, t(l, "MİRA Günlüğü", "MIRA's Journal"));
    game.journal_unread = false;
    let sel = ui_s.journal_sel.filter(|s| game.journal.contains(s)).or(game.journal.last().copied());
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(230.0);
            egui::ScrollArea::vertical().id_salt("jl").max_height(h - 80.0).show(ui, |ui| {
                for id in game.journal.iter().rev() {
                    let e = &story::JOURNAL[*id as usize];
                    if ui.add(egui::Button::new(RichText::new(tx(l, e.title)).size(14.5)).selected(sel == Some(*id)).min_size(egui::vec2(220.0, 30.0))).clicked() {
                        ui_s.journal_sel = Some(*id);
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{}: {}/{}", t(l, "Kara kutu", "Black box"), game.fragments, story::FRAG_COUNT)).size(13.0).color(DIM));
            });
        });
        ui.separator();
        ui.vertical(|ui| {
            egui::ScrollArea::vertical().id_salt("jb").max_height(h - 80.0).show(ui, |ui| {
                if let Some(id) = sel {
                    let e = &story::JOURNAL[id as usize];
                    ui.label(RichText::new(tx(l, e.title)).size(20.0).strong());
                    ui.add_space(4.0);
                    ui.label(RichText::new(story::fill(tx(l, e.body), &sys.name, &sys.planets[sys.home].name)).size(15.5));
                }
            });
        });
    });
}

fn recipe_row(ui: &mut egui::Ui, l: Lang, game: &Game, idx: usize) -> bool {
    let r = &RECIPES[idx];
    let mut clicked = false;
    egui::Frame::new().fill(Color32::from_rgb(24, 30, 44)).corner_radius(6).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), Sense::hover());
            item_icon(ui.painter(), rect.center(), 14.0, if r.data > 0 { r.inputs[0].0 } else { r.output.0 });
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() - 110.0);
                let title = if r.data > 0 { format!("{} → +{} {}", r.inputs[0].0.name(l), r.data, t(l, "Veri", "Data")) } else { format!("{} ×{}", r.output.0.name(l), r.output.1) };
                ui.label(RichText::new(title).size(15.5).strong());
                cost_row(ui, l, game, r.inputs);
            });
            if ui.add_enabled(game.inv.has_all(r.inputs), egui::Button::new(RichText::new(t(l, "Üret", "Make")).size(15.0)).min_size(egui::vec2(90.0, 34.0))).clicked() {
                clicked = true;
            }
        });
    });
    clicked
}

#[allow(clippy::too_many_arguments)]
fn structure_panel(ui: &mut egui::Ui, ui_s: &mut Ui, game: &mut Game, sys: &StarSystem, grid: &crate::network::Grid, id: u32, l: Lang, max_h: f32) -> bool {
    let mut keep = true;
    let Some(s) = game.structs().iter().find(|s| s.id == id).cloned() else { return false };
    let def = s.kind.def();
    ui.horizontal(|ui| {
        ui.label(RichText::new(tx(l, def.name)).size(22.0).strong().color(ACCENT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("✖").clicked() {
                keep = false;
            }
        });
    });
    ui.label(RichText::new(tx(l, def.desc)).size(13.5).color(DIM));
    ui.separator();
    crate::network::hud::power_section(ui, game, grid, id, l);
    let mut dismantle_ok = !matches!(s.kind, SK::Pod);

    if let Some(station) = s.kind.station() {
        if let Some(job) = &s.job {
            let r = &RECIPES[job.recipe as usize];
            let name = if r.data > 0 { r.inputs[0].0.name(l) } else { r.output.0.name(l) };
            ui.add(egui::ProgressBar::new(job.progress).text(format!("{} ×{}", name, job.count)).fill(Color32::from_rgb(200, 110, 40)));
            ui.ctx().request_repaint();
        } else {
            ui.label(RichText::new(t(l, "Boşta", "Idle")).color(DIM));
        }
        if !s.out.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for (item, n) in &s.out {
                    slot(ui, l, Some((*item, *n)), false);
                }
                if ui.add(egui::Button::new(RichText::new(t(l, "Hepsini al", "Take all")).size(16.0)).min_size(egui::vec2(110.0, 40.0))).clicked() {
                    let outs = s.out.clone();
                    let mut rest = Vec::new();
                    for (item, n) in outs {
                        let added = give(game, ui_s, item, n);
                        if added < n {
                            rest.push((item, n - added));
                        }
                    }
                    if let Some(st) = game.find_struct(id) {
                        st.out = rest;
                    }
                }
            });
        }
        ui.separator();
        egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
            for (i, r) in RECIPES.iter().enumerate() {
                if r.station != station {
                    continue;
                }
                if !game.tech_ok(r.tech) {
                    if let Some(tech) = r.tech {
                        ui.label(RichText::new(format!("🔒 {} ({})", r.output.0.name(l), tx(l, tech.def().name))).size(13.5).color(DIM));
                    }
                    continue;
                }
                let busy_other = s.job.as_ref().map(|j| j.recipe as usize != i).unwrap_or(false);
                ui.add_enabled_ui(!busy_other, |ui| {
                    if recipe_row(ui, l, game, i) && game.inv.remove_all(r.inputs) {
                        if let Some(st) = game.find_struct(id) {
                            match &mut st.job {
                                Some(j) => j.count += 1,
                                None => st.job = Some(Job { recipe: i as u16, count: 1, progress: 0.0 }),
                            }
                        }
                    }
                });
            }
            if station == Station::Research {
                ui.label(RichText::new(t(l, "Numuneler gezegen yüzeyine dağılmış mavi ışıklı oluşumlardır.", "Specimens are the glowing blue formations scattered across the surface.")).size(13.0).color(DIM));
            }
        });
        dismantle_ok = s.job.is_none();
    }

    match s.kind {
        SK::Pod => {
            ui.label(format!("{}: {:.0} m", t(l, "Oksijen alanı", "Oxygen zone"), def.oxygen));
            ui.label(format!("⚡ +{:.0}", def.power));
            if ui.add(egui::Button::new(RichText::new(t(l, "Oyunu kaydet", "Save game")).size(16.0)).min_size(egui::vec2(160.0, 40.0))).clicked() {
                ui_s.actions.push(Action::Save);
            }
        }
        SK::BioGen => {
            ui.add(egui::ProgressBar::new((s.fuel / (BIO_BURN * 10.0)).min(1.0)).text(format!("{}: {:.0} s", t(l, "Yanma süresi", "Burn time"), s.fuel)));
            ui.horizontal(|ui| {
                for n in [1u32, 5] {
                    if ui.add_enabled(game.inv.count(Item::Organic) >= n, egui::Button::new(format!("+{} {}", n, Item::Organic.name(l))).min_size(egui::vec2(120.0, 38.0))).clicked() && game.inv.remove(Item::Organic, n) {
                        if let Some(st) = game.find_struct(id) {
                            st.fuel += BIO_BURN * n as f32;
                        }
                    }
                }
            });
            ui.label(RichText::new(t(l, "Yalnızca güce ihtiyaç varken yakıt harcar.", "Only burns fuel while power is needed.")).size(13.0).color(DIM));
        }
        SK::Solar => {
            ui.label(format!("⚡ +{:.0} ({})", def.power, t(l, "gündüz", "daylight")));
        }
        SK::Rtg => {
            ui.label(format!("⚡ +{:.0}", def.power));
        }
        SK::Battery => {
            ui.add(egui::ProgressBar::new(s.charge / BATTERY_CAP).text(format!("{:.0} / {:.0}", s.charge, BATTERY_CAP)));
        }
        SK::Storage => {
            ui.label(RichText::new(t(l, "Depo (al)", "Crate (take)")).strong());
            ui.horizontal_wrapped(|ui| {
                for i in 0..s.store.slots.len() {
                    if slot(ui, l, s.store.slots[i], false).clicked() {
                        if let Some((item, n)) = s.store.slots[i] {
                            let left = game.inv.add(item, n);
                            if let Some(st) = game.find_struct(id) {
                                st.store.slots[i] = if left > 0 { Some((item, left)) } else { None };
                            }
                        }
                    }
                }
            });
            ui.label(RichText::new(t(l, "Sırt çantası (koy)", "Backpack (store)")).strong());
            ui.horizontal_wrapped(|ui| {
                for i in 0..game.inv.slots.len() {
                    if slot(ui, l, game.inv.slots[i], false).clicked() {
                        if let Some((item, n)) = game.inv.slots[i] {
                            let mut left = n;
                            if let Some(st) = game.find_struct(id) {
                                left = st.store.add(item, n);
                            }
                            game.inv.slots[i] = if left > 0 { Some((item, left)) } else { None };
                        }
                    }
                }
            });
            dismantle_ok = s.store.slots.iter().all(|x| x.is_none());
        }
        SK::Shuttle => {
            ui.add(egui::ProgressBar::new((s.fuel / 15.0).min(1.0)).text(format!("{}: {:.0}", Item::Fuel.name(l), s.fuel)).fill(c32(Item::Fuel.def().color)));
            ui.horizontal(|ui| {
                for n in [1u32, 5] {
                    if ui.add_enabled(game.inv.count(Item::Fuel) >= n, egui::Button::new(format!("+{} {}", n, Item::Fuel.name(l))).min_size(egui::vec2(110.0, 38.0))).clicked() && game.inv.remove(Item::Fuel, n) {
                        if let Some(st) = game.find_struct(id) {
                            st.fuel += n as f32;
                        }
                    }
                }
            });
            let cheapest = (0..sys.planets.len()).filter(|i| *i != game.planet).map(|i| sys.fuel_cost(game.planet, i)).min().unwrap_or(3);
            ui.label(RichText::new(format!("{}: {}", t(l, "En yakın gezegen için yakıt", "Fuel for the nearest planet"), cheapest)).size(13.5).color(DIM));
            if ui.add(egui::Button::new(RichText::new(format!("🚀 {}", t(l, "Bin ve rota seç", "Board and pick a course"))).size(17.0)).min_size(egui::vec2(240.0, 44.0))).clicked() {
                ui_s.travel_from = Some(id);
                ui_s.tab = Tab::Map;
                ui_s.open = true;
                keep = false;
            }
            dismantle_ok = s.fuel < 0.5;
        }
        SK::Shipyard => {
            ui.label(RichText::new(t(l, "Öncü II", "Pioneer II")).size(18.0).strong());
            for (item, need) in SHIP_PARTS {
                let have = s.delivered_count(item);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
                    item_icon(ui.painter(), r.center(), 10.0, item);
                    let done = have >= need;
                    ui.label(RichText::new(format!("{}  {}/{}", item.name(l), have, need)).size(16.0).color(if done { Color32::from_rgb(150, 230, 150) } else { Color32::WHITE }));
                    let inv = game.inv.count(item);
                    if !done && ui.add_enabled(inv > 0, egui::Button::new(format!("{} ({})", t(l, "Teslim et", "Deliver"), inv.min(need - have)))).clicked() {
                        let n = inv.min(need - have);
                        game.inv.remove(item, n);
                        if let Some(st) = game.find_struct(id) {
                            match st.delivered.iter_mut().find(|d| d.0 == item) {
                                Some(d) => d.1 += n,
                                None => st.delivered.push((item, n)),
                            }
                        }
                    }
                });
            }
            ui.add_space(6.0);
            if s.ship_complete() {
                if ui.add(egui::Button::new(RichText::new(format!("🚀 {}", t(l, "FIRLAT", "LAUNCH"))).size(24.0)).min_size(egui::vec2(260.0, 56.0)).fill(Color32::from_rgb(190, 90, 20))).clicked() {
                    ui_s.actions.push(Action::Finale(id));
                    keep = false;
                }
            } else {
                ui.label(RichText::new(t(l, "Tüm parçalar teslim edilince fırlatma açılır.", "Launch unlocks when every part is delivered.")).color(DIM));
            }
            dismantle_ok = s.delivered.is_empty();
        }
        _ => {}
    }

    if dismantle_ok {
        ui.add_space(6.0);
        if ui.add(egui::Button::new(RichText::new(t(l, "Sök (malzemeler iade)", "Dismantle (refund)")).size(13.5).color(Color32::from_rgb(255, 150, 130)))).clicked() {
            ui_s.actions.push(Action::Dismantle(id));
            keep = false;
        }
    }
    keep
}
