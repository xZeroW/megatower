//! Keyboard bindings and intent ordering translated from the original input map
//! `[input]` and `GameplayState.process_inputs()`.
//!
//! Keys map to [`GameAction`]s through a `leafwing-input-manager` input map
//! rebuilt from the player's saved [`KeyBindings`]; the frame's action state is
//! then turned into [`GameplayCommand`] intents in the source's order.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

/// Default keyboard binding per rebindable action (original input map).
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

/// The rebindable actions (`Options.BINDINGS`).
#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum GameAction {
    QuickBuy1,
    QuickBuy2,
    QuickBuy3,
    QuickBuy4,
    SortInventory,
    RefreshMarket,
    SellItem,
    PrimaryAction,
    BaseAbility,
    LearnedAbility1,
    LearnedAbility2,
    Stance,
    SelectLeftEnemy,
    SelectRightEnemy,
}

impl GameAction {
    /// Each action with its input action name.
    pub const ALL: [(Self, &'static str); 14] = [
        (Self::QuickBuy1, "quick_buy_item_1"),
        (Self::QuickBuy2, "quick_buy_item_2"),
        (Self::QuickBuy3, "quick_buy_item_3"),
        (Self::QuickBuy4, "quick_buy_item_4"),
        (Self::SortInventory, "sort_inventory"),
        (Self::RefreshMarket, "refresh_market"),
        (Self::SellItem, "sell_item"),
        (Self::PrimaryAction, "primary_action"),
        (Self::BaseAbility, "base_ability"),
        (Self::LearnedAbility1, "learned_ability_1"),
        (Self::LearnedAbility2, "learned_ability_2"),
        (Self::Stance, "stance"),
        (Self::SelectLeftEnemy, "select_left_enemy"),
        (Self::SelectRightEnemy, "select_right_enemy"),
    ];
}

/// The input map for the current bindings; unbound actions get no key.
pub fn input_map(bindings: &KeyBindings) -> InputMap<GameAction> {
    let mut map = InputMap::default();
    for (action, name) in GameAction::ALL {
        if let Some(key) = bindings.get(name) {
            map.insert(action, key);
        }
    }
    map
}

#[derive(AutoPlugin)]
pub struct GameInputPlugin;

impl Plugin for GameInputPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<GameAction>::default())
            .init_resource::<InputContext>()
            .init_resource::<ActiveBindings>();
    }
}

/// The bindings currently in force. The layer that edits them (the Options
/// screen) writes this resource; the input map follows it.
#[derive(Resource, Default)]
pub struct ActiveBindings(pub KeyBindings);

/// Ordering handle for systems that must run before the input map is synced.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputSystems;

/// Keeps the player's input map in step with the bindings in Options, so a
/// rebind takes effect immediately.
#[auto_add_system(plugin = GameInputPlugin, schedule = Update, config(in_set = InputSystems))]
fn sync_input_map(
    mut commands: Commands,
    active: Res<ActiveBindings>,
    player: Query<Entity, With<InputMap<GameAction>>>,
    mut applied: Local<Option<KeyBindings>>,
) {
    let bindings = &active.0;
    if applied.as_ref() == Some(bindings) {
        return;
    }
    *applied = Some(bindings.clone());
    match player.single() {
        Ok(entity) => {
            commands.entity(entity).insert(input_map(bindings));
        }
        Err(_) => {
            commands.spawn((input_map(bindings), Name::new("Player input")));
        }
    }
}

/// `GameplayState.process_inputs()`: the frame's actions as commands.
/// `keys` is only read for the debug-build Shift+E shortcut.
pub fn commands_for_actions(
    actions: &ActionState<GameAction>,
    keys: &ButtonInput<KeyCode>,
    context: InputContext,
) -> Vec<GameplayCommand> {
    if context.text_input_focused || context.player_died {
        return Vec::new();
    }

    let just = |key| keys.just_pressed(key);
    let held = |key| keys.pressed(key);
    let just_action = |action: GameAction| actions.just_pressed(&action);
    let held_action = |action: GameAction| actions.pressed(&action);
    let mut commands = Vec::new();

    if just_action(GameAction::RefreshMarket) {
        commands.push(GameplayCommand::RefreshMarket);
    }

    // The source uses independent `if`s; if several number keys are pressed
    // in one frame, the highest numbered one wins.
    let mut quick_buy = None;
    for (action, slot) in [
        (GameAction::QuickBuy1, 0),
        (GameAction::QuickBuy2, 1),
        (GameAction::QuickBuy3, 2),
        (GameAction::QuickBuy4, 3),
    ] {
        if just_action(action) {
            quick_buy = Some(slot);
        }
    }

    if just_action(GameAction::SortInventory) {
        commands.push(GameplayCommand::SortInventory);
    }
    if let Some(slot) = quick_buy {
        commands.push(GameplayCommand::QuickBuy(slot));
    }

    for (action, command) in [
        (GameAction::PrimaryAction, GameplayCommand::PrimaryAction),
        (GameAction::BaseAbility, GameplayCommand::BaseAbility),
        (
            GameAction::LearnedAbility1,
            GameplayCommand::LearnedAbility1,
        ),
        (
            GameAction::LearnedAbility2,
            GameplayCommand::LearnedAbility2,
        ),
        (GameAction::Stance, GameplayCommand::Stance),
    ] {
        if held_action(action) {
            commands.push(command);
        }
    }

    if held_action(GameAction::SellItem)
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

    /// What the input manager would report for these keys: an action bound
    /// to a just-pressed key is just pressed, one bound to a held key pressed.
    fn commands_for_bindings(
        keys: &ButtonInput<KeyCode>,
        context: InputContext,
        bindings: &KeyBindings,
    ) -> Vec<GameplayCommand> {
        let mut actions = ActionState::<GameAction>::default();
        let now = bevy::platform::time::Instant::now();
        for (action, name) in GameAction::ALL {
            if bindings.get(name).is_some_and(|k| keys.pressed(k)) {
                actions.press(&action);
            }
        }
        actions.tick(now, now);
        for (action, name) in GameAction::ALL {
            if bindings
                .get(name)
                .is_some_and(|k| keys.pressed(k) && keys.just_pressed(k))
            {
                actions.release(&action);
                actions.press(&action);
            }
        }
        commands_for_actions(&actions, keys, context)
    }

    fn commands_for_input(
        keys: &ButtonInput<KeyCode>,
        context: InputContext,
    ) -> Vec<GameplayCommand> {
        commands_for_bindings(keys, context, &KeyBindings::default())
    }

    #[test]
    fn input_map_follows_rebinding_and_unbinds_stolen_keys() {
        let mut bindings = KeyBindings::default();
        bindings.rebind("primary_action", KeyCode::KeyR);
        let map = input_map(&bindings);
        assert!(map.get(&GameAction::RefreshMarket).is_none());
        assert_eq!(
            map.get(&GameAction::PrimaryAction).map(|keys| keys.len()),
            Some(1)
        );
    }

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
        let got = commands_for_input(&input, context);
        assert!(got.is_empty(), "{got:?}");
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
