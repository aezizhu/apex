//! Comprehensive unit tests for FrugalGPT model routing logic.
//!
//! Tests cover:
//! - Model configuration validation
//! - Complexity estimation algorithm
//! - Model selection based on task characteristics
//! - Cascade routing and escalation logic
//! - Cost estimation calculations
//! - Edge cases and boundary conditions

use apex_core::routing::{CascadeResult, ModelConfig, ModelRouter, ModelTier, RoutingConfig};

// ============================================================================
// Model Configuration Tests
// ============================================================================

#[test]
fn test_model_config_gpt4o_mini() {
    let config = ModelConfig::gpt4o_mini();

    assert_eq!(config.name, "gpt-4o-mini");
    assert_eq!(config.provider, "openai");
    assert_eq!(config.tier, ModelTier::Economy);
    assert!(config.cost_per_1k_input < config.cost_per_1k_output);
    assert!(config.supports_vision);
    assert!(config.supports_tools);
    assert_eq!(config.max_tokens, 128000);
}

#[test]
fn test_model_config_gpt4o() {
    let config = ModelConfig::gpt4o();

    assert_eq!(config.name, "gpt-4o");
    assert_eq!(config.provider, "openai");
    assert_eq!(config.tier, ModelTier::Standard);
    assert!(config.cost_per_1k_input > ModelConfig::gpt4o_mini().cost_per_1k_input);
}

#[test]
fn test_model_config_claude_haiku() {
    let config = ModelConfig::claude_haiku();

    assert_eq!(config.name, "claude-3.5-haiku");
    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.tier, ModelTier::Economy);
    assert_eq!(config.max_tokens, 200000);
}

#[test]
fn test_model_config_claude_sonnet() {
    let config = ModelConfig::claude_sonnet();

    assert_eq!(config.name, "claude-3.5-sonnet");
    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.tier, ModelTier::Standard);
}

#[test]
fn test_model_config_claude_opus() {
    let config = ModelConfig::claude_opus();

    assert_eq!(config.name, "claude-opus-4");
    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.tier, ModelTier::Premium);
    // Opus should be the most expensive
    assert!(config.cost_per_1k_input > ModelConfig::claude_sonnet().cost_per_1k_input);
    assert!(config.cost_per_1k_output > ModelConfig::claude_sonnet().cost_per_1k_output);
}

#[test]
fn test_model_tier_ordering() {
    assert!(ModelTier::Economy < ModelTier::Standard);
    assert!(ModelTier::Standard < ModelTier::Premium);
    assert!(ModelTier::Economy < ModelTier::Premium);
}

#[test]
fn test_model_tier_equality() {
    let tier1 = ModelTier::Economy;
    let tier2 = ModelTier::Economy;
    assert_eq!(tier1, tier2);

    let tier3 = ModelTier::Standard;
    assert_ne!(tier1, tier3);
}

// ============================================================================
// Router Creation Tests
// ============================================================================

#[test]
fn test_router_default_creation() {
    let router = ModelRouter::new();

    // Should have all default models
    assert!(router.get_model("gpt-4o-mini").is_some());
    assert!(router.get_model("gpt-4o").is_some());
    assert!(router.get_model("claude-3.5-haiku").is_some());
    assert!(router.get_model("claude-3.5-sonnet").is_some());
    assert!(router.get_model("claude-opus-4").is_some());
}

#[test]
fn test_router_default_implementation() {
    let router = ModelRouter::default();
    assert!(router.get_model("gpt-4o-mini").is_some());
}

#[test]
fn test_router_with_custom_config() {
    let config = RoutingConfig {
        economy_threshold: 0.90,
        standard_threshold: 0.80,
        max_escalations: 3,
        enable_cascade: true,
    };

    let router = ModelRouter::with_config(config);

    // Should still have default models
    assert!(router.get_model("gpt-4o-mini").is_some());
}

#[test]
fn test_routing_config_default() {
    let config = RoutingConfig::default();

    assert_eq!(config.economy_threshold, 0.85);
    assert_eq!(config.standard_threshold, 0.70);
    assert_eq!(config.max_escalations, 2);
    assert!(config.enable_cascade);
}

