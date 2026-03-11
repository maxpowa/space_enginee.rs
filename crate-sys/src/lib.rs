#![allow(non_camel_case_types, non_snake_case, unused_imports)]

// Re-export the compat crate at `crate::compat` so generated code paths
// like `crate::compat::DateTime` resolve correctly.
pub use space_engineers_compat as compat;

// Re-export math types at `crate::math` so generated code paths
// like `crate::math::Vector3F` resolve correctly.
pub mod math {
    pub use space_engineers_compat::math::*;
}

pub mod types;

#[cfg(test)]
mod tests {
    use super::types::*;

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

    /// Minimal test to isolate the "invalid type: string "", expected u64" error
    #[test]
    fn test_minimal_checkpoint_fields() {
        // Step 1: Test just the Factions field with empty inner Factions
        let xml = r#"<MyObjectBuilder_FactionCollection>
            <Factions />
        </MyObjectBuilder_FactionCollection>"#;
        let _: MyObjectBuilder_FactionCollection = quick_xml::de::from_str(xml)
            .expect("FactionCollection with empty Factions");
        println!("Step 1 passed: FactionCollection with empty Factions");

        // Step 2: Test nested vec with xml_vec - include all required fields
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
        let fc: MyObjectBuilder_FactionCollection = quick_xml::de::from_str(xml)
            .expect("FactionCollection with one Faction");
        assert_eq!(fc.factions.len(), 1);
        println!("Step 2 passed: FactionCollection with one Faction");

        // Step 3: Test Vec with multiple items
        let xml = r#"<MyObjectBuilder_FactionCollection xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
            <Factions>
                <MyObjectBuilder_Faction>
                    <FactionId>1</FactionId>
                    <Tag>A</Tag>
                    <Name>A</Name>
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
                <MyObjectBuilder_Faction>
                    <FactionId>2</FactionId>
                    <Tag>B</Tag>
                    <Name>B</Name>
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
        let fc: MyObjectBuilder_FactionCollection = quick_xml::de::from_str(xml)
            .expect("FactionCollection with two Factions");
        assert_eq!(fc.factions.len(), 2);
        println!("Step 3 passed: FactionCollection with two Factions");

        println!("All minimal tests passed!");

        // Step 4: Test Nullable<WorkshopId> with xsi:nil="true" inside a Faction
        let xml = r#"<MyObjectBuilder_Faction xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
          <FactionId>123</FactionId>
          <Tag>TEST</Tag>
          <Name>Test</Name>
          <Members />
          <JoinRequests />
          <FactionType>None</FactionType>
          <Stations />
          <CustomColor x="0" y="0" z="0" />
          <IconColor x="0" y="0" z="0" />
          <TransferedPCUDelta>0</TransferedPCUDelta>
          <Score>0</Score>
          <ObjectivePercentageCompleted>0</ObjectivePercentageCompleted>
          <FactionIconWorkshopId xsi:nil="true" />
          <DamageInflicted><dictionary /></DamageInflicted>
        </MyObjectBuilder_Faction>"#;
        let faction: MyObjectBuilder_Faction = quick_xml::de::from_str(xml)
            .expect("Faction with FactionIconWorkshopId xsi:nil=true");
        println!("Step 4 passed: Faction with FactionIconWorkshopId xsi:nil=true");
    }

