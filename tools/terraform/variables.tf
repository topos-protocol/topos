variable "cluster_name" {
  type = string 
}

// TCE node docker image
variable "tce_node_image" {
  type = string
}

// Cert spammer docker image
variable "cert_spammer_image" {
  type = string
}

// TCE process docker image registry
variable "docker_registry" {
  type    = string
  default = "ghcr.io"
}

// TCE process docker image registry user
variable "docker_registry_auth_user" {
  type = string
}
// TCE process docker image registry pwd
variable "docker_registry_auth_pwd" {
  type = string
}

// Jaeger agent endpoint
variable "jaeger_endpoint" {
  type = string
}

// Jaeger service name
variable "jaeger_service_name" {
  type = string
  default = "undefined"
}

variable "location" {
  type    = string
  default = "eastus"
}

// Number of k8s nodes
variable "node_count" {
  type    = number
  default = 3 // Max is 1,000
  # default = 100 // Max is 1,000
}

// Number of TCE processes
variable "replica_count" {
  type    = number
  default = 10 // Max is 250,000 (250 per k8s node)
  # default = 1000 // Max is 250,000 (250 per k8s node)
}

variable "vm_size" {
  type    = string
  default = "Standard_D2_v2"
}