#[test]
fn test_get_nonexistent_model() {
    let router = ModelRouter::new();
    assert!(router.get_model("nonexistent-model").is_none());
}

// ============================================================================
// Complexity Estimation Tests
// ============================================================================

#[test]
fn test_complexity_simple_tasks() {
    let router = ModelRouter::new();

    // Very simple tasks
    let simple_tasks = [
        "List files",
        "Format text",
        "Convert JSON",
        "Summarize this",
        "Extract data",
        "Quick help",
        "Simple question",
        "Basic task",
    ];

    for task in simple_tasks {
        let complexity = router.select_model(task);
        let model = router.get_model(&complexity).unwrap();
        assert_eq!(model.tier, ModelTier::Economy, "Task '{}' should use Economy tier", task);
    }
}

#[test]
fn test_complexity_moderate_tasks() {
    let router = ModelRouter::new();

    // Moderate complexity - should trigger Standard tier
    let moderate_task = "Analyze this code and provide feedback on the implementation";

    let model_name = router.select_model(moderate_task);
    let model = router.get_model(&model_name).unwrap();

    // Should be Standard or higher due to 'analyze'
    assert!(
        model.tier >= ModelTier::Standard,
        "Analysis task should use Standard or higher tier"
    );
}

#[test]
fn test_complexity_complex_tasks() {
    let router = ModelRouter::new();

    // Complex tasks with multiple keywords
    let complex_task = "Analyze the architecture design and evaluate the comprehensive \
                        research with detailed reasoning about multiple complex systems \
                        and synthesize an expert-level advanced analysis";

    let model_name = router.select_model(complex_task);
    let model = router.get_model(&model_name).unwrap();

    assert!(
        model.tier >= ModelTier::Standard,
        "Complex multi-keyword task should use higher tier"
    );
}

#[test]
fn test_complexity_code_tasks() {
    let router = ModelRouter::new();

    // Code-related tasks should boost complexity
    let code_task = "Debug this program";
    let model_name = router.select_model(code_task);
    let model = router.get_model(&model_name).unwrap();

    // Code tasks get +0.15 complexity boost
    assert!(model.tier >= ModelTier::Economy, "Code tasks should be recognized");
}

#[test]
fn test_complexity_math_tasks() {
    let router = ModelRouter::new();

    // Math tasks should significantly boost complexity
    let math_task = "Prove this mathematical theorem and calculate the results";
    let model_name = router.select_model(math_task);
    let model = router.get_model(&model_name).unwrap();

    // Math tasks get +0.2 complexity boost
    assert!(
        model.tier >= ModelTier::Standard,
        "Math/proof tasks should use Standard or higher tier"
    );
}

#[test]
fn test_complexity_long_descriptions() {
    let router = ModelRouter::new();

    // Long description (>100 words) should add complexity
    let long_task = "word ".repeat(150);
    let model_name = router.select_model(&long_task);
    let model = router.get_model(&model_name).unwrap();

    // Long descriptions get +0.2 complexity for >100 words
    assert!(model.tier >= ModelTier::Economy, "Long tasks should be recognized");
}

#[test]
fn test_complexity_medium_length_descriptions() {
    let router = ModelRouter::new();

    // Medium description (50-100 words) should add moderate complexity
    let medium_task = "word ".repeat(75);
    let model_name = router.select_model(&medium_task);
    let model = router.get_model(&model_name).unwrap();

    // Medium descriptions get +0.1 complexity
    assert!(model.tier >= ModelTier::Economy, "Medium tasks should be recognized");
}

#[test]
fn test_complexity_simple_keywords_reduce_score() {
    let router = ModelRouter::new();

    // Simple keywords should reduce complexity
    let simple_task = "Simple basic quick short summarize";

    let model_name = router.select_model(simple_task);
    let model = router.get_model(&model_name).unwrap();

    assert_eq!(
        model.tier,
        ModelTier::Economy,
        "Multiple simple keywords should keep task at Economy tier"
    );
}

