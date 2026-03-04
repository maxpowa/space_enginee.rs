using System.Diagnostics.CodeAnalysis;
using HarmonyLib;
using Newtonsoft.Json;
using Sandbox.Engine.Multiplayer;
using VRage.Network;

namespace StandaloneExtractor.Plugin;

public class ReplicationEvents
{
    private readonly string _outputPath;
    private readonly MyTypeTable _typeTable;
    private readonly List<MySynchronizedTypeInfo> _typeIndexedList;

    public ReplicationEvents(string outputPath)
    {
        this._outputPath = outputPath;

        var replicationLayer = new MyReplicationSingle(new EndpointId(0));
        replicationLayer.RegisterFromGameAssemblies();

        this._typeTable = Traverse.Create(replicationLayer).Field("m_typeTable").GetValue() as MyTypeTable ??
                          throw new InvalidOperationException("Couldn't load type table");
        this._typeIndexedList =
            Traverse.Create(_typeTable).Field("m_idToType").GetValue() as List<MySynchronizedTypeInfo> ??
            throw new InvalidOperationException("Couldn't load type indexed list");
    }

    private static Dictionary<uint, CallSite> GetEventTableMapping(MyEventTable eventTable)
    {
        return Traverse.Create(eventTable).Field("m_idToEvent").GetValue() as Dictionary<uint, CallSite> ??
               throw new InvalidOperationException("Couldn't get event table mapping");
    }

    public List<Type> GetAllTypes()
    {
        var list = new List<Type>();
        list.AddList(
            _typeIndexedList
                .SelectMany(each =>
                GetEventTableMapping(each.EventTable)
                        .SelectMany(pair => pair.Value.MethodInfo.GetParameters().Types())).ToList()
        );
        list.AddList(
            GetEventTableMapping(_typeTable.StaticEventTable)
                .SelectMany(pair => pair.Value.MethodInfo.GetParameters().Types()).ToList()
        );
        return list.Distinct().ToList();
    }
    
    [SuppressMessage("ReSharper", "NotAccessedField.Local")]
    private class EventTypeInfo
    {
        public uint Id;
        public bool IsProxied;
        public int ParentTypeHash;
        public bool IsReliable;
        public bool IsBlocking;
        public string CallSiteFlags;
        public string ValidationFlags;
        public string MethodDescriptor;
    }
    
    private static EventTypeInfo BuildEventTypeInfo(KeyValuePair<uint, CallSite> pair)
    {
        // REF: VRage.Network.MyReplicationLayer.DispatchEvent[T1, T2, T3, T4, T5, T6, T7, T8]
        // If the first parameter is IMyEventProxy, then it's a proxy event.
        var arg1 = pair.Value.MethodInfo.GetParameters().FirstOrDefault();
        var isProxied = arg1 != null && typeof(IMyEventProxy).IsAssignableFrom(arg1.ParameterType);
        
        return new EventTypeInfo{
            Id = pair.Value.Id,
            IsProxied = isProxied,
            ParentTypeHash = MySynchronizedTypeInfo.GetHashFromType(pair.Value.MethodInfo.DeclaringType),
            IsReliable = pair.Value.IsReliable,
            IsBlocking = pair.Value.IsBlocking,
            CallSiteFlags = pair.Value.CallSiteFlags.ToString(),
            ValidationFlags = pair.Value.ValidationFlags.ToString(),
            MethodDescriptor = pair.Value.MethodInfo.ToString()!,
        };
    }

    /// <summary>
    /// Serializes the current loaded assemblies RPC objects and their internal events, as well as the global events list.
    /// </summary>
    public void Serialize()
    {
        var eventListFilename = Path.Combine(_outputPath, "id_to_type.json");
        using (var writer = new StreamWriter(eventListFilename))
        using (var json = new JsonTextWriter(writer))
        {
            json.Formatting = Formatting.Indented;
            var ser = new JsonSerializer();
            ser.Serialize(json, _typeIndexedList.Select((x, i) =>
            {
                var events = GetEventTableMapping(x.EventTable);
                var count = this._typeTable.Get(x.Type).EventTable.Count;
                var isNetProxy = typeof(IMyEventProxy).IsAssignableFrom(x.Type);
                return new
                {
                    Id = i, // This index is the same as the MySynchronizedTypeInfo.TypeId, so just use this for simplicity
                    x.TypeHash,
                    x.IsReplicated,
                    x.FullTypeName,
                    InstanceEvents = events.Select(BuildEventTypeInfo)
                };
            }));
        }

        var eventTable = _typeTable.StaticEventTable;
        var eventTableIdMapping = eventTable != null
            ? GetEventTableMapping(eventTable)
            : throw new Exception("Couldn't get static event table mapping");
        var eventTypesFilename = Path.Combine(_outputPath, "static_events.json");
        using (var writer = new StreamWriter(eventTypesFilename))
        using (var json = new JsonTextWriter(writer))
        {
            json.Formatting = Formatting.Indented;
            var ser = new JsonSerializer();
            ser.Serialize(json, eventTableIdMapping.Select(BuildEventTypeInfo));
        }
    }
}

