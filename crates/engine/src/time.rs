//! The single time rule, shared by every action.

/// What happens to the token after paying a cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spend {
    /// The active player keeps the token with this budget left.
    Stay(u32),
    /// The opponent receives the token with this budget.
    HandOver(u32),
}

/// Pays `cost` out of `budget`: if `cost < budget` the player stays with `budget - cost`;
/// otherwise (a cost exactly equal to the budget crosses the boundary) the opponent receives
/// `min(cost - budget + 1, opp_max)`.
pub(crate) fn spend(budget: u32, cost: u32, my_max: u32, opp_max: u32) -> Spend {
    debug_assert!(
        (1..=my_max).contains(&budget),
        "budget outside 1..=max_time"
    );
    if cost < budget {
        return Spend::Stay(budget - cost);
    }
    // u64: `cost - budget + 1` cannot overflow even for `cost == u32::MAX`.
    let received = u64::from(cost) - u64::from(budget) + 1;
    let capped = received.min(u64::from(opp_max));
    Spend::HandOver(u32::try_from(capped).unwrap_or(opp_max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cost_below_the_budget_keeps_the_token() {
        assert_eq!(spend(3, 1, 8, 8), Spend::Stay(2));
        assert_eq!(spend(8, 7, 8, 8), Spend::Stay(1));
    }

    #[test]
    fn a_cost_equal_to_the_budget_crosses_and_the_opponent_gets_one() {
        assert_eq!(spend(3, 3, 8, 8), Spend::HandOver(1));
        assert_eq!(spend(1, 1, 8, 8), Spend::HandOver(1));
    }

    #[test]
    fn a_cost_above_the_budget_gives_the_excess_plus_one() {
        assert_eq!(spend(3, 5, 8, 8), Spend::HandOver(3));
    }

    #[test]
    fn the_receivers_max_time_caps_the_budget() {
        assert_eq!(spend(3, 9, 8, 5), Spend::HandOver(5));
        assert_eq!(spend(3, 5, 8, 2), Spend::HandOver(2));
    }

    #[test]
    fn a_cost_above_my_own_max_time_is_fine_and_truncated() {
        assert_eq!(spend(3, 20, 8, 10), Spend::HandOver(10));
        assert_eq!(spend(3, 9, 8, 10), Spend::HandOver(7));
    }

    #[test]
    fn extreme_values_do_not_overflow() {
        assert_eq!(spend(1, u32::MAX, 8, 8), Spend::HandOver(8));
        assert_eq!(spend(u32::MAX, u32::MAX, u32::MAX, 8), Spend::HandOver(1));
    }
}