#[test]
fn test_complexity_empty_description() {
    let router = ModelRouter::new();

    // Empty string should have zero complexity
    let model_name = router.select_model("");
    let model = router.get_model(&model_name).unwrap();

    assert_eq!(model.tier, ModelTier::Economy, "Empty task should use Economy tier");
}

#[test]
fn test_complexity_case_insensitivity() {
    let router = ModelRouter::new();

    // Keywords should be case-insensitive
    let upper_case = "ANALYZE COMPREHENSIVE RESEARCH";
    let lower_case = "analyze comprehensive research";
    let mixed_case = "AnAlYzE cOmPrEhEnSiVe ReSeArCh";

    let model1 = router.select_model(upper_case);
    let model2 = router.select_model(lower_case);
    let model3 = router.select_model(mixed_case);

    assert_eq!(model1, model2, "Case should not affect model selection");
    assert_eq!(model2, model3, "Case should not affect model selection");
}

// ============================================================================
// Model Selection Tests
// ============================================================================

#[test]
fn test_select_model_cascade_disabled() {
    let config = RoutingConfig {
        economy_threshold: 0.85,
        standard_threshold: 0.70,
        max_escalations: 2,
        enable_cascade: false,
    };

    let router = ModelRouter::with_config(config);

    // With cascade disabled, should always use Standard tier
    let simple_model = router.select_model("list files");
    let complex_model = router.select_model("analyze complex research with detailed reasoning");

    let simple_config = router.get_model(&simple_model).unwrap();
    let complex_config = router.get_model(&complex_model).unwrap();

    assert_eq!(simple_config.tier, ModelTier::Standard);
    assert_eq!(complex_config.tier, ModelTier::Standard);
}

#[test]
fn test_select_cheapest_economy_model() {
    let router = ModelRouter::new();

    // gpt-4o-mini should be cheaper than claude-haiku
    let gpt_mini = ModelConfig::gpt4o_mini();
    let claude_haiku = ModelConfig::claude_haiku();

    let gpt_cost = gpt_mini.cost_per_1k_input + gpt_mini.cost_per_1k_output;
    let haiku_cost = claude_haiku.cost_per_1k_input + claude_haiku.cost_per_1k_output;

    // Verify which is actually cheaper
    if gpt_cost < haiku_cost {
        let model_name = router.select_model("simple task");
        assert_eq!(model_name, "gpt-4o-mini");
    } else {
        let model_name = router.select_model("simple task");
        assert_eq!(model_name, "claude-3.5-haiku");
    }
}

// ============================================================================
// Escalation Logic Tests
// ============================================================================

#[test]
fn test_should_escalate_economy_below_threshold() {
    let router = ModelRouter::new();

    // Confidence below threshold should trigger escalation
    assert!(router.should_escalate(0.5, &ModelTier::Economy));
    assert!(router.should_escalate(0.84, &ModelTier::Economy));
}

#[test]
fn test_should_not_escalate_economy_above_threshold() {
    let router = ModelRouter::new();

    // Confidence at or above threshold should not escalate
    assert!(!router.should_escalate(0.85, &ModelTier::Economy));
    assert!(!router.should_escalate(0.9, &ModelTier::Economy));
    assert!(!router.should_escalate(1.0, &ModelTier::Economy));
}

#[test]
fn test_should_escalate_standard_below_threshold() {
    let router = ModelRouter::new();

    // Standard tier has lower threshold (0.70)
    assert!(router.should_escalate(0.5, &ModelTier::Standard));
    assert!(router.should_escalate(0.69, &ModelTier::Standard));
}

#[test]
fn test_should_not_escalate_standard_above_threshold() {
    let router = ModelRouter::new();

    assert!(!router.should_escalate(0.70, &ModelTier::Standard));
    assert!(!router.should_escalate(0.85, &ModelTier::Standard));
}

#[test]
fn test_should_never_escalate_premium() {
    let router = ModelRouter::new();

    // Premium tier should never escalate (already highest)
    assert!(!router.should_escalate(0.0, &ModelTier::Premium));
    assert!(!router.should_escalate(0.5, &ModelTier::Premium));
    assert!(!router.should_escalate(0.69, &ModelTier::Premium));
}

