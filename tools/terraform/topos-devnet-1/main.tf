variable "name" {}
variable "region" {}
variable "cidr_block" {}
variable "min_num_nodes" {}
variable "max_num_nodes" {}

# Recipe to create a k8s cluster in 2 AZs
# access to the internet provided by 2 public subnets

locals {
  subnets = cidrsubnets("${var.cidr_block}", "2", "2", "2", "2")
}

data "aws_availability_zones" "available" {
  state = "available"
}

provider "aws" {
  region = var.region
}

module "eks" {
  source = "git::https://github.com/toposware/infrastructure-as-code.git//terraform/modules/aws/eks"

  name   = var.name
  region = var.region

  cidr_block         = var.cidr_block
  private_subnets    = slice(local.subnets, 0, 2)
  public_subnets     = slice(local.subnets, 2, 4)
  availability_zones = data.aws_availability_zones.available

  min_num_nodes = var.min_num_nodes
  max_num_nodes = var.max_num_nodes

  create_ebs          = false
}

output "cluster_name" {
  value = module.eks.cluster_name
}

output "eks_efs_csi_irsa_arn" {
  value = module.eks.eks_efs_csi_irsa_arn
}

output "eks_external_dns_irsa_arn" {
  value = module.eks.eks_external_dns_irsa_arn
}

output "eks_efs_id" {
  value = module.eks.eks_efs_id
}
