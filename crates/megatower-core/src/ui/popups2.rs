//! Popup scripts: `market_popup_container` item state, `hub_action_panel`,
//! `item_found_popup`, `credits_popup`, `adventurer_profile_popup`,
//! `adventurer_selection_popup_container`, `no_tutorial_popup`.

use megatower_kernel::format::{self, Rules};

/// `credits_popup.gd` `credits_dict` in source order: (group, [(sub group, people)]).
pub const CREDITS: &[(&str, &[(&str, &[&str])])] = &[
    (
        "INSTINCT3",
        &[
            ("HandOfBlood", &["Maximilian Paul Karl-Heinz Knabe"]),
            ("Product Manager Publishing", &["Melvin Frank"]),
            ("Head of Business Development", &["Olga Voronina"]),
            (
                "The whole Team HandOfBlood, especially",
                &[
                    "Carl Brose",
                    "Denis Grasse",
                    "Nik Peters",
                    "Solveig Brockhaus",
                ],
            ),
        ],
    ),
    (
        "RAVENAGE GAMES",
        &[
            ("CMO", &["Anton Bond"]),
            ("Publishing Producer", &["Imad Khalil"]),
            ("Community Manager", &["Mehdi El-Assimi"]),
            ("Product Lead Designer", &["Sergey Solodukhin"]),
            ("Head of Business Development", &["Olga Voronina"]),
            ("Community Director", &["Mike Pyslar"]),
            ("Associate Publishing Producer", &["Anton Emelianov"]),
            (
                "Marketing Managers",
                &["Ekaterina Nalgieva", "Polina Malahova"],
            ),
            ("Systems Administrator", &["Kirill Obukhov"]),
            ("Business Development Manager", &["Ilya Revunov"]),
            (
                "Designers",
                &[
                    "Mikhail Ermakov",
                    "Elizaveta Karikh",
                    "Dima Kalmykov",
                    "July Baurova",
                ],
            ),
            ("Financial Manager", &["Nana Gogoladze"]),
            ("Producers", &["Mark Shumarov", "Viktor Reznikov"]),
            ("Analyst", &["Vershinin Vladislav"]),
        ],
    ),
    (
        "",
        &[
            ("Music and Sounds", &["Guilherme Alexander"]),
            ("Enemy and Item Art", &["Biohazard / Luca Tomas Caceres"]),
            ("French Correction", &["Ralekk N.K."]),
            ("", &[]),
            (
                "Playtesters",
                &[
                    "Wheelzy",
                    "Nihilty",
                    "Dix-Lan",
                    "MaverickAirsoft",
                    "Maross80",
                    "Niel \"Fiela\" Smit",
                    "Mircon",
                    "Jetnerd",
                    "Jefferson \"Jeffz\" Rodrigues",
                    "Zapallo",
                    "Pingas Khan",
                    "QuakeyJakey",
                    "ACturntUp",
                    "Zuki",
                    "Sword_kirby777",
                    "NSJ",
                    "ShadowHacker808",
                    "hofpants",
                    "Hoaq",
                    "Khanbad",
                    "Akiiro",
                    "Kaidak",
                    "YoJune",
                    "Track",
                    "Shenab Al-Rhajiim",
                    "As2piK",
                    "rchromatic",
                    "Asekelo",
                    "noribentou",
                    "BoxIsSleepy",
                    "SkifAlef",
                    "Arwen",
                    "Caborus",
                    "IdentityUnk",
                    "Loam",
                    "Aiedail",
                    "Truehearted (TSV)",
                    "omightymerlin",
                    "LtShinySides",
                    "Kailas",
                    "Igor Kabanov",
                    "Justin Carroll",
                    "otDan",
                    "Azoozy*",
                    "Hesanka",
                    "Nickypoo",
                    "Jesse G",
                    "Squirrelanna",
                    "Hesanka",
                    "ChaosGrimmon",
                ],
            ),
        ],
    ),
];

/// Entry shown for one market item in the library popup.
#[derive(Clone, Debug, PartialEq)]
pub struct MarketItemView {
    pub rarity_texture: &'static str,
    pub rarity_alpha: f32,
    pub item_alpha: f32,
    pub lock_visible: bool,
    pub lock_y: Option<f32>,
}

/// `MarketPopupContainer.update_item_texture` (after `apply_data`): all
/// entries use the toggle-off border; banished items fade the border, and
/// requirement-locked items fade border and icon and show the lock at y=23.
pub fn market_item_view(is_banish: bool, requirement_locked: bool) -> MarketItemView {
    let mut v = MarketItemView {
        rarity_texture: crate::ui::item_visual::TOGGLE_OFF_BORDER,
        rarity_alpha: 1.0,
        item_alpha: 1.0,
        lock_visible: false,
        lock_y: None,
    };
    if is_banish {
        v.rarity_alpha = 0.5;
    }
    if requirement_locked {
        v.lock_visible = true;
        v.lock_y = Some(23.0);
        v.rarity_alpha = 0.5;
        v.item_alpha = 0.5;
    }
    v
}

/// `create_item`: items made from a resource with a passive start reforged
/// with that passive at level 1.
pub fn market_item_reforge(has_passive: bool) -> (bool, i32) {
    (has_passive, i32::from(has_passive))
}

