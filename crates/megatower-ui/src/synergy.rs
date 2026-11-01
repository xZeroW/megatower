//! `synergy_container.gd` data assignment, independent of the original game controls.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetPresentation<T> {
    pub icon: T,
    pub color: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynergyPresentation<T> {
    pub main: SetPresentation<T>,
    pub synergy: SetPresentation<T>,
    pub result: SetPresentation<T>,
}

/// Mirrors `apply_item_sets`: the result reuses the original set icon while
/// taking the specialization color. The three returned values are also the
/// corresponding hover-data identities in source order.
pub fn apply_item_sets<T: Clone>(
    synergy_set: SetPresentation<T>,
    original_set: SetPresentation<T>,
    specialization_color: [u8; 4],
) -> SynergyPresentation<T> {
    SynergyPresentation {
        main: synergy_set,
        synergy: original_set.clone(),
        result: SetPresentation {
            icon: original_set.icon,
            color: specialization_color,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_reuses_original_icon_for_result_only() {
        let main = SetPresentation {
            icon: "main",
            color: [1, 2, 3, 4],
        };
        let original = SetPresentation {
            icon: "original",
            color: [5, 6, 7, 8],
        };
        assert_eq!(
            apply_item_sets(main, original, [9, 10, 11, 12]),
            SynergyPresentation {
                main: SetPresentation {
                    icon: "main",
                    color: [1, 2, 3, 4],
                },
                synergy: SetPresentation {
                    icon: "original",
                    color: [5, 6, 7, 8],
                },
                result: SetPresentation {
                    icon: "original",
                    color: [9, 10, 11, 12],
                },
            }
        );
    }
}
