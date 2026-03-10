// Re-export the compat crate (hand-written compatibility types)
pub use space_engineers_compat as compat;
pub use space_engineers_compat::direction;
pub use space_engineers_compat::direction::*;
pub use space_engineers_compat::math;
pub use space_engineers_compat::math::*;
pub use space_engineers_compat::*;

// Re-export the sys crate (auto-generated SE data structures)
pub use space_engineers_sys::types;
pub use space_engineers_sys::types::*;
#[cfg(test)]
mod tests {
    use super::*;
    /// Verifies that a single `MyObjectBuilder_Faction` round-trips through XML,
    /// including self-closing empty elements for Vec fields like `<Stations />`,
    /// `<Members />`, and `<JoinRequests />`.
    ///
    /// Space Engineers serializes empty collections as self-closing XML elements
    /// (e.g. `<Stations />`). quick_xml must treat these as empty sequences rather
    /// than attempting to deserialize one empty item, which would fail with
    /// "missing field" errors on required fields like `FactionId` or `PlayerId`.
    #[test]
    fn test_faction_with_self_closing_empty_vecs() {
        let xml = r#"<MyObjectBuilder_Faction xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <FactionId>275081873502543841</FactionId>
          <Tag>FCTM</Tag>
          <Name>The Factorum</Name>
          <Description>A faction of builders and engineers.</Description>
          <PrivateInfo />
          <Members>
            <MyObjectBuilder_FactionMember>
              <PlayerId>144115188075855897</PlayerId>
              <IsLeader>true</IsLeader>
              <IsFounder>true</IsFounder>
            </MyObjectBuilder_FactionMember>
          </Members>
          <JoinRequests />
          <AutoAcceptMember>false</AutoAcceptMember>
          <AutoAcceptPeace>false</AutoAcceptPeace>
          <AcceptHumans>false</AcceptHumans>
          <EnableFriendlyFire>false</EnableFriendlyFire>
          <FactionType>None</FactionType>
          <FactionTypeString>None</FactionTypeString>
          <Stations />
          <CustomColor x="0" y="-0.8" z="-0.450000018" />
          <IconColor x="0" y="-0.8" z="0.55" />
          <FactionIcon>Textures\FactionLogo\Factorum.dds</FactionIcon>
          <TransferedPCUDelta>0</TransferedPCUDelta>
          <Score>0</Score>
          <ObjectivePercentageCompleted>0</ObjectivePercentageCompleted>
          <FactionIconWorkshopId xsi:nil="true" />
          <DamageInflicted><dictionary /></DamageInflicted>
        </MyObjectBuilder_Faction>"#;
        let faction: MyObjectBuilder_Faction = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(faction.faction_id, 275081873502543841);
        assert_eq!(faction.tag, "FCTM");
        assert_eq!(faction.members.len(), 1);
        assert_eq!(faction.members[0].player_id, 144115188075855897);
        assert!(faction.join_requests.is_empty());
        assert!(faction.stations.is_empty());
    }
    /// Verifies that `MyObjectBuilder_FactionCollection` deserializes correctly
    /// when its inner `<Factions>` element is self-closing (i.e. no factions).
    #[test]
    fn test_faction_collection_with_empty_factions() {
        let xml = r#"<MyObjectBuilder_FactionCollection>
          <Factions />
        </MyObjectBuilder_FactionCollection>"#;
        let collection: MyObjectBuilder_FactionCollection =
            quick_xml::de::from_str(xml).unwrap();
        assert!(collection.factions.is_empty());
    }
    /// Verifies that a `MyObjectBuilder_FactionCollection` containing one faction
    /// with self-closing Vec elements deserializes correctly.
    #[test]
    fn test_faction_collection_with_one_faction() {
        let xml = r#"<MyObjectBuilder_FactionCollection xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <Factions>
            <MyObjectBuilder_Faction>
              <FactionId>123</FactionId>
              <Tag>TEST</Tag>
              <Name>Test Faction</Name>
              <Description />
              <PrivateInfo />
              <Members />
              <JoinRequests />
              <AutoAcceptMember>false</AutoAcceptMember>
              <AutoAcceptPeace>false</AutoAcceptPeace>
              <AcceptHumans>false</AcceptHumans>
              <EnableFriendlyFire>false</EnableFriendlyFire>
              <FactionType>None</FactionType>
              <FactionTypeString>None</FactionTypeString>
              <Stations />
              <CustomColor x="0" y="0" z="0" />
              <IconColor x="0" y="0" z="0" />
              <FactionIcon />
              <TransferedPCUDelta>0</TransferedPCUDelta>
              <Score>0</Score>
              <ObjectivePercentageCompleted>0</ObjectivePercentageCompleted>
              <FactionIconWorkshopId xsi:nil="true" />
              <DamageInflicted><dictionary /></DamageInflicted>
            </MyObjectBuilder_Faction>
          </Factions>
        </MyObjectBuilder_FactionCollection>"#;
        let collection: MyObjectBuilder_FactionCollection =
            quick_xml::de::from_str(xml).unwrap();
        assert_eq!(collection.factions.len(), 1);
        assert_eq!(collection.factions[0].faction_id, 123);
        assert_eq!(collection.factions[0].tag, "TEST");
        assert!(collection.factions[0].members.is_empty());
        assert!(collection.factions[0].stations.is_empty());
    }
    /// Verifies that `MyObjectBuilder_Station` deserializes correctly with
    /// a self-closing `<StoreItems />` element.
    #[test]
    fn test_station_with_self_closing_store_items() {
        let xml = r#"<MyObjectBuilder_Station>
          <Id>873803852734506309</Id>
          <Position x="1151579.2" y="-1849528.7" z="-4017654.2" />
          <Up x="-0.835" y="-0.377" z="0.399" />
          <Forward x="-0.018" y="-0.707" z="-0.707" />
          <StationType>SpaceStation</StationType>
          <IsDeepSpaceStation>false</IsDeepSpaceStation>
          <StationEntityId>0</StationEntityId>
          <FactionId>258961289160008772</FactionId>
          <PrefabName>Economy_SpaceStation_4</PrefabName>
          <SafeZoneEntityId>0</SafeZoneEntityId>
          <StoreItems />
          <IsOnPlanetWithAtmosphere>false</IsOnPlanetWithAtmosphere>
        </MyObjectBuilder_Station>"#;
        let station: MyObjectBuilder_Station = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(station.id, 873803852734506309);
        assert_eq!(station.faction_id, 258961289160008772);
        assert_eq!(station.prefab_name, "Economy_SpaceStation_4");
        assert!(station.store_items.is_empty());
    }
}