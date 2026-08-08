workspace "Shop Deployment" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            web = container "Web App" "Storefront" "TypeScript"
            api = container "API" "Handles requests" "Rust"
            db = container "Database" "Stores data" "PostgreSQL" { tags "Database" }
            web -> api "calls"
            api -> db "reads and writes" { kind sync }
        }
        customer -> web "shops on"

        production = deploymentEnvironment "Production" {
            deploymentNode "Amazon Web Services" {
                tags "Amazon Web Services - Cloud"

                deploymentNode "Autoscaling group" {
                    deploymentNode "EC2 instance" {
                        webInstance = containerInstance web
                        apiInstance = containerInstance api
                    }
                }

                lb = infrastructureNode "Load Balancer" {
                    tags "Amazon Web Services - Elastic Load Balancing"
                }

                deploymentNode "RDS" {
                    dbInstance = containerInstance db
                }
            }

            lb -> webInstance "routes to" "HTTPS"
            apiInstance -> dbInstance "reads and writes" "SQL/TCP"
        }
    }
    views {
        auto
        deployment shop "Production" {
            include *
            autoLayout
        }
    }
}
