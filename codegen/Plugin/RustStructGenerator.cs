using System.Collections.Concurrent;
using System.ComponentModel;
using System.Reflection;
using System.Text;
using System.Xml.Serialization;
using ProtoBuf;
using VRage;
using VRage.Collections;
using VRage.ObjectBuilder;
using VRage.Serialization;
using VRageMath;

namespace StandaloneExtractor.Plugin;

/// <summary>
/// Generates Rust structs from C# types for Space Engineers serialization.
/// 
/// ## Three Serialization Systems
/// 
/// Space Engineers uses three independent serialization systems, each with different
/// rules for which members are included. Our codegen maps these to Rust attributes:
/// 
/// ### 1. XML Serialization (serde + quick-xml)
/// Used for: Save files, world data, definitions
/// 
/// | C# Attribute       | Rust Attribute                    | Behavior                    |
/// |--------------------|-----------------------------------|-----------------------------|
/// | [XmlIgnore]        | #[serde(skip)]                    | Exclude from XML            |
/// | [XmlElement("X")]  | #[serde(rename = "X")]            | Element with custom name    |
/// | [XmlAttribute("X")]| #[serde(rename = "@X")]           | XML attribute (@ prefix)    |
/// | (default)          | All public properties             | Included by default         |
/// 
/// ### 2. Protobuf Serialization (proto_rs)
/// Used for: Client-server communication, some save data
/// 
/// | C# Attribute       | Rust Attribute                    | Behavior                    |
/// |--------------------|-----------------------------------|-----------------------------|
/// | [ProtoMember(N)]   | #[proto(tag = N)]                 | Include with tag N          |
/// | (no ProtoMember)   | #[proto(skip)]                    | Exclude from protobuf       |
/// | [ProtoContract]    | #[proto_rs::proto_message]        | Type-level marker           |
/// 
/// ### 3. Network/Binary Serialization (Deku)
/// Used for: Real-time network replication events (RPC), state synchronization
/// 
/// | C# Attribute/Rule           | Rust Attribute      | Behavior                        |
/// |-----------------------------|---------------------|---------------------------------|
/// | [NoSerialize]               | #[deku(skip)]       | Exclude from network            |
/// | [Serialize]                 | (force include)     | Include even if private setter  |
/// | private setter              | #[deku(skip)]       | Exclude (not network-public)    |
/// | Type not Deku-compatible    | #[deku(skip)]       | Can't serialize this type       |
/// 
/// The network serializer (MySerializerObject) uses different rules than XML/Protobuf:
/// - Only members with public getter AND public setter are included
/// - [NoSerialize] excludes, [Serialize] forces inclusion
/// - See: VRage\VRage\Serialization\MySerializerObject.cs
/// - See: VRage.Library\System\Reflection\MemberAccess.cs (IsMemberPublic)
/// 
/// ## Example: SerializableDefinitionId
/// 
/// ```csharp
/// // Field - excluded from ALL serialization
/// [XmlIgnore]
/// [NoSerialize]
/// public MyObjectBuilderType TypeId;
/// 
/// // XML/Protobuf only - NOT sent over network
/// [ProtoMember(1)]
/// [XmlAttribute("Type")]
/// [NoSerialize]  // &lt;-- NOT sent over network
/// public string TypeIdStringAttribute { get; set; }
/// 
/// // Network only - private with [Serialize] override
/// [Serialize]  // &lt;-- Forces inclusion in network despite being private
/// private ushort m_binaryTypeId { get; set; }
/// ```
/// 
/// ## Generated Rust Mapping
/// 
/// The same field may have different skip attributes for each system:
/// ```rust
/// #[proto(skip)]           // No ProtoMember tag
/// #[serde(skip)]           // Has [XmlIgnore]
/// #[deku(skip)]            // Has [NoSerialize] or type not Deku-compatible
/// pub type_id: MyObjectBuilderType,
/// ```
/// </summary>
public class RustStructGenerator
{
    private static string StringToSnakeCase(string str)
    {
        if (string.IsNullOrEmpty(str)) return str;
        if (str.ToUpper() == str) return str;
        var sb = new StringBuilder();
        var isFirst = true;
        for (var i = 0; i < str.Length; i++)
        {
            var c = str[i];
            if (isFirst)
            {
                sb.Append(char.ToLower(c));
                isFirst = false;
                continue;
            }

            var isCurrentUpper = char.IsUpper(c);
            var isPrevUpper = char.IsUpper(str[i - 1]);
            var isPrevUnderscore = str[i - 1] == '_';
            var isNextUpper = (i + 1 < str.Length) && char.IsUpper(str[i + 1]);
            // Only insert underscore if:
            // - current is uppercase
            // - previous is not uppercase (to avoid FOO -> f_o_o)
            // - previous is not underscore
            // - next is not uppercase (to handle FooGPS -> foo_gps)
            if (isCurrentUpper && (!isPrevUpper || !isNextUpper) && !isPrevUnderscore)
            {
                sb.Append('_');
            }

            sb.Append(char.ToLower(c));
        }

        var result = sb.ToString();
        
        // Collapse multiple underscores into one
        while (result.Contains("__"))
            result = result.Replace("__", "_");
        
        // Remove leading underscore if any
        result = result.TrimStart('_');
        
        return result;
    }

    private static bool IsTypeNullable(Type type) =>
        type.IsGenericType && type.GetGenericTypeDefinition() == typeof(Nullable<>);

    private static bool IsTypeHashMap(Type type) => type.IsGenericType &&
                                                    (type.GetGenericTypeDefinition() == typeof(Dictionary<,>) ||
                                                     type.GetGenericTypeDefinition() == typeof(ConcurrentDictionary<,>) ||
                                                     type.GetGenericTypeDefinition() ==
                                                     typeof(DictionaryReader<,>));

    private static bool IsTypeArray(Type type) => type.IsArray || type.IsGenericType &&
        (type.GetGenericTypeDefinition() == typeof(List<>) ||
         type.GetGenericTypeDefinition() ==
         typeof(HashSet<>) ||
         type.GetGenericTypeDefinition() == typeof(MyConcurrentHashSet<>) ||
         type.GetGenericTypeDefinition() == typeof(ListReader<>) ||
         type.GetGenericTypeDefinition() == typeof(HashSetReader<>) ||
         type.GetGenericTypeDefinition() == typeof(ICollection<>) ||
         type.GetGenericTypeDefinition() == typeof(MySerializableList<>));

    private static string GenericTypeName(Type type)
    {
        var structName = (RustKeywords.Contains(type.Name.Split('`')[0])
            ? $"r#{type.Name.Split('`')[0]}"
            : type.Name.Split('`')[0]);
        var genericArguments = string.Join(",", type.GenericTypeArguments.Select(RecursiveTypeName));
        // if (structName == "SerializableDictionary")
        // {
        //     // return $"{structName}{genericArguments.Replace(",", "").Replace("<", "").Replace(">", "").Replace(":", "")}";
        // }
        if (structName == "SerializableDictionary")
        {
            return $"crate::compat::VarMap<{genericArguments}>";
        }
        if (structName == "MyTuple")
        {
            return $"crate::compat::Tuple<{genericArguments}>";
        }
        if (structName == "MySerializableList")
        {
            return $"Vec<{genericArguments}>";
        }
        return $"{structName}<{genericArguments}>";
    }
    
    private static string QualifiedRustName(Type type)
    {
        var name = type.DeclaringType != null ? $"{type.DeclaringType.Name}_{type.Name}" : type.Name;
        // Strip backtick+arity from generic names
        if (name.Contains('`')) name = name.Split('`')[0];
        return RustKeywords.Contains(name) ? $"r#{name}" : name;
    }

    private static string RecursiveTypeName(Type type)
    {
        return type switch
        {
            _ when type == typeof(byte) => "i32",
            _ when type == typeof(sbyte) => "u32",
            _ when type == typeof(short) => "i32",
            _ when type == typeof(ushort) => "u32",
            _ when type == typeof(int) => "i32",
            _ when type == typeof(uint) => "u32",
            _ when type == typeof(long) => "i64",
            _ when type == typeof(ulong) => "u64",
            _ when type == typeof(float) => "f32",
            _ when type == typeof(double) => "f64",
            _ when type == typeof(bool) => "bool",
            _ when type == typeof(string) => "String",
            _ when type == typeof(object) => "Vec<u8>",
            _ when type == typeof(DateTime) => "crate::compat::DateTime",
            _ when type == typeof(TimeSpan) => "crate::compat::TimeSpan",
            _ when type == typeof(Guid) => "crate::compat::Guid",
            _ when type == typeof(decimal) => "crate::compat::Decimal",
            _ when type == typeof(Vector2) => "crate::math::Vector2F",
            _ when type == typeof(SerializableVector2) => "crate::math::SerializableVector2F",
            _ when type == typeof(Vector3D) => "crate::math::Vector3D",
            _ when type == typeof(SerializableVector3D) => "crate::math::SerializableVector3D",
            _ when type == typeof(Vector3) => "crate::math::Vector3F",
            _ when type == typeof(SerializableVector3) => "crate::math::SerializableVector3F",
            _ when type == typeof(Vector3I) => "crate::math::Vector3I",
            _ when type == typeof(SerializableVector3I) => "crate::math::SerializableVector3I",
            _ when type == typeof(Quaternion) => "crate::math::Quaternion",
            _ when type == typeof(Matrix3x3) => "crate::math::Matrix3x3",
            _ when type == typeof(MatrixD) => "crate::math::MatrixD",
            _ when type == typeof(SerializableBoundingBoxD) => "crate::math::SerializableBoundingBoxD",
            _ when type == typeof(BoundingBoxD) => "crate::math::BoundingBoxD",
            _ when type == typeof(Base6Directions.Direction) => "crate::compat::direction::Direction",
            _ when IsTypeNullable(type) =>
                type.GenericTypeArguments[0].IsEnum && type.GenericTypeArguments[0].GetCustomAttributes(typeof(FlagsAttribute), true).Length > 0
                    ? $"crate::compat::Nullable<crate::compat::BitField<{RecursiveTypeName(type.GenericTypeArguments[0])}>>"
                    : $"crate::compat::Nullable<{RecursiveTypeName(type.GenericTypeArguments[0])}>",
            _ when IsTypeHashMap(type) =>
                $"::std::collections::HashMap<{RecursiveTypeName(type.GenericTypeArguments[0])}, {RecursiveTypeName(type.GenericTypeArguments[1])}>",
            _ when IsTypeArray(type) =>
                $"Vec<{RecursiveTypeName(type.GetElementType() ?? type.GenericTypeArguments[0])}>",
            _ when type.IsEnum => QualifiedRustName(type),
            _ when type.IsGenericType => GenericTypeName(type),
            _ => QualifiedRustName(type)
        };
    }

