//! FrugalGPT-style Adaptive Model Routing
//!
//! Implements a cascade strategy where cheaper models are tried first,
//! escalating to more expensive models only when needed.

use serde::{Deserialize, Serialize};

/// Model tier in the cascade.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ModelTier {
    /// Cheapest, fastest models (GPT-4o-mini, Claude Haiku)
    Economy,
    /// Mid-tier models (GPT-4o, Claude Sonnet)
    Standard,
    /// Most capable models (GPT-4, Claude Opus)
    Premium,
}

/// Model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    pub provider: String,
    pub tier: ModelTier,
    pub cost_per_1k_input: f64,
    pub cost_per_1k_output: f64,
    pub max_tokens: u32,
    pub supports_vision: bool,
    pub supports_tools: bool,
}

impl ModelConfig {
    /// OpenAI GPT-4o-mini
    pub fn gpt4o_mini() -> Self {
        Self {
            name: "gpt-4o-mini".to_string(),
            provider: "openai".to_string(),
            tier: ModelTier::Economy,
            cost_per_1k_input: 0.00015,
            cost_per_1k_output: 0.0006,
            max_tokens: 128000,
            supports_vision: true,
            supports_tools: true,
        }
    }

    /// OpenAI GPT-4o
    pub fn gpt4o() -> Self {
        Self {
            name: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            tier: ModelTier::Standard,
            cost_per_1k_input: 0.005,
            cost_per_1k_output: 0.015,
            max_tokens: 128000,
            supports_vision: true,
            supports_tools: true,
        }
    }

    /// Anthropic Claude 3.5 Haiku
    pub fn claude_haiku() -> Self {
        Self {
            name: "claude-3.5-haiku".to_string(),
            provider: "anthropic".to_string(),
            tier: ModelTier::Economy,
            cost_per_1k_input: 0.00025,
            cost_per_1k_output: 0.00125,
            max_tokens: 200000,
            supports_vision: true,
            supports_tools: true,
        }
    }

    /// Anthropic Claude 3.5 Sonnet
    pub fn claude_sonnet() -> Self {
        Self {
            name: "claude-3.5-sonnet".to_string(),
            provider: "anthropic".to_string(),
            tier: ModelTier::Standard,
            cost_per_1k_input: 0.003,
            cost_per_1k_output: 0.015,
            max_tokens: 200000,
            supports_vision: true,
            supports_tools: true,
        }
    }

    /// Anthropic Claude Opus 4
    pub fn claude_opus() -> Self {
        Self {
            name: "claude-opus-4".to_string(),
            provider: "anthropic".to_string(),
            tier: ModelTier::Premium,
            cost_per_1k_input: 0.015,
            cost_per_1k_output: 0.075,
            max_tokens: 200000,
            supports_vision: true,
            supports_tools: true,
        }
    }
}

/// Model routing configuration.
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Confidence threshold for accepting economy tier response
    pub economy_threshold: f64,

    /// Confidence threshold for accepting standard tier response
    pub standard_threshold: f64,

    /// Maximum escalation attempts
    pub max_escalations: u32,

    /// Enable cascade routing
    pub enable_cascade: bool,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            economy_threshold: 0.85,
            standard_threshold: 0.70,
            max_escalations: 2,
            enable_cascade: true,
        }
    }
}

/// Model router implementing FrugalGPT-style cascade.
pub struct ModelRouter {
    /// Available models by tier
    models: Vec<ModelConfig>,

    /// Routing configuration
    config: RoutingConfig,
}

impl ModelRouter {
    /// Create a new model router with default models.
    pub fn new() -> Self {
        let models = vec![
            ModelConfig::gpt4o_mini(),
            ModelConfig::claude_haiku(),
            ModelConfig::gpt4o(),
            ModelConfig::claude_sonnet(),
            ModelConfig::claude_opus(),
        ];

        Self {
            models,
            config: RoutingConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: RoutingConfig) -> Self {
        let mut router = Self::new();
        router.config = config;
        router
    }

    /// Select the most appropriate model for a task.
    ///
    /// Uses heuristics based on task complexity to choose initial model.
    pub fn select_model(&self, task_description: &str) -> String {
        if !self.config.enable_cascade {
            // Default to standard tier if cascade disabled
            return self.models.iter()
                .find(|m| m.tier == ModelTier::Standard)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "gpt-4o".to_string());
        }

        let complexity = self.estimate_complexity(task_description);

        let target_tier = if complexity < 0.3 {
            ModelTier::Economy
        } else if complexity < 0.7 {
            ModelTier::Standard
        } else {
            ModelTier::Premium
        };

        self.get_cheapest_model_for_tier(&target_tier)
    }

