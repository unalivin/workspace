#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_scores_are_bounded_and_explainable() {
        assert_eq!(calculate_host_risk(0, 0, 0, 0, 1), 0);
        assert_eq!(calculate_host_risk(1, 0, 0, 0, 1), 18);
        assert_eq!(calculate_host_risk(20, 20, 20, 20, 8), 100);
        assert_eq!(risk_band(0), "low");
        assert_eq!(risk_band(50), "high");
        assert_eq!(risk_band(100), "critical");
    }
}
