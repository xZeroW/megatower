//! `Format.number` from `classes/utils/format.gd`.

#[derive(Clone, Copy, Default)]
pub struct Rules {
    pub prefix: bool,
    pub suffix: bool,
    pub percentage: bool,
    pub ignore_plus: bool,
    pub whole: bool,
}

const SUFFIXES: [(f64, &str); 23] = [
    (1e3, "K"),
    (1e6, "M"),
    (1e9, "B"),
    (1e12, "T"),
    (1e15, "Q"),
    (1e18, "S"),
    (1e21, "O"),
    (1e24, "N"),
    (1e27, "D"),
    (1e30, "U"),
    (1e33, "V"),
    (1e36, "G"),
    (1e39, "R"),
    (1e42, "E"),
    (1e45, "F"),
    (1e48, "H"),
    (1e51, "I"),
    (1e54, "J"),
    (1e57, "L"),
    (1e60, "P"),
    (1e63, "X"),
    (1.0000000000000001e66, "Y"),
    (1e69, "Z"),
];

/// GDScript `str(float)`: shortest round-trip, whole values keep ".0".
fn gd_str(value: f64) -> String {
    if value == 0.0 {
        return "0.0".to_owned();
    }
    if value.abs() >= 1e15 {
        return format!("{value:.0}");
    }
    format!("{value:?}")
}

fn trim_suffix<'a>(text: &'a str, suffix: &str) -> &'a str {
    text.strip_suffix(suffix).unwrap_or(text)
}

pub fn number(amount: f64, rules: Rules) -> String {
    let mut num = gd_str(amount);
    let mut prefix = "";
    let mut suffix = "";
    if amount < 1000.0 {
        if amount >= 100.0 {
            num = gd_str(amount.floor());
        } else {
            num = gd_str((amount * 10.0).floor() / 10.0);
            if rules.whole {
                num = gd_str(amount.round());
            }
        }
    }
    let mut num = trim_suffix(&num, ".0").to_string();
    if let Some(rest) = num.strip_prefix('-') {
        num = rest.to_string();
        prefix = "-";
    }
    if rules.prefix && !rules.ignore_plus && amount > 0.0 {
        prefix = "+";
    }
    if amount >= 1000.0 {
        if rules.suffix {
            for (limit, tag) in SUFFIXES.iter() {
                if amount >= *limit {
                    num = gd_str(amount / limit);
                    suffix = tag;
                }
            }
            if num.contains('.') {
                num = num.chars().take(4).collect();
                while num.ends_with('0') {
                    num.pop();
                }
                num = trim_suffix(&num, ".").to_string();
            }
            if !num.contains('.') {
                num = num.chars().take(3).collect();
            }
        } else {
            let mut i = num.len() as i64 - 3;
            while i > 0 {
                num.insert(i as usize, ',');
                i -= 3;
            }
            num = num.trim_start_matches(',').to_string();
        }
    }
    let percent = if rules.percentage { "%" } else { "" };
    format!("{prefix}{num}{suffix}{percent}")
}

/// `Format.time`.
pub fn time(time_delta: f64) -> String {
    let hours = (time_delta / 3600.0) as i64;
    let minutes = ((time_delta as i64 % 3600) as f64 / 60.0) as i64;
    let seconds = time_delta as i64 % 60;
    let milliseconds = ((time_delta - time_delta as i64 as f64) * 1000.0) as i64;
    if hours == 0 {
        format!("{minutes:02}:{seconds:02}.{milliseconds:03}")
    } else {
        format!("{hours:02}:{minutes:02}:{seconds:02}.{milliseconds:03}")
    }
}

/// `Format.to_seconds`.
pub fn to_seconds(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = seconds % 3600 / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m {secs:02}s")
    } else {
        format!("{minutes}m {secs:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Golden {
        numbers: Vec<(f64, u8, String)>,
        times: Vec<(f64, String)>,
        seconds: Vec<(i64, String)>,
    }
    const S: Rules = Rules {
        prefix: false,
        suffix: true,
        percentage: false,
        ignore_plus: false,
        whole: false,
    };

    #[test]
    fn small_and_large() {
        assert_eq!(number(7.0, S), "7");
        assert_eq!(number(12.34, S), "12.3");
        assert_eq!(number(999.9, S), "999");
        assert_eq!(number(8244.0, S), "8.24K");
        assert_eq!(number(1_500_000.0, S), "1.5M");
        assert_eq!(number(-5.0, S), "-5");
        assert_eq!(
            number(
                5.0,
                Rules {
                    prefix: true,
                    ..Default::default()
                }
            ),
            "+5"
        );
        assert_eq!(number(1234567.0, Rules::default()), "1,234,567");
    }

    #[test]
    fn matches_recovered_format_oracle() {
        let golden: Golden = serde_json::from_str(include_str!("../artifacts/format-golden.json"))
            .expect("format oracle JSON");
        for (amount, rules, expected) in golden.numbers {
            let has = |rule| rules & (1_u8 << rule) != 0_u8;
            assert_eq!(
                number(
                    amount,
                    Rules {
                        prefix: has(0),
                        suffix: has(1),
                        percentage: has(2),
                        ignore_plus: has(3),
                        whole: has(4),
                    },
                ),
                expected,
                "number({amount}, {rules})"
            );
        }
        for (input, expected) in golden.times {
            assert_eq!(time(input), expected, "time({input})");
        }
        for (input, expected) in golden.seconds {
            assert_eq!(to_seconds(input), expected, "to_seconds({input})");
        }
    }
}
