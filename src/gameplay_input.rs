//! Keyboard bindings and intent ordering translated from `project.godot`
//! `[input]` and `GameplayState.process_inputs()`.
//!
//! Commands are intents. Market, inventory, and battle systems must validate
//! and execute them; this module does not pretend those managers are ported.

use bevy::prelude::{ButtonInput, KeyCode, Message, MessageWriter, Res, Resource};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Message)]
pub enum GameplayCommand {
    RefreshMarket,
    SortInventory,
    QuickBuy(usize),
    PrimaryAction,
    BaseAbility,
    LearnedAbility1,
    LearnedAbility2,
    Stance,
    Sell(usize),
    DebugCompleteEncounter,
}

#[derive(Clone, Copy, Debug, Default, Resource)]
pub struct InputContext {
    pub text_input_focused: bool,
    pub player_died: bool,
    pub debug_build: bool,
    pub battle_exists: bool,
    pub hovered_slot: Option<usize>,
    pub hovered_slot_is_market: bool,
    pub dragged_slot: Option<usize>,
}

/// Default keyboard binding per rebindable action (`project.godot` `[input]`).
pub const DEFAULT_KEYS: [(&str, KeyCode); 14] = [
    ("quick_buy_item_1", KeyCode::Digit1),
    ("quick_buy_item_2", KeyCode::Digit2),
    ("quick_buy_item_3", KeyCode::Digit3),
    ("quick_buy_item_4", KeyCode::Digit4),
    ("sort_inventory", KeyCode::KeyD),
    ("refresh_market", KeyCode::KeyR),
    ("sell_item", KeyCode::KeyS),
    ("primary_action", KeyCode::Space),
    ("base_ability", KeyCode::KeyW),
    ("learned_ability_1", KeyCode::KeyQ),
    ("learned_ability_2", KeyCode::KeyE),
    ("stance", KeyCode::KeyV),
    ("select_left_enemy", KeyCode::ArrowLeft),
    ("select_right_enemy", KeyCode::ArrowRight),
];

/// Current key per action; `None` once another action stole the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBindings(pub std::collections::BTreeMap<String, Option<KeyCode>>);

impl Default for KeyBindings {
    fn default() -> Self {
        Self(
            DEFAULT_KEYS
                .iter()
                .map(|(a, k)| ((*a).to_owned(), Some(*k)))
                .collect(),
        )
    }
}

impl KeyBindings {
    pub fn get(&self, action: &str) -> Option<KeyCode> {
        self.0.get(action).copied().flatten()
    }

    /// `Options.set_key_bind` for keyboard events: the key is stolen from
    /// every other action before it is assigned.
    pub fn rebind(&mut self, action: &str, key: KeyCode) {
        for (name, bound) in self.0.iter_mut() {
            if name != action && *bound == Some(key) {
                *bound = None;
            }
        }
        self.0.insert(action.to_owned(), Some(key));
    }
}

/// Bridges Bevy keyboard state to the typed gameplay-intent message stream.
/// The gameplay managers are not ported yet, so this deliberately has no
/// consumer that could mutate run state with incomplete rules.
pub fn emit_gameplay_commands(
    input: Res<ButtonInput<KeyCode>>,
    context: Res<InputContext>,
    mut commands: MessageWriter<GameplayCommand>,
) {
    for command in commands_for_input(&input, *context) {
        commands.write(command);
    }
}

pub fn commands_for_input(
    input: &ButtonInput<KeyCode>,
    context: InputContext,
) -> Vec<GameplayCommand> {
    commands_for_bindings(input, context, &KeyBindings::default())
}

