workspace "Catalog" {
    model {
        customer = person "Customer"
        !include catalog/customers.dsl
        !include catalog/orders.dsl
        customer -> ordersApi "places orders via"
        ordersApi -> customersApi "looks up customer" { kind sync }
    }
    views {
        auto
    }
}
