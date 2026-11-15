use crate::app::{Tab, UiState};
use crate::layout as L;
use crate::shapes;
use crate::theme::{alpha, lerp_color};
use crate::ui::{Ui, DENSE};
use macroquad::prelude::*;
use sim::content::{FACTIONS, NODES, REGIONS, RIVALS};
use sim::{Game, NodeId, NodeKind, Owner};

/// The Dominion tracker strip over the top of the map.
pub fn draw(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let r = L::TRACKER;
    draw_rectangle(r.x, r.y, r.w, r.h, alpha(lerp_color(t.panel, t.black, 0.3), 0.82));
    draw_line(r.x, r.y + r.h, r.x + r.w, r.y + r.h, 1.0, t.border);
    let cy = r.y + r.h * 0.5;
    for (ri, reg) in REGIONS.iter().enumerate() {
        let x0 = 2.0 + L::TRACKER_GROUP_W * ri as f32;
        if ri > 0 {
            draw_line(x0, r.y + 3.0, x0, r.y + r.h - 3.0, 1.0, t.border);
        }
        let ruled = game.region_ruled(reg.id);
        ui.text(reg.code, x0 + 8.0, cy + 5.0, DENSE, if ruled { t.teal } else { t.dim });
        let mut resource_i = 0;
        for def in NODES.iter().filter(|n| n.region == reg.id) {
            let ns = &game.state().nodes[def.id.index()];
            let owned = ns.owner == Owner::Player;
            let x = match def.kind {
                NodeKind::Stronghold => x0 + 100.0,
                NodeKind::Lab | NodeKind::Core => x0 + 124.0,
                _ => {
                    resource_i += 1;
                    x0 + 26.0 + 18.0 * resource_i as f32
                }
            };
            let c = vec2(x, cy);
            let hostile_col = match (def.faction, def.rival) {
                (Some(f), _) => t.faction(f),
                (_, Some(rv)) => t.rival(rv),
                _ => t.grey,
            };
            let col = if owned {
                t.teal
            } else if ns.severed {
                t.danger
            } else if ns.owner == Owner::Hostile {
                hostile_col
            } else {
                t.grey
            };
            match def.kind {
                NodeKind::Core => shapes::diamond(c, 7.0, col),
                NodeKind::Stronghold => {
                    if owned {
                        draw_rectangle(c.x - 6.0, c.y - 1.0, 12.0, 6.0, alpha(t.gold, 0.9));
                    }
                    shapes::crown(c + vec2(0.0, 1.0), 6.0, if owned { t.gold } else { col });
                }
                NodeKind::Lab => {
                    if owned {
                        draw_circle(c.x, c.y, 6.0, col);
                    }
                    draw_circle_lines(c.x, c.y, 6.0, 1.5, col);
                    draw_circle_lines(c.x, c.y, 2.5, 1.0, if owned { t.panel } else { col });
                }
                _ => {
                    if owned {
                        draw_rectangle(c.x - 5.0, c.y - 5.0, 10.0, 10.0, col);
                    } else {
                        draw_rectangle_lines(c.x - 5.0, c.y - 5.0, 10.0, 10.0, 2.0, col);
                    }
                }
            }
            let hit = Rect::new(c.x - 9.0, r.y, 18.0, r.h);
            if ui.hovered(hit) {
                let state = if owned {
                    "Owned"
                } else if ns.severed {
                    "Severed"
                } else if ns.owner == Owner::Hostile {
                    "Hostile"
                } else {
                    "Neutral"
                };
                ui.tip(hit, &format!("{}\n{}, {}: {}", def.name, crate::text::node_kind_name(def.kind), reg.name, state));
                if ui.input.left_pressed {
                    select(st, def.id, owned, ui.time);
                }
            }
        }
    }
}

/// Opens the tab that matches a node, selects its owner and pings it on the map.
pub fn select(st: &mut UiState, node: NodeId, owned: bool, time: f64) {
    let def = &NODES[node.index()];
    st.selected = Some(node);
    st.ping = Some((node, time));
    st.tab = Tab::Node;
    if let Some(f) = def.faction {
        st.sel_faction = FACTIONS[f.index()].id;
        if !owned {
            st.tab = Tab::Humans;
        }
    }
    if let Some(rv) = def.rival {
        st.sel_rival = RIVALS[rv.index()].id;
        if !owned {
            st.tab = Tab::Rivals;
        }
    }
}
