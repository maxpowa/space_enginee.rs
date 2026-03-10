use deku::prelude::*;
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};

// Original type: VRage.SerializableVector2
#[::proto_rs::proto_message]
#[derive(
    Default,
    Debug,
    Clone,
    PartialEq,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::deku::DekuRead,
    ::deku::DekuWrite,
)]
pub struct SerializableVector2F {
    #[proto(tag = 1)]
    #[serde(rename = "@x", alias = "@X")]
    pub x: f32,
    #[proto(tag = 4)]
    #[serde(rename = "@y", alias = "@Y")]
    pub y: f32,
}
impl SerializableVector2F {
    pub fn new(x: f32, y: f32) -> Self {
        SerializableVector2F { x, y }
    }
}
impl From<SerializableVector2F> for Vec2 {
    fn from(value: SerializableVector2F) -> Self {
        Vec2::new(value.x, value.y)
    }
}
impl From<Vec2> for SerializableVector2F {
    fn from(value: Vec2) -> Self {
        SerializableVector2F::new(value.x, value.y)
    }
}

// Original type: VRageMath.Vector2
#[::proto_rs::proto_message]
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::deku::DekuRead,
    ::deku::DekuWrite,
)]
pub struct Vector2F {
    #[proto(tag = 1)]
    #[serde(rename = "X", alias = "x")]
    pub x: f32,
    #[proto(tag = 4)]
    #[serde(rename = "Y", alias = "y")]
    pub y: f32,
}
impl Vector2F {
    pub fn new(x: f32, y: f32) -> Self {
        Vector2F { x, y }
    }
}
impl From<Vector2F> for Vec2 {
    fn from(value: Vector2F) -> Self {
        Vec2::new(value.x, value.y)
    }
}
impl From<Vec2> for Vector2F {
    fn from(value: Vec2) -> Self {
        Vector2F::new(value.x, value.y)
    }
}

// Original type: VRage.SerializableVector3D
#[::proto_rs::proto_message]
#[derive(Debug, Default, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
pub struct SerializableVector3D {
    #[proto(tag = 1)]
    #[serde(rename = "@x", alias = "@X")]
    pub x: f64,
    #[proto(tag = 4)]
    #[serde(rename = "@y", alias = "@Y")]
    pub y: f64,
    #[proto(tag = 7)]
    #[serde(rename = "@z", alias = "@Z")]
    pub z: f64,
}
impl SerializableVector3D {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        SerializableVector3D { x, y, z }
    }
}
impl From<SerializableVector3D> for Vec3 {
    fn from(value: SerializableVector3D) -> Self {
        Vec3::new(value.x as f32, value.y as f32, value.z as f32)
    }
}
impl From<Vec3> for SerializableVector3D {
    fn from(value: Vec3) -> Self {
        SerializableVector3D::new(value.x as f64, value.y as f64, value.z as f64)
    }
}

// Original type: VRageMath.Vector3D
#[::proto_rs::proto_message]
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::deku::DekuRead,
    ::deku::DekuWrite,
)]
pub struct Vector3D {
    #[proto(tag = 1)]
    #[serde(rename = "X", alias = "x")]
    #[deku(
        reader = "Vector3D::read_value(deku::reader)",
        writer = "Vector3D::write_value(deku::writer, self.x)"
    )]
    pub x: f64,
    #[proto(tag = 4)]
    #[serde(rename = "Y", alias = "y")]
    #[deku(
        reader = "Vector3D::read_value(deku::reader)",
        writer = "Vector3D::write_value(deku::writer, self.y)"
    )]
    pub y: f64,
    #[proto(tag = 7)]
    #[serde(rename = "Z", alias = "z")]
    #[deku(
        reader = "Vector3D::read_value(deku::reader)",
        writer = "Vector3D::write_value(deku::writer, self.z)"
    )]
    pub z: f64,
}
impl Vector3D {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Vector3D { x, y, z }
    }

    fn read_value<R: std::io::Read + std::io::Seek>(
        reader: &mut deku::reader::Reader<R>,
    ) -> Result<f64, DekuError> {
        Ok(u64::from_reader_with_ctx(reader, ())? as f64)
    }

    fn write_value<W: std::io::Write + std::io::Seek>(
        writer: &mut Writer<W>,
        input: f64,
    ) -> Result<(), DekuError> {
        (input as u64).to_writer(writer, ())
    }
}
impl std::hash::Hash for Vector3D {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.x.to_bits());
        state.write_u64(self.y.to_bits());
        state.write_u64(self.z.to_bits());
    }
}
impl std::cmp::Eq for Vector3D {}
impl From<Vector3D> for Vec3 {
    fn from(value: Vector3D) -> Self {
        Vec3::new(value.x as f32, value.y as f32, value.z as f32)
    }
}
impl From<Vec3> for Vector3D {
    fn from(value: Vec3) -> Self {
        Vector3D::new(value.x as f64, value.y as f64, value.z as f64)
    }
}