    #[test]
    fn test_parse_session_settings() {
        // Test just MyObjectBuilder_SessionSettings first to isolate
        let settings_xml = r#"<MyObjectBuilder_SessionSettings>
            <GameMode>Survival</GameMode>
            <InventorySizeMultiplier>1</InventorySizeMultiplier>
            <BlocksInventorySizeMultiplier>1</BlocksInventorySizeMultiplier>
            <AssemblerSpeedMultiplier>1</AssemblerSpeedMultiplier>
            <AssemblerEfficiencyMultiplier>1</AssemblerEfficiencyMultiplier>
            <RefinerySpeedMultiplier>1</RefinerySpeedMultiplier>
            <OnlineMode>OFFLINE</OnlineMode>
            <MaxPlayers>1</MaxPlayers>
            <MaxFloatingObjects>50</MaxFloatingObjects>
            <TotalBotLimit>10</TotalBotLimit>
            <MaxBackupSaves>0</MaxBackupSaves>
            <MaxGridSize>0</MaxGridSize>
            <MaxBlocksPerPlayer>0</MaxBlocksPerPlayer>
            <TotalPCU>100000</TotalPCU>
            <PiratePCU>0</PiratePCU>
            <GlobalEncounterPCU>0</GlobalEncounterPCU>
            <MaxFactionsCount>0</MaxFactionsCount>
            <BlockLimitsEnabled>GLOBALLY</BlockLimitsEnabled>
            <EnableRemoteBlockRemoval>false</EnableRemoteBlockRemoval>
            <EnvironmentHostility>SAFE</EnvironmentHostility>
            <AutoHealing>true</AutoHealing>
            <EnableCopyPaste>true</EnableCopyPaste>
            <WeaponsEnabled>true</WeaponsEnabled>
            <ShowPlayerNamesOnHud>true</ShowPlayerNamesOnHud>
            <ThrusterDamage>true</ThrusterDamage>
            <CargoShipsEnabled>false</CargoShipsEnabled>
            <EnableSpectator>false</EnableSpectator>
            <WorldSizeKm>0</WorldSizeKm>
            <RespawnShipDelete>false</RespawnShipDelete>
            <ResetOwnership>false</ResetOwnership>
            <WelderSpeedMultiplier>1</WelderSpeedMultiplier>
            <GrinderSpeedMultiplier>1</GrinderSpeedMultiplier>
            <RealisticSound>false</RealisticSound>
            <HackSpeedMultiplier>1</HackSpeedMultiplier>
            <PermanentDeath>false</PermanentDeath>
            <AutoSaveInMinutes>0</AutoSaveInMinutes>
            <EnableSaving>true</EnableSaving>
            <InfiniteAmmo>false</InfiniteAmmo>
            <EnableContainerDrops>false</EnableContainerDrops>
            <SpawnShipTimeMultiplier>0</SpawnShipTimeMultiplier>
            <ProceduralDensity>0</ProceduralDensity>
            <ProceduralSeed>0</ProceduralSeed>
            <DestructibleBlocks>true</DestructibleBlocks>
            <EnableIngameScripts>true</EnableIngameScripts>
            <ViewDistance>15000</ViewDistance>
            <EnableToolShake>true</EnableToolShake>
            <VoxelGeneratorVersion>4</VoxelGeneratorVersion>
            <EnableOxygen>true</EnableOxygen>
            <EnableOxygenPressurization>true</EnableOxygenPressurization>
            <Enable3rdPersonView>true</Enable3rdPersonView>
            <EnableEncounters>false</EnableEncounters>
            <EnableConvertToStation>true</EnableConvertToStation>
            <StationVoxelSupport>false</StationVoxelSupport>
            <EnableSunRotation>true</EnableSunRotation>
            <EnableRespawnShips>true</EnableRespawnShips>
            <ScenarioEditMode>false</ScenarioEditMode>
            <Scenario>false</Scenario>
            <UpdateRespawnDictionary>false</UpdateRespawnDictionary>
            <CanJoinRunning>false</CanJoinRunning>
            <PhysicsIterations>8</PhysicsIterations>
            <SunRotationIntervalMinutes>120</SunRotationIntervalMinutes>
            <EnableJetpack>true</EnableJetpack>
            <SpawnWithTools>true</SpawnWithTools>
            <BlueprintShareTimeout>30</BlueprintShareTimeout>
            <BlueprintShare>true</BlueprintShare>
            <StartInRespawnScreen>false</StartInRespawnScreen>
            <EnableVoxelDestruction>true</EnableVoxelDestruction>
            <MaxDrones>5</MaxDrones>
            <EnableDrones>true</EnableDrones>
            <EnableWolfs>false</EnableWolfs>
            <EnableSpiders>false</EnableSpiders>
            <FloraDensityMultiplier>1</FloraDensityMultiplier>
            <BlockTypeLimits><dictionary /></BlockTypeLimits>
            <EnableScripterRole>false</EnableScripterRole>
            <MinDropContainerRespawnTime>5</MinDropContainerRespawnTime>
            <MaxDropContainerRespawnTime>20</MaxDropContainerRespawnTime>
            <EnableTurretsFriendlyFire>false</EnableTurretsFriendlyFire>
            <EnableSubgridDamage>false</EnableSubgridDamage>
            <SyncDistance>3000</SyncDistance>
            <ExperimentalMode>false</ExperimentalMode>
            <AdaptiveSimulationQuality>true</AdaptiveSimulationQuality>
            <EnableVoxelHand>true</EnableVoxelHand>
            <RemoveOldIdentitiesH>0</RemoveOldIdentitiesH>
            <TrashRemovalEnabled>true</TrashRemovalEnabled>
            <StopGridsPeriodMin>15</StopGridsPeriodMin>
            <TrashFlagsValue>0</TrashFlagsValue>
            <AFKTimeountMin>0</AFKTimeountMin>
            <BlockCountThreshold>20</BlockCountThreshold>
            <PlayerDistanceThreshold>500</PlayerDistanceThreshold>
            <OptimalGridCount>0</OptimalGridCount>
            <PlayerInactivityThreshold>0</PlayerInactivityThreshold>
            <PlayerCharacterRemovalThreshold>15</PlayerCharacterRemovalThreshold>
            <VoxelTrashRemovalEnabled>false</VoxelTrashRemovalEnabled>
            <VoxelPlayerDistanceThreshold>5000</VoxelPlayerDistanceThreshold>
            <VoxelGridDistanceThreshold>5000</VoxelGridDistanceThreshold>
            <VoxelAgeThreshold>24</VoxelAgeThreshold>
            <EnableResearch>false</EnableResearch>
            <EnableGoodBotHints>false</EnableGoodBotHints>
            <OptimalSpawnDistance>16000</OptimalSpawnDistance>
            <EnableAutorespawn>true</EnableAutorespawn>
            <EnableBountyContracts>true</EnableBountyContracts>
            <EnableSupergridding>false</EnableSupergridding>
            <EnableEconomy>false</EnableEconomy>
            <DepositsCountCoefficient>1</DepositsCountCoefficient>
            <DepositSizeDenominator>1</DepositSizeDenominator>
            <WeatherSystem>false</WeatherSystem>
            <WeatherLightingDamage>false</WeatherLightingDamage>
            <HarvestRatioMultiplier>1</HarvestRatioMultiplier>
            <TradeFactionsCount>0</TradeFactionsCount>
            <StationsDistanceInnerRadius>0</StationsDistanceInnerRadius>
            <StationsDistanceOuterRadiusStart>0</StationsDistanceOuterRadiusStart>
            <StationsDistanceOuterRadiusEnd>0</StationsDistanceOuterRadiusEnd>
            <EconomyTickInSeconds>1200</EconomyTickInSeconds>
            <NPCGridClaimTimeLimit>120</NPCGridClaimTimeLimit>
            <SimplifiedSimulation>false</SimplifiedSimulation>
            <EnablePcuTrading>true</EnablePcuTrading>
            <FamilySharing>true</FamilySharing>
            <EnableSelectivePhysicsUpdates>false</EnableSelectivePhysicsUpdates>
            <PredefinedAsteroids>true</PredefinedAsteroids>
            <UseConsolePCU>false</UseConsolePCU>
            <MaxPlanets>99</MaxPlanets>
            <OffensiveWordsFiltering>false</OffensiveWordsFiltering>
            <AdjustableMaxVehicleSpeed>true</AdjustableMaxVehicleSpeed>
            <EnableMatchComponent>false</EnableMatchComponent>
            <PreMatchDuration>0</PreMatchDuration>
            <MatchDuration>0</MatchDuration>
            <PostMatchDuration>0</PostMatchDuration>
            <EnableFriendlyFire>true</EnableFriendlyFire>
            <EnableTeamBalancing>false</EnableTeamBalancing>
            <CharacterSpeedMultiplier>1</CharacterSpeedMultiplier>
            <EnableRecoil>true</EnableRecoil>
            <EnvironmentDamageMultiplier>1</EnvironmentDamageMultiplier>
            <EnableGamepadAimAssist>false</EnableGamepadAimAssist>
            <BackpackDespawnTimer>5</BackpackDespawnTimer>
            <EnableFactionPlayerNames>false</EnableFactionPlayerNames>
            <EnableTeamScoreCounters>true</EnableTeamScoreCounters>
            <EnableSpaceSuitRespawn>true</EnableSpaceSuitRespawn>
            <MatchRestartWhenEmptyTime>0</MatchRestartWhenEmptyTime>
            <EnableFactionVoiceChat>false</EnableFactionVoiceChat>
            <EnableOrca>true</EnableOrca>
            <MaxProductionQueueLength>50</MaxProductionQueueLength>
            <PrefetchShapeRayLengthLimit>15000</PrefetchShapeRayLengthLimit>
            <EnemyTargetIndicatorDistance>20</EnemyTargetIndicatorDistance>
            <EnableTrashSettingsPlatformOverride>true</EnableTrashSettingsPlatformOverride>
            <MinimumWorldSize>125</MinimumWorldSize>
            <MaxCargoBags>100</MaxCargoBags>
            <TrashCleanerCargoBagsMaxLiveTime>30</TrashCleanerCargoBagsMaxLiveTime>
            <ScrapEnabled>true</ScrapEnabled>
            <BroadcastControllerMaxOfflineTransmitDistance>200</BroadcastControllerMaxOfflineTransmitDistance>
            <TemporaryContainers>true</TemporaryContainers>
            <GlobalEncounterTimer>15</GlobalEncounterTimer>
            <GlobalEncounterCap>1</GlobalEncounterCap>
            <GlobalEncounterEnableRemovalTimer>true</GlobalEncounterEnableRemovalTimer>
            <GlobalEncounterMinRemovalTimer>90</GlobalEncounterMinRemovalTimer>
            <GlobalEncounterMaxRemovalTimer>180</GlobalEncounterMaxRemovalTimer>
            <GlobalEncounterRemovalTimeClock>30</GlobalEncounterRemovalTimeClock>
            <EncounterDensity>0</EncounterDensity>
            <EncounterGeneratorVersion>6</EncounterGeneratorVersion>
            <EnablePlanetaryEncounters>false</EnablePlanetaryEncounters>
            <PlanetaryEncounterTimerMin>15</PlanetaryEncounterTimerMin>
            <PlanetaryEncounterTimerMax>30</PlanetaryEncounterTimerMax>
            <PlanetaryEncounterTimerFirst>5</PlanetaryEncounterTimerFirst>
            <PlanetaryEncounterExistingStructuresRange>7000</PlanetaryEncounterExistingStructuresRange>
            <PlanetaryEncounterAreaLockdownRange>10000</PlanetaryEncounterAreaLockdownRange>
            <PlanetaryEncounterDesiredSpawnRange>6000</PlanetaryEncounterDesiredSpawnRange>
            <PlanetaryEncounterPresenceRange>20000</PlanetaryEncounterPresenceRange>
            <PlanetaryEncounterDespawnTimeout>120</PlanetaryEncounterDespawnTimeout>
            <LimitBlocksBy>BlockPairName</LimitBlocksBy>
        </MyObjectBuilder_SessionSettings>"#;
        
        let settings: MyObjectBuilder_SessionSettings = quick_xml::de::from_str(settings_xml)
            .expect("Failed to parse session settings XML");
        
        // Verify basic fields
        assert_eq!(settings.game_mode, MyGameModeEnum::Survival);
        assert_eq!(settings.max_players, 1);
        assert_eq!(settings.view_distance, 15000);
    }

