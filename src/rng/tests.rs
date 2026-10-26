use super::*;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Case {
    seed: String,
    operation: String,
    values: Vec<Value>,
    states: Vec<String>,
}

#[test]
fn all_operations_and_consumed_states_match_godot_4_4_1() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../artifacts/rng-golden.json")).unwrap();
    assert_eq!(cases.len(), 60);
    for case in cases {
        let seed = case.seed.parse::<i64>().unwrap() as u64;
        let mut rng = GodotRng::seeded(seed);
        let mut global = GlobalRng::seeded(seed);
        if !case.states.is_empty() {
            assert_eq!(rng.state(), case.states[0].parse::<i64>().unwrap() as u64);
        }
        for (index, expected) in case.values.iter().enumerate() {
            let actual: Value = match case.operation.as_str() {
                "randi" => rng.randi().into(),
                "randf" => (rng.randf() as f32).to_bits().into(),
                "range" => {
                    let pair = [
                        (0, 0),
                        (0, 1),
                        (7, -9),
                        (-10000, 20000),
                        (0, 2147483646),
                        (0, 1000000000),
                    ][index % 6];
                    rng.randi_range(pair.0, pair.1).unwrap().into()
                }
                "float_range" => rng.randf_range(-3.25, 8.5).to_bits().into(),
                "weighted" => {
                    let weights: &[f32] = match index % 5 {
                        0 => &[1.0, 1.0, 1.0],
                        1 => &[0.0, 2.5, 0.125, 10.0],
                        2 => &[0.0, 0.0],
                        3 => &[1.0],
                        _ => &[1e-30, 1e-20, 1e20],
                    };
                    rng.rand_weighted(weights).map_or(-1, |i| i as i32).into()
                }
                "pick" => rng.pick_random(7).unwrap().into(),
                "shuffle" => {
                    let mut array: Vec<i32> = (0..(index % 20) as i32).collect();
                    rng.shuffle_array(&mut array);
                    serde_json::to_value(array).unwrap()
                }
                "global" => global.randf().to_bits().to_string().into(),
                "string" => global
                    .generate_random_string([0, 1, 16, 62, 100, -1, -62][index % 7])
                    .into(),
                "skew" => global.random_skewed_number(1000).into(),
                other => panic!("unknown oracle operation {other}"),
            };
            assert_eq!(
                &actual, expected,
                "{} seed={} index={index}",
                case.operation, case.seed
            );
            if !case.states.is_empty() {
                assert_eq!(
                    rng.state(),
                    case.states[index + 1].parse::<i64>().unwrap() as u64,
                    "state: {} seed={} index={index}",
                    case.operation,
                    case.seed
                );
            }
        }
    }
}

#[test]
fn streams_restore_bit_patterns_without_reseeding_or_touching_other_streams() {
    let mut streams = RngStreams::seeded(42);
    let initial = streams.get_states();
    assert!(initial.iter().all(|state| *state == initial[0]));
    streams.gameplay.randi();
    assert_eq!(&streams.get_states()[1..], &initial[1..]);
    let checkpoint = streams.get_states();
    let next = streams.gameplay.randi();
    streams.restore_states(checkpoint);
    assert_eq!(streams.gameplay.randi(), next);
    assert_eq!(streams.gameplay.seed(), 42);
    streams.restore_states([-1, i64::MIN, 0, 1]);
    assert_eq!(streams.get_states(), [-1, i64::MIN, 0, 1]);
    streams.set_base_seed(42);
    assert_eq!(streams.get_states(), initial);
    let state = streams.gameplay.state();
    assert_eq!(streams.gameplay.pick_random(0), None);
    assert_eq!(streams.gameplay.randi_range(7, 7), Some(7));
    assert_eq!(streams.gameplay.randi_range(i32::MIN, i32::MAX), None);
    assert_eq!(streams.gameplay.state(), state);
    streams.gameplay.set_state(0);
    assert_eq!(streams.gameplay.randf(), 0.0);
    assert_eq!(streams.gameplay.state(), INCREMENT);
}