// Original type: VRage.SerializableVector3F
#[::proto_rs::proto_message]
#[derive(Debug, Default, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
pub struct SerializableVector3F {
    #[proto(tag = 1)]
    #[serde(rename = "@x", alias = "@X")]
    pub x: f32,
    #[proto(tag = 4)]
    #[serde(rename = "@y", alias = "@Y")]
    pub y: f32,
    #[proto(tag = 7)]
    #[serde(rename = "@z", alias = "@Z")]
    pub z: f32,
}
impl SerializableVector3F {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        SerializableVector3F { x, y, z }
    }
}
impl From<SerializableVector3F> for Vec3 {
    fn from(value: SerializableVector3F) -> Self {
        Vec3::new(value.x, value.y, value.z)
    }
}
impl From<Vec3> for SerializableVector3F {
    fn from(value: Vec3) -> Self {
        SerializableVector3F::new(value.x, value.y, value.z)
    }
}

// Original type: VRageMath.Vector3F
#[::proto_rs::proto_message]
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::deku::DekuRead,
    ::deku::DekuWrite,
)]
pub struct Vector3F {
    #[proto(tag = 1)]
    #[serde(rename = "X", alias = "x")]
    pub x: f32,
    #[proto(tag = 4)]
    #[serde(rename = "Y", alias = "y")]
    pub y: f32,
    #[proto(tag = 7)]
    #[serde(rename = "Z", alias = "z")]
    pub z: f32,
}
impl Vector3F {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vector3F { x, y, z }
    }
}
impl From<Vector3F> for Vec3 {
    fn from(value: Vector3F) -> Self {
        Vec3::new(value.x, value.y, value.z)
    }
}
impl From<Vec3> for Vector3F {
    fn from(value: Vec3) -> Self {
        Vector3F::new(value.x, value.y, value.z)
    }
}

// Original type: VRageMath.SerializableVector3I
#[::proto_rs::proto_message]
#[derive(Debug, Default, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
pub struct SerializableVector3I {
    #[proto(tag = 1)]
    #[serde(rename = "@x", alias = "@X")]
    pub x: i32,
    #[proto(tag = 4)]
    #[serde(rename = "@y", alias = "@Y")]
    pub y: i32,
    #[proto(tag = 7)]
    #[serde(rename = "@z", alias = "@Z")]
    pub z: i32,
}
impl SerializableVector3I {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        SerializableVector3I { x, y, z }
    }
}
impl From<SerializableVector3I> for Vec3 {
    fn from(value: SerializableVector3I) -> Self {
        Vec3::new(value.x as f32, value.y as f32, value.z as f32)
    }
}
impl From<Vec3> for SerializableVector3I {
    fn from(value: Vec3) -> Self {
        SerializableVector3I::new(value.x as i32, value.y as i32, value.z as i32)
    }
}

// Original type: VRageMath.Vector3I
#[::proto_rs::proto_message]
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::deku::DekuRead,
    ::deku::DekuWrite,
)]
pub struct Vector3I {
    #[proto(tag = 1)]
    #[serde(rename = "X", alias = "x")]
    pub x: i32,
    #[proto(tag = 4)]
    #[serde(rename = "Y", alias = "y")]
    pub y: i32,
    #[proto(tag = 7)]
    #[serde(rename = "Z", alias = "z")]
    pub z: i32,
}
impl Vector3I {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Vector3I { x, y, z }
    }
}
impl From<Vector3I> for Vec3 {
    fn from(value: Vector3I) -> Self {
        Vec3::new(value.x as f32, value.y as f32, value.z as f32)
    }
}
impl From<Vec3> for Vector3I {
    fn from(value: Vec3) -> Self {
        Vector3I::new(value.x as i32, value.y as i32, value.z as i32)
    }
}