    /// <summary>
    /// Returns the Rust type name with Deku wrapper types for bit-aligned serialization.
    /// Primitives are wrapped in BitAligned&lt;T&gt;, strings in VarString, byte arrays in VarBytes.
    /// </summary>
    private static string DekuTypeName(Type type)
    {
        return type switch
        {
            // Primitives wrapped in BitAligned<T> for bit-aligned reads
            _ when type == typeof(byte) => "crate::compat::BitAligned<i32>",
            _ when type == typeof(sbyte) => "crate::compat::BitAligned<u32>",
            _ when type == typeof(short) => "crate::compat::BitAligned<i32>",
            _ when type == typeof(ushort) => "crate::compat::BitAligned<u32>",
            _ when type == typeof(int) => "crate::compat::BitAligned<i32>",
            _ when type == typeof(uint) => "crate::compat::BitAligned<u32>",
            _ when type == typeof(long) => "crate::compat::BitAligned<i64>",
            _ when type == typeof(ulong) => "crate::compat::BitAligned<u64>",
            _ when type == typeof(float) => "crate::compat::BitAligned<f32>",
            _ when type == typeof(double) => "crate::compat::BitAligned<f64>",
            _ when type == typeof(bool) => "crate::compat::BitBool",
            _ when type == typeof(string) => "crate::compat::VarString",
            _ when type == typeof(object) => "crate::compat::VarBytes",
            // Other types use their regular names (they have Deku derives themselves)
            _ when type == typeof(DateTime) => "crate::compat::DateTime",
            _ when type == typeof(TimeSpan) => "crate::compat::TimeSpan",
            _ when type == typeof(Guid) => "crate::compat::Guid",
            _ when type == typeof(decimal) => "crate::compat::Decimal",
            _ when type == typeof(Vector2) => "crate::math::Vector2F",
            _ when type == typeof(SerializableVector2) => "crate::math::SerializableVector2F",
            _ when type == typeof(Vector3D) => "crate::math::Vector3D",
            _ when type == typeof(SerializableVector3D) => "crate::math::SerializableVector3D",
            _ when type == typeof(Vector3) => "crate::math::Vector3F",
            _ when type == typeof(SerializableVector3) => "crate::math::SerializableVector3F",
            _ when type == typeof(Vector3I) => "crate::math::Vector3I",
            _ when type == typeof(SerializableVector3I) => "crate::math::SerializableVector3I",
            _ when type == typeof(Quaternion) => "crate::math::Quaternion",
            _ when type == typeof(Matrix3x3) => "crate::math::Matrix3x3",
            _ when type == typeof(MatrixD) => "crate::math::MatrixD",
            _ when type == typeof(SerializableBoundingBoxD) => "crate::math::SerializableBoundingBoxD",
            _ when type == typeof(BoundingBoxD) => "crate::math::BoundingBoxD",
            _ when type == typeof(Base6Directions.Direction) => "crate::compat::direction::Direction",
            _ when IsTypeNullable(type) =>
                type.GenericTypeArguments[0].IsEnum && type.GenericTypeArguments[0].GetCustomAttributes(typeof(FlagsAttribute), true).Length > 0
                    ? $"crate::compat::Nullable<crate::compat::BitField<{DekuTypeName(type.GenericTypeArguments[0])}>>"
                    : $"crate::compat::Nullable<{DekuTypeName(type.GenericTypeArguments[0])}>",
            _ when IsTypeHashMap(type) =>
                $"::std::collections::HashMap<{DekuTypeName(type.GenericTypeArguments[0])}, {DekuTypeName(type.GenericTypeArguments[1])}>",
            _ when IsTypeArray(type) =>
                $"crate::compat::VarVec<{DekuTypeName(type.GetElementType() ?? type.GenericTypeArguments[0])}>",
            _ when type.IsEnum => QualifiedRustName(type),
            _ when type.IsGenericType => GenericTypeName(type),
            _ => QualifiedRustName(type)
        };
    }
    
    /// <summary>
    /// Returns the Rust type name for use in crate-transport (events.rs).
    /// Assumes common types are imported at the top of the file.
    /// Complex types are prefixed with space_engineers_sys::.
    /// </summary>
    public static string DekuTypeNameForTransport(Type type)
    {
        return type switch
        {
            // Primitives - use wrapper types (imported at top of file)
            _ when type == typeof(byte) => "BitAligned<u8>",
            _ when type == typeof(sbyte) => "BitAligned<i8>",
            _ when type == typeof(short) => "BitAligned<i16>",
            _ when type == typeof(ushort) => "BitAligned<u16>",
            _ when type == typeof(int) => "BitAligned<i32>",
            _ when type == typeof(uint) => "BitAligned<u32>",
            _ when type == typeof(long) => "BitAligned<i64>",
            _ when type == typeof(ulong) => "BitAligned<u64>",
            _ when type == typeof(float) => "BitAligned<f32>",
            _ when type == typeof(double) => "BitAligned<f64>",
            _ when type == typeof(bool) => "BitBool",
            _ when type == typeof(string) => "VarString",
            _ when type == typeof(byte[]) => "VarBytes",
            _ when type == typeof(object) => "VarBytes",
            // BCL types from compat
            _ when type == typeof(DateTime) => "space_engineers_compat::DateTime",
            _ when type == typeof(TimeSpan) => "space_engineers_compat::TimeSpan",
            _ when type == typeof(Guid) => "space_engineers_compat::Guid",
            _ when type == typeof(decimal) => "space_engineers_compat::Decimal",
            // Math types (imported at top)
            _ when type == typeof(Vector3D) => "Vector3D",
            _ when type == typeof(Vector3) => "Vector3F",
            _ when type == typeof(Vector2) => "space_engineers_sys::math::Vector2F",
            _ when type == typeof(SerializableVector3D) => "space_engineers_sys::math::SerializableVector3D",
            _ when type == typeof(SerializableVector3) => "space_engineers_sys::math::SerializableVector3F",
            _ when type == typeof(Vector3I) => "space_engineers_sys::math::Vector3I",
            _ when type == typeof(SerializableVector3I) => "space_engineers_sys::math::SerializableVector3I",
            _ when type == typeof(Quaternion) => "space_engineers_sys::math::Quaternion",
            _ when type == typeof(Matrix3x3) => "space_engineers_sys::math::Matrix3x3",
            _ when type == typeof(MatrixD) => "space_engineers_sys::math::MatrixD",
            _ when type == typeof(BoundingBoxD) => "space_engineers_sys::math::BoundingBoxD",
            _ when type == typeof(SerializableBoundingBoxD) => "space_engineers_sys::math::SerializableBoundingBoxD",
            // Direction enum (nested type in Base6Directions)
            _ when type == typeof(Base6Directions.Direction) => "space_engineers_compat::direction::Direction",
            // Generics
            _ when IsTypeNullable(type) =>
                $"Nullable<{DekuTypeNameForTransport(type.GenericTypeArguments[0])}>",
            // VRage.Collections generics - use compat wrappers (VarMap is Deku-compatible)
            _ when type.IsGenericType && type.GetGenericTypeDefinition().Name.StartsWith("SerializableDictionary") =>
                $"VarMap<{DekuTypeNameForTransport(type.GenericTypeArguments[0])}, {DekuTypeNameForTransport(type.GenericTypeArguments[1])}>",
            _ when type.IsGenericType && type.GetGenericTypeDefinition().Name.StartsWith("MyTuple") =>
                $"space_engineers_compat::Tuple<{string.Join(", ", type.GenericTypeArguments.Select(DekuTypeNameForTransport))}>",
            // HashMaps - use VarMap if key/value types are Deku-compatible
            _ when IsTypeHashMap(type) && IsDekuCompatible(type.GenericTypeArguments[0], []) && IsDekuCompatible(type.GenericTypeArguments[1], []) =>
                $"VarMap<{DekuTypeNameForTransport(type.GenericTypeArguments[0])}, {DekuTypeNameForTransport(type.GenericTypeArguments[1])}>",
            // HashMaps with non-Deku-compatible key/value - use placeholder
            _ when IsTypeHashMap(type) =>
                $"/* {type.FullName} - HashMap not Deku-compatible */ VarBytes",
            // Arrays/Lists - VarVec with length prefix
            _ when IsTypeArray(type) =>
                $"VarVec<{DekuTypeNameForTransport(type.GetElementType() ?? type.GenericTypeArguments[0])}>",
            // Flags enums - wrap with BitField (same as RustStructGenerator does for fields)
            _ when type.IsEnum && type.GetCustomAttributes(typeof(FlagsAttribute), true).Length > 0 =>
                $"space_engineers_compat::BitField<space_engineers_sys::types::{QualifiedRustName(type)}>",
            // Non-flags enums in types module
            _ when type.IsEnum =>
                $"space_engineers_sys::types::{QualifiedRustName(type)}",
            // Complex types - check if Deku compatible
            _ when !IsDekuCompatible(type, []) =>
                $"/* {type.FullName} - not Deku-compatible */ VarBytes",
            // Complex types (structs/classes) in types module
            _ => $"space_engineers_sys::types::{QualifiedRustName(type)}"
        };
    }

