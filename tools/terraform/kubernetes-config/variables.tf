variable "cluster_name" {
  type = string
}

variable "tce_node_image" {
  type = string
}

variable "cert_spammer_image" {
  type = string
}

variable "docker_registry" {
  type = string
}

variable "docker_registry_auth_user" {
  type = string
}

variable "docker_registry_auth_pwd" {
  type = string
}

variable "jaeger_endpoint" {
  type = string
}

variable "jaeger_service_name" {
  type = string
}

variable "kubeconfig" {
  type = string
}

variable "replica_count" {
  type = number
}
