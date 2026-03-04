
// Original enum: VRageMath.Base6Directions+Direction
#[::proto_rs::proto_message]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, ::serde::Serialize, ::serde::Deserialize)]
#[serde(rename = "Direction")]
pub enum Direction {
    #[default]
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
}

// Original enum: VRageMath.Base6Directions+DirectionFlags
#[::enumflags2::bitflags]
#[repr(u8)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
#[serde(rename = "DirectionFlags")]
pub enum DirectionFlags {
    #[default]
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
}

// Original enum: VRageMath.Base6Directions+Axis
#[::proto_rs::proto_message]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
#[serde(rename = "Axis")]
pub enum Axis {
    #[default]
    ForwardBackward,
    LeftRight,
    UpDown,
}