    private class ExtraSerializationInfo
    {
        public MemberInfo Member;

        public ProtoMemberAttribute[] ProtoMemberAttributes =>
            (ProtoMemberAttribute[])Member.GetCustomAttributes(typeof(ProtoMemberAttribute), true);

        public XmlAttributeAttribute[] XmlAttributeAttributes =>
            (XmlAttributeAttribute[])Member.GetCustomAttributes(typeof(XmlAttributeAttribute), true);
        
        public bool IsProtoMember => ProtoMemberAttributes.Length > 0;
        public bool IsProtoRequired => IsProtoMember && ProtoMemberAttributes.Any(m => m.IsRequired);
        public int ProtoTag => IsProtoMember ? ProtoMemberAttributes.Last().Tag : int.MinValue;
        public bool IsXmlAttribute => XmlAttributeAttributes.Length > 0;

        public bool IsXmlIgnore => Member.GetCustomAttributes(typeof(XmlIgnoreAttribute), true).Length > 0;

        public bool NoSerialize =>
            Member.GetCustomAttributes(typeof(NoSerializeAttribute), true).Length > 0;

        /// <summary>
        /// Returns the element name from [XmlArrayItem("...")] if present, otherwise null.
        /// </summary>
        public string? XmlArrayItemName
        {
            get
            {
                var attr = Member.GetCustomAttributes(typeof(XmlArrayItemAttribute), true)
                    .OfType<XmlArrayItemAttribute>()
                    .FirstOrDefault();
                return attr?.ElementName;
            }
        }

        /// <summary>
        /// The CLR type of the member (field or property).
        /// </summary>
        public Type MemberType => Member switch
        {
            FieldInfo f => f.FieldType,
            PropertyInfo p => p.PropertyType,
            _ => typeof(object)
        };

        /// <summary>
        /// Returns the value from [DefaultValue(...)] attribute if present, otherwise null.
        /// </summary>
        public object? DefaultValueAttributeValue
        {
            get
            {
                var attr = Member.GetCustomAttributes(typeof(DefaultValueAttribute), true)
                    .OfType<DefaultValueAttribute>()
                    .FirstOrDefault();
                return attr?.Value;
            }
        }

        // Cache for default instances per type
        private static readonly ConcurrentDictionary<Type, object?> _defaultInstances = new();

        /// <summary>
        /// Returns the field initializer value by creating a default instance of the declaring type.
        /// Returns null if it cannot be determined or if the member is not a field.
        /// </summary>
        public object? FieldInitializerValue
        {
            get
            {
                if (Member is not FieldInfo fieldInfo) return null;
                var declaringType = Member.DeclaringType;
                if (declaringType == null) return null;

                try
                {
                    var instance = _defaultInstances.GetOrAdd(declaringType, t =>
                    {
                        try
                        {
                            return Activator.CreateInstance(t);
                        }
                        catch
                        {
                            return null;
                        }
                    });
                    
                    if (instance == null) return null;
                    return fieldInfo.GetValue(instance);
                }
                catch
                {
                    return null;
                }
            }
        }

        /// <summary>
        /// Returns the effective numeric default value, checking [DefaultValue] first, then field initializer.
        /// </summary>
        public object? EffectiveNumericDefaultValue
        {
            get
            {
                // First check [DefaultValue] attribute
                var attrValue = DefaultValueAttributeValue;
                if (attrValue != null) return attrValue;
                
                // Then check field initializer for numeric types
                if (!IsNumericType) return null;
                return FieldInitializerValue;
            }
        }

        /// <summary>
        /// True if the member type is a supported numeric type.
        /// </summary>
        private bool IsNumericType =>
            MemberType == typeof(byte) || MemberType == typeof(sbyte) ||
            MemberType == typeof(short) || MemberType == typeof(ushort) ||
            MemberType == typeof(int) || MemberType == typeof(uint) ||
            MemberType == typeof(long) || MemberType == typeof(ulong) ||
            MemberType == typeof(float) || MemberType == typeof(double);

        /// <summary>
        /// True when the field has a numeric default value (via [DefaultValue] or field initializer) that is non-zero.
        /// </summary>
        public bool HasNumericDefaultValue
        {
            get
            {
                var defaultVal = EffectiveNumericDefaultValue;
                if (defaultVal == null) return false;
                // Check if it's a supported numeric type
                return IsNumericType;
            }
        }

        /// <summary>
        /// True when the numeric default value is zero (can use serde's built-in default).
        /// </summary>
        public bool IsNumericDefaultZero
        {
            get
            {
                if (!HasNumericDefaultValue) return false;
                var defaultVal = EffectiveNumericDefaultValue!;
                return MemberType == typeof(byte) && Convert.ToInt32(defaultVal) == 0 ||
                       MemberType == typeof(sbyte) && Convert.ToInt32(defaultVal) == 0 ||
                       MemberType == typeof(short) && Convert.ToInt32(defaultVal) == 0 ||
                       MemberType == typeof(ushort) && Convert.ToInt32(defaultVal) == 0 ||
                       MemberType == typeof(int) && Convert.ToInt32(defaultVal) == 0 ||
                       MemberType == typeof(uint) && Convert.ToUInt32(defaultVal) == 0 ||
                       MemberType == typeof(long) && Convert.ToInt64(defaultVal) == 0 ||
                       MemberType == typeof(ulong) && Convert.ToUInt64(defaultVal) == 0 ||
                       MemberType == typeof(float) && Convert.ToSingle(defaultVal) == 0f ||
                       MemberType == typeof(double) && Convert.ToDouble(defaultVal) == 0d;
            }
        }

        /// <summary>
        /// Returns the Rust literal for the numeric default value.
        /// </summary>
        public string? NumericDefaultRustLiteral
        {
            get
            {
                if (!HasNumericDefaultValue) return null;
                var defaultVal = EffectiveNumericDefaultValue!;
                return MemberType switch
                {
                    _ when MemberType == typeof(byte) => Convert.ToInt32(defaultVal).ToString(),
                    _ when MemberType == typeof(sbyte) => Convert.ToInt32(defaultVal).ToString(),
                    _ when MemberType == typeof(short) => Convert.ToInt32(defaultVal).ToString(),
                    _ when MemberType == typeof(ushort) => Convert.ToInt32(defaultVal).ToString(),
                    _ when MemberType == typeof(int) => Convert.ToInt32(defaultVal).ToString(),
                    _ when MemberType == typeof(uint) => Convert.ToUInt32(defaultVal).ToString(),
                    _ when MemberType == typeof(long) => Convert.ToInt64(defaultVal).ToString(),
                    _ when MemberType == typeof(ulong) => Convert.ToUInt64(defaultVal).ToString(),
                    _ when MemberType == typeof(float) => Convert.ToSingle(defaultVal).ToString("G", System.Globalization.CultureInfo.InvariantCulture) + "f32",
                    _ when MemberType == typeof(double) => Convert.ToDouble(defaultVal).ToString("G", System.Globalization.CultureInfo.InvariantCulture) + "f64",
                    _ => null
                };
            }
        }

        /// <summary>
        /// True when the field has a natural default value in Rust and C#, so a
        /// missing XML element should produce that default instead of a
        /// deserialization error.  Covers collections (Vec, HashMap,
        /// SerializableDictionary), booleans (default <c>false</c>),
        /// strings (default <c>""</c>), numeric types (all have Default in Rust),
        /// nullable types (C# T? maps to Nullable<T> which has Default),
        /// flags enums with zero default (BitField<T> has Default),
        /// non-flags enums (all derive Default in Rust),
        /// special value types (DateTime, TimeSpan, Guid),
        /// and struct types (all generated Rust structs derive Default).
        /// </summary>
        public bool HasSerdeDefault =>
            IsTypeArray(MemberType) || IsTypeHashMap(MemberType) ||
            (MemberType.IsGenericType && MemberType.GetGenericTypeDefinition() == typeof(SerializableDictionary<,>)) ||
            MemberType == typeof(bool) || MemberType == typeof(string) || IsNumericType ||
            IsTypeNullable(MemberType) || HasFlagsEnumZeroDefault || HasStructFieldInitializer ||
            IsSpecialValueType || IsNonFlagsEnum;
        
        /// <summary>
        /// True for DateTime, TimeSpan, Guid - C# value types that map to Rust types with Default.
        /// </summary>
        private bool IsSpecialValueType =>
            MemberType == typeof(DateTime) || MemberType == typeof(TimeSpan) || MemberType == typeof(Guid);
        
        /// <summary>
        /// True for non-flags enums. All generated Rust enums derive Default.
        /// </summary>
        private bool IsNonFlagsEnum =>
            MemberType.IsEnum && MemberType.GetCustomAttributes(typeof(FlagsAttribute), true).Length == 0;

        /// <summary>
        /// True when the field is a struct type (class in C#) and has a non-null field initializer.
        /// This means the Rust struct should use serde(default) since the struct derives Default.
        /// Since all generated Rust structs derive Default, we can use default for all class-type fields.
        /// </summary>
        private bool HasStructFieldInitializer
        {
            get
            {
                // Only check for class types (C# structs would be value types)
                if (!MemberType.IsClass || MemberType == typeof(string)) return false;
                // Skip arrays and generic types - they're handled elsewhere
                if (MemberType.IsArray || MemberType.IsGenericType) return false;
                // All generated Rust structs derive Default, so we can always use serde(default)
                return true;
            }
        }

        /// <summary>
        /// True when the field is a flags enum and has [DefaultValue] with value 0.
        /// </summary>
        private bool HasFlagsEnumZeroDefault
        {
            get
            {
                var attrValue = DefaultValueAttributeValue;
                if (attrValue == null) return false;
                var enumType = attrValue.GetType();
                if (!enumType.IsEnum) return false;
                if (enumType.GetCustomAttributes(typeof(FlagsAttribute), true).Length == 0) return false;
                // Check if the value is 0
                return Convert.ToInt64(attrValue) == 0;
            }
        }
        
