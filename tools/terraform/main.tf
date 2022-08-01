terraform {
  required_providers {
    kubernetes = {
      source = "hashicorp/kubernetes"
      version = ">= 2.11.0"
    }
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "3.6.0"
    }
  }
}

data "azurerm_kubernetes_cluster" "default" {
  depends_on          = [module.aks-cluster] # refresh cluster state before reading
  name                = var.cluster_name
  resource_group_name = var.cluster_name
}

provider "kubernetes" {
  host                   = data.azurerm_kubernetes_cluster.default.kube_config.0.host
  client_certificate     = base64decode(data.azurerm_kubernetes_cluster.default.kube_config.0.client_certificate)
  client_key             = base64decode(data.azurerm_kubernetes_cluster.default.kube_config.0.client_key)
  cluster_ca_certificate = base64decode(data.azurerm_kubernetes_cluster.default.kube_config.0.cluster_ca_certificate)
}

provider "azurerm" {
  features {}
}

module "aks-cluster" {
  source       = "./aks-cluster"
  cluster_name = var.cluster_name
  location     = var.location
  node_count   = var.node_count
  vm_size      = var.vm_size
}

module "kubernetes-config" {
  depends_on                 = [module.aks-cluster]
  source                     = "./kubernetes-config"
  cluster_name               = var.cluster_name
  tce_node_image             = var.tce_node_image
  cert_spammer_image         = var.cert_spammer_image
  docker_registry            = var.docker_registry
  docker_registry_auth_user  = var.docker_registry_auth_user
  docker_registry_auth_pwd   = var.docker_registry_auth_pwd
  jaeger_endpoint            = var.jaeger_endpoint
  kubeconfig                 = data.azurerm_kubernetes_cluster.default.kube_config_raw
  replica_count              = var.replica_count
}
