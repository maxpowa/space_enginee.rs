// Original enum: VRageMath.Base6Directions+Direction
#[::proto_rs::proto_message]
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::deku::DekuRead,
    ::deku::DekuWrite,
)]
#[deku(id_type = "u8", bits = 3)]
#[serde(rename = "Direction")]
pub enum Direction {
    #[default]
    #[deku(id = 0)]
    Forward,
    #[deku(id = 1)]
    Backward,
    #[deku(id = 2)]
    Left,
    #[deku(id = 3)]
    Right,
    #[deku(id = 4)]
    Up,
    #[deku(id = 5)]
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
