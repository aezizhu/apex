# =============================================================================
# Apex Infrastructure - Main Configuration
# =============================================================================
# This Terraform configuration deploys a complete AWS EKS-based infrastructure
# including VPC, EKS cluster, RDS PostgreSQL, ElastiCache Redis, and monitoring.
# =============================================================================

# -----------------------------------------------------------------------------
# Provider Configuration
# -----------------------------------------------------------------------------

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = merge(
      {
        Project     = var.project_name
        Environment = var.environment
        ManagedBy   = "terraform"
      },
      var.tags
    )
  }
}

# Kubernetes provider configuration (depends on EKS cluster)
provider "kubernetes" {
  host                   = module.eks.cluster_endpoint
  cluster_ca_certificate = base64decode(module.eks.cluster_certificate_authority_data)

  exec {
    api_version = "client.authentication.k8s.io/v1beta1"
    command     = "aws"
    args        = ["eks", "get-token", "--cluster-name", module.eks.cluster_name]
  }
}

# Helm provider configuration
provider "helm" {
  kubernetes {
    host                   = module.eks.cluster_endpoint
    cluster_ca_certificate = base64decode(module.eks.cluster_certificate_authority_data)

    exec {
      api_version = "client.authentication.k8s.io/v1beta1"
      command     = "aws"
      args        = ["eks", "get-token", "--cluster-name", module.eks.cluster_name]
    }
  }
}

# -----------------------------------------------------------------------------
# Data Sources
# -----------------------------------------------------------------------------

data "aws_caller_identity" "current" {}

data "aws_availability_zones" "available" {
  state = "available"
}

# -----------------------------------------------------------------------------
# Local Variables
# -----------------------------------------------------------------------------

locals {
  name_prefix = "${var.project_name}-${var.environment}"

  common_tags = {
    Project     = var.project_name
    Environment = var.environment
    ManagedBy   = "terraform"
  }

  # Use provided AZs or default to first 3 available
  azs = length(var.availability_zones) > 0 ? var.availability_zones : slice(data.aws_availability_zones.available.names, 0, 3)
}

# -----------------------------------------------------------------------------
# VPC Module
# -----------------------------------------------------------------------------

module "vpc" {
  source = "./modules/vpc"

  name_prefix           = local.name_prefix
  vpc_cidr              = var.vpc_cidr
  availability_zones    = local.azs
  private_subnet_cidrs  = var.private_subnet_cidrs
  public_subnet_cidrs   = var.public_subnet_cidrs
  database_subnet_cidrs = var.database_subnet_cidrs

  enable_nat_gateway = var.enable_nat_gateway
  single_nat_gateway = var.single_nat_gateway

  # EKS-specific tags for subnet discovery
  eks_cluster_name = local.name_prefix

  tags = local.common_tags
}

# -----------------------------------------------------------------------------
# EKS Module
# -----------------------------------------------------------------------------

module "eks" {
  source = "./modules/eks"

  name_prefix     = local.name_prefix
  cluster_version = var.eks_cluster_version

  vpc_id             = module.vpc.vpc_id
  private_subnet_ids = module.vpc.private_subnet_ids
  public_subnet_ids  = module.vpc.public_subnet_ids

  node_instance_types = var.eks_node_instance_types
  node_desired_size   = var.eks_node_desired_size
  node_min_size       = var.eks_node_min_size
  node_max_size       = var.eks_node_max_size
  node_disk_size      = var.eks_node_disk_size

  enable_cluster_autoscaler = var.enable_cluster_autoscaler

  tags = local.common_tags

  depends_on = [module.vpc]
}

# -----------------------------------------------------------------------------
# RDS Module
# -----------------------------------------------------------------------------

module "rds" {
  source = "./modules/rds"

  name_prefix = local.name_prefix

  vpc_id                 = module.vpc.vpc_id
  database_subnet_ids    = module.vpc.database_subnet_ids
  allowed_security_groups = [module.eks.node_security_group_id]

  instance_class        = var.rds_instance_class
  allocated_storage     = var.rds_allocated_storage
  max_allocated_storage = var.rds_max_allocated_storage
  engine_version        = var.rds_engine_version

  database_name = var.rds_database_name
  username      = var.rds_username

  multi_az                = var.rds_multi_az
  backup_retention_period = var.rds_backup_retention_period
  deletion_protection     = var.rds_deletion_protection
  skip_final_snapshot     = var.rds_skip_final_snapshot

  enable_enhanced_monitoring = var.enable_enhanced_monitoring
  monitoring_interval        = var.monitoring_interval

  tags = local.common_tags

  depends_on = [module.vpc]
}

# -----------------------------------------------------------------------------
# ElastiCache Module
# -----------------------------------------------------------------------------

module "elasticache" {
  source = "./modules/elasticache"

  name_prefix = local.name_prefix

  vpc_id                  = module.vpc.vpc_id
  private_subnet_ids      = module.vpc.private_subnet_ids
  allowed_security_groups = [module.eks.node_security_group_id]

  node_type               = var.elasticache_node_type
  num_cache_nodes         = var.elasticache_num_cache_nodes
  engine_version          = var.elasticache_engine_version
  parameter_group_family  = var.elasticache_parameter_group_family

  automatic_failover = var.elasticache_automatic_failover
  multi_az           = var.elasticache_multi_az

  tags = local.common_tags

  depends_on = [module.vpc]
}

# -----------------------------------------------------------------------------
# Monitoring Module
# -----------------------------------------------------------------------------

module "monitoring" {
  source = "./modules/monitoring"

  name_prefix  = local.name_prefix
  aws_region   = var.aws_region

  eks_cluster_name = module.eks.cluster_name
  rds_instance_id  = module.rds.instance_id
  elasticache_cluster_id = module.elasticache.cluster_id

  enable_container_insights = var.enable_container_insights
  log_retention_days        = var.log_retention_days
  alarm_email               = var.alarm_email

  tags = local.common_tags

  depends_on = [module.eks, module.rds, module.elasticache]
}