        /// <summary>
        /// True when the field has a [DefaultValue] attribute with a non-flags enum value.
        /// Flags enums are handled differently (via HasFlagsEnumZeroDefault or not supported yet).
        /// </summary>
        public bool HasEnumDefaultValue
        {
            get
            {
                var attrValue = DefaultValueAttributeValue;
                if (attrValue == null) return false;
                var enumType = attrValue.GetType();
                if (!enumType.IsEnum) return false;
                // Skip flags enums - they're handled via HasFlagsEnumZeroDefault or not supported
                if (enumType.GetCustomAttributes(typeof(FlagsAttribute), true).Length > 0) return false;
                return true;
            }
        }

        /// <summary>
        /// Returns the Rust literal for the enum default value, e.g. "EnumType::Variant".
        /// Returns null if not an enum default or if it's a flags enum.
        /// </summary>
        public string? EnumDefaultRustLiteral
        {
            get
            {
                if (!HasEnumDefaultValue) return null;
                var attrValue = DefaultValueAttributeValue!;
                var enumType = attrValue.GetType();
                var variant = Enum.GetName(enumType, attrValue);
                return $"{QualifiedRustName(enumType)}::{variant}";
            }
        }
    }

    private class ExtraTypeInfo
    {
        public Type Type;

        public string Name => HasXmlRootName ? XmlRootName! : Type.Name;
        
        private string? XmlRootName => Type.GetCustomAttributes(typeof(XmlRootAttribute), true).FirstOrDefault() is XmlRootAttribute xmlRootAttr
            ? xmlRootAttr.ElementName
            : null;
        private bool HasXmlRootName => XmlRootName != null;

        public string SanitizedTypeName => RecursiveTypeName(Type);
        public string DekuSanitizedTypeName => DekuTypeName(Type);
        
        public bool IsEnumFlags() => IsEnumFlags(Type);

        private bool IsEnumFlags(Type type) => type.IsEnum && type.GetCustomAttributes(typeof(FlagsAttribute), true).Length > 0;

        public bool HasRustType => RecursiveTypeName(Type) != QualifiedRustName(Type);
        
        public bool IsArray => IsTypeArray(Type);
        public bool IsNullable => IsTypeNullable(Type);
        public bool IsHashMap => IsTypeHashMap(Type);
        public bool IsOptional => IsNullable;

        public string ProtobufType => TypeToProtobufType(Type);

        private string TypeToProtobufType(Type type, bool isNested = false)
        {
            return type switch
            {
                _ when type == typeof(byte) => "int32",
                _ when type == typeof(sbyte) => "uint32",
                _ when type == typeof(short) => "int32",
                _ when type == typeof(ushort) => "uint32",
                _ when type == typeof(int) => "int32",
                _ when type == typeof(uint) => "uint32",
                _ when type == typeof(long) => "int64",
                _ when type == typeof(ulong) => "uint64",
                _ when type == typeof(float) => "float",
                _ when type == typeof(double) => "double",
                _ when type == typeof(bool) => "bool",
                _ when type == typeof(string) => "string",
                _ when type == typeof(byte[]) => "bytes",
                _ when type == typeof(object) => "bytes",
                _ when IsTypeNullable(type) =>
                    $"{TypeToProtobufType(type.GetGenericArguments()[0], true)}, optional",
                _ when IsTypeArray(type) =>
                    $"{TypeToProtobufType(type.GetElementType() ?? type.GenericTypeArguments[0], true)}, repeated",
                _ when IsTypeHashMap(type) =>
                    $"hash_map = \"{TypeToProtobufType(type.GenericTypeArguments[0], true)}, {TypeToProtobufType(type.GenericTypeArguments[1], true)}\"",
                // Flag enums are represented as int32 in Protobuf
                _ when IsEnumFlags(type) => $"int32",
                // Regular enums are represented as enumerations in Protobuf
                _ when type.IsEnum && !isNested => $"enumeration = \"{SanitizedTypeName}\"",
                _ => "message" // Assume it's a message
            };
        }
    }

    private static readonly HashSet<string> RustKeywords =
    [
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
        "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
        "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract",
        "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield"
    ];

    static HashSet<Type> _processedTypes = [];
    /// Tracks XmlArrayItem wrapper type names already emitted, to avoid duplicates.
    static HashSet<string> _emittedXmlArrayItemWrappers = [];
    /// Types that need Deku derives (replication event argument types and their dependencies).
    static HashSet<Type> _dekuTypes = [];

    private static bool NeedsDekuDerives(Type type) => _dekuTypes.Contains(type);

    /// <summary>
    /// Checks if a type will have Deku support in the generated code.
    /// This differs from IsDekuCompatible in that it checks if the type WILL HAVE
    /// Deku derives (for complex types), not just if it's structurally compatible.
    /// </summary>
    private static bool WillHaveDekuSupport(Type type)
    {
        // Primitives are supported via BitAligned wrappers
        if (type.IsPrimitive) return true;
        
        // String and object are supported via VarString/VarBytes wrappers
        if (type == typeof(string) || type == typeof(object) || type == typeof(byte[]))
            return true;
        
        // DateTime, TimeSpan, Guid, decimal are supported via space_engineers_compat wrappers
        if (type == typeof(DateTime) || type == typeof(TimeSpan) || 
            type == typeof(Guid) || type == typeof(decimal))
            return true;
        
        // Nullable<T> - supported via Nullable wrapper if inner type is supported
        if (IsTypeNullable(type))
            return WillHaveDekuSupport(type.GenericTypeArguments[0]);
        
        // Arrays/Lists - supported via VarVec if element type is supported
        if (IsTypeArray(type))
        {
            var elementType = type.GetElementType() ?? type.GenericTypeArguments[0];
            return WillHaveDekuSupport(elementType);
        }
        
        // SerializableDictionary - supported via VarMap if key/value types are supported
        if (type.IsGenericType && type.GetGenericTypeDefinition() == typeof(SerializableDictionary<,>))
        {
            return WillHaveDekuSupport(type.GenericTypeArguments[0]) &&
                   WillHaveDekuSupport(type.GenericTypeArguments[1]);
        }
        
        // Raw Dictionary - supported via VarMap if key/value types are supported
        if (IsTypeHashMap(type))
        {
            return WillHaveDekuSupport(type.GenericTypeArguments[0]) &&
                   WillHaveDekuSupport(type.GenericTypeArguments[1]);
        }
        
        // Other generic types - not supported
        if (type.IsGenericType)
            return false;
        
        // All enums are supported (regular enums or flags via BitField wrapper)
        if (type.IsEnum)
            return true;
        
        // Math types from crate-compat have Deku support
        Type[] dekuCompatMathTypes = [
            typeof(VRageMath.Vector2),
            typeof(VRageMath.Vector3),
            typeof(VRageMath.Vector3D),
            typeof(VRageMath.Vector3I),
            typeof(VRageMath.Quaternion),
            typeof(VRageMath.Matrix3x3),
            typeof(VRageMath.MatrixD),
            typeof(VRageMath.BoundingBoxD),
            typeof(VRage.SerializableVector2),
            typeof(VRage.SerializableVector3),
            typeof(VRage.SerializableVector3D),
            typeof(VRage.SerializableVector3I),
            typeof(VRage.SerializableBoundingBoxD),
        ];
        if (dekuCompatMathTypes.Contains(type))
            return true;
        
        // For complex types (structs/classes), check if they're in _dekuTypes
        // (will have Deku derives in generated code)
        return NeedsDekuDerives(type);
    }

    /// <summary>
    /// Checks if a type can be serialized with Deku (only primitives, enums, and structs with Deku-compatible fields).
    /// </summary>
    private static bool IsDekuCompatible(Type type, HashSet<Type> checking)
    {
        // Prevent infinite recursion
        if (!checking.Add(type)) return true;
        
        try
        {
            // Primitives are Deku-compatible
            if (type.IsPrimitive) return true;
            
            // String and object are Deku-compatible via VarString/VarBytes wrappers
            if (type == typeof(string) || type == typeof(object))
                return true;
            
            // DateTime, TimeSpan, Guid, decimal are Deku-compatible via space_engineers_compat wrappers
            if (type == typeof(DateTime) || type == typeof(TimeSpan) || 
                type == typeof(Guid) || type == typeof(decimal))
                return true;
            
            // Nullable<T> - Deku-compatible via our crate::compat::Nullable
            if (IsTypeNullable(type))
                return IsDekuCompatible(type.GenericTypeArguments[0], checking);
            
            // Arrays/Lists/HashSets - Deku-compatible via length-prefixed Vec
            if (IsTypeArray(type))
            {
                var elementType = type.GetElementType() ?? type.GenericTypeArguments[0];
                return IsDekuCompatible(elementType, checking);
            }
            
            // SerializableDictionary is Deku-compatible via VarMap wrapper
            if (type.IsGenericType && type.GetGenericTypeDefinition() == typeof(SerializableDictionary<,>))
            {
                return IsDekuCompatible(type.GenericTypeArguments[0], checking) &&
                       IsDekuCompatible(type.GenericTypeArguments[1], checking);
            }
            
            // Raw HashMaps/Dictionaries are NOT Deku-compatible (need VarMap wrapper)
            if (IsTypeHashMap(type))
                return false;
            
            // Other generic types - not compatible
            if (type.IsGenericType)
                return false;
            
            // All enums are Deku-compatible (flags enums via BitField wrapper)
            if (type.IsEnum)
                return true;
            
            // Math types from crate-compat have Deku support
            Type[] dekuCompatMathTypes = [
                typeof(VRageMath.Vector2),
                typeof(VRageMath.Vector3),
                typeof(VRageMath.Vector3D),
                typeof(VRageMath.Vector3I),
                typeof(VRageMath.Quaternion),
                typeof(VRageMath.Matrix3x3),
                typeof(VRageMath.MatrixD),
                typeof(VRageMath.BoundingBoxD),
                typeof(VRage.SerializableVector2),
                typeof(VRage.SerializableVector3),
                typeof(VRage.SerializableVector3D),
                typeof(VRage.SerializableVector3I),
                typeof(VRage.SerializableBoundingBoxD),
            ];
            if (dekuCompatMathTypes.Contains(type))
                return true;
            
            // For structs/classes, check all fields and properties that would be network-serialized
            // Use GetNetworkSerializableMembers to match the game's serialization rules
            // (skips properties with private setters, [NoSerialize] attributes, etc.)
            var (fieldInfos, propertyInfos) = GetNetworkSerializableMembers(type);
            foreach (var field in fieldInfos)
            {
                if (!IsDekuCompatible(field.FieldType, checking))
                    return false;
            }
            foreach (var prop in propertyInfos)
            {
                if (!IsDekuCompatible(prop.PropertyType, checking))
                    return false;
            }
            
            return true;
        }
        finally
        {
            checking.Remove(type);
        }
    }

