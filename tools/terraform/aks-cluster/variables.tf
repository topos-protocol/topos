variable "kubernetes_version" {
  default = "1.18"
}

variable "cluster_name" {
  type = string
}

variable "location" {
  type = string
}

variable "node_count" {
  type = number
}

variable "vm_size" {
  type = string
}
