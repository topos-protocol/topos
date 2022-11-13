terraform {
  required_providers {
    kubernetes = {
      source = "hashicorp/kubernetes"
      version = ">= 2.11.0"
    }
  }
}

resource "kubernetes_namespace" "namespace" {
  metadata {
    name = "test"
  }
}

resource "kubernetes_secret" "secret" {
  metadata {
    name = "docker-cfg"
    namespace = kubernetes_namespace.namespace.metadata.0.name
  }

  data = {
    ".dockerconfigjson" = jsonencode({
      auths = {
        "${var.docker_registry}" = {
          "username" = var.docker_registry_auth_user
          "password" = var.docker_registry_auth_pwd
          "auth"     = base64encode("${var.docker_registry_auth_user}:${var.docker_registry_auth_pwd}")
        }
      }
    })
  }

  type = "kubernetes.io/dockerconfigjson"
}

resource "kubernetes_deployment" "bootnode" {
  metadata {
    name = "bootnode"
    namespace= kubernetes_namespace.namespace.metadata.0.name
  }
  spec {
    replicas = 1

    selector {
      match_labels = {
        app = "TCE-Bootnode"
      }
    }
    template {
      metadata {
        labels = {
          app  = "TCE-Bootnode"
        }
      }
      spec {
        image_pull_secrets {
          name = kubernetes_secret.secret.metadata.0.name
        }

        container {
          image = var.tce_node_image
          image_pull_policy = "Always"
          name  = "tce"

          port {
            name = "p2p"
            container_port = "9090"
          }

          env {
            name  = "TCE_LOCAL_KS"
            value = 1 # 12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X
          }

          env {
            name  = "TCE_JAEGER_AGENT"
            value = var.jaeger_endpoint
          }

          env {
            name  = "TCE_JAEGER_SERVICE_NAME"
            value = "${var.jaeger_service_name}-boot"
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "bootnode" {
  metadata {
    name = "tce-boot"
    namespace = "${kubernetes_namespace.namespace.metadata.0.name}"
  }
  spec {
    selector = {
      app = kubernetes_deployment.bootnode.spec.0.template.0.metadata.0.labels.app
    }

    session_affinity = "ClientIP"

    port {
      port        = 8080
      target_port = "p2p"
    }
  }
}

resource "kubernetes_deployment" "replicas" {
  metadata {
    name = "tce-replicas"
    namespace= kubernetes_namespace.namespace.metadata.0.name
  }
  spec {
    replicas = var.replica_count

    selector {
      match_labels = {
        app = "TCE-Replicas"
      }
    }
    template {
      metadata {
        labels = {
          app  = "TCE-Replicas"
        }
      }
      spec {

        image_pull_secrets {
          name = kubernetes_secret.secret.metadata.0.name
        }

        container {
          image = var.tce_node_image
          image_pull_policy = "Always"
          name  = "tce"

          port {
            name = "p2p"
            container_port = "9090"
          }

          port {
            name = "api"
            container_port = "1340"
            protocol = "TCP"
          }

          env {
            name  = "RUST_LOG"
            value = "debug"
          }

          env {
            name  = "TCE_API_ADDR"
            value = "0.0.0.0:1340"
          }

          env {
            name  = "TCE_JAEGER_AGENT"
            value = var.jaeger_endpoint
          }

          env {
            name  = "TCE_JAEGER_SERVICE_NAME"
            value = "${var.jaeger_service_name}-replicas"
          }

          env {
            name  = "TCE_BOOT_PEERS"
            value = "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /dns4/${kubernetes_service.bootnode.metadata.0.name}/tcp/${kubernetes_service.bootnode.spec.0.port.0.port}"
          }

          liveness_probe {
            initial_delay_seconds = 10
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "replicas" {
  metadata {
    name = "replicas"
    namespace = "${kubernetes_namespace.namespace.metadata.0.name}"
  }
  spec {
    selector = {
      app = kubernetes_deployment.replicas.spec.0.template.0.metadata.0.labels.app
    }

    session_affinity = "ClientIP"

    port {
      port        = 8080
      target_port = "api"
    }
  }
}

resource "kubernetes_deployment" "cert_spammer" {
  depends_on = [
    kubernetes_deployment.replicas
  ]
  metadata {
    name = "cert-spammer"
    namespace= kubernetes_namespace.namespace.metadata.0.name
  }
  spec {
    replicas = 1

    selector {
      match_labels = {
        app = "Cert-Spammer"
      }
    }
    template {
      metadata {
        labels = {
          app  = "Cert-Spammer"
        }
      }
      spec {

        image_pull_secrets {
          name = kubernetes_secret.secret.metadata.0.name
        }

        container {
          image = var.cert_spammer_image
          image_pull_policy = "Always"
          name  = "cert-spammer"

          env {
            name  = "TARGET_NODE"
            value = "http://${kubernetes_service.replicas.metadata.0.name}:${kubernetes_service.replicas.spec.0.port.0.port}"
          }
          
          env {
            name  = "RUST_LOG"
            value = "info"
          }
        }
      }
    }
  }
}

resource "local_file" "kubeconfig" {
  content = var.kubeconfig
  filename = "${path.root}/kubeconfig"
}
