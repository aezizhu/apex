# Apex Terraform Infrastructure

This Terraform configuration deploys a complete AWS EKS-based infrastructure for the Apex application.

## Architecture Overview

```
                                    ┌─────────────────────────────────────────────────────────┐
                                    │                         VPC                              │
                                    │                                                          │
    ┌───────────┐                   │  ┌─────────────────────────────────────────────────┐    │
    │  Internet │◄──────────────────┼──│              Public Subnets                      │    │
    └───────────┘                   │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐          │    │
          │                         │  │  │  NAT    │  │  NAT    │  │  NAT    │          │    │
          ▼                         │  │  │ Gateway │  │ Gateway │  │ Gateway │          │    │
    ┌───────────┐                   │  │  └────┬────┘  └────┬────┘  └────┬────┘          │    │
    │  Internet │                   │  │       │            │            │               │    │
    │  Gateway  │                   │  └───────┼────────────┼────────────┼───────────────┘    │
    └───────────┘                   │          │            │            │                    │
                                    │  ┌───────▼────────────▼────────────▼───────────────┐    │
                                    │  │              Private Subnets                     │    │
                                    │  │                                                  │    │
                                    │  │    ┌─────────────────────────────────────┐      │    │
                                    │  │    │           EKS Cluster                │      │    │
                                    │  │    │  ┌─────────────────────────────┐    │      │    │
                                    │  │    │  │      Managed Node Groups     │    │      │    │
                                    │  │    │  │  ┌─────┐ ┌─────┐ ┌─────┐    │    │      │    │
                                    │  │    │  │  │Node │ │Node │ │Node │    │    │      │    │
                                    │  │    │  │  └─────┘ └─────┘ └─────┘    │    │      │    │
                                    │  │    │  └─────────────────────────────┘    │      │    │
                                    │  │    └─────────────────────────────────────┘      │    │
                                    │  └──────────────────────────────────────────────────┘    │
                                    │                                                          │
                                    │  ┌──────────────────────────────────────────────────┐    │
                                    │  │              Database Subnets                     │    │
                                    │  │                                                   │    │
                                    │  │    ┌─────────────┐      ┌─────────────────┐      │    │
                                    │  │    │     RDS     │      │   ElastiCache   │      │    │
                                    │  │    │  PostgreSQL │      │      Redis      │      │    │
                                    │  │    │  (Multi-AZ) │      │    (Cluster)    │      │    │
                                    │  │    └─────────────┘      └─────────────────┘      │    │
                                    │  └──────────────────────────────────────────────────┘    │
                                    └─────────────────────────────────────────────────────────┘
```

## Components

### VPC Module (`modules/vpc`)
- VPC with DNS support enabled
- Public subnets (for NAT Gateways and Load Balancers)
- Private subnets (for EKS worker nodes)
- Database subnets (isolated for RDS and ElastiCache)
- NAT Gateways for outbound internet access
- VPC Flow Logs for network monitoring

### EKS Module (`modules/eks`)
- EKS cluster with Kubernetes 1.28
- Managed node groups with auto-scaling
- OIDC provider for IRSA (IAM Roles for Service Accounts)
- Security groups for cluster and nodes
- AWS Load Balancer Controller IAM role
- Cluster Autoscaler IAM role
- Core add-ons (VPC CNI, CoreDNS, kube-proxy)

### RDS Module (`modules/rds`)
- PostgreSQL 15.4 instance
- Encrypted storage (gp3)
- Automated backups
- Enhanced monitoring
- Performance Insights
- Password stored in Secrets Manager
- Custom parameter group

### ElastiCache Module (`modules/elasticache`)
- Redis 7.0 cluster
- Encryption at rest and in transit
- Automated snapshots
- Custom parameter group

### Monitoring Module (`modules/monitoring`)
- CloudWatch log groups
- SNS topic for alerts
- CloudWatch alarms for:
  - EKS node health and resource utilization
  - RDS CPU, storage, connections, and latency
  - ElastiCache CPU, memory, and evictions
- CloudWatch dashboard

## Prerequisites

1. **AWS CLI** configured with appropriate credentials
2. **Terraform** >= 1.5.0
3. **kubectl** for EKS cluster access
4. **S3 bucket** for Terraform state (optional but recommended)
5. **DynamoDB table** for state locking (optional but recommended)

## Quick Start

### 1. Configure Backend (Recommended)

Create an S3 bucket and DynamoDB table for state management:

```bash
# Create S3 bucket
aws s3 mb s3://apex-terraform-state --region us-west-2

# Enable versioning
aws s3api put-bucket-versioning \
  --bucket apex-terraform-state \
  --versioning-configuration Status=Enabled

# Create DynamoDB table for locking
aws dynamodb create-table \
  --table-name terraform-state-lock \
  --attribute-definitions AttributeName=LockID,AttributeType=S \
  --key-schema AttributeName=LockID,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST \
  --region us-west-2
```