/// `process_item_press`: clicking a hovered banished item un-banishes it.
/// Returns the indexes to un-banish (hovered item texture rects).
pub fn pressed_items(hovered: &[bool]) -> Vec<usize> {
    hovered
        .iter()
        .enumerate()
        .filter(|(_, h)| **h)
        .map(|(i, _)| i)
        .collect()
}

pub const HUB_NEUTRAL: [f32; 4] = [0.4, 0.360_784_3, 0.341_176_5, 1.0]; // #665c57
pub const HUB_HOVER_GOLD: [f32; 4] = [0.976_470_6, 0.639_215_7, 0.105_882_35, 1.0]; // f9a31b
pub const DARK_ORANGE: [f32; 4] = [1.0, 0.549_019_6, 0.0, 1.0];

#[derive(Clone, Debug, PartialEq)]
pub struct HubPanelColors {
    pub split_label: [f32; 4],
    pub split_icon: [f32; 4],
    pub price: [f32; 4],
}

/// `HubActionPanel._process` colors.
pub fn hub_colors(sell_hovered: bool, split_hovered: bool) -> HubPanelColors {
    let mut c = HubPanelColors {
        split_label: HUB_NEUTRAL,
        split_icon: HUB_NEUTRAL,
        price: HUB_NEUTRAL,
    };
    if sell_hovered {
        c.price = HUB_HOVER_GOLD;
    }
    if split_hovered {
        c.split_label = DARK_ORANGE;
        c.split_icon = DARK_ORANGE;
    }
    c
}

/// `HubActionPanel.set_price`: (text, visible).
pub fn hub_price(price: f64) -> (String, bool) {
    let suffix = Rules {
        suffix: true,
        ..Rules::default()
    };
    (format!("+{}", format::number(price, suffix)), price > 0.0)
}

pub const ITEM_FOUND_LIFE: f32 = 2.25;
pub const ITEM_FOUND_DROP: f32 = 25.0;

pub const SPEEDRUN_SUBMIT_URL: &str =
    "https://www.speedrun.com/Megaloot/runs/new?h=floor-60&x=wkpxme02";

/// `AdventurerSelectionPopup.update_cards`: card minimum size.
pub const ADVENTURER_CARD_SIZE: [f32; 2] = [54.0, 54.0];

/// `CreditsPopup`: the empty sub group name draws no heading.
pub fn credits_has_heading(sub_group: &str) -> bool {
    !sub_group.is_empty()
}

/// Person label alpha.
pub const CREDITS_PERSON_ALPHA: f32 = 0.75;

/// `LootStashPopupContainer._process`: the container y is snapped to a whole
/// pixel every frame.
pub fn stash_popup_y(y: f32) -> f32 {
    y.round()
}

/// `DebugCanvasLayer.push_result`: results accumulate one per line.
pub fn push_result(log: &mut String, result: &str) {
    log.push_str(result);
    log.push('\n');
}

/// `DebugCanvasLayer._input` outcome for a key event: toggles only in debug
/// builds on the console key; confirm captures the command text.
#[derive(Debug, PartialEq, Eq)]
pub enum ConsoleInput {
    Ignored,
    Toggle,
    Confirm,
}

pub fn console_input(
    debug_build: bool,
    toggle_pressed: bool,
    confirm_pressed: bool,
) -> ConsoleInput {
    if !debug_build {
        return ConsoleInput::Ignored;
    }
    if toggle_pressed {
        return ConsoleInput::Toggle;
    }
    if confirm_pressed {
        return ConsoleInput::Confirm;
    }
    ConsoleInput::Ignored
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_and_hub() {
        assert_eq!(CREDITS.len(), 3);
        assert_eq!(CREDITS[0].0, "INSTINCT3");
        let last = CREDITS[2].1;
        assert_eq!(
            last.iter()
                .find(|(s, _)| *s == "Playtesters")
                .unwrap()
                .1
                .len(),
            50
        );
        assert!(!credits_has_heading(""));
        assert_eq!(hub_price(1500.0), ("+1.5K".to_string(), true));
        assert!(!hub_price(0.0).1);
        assert_eq!(hub_colors(true, true).price, HUB_HOVER_GOLD);
        assert_eq!(hub_colors(false, true).split_label, DARK_ORANGE);
        assert_eq!(hub_colors(false, false).split_icon, HUB_NEUTRAL);
    }

    #[test]
    fn console_and_stash() {
        let mut log = String::new();
        push_result(&mut log, "ok");
        push_result(&mut log, "again");
        assert_eq!(log, "ok\nagain\n");
        assert_eq!(console_input(true, true, true), ConsoleInput::Toggle);
        assert_eq!(console_input(false, true, false), ConsoleInput::Ignored);
        assert_eq!(console_input(true, false, true), ConsoleInput::Confirm);
        assert_eq!(stash_popup_y(10.5), 11.0);
    }

    #[test]
    fn market_views() {
        let v = market_item_view(true, true);
        assert_eq!(
            (v.rarity_alpha, v.item_alpha, v.lock_y),
            (0.5, 0.5, Some(23.0))
        );
        assert_eq!(market_item_view(false, false).rarity_alpha, 1.0);
        assert_eq!(market_item_reforge(true), (true, 1));
        assert_eq!(pressed_items(&[false, true, true]), [1, 2]);
    }
}
