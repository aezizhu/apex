# =============================================================================
# Apex Infrastructure - Development Environment
# =============================================================================
# Development environment configuration with cost-optimized settings.
# Uses smaller instance sizes, single NAT gateway, and reduced redundancy.
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
    # key            = "dev/terraform.tfstate"
    # region         = "us-west-2"
    # encrypt        = true
    # dynamodb_table = "terraform-state-lock"
  }
}

# -----------------------------------------------------------------------------
# Local Variables
# -----------------------------------------------------------------------------

locals {
  environment = "dev"
  aws_region  = "us-west-2"

  # Cost-optimized settings for development
  tags = {
    Project     = "apex"
    Environment = local.environment
    ManagedBy   = "terraform"
    CostCenter  = "development"
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

  # VPC Configuration - Development (cost-optimized)
  vpc_cidr              = "10.0.0.0/16"
  availability_zones    = ["us-west-2a", "us-west-2b"]  # 2 AZs for dev
  private_subnet_cidrs  = ["10.0.1.0/24", "10.0.2.0/24"]
  public_subnet_cidrs   = ["10.0.101.0/24", "10.0.102.0/24"]
  database_subnet_cidrs = ["10.0.201.0/24", "10.0.202.0/24"]
  enable_nat_gateway    = true
  single_nat_gateway    = true  # Single NAT to reduce costs

  # EKS Configuration - Development (smaller instances)
  eks_cluster_version     = "1.28"
  eks_node_instance_types = ["t3.medium"]
  eks_node_desired_size   = 2
  eks_node_min_size       = 1
  eks_node_max_size       = 4
  eks_node_disk_size      = 30
  enable_cluster_autoscaler = true

  # RDS Configuration - Development (single AZ, smaller instance)
  rds_instance_class          = "db.t3.small"
  rds_allocated_storage       = 20
  rds_max_allocated_storage   = 50
  rds_engine_version          = "15.4"
  rds_database_name           = "apex_dev"
  rds_username                = "apex_admin"
  rds_multi_az                = false  # Single AZ for dev
  rds_backup_retention_period = 3
  rds_deletion_protection     = false
  rds_skip_final_snapshot     = true

  # ElastiCache Configuration - Development (single node)
  elasticache_node_type       = "cache.t3.micro"
  elasticache_num_cache_nodes = 1
  elasticache_engine_version  = "7.0"
  elasticache_automatic_failover = false
  elasticache_multi_az        = false

  # Monitoring Configuration - Development
  enable_container_insights = true
  log_retention_days        = 14
  alarm_email               = ""  # Set your email for alerts
  enable_enhanced_monitoring = true
  monitoring_interval       = 60
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
