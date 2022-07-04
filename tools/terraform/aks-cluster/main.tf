terraform {
  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "=3.6.0"
    }
  }
}

provider "azurerm" {
  features {}
}

resource "azurerm_resource_group" "resource_group" {
  name     = var.cluster_name
  location = var.location
}

resource "azurerm_virtual_network" "vnet" {
  name                = var.cluster_name
  address_space       = ["10.10.0.0/16"]
  location            = azurerm_resource_group.resource_group.location
  resource_group_name = azurerm_resource_group.resource_group.name
}

resource "azurerm_subnet" "nodepool" {
  name                 = "default"
  virtual_network_name = var.cluster_name
  resource_group_name  = azurerm_resource_group.resource_group.name
  address_prefixes     = ["10.10.1.0/24"]
}

resource "azurerm_subnet" "aci" {
  name                 = "aci"
  virtual_network_name = var.cluster_name
  resource_group_name  = azurerm_resource_group.resource_group.name
  address_prefixes     = ["10.10.3.0/24"]

  delegation {
    name = "aciDelegation"
    service_delegation {
      name    = "Microsoft.ContainerInstance/containerGroups"
      actions = ["Microsoft.Network/virtualNetworks/subnets/action"]
    }
  }
}

resource "azurerm_kubernetes_cluster" "cluster" {
  name                = var.cluster_name
  location            = azurerm_resource_group.resource_group.location
  resource_group_name = azurerm_resource_group.resource_group.name
  dns_prefix          = var.cluster_name

  default_node_pool {
    name       = "default"
    node_count = var.node_count
    vm_size    = var.vm_size
    vnet_subnet_id = azurerm_subnet.nodepool.id
  }

  identity {
    type = "SystemAssigned"
  }

  network_profile {
    network_plugin    = "azure"
    network_policy    = "azure"
    load_balancer_sku = "standard"
  }

  aci_connector_linux {
    subnet_name = azurerm_subnet.aci.name
  }
}

resource "azurerm_role_assignment" "role_assignment" {
  scope                = azurerm_subnet.aci.id
  role_definition_name = "Network Contributor"
  principal_id         = azurerm_kubernetes_cluster.cluster.identity.0.principal_id
}