//! Small utility scripts: `ImageUtils`, `NodeUtils`, `ObjUtils`, `Platform`
//! (without Steam), `PauseManager`, and `Info.from_stat`.

use crate::bb_scripts::BbColor;
use bevy::prelude::*;
use megatower_kernel::format::{self, Rules};

/// `ImageUtils.Bounds`. As in the source, `right_pos` is the minimum x and
/// `left_pos` the maximum x.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub top: i32,
    pub bottom: i32,
    pub right: i32,
    pub left: i32,
}

/// `get_png_bounds` over a row-major alpha channel (`width * height`).
/// Scanning is column-major in the source, which does not affect min/max.
pub fn png_bounds(alpha: &[u8], width: usize, height: usize) -> Bounds {
    let mut b = Bounds {
        top: height as i32,
        bottom: 0,
        right: width as i32,
        left: 0,
    };
    for x in 0..width {
        for y in 0..height {
            if alpha[y * width + x] != 0 {
                b.top = b.top.min(y as i32);
                b.bottom = b.bottom.max(y as i32);
                b.right = b.right.min(x as i32);
                b.left = b.left.max(x as i32);
            }
        }
    }
    b
}

/// Godot `roundi`: halves away from zero.
fn roundi(v: f32) -> i32 {
    v.round() as i32
}

/// `get_rect2i_from_bounds` as (x, y, width, height). Width is
/// `left - right`, matching the source's naming.
pub fn rect_from_bounds(b: Option<Bounds>) -> (i32, i32, i32, i32) {
    let Some(b) = b else { return (0, 0, 0, 0) };
    let height = b.bottom - b.top;
    let width = b.left - b.right;
    let cy = b.top + roundi(height as f32 / 2.0);
    let cx = b.right + roundi(width as f32 / 2.0);
    (cx, cy, width, height)
}

/// `NodeUtils.get_all_children`: pre-order including the node itself; an
/// invalid entity yields nothing.
pub fn all_nodes(world: &World, root: Entity, out: &mut Vec<Entity>) {
    let Ok(e) = world.get_entity(root) else {
        return;
    };
    out.push(root);
    if let Some(children) = e.get::<Children>() {
        for child in children.iter() {
            all_nodes(world, child, out);
        }
    }
}

/// `Platform` without Steam (`steam == null`): every query returns its
/// inactive default.
pub mod platform {
    pub fn is_active() -> bool {
        false
    }
    pub fn steam_id() -> i64 {
        -1
    }
    pub fn app_id() -> i64 {
        -1
    }
    pub fn own_name() -> &'static str {
        ""
    }
    pub fn language() -> &'static str {
        ""
    }
    pub fn has_achievement(_: &str) -> bool {
        false
    }
}

/// `PauseManager._process` outside debug input handling: the tree is paused
/// only in the gameplay state with fewer than two lobby players and a hidden
/// gameplay canvas.
pub fn should_pause(in_gameplay_state: bool, lobby_players: usize, canvas_visible: bool) -> bool {
    in_gameplay_state && lobby_players < 2 && !canvas_visible
}

/// One display piece of `Info.from_stat`.
#[derive(Clone, Debug, PartialEq)]
pub enum InfoPiece {
    Text {
        text: String,
        color: BbColor,
    },
    /// The stat container with its lowercased translated name.
    Stat {
        stat: String,
        name_lower: String,
    },
    /// An item set marker; `remove_space` is true after the first.
    SetMultiplier {
        set: String,
        remove_space: bool,
    },
}

pub struct FromStatArgs<'a> {
    /// Stat shown (after `transformed_stats` substitution by the caller).
    pub stat: &'a str,
    pub translated_name: &'a str,
    pub is_percentage: bool,
    pub amount: f64,
    pub is_modifier: bool,
    pub item_sets: &'a [String],
    pub new_stat_value: f64,
    pub is_bonus: bool,
    pub is_reforged: bool,
}

