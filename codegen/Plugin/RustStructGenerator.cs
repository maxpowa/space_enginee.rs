using System.Collections.Concurrent;
using System.Reflection;
using System.Text;
using System.Xml.Serialization;
using ProtoBuf;
using VRage;
using VRage.Collections;
using VRage.Serialization;
using VRageMath;

namespace StandaloneExtractor;

// serde + proto_rs + enumflags2 + quick-xml
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

        return sb.ToString();
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
         type.GetGenericTypeDefinition() == typeof(ICollection<>));

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
            return $"crate::compat::SerializableDictionary<{genericArguments}>";
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

    static bool IsTypeEmpty(Type type)
    {
        if (type.IsPrimitive || type.IsValueType || type.IsEnum) return false;

        var (fieldInfos, propertyInfos) = GetPublicTypeMembers(type);
        return fieldInfos.Length == 0 && propertyInfos.Length == 0;
    }

    static bool IsTypeIgnored(Type type)
    {
        if (type.IsEnum || type.IsPrimitive || type.IsValueType) return false;
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
            // Skip proto_message for enums with an "Error" variant — it conflicts with TryFrom::Error
            var hasErrorVariant = type.GetFields(BindingFlags.Public | BindingFlags.Static).Any(f => f.Name == "Error");
            if (!hasErrorVariant)
                writer.WriteLine("#[::proto_rs::proto_message]");
        }

        List<string> deriveTraits =
            ["Debug", "Clone", "Copy", "PartialEq", "Eq", "Hash", "PartialOrd", "Ord", "::serde::Serialize", "::serde::Deserialize"];
        if (!isFlags) deriveTraits.Insert(0, "Default");
        writer.WriteLine(
                $"#[derive({string.Join(", ", deriveTraits)})]");
        
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
                if (!setDefault) 
                {
                    writer.WriteLine($"    #[default]");
                    setDefault = true;
                }
                writer.WriteLine($"    {name},");
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
            if (!type.IsPrimitive && !type.IsValueType && !type.IsEnum)
            {

                var isStubProtobuf = type.GetCustomAttributes(typeof(ProtoContractAttribute), true).Length > 0 
                    || type.GetCustomAttributes(typeof(XmlSerializerAssemblyAttribute), true).Length > 0;
                // Sanitize name: strip backtick+arity from generic type names (e.g. MyList`1 -> MyList)
                var sanitizedName = QualifiedRustName(type);
                var serializeName = type.Name.Contains('`') ? type.Name.Split('`')[0] : type.Name;
                writer.WriteLine($"// Stub for empty/abstract type: {type.FullName}");
                if (isStubProtobuf) writer.WriteLine("#[::proto_rs::proto_message]");
                writer.WriteLine("#[derive(Debug, Default, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]");
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
            writer.WriteLine("#[derive(Debug, Default, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]");
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
        var members = new List<Tuple<string, string, ExtraTypeInfo, ExtraSerializationInfo>>();
        foreach (var field in fieldInfos)
        {
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
            if (!WriteRustStructAndDependents(prop.PropertyType, writer))
            {
                Console.WriteLine(
                    $"// Skipped property `{prop.Name}` with no public members ({prop.PropertyType.FullName})");
                continue;
            }

            members.Add(BuildIntermediateMemberInfo(prop.PropertyType, prop));
        }

        var isProtobuf = type.GetCustomAttributes(typeof(ProtoContractAttribute), true).Length > 0;

        writer.WriteLine($"// Original type: {type.FullName}");
        List<string> traits = ["Debug", "Default", "Clone", "PartialEq", "::serde::Serialize", "::serde::Deserialize"];
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
                writer.WriteLine(memberInfo.IsXmlAttribute
                    ? $"    #[serde(rename = \"@{memberName}\")]"
                    : $"    #[serde(rename = \"{memberName}\")]");
            }
            else
            {
                writer.WriteLine($"    #[serde(skip)]");
            }

            var rustTypeName = extraTypeInfo.IsEnumFlags() ? $"crate::compat::BitField<{extraTypeInfo.SanitizedTypeName}>" : extraTypeInfo.SanitizedTypeName;
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

    public static void GenerateRustStructs(List<Type> baseTypes, string outputPath, string filename = "game_data.rs")
    {
        _processedTypes.Clear();
        _floatCheckVisited.Clear();

        var eventListFilename = Path.Combine(outputPath, filename);
        using var writer = new StreamWriter(eventListFilename);
        writer.WriteLine("// Auto-generated by StandaloneExtractor — do not edit manually");
        writer.WriteLine("#![allow(non_camel_case_types, non_snake_case, unused_imports)]");
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
    }
}

