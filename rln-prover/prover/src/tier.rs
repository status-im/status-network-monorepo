use std::collections::HashSet;
use std::ops::ControlFlow;
// third-party
use alloy::primitives::U256;
use derive_more::{Deref, DerefMut, From, Into};
// internal
use smart_contract::Tier;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, From, Into)]
pub struct TierLimit(u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash, From, Into)]
pub struct TierName(String);

impl From<&str> for TierName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Default, From, Into, Deref, DerefMut, PartialEq)]
pub struct TierLimits(Vec<Tier>);

impl<const N: usize> From<[Tier; N]> for TierLimits {
    fn from(value: [Tier; N]) -> Self {
        Self(Vec::from(value))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TierMatch {
    /// Karma is below the lowest tier
    UnderLowest,
    /// Karma is above the highest tier
    AboveHighest,
    /// Karma is in the range of a defined tier.
    Matched(Tier),
}

impl TierLimits {
    /// Validate tier limits (unique names, increasing min & max karma ...)
    pub(crate) fn validate(&self) -> Result<(), ValidateTierLimitsError> {
        #[derive(Debug, Default)]
        struct Context<'a> {
            tier_names: HashSet<String>,
            prev_min: Option<&'a U256>,
            prev_max: Option<&'a U256>,
            prev_tx_per_epoch: Option<&'a u32>,
        }

        let _context = self
            .0
            .iter()
            .try_fold(Context::default(), |mut state, tier| {
                if let Some(prev_min) = state.prev_min
                    && tier.min_karma <= *prev_min
                {
                    return Err(ValidateTierLimitsError::InvalidMinKarmaAmount);
                }

                if let Some(prev_max) = state.prev_max
                    && tier.min_karma <= *prev_max
                {
                    return Err(ValidateTierLimitsError::InvalidMinKarmaAmount);
                }

                if tier.min_karma >= tier.max_karma {
                    return Err(ValidateTierLimitsError::InvalidMaxKarmaAmount);
                }

                if tier.tx_per_epoch <= *state.prev_tx_per_epoch.unwrap_or(&0) {
                    return Err(ValidateTierLimitsError::InvalidTierLimit);
                }

                if state.tier_names.contains(&tier.name) {
                    return Err(ValidateTierLimitsError::NonUniqueTierName);
                }

                state.prev_min = Some(&tier.min_karma);
                state.prev_max = Some(&tier.max_karma);
                state.prev_tx_per_epoch = Some(&tier.tx_per_epoch);
                state.tier_names.insert(tier.name.clone());
                Ok(state)
            })?;

        Ok(())
    }

    /// Given some karma amount, find the matching Tier. Assume all tiers are active.
    pub(crate) fn get_tier_by_karma(&self, karma_amount: &U256) -> TierMatch {
        struct Context<'a> {
            current: Option<&'a Tier>,
        }

        let ctx_initial = Context { current: None };
        let ctx = self.0.iter().try_fold(ctx_initial, |mut state, tier| {
            if karma_amount < &tier.min_karma {
                // Early break - above lowest tier (< lowest_tier.min_karma)
                ControlFlow::Break(state)
            } else if karma_amount >= &tier.min_karma && karma_amount <= &tier.max_karma {
                // Found a match - update ctx and break
                state.current = Some(tier);
                ControlFlow::Break(state)
            } else {
                ControlFlow::Continue(state)
            }
        });

        if let Some(ctx) = ctx.break_value() {
            // ControlFlow::Break
            if let Some(tier) = ctx.current {
                TierMatch::Matched(tier.clone())
            } else {
                TierMatch::UnderLowest
            }
        } else {
            // ControlFlow::Continue
            TierMatch::AboveHighest
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidateTierLimitsError {
    #[error("Invalid Karma amount (must be increasing)")]
    InvalidMinKarmaAmount,
    #[error("Invalid Karma max amount")]
    InvalidMaxKarmaAmount,
    #[error("Invalid Tier limit (must be increasing)")]
    InvalidTierLimit,
    #[error("Non unique Tier name")]
    NonUniqueTierName,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;
    use claims::assert_matches;

    #[test]
    fn test_validate_failed_when_karma_overlapping_between_tier() {
        let tier_limits = TierLimits::from([
            Tier {
                name: "Basic".to_string(),
                min_karma: U256::from(10),
                max_karma: U256::from(100),
                tx_per_epoch: 6,
            },
            Tier {
                name: "Active".to_string(),
                min_karma: U256::from(50),
                max_karma: U256::from(150),
                tx_per_epoch: 120,
            },
        ]);

        assert_matches!(
            tier_limits.validate(),
            Err(ValidateTierLimitsError::InvalidMinKarmaAmount)
        );
    }

    #[test]
    fn test_validate_fails_when_min_karma_equal_or_greater_max_karma() {
        let tier_limits = TierLimits::from([Tier {
            name: "Basic".to_string(),
            min_karma: U256::from(100),
            max_karma: U256::from(100),
            tx_per_epoch: 6,
        }]);

        assert_matches!(
            tier_limits.validate(),
            Err(ValidateTierLimitsError::InvalidMaxKarmaAmount)
        );

        let tier_limits = TierLimits::from([Tier {
            name: "Basic".to_string(),
            min_karma: U256::from(500),
            max_karma: U256::from(100),
            tx_per_epoch: 6,
        }]);

        assert_matches!(
            tier_limits.validate(),
            Err(ValidateTierLimitsError::InvalidMaxKarmaAmount)
        );
    }

    #[test]
    fn test_validate_fails_with_non_increasing_or_decreasing_min_karma() {
        // Case 1: Duplicate min_karma values
        {
            let tier_limits = TierLimits::from([
                Tier {
                    name: "Basic".to_string(),
                    min_karma: U256::from(10),
                    max_karma: U256::from(49),
                    tx_per_epoch: 6,
                },
                Tier {
                    name: "Active".to_string(),
                    min_karma: U256::from(10),
                    max_karma: U256::from(99),
                    tx_per_epoch: 120,
                },
            ]);

            assert_matches!(
                tier_limits.validate(),
                Err(ValidateTierLimitsError::InvalidMinKarmaAmount)
            );
        }

        // Case 2: Decreasing min_karma values
        {
            let tier_limits = TierLimits::from([
                Tier {
                    name: "Basic".to_string(),
                    min_karma: U256::from(50),
                    max_karma: U256::from(99),
                    tx_per_epoch: 6,
                },
                Tier {
                    name: "Active".to_string(),
                    min_karma: U256::from(10),
                    max_karma: U256::from(49),
                    tx_per_epoch: 120,
                },
            ]);

            assert_matches!(
                tier_limits.validate(),
                Err(ValidateTierLimitsError::InvalidMinKarmaAmount)
            );
        }
    }

    #[test]
    fn test_validate_fails_with_non_increasing_or_decreasing_tx_per_epoch() {
        // Case 1: Duplicate tx_per_epoch values
        {
            let tier_limits = TierLimits::from([
                Tier {
                    name: "Basic".to_string(),
                    min_karma: U256::from(10),
                    max_karma: U256::from(49),
                    tx_per_epoch: 120,
                },
                Tier {
                    name: "Active".to_string(),
                    min_karma: U256::from(50),
                    max_karma: U256::from(99),
                    tx_per_epoch: 120,
                },
            ]);

            assert_matches!(
                tier_limits.validate(),
                Err(ValidateTierLimitsError::InvalidTierLimit)
            );
        }

        // Case 2: Decreasing tx_per_epoch values
        {
            let tier_limits = TierLimits::from([
                Tier {
                    name: "Basic".to_string(),
                    min_karma: U256::from(10),
                    max_karma: U256::from(49),
                    tx_per_epoch: 120,
                },
                Tier {
                    name: "Active".to_string(),
                    min_karma: U256::from(50),
                    max_karma: U256::from(99),
                    tx_per_epoch: 6,
                },
            ]);

            assert_matches!(
                tier_limits.validate(),
                Err(ValidateTierLimitsError::InvalidTierLimit)
            );
        }
    }

    #[test]
    fn test_validate_fails_with_duplicate_tier_names() {
        let tier_limits = TierLimits::from([
            Tier {
                name: "Basic".to_string(),
                min_karma: U256::from(10),
                max_karma: U256::from(49),
                tx_per_epoch: 6,
            },
            Tier {
                name: "Basic".to_string(),
                min_karma: U256::from(50),
                max_karma: U256::from(99),
                tx_per_epoch: 120,
            },
        ]);

        assert_matches!(
            tier_limits.validate(),
            Err(ValidateTierLimitsError::NonUniqueTierName)
        );
    }

    #[test]
    fn test_validate_and_get_tier_by_karma_with_empty_tier_limits() {
        let tier_limits = TierLimits::default();
        assert!(tier_limits.validate().is_ok());

        // XXX: make sense to test against a empty TierLimits?
        let result = tier_limits.get_tier_by_karma(&U256::ZERO);
        assert_eq!(result, TierMatch::AboveHighest);
    }

    #[test]
    fn test_get_tier_by_karma_bounds_and_ranges() {
        let tier_limits = TierLimits::from([
            Tier {
                name: "Basic".to_string(),
                min_karma: U256::from(10),
                max_karma: U256::from(49),
                tx_per_epoch: 6,
            },
            Tier {
                name: "Active".to_string(),
                min_karma: U256::from(50),
                max_karma: U256::from(99),
                tx_per_epoch: 120,
            },
            Tier {
                name: "Regular".to_string(),
                min_karma: U256::from(100),
                max_karma: U256::from(499),
                tx_per_epoch: 720,
            },
        ]);

        // Case 1: Zero karma
        let result = tier_limits.get_tier_by_karma(&U256::ZERO);
        assert_eq!(result, TierMatch::UnderLowest);

        // Case 2: Karma below all tiers
        let result = tier_limits.get_tier_by_karma(&U256::from(5));
        assert_eq!(result, TierMatch::UnderLowest);

        // Case 3: Exact match on min_karma (start of first tier)
        let result = tier_limits.get_tier_by_karma(&U256::from(10));
        if let TierMatch::Matched(tier) = result {
            assert_eq!(tier.name, "Basic");
        } else {
            panic!("Expected TierMatch::Matched, got {result:?}");
        }

        // Case 4: Exact match on a tier boundary (start of second tier)
        let result = tier_limits.get_tier_by_karma(&U256::from(50));
        if let TierMatch::Matched(tier) = result {
            assert_eq!(tier.name, "Active");
        } else {
            panic!("Expected TierMatch::Matched, got {result:?}");
        }

        // Case 5: Karma within a tier range (between third tier)
        let result = tier_limits.get_tier_by_karma(&U256::from(250));
        if let TierMatch::Matched(tier) = result {
            assert_eq!(tier.name, "Regular");
        } else {
            panic!("Expected TierMatch, got {result:?}");
        }

        // Case 6: Exact match on max_karma (end of the third tier)
        let result = tier_limits.get_tier_by_karma(&U256::from(499));
        if let TierMatch::Matched(tier) = result {
            assert_eq!(tier.name, "Regular");
        } else {
            panic!("Expected TierMatch, got {result:?}");
        }

        // Case 7: Karma above all tiers
        let result = tier_limits.get_tier_by_karma(&U256::from(1000));
        assert_eq!(result, TierMatch::AboveHighest);
    }
}