    /// Get the cheapest model for a given tier.
    fn get_cheapest_model_for_tier(&self, tier: &ModelTier) -> String {
        self.models.iter()
            .filter(|m| &m.tier == tier)
            .min_by(|a, b| {
                let cost_a = a.cost_per_1k_input + a.cost_per_1k_output;
                let cost_b = b.cost_per_1k_input + b.cost_per_1k_output;
                cost_a.partial_cmp(&cost_b).unwrap()
            })
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "gpt-4o-mini".to_string())
    }

    /// Estimate task complexity (0.0 - 1.0).
    fn estimate_complexity(&self, task_description: &str) -> f64 {
        let mut score: f64 = 0.0;
        let desc_lower = task_description.to_lowercase();

        // Length-based complexity
        let word_count = task_description.split_whitespace().count();
        if word_count > 100 {
            score += 0.2;
        } else if word_count > 50 {
            score += 0.1;
        }

        // Keyword-based complexity
        let complex_keywords = [
            "analyze", "synthesize", "compare", "evaluate", "design",
            "architecture", "complex", "multiple", "reasoning", "step-by-step",
            "research", "comprehensive", "detailed", "expert", "advanced"
        ];

        let simple_keywords = [
            "simple", "basic", "quick", "short", "summarize",
            "extract", "list", "format", "convert", "translate"
        ];

        for keyword in complex_keywords {
            if desc_lower.contains(keyword) {
                score += 0.1;
            }
        }

        for keyword in simple_keywords {
            if desc_lower.contains(keyword) {
                score -= 0.1;
            }
        }

        // Code-related tasks often need better models
        if desc_lower.contains("code") || desc_lower.contains("program") || desc_lower.contains("debug") {
            score += 0.2;
        }

        // Math and reasoning tasks
        if desc_lower.contains("math") || desc_lower.contains("calculate") || desc_lower.contains("prove") {
            score += 0.3;
        }

        score.clamp(0.0, 1.0)
    }

    /// Determine if response should be escalated to a higher tier.
    pub fn should_escalate(&self, confidence: f64, current_tier: &ModelTier) -> bool {
        match current_tier {
            ModelTier::Economy => confidence < self.config.economy_threshold,
            ModelTier::Standard => confidence < self.config.standard_threshold,
            ModelTier::Premium => false, // Already at highest tier
        }
    }

    /// Get the next tier for escalation.
    pub fn escalate_tier(&self, current_tier: &ModelTier) -> Option<ModelTier> {
        match current_tier {
            ModelTier::Economy => Some(ModelTier::Standard),
            ModelTier::Standard => Some(ModelTier::Premium),
            ModelTier::Premium => None,
        }
    }

    /// Get model by name.
    pub fn get_model(&self, name: &str) -> Option<&ModelConfig> {
        self.models.iter().find(|m| m.name == name)
    }

    /// Calculate estimated cost for a task.
    pub fn estimate_cost(&self, model_name: &str, input_tokens: u32, output_tokens: u32) -> f64 {
        self.get_model(model_name)
            .map(|m| {
                (input_tokens as f64 / 1000.0 * m.cost_per_1k_input) +
                (output_tokens as f64 / 1000.0 * m.cost_per_1k_output)
            })
            .unwrap_or(0.0)
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a cascade routing attempt.
#[derive(Debug, Clone)]
pub struct CascadeResult {
    /// Final model used
    pub model: String,

    /// Number of escalations performed
    pub escalations: u32,

    /// Total cost across all attempts
    pub total_cost: f64,

    /// Total tokens used
    pub total_tokens: u64,

    /// Final response
    pub response: String,

    /// Final confidence score
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_estimation() {
        let router = ModelRouter::new();

        // Simple task
        let simple = router.estimate_complexity("List the files in the directory");
        assert!(simple < 0.3);

        // Complex task
        let complex = router.estimate_complexity(
            "Analyze the codebase architecture and design a comprehensive testing strategy \
             that covers multiple modules with detailed reasoning about edge cases"
        );
        assert!(complex > 0.5);
    }

    #[test]
    fn test_model_selection() {
        let router = ModelRouter::new();

        // Simple task should use economy
        let model = router.select_model("Format this text");
        let config = router.get_model(&model).unwrap();
        assert_eq!(config.tier, ModelTier::Economy);

        // Complex task should use higher tier
        let model = router.select_model(
            "Analyze this complex mathematical proof and evaluate its correctness with detailed reasoning"
        );
        let config = router.get_model(&model).unwrap();
        assert!(config.tier >= ModelTier::Standard);
    }

    #[test]
    fn test_escalation() {
        let router = ModelRouter::new();

        assert!(router.should_escalate(0.5, &ModelTier::Economy));
        assert!(!router.should_escalate(0.9, &ModelTier::Economy));
        assert!(!router.should_escalate(0.5, &ModelTier::Premium));

        assert_eq!(router.escalate_tier(&ModelTier::Economy), Some(ModelTier::Standard));
        assert_eq!(router.escalate_tier(&ModelTier::Premium), None);
    }
}
