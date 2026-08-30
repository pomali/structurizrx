workspace "Order Landscape" {
    model {
        customer = person "Customer"
        !include customers.dsl
        !include orders.dsl
        customer -> ordersApi "places orders via"
        ordersApi -> customersApi "looks up customer" { kind sync }
    }
    views {
        auto
    }
}