    #[test]
    fn test_parse_world_sample() {
        // Test the full world sample
        let xml = include_str!("../test_data/world_sample.xml");
        
        println!("Parsing full world sample ({} bytes)...", xml.len());
        
        let world: MyObjectBuilder_World = match quick_xml::de::from_str(xml) {
            Ok(w) => {
                println!("SUCCESS!");
                w
            }
            Err(e) => {
                println!("FAILED: {e:?}");
                
                // Try to get the position where we failed
                // quick-xml may give us byte position in the error
                let error_str = format!("{:?}", e);
                println!("\nError details: {}", error_str);
                
                panic!("Failed to parse world: {e}");
            }
        };

        // Checkpoint basics
        assert_eq!(world.checkpoint.session_name, "Empty World");
        assert_eq!(world.checkpoint.app_version, 1208015);
        assert!(!world.checkpoint.spectator_is_light_on);
        
        // Settings
        assert_eq!(world.checkpoint.settings.game_mode, MyGameModeEnum::Survival);
        assert_eq!(world.checkpoint.settings.inventory_size_multiplier, 3.0);
        assert_eq!(world.checkpoint.settings.online_mode, MyOnlineModeEnum::PUBLIC);
        assert_eq!(world.checkpoint.settings.max_players, 4);
        assert_eq!(world.checkpoint.settings.view_distance, 15000);
        assert!(world.checkpoint.settings.enable_jetpack);
        assert!(!world.checkpoint.settings.enable_ingame_scripts);
        
        // Factions
        assert!(world.checkpoint.factions.factions.len() >= 2, "Expected at least 2 factions");
        let pirates = &world.checkpoint.factions.factions[0];
        assert_eq!(pirates.tag, "SPRT");
        assert_eq!(pirates.name, "Space Pirates");
        assert_eq!(pirates.faction_type, MyFactionTypes::Pirate);
        assert!(!pirates.stations.is_empty(), "Pirates should have stations");
    }
}