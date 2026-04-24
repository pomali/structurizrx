workspace "Apple Banana Engine" "Generic workspace description" {

    !identifiers hierarchical

    model {
        person_apple = person "Apple Team" "Generic role description"
        person_banana = person "Banana Architect" "Generic role description"
        person_cherry = person "Cherry Manager" "Generic role description"
        person_date = person "Date Partner" "Generic role description"
        person_grape = person "Grape Person" "Generic role description"
        person_lemon = person "Lemon Group" "Generic role description"

        system_mango = softwareSystem "Banana Network" "Generic external system" {
            tags "External"
        }

        system_orange = softwareSystem "Apple Host App" "Generic external system" {
            tags "External"
        }

        system_peach = softwareSystem "Apple Platform" "Generic internal platform" {
            node_apple = container "Apple Studio" "Type A" "Generic container description"
            node_banana = container "Banana Space" "Type B" "Generic container description"

            node_cherry = container "Apple Module" "Type C" "Generic container description"
            node_date = container "Banana Module" "Type C" "Generic container description"
            node_fig = container "Cherry Module" "Type C" "Generic container description"

            node_grape = container "Apple Bridge" "Type D" "Generic container description"
            node_kiwi = container "Banana Bridge" "Type D" "Generic container description"
            node_lemon = container "Cherry Bridge" "Type D" "Generic container description"

            node_mango = container "Apple Unit" "Type A" "Generic container description"
            node_nectarine = container "Banana Unit" "Type A" "Generic container description"
            node_orange = container "Cherry Unit" "Type A" "Generic container description"
            node_papaya = container "Date Unit" "Type A" "Generic container description"

            node_quince = container "Shared Module" "Type A" "Generic container description"
        }

        person_apple -> system_orange "Apple link"
        person_apple -> system_peach.node_cherry "Banana link"
        person_apple -> system_peach.node_date "Cherry link"
        person_apple -> system_peach.node_fig "Date link"
        person_apple -> system_peach.node_apple "Fig link"

        person_banana -> system_peach.node_apple "Grape link"
        person_banana -> system_peach.node_banana "Kiwi link"

        person_cherry -> system_peach "Lemon link"
        person_date -> system_peach "Mango link"
        person_grape -> system_peach.node_banana "Orange link"
        person_lemon -> system_peach "Peach link"

        person_apple -> system_peach "Pear link"
        person_banana -> system_peach "Plum link"
        person_grape -> system_peach "Berry link"

        person_cherry -> system_mango "Apple path"
        person_grape -> system_mango "Banana path"

        system_orange -> system_peach.node_cherry "Cherry path"
        system_orange -> system_peach.node_date "Date path"
        system_orange -> system_peach.node_fig "Fig path"

        system_peach.node_cherry -> system_peach.node_grape "Grape path"
        system_peach.node_date -> system_peach.node_kiwi "Kiwi path"
        system_peach.node_fig -> system_peach.node_lemon "Lemon path"

        system_peach.node_grape -> system_peach.node_mango "Mango path"
        system_peach.node_kiwi -> system_peach.node_nectarine "Orange path"
        system_peach.node_lemon -> system_peach.node_orange "Peach path"

        system_peach.node_apple -> system_peach.node_quince "Pear path"

        system_peach.node_mango -> system_peach.node_quince "Plum path"
        system_peach.node_nectarine -> system_peach.node_quince "Berry path"
        system_peach.node_orange -> system_peach.node_quince "Apple route"
        system_peach.node_papaya -> system_peach.node_quince "Banana route"
    }

    views {
        systemContext system_peach "SystemContext" {
            include person_apple
            include person_banana
            include person_cherry
            include person_date
            include person_grape
            include person_lemon
            include system_mango
            include system_orange
            include system_peach
            autolayout lr
        }

        systemContext system_peach "BusinessView" {
            include person_apple
            include person_banana
            include person_cherry
            include person_date
            include person_grape
            include person_lemon
            include system_mango
            include system_peach
            autolayout lr
        }

        container system_peach "Containers" {
            include *
        }

        container system_peach "PlatformZoom" {
            include person_apple
            include person_banana
            include person_cherry
            include person_date
            include person_grape
            include person_lemon
            include system_mango
            include system_orange
            include system_peach.node_apple
            include system_peach.node_banana
            autolayout lr
        }

        styles {
            element "Element" {
                background "#0b132b"
                color "#f8f9fa"
                stroke "#5bc0be"
                strokeWidth 3
                shape roundedbox
            }
            element "Person" {
                shape person
                background "#ffd166"
                color "#0b132b"
            }
            element "Software System" {
                background "#1c2541"
            }
            element "Container" {
                background "#3a506b"
            }
            element "External" {
                background #adb5bd
                color #0b132b
                stroke #495057
            }
            element "Boundary" {
                strokeWidth 2
            }
            relationship "Relationship" {
                thickness 3
                color #5bc0be
            }
        }
    }

    configuration {
        scope softwaresystem
    }

}