variable "name" {}
variable "region" {}
variable "cidr_block" {}
variable "instance_type" {}
variable "min_num_nodes" {}
variable "max_num_nodes" {}

# Recipe to create a k8s cluster in 2 AZs
# access to the internet provided by 2 public subnets

provider "aws" {
  region = var.region
}

module "network" {
  source = "git::https://github.com/toposware/infrastructure-as-code.git//terraform/modules/aws/network?ref=v0.1.1-rc.5"

  name       = var.name
  cidr_block = var.cidr_block
  subnet_num = 4

}

module "eks" {
  source = "git::https://github.com/toposware/infrastructure-as-code.git//terraform/modules/aws/eks?ref=v0.1.1-rc.5"

  name = var.name

  vpc_id             = module.network.vpc_id
  subnet_public_ids  = module.network.subnet_public_ids
  subnet_private_ids = module.network.subnet_private_ids
  instance_type      = var.instance_type
  min_num_nodes      = var.min_num_nodes
  max_num_nodes      = var.max_num_nodes

  create_ebs = false

  depends_on = [
    module.network
  ]
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
