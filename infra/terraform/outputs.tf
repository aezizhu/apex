# =============================================================================
# VPC Outputs
# =============================================================================

output "vpc_id" {
  description = "ID of the VPC"
  value       = module.vpc.vpc_id
}

output "vpc_cidr" {
  description = "CIDR block of the VPC"
  value       = module.vpc.vpc_cidr
}

output "private_subnet_ids" {
  description = "IDs of private subnets"
  value       = module.vpc.private_subnet_ids
}

output "public_subnet_ids" {
  description = "IDs of public subnets"
  value       = module.vpc.public_subnet_ids
}

output "database_subnet_ids" {
  description = "IDs of database subnets"
  value       = module.vpc.database_subnet_ids
}

output "nat_gateway_ips" {
  description = "Elastic IPs of NAT Gateways"
  value       = module.vpc.nat_gateway_ips
}

# =============================================================================
# EKS Outputs
# =============================================================================

output "eks_cluster_id" {
  description = "ID of the EKS cluster"
  value       = module.eks.cluster_id
}

output "eks_cluster_name" {
  description = "Name of the EKS cluster"
  value       = module.eks.cluster_name
}

output "eks_cluster_endpoint" {
  description = "Endpoint for the EKS cluster API server"
  value       = module.eks.cluster_endpoint
}

output "eks_cluster_certificate_authority_data" {
  description = "Base64 encoded certificate data for the cluster"
  value       = module.eks.cluster_certificate_authority_data
  sensitive   = true
}

output "eks_cluster_security_group_id" {
  description = "Security group ID attached to the EKS cluster"
  value       = module.eks.cluster_security_group_id
}

output "eks_node_security_group_id" {
  description = "Security group ID attached to the EKS nodes"
  value       = module.eks.node_security_group_id
}

output "eks_oidc_provider_arn" {
  description = "ARN of the OIDC Provider for IRSA"
  value       = module.eks.oidc_provider_arn
}

output "eks_kubectl_config" {
  description = "kubectl config command"
  value       = "aws eks update-kubeconfig --region ${var.aws_region} --name ${module.eks.cluster_name}"
}

# =============================================================================
# RDS Outputs
# =============================================================================

output "rds_endpoint" {
  description = "RDS instance endpoint"
  value       = module.rds.endpoint
}

output "rds_port" {
  description = "RDS instance port"
  value       = module.rds.port
}

output "rds_database_name" {
  description = "Name of the database"
  value       = module.rds.database_name
}

output "rds_username" {
  description = "Master username"
  value       = module.rds.username
  sensitive   = true
}

output "rds_password_secret_arn" {
  description = "ARN of the Secrets Manager secret containing the password"
  value       = module.rds.password_secret_arn
}

output "rds_instance_id" {
  description = "RDS instance identifier"
  value       = module.rds.instance_id
}

# =============================================================================
# ElastiCache Outputs
# =============================================================================

output "elasticache_endpoint" {
  description = "ElastiCache primary endpoint"
  value       = module.elasticache.primary_endpoint
}

output "elasticache_port" {
  description = "ElastiCache port"
  value       = module.elasticache.port
}

output "elasticache_configuration_endpoint" {
  description = "ElastiCache configuration endpoint (for cluster mode)"
  value       = module.elasticache.configuration_endpoint
}

output "elasticache_cluster_id" {
  description = "ElastiCache cluster identifier"
  value       = module.elasticache.cluster_id
}

# =============================================================================
# Monitoring Outputs
# =============================================================================

output "cloudwatch_log_group_name" {
  description = "CloudWatch log group name for EKS"
  value       = module.monitoring.log_group_name
}

output "sns_topic_arn" {
  description = "SNS topic ARN for alerts"
  value       = module.monitoring.sns_topic_arn
}

output "cloudwatch_dashboard_url" {
  description = "URL to CloudWatch dashboard"
  value       = module.monitoring.dashboard_url
}

# =============================================================================
# Connection Strings (for application configuration)
# =============================================================================

output "database_connection_string" {
  description = "PostgreSQL connection string template (password from Secrets Manager)"
  value       = "postgresql://${module.rds.username}:<password>@${module.rds.endpoint}/${module.rds.database_name}"
  sensitive   = true
}

output "redis_connection_string" {
  description = "Redis connection string"
  value       = "redis://${module.elasticache.primary_endpoint}:${module.elasticache.port}"
}