pub fn commands_for_bindings(
    input: &ButtonInput<KeyCode>,
    context: InputContext,
    bindings: &KeyBindings,
) -> Vec<GameplayCommand> {
    if context.text_input_focused || context.player_died {
        return Vec::new();
    }

    let just = |key| input.just_pressed(key);
    let held = |key| input.pressed(key);
    let just_action = |action: &str| bindings.get(action).is_some_and(|k| input.just_pressed(k));
    let held_action = |action: &str| bindings.get(action).is_some_and(|k| input.pressed(k));
    let mut commands = Vec::new();

    if just_action("refresh_market") {
        commands.push(GameplayCommand::RefreshMarket);
    }

    // The source uses independent `if`s; if several number keys are pressed
    // in one frame, the highest numbered one wins.
    let mut quick_buy = None;
    for (action, slot) in [
        ("quick_buy_item_1", 0),
        ("quick_buy_item_2", 1),
        ("quick_buy_item_3", 2),
        ("quick_buy_item_4", 3),
    ] {
        if just_action(action) {
            quick_buy = Some(slot);
        }
    }

    if just_action("sort_inventory") {
        commands.push(GameplayCommand::SortInventory);
    }
    if let Some(slot) = quick_buy {
        commands.push(GameplayCommand::QuickBuy(slot));
    }

    for (action, command) in [
        ("primary_action", GameplayCommand::PrimaryAction),
        ("base_ability", GameplayCommand::BaseAbility),
        ("learned_ability_1", GameplayCommand::LearnedAbility1),
        ("learned_ability_2", GameplayCommand::LearnedAbility2),
        ("stance", GameplayCommand::Stance),
    ] {
        if held_action(action) {
            commands.push(command);
        }
    }

    if held_action("sell_item")
        && context.hovered_slot.is_some()
        && !context.hovered_slot_is_market
        && let Some(slot) = context.dragged_slot.or(context.hovered_slot)
    {
        commands.push(GameplayCommand::Sell(slot));
    }

    if context.debug_build
        && context.battle_exists
        && just(KeyCode::KeyE)
        && (held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight))
    {
        commands.push(GameplayCommand::DebugCompleteEncounter);
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_distinguish_edge_actions_from_held_actions() {
        let mut input = ButtonInput::default();
        input.press(KeyCode::Space);
        let context = InputContext::default();
        assert_eq!(
            commands_for_input(&input, context),
            vec![GameplayCommand::PrimaryAction]
        );

        input.release(KeyCode::Space);
        input.press(KeyCode::KeyR);
        input.clear_just_pressed(KeyCode::KeyR);
        assert!(commands_for_input(&input, context).is_empty());
    }

    #[test]
    fn market_commands_keep_source_order_and_quick_buy_priority() {
        let mut input = ButtonInput::default();
        for key in [
            KeyCode::KeyR,
            KeyCode::Digit1,
            KeyCode::Digit4,
            KeyCode::KeyD,
        ] {
            input.press(key);
        }
        assert_eq!(
            commands_for_input(&input, InputContext::default()),
            vec![
                GameplayCommand::RefreshMarket,
                GameplayCommand::SortInventory,
                GameplayCommand::QuickBuy(3),
            ]
        );
    }

    #[test]
    fn sale_prefers_dragged_slot_and_rejects_market_hover() {
        let mut input = ButtonInput::default();
        input.press(KeyCode::KeyS);
        let mut context = InputContext {
            hovered_slot: Some(2),
            ..Default::default()
        };
        assert_eq!(
            commands_for_input(&input, context),
            vec![GameplayCommand::Sell(2)]
        );

        context.dragged_slot = Some(7);
        context.hovered_slot_is_market = false;
        assert_eq!(
            commands_for_input(&input, context),
            vec![GameplayCommand::Sell(7)]
        );
        context.hovered_slot_is_market = true;
        assert!(commands_for_input(&input, context).is_empty());
        context.dragged_slot = None;
        assert!(commands_for_input(&input, context).is_empty());
    }

    #[test]
    fn text_entry_and_death_suppress_all_gameplay_input() {
        let mut input = ButtonInput::default();
        input.press(KeyCode::Space);
        input.press(KeyCode::KeyR);
        assert!(
            commands_for_input(
                &input,
                InputContext {
                    text_input_focused: true,
                    ..Default::default()
                }
            )
            .is_empty()
        );
        assert!(
            commands_for_input(
                &input,
                InputContext {
                    player_died: true,
                    ..Default::default()
                }
            )
            .is_empty()
        );
    }

    #[test]
    fn debug_encounter_shortcut_requires_debug_build_and_existing_battle() {
        let mut input = ButtonInput::default();
        input.press(KeyCode::ShiftLeft);
        input.clear_just_pressed(KeyCode::ShiftLeft);
        input.press(KeyCode::KeyE);
        let ready = InputContext {
            debug_build: true,
            battle_exists: true,
            ..Default::default()
        };
        let commands = commands_for_input(&input, ready);
        assert!(commands.contains(&GameplayCommand::LearnedAbility2));
        assert!(commands.contains(&GameplayCommand::DebugCompleteEncounter));

        assert!(
            !commands_for_input(
                &input,
                InputContext {
                    debug_build: false,
                    ..ready
                }
            )
            .contains(&GameplayCommand::DebugCompleteEncounter)
        );
    }

    #[test]
    fn rebinding_moves_actions_and_steals_keys() {
        let mut bindings = KeyBindings::default();
        bindings.rebind("primary_action", KeyCode::KeyR);
        assert_eq!(bindings.get("refresh_market"), None);
        let mut input = ButtonInput::default();
        input.press(KeyCode::KeyR);
        assert_eq!(
            commands_for_bindings(&input, InputContext::default(), &bindings),
            vec![GameplayCommand::PrimaryAction]
        );
        input.release(KeyCode::KeyR);
        input.clear();
        input.press(KeyCode::Space);
        assert!(commands_for_bindings(&input, InputContext::default(), &bindings).is_empty());
    }
}
