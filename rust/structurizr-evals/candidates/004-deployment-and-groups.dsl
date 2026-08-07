workspace "Shop Ops" {
    model {
        customer = person "Customer"
        shop = softwareSystem "Shop" {
            group "Core" {
                web = container "Web App" "Storefront" "TypeScript"
                api = container "API" "Handles requests" "Rust"
            }
            group "Data" {
                db = container "Database" "Stores data" "PostgreSQL" { tags "Database" }
            }
            web -> api "calls"
            api -> db "reads and writes" { kind sync }
        }
        customer -> web "shops on"

        production = deploymentEnvironment "Production" {
            deploymentNode "Amazon Web Services" {
                lb = infrastructureNode "Load Balancer"
                deploymentNode "EC2" {
                    apiInstance = containerInstance api
                }
                deploymentNode "RDS" {
                    dbInstance = containerInstance db
                }
            }
            lb -> apiInstance "routes to"
            apiInstance -> dbInstance "reads and writes"
        }
    }
    views {
        auto
        deployment shop "Production" {
            include *
        }
    }
}