    /// <summary>
    /// Recursively collects all types that a given type depends on (fields, properties, generic arguments).
    /// </summary>
    private static void CollectDependentTypes(Type type, HashSet<Type> collected, HashSet<Type> visited)
    {
        if (!visited.Add(type)) return;
        
        // Skip primitives and built-in types that have known Rust mappings
        if (type.IsPrimitive || type == typeof(string) || type == typeof(object) ||
            type == typeof(DateTime) || type == typeof(TimeSpan) || type == typeof(Guid) ||
            type == typeof(decimal))
            return;
        
        // Handle nullable types
        if (IsTypeNullable(type))
        {
            CollectDependentTypes(type.GenericTypeArguments[0], collected, visited);
            return;
        }
        
        // Handle array/list types
        if (IsTypeArray(type))
        {
            var elemType = type.GetElementType() ?? type.GenericTypeArguments[0];
            CollectDependentTypes(elemType, collected, visited);
            return;
        }
        
        // Handle HashMap types
        if (IsTypeHashMap(type))
        {
            CollectDependentTypes(type.GenericTypeArguments[0], collected, visited);
            CollectDependentTypes(type.GenericTypeArguments[1], collected, visited);
            return;
        }
        
        // Handle other generic types
        if (type.IsGenericType)
        {
            foreach (var arg in type.GenericTypeArguments)
                CollectDependentTypes(arg, collected, visited);
            // Still add the type itself if it's a concrete type we'll generate
            var typeInfo = new ExtraTypeInfo { Type = type };
            if (!typeInfo.HasRustType)
                collected.Add(type);
            return;
        }
        
        // Add enums
        if (type.IsEnum)
        {
            collected.Add(type);
            return;
        }
        
        // Add the type itself
        collected.Add(type);
        
        // Recurse into fields and properties that would be network-serialized
        // Use GetNetworkSerializableMembers to match the game's serialization rules
        var (fieldInfos, propertyInfos) = GetNetworkSerializableMembers(type);
        foreach (var field in fieldInfos)
            CollectDependentTypes(field.FieldType, collected, visited);
        foreach (var prop in propertyInfos)
            CollectDependentTypes(prop.PropertyType, collected, visited);
    }

    /// <summary>
    /// Expands a set of types to include all their transitive dependencies,
    /// filtering to only include Deku-compatible types.
    /// </summary>
    private static HashSet<Type> ExpandWithDependencies(HashSet<Type> types)
    {
        var result = new HashSet<Type>();
        var visited = new HashSet<Type>();
        foreach (var type in types)
            CollectDependentTypes(type, result, visited);
        
        // Filter to only Deku-compatible types
        var compatible = new HashSet<Type>();
        foreach (var type in result)
        {
            if (IsDekuCompatible(type, []))
                compatible.Add(type);
        }
        return compatible;
    }

    static bool IsTypeEmpty(Type type)
    {
        if (type.IsPrimitive || type.IsValueType || type.IsEnum) return false;

        var (fieldInfos, propertyInfos) = GetPublicTypeMembers(type);
        return fieldInfos.Length == 0 && propertyInfos.Length == 0;
    }

    static bool IsTypeIgnored(Type type)
    {
        if (type.IsEnum || type.IsPrimitive || type.IsValueType) return false;
        // Types used in network replication (Deku) should not be ignored
        if (_dekuTypes.Contains(type)) return false;
        if (type.GetCustomAttributes(typeof(XmlSerializerAssemblyAttribute), true).Length <= 0 &&
            type.GetCustomAttributes(typeof(ProtoContractAttribute), true).Length <= 0) return true;
        if (typeof(Type) == type) return true;
        if (typeof(Delegate).IsAssignableFrom(type)) return true;
        return false;
    }

    static (FieldInfo[], PropertyInfo[]) GetPublicTypeMembers(Type type)
    {
        return (type.GetFields(BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly),
            type.GetProperties(BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly));
    }

    /// <summary>
    /// Gets members that would actually be serialized over the network by the game's serializer.
    /// 
    /// The game's serialization rules (from VRage.Serialization.MySerializerObject):
    /// - Must NOT have [NoSerialize] attribute
    /// - Must EITHER have [Serialize] attribute OR be "member public"
    /// 
    /// "Member public" for properties (from VRage.Library.System.Reflection.MemberAccess.IsMemberPublic):
    /// - GetGetMethod() must return non-null (has public getter)
    /// - GetSetMethod() must return non-null (has public setter - returns null for private set!)
    /// - Both must have MethodAttributes.Public
    /// 
    /// This is why types like MyStoreItem with `Action { get; private set; }` are actually
    /// Deku-compatible - the Actions never cross the wire because the game skips them.
    /// </summary>
    static (FieldInfo[], PropertyInfo[]) GetNetworkSerializableMembers(Type type)
    {
        var fields = type.GetFields(BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly)
            .Where(f => !Attribute.IsDefined(f, typeof(VRage.Serialization.NoSerializeAttribute)))
            .ToArray();
        
        var properties = type.GetProperties(BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly)
            .Where(p => !Attribute.IsDefined(p, typeof(VRage.Serialization.NoSerializeAttribute)))
            .Where(p => Attribute.IsDefined(p, typeof(VRage.Serialization.SerializeAttribute)) || IsPropertyNetworkPublic(p))
            .ToArray();
        
        return (fields, properties);
    }

    /// <summary>
    /// Checks if a property is considered "public" for network serialization purposes.
    /// Matches the game's IsMemberPublic behavior from VRage.Library.
    /// 
    /// Key insight: GetSetMethod() returns null for private setters, so properties
    /// like `public Action OnCancel { get; private set; }` return false here.
    /// </summary>
    static bool IsPropertyNetworkPublic(PropertyInfo prop)
    {
        var getter = prop.GetGetMethod();
        var setter = prop.GetSetMethod(); // Returns null for private set!
        
        if (getter == null || setter == null)
            return false;
        
        return getter.IsPublic && setter.IsPublic;
    }

    static Tuple<string, string, ExtraTypeInfo, ExtraSerializationInfo> BuildIntermediateMemberInfo(Type type, MemberInfo member)
    {
        var snakeName = StringToSnakeCase(member.Name);
        var sanitizedName = RustKeywords.Contains(snakeName) ? $"r#{snakeName}" : snakeName;
        return new Tuple<string, string, ExtraTypeInfo, ExtraSerializationInfo>(
            member.Name,
            sanitizedName,
            new ExtraTypeInfo { Type = type },
            new ExtraSerializationInfo
            {
                Member = member,
            }
        );
    }