### 2. Deploy Development Environment

```bash
cd environments/dev

# Uncomment and configure the backend block in main.tf

# Initialize Terraform
terraform init

# Review the plan
terraform plan

# Apply the configuration
terraform apply
```

### 3. Deploy Production Environment

```bash
cd environments/prod

# Uncomment and configure the backend block in main.tf
# IMPORTANT: Set the alarm_email variable for notifications

# Initialize Terraform
terraform init

# Review the plan
terraform plan

# Apply the configuration
terraform apply
```

### 4. Connect to EKS Cluster

```bash
# Get the kubectl command from outputs
terraform output eks_kubectl_config

# Run the command (example)
aws eks update-kubeconfig --region us-west-2 --name apex-dev

# Verify connection
kubectl get nodes
```

## Environment Differences

| Feature | Development | Production |
|---------|-------------|------------|
| Availability Zones | 2 | 3 |
| NAT Gateways | 1 (shared) | 3 (per AZ) |
| EKS Node Instance | t3.medium | t3.large/xlarge |
| EKS Node Count | 1-4 | 2-10 |
| RDS Instance | db.t3.small | db.r6g.large |
| RDS Multi-AZ | No | Yes |
| RDS Storage | 20-50 GB | 100-500 GB |
| RDS Backups | 3 days | 30 days |
| ElastiCache Instance | cache.t3.micro | cache.r6g.large |
| ElastiCache Nodes | 1 | 2 |
| ElastiCache Multi-AZ | No | Yes |
| Log Retention | 14 days | 90 days |
| WAF | No | Yes |
| KMS Encryption | Default | Custom keys |

## Configuration Variables

### Required Variables

| Variable | Description |
|----------|-------------|
| `project_name` | Name of the project (default: apex) |
| `environment` | Environment name (dev, staging, prod) |
| `aws_region` | AWS region for deployment |

### Optional Variables

See `variables.tf` for a complete list of configurable options.

## Outputs

| Output | Description |
|--------|-------------|
| `vpc_id` | ID of the VPC |
| `eks_cluster_name` | Name of the EKS cluster |
| `eks_cluster_endpoint` | Endpoint for EKS API server |
| `eks_kubectl_config` | Command to configure kubectl |
| `rds_endpoint` | RDS instance endpoint |
| `rds_password_secret_arn` | ARN of the password secret |
| `elasticache_endpoint` | ElastiCache primary endpoint |
| `database_connection_string` | PostgreSQL connection string template |
| `redis_connection_string` | Redis connection string |
| `cloudwatch_dashboard_url` | URL to CloudWatch dashboard |

## Security Considerations

### Production Environment
- RDS deletion protection enabled
- Multi-AZ deployments for high availability
- WAF with AWS managed rules
- Custom KMS keys for encryption
- VPC Flow Logs enabled
- Enhanced monitoring enabled

### Secrets Management
- RDS passwords stored in AWS Secrets Manager
- Use IRSA for pod-level AWS access
- Avoid hardcoding credentials in application code

### Network Security
- Private subnets for workloads
- Security groups with least privilege
- No public access to databases

## Cost Estimation

### Development Environment (Monthly)
- EKS Cluster: ~$73
- EKS Nodes (2x t3.medium): ~$60
- NAT Gateway (1): ~$45
- RDS (db.t3.small): ~$25
- ElastiCache (cache.t3.micro): ~$12
- **Estimated Total: ~$215/month**

### Production Environment (Monthly)
- EKS Cluster: ~$73
- EKS Nodes (3x t3.large): ~$180
- NAT Gateways (3): ~$135
- RDS (db.r6g.large Multi-AZ): ~$350
- ElastiCache (2x cache.r6g.large): ~$220
- WAF: ~$10
- **Estimated Total: ~$968/month**

*Note: Costs vary by region and actual usage.*

## Maintenance

### Updating Kubernetes Version
```bash
# Update eks_cluster_version in your environment
# Then apply changes
terraform apply
```

### Scaling Node Groups
```bash
# Modify eks_node_desired_size, eks_node_min_size, eks_node_max_size
terraform apply
```

### Rotating RDS Password
```bash
# Generate new password in Secrets Manager
aws secretsmanager rotate-secret --secret-id apex-dev-rds-password
```

## Troubleshooting

### EKS Cluster Access Issues
```bash
# Verify AWS identity
aws sts get-caller-identity

# Update kubeconfig
aws eks update-kubeconfig --region us-west-2 --name apex-dev
```

### Terraform State Lock Issues
```bash
# Force unlock (use with caution)
terraform force-unlock LOCK_ID
```

### Node Group Scaling Issues
- Check CloudWatch logs for EKS cluster
- Verify IAM permissions for autoscaling
- Check for resource quotas in AWS

## Contributing

1. Create a feature branch
2. Make changes
3. Run `terraform fmt` and `terraform validate`
4. Submit a pull request

## License

This infrastructure code is proprietary to the Apex project.