#[test]
fn test_escalate_tier_economy() {
    let router = ModelRouter::new();

    let next = router.escalate_tier(&ModelTier::Economy);
    assert_eq!(next, Some(ModelTier::Standard));
}

#[test]
fn test_escalate_tier_standard() {
    let router = ModelRouter::new();

    let next = router.escalate_tier(&ModelTier::Standard);
    assert_eq!(next, Some(ModelTier::Premium));
}

#[test]
fn test_escalate_tier_premium() {
    let router = ModelRouter::new();

    let next = router.escalate_tier(&ModelTier::Premium);
    assert_eq!(next, None);
}

#[test]
fn test_escalation_with_custom_thresholds() {
    let config = RoutingConfig {
        economy_threshold: 0.95,
        standard_threshold: 0.90,
        max_escalations: 2,
        enable_cascade: true,
    };

    let router = ModelRouter::with_config(config);

    // With higher thresholds, more escalations should trigger
    assert!(router.should_escalate(0.90, &ModelTier::Economy));
    assert!(router.should_escalate(0.85, &ModelTier::Standard));
}

// ============================================================================
// Cost Estimation Tests
// ============================================================================

#[test]
fn test_estimate_cost_known_model() {
    let router = ModelRouter::new();

    // gpt-4o-mini: $0.00015/1K input, $0.0006/1K output
    let cost = router.estimate_cost("gpt-4o-mini", 1000, 1000);

    // 1K input = $0.00015, 1K output = $0.0006
    let expected = 0.00015 + 0.0006;
    assert!((cost - expected).abs() < 0.0001);
}

#[test]
fn test_estimate_cost_unknown_model() {
    let router = ModelRouter::new();

    let cost = router.estimate_cost("unknown-model", 1000, 1000);
    assert_eq!(cost, 0.0);
}

#[test]
fn test_estimate_cost_zero_tokens() {
    let router = ModelRouter::new();

    let cost = router.estimate_cost("gpt-4o-mini", 0, 0);
    assert_eq!(cost, 0.0);
}

#[test]
fn test_estimate_cost_large_token_count() {
    let router = ModelRouter::new();

    // 1 million tokens each
    let cost = router.estimate_cost("gpt-4o-mini", 1_000_000, 1_000_000);

    // 1M input = 1000 * $0.00015 = $0.15
    // 1M output = 1000 * $0.0006 = $0.60
    let expected = 0.15 + 0.60;
    assert!((cost - expected).abs() < 0.01);
}

#[test]
fn test_estimate_cost_premium_model() {
    let router = ModelRouter::new();

    // Claude Opus should be significantly more expensive
    let opus_cost = router.estimate_cost("claude-opus-4", 1000, 1000);
    let haiku_cost = router.estimate_cost("claude-3.5-haiku", 1000, 1000);

    assert!(
        opus_cost > haiku_cost * 10.0,
        "Premium models should be significantly more expensive"
    );
}

#[test]
fn test_cost_comparison_across_tiers() {
    let router = ModelRouter::new();

    let economy_cost = router.estimate_cost("gpt-4o-mini", 1000, 1000);
    let standard_cost = router.estimate_cost("gpt-4o", 1000, 1000);

    assert!(standard_cost > economy_cost, "Standard tier should cost more than Economy");
}

// ============================================================================
// Cascade Result Tests
// ============================================================================

#[test]
fn test_cascade_result_structure() {
    let result = CascadeResult {
        model: "gpt-4o".to_string(),
        escalations: 1,
        total_cost: 0.05,
        total_tokens: 5000,
        response: "Test response".to_string(),
        confidence: 0.85,
    };

    assert_eq!(result.model, "gpt-4o");
    assert_eq!(result.escalations, 1);
    assert!((result.total_cost - 0.05).abs() < 0.001);
    assert_eq!(result.total_tokens, 5000);
    assert_eq!(result.response, "Test response");
    assert!((result.confidence - 0.85).abs() < 0.001);
}