// Original type: VRageMath.Quaternion
#[::proto_rs::proto_message]
#[derive(Default, Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
pub struct Quaternion {
    #[proto(tag = 1)]
    #[serde(rename = "X")]
    pub x: f32,
    #[proto(tag = 4)]
    #[serde(rename = "Y")]
    pub y: f32,
    #[proto(tag = 7)]
    #[serde(rename = "Z")]
    pub z: f32,
    #[proto(tag = 10)]
    #[serde(rename = "W")]
    pub w: f32,
}
impl Quaternion {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Quaternion { x, y, z, w }
    }
}
impl From<Quaternion> for Quat {
    fn from(value: Quaternion) -> Self {
        Quat::from_xyzw(value.x, value.y, value.z, value.w)
    }
}
impl From<Quat> for Quaternion {
    fn from(value: Quat) -> Self {
        Quaternion::new(value.x, value.y, value.z, value.w)
    }
}

// Original type: VRageMath.Matrix3x3
#[::proto_rs::proto_message]
#[derive(
    Debug,
    Clone,
    PartialEq,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::deku::DekuRead,
    ::deku::DekuWrite,
)]
pub struct Matrix3x3 {
    #[proto(tag = 1)]
    #[serde(rename = "M11")]
    pub m11: f32,
    #[proto(tag = 4)]
    #[serde(rename = "M12")]
    pub m12: f32,
    #[proto(tag = 7)]
    #[serde(rename = "M13")]
    pub m13: f32,
    #[proto(tag = 10)]
    #[serde(rename = "M21")]
    pub m21: f32,
    #[proto(tag = 13)]
    #[serde(rename = "M22")]
    pub m22: f32,
    #[proto(tag = 16)]
    #[serde(rename = "M23")]
    pub m23: f32,
    #[proto(tag = 19)]
    #[serde(rename = "M31")]
    pub m31: f32,
    #[proto(tag = 22)]
    #[serde(rename = "M32")]
    pub m32: f32,
    #[proto(tag = 25)]
    #[serde(rename = "M33")]
    pub m33: f32,
}
#[rustfmt::skip]
impl Matrix3x3 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        m11: f32, m12: f32, m13: f32,
        m21: f32, m22: f32, m23: f32,
        m31: f32, m32: f32, m33: f32,
    ) -> Self {
        Matrix3x3 {
            m11, m12, m13,
            m21, m22, m23,
            m31, m32, m33,
        }
    }
}
#[rustfmt::skip]
impl From<Matrix3x3> for Mat3 {
    fn from(value: Matrix3x3) -> Self {
        Mat3::from_cols_array(&[
            value.m11, value.m21, value.m31,
            value.m12, value.m22, value.m32,
            value.m13, value.m23, value.m33,
        ])
    }
}
#[rustfmt::skip]
impl From<Mat3> for Matrix3x3 {
    fn from(value: Mat3) -> Self {
        let cols = value.to_cols_array();
        Matrix3x3::new(
            cols[0], cols[3], cols[6], //
            cols[1], cols[4], cols[7], //
            cols[2], cols[5], cols[8], //
        )
    }
}

