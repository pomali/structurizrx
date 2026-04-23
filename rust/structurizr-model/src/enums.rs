use serde::{Deserialize, Serialize};

/// Shape of an element.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum Shape {
    #[default]
    Box,
    Circle,
    Component,
    Cylinder,
    Ellipse,
    Hexagon,
    Person,
    Pipe,
    Robot,
    RoundedBox,
    WebBrowser,
    Window,
    MobileDeviceLandscape,
    MobileDevicePortrait,
    Folder,
    Diamond,
}

/// Border style.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum Border {
    #[default]
    Dashed,
    Solid,
    Dotted,
}

/// Line style for relationships.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum LineStyle {
    #[default]
    Dashed,
    Solid,
    Dotted,
}

/// Routing style.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum Routing {
    #[default]
    Direct,
    Curved,
    Orthogonal,
}

/// Rank direction for automatic layout.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum RankDirection {
    #[default]
    TopBottom,
    BottomTop,
    LeftRight,
    RightLeft,
}

/// Interaction style for a relationship.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum InteractionStyle {
    #[default]
    Synchronous,
    Asynchronous,
}

/// Location of a person or software system.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum Location {
    #[default]
    Unspecified,
    Internal,
    External,
}

/// Paper size for views.
#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum PaperSize {
    #[default]
    A0_Landscape,
    A0_Portrait,
    A1_Landscape,
    A1_Portrait,
    A2_Landscape,
    A2_Portrait,
    A3_Landscape,
    A3_Portrait,
    A4_Landscape,
    A4_Portrait,
    A5_Landscape,
    A5_Portrait,
    Letter_Landscape,
    Letter_Portrait,
    Legal_Landscape,
    Legal_Portrait,
    Slide_4_3,
    Slide_16_9,
    Slide_16_10,
}

/// Filter mode for filtered views.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum FilterMode {
    #[default]
    Include,
    Exclude,
}

/// Sort order for views.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum ViewSortOrder {
    #[default]
    Default,
    Type,
    Key,
}