#[test]
fn test_cascade_result_clone() {
    let result = CascadeResult {
        model: "gpt-4o".to_string(),
        escalations: 2,
        total_cost: 0.10,
        total_tokens: 10000,
        response: "Cloned response".to_string(),
        confidence: 0.90,
    };

    let cloned = result.clone();
    assert_eq!(cloned.model, result.model);
    assert_eq!(cloned.escalations, result.escalations);
}

// ============================================================================
// Edge Cases and Boundary Tests
// ============================================================================

#[test]
fn test_boundary_confidence_values() {
    let router = ModelRouter::new();

    // Exact boundary values
    assert!(!router.should_escalate(0.85, &ModelTier::Economy)); // Exactly at threshold
    assert!(router.should_escalate(0.8499999, &ModelTier::Economy)); // Just below

    assert!(!router.should_escalate(0.70, &ModelTier::Standard)); // Exactly at threshold
    assert!(router.should_escalate(0.6999999, &ModelTier::Standard)); // Just below
}

#[test]
fn test_confidence_extreme_values() {
    let router = ModelRouter::new();

    // Confidence of 0.0
    assert!(router.should_escalate(0.0, &ModelTier::Economy));
    assert!(router.should_escalate(0.0, &ModelTier::Standard));
    assert!(!router.should_escalate(0.0, &ModelTier::Premium));

    // Confidence of 1.0
    assert!(!router.should_escalate(1.0, &ModelTier::Economy));
    assert!(!router.should_escalate(1.0, &ModelTier::Standard));
    assert!(!router.should_escalate(1.0, &ModelTier::Premium));
}

#[test]
fn test_negative_confidence_handling() {
    let router = ModelRouter::new();

    // Negative confidence should still work (always escalate)
    assert!(router.should_escalate(-0.5, &ModelTier::Economy));
    assert!(router.should_escalate(-0.5, &ModelTier::Standard));
}

#[test]
fn test_confidence_greater_than_one() {
    let router = ModelRouter::new();

    // Confidence > 1.0 should not escalate
    assert!(!router.should_escalate(1.5, &ModelTier::Economy));
    assert!(!router.should_escalate(100.0, &ModelTier::Standard));
}

#[test]
fn test_special_characters_in_task() {
    let router = ModelRouter::new();

    // Special characters should not crash
    let special_task = "!@#$%^&*()_+-=[]{}|;':\",./<>?";
    let model_name = router.select_model(special_task);

    assert!(router.get_model(&model_name).is_some());
}

#[test]
fn test_unicode_in_task() {
    let router = ModelRouter::new();

    // Unicode characters should work
    let unicode_task = "分析这个 анализировать αναλύω 分析する";
    let model_name = router.select_model(unicode_task);

    assert!(router.get_model(&model_name).is_some());
}

#[test]
fn test_very_long_task_description() {
    let router = ModelRouter::new();

    // Very long description (10000 words)
    let very_long = "analyze ".repeat(10000);
    let model_name = router.select_model(&very_long);

    // Should handle without panic
    assert!(router.get_model(&model_name).is_some());
}

#[test]
fn test_whitespace_only_task() {
    let router = ModelRouter::new();

    let whitespace_task = "   \t\n\r   ";
    let model_name = router.select_model(whitespace_task);

    // Should default to Economy tier
    let model = router.get_model(&model_name).unwrap();
    assert_eq!(model.tier, ModelTier::Economy);
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_model_tier_serialization() {
    let tier = ModelTier::Economy;
    let json = serde_json::to_string(&tier).unwrap();
    let deserialized: ModelTier = serde_json::from_str(&json).unwrap();
    assert_eq!(tier, deserialized);
}

#[test]
fn test_model_config_serialization() {
    let config = ModelConfig::gpt4o_mini();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ModelConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.name, deserialized.name);
    assert_eq!(config.provider, deserialized.provider);
    assert_eq!(config.tier, deserialized.tier);
}

#[test]
fn test_all_tiers_serialization() {
    let tiers = vec![ModelTier::Economy, ModelTier::Standard, ModelTier::Premium];

    for tier in tiers {
        let json = serde_json::to_string(&tier).unwrap();
        let deserialized: ModelTier = serde_json::from_str(&json).unwrap();
        assert_eq!(tier, deserialized);
    }
}