    static bool WriteEnum(Type type, StreamWriter writer)
    {
        writer.WriteLine($"// Original enum: {type.FullName}");
        var isFlags = type.GetCustomAttributes(typeof(FlagsAttribute), true).Length > 0;
        var needsDeku = NeedsDekuDerives(type);
        
        if (isFlags)
        {
            var enumFields = type.GetFields(BindingFlags.Public | BindingFlags.Static);
            var deriveArguments = "";
            var defaultValueField = Array.Find(enumFields, f => f.Name == "Default");
            if (defaultValueField != null)
            {
                // Get the default value and write it as bitflags default
                var defaultValue = Convert.ToUInt64(defaultValueField.GetRawConstantValue() ?? 0);
                // We need to write the default as the identifiers bitbanged together
                // for example, if Default = A | C, we need to write "A | C"
                var defaultFlags = new List<string>();
                foreach (var field in enumFields)
                {
                    if (field.Name == "Default") continue;
                    var value = Convert.ToUInt64(field.GetRawConstantValue() ?? 0);
                    if (value == 0) continue;
                    if ((defaultValue & value) == value)
                    {
                        defaultFlags.Add(field.Name);
                    }
                }

                deriveArguments += $"(default = {string.Join(" | ", defaultFlags)})";
            }
            
            var enumReprType = enumFields.First().GetRawConstantValue()?.GetType();
            writer.WriteLine($"#[::enumflags2::bitflags{deriveArguments}]");
            writer.WriteLine($"#[repr({RecursiveTypeName(enumReprType!).Replace("i", "u")})]");
        }
        else
        {
            writer.WriteLine("#[::proto_rs::proto_message]");
        }

        List<string> deriveTraits =
            ["Debug", "Clone", "Copy", "PartialEq", "Eq", "Hash", "PartialOrd", "Ord", "::serde::Serialize", "::serde::Deserialize"];
        if (!isFlags) deriveTraits.Insert(0, "Default");
        // Deku is only compatible with non-flags enums (flags use enumflags2)
        if (needsDeku && !isFlags)
        {
            deriveTraits.Add("::deku::DekuRead");
            deriveTraits.Add("::deku::DekuWrite");
        }
        writer.WriteLine(
                $"#[derive({string.Join(", ", deriveTraits)})]");
        
        // For non-flags enums with Deku, add bit-packed serialization attribute
        // SE uses: bitCount = (int)Math.Log(maxValue, 2.0) + 1; then ReadUInt64(bitCount)
        if (needsDeku && !isFlags)
        {
            var enumValues = type.GetFields(BindingFlags.Public | BindingFlags.Static)
                .Select(f => Convert.ToUInt64(f.GetRawConstantValue() ?? 0))
                .ToList();
            var maxValue = enumValues.Count > 0 ? enumValues.Max() : 0UL;
            var bitCount = maxValue > 0 ? (int)Math.Log(maxValue, 2.0) + 1 : 1;
            var underlyingType = Enum.GetUnderlyingType(type);
            var dekuType = underlyingType == typeof(byte) || underlyingType == typeof(sbyte) ? "u8" :
                           underlyingType == typeof(short) || underlyingType == typeof(ushort) ? "u16" :
                           underlyingType == typeof(int) || underlyingType == typeof(uint) ? "u32" : "u64";
            writer.WriteLine($"#[deku(id_type = \"{dekuType}\", bits = {bitCount})]");
        }
        
        var qualifiedName = QualifiedRustName(type);
        writer.WriteLine($"#[serde(rename = \"{type.Name}\")]");
        writer.WriteLine($"pub enum {qualifiedName} {{");

        var fields = type.GetFields(BindingFlags.Public | BindingFlags.Static);
        var compositeValues = new List<(string, string)>();
        var setDefault = false;
        foreach (var field in fields)
        {
            var name = field.Name;
            if (isFlags)
            {
                // if a value appears multiple times, we only want to treat the first instance normally, the following instances should be skipped as composite values
                var existingValue = fields.Where(f => Convert.ToInt64(f.GetRawConstantValue()) == Convert.ToInt64(field.GetRawConstantValue())).ToList();
                // check if we are the first index of this value
                var isFirstIndex = existingValue.IndexOf(field) == 0;
                
                var value = Convert.ToInt64(field.GetRawConstantValue());
                // if value has more than one bit handle it
                if ((value & (value - 1)) != 0 || name == "Default" || name == "All" || (existingValue.Count > 1 && !isFirstIndex))
                {
                    writer.WriteLine($"    // Skipping composite value in flags enum ({name} = {value})");
                    // -1 or "All" means all bits set
                    if (name == "All" || value == -1)
                    {
                        compositeValues.Add((name, "::enumflags2::BitFlags::<Self>::ALL"));
                        continue;
                    }
                    
                    // extract the individual bits, match against other enum values and build bitflags expression
                    var individualBits = new List<string>();
                    for (long bit = 1; bit != 0 && bit <= value; bit <<= 1)
                    {
                        if ((value & bit) != bit) continue;
                        var matchingField = fields.FirstOrDefault(f => Convert.ToInt64(f.GetRawConstantValue()) == bit);
                        if (matchingField != null)
                        {
                            individualBits.Add(matchingField.Name);
                        }
                    }

                    compositeValues.Add((name, $"::enumflags2::make_bitflags!(Self::{{{string.Join(" | ", individualBits)}}})"));
                    continue;
                }

                switch (value)
                {
                    case -1:
                        compositeValues.Add((name, "::enumflags2::BitFlags::<Self>::ALL"));
                        writer.WriteLine($"    // Skipping {value} value in flags enum ({name})");
                        continue;
                    case 0:
                        compositeValues.Add((name, "::enumflags2::BitFlags::<Self>::EMPTY"));
                        writer.WriteLine($"    // Skipping {value} value in flags enum ({name})");
                        continue;
                    default:
                        writer.WriteLine($"    {name} = {value},");
                        break;
                }
            }
            else
            {
                var enumValue = Convert.ToUInt64(field.GetRawConstantValue() ?? 0);
                if (!setDefault) 
                {
                    writer.WriteLine($"    #[default]");
                    setDefault = true;
                }
                if (needsDeku)
                {
                    writer.WriteLine($"    #[deku(id = \"{enumValue}\")]");
                }
                // Rename "Error" variant to avoid conflict with TryFrom::Error associated type
                if (name == "Error")
                {
                    writer.WriteLine($"    #[serde(rename = \"Error\")]");
                    writer.WriteLine($"    Error_,");
                }
                else
                {
                    writer.WriteLine($"    {name},");
                }
            }
        }
        
        writer.WriteLine("}");

        if (compositeValues.Count > 0)
        {
            writer.WriteLine($"impl {qualifiedName} {{");
            foreach (var (name, value) in compositeValues)
                writer.WriteLine($"    pub const {StringToSnakeCase(name).ToUpper()}: ::enumflags2::BitFlags<Self> = {value};");
            writer.WriteLine("}");
        }


        return true;
    }

//     private static bool WriteSerializableDictionary(ExtraTypeInfo type, StreamWriter writer)
//     {
//         var keyTypeInfo = new ExtraTypeInfo { Type = type.Type.GenericTypeArguments[0] };
//         var valTypeInfo = new ExtraTypeInfo { Type = type.Type.GenericTypeArguments[1] };
//         
//         writer.WriteLine(@$"#[derive(Debug, Clone, PartialEq)]
// #[::proto_rs::proto_message]
// pub struct {type.SanitizedTypeName}(
//     #[proto(tag = 1)] 
//     pub ::std::collections::HashMap<{keyTypeInfo.SanitizedTypeName}, {valTypeInfo.SanitizedTypeName}>,
// );
//
// impl ::serde::Serialize for {type.SanitizedTypeName} {{
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {{
//         #[derive(::serde::Serialize)]
//         #[serde(rename = ""item"")]
//         struct {type.SanitizedTypeName}EntryRef<'a> {{
//             #[serde(rename = ""Key"")]
//             k: &'a {keyTypeInfo.SanitizedTypeName},
//             #[serde(rename = ""Value"")]
//             v: &'a {valTypeInfo.SanitizedTypeName},
//         }}
//
//         let mut state = serializer.serialize_struct(""{type.SanitizedTypeName}"", 1)?;
//         let entries_iter = self.0.iter().map(|(k, v)| {type.SanitizedTypeName}EntryRef {{
//             k,
//             v,
//         }});
//         let entries: Vec<_> = entries_iter.collect();
//         ::serde::ser::SerializeStruct::serialize_field(&mut state, ""dictionary"", &entries)?;
//         ::serde::ser::SerializeStruct::end(state)
//     }}
// }}
//
// impl<'de> ::serde::Deserialize<'de> for {type.SanitizedTypeName} {{
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {{
//         // Owned version for deserialization
//         #[derive(::serde::Deserialize)]
//         #[serde(rename = ""item"")]
//         struct {type.SanitizedTypeName}Entry {{
//             #[serde(rename = ""Key"")]
//             k: {keyTypeInfo.SanitizedTypeName},
//             #[serde(rename = ""Value"")]
//             v: {valTypeInfo.SanitizedTypeName},
//         }}
//         #[derive(::serde::Deserialize)]
//         #[serde(rename = ""Dictionary"")]
//         struct Helper {{
//             #[serde(rename = ""dictionary"")]
//             items: Vec<{type.SanitizedTypeName}Entry>,
//         }}
//         let helper = Helper::deserialize(deserializer)?;
//         let map = helper
//             .items
//             .into_iter()
//             .map(|entry| (entry.k, entry.v))
//             .collect();
//         Ok({type.SanitizedTypeName}(map))
//     }}
// }}
//
// impl ::std::default::Default for {type.SanitizedTypeName} {{
//     fn default() -> Self {{
//         {type.SanitizedTypeName}(::std::collections::HashMap::new())
//     }}
// }}
// ");
//         return true;
//     }

    static readonly HashSet<Type> _floatCheckVisited = new();
    static bool TypeContainsFloat(Type type)
    {
        if (type == typeof(float) || type == typeof(double)) return true;
        if (type.IsPrimitive || type.IsEnum || type == typeof(string) || type == typeof(decimal)) return false;
        if (type == typeof(DateTime) || type == typeof(TimeSpan) || type == typeof(Guid)) return false;
        // Cycle detection: if we've already visited this type, assume no float (safe default)
        if (!_floatCheckVisited.Add(type)) return false;
        try
        {
            if (IsTypeNullable(type)) return TypeContainsFloat(type.GetGenericTypeDefinition());
            if (IsTypeArray(type)) return TypeContainsFloat(type.GetElementType() ?? type.GenericTypeArguments[0]);
            if (IsTypeHashMap(type)) return true; // HashMap keys/values could be anything, skip Hash
            var (fieldInfos, propertyInfos) = GetPublicTypeMembers(type);
            foreach (var field in fieldInfos)
                if (TypeContainsFloat(field.FieldType)) return true;
            foreach (var prop in propertyInfos)
                if (TypeContainsFloat(prop.PropertyType)) return true;
            return false;
        }
        finally
        {
            _floatCheckVisited.Remove(type);
        }
    }

    static bool WriteRustStructAndDependents(Type type, StreamWriter writer)
    {
        // Guard against re-processing the same type (prevents infinite recursion)
        if (!_processedTypes.Add(type))
            return true;

        if (IsTypeEmpty(type))
        {
            // Array types and generics with a known Rust mapping don't need stubs for the container,
            // but we still need to process the element/generic argument types
            if (type.IsArray)
            {
                var elemType = type.GetElementType()!;
                WriteRustStructAndDependents(elemType, writer);
                return true;
            }
            if (type.IsGenericType && new ExtraTypeInfo { Type = type }.HasRustType)
            {
                foreach (var typeArg in type.GenericTypeArguments)
                    WriteRustStructAndDependents(typeArg, writer);
                return true;
            }

            // Emit a stub for empty types so they can be referenced by other types
            // But skip types that have built-in Rust mappings (object, string, primitives, etc.)
            var emptyTypeInfo = new ExtraTypeInfo { Type = type };
            if (emptyTypeInfo.HasRustType)
                return true;
            
            if (!type.IsPrimitive && !type.IsValueType && !type.IsEnum)
            {

                var isStubProtobuf = type.GetCustomAttributes(typeof(ProtoContractAttribute), true).Length > 0 
                    || type.GetCustomAttributes(typeof(XmlSerializerAssemblyAttribute), true).Length > 0;
                // Sanitize name: strip backtick+arity from generic type names (e.g. MyList`1 -> MyList)
                var sanitizedName = QualifiedRustName(type);
                var serializeName = type.Name.Contains('`') ? type.Name.Split('`')[0] : type.Name;
                writer.WriteLine($"// Stub for empty/abstract type: {type.FullName}");
                if (isStubProtobuf) writer.WriteLine("#[::proto_rs::proto_message]");
                var stubTraits = new List<string> { "Debug", "Default", "Clone", "PartialEq", "::serde::Serialize", "::serde::Deserialize" };
                if (NeedsDekuDerives(type)) { stubTraits.Add("::deku::DekuRead"); stubTraits.Add("::deku::DekuWrite"); }
                writer.WriteLine($"#[derive({string.Join(", ", stubTraits)})]");
                writer.WriteLine($"#[serde(rename = \"{serializeName}\")]");
                writer.WriteLine($"pub struct {sanitizedName} {{}}");
            }
            return true;
        }

        var typeInfo = new ExtraTypeInfo { Type = type };
        if (typeInfo.HasRustType)
        {
            writer.WriteLine($"// Note: Type mapping applied from {type.FullName} to {typeInfo.SanitizedTypeName}");
            if (!type.IsGenericType) return true;
            foreach (var typeArg in type.GenericTypeArguments)
            {
                if (!WriteRustStructAndDependents(typeArg, writer))
                {
                    Console.WriteLine(
                        $"// Skipped generic argument type with no public members: {typeArg.FullName}");
                }
            }
            return true;
        }

        Type[] specialTypes = [typeof(DateTime), typeof(TimeSpan), typeof(Guid), typeof(decimal), typeof(List<>), typeof(HashSet<>), typeof(SerializableDictionary<,>)];
        if (specialTypes.Contains(type) ||
            (type.IsGenericType && specialTypes.Contains(type.GetGenericTypeDefinition())))
        {
            var handled = type switch
            {
                _ when type.IsGenericType && type.GetGenericTypeDefinition() == typeof(SerializableDictionary<,>) =>
                    // WriteSerializableDictionary(typeInfo, writer),
                    true,
                _ => false
            };

            if (!handled)
            {
                // protobuf-net existed before well known types for these objects...
                // https://github.com/protobuf-net/protobuf-net/blob/main/src/Tools/bcl.proto
                writer.WriteLine($"// Note: Special type {type.FullName} expects existing type");
            }
            return true;
        }

        if (IsTypeIgnored(type))
        {
            // Types with a known Rust built-in mapping don't need stubs
            if (type == typeof(string) || type == typeof(object))
                return true;

            // Types whose RecursiveTypeName differs from their raw name have explicit mappings
            var rustName = RecursiveTypeName(type);
            var defaultName = QualifiedRustName(type);
            if (rustName != defaultName)
                return true;
            

            // Emit a stub so other generated types can reference this type
            var ignoredName = QualifiedRustName(type);
            var ignoredSerializeName = type.Name.Contains('`') ? type.Name.Split('`')[0] : type.Name;
            writer.WriteLine($"// Stub for ignored type (no serialization attributes): {type.FullName}");
            var ignoredTraits = new List<string> { "Debug", "Default", "Clone", "PartialEq", "::serde::Serialize", "::serde::Deserialize" };
            if (NeedsDekuDerives(type)) { ignoredTraits.Add("::deku::DekuRead"); ignoredTraits.Add("::deku::DekuWrite"); }
            writer.WriteLine($"#[derive({string.Join(", ", ignoredTraits)})]");
            writer.WriteLine($"#[serde(rename = \"{ignoredSerializeName}\")]");
            writer.WriteLine($"pub struct {ignoredName} {{}}");
            return true;
        }

        // Special handling for enums
        if (type.IsEnum)
        {
            return WriteEnum(type, writer);
        }

        var (fieldInfos, propertyInfos) = GetPublicTypeMembers(type);
        var isProtobuf = type.GetCustomAttributes(typeof(ProtoContractAttribute), true).Length > 0;
        var isDekuOnly = NeedsDekuDerives(type) && !isProtobuf;
        var members = new List<Tuple<string, string, ExtraTypeInfo, ExtraSerializationInfo>>();
        foreach (var field in fieldInfos)
        {
            // For Deku-only types (no protobuf), skip fields marked [NoSerialize]
            if (isDekuOnly && field.GetCustomAttributes(typeof(NoSerializeAttribute), true).Length > 0)
                continue;

            if (!WriteRustStructAndDependents(field.FieldType, writer))
            {
                Console.WriteLine(
                    $"// Skipped field `{field.Name}` with no public members ({field.FieldType.FullName})");
                continue;
            }

            members.Add(BuildIntermediateMemberInfo(field.FieldType, field));
        }

        foreach (var prop in propertyInfos)
        {
            var propSerInfo = new ExtraSerializationInfo { Member = prop };

            // Skip computed / alias properties marked [NoSerialize] that have
            // no explicit XML serialization attributes.  Properties with
            // [XmlAttribute] or [XmlElement] are needed for XML deserialization
            // even when [NoSerialize] suppresses protobuf serialization
            // (e.g. SerializableDefinitionId.TypeIdStringAttribute).
            // Properties with neither (e.g. MyObjectBuilder_Identity.PlayerId)
            // are pure aliases that delegate to another field and should be
            // omitted to avoid duplicate data.
            if (propSerInfo.NoSerialize && !propSerInfo.IsXmlAttribute
                && prop.GetCustomAttributes(typeof(XmlElementAttribute), true).Length == 0)
                continue;
            
            // Skip properties that have no serialization attributes at all.
            // These are typically computed properties or aliases (e.g. MaxPlayers).
            // Exception: Deku-only types include all public properties since they
            // use network replication via IMemberAccessor, not proto/XML attributes.
            var hasXmlElement = prop.GetCustomAttributes(typeof(XmlElementAttribute), true).Length > 0;
            if (!isDekuOnly && !propSerInfo.IsProtoMember && !propSerInfo.IsXmlAttribute && !hasXmlElement)
                continue;

            // For Deku-only types (no protobuf), completely drop fields that would be:
            // - serde(skip): XmlIgnore
            // - deku(skip): not network-public (private setter) and no [Serialize]
            // These fields exist only for local runtime state (e.g., Action delegates)
            // and are never serialized in any format.
            if (isDekuOnly && propSerInfo.IsXmlIgnore)
            {
                var isNetworkSerializable = IsPropertyNetworkPublic(prop) ||
                    Attribute.IsDefined(prop, typeof(VRage.Serialization.SerializeAttribute));
                if (!isNetworkSerializable)
                {
                    Console.WriteLine($"// Dropped non-serialized property `{prop.Name}` (serde+deku skip)");
                    continue;
                }
            }

            if (!WriteRustStructAndDependents(prop.PropertyType, writer))
            {
                Console.WriteLine(
                    $"// Skipped property `{prop.Name}` with no public members ({prop.PropertyType.FullName})");
                continue;
            }

            members.Add(BuildIntermediateMemberInfo(prop.PropertyType, prop));
        }

        // Check if any member has a non-zero numeric default or enum default (needs serde_inline_default on struct)
        var hasInlineDefaults = members.Any(m => 
            (m.Item4.HasNumericDefaultValue && !m.Item4.IsNumericDefaultZero) || m.Item4.HasEnumDefaultValue);

        writer.WriteLine($"// Original type: {type.FullName}");
        
        // If any field has a non-zero numeric default or enum default, add serde_inline_default before derives
        if (hasInlineDefaults)
            writer.WriteLine("#[::serde_inline_default::serde_inline_default]");
        
        List<string> traits = ["Debug", "Default", "Clone", "PartialEq", "::serde::Serialize", "::serde::Deserialize"];
        // Add Deku derives for types used in network replication
        if (NeedsDekuDerives(type))
        {
            traits.Add("::deku::DekuRead");
            traits.Add("::deku::DekuWrite");
        }
        if (isProtobuf) 
        {
            // traits.Add("::prost::Message");
            writer.WriteLine("#[::proto_rs::proto_message]");
        }
        // If the type inherits IEquatable<T>, we should also derive Eq
        // (but only if the type doesn't contain float fields, since f32/f64 don't impl Eq)
        if (!TypeContainsFloat(type) && type.GetInterfaces().Any(i =>
                i.IsGenericType && i.GetGenericTypeDefinition() == typeof(IEquatable<>) &&
                i.GetGenericArguments()[0] == type))
        {
            traits.Add("Eq");
        }
        // If the type overrides GetHashCode, we should also derive Hash
        // (but only if the type doesn't contain float fields, since f32/f64 don't impl Hash)
        var getHashCodeMethod = type.GetMethod("GetHashCode", BindingFlags.Public |
                                                    BindingFlags.Instance);
        if (getHashCodeMethod != null && getHashCodeMethod.DeclaringType == type && !TypeContainsFloat(type))
        {
            traits.Add("Hash");
            if (!traits.Contains("Eq")) traits.Add("Eq");
            traits.Add("PartialOrd");
            traits.Add("Ord");
        }
        writer.WriteLine(
            $"#[derive({string.Join(", ", traits)})]");
        writer.WriteLine($"#[serde(rename = \"{typeInfo.Name}\")]");
        writer.WriteLine($"pub struct {typeInfo.SanitizedTypeName} {{");
        
        var index = 0;
        foreach (var (memberName, sanitizedName, extraTypeInfo, memberInfo) in members)
        {
            // Determine if each serialization method would skip this field
            var serdeWillSkip = memberInfo.IsXmlIgnore;
            var protoWillSkip = !isProtobuf || !memberInfo.IsProtoMember || memberInfo.NoSerialize;
            
            var dekuWillSkip = false;
            if (NeedsDekuDerives(type))
            {
                if (!WillHaveDekuSupport(extraTypeInfo.Type))
                {
                    dekuWillSkip = true;
                }
                else if (memberInfo.Member is PropertyInfo prop)
                {
                    if (!IsPropertyNetworkPublic(prop) && 
                        !Attribute.IsDefined(prop, typeof(VRage.Serialization.SerializeAttribute)))
                    {
                        dekuWillSkip = true;
                    }
                }
            }
            else
            {
                // Non-Deku types don't need the field for Deku, so consider it "skipped"
                dekuWillSkip = true;
            }
            
            // If ALL three serialization methods skip this field, drop it entirely
            if (serdeWillSkip && protoWillSkip && dekuWillSkip)
            {
                Console.WriteLine($"// Dropped field `{memberName}` (skipped by all serialization methods)");
                continue;
            }

            if (isProtobuf)
            {
                if (memberInfo.IsProtoMember && !memberInfo.NoSerialize)
                {
                    if (index != members.FindIndex((m) => m.Item4.ProtoTag == memberInfo.ProtoTag))
                    {
                        writer.WriteLine(
                            $"    // Warning: Duplicate Protobuf tag {memberInfo.ProtoTag} in type {type.FullName}");
                        writer.WriteLine($"    #[proto(skip)]");
                    }
                    else
                    {
                        writer.WriteLine($"    #[proto(tag = {memberInfo.ProtoTag})]");
                    }
                }
                else
                {
                    writer.WriteLine($"    #[proto(skip)]");
                }
            }

            if (!memberInfo.IsXmlIgnore)
            {
                var serdeParts = new List<string>();

                // Rename attribute (@ prefix for XML attributes)
                serdeParts.Add(memberInfo.IsXmlAttribute
                    ? $"rename = \"@{memberName}\""
                    : $"rename = \"{memberName}\"");

                // Non-zero numeric defaults and enum defaults use #[serde_inline_default(value)] as a separate attribute.
                // Zero/empty defaults (booleans, strings, collections) use serde's built-in default.
                string? inlineDefaultLiteral = null;
                if (memberInfo.HasNumericDefaultValue && !memberInfo.IsNumericDefaultZero)
                    inlineDefaultLiteral = memberInfo.NumericDefaultRustLiteral;
                else if (memberInfo.HasEnumDefaultValue)
                    inlineDefaultLiteral = memberInfo.EnumDefaultRustLiteral;
                
                // For Deku types, wrap numeric defaults in BitAligned()
                if (inlineDefaultLiteral != null && NeedsDekuDerives(type) && memberInfo.HasNumericDefaultValue)
                    inlineDefaultLiteral = $"crate::compat::BitAligned({inlineDefaultLiteral})";
                
                // Default for types with natural zero/empty defaults (collections, booleans,
                // strings).  XML attributes are also optional and should default when absent.
                // Skip adding serde(default) if we're using serde_inline_default for this field.
                if ((memberInfo.HasSerdeDefault || memberInfo.IsXmlAttribute) && inlineDefaultLiteral == null)
                {
                    serdeParts.Add("default");
                }

                // Vec fields need a custom deserializer to handle self-closing XML
                // elements like `<Members />`.  The XmlArrayItem path below already
                // supplies its own deserialize_with, so we only add the generic one
                // when no XmlArrayItem override is present.
                // Skip for Deku types - they use VarVec which handles serialization differently.
                var xmlArrayItemName2 = memberInfo.XmlArrayItemName;
                var hasXmlArrayItemOverride = xmlArrayItemName2 != null && extraTypeInfo.IsArray &&
                                             xmlArrayItemName2 != new ExtraTypeInfo { Type = extraTypeInfo.Type.GetElementType() ?? extraTypeInfo.Type.GenericTypeArguments[0] }.Name;
                if (extraTypeInfo.IsArray && !hasXmlArrayItemOverride && !NeedsDekuDerives(type))
                    serdeParts.Add("deserialize_with = \"crate::compat::xml_vec::deserialize\"");

                // Emit serde_inline_default attribute for non-zero numeric defaults and enum defaults
                if (inlineDefaultLiteral != null)
                    writer.WriteLine($"    #[serde_inline_default({inlineDefaultLiteral})]");
                
                writer.WriteLine($"    #[serde({string.Join(", ", serdeParts)})]");
            }
            else
            {
                writer.WriteLine($"    #[serde(skip)]");
            }

            // For Deku types, skip fields/properties in two cases:
            // 1. The field's type won't have Deku support (complex type not in _dekuTypes)
            // 2. Properties with private setters (not network-serializable per game rules)
            // This matches the game's MySerializerObject behavior - see rpc.rs documentation.
            if (NeedsDekuDerives(type))
            {
                var needsDekuSkip = false;
                
                // Check if the member type will have Deku support
                if (!WillHaveDekuSupport(extraTypeInfo.Type))
                {
                    needsDekuSkip = true;
                }
                // Check if property has private setter (not network-serializable)
                else if (memberInfo.Member is PropertyInfo memberProp)
                {
                    if (!IsPropertyNetworkPublic(memberProp) && 
                        !Attribute.IsDefined(memberProp, typeof(VRage.Serialization.SerializeAttribute)))
                    {
                        needsDekuSkip = true;
                    }
                }
                
                if (needsDekuSkip)
                {
                    writer.WriteLine($"    #[deku(skip)]");
                }
            }

            // Use BitAligned field types for types with Deku derives
            var rustTypeName = extraTypeInfo.IsEnumFlags() 
                ? $"crate::compat::BitField<{extraTypeInfo.SanitizedTypeName}>" 
                : NeedsDekuDerives(type) 
                    ? extraTypeInfo.DekuSanitizedTypeName 
                    : extraTypeInfo.SanitizedTypeName;

            // If the field has an [XmlArrayItem("...")] attribute, emit a second
            // #[serde] line with serialize_with/deserialize_with pointing to
            // the module generated by define_xml_array_item!.
            // The field type stays Vec<T> — no wrapper types, no proto changes.
            //
            // However, if the item name already matches the element type's serde
            // rename, quick-xml will naturally use the correct element name and
            // the custom functions are unnecessary.
            // Skip for Deku types - they use VarVec which has different serialization.
            var xmlArrayItemName = memberInfo.XmlArrayItemName;
            if (xmlArrayItemName != null && extraTypeInfo.IsArray && !memberInfo.IsXmlIgnore && !NeedsDekuDerives(type))
            {
                var elementType = extraTypeInfo.Type.GetElementType() ?? extraTypeInfo.Type.GenericTypeArguments[0];
                var elementTypeInfo = new ExtraTypeInfo { Type = elementType };
                if (xmlArrayItemName != elementTypeInfo.Name)
                {
                    _emittedXmlArrayItemWrappers.Add(xmlArrayItemName);
                    writer.WriteLine($"    #[serde(serialize_with = \"xml_array_item::{xmlArrayItemName}::serialize\",");
                    writer.WriteLine($"            deserialize_with = \"xml_array_item::{xmlArrayItemName}::deserialize\")]");
                }
            }
            writer.WriteLine($"    pub {sanitizedName}: {rustTypeName},");
            
            // If this is an enum flags field, we need to generate a helper method for it later
            // if (extraTypeInfo.IsEnumFlags()) flagFields.Add((sanitizedName, extraTypeInfo));
            
            index++;
        }

        writer.WriteLine("}");

        // if (flagFields.Count > 0)
        // {
        //     writer.WriteLine("// Note: This isn't the most ergonomic way to handle flag fields, but it is what we've got with Prost.");
        //     writer.WriteLine($"impl {type.Name} {{");
        //     foreach (var flagField in flagFields)
        //     {
        //         writer.WriteLine(
        //             $"    pub fn {flagField.Item1}(&self) -> ::enumflags2::BitFlags<{flagField.Item2.SanitizedTypeName}> {{");
        //         writer.WriteLine(
        //             $"        ::enumflags2::BitFlags::from_bits_truncate(self.{flagField.Item1} as u32)");
        //         writer.WriteLine("    }");
        //         writer.WriteLine("}");
        //     }
        // }

        return true;
    }

    public static void GenerateRustStructs(List<Type> baseTypes, string outputPath, string filename = "game_data.rs", HashSet<Type>? dekuTypes = null)
    {
        _processedTypes.Clear();
        _floatCheckVisited.Clear();
        _emittedXmlArrayItemWrappers.Clear();
        // Expand dekuTypes to include all transitive dependencies
        _dekuTypes = dekuTypes != null ? ExpandWithDependencies(dekuTypes) : [];

        var eventListFilename = Path.Combine(outputPath, filename);
        using var writer = new StreamWriter(eventListFilename);
        writer.WriteLine("// Auto-generated by StandaloneExtractor — do not edit manually");
        writer.WriteLine("#![cfg_attr(rustfmt, rustfmt_skip)]");
        writer.WriteLine("#![allow(non_camel_case_types, non_snake_case, unused_imports, clippy::all, clippy::pedantic, clippy::suspicious)]");
        writer.WriteLine();
        writer.WriteLine("use crate::*;");
        writer.WriteLine();
        foreach (var type in baseTypes)
        {
            if (!WriteRustStructAndDependents(type, writer))
            {
                writer.WriteLine("// Skipped type with no public members: " + type.FullName);
            }
        }
        
        // Emit a single namespace module containing all XmlArrayItem helpers.
        // This prevents the generated modules from conflicting with real type names.
        if (_emittedXmlArrayItemWrappers.Count > 0)
        {
            writer.WriteLine();
            writer.WriteLine("/// Namespace for XmlArrayItem serialize/deserialize helpers.");
            writer.WriteLine("/// Each sub-module is generated by `define_xml_array_item!` and provides");
            writer.WriteLine("/// `serialize` / `deserialize` functions for a specific XML element name.");
            writer.WriteLine("pub mod xml_array_item {");
            foreach (var name in _emittedXmlArrayItemWrappers)
            {
                writer.WriteLine($"    crate::compat::define_xml_array_item!({name});");
            }
            writer.WriteLine("}");
        }
    }
}

