# =============================================================================
# Apex Infrastructure - Production Environment
# =============================================================================
# Production environment configuration with high availability settings.
# Uses Multi-AZ deployments, larger instances, and full redundancy.
# =============================================================================

terraform {
  required_version = ">= 1.5.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.11"
    }
    tls = {
      source  = "hashicorp/tls"
      version = "~> 4.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.5"
    }
  }

  backend "s3" {
    # Configure these values for your environment
    # bucket         = "apex-terraform-state"
    # key            = "prod/terraform.tfstate"
    # region         = "us-west-2"
    # encrypt        = true
    # dynamodb_table = "terraform-state-lock"
  }
}

# -----------------------------------------------------------------------------
# Local Variables
# -----------------------------------------------------------------------------

locals {
  environment = "prod"
  aws_region  = "us-west-2"

  # Production tags
  tags = {
    Project     = "apex"
    Environment = local.environment
    ManagedBy   = "terraform"
    CostCenter  = "production"
    Compliance  = "required"
  }
}

# -----------------------------------------------------------------------------
# Root Module
# -----------------------------------------------------------------------------

module "apex" {
  source = "../../"

  # General Configuration
  project_name = "apex"
  environment  = local.environment
  aws_region   = local.aws_region
  tags         = local.tags

  # VPC Configuration - Production (full redundancy)
  vpc_cidr              = "10.1.0.0/16"  # Different CIDR from dev
  availability_zones    = ["us-west-2a", "us-west-2b", "us-west-2c"]  # 3 AZs for prod
  private_subnet_cidrs  = ["10.1.1.0/24", "10.1.2.0/24", "10.1.3.0/24"]
  public_subnet_cidrs   = ["10.1.101.0/24", "10.1.102.0/24", "10.1.103.0/24"]
  database_subnet_cidrs = ["10.1.201.0/24", "10.1.202.0/24", "10.1.203.0/24"]
  enable_nat_gateway    = true
  single_nat_gateway    = false  # NAT per AZ for high availability

  # EKS Configuration - Production (larger instances, more nodes)
  eks_cluster_version     = "1.28"
  eks_node_instance_types = ["t3.large", "t3.xlarge"]
  eks_node_desired_size   = 3
  eks_node_min_size       = 2
  eks_node_max_size       = 10
  eks_node_disk_size      = 100
  enable_cluster_autoscaler = true

  # RDS Configuration - Production (Multi-AZ, larger instance)
  rds_instance_class          = "db.r6g.large"
  rds_allocated_storage       = 100
  rds_max_allocated_storage   = 500
  rds_engine_version          = "15.4"
  rds_database_name           = "apex_prod"
  rds_username                = "apex_admin"
  rds_multi_az                = true  # Multi-AZ for high availability
  rds_backup_retention_period = 30
  rds_deletion_protection     = true  # Protect against accidental deletion
  rds_skip_final_snapshot     = false

  # ElastiCache Configuration - Production (cluster mode with failover)
  elasticache_node_type       = "cache.r6g.large"
  elasticache_num_cache_nodes = 2
  elasticache_engine_version  = "7.0"
  elasticache_automatic_failover = true
  elasticache_multi_az        = true

  # Monitoring Configuration - Production (extended retention)
  enable_container_insights = true
  log_retention_days        = 90
  alarm_email               = ""  # REQUIRED: Set your ops team email
  enable_enhanced_monitoring = true
  monitoring_interval       = 30  # More frequent monitoring
}

# -----------------------------------------------------------------------------
# Additional Production Security - KMS Keys
# -----------------------------------------------------------------------------

resource "aws_kms_key" "eks" {
  description             = "KMS key for EKS secrets encryption"
  deletion_window_in_days = 30
  enable_key_rotation     = true

  tags = merge(local.tags, {
    Name = "apex-prod-eks-kms"
  })
}

resource "aws_kms_alias" "eks" {
  name          = "alias/apex-prod-eks"
  target_key_id = aws_kms_key.eks.key_id
}

resource "aws_kms_key" "rds" {
  description             = "KMS key for RDS encryption"
  deletion_window_in_days = 30
  enable_key_rotation     = true

  tags = merge(local.tags, {
    Name = "apex-prod-rds-kms"
  })
}

resource "aws_kms_alias" "rds" {
  name          = "alias/apex-prod-rds"
  target_key_id = aws_kms_key.rds.key_id
}

# -----------------------------------------------------------------------------
# Production WAF for Application Load Balancer
# -----------------------------------------------------------------------------

resource "aws_wafv2_web_acl" "main" {
  name        = "apex-prod-waf"
  description = "WAF ACL for Apex production"
  scope       = "REGIONAL"

  default_action {
    allow {}
  }

  # AWS Managed Rule - Common Rule Set
  rule {
    name     = "AWSManagedRulesCommonRuleSet"
    priority = 1

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesCommonRuleSet"
        vendor_name = "AWS"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "AWSManagedRulesCommonRuleSetMetric"
      sampled_requests_enabled   = true
    }
  }

  # AWS Managed Rule - SQL Injection
  rule {
    name     = "AWSManagedRulesSQLiRuleSet"
    priority = 2

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesSQLiRuleSet"
        vendor_name = "AWS"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "AWSManagedRulesSQLiRuleSetMetric"
      sampled_requests_enabled   = true
    }
  }

  # Rate Limiting Rule
  rule {
    name     = "RateLimitRule"
    priority = 3

    action {
      block {}
    }

    statement {
      rate_based_statement {
        limit              = 2000
        aggregate_key_type = "IP"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "RateLimitRuleMetric"
      sampled_requests_enabled   = true
    }
  }

  visibility_config {
    cloudwatch_metrics_enabled = true
    metric_name                = "ApexProdWAFMetric"
    sampled_requests_enabled   = true
  }

  tags = local.tags
}

# -----------------------------------------------------------------------------
# Outputs
# -----------------------------------------------------------------------------

output "vpc_id" {
  description = "VPC ID"
  value       = module.apex.vpc_id
}

output "eks_cluster_name" {
  description = "EKS cluster name"
  value       = module.apex.eks_cluster_name
}

output "eks_cluster_endpoint" {
  description = "EKS cluster endpoint"
  value       = module.apex.eks_cluster_endpoint
}

output "eks_kubectl_config" {
  description = "kubectl configuration command"
  value       = module.apex.eks_kubectl_config
}

output "rds_endpoint" {
  description = "RDS endpoint"
  value       = module.apex.rds_endpoint
}

output "rds_password_secret_arn" {
  description = "ARN of the RDS password secret"
  value       = module.apex.rds_password_secret_arn
}

output "elasticache_endpoint" {
  description = "ElastiCache endpoint"
  value       = module.apex.elasticache_endpoint
}

output "database_connection_string" {
  description = "Database connection string template"
  value       = module.apex.database_connection_string
  sensitive   = true
}

output "redis_connection_string" {
  description = "Redis connection string"
  value       = module.apex.redis_connection_string
}

output "cloudwatch_dashboard_url" {
  description = "CloudWatch dashboard URL"
  value       = module.apex.cloudwatch_dashboard_url
}

output "waf_acl_arn" {
  description = "WAF Web ACL ARN"
  value       = aws_wafv2_web_acl.main.arn
}

output "eks_kms_key_arn" {
  description = "KMS key ARN for EKS"
  value       = aws_kms_key.eks.arn
}

output "rds_kms_key_arn" {
  description = "KMS key ARN for RDS"
  value       = aws_kms_key.rds.arn
}