/// `Info.from_stat`.
pub fn from_stat(a: &FromStatArgs) -> Vec<InfoPiece> {
    let mut rules = Rules {
        prefix: true,
        ..Rules::default()
    };
    if a.is_percentage || a.is_modifier {
        rules.percentage = true;
    }
    if a.amount > 100_000_000.0 {
        rules.suffix = true;
    }
    let bonus_amount = a.new_stat_value - a.amount.max(0.0);
    let bonus_str = format::number(bonus_amount, rules);
    let amount_str = format::number(a.amount, rules);
    let amount_text = if a.is_reforged {
        amount_str
    } else {
        format!("{amount_str} ")
    };
    let amount_color = if a.is_bonus {
        BbColor::LightGreen
    } else {
        BbColor::DarkGray
    };

    let mut out = vec![
        InfoPiece::Text {
            text: "\n".into(),
            color: BbColor::DarkGray,
        },
        InfoPiece::Text {
            text: amount_text,
            color: amount_color,
        },
    ];
    if a.is_reforged {
        out.push(InfoPiece::Text {
            text: "* ".into(),
            color: BbColor::DimGray,
        });
    }
    out.push(InfoPiece::Stat {
        stat: a.stat.into(),
        name_lower: a.translated_name.to_lowercase(),
    });
    if bonus_amount > 0.0 {
        out.push(InfoPiece::Text {
            text: format!(" {bonus_str}"),
            color: BbColor::LightGreen,
        });
    }
    for (idx, set) in a.item_sets.iter().enumerate() {
        out.push(InfoPiece::SetMultiplier {
            set: set.clone(),
            remove_space: idx > 0,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_keep_source_axis_naming() {
        let mut alpha = vec![0u8; 5 * 4];
        alpha[1 * 5 + 1] = 9;
        alpha[3 * 5 + 3] = 1;
        let b = png_bounds(&alpha, 5, 4);
        assert_eq!(
            b,
            Bounds {
                top: 1,
                bottom: 3,
                right: 1,
                left: 3
            }
        );
        assert_eq!(rect_from_bounds(Some(b)), (2, 2, 2, 2));
        // all transparent: initial sentinels remain, producing a negative rect
        let e = png_bounds(&[0; 6], 3, 2);
        assert_eq!(
            e,
            Bounds {
                top: 2,
                bottom: 0,
                right: 3,
                left: 0
            }
        );
        assert_eq!(rect_from_bounds(None), (0, 0, 0, 0));
    }

    #[test]
    fn descendants_preorder() {
        let mut w = World::new();
        let c1 = w.spawn_empty().id();
        let c2 = w.spawn_empty().id();
        let root = w.spawn_empty().add_children(&[c1, c2]).id();
        let mut v = vec![];
        all_nodes(&w, root, &mut v);
        assert_eq!(v, [root, c1, c2]);
        let gone = w.spawn_empty().id();
        w.despawn(gone);
        let mut v = vec![];
        all_nodes(&w, gone, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn info_from_stat_layout() {
        let sets = ["A".to_string(), "B".to_string()];
        let out = from_stat(&FromStatArgs {
            stat: "ARMOR",
            translated_name: "Armor",
            is_percentage: false,
            amount: 5.0,
            is_modifier: false,
            item_sets: &sets,
            new_stat_value: 8.0,
            is_bonus: true,
            is_reforged: true,
        });
        assert_eq!(out.len(), 7);
        assert!(
            matches!(&out[1], InfoPiece::Text { text, color: BbColor::LightGreen } if text == "+5")
        );
        assert!(matches!(&out[4], InfoPiece::Text { text, .. } if text.starts_with(" +3")));
        assert_eq!(
            out[6],
            InfoPiece::SetMultiplier {
                set: "B".into(),
                remove_space: true
            }
        );
        assert!(should_pause(true, 1, false) && !should_pause(true, 2, false));
    }
}