// Original type: VRageMath.MatrixD
#[::proto_rs::proto_message]
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::deku::DekuRead,
    ::deku::DekuWrite,
)]
pub struct MatrixD {
    #[proto(tag = 1)]
    #[serde(rename = "M11")]
    pub m11: f64,
    #[proto(tag = 4)]
    #[serde(rename = "M12")]
    pub m12: f64,
    #[proto(tag = 7)]
    #[serde(rename = "M13")]
    pub m13: f64,
    #[proto(tag = 10)]
    #[serde(rename = "M14")]
    pub m14: f64,
    #[proto(tag = 13)]
    #[serde(rename = "M21")]
    pub m21: f64,
    #[proto(tag = 16)]
    #[serde(rename = "M22")]
    pub m22: f64,
    #[proto(tag = 19)]
    #[serde(rename = "M23")]
    pub m23: f64,
    #[proto(tag = 22)]
    #[serde(rename = "M24")]
    pub m24: f64,
    #[proto(tag = 25)]
    #[serde(rename = "M31")]
    pub m31: f64,
    #[proto(tag = 28)]
    #[serde(rename = "M32")]
    pub m32: f64,
    #[proto(tag = 31)]
    #[serde(rename = "M33")]
    pub m33: f64,
    #[proto(tag = 34)]
    #[serde(rename = "M34")]
    pub m34: f64,
    #[proto(tag = 37)]
    #[serde(rename = "M41")]
    pub m41: f64,
    #[proto(tag = 40)]
    #[serde(rename = "M42")]
    pub m42: f64,
    #[proto(tag = 43)]
    #[serde(rename = "M43")]
    pub m43: f64,
    #[proto(tag = 46)]
    #[serde(rename = "M44")]
    pub m44: f64,
}
#[rustfmt::skip]
impl MatrixD {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        m11: f64, m12: f64, m13: f64, m14: f64,
        m21: f64, m22: f64, m23: f64, m24: f64,
        m31: f64, m32: f64, m33: f64, m34: f64,
        m41: f64, m42: f64, m43: f64, m44: f64,
    ) -> Self {
        MatrixD {
            m11, m12, m13, m14,
            m21, m22, m23, m24,
            m31, m32, m33, m34,
            m41, m42, m43, m44,
        }
    }
}
#[rustfmt::skip]
impl From<MatrixD> for Mat4 {
    fn from(value: MatrixD) -> Self {
        Mat4::from_cols_array(&[
            value.m11 as f32, value.m21 as f32, value.m31 as f32, value.m41 as f32,
            value.m12 as f32, value.m22 as f32, value.m32 as f32, value.m42 as f32,
            value.m13 as f32, value.m23 as f32, value.m33 as f32, value.m43 as f32,
            value.m14 as f32, value.m24 as f32, value.m34 as f32, value.m44 as f32,
        ])
    }
}
#[rustfmt::skip]
impl From<Mat4> for MatrixD {
    fn from(value: Mat4) -> Self {
        let cols = value.to_cols_array();
        MatrixD::new(
            cols[0] as f64, cols[4] as f64, cols[8] as f64, cols[12] as f64,
            cols[1] as f64, cols[5] as f64, cols[9] as f64, cols[13] as f64,
            cols[2] as f64, cols[6] as f64, cols[10] as f64, cols[14] as f64,
            cols[3] as f64, cols[7] as f64, cols[11] as f64, cols[15] as f64,
        )
    }
}

// Original type: VRage.SerializableBoundingBoxD
#[::proto_rs::proto_message]
#[derive(Debug, Default, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
pub struct SerializableBoundingBoxD {
    #[proto(tag = 1)]
    #[serde(rename = "Min")]
    pub min: crate::math::SerializableVector3D,
    #[proto(tag = 4)]
    #[serde(rename = "Max")]
    pub max: crate::math::SerializableVector3D,
}
impl SerializableBoundingBoxD {
    pub fn new(
        min: crate::math::SerializableVector3D,
        max: crate::math::SerializableVector3D,
    ) -> Self {
        SerializableBoundingBoxD { min, max }
    }
}

// Original type: VRageMath.BoundingBoxD
#[::proto_rs::proto_message]
#[derive(Default, Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize, Eq, Hash)]
pub struct BoundingBoxD {
    #[proto(tag = 1)]
    #[serde(rename = "Min")]
    pub min: crate::math::Vector3D,
    #[proto(tag = 4)]
    #[serde(rename = "Max")]
    pub max: crate::math::Vector3D,
}
impl BoundingBoxD {
    pub fn new(min: crate::math::Vector3D, max: crate::math::Vector3D) -> Self {
        BoundingBoxD { min, max }
    }
}
