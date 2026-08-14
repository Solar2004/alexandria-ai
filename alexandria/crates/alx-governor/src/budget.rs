//! Presupuesto por tarea (plan 09 §3): T1=2k, T2=15k, T3=60k tokens/iteración.

use alx_core::types::{ModelTier, TokenBudget};

/// Presupuesto por tier, en tokens por iteración.
pub const T1_BUDGET: u32 = 2000;
pub const T2_BUDGET: u32 = 15000;
pub const T3_BUDGET: u32 = 60000;

/// Asigna y trackea presupuestos. Sin estado propio: opera sobre `TokenBudget`
/// de alx-core (que ya fija warn_at_pct=80, hard_cap_pct=100).
pub struct BudgetManager;

impl BudgetManager {
    /// Presupuesto inicial para el tier.
    pub fn allocate(tier: &ModelTier) -> TokenBudget {
        let total = match tier {
            ModelTier::T1Cheap => T1_BUDGET,
            ModelTier::T2Medium => T2_BUDGET,
            ModelTier::T3Premium => T3_BUDGET,
        };
        TokenBudget::new(total)
    }

    /// Registra un gasto de `tokens` en el presupuesto (satura en `total`).
    pub fn track(budget: &mut TokenBudget, tokens: u32) {
        budget.spend(tokens);
    }

    /// ¿Presupuesto agotado (≥ hard_cap)?
    pub fn is_over(budget: &TokenBudget) -> bool {
        budget.is_over()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_budgets_correct() {
        assert_eq!(BudgetManager::allocate(&ModelTier::T1Cheap).total, 2000);
        assert_eq!(BudgetManager::allocate(&ModelTier::T2Medium).total, 15000);
        assert_eq!(BudgetManager::allocate(&ModelTier::T3Premium).total, 60000);
    }

    #[test]
    fn allocate_starts_empty() {
        for tier in [ModelTier::T1Cheap, ModelTier::T2Medium, ModelTier::T3Premium] {
            let b = BudgetManager::allocate(&tier);
            assert_eq!(b.spent, 0);
            assert!(!b.is_over());
        }
    }

    #[test]
    fn track_spends_and_warns() {
        let mut b = BudgetManager::allocate(&ModelTier::T1Cheap);
        // 80% de 2000 = 1600 → warning pero no over.
        BudgetManager::track(&mut b, 1600);
        assert_eq!(b.spent, 1600);
        assert!(b.is_warning());
        assert!(!BudgetManager::is_over(&b));
    }

    #[test]
    fn track_saturates_at_cap() {
        let mut b = BudgetManager::allocate(&ModelTier::T2Medium);
        BudgetManager::track(&mut b, 999_999);
        assert_eq!(b.spent, 15000);
        assert!(BudgetManager::is_over(&b));
    }

    #[test]
    fn t3_over_only_at_cap() {
        let mut b = BudgetManager::allocate(&ModelTier::T3Premium);
        BudgetManager::track(&mut b, 59999);
        assert!(!BudgetManager::is_over(&b));
        BudgetManager::track(&mut b, 1);
        assert!(BudgetManager::is_over(&b));
    }
}
