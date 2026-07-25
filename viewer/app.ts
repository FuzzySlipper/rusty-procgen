import {
  ASHA_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS,
  mountAshaRendererInspectionSurface,
  type AshaRendererInspectionSurface,
} from '@asha/renderer-host';

import {
  compilePlacementExtrusion,
  type PiecePlacementPolicy,
  type VoxelExtrusionPlan,
} from '../src/voxel-extrusion.js';
import {
  buildVoxelInspectionProjection,
  type VoxelInspectionProjection,
} from '../src/voxel-inspection-projection.js';

interface AcceptedArtifact {
  readonly artifactId: string;
  readonly candidateHash: string;
  readonly layoutHash: string;
  readonly validationRef: string;
  readonly scoreRef: string;
  readonly candidate: CandidateArtifact;
  readonly layout: LayoutArtifact;
  readonly scoreSummary: ScoreReport;
}

interface CandidateArtifact {
  readonly candidateId: string;
  readonly provenance: readonly ProvenanceStep[];
}

interface ProvenanceStep {
  readonly step: number;
  readonly command: string;
  readonly seed: number | null;
  readonly summary: string;
}

interface LayoutArtifact {
  readonly layoutId: string;
  readonly candidateId: string;
  readonly rooms: readonly LayoutRoom[];
  readonly links: readonly LayoutLink[];
}

interface LayoutRoom {
  readonly nodeId: string;
  readonly kind: string;
  readonly label: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

interface LayoutLink {
  readonly edgeId: string;
  readonly fromNode: string;
  readonly toNode: string;
  readonly kind: string;
  readonly traversal: string;
  readonly requiredItem: string | null;
}

interface ScoreReport {
  readonly overall: number;
  readonly metrics: Record<string, number>;
}

interface SelectionReport {
  readonly batchId: string;
  readonly profileId?: string;
  readonly profileRef?: string;
  readonly requestedCount: number;
  readonly generatedCount: number;
  readonly accepted: readonly SelectionEntry[];
  readonly rejected: readonly SelectionRejection[];
}

interface SelectionEntry {
  readonly candidateId: string;
  readonly profileSequence?: string;
  readonly artifactRef: string;
  readonly validationRef: string;
  readonly scoreRef: string;
  readonly layoutRef: string;
  readonly analysisRef?: string;
  readonly compatibleRulesRef?: string;
  readonly spatialIntentRef?: string;
  readonly intermediateBreakdownRef?: string;
  readonly intermediateValidationRef?: string;
  readonly physicalConnectionPlanRef?: string;
  readonly geometryRef?: string;
  readonly geometryValidationRef?: string;
  readonly htmlPreviewRef?: string;
  readonly htmlRef?: string;
  readonly shapeCatalogRef?: string;
  readonly catalogInspectionRef?: string;
  readonly piecePlanRef?: string;
  readonly shapeMatchRef?: string;
  readonly piecePlacementRef?: string;
  readonly piecePlacementValidationRef?: string;
  readonly builtFlowValidationRef?: string;
  readonly overall: number;
  readonly metrics: Record<string, number>;
  readonly tags: readonly string[];
}

interface SelectionRejection {
  readonly candidateId: string;
  readonly profileSequence?: string;
  readonly candidateRef: string;
  readonly physicalConnectionPlanRef?: string;
  readonly diagnostics: readonly Diagnostic[];
}

interface ValidationReport {
  readonly ok: boolean;
  readonly fatalCount: number;
  readonly diagnostics: readonly Diagnostic[];
}

interface BuiltFlowValidationReport extends ValidationReport {
  readonly kind: 'asha_procgen.validation.built_flow.v1';
  readonly placementId: string;
  readonly portalCount: number;
  readonly progression: readonly BuiltFlowProgressionStep[];
}

interface BuiltFlowProgressionStep {
  readonly step: number;
  readonly items: readonly string[];
  readonly reachableNodes: readonly string[];
  readonly reachableEdges: readonly string[];
  readonly openPortals: readonly string[];
}

interface Diagnostic {
  readonly code: string;
  readonly severity: string;
  readonly node?: string | null;
  readonly edge?: string | null;
  readonly detail: string;
  readonly repairHint?: string;
}

interface SpatialIntentReport {
  readonly annotations: readonly SpatialIntentAnnotation[];
}

interface SpatialIntentAnnotation {
  readonly targetType: string;
  readonly targetId: string;
  readonly intents: readonly string[];
}

interface IntermediateBreakdown {
  readonly schemaVersion: number;
  readonly regions: readonly IntermediateRegion[];
  readonly connectors: readonly IntermediateConnector[];
  readonly constraints: readonly IntermediateConstraint[];
}

interface IntermediateRegion {
  readonly id: string;
  readonly nodeIds?: readonly string[];
  readonly role: string;
  readonly anchorNode?: string | null;
  readonly geometryRole?: string;
  readonly footprintClass?: string;
  readonly scaleBand?: string;
  readonly anchorQuality?: string;
  readonly entranceExpectations?: readonly string[];
}

interface IntermediateConnector {
  readonly id: string;
  readonly edgeId: string;
  readonly fromRegion: string;
  readonly toRegion: string;
  readonly intents: readonly string[];
  readonly affordances?: readonly string[];
  readonly constraintRefs?: readonly string[];
}

interface IntermediateConstraint {
  readonly code: string;
  readonly target: string;
}

interface IntermediateContext {
  readonly spatialIntent: SpatialIntentReport | null;
  readonly breakdown: IntermediateBreakdown | null;
  readonly validation: ValidationReport | null;
}

interface Geometry2dArtifact {
  readonly geometryId: string;
  readonly candidateId: string;
  readonly sourceConnectionPlanRef: string;
  readonly connectionPlanId: string;
  readonly layoutPolicy: GeometryLayoutPolicy;
  readonly layoutSearch: GeometryLayoutSearchEvidence;
  readonly bounds: GeometryBounds;
  readonly rooms: readonly GeometryRoom[];
  readonly corridors: readonly GeometryCorridor[];
  readonly contents: readonly GeometryContent[];
}

interface GeometryLayoutPolicy {
  readonly kind: 'asha_procgen.geometry_layout_policy.v1';
  readonly schemaVersion: 1;
  readonly initialRoomMargin: number;
  readonly initialColumnGap: number;
  readonly initialRowGap: number;
  readonly roomMarginGrowth: number;
  readonly columnGapGrowth: number;
  readonly rowGapGrowth: number;
  readonly maxSpacingTiers: number;
  readonly roomOrderAttemptsPerTier: number;
  readonly maxSearchAttempts: number;
}

interface GeometryLayoutSearchEvidence {
  readonly spacingTier: number;
  readonly roomOrderAttempt: number;
  readonly routeOrderAttempt: number;
  readonly searchAttempts: number;
  readonly effectiveSpacing: {
    readonly roomMargin: number;
    readonly columnGap: number;
    readonly rowGap: number;
  };
}

interface GeometryBounds {
  readonly width: number;
  readonly height: number;
  readonly grid: number;
}

interface GeometryRoom {
  readonly id: string;
  readonly sourceRegion: string;
  readonly sourceNodes: readonly string[];
  readonly role: string;
  readonly geometryRole: string;
  readonly footprintClass: string;
  readonly rect: GeometryRect;
  readonly ports: readonly GeometryRoomPort[];
  readonly styleTags: readonly string[];
}

interface GeometryRoomPort {
  readonly id: string;
  readonly sectionId: string;
  readonly side: 'north' | 'east' | 'south' | 'west';
  readonly point: GeometryPoint;
  readonly width: number;
}

interface GeometryRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

interface GeometryCorridor {
  readonly id: string;
  readonly physicalSection: string;
  readonly sourceConnector: string;
  readonly sourceEdge: string;
  readonly sourceConnectors: readonly string[];
  readonly sourceEdges: readonly string[];
  readonly traversalRefs: readonly PhysicalTraversalRef[];
  readonly fromRoom: string;
  readonly toRoom: string;
  readonly traversalHint: string;
  readonly semanticTags: readonly string[];
  readonly width: number;
  readonly fromPort: string;
  readonly toPort: string;
  readonly points: readonly GeometryPoint[];
}

interface PhysicalTraversalRef {
  readonly connectorId: string;
  readonly edgeId: string;
  readonly fromRegion: string;
  readonly toRegion: string;
  readonly traversal: string;
  readonly requiredItem: string | null;
}

interface GeometryPoint {
  readonly x: number;
  readonly y: number;
}

interface GeometryContent {
  readonly id: string;
  readonly roomId: string;
  readonly sourceRef: string;
  readonly kind: string;
  readonly label: string;
  readonly tags: readonly string[];
}

interface ShapeCatalog {
  readonly kind: string;
  readonly catalogId: string;
  readonly cellSize: number;
  readonly placementPolicy: PiecePlacementPolicy;
  readonly catalogSearchPolicy?: CatalogSearchPolicy;
  readonly shapes: readonly CatalogShape[];
}

interface CatalogSearchPolicy {
  readonly schemaVersion: 1;
  readonly maxDecisions: number;
  readonly maxBacktracks: number;
  readonly maxChainExpansionsPerSection: number;
  readonly maxRoomOriginAlternatives: number;
  readonly maxRoomRotationAlternatives: number;
}

interface CatalogShape {
  readonly shapeId: string;
  readonly label: string;
  readonly pieceKinds: readonly string[];
  readonly footprint: readonly GridCell[];
  readonly reservedCells: readonly GridCell[];
  readonly exits: readonly CatalogExit[];
  readonly allowedTransforms: readonly string[];
  readonly featureSockets: readonly CatalogSocket[];
  readonly tags: readonly string[];
}

interface CatalogExit {
  readonly id: string;
  readonly x: number;
  readonly y: number;
  readonly direction: string;
  readonly width: number;
  readonly tags: readonly string[];
}

interface CatalogSocket {
  readonly id: string;
  readonly kind: string;
  readonly x: number;
  readonly y: number;
  readonly tags: readonly string[];
}

interface PiecePlacement {
  readonly kind: string;
  readonly placementId: string;
  readonly planId: string;
  readonly catalogId: string;
  readonly matchId: string;
  readonly corridorRealization?: CorridorRealization;
  readonly sourceCatalogRef?: string;
  readonly cellSize: number;
  readonly gridConnectivity: 'four_way' | 'eight_way';
  readonly placementPolicy: PiecePlacementPolicy;
  readonly catalogSearch?: {
    readonly schemaVersion: 1;
    readonly decisions: number;
    readonly backtracks: number;
    readonly chainExpansions: number;
    readonly selected: readonly unknown[];
  };
  readonly instances: readonly PieceInstance[];
  readonly gluedExits: readonly GluedExit[];
  readonly gatePortals: readonly GatePortal[];
  readonly occupiedCells: readonly PlacementCellRef[];
  readonly connectionCells: readonly PlacementCellRef[];
  readonly reservedCells: readonly PlacementCellRef[];
  readonly danglingExits: readonly DanglingExit[];
}

interface PieceInstance {
  readonly instanceId: string;
  readonly pieceId: string;
  readonly requirementKind: string;
  readonly role: string;
  readonly shapeId: string;
  readonly transform: string;
  readonly origin: GridCell;
  readonly occupiedCells: readonly GridCell[];
  readonly reservedCells: readonly GridCell[];
  readonly exitMap: readonly MatchedExit[];
  readonly featurePlacements: readonly MatchedSocket[];
  readonly sourceRequirementRef: string;
  readonly sourceRefs: readonly string[];
  readonly tags: readonly string[];
}

interface GridCell {
  readonly x: number;
  readonly y: number;
}

interface PlacementCellRef {
  readonly instanceId: string;
  readonly x: number;
  readonly y: number;
}

interface GluedExit {
  readonly id: string;
  readonly linkId: string;
  readonly fromInstance: string;
  readonly fromExit: string;
  readonly fromCell: GridCell;
  readonly fromDirection: 'north' | 'east' | 'south' | 'west';
  readonly fromWidth: number;
  readonly toInstance: string;
  readonly toExit: string;
  readonly toCell: GridCell;
  readonly toDirection: 'north' | 'east' | 'south' | 'west';
  readonly toWidth: number;
  readonly sourceCorridor: string;
  readonly sourceSection: string;
  readonly sourceEdge: string;
  readonly sourceEdges: readonly string[];
  readonly traversalRefs: readonly PhysicalTraversalRef[];
  readonly sourceRef: string;
  readonly traversal: string;
  readonly requiredItem: string | null;
  readonly tags: readonly string[];
}

interface GatePortal {
  readonly id: string;
  readonly sourceSection: string;
  readonly sourceEdge: string;
  readonly sourceEdges: readonly string[];
  readonly traversalRefs: readonly PhysicalTraversalRef[];
  readonly sourceCorridor: string;
  readonly linkId: string;
  readonly fromPiece: string;
  readonly fromInstance: string;
  readonly toPiece: string;
  readonly toInstance: string;
  readonly cells: readonly GridCell[];
  readonly orientation: 'north' | 'east' | 'south' | 'west';
  readonly width: number;
  readonly traversal: string;
  readonly requiredItem: string | null;
  readonly provenance: readonly string[];
}

interface DanglingExit {
  readonly instanceId: string;
  readonly exitId: string;
  readonly reason: string;
}

interface MatchedExit {
  readonly requirementExitId: string;
  readonly catalogExitId: string;
  readonly x: number;
  readonly y: number;
  readonly direction: string;
  readonly width: number;
}

interface MatchedSocket {
  readonly requiredSocket: string;
  readonly catalogSocketId: string;
  readonly kind: string;
}

interface NativeVoxelEvidence {
  readonly placementId: string;
  readonly ashaEngineCommit: string;
  readonly authority: {
    readonly voxelStateHash: string;
    readonly deterministic: boolean;
    readonly acceptedCommands: number;
    readonly rejectedCommands: number;
  };
}

interface PlacementPolicyExperimentResponse {
  readonly kind: 'asha_procgen.placement_policy_experiment.v1';
  readonly experimentId: string;
  readonly candidateId: string;
  readonly placementPolicy: PiecePlacementPolicy;
  readonly placement: PiecePlacement;
  readonly validation: ValidationReport;
  readonly persisted: false;
  readonly nativeAuthority: false;
}

interface PlacementPolicyExperimentError {
  readonly error: string;
  readonly detail: string;
  readonly evidence?: PureCatalogExhaustionEvidence;
}

interface PureCatalogExhaustionEvidence {
  readonly kind: 'asha_procgen.pure_catalog_exhaustion.v1';
  readonly schemaVersion: 1;
  readonly failure: {
    readonly reason: string;
    readonly detail: string;
    readonly pieceId: string;
    readonly requirementKind: string;
    readonly requiredEndpoints: readonly {
      readonly id: string;
      readonly direction: string;
    }[];
    readonly fixedPort?: {
      readonly neighborPieceId: string;
      readonly neighborExitId: string;
      readonly cell: { readonly x: number; readonly y: number };
      readonly direction: string;
      readonly requiredOppositeDirection: string;
      readonly offsetFromEnvelopeAnchor?: { readonly x: number; readonly y: number } | null;
    } | null;
    readonly originBounds?: {
      readonly minX: number;
      readonly maxX: number;
      readonly minY: number;
      readonly maxY: number;
    } | null;
    readonly laneEnvelope?: {
      readonly sourceHint: string;
      readonly envelopeCells: number;
      readonly bounds: {
        readonly minX: number;
        readonly maxX: number;
        readonly minY: number;
        readonly maxY: number;
      };
    } | null;
    readonly exhaustedFamilies: readonly string[];
    readonly candidateCount: number;
  };
  readonly budgets: {
    readonly maxDecisions: number;
    readonly decisions: number;
    readonly maxBacktracks: number;
    readonly backtracks: number;
    readonly maxChainExpansionsPerSection: number;
    readonly chainExpansions: number;
  };
}

interface GeometryLayoutPolicyExperimentResponse {
  readonly kind: 'asha_procgen.geometry_layout_policy_experiment.v1';
  readonly experimentId: string;
  readonly candidateId: string;
  readonly geometryLayoutPolicy: GeometryLayoutPolicy;
  readonly geometry: Geometry2dArtifact;
  readonly geometryValidation: ValidationReport;
  readonly placement: PiecePlacement;
  readonly placementValidation: ValidationReport;
  readonly builtFlowValidation: BuiltFlowValidationReport;
  readonly persisted: false;
  readonly nativeAuthority: false;
}

type CorridorRealization = 'catalog' | 'hybrid' | 'procedural';

interface CorridorRealizationExperimentResponse {
  readonly kind: 'asha_procgen.corridor_realization_experiment.v1';
  readonly experimentId: string;
  readonly candidateId: string;
  readonly corridorRealization: CorridorRealization;
  readonly placement: PiecePlacement;
  readonly placementValidation: ValidationReport;
  readonly builtFlowValidation: BuiltFlowValidationReport;
  readonly metrics: {
    readonly prefabInstances: number;
    readonly corridorPrefabInstances: number;
    readonly corridorPrefabCells: number;
    readonly routedCorridorCells: number;
    readonly footprintWidth: number;
    readonly footprintHeight: number;
  };
  readonly persisted: false;
  readonly nativeAuthority: false;
}

interface GenerationConfigSetting<T> {
  value: T;
  readonly defaultValue: T;
}

interface ViewerGenerationConfig {
  readonly kind: 'asha_procgen.viewer_generation_config.v1';
  readonly schemaVersion: 1;
  readonly geometryLayoutPolicy: {
    readonly initialRoomMargin: GenerationConfigSetting<number>;
    readonly initialColumnGap: GenerationConfigSetting<number>;
    readonly initialRowGap: GenerationConfigSetting<number>;
    readonly roomMarginGrowth: GenerationConfigSetting<number>;
    readonly columnGapGrowth: GenerationConfigSetting<number>;
    readonly rowGapGrowth: GenerationConfigSetting<number>;
    readonly maxSpacingTiers: GenerationConfigSetting<number>;
    readonly roomOrderAttemptsPerTier: GenerationConfigSetting<number>;
    readonly maxSearchAttempts: GenerationConfigSetting<number>;
  };
  readonly placementPolicy: {
    readonly minimumClearanceCells: GenerationConfigSetting<number>;
    readonly wallThicknessCells: GenerationConfigSetting<number>;
  };
  readonly catalogAwareGenerationPolicy: {
    readonly maxGenerationAttempts: GenerationConfigSetting<number>;
    readonly initialRoomSlackCells: GenerationConfigSetting<number>;
    readonly roomSlackGrowthCells: GenerationConfigSetting<number>;
    readonly maxRoomCandidates: GenerationConfigSetting<number>;
    readonly maxRoutingStatesPerSection: GenerationConfigSetting<number>;
    readonly routeMarginCells: GenerationConfigSetting<number>;
    readonly guideDistanceWeight: GenerationConfigSetting<number>;
    readonly turnPenalty: GenerationConfigSetting<number>;
  };
  readonly corridorRealization: GenerationConfigSetting<CorridorRealization>;
}

interface GenerationConfigRebuildResponse {
  readonly kind: 'asha_procgen.viewer_generation_rebuild.v1';
  readonly buildId: string;
  readonly candidateId: string;
  readonly config: ViewerGenerationConfig;
  readonly geometry: Geometry2dArtifact;
  readonly geometryValidation: ValidationReport;
  readonly placement: PiecePlacement;
  readonly placementValidation: ValidationReport;
  readonly builtFlowValidation: BuiltFlowValidationReport;
  readonly catalogAwareGeneration: {
    readonly policy: Record<string, unknown>;
    readonly attempts: readonly Record<string, unknown>[];
    readonly selectedAttempt: number;
  } | null;
  readonly metrics: CorridorRealizationExperimentResponse['metrics'];
  readonly persisted: true;
  readonly nativeAuthority: false;
}

const svg = document.querySelector<SVGSVGElement>('#layout');
const summary = document.querySelector<HTMLElement>('#summary');
const batchList = document.querySelector<HTMLElement>('#batch-list');
const diagnostics = document.querySelector<HTMLElement>('#diagnostics');
const viewTabs = document.querySelectorAll<HTMLButtonElement>('[data-view]');
const voxel3dPanel = document.querySelector<HTMLElement>('#voxel-3d-panel');
const voxel3dCanvas = document.querySelector<HTMLCanvasElement>('#voxel-3d-canvas');
const voxel3dDiagnostic = document.querySelector<HTMLElement>('#voxel-3d-diagnostic');
const voxel3dDoorState = document.querySelector<HTMLSelectElement>('#voxel-3d-door-state');
const voxel3dDoorLegend = document.querySelector<HTMLElement>('#voxel-3d-door-legend');
const generationConfigPanelElement = document.querySelector<HTMLElement>('#generation-config-panel');
const generationConfigFormElement = document.querySelector<HTMLFormElement>('#generation-config-form');
const generationConfigModeElement = document.querySelector<HTMLElement>('#generation-config-mode');
const generationConfigApplyElement = document.querySelector<HTMLButtonElement>('#generation-config-apply');
const generationConfigResetElement = document.querySelector<HTMLButtonElement>('#generation-config-reset');
const generationConfigValidationElement = document.querySelector<HTMLElement>('#generation-config-validation');
const generationConfigImpactElement = document.querySelector<HTMLElement>('#generation-config-impact');
const generationConfigStatusElement = document.querySelector<HTMLElement>('#generation-config-status');
const generationConfigInitialMarginElement = document.querySelector<HTMLInputElement>('#generation-config-initial-margin');
const generationConfigInitialColumnGapElement = document.querySelector<HTMLInputElement>('#generation-config-initial-column-gap');
const generationConfigInitialRowGapElement = document.querySelector<HTMLInputElement>('#generation-config-initial-row-gap');
const generationConfigMarginGrowthElement = document.querySelector<HTMLInputElement>('#generation-config-margin-growth');
const generationConfigColumnGrowthElement = document.querySelector<HTMLInputElement>('#generation-config-column-growth');
const generationConfigRowGrowthElement = document.querySelector<HTMLInputElement>('#generation-config-row-growth');
const generationConfigMaxTiersElement = document.querySelector<HTMLInputElement>('#generation-config-max-tiers');
const generationConfigRoomAttemptsElement = document.querySelector<HTMLInputElement>('#generation-config-room-attempts');
const generationConfigMaxAttemptsElement = document.querySelector<HTMLInputElement>('#generation-config-max-attempts');
const generationConfigClearanceElement = document.querySelector<HTMLInputElement>('#generation-config-clearance');
const generationConfigWallThicknessElement = document.querySelector<HTMLInputElement>('#generation-config-wall-thickness');
const generationConfigCorridorRealizationElement = document.querySelector<HTMLSelectElement>('#generation-config-corridor-realization');
const generationConfigCatalogAttemptsElement = document.querySelector<HTMLInputElement>('#generation-config-catalog-attempts');
const generationConfigCatalogInitialSlackElement = document.querySelector<HTMLInputElement>('#generation-config-catalog-initial-slack');
const generationConfigCatalogSlackGrowthElement = document.querySelector<HTMLInputElement>('#generation-config-catalog-slack-growth');
const generationConfigCatalogRoomCandidatesElement = document.querySelector<HTMLInputElement>('#generation-config-catalog-room-candidates');
const generationConfigCatalogRouteStatesElement = document.querySelector<HTMLInputElement>('#generation-config-catalog-route-states');
const generationConfigCatalogRouteMarginElement = document.querySelector<HTMLInputElement>('#generation-config-catalog-route-margin');
const generationConfigCatalogGuideWeightElement = document.querySelector<HTMLInputElement>('#generation-config-catalog-guide-weight');
const generationConfigCatalogTurnPenaltyElement = document.querySelector<HTMLInputElement>('#generation-config-catalog-turn-penalty');
const geometryPolicyPanelElement = document.querySelector<HTMLElement>('#geometry-policy-panel');
const geometryPolicyFormElement = document.querySelector<HTMLFormElement>('#geometry-policy-form');
const geometryPolicyInitialMarginElement = document.querySelector<HTMLInputElement>('#geometry-policy-initial-margin');
const geometryPolicyInitialColumnGapElement = document.querySelector<HTMLInputElement>('#geometry-policy-initial-column-gap');
const geometryPolicyInitialRowGapElement = document.querySelector<HTMLInputElement>('#geometry-policy-initial-row-gap');
const geometryPolicyMarginGrowthElement = document.querySelector<HTMLInputElement>('#geometry-policy-margin-growth');
const geometryPolicyColumnGrowthElement = document.querySelector<HTMLInputElement>('#geometry-policy-column-growth');
const geometryPolicyRowGrowthElement = document.querySelector<HTMLInputElement>('#geometry-policy-row-growth');
const geometryPolicyMaxTiersElement = document.querySelector<HTMLInputElement>('#geometry-policy-max-tiers');
const geometryPolicyRoomAttemptsElement = document.querySelector<HTMLInputElement>('#geometry-policy-room-attempts');
const geometryPolicyMaxAttemptsElement = document.querySelector<HTMLInputElement>('#geometry-policy-max-attempts');
const geometryPolicyApplyElement = document.querySelector<HTMLButtonElement>('#geometry-policy-apply');
const geometryPolicyResetElement = document.querySelector<HTMLButtonElement>('#geometry-policy-reset');
const geometryPolicyModeElement = document.querySelector<HTMLElement>('#geometry-policy-mode');
const geometryPolicyBudgetElement = document.querySelector<HTMLElement>('#geometry-policy-budget');
const geometryPolicyValidationElement = document.querySelector<HTMLElement>('#geometry-policy-validation');
const geometryPolicyImpactElement = document.querySelector<HTMLElement>('#geometry-policy-impact');
const geometryPolicyStatusElement = document.querySelector<HTMLElement>('#geometry-policy-status');
const geometryPolicyPresetsElements = document.querySelectorAll<HTMLButtonElement>('[data-geometry-policy-preset]');
const corridorRealizationPanelElement = document.querySelector<HTMLElement>('#corridor-realization-panel');
const corridorRealizationFormElement = document.querySelector<HTMLFormElement>('#corridor-realization-form');
const corridorRealizationSelectElement = document.querySelector<HTMLSelectElement>('#corridor-realization-select');
const corridorRealizationApplyElement = document.querySelector<HTMLButtonElement>('#corridor-realization-apply');
const corridorRealizationResetElement = document.querySelector<HTMLButtonElement>('#corridor-realization-reset');
const corridorRealizationModeElement = document.querySelector<HTMLElement>('#corridor-realization-mode');
const corridorRealizationImpactElement = document.querySelector<HTMLElement>('#corridor-realization-impact');
const corridorRealizationStatusElement = document.querySelector<HTMLElement>('#corridor-realization-status');
const policyPanel = document.querySelector<HTMLElement>('#placement-policy-panel');
const policyForm = document.querySelector<HTMLFormElement>('#placement-policy-form');
const policyClearance = document.querySelector<HTMLInputElement>('#placement-policy-clearance');
const policyWallThickness = document.querySelector<HTMLInputElement>('#placement-policy-wall-thickness');
const policyApply = document.querySelector<HTMLButtonElement>('#placement-policy-apply');
const policyReset = document.querySelector<HTMLButtonElement>('#placement-policy-reset');
const policyMode = document.querySelector<HTMLElement>('#placement-policy-mode');
const policyValidation = document.querySelector<HTMLElement>('#placement-policy-validation');
const policyImpact = document.querySelector<HTMLElement>('#placement-policy-impact');
const policyStatus = document.querySelector<HTMLElement>('#placement-policy-status');
const policyPresets = document.querySelectorAll<HTMLButtonElement>('[data-policy-preset]');

if (
  svg === null
  || summary === null
  || batchList === null
  || diagnostics === null
  || voxel3dPanel === null
  || voxel3dCanvas === null
  || voxel3dDiagnostic === null
  || voxel3dDoorState === null
  || voxel3dDoorLegend === null
  || generationConfigPanelElement === null
  || generationConfigFormElement === null
  || generationConfigModeElement === null
  || generationConfigApplyElement === null
  || generationConfigResetElement === null
  || generationConfigValidationElement === null
  || generationConfigImpactElement === null
  || generationConfigStatusElement === null
  || generationConfigInitialMarginElement === null
  || generationConfigInitialColumnGapElement === null
  || generationConfigInitialRowGapElement === null
  || generationConfigMarginGrowthElement === null
  || generationConfigColumnGrowthElement === null
  || generationConfigRowGrowthElement === null
  || generationConfigMaxTiersElement === null
  || generationConfigRoomAttemptsElement === null
  || generationConfigMaxAttemptsElement === null
  || generationConfigClearanceElement === null
  || generationConfigWallThicknessElement === null
  || generationConfigCorridorRealizationElement === null
  || generationConfigCatalogAttemptsElement === null
  || generationConfigCatalogInitialSlackElement === null
  || generationConfigCatalogSlackGrowthElement === null
  || generationConfigCatalogRoomCandidatesElement === null
  || generationConfigCatalogRouteStatesElement === null
  || generationConfigCatalogRouteMarginElement === null
  || generationConfigCatalogGuideWeightElement === null
  || generationConfigCatalogTurnPenaltyElement === null
  || geometryPolicyPanelElement === null
  || geometryPolicyFormElement === null
  || geometryPolicyInitialMarginElement === null
  || geometryPolicyInitialColumnGapElement === null
  || geometryPolicyInitialRowGapElement === null
  || geometryPolicyMarginGrowthElement === null
  || geometryPolicyColumnGrowthElement === null
  || geometryPolicyRowGrowthElement === null
  || geometryPolicyMaxTiersElement === null
  || geometryPolicyRoomAttemptsElement === null
  || geometryPolicyMaxAttemptsElement === null
  || geometryPolicyApplyElement === null
  || geometryPolicyResetElement === null
  || geometryPolicyModeElement === null
  || geometryPolicyBudgetElement === null
  || geometryPolicyValidationElement === null
  || geometryPolicyImpactElement === null
  || geometryPolicyStatusElement === null
  || corridorRealizationPanelElement === null
  || corridorRealizationFormElement === null
  || corridorRealizationSelectElement === null
  || corridorRealizationApplyElement === null
  || corridorRealizationResetElement === null
  || corridorRealizationModeElement === null
  || corridorRealizationImpactElement === null
  || corridorRealizationStatusElement === null
  || policyPanel === null
  || policyForm === null
  || policyClearance === null
  || policyWallThickness === null
  || policyApply === null
  || policyReset === null
  || policyMode === null
  || policyValidation === null
  || policyImpact === null
  || policyStatus === null
) {
  throw new Error('viewer mount elements are missing');
}

type ViewMode = 'layout' | 'intermediate' | 'build' | 'voxel' | 'voxel3d' | 'catalog';

const layoutSvg = svg;
const summaryPanel = summary;
const batchPanel = batchList;
const diagnosticsPanel = diagnostics;
const voxelInspectionPanel = voxel3dPanel;
const voxelInspectionCanvas = voxel3dCanvas;
const voxelInspectionDiagnostic = voxel3dDiagnostic;
const voxelDoorStateControl = voxel3dDoorState;
const voxelDoorLegend = voxel3dDoorLegend;
const generationConfigPanel = generationConfigPanelElement;
const generationConfigForm = generationConfigFormElement;
const generationConfigMode = generationConfigModeElement;
const generationConfigApply = generationConfigApplyElement;
const generationConfigReset = generationConfigResetElement;
const generationConfigValidation = generationConfigValidationElement;
const generationConfigImpact = generationConfigImpactElement;
const generationConfigStatus = generationConfigStatusElement;
const generationConfigInitialMargin = generationConfigInitialMarginElement;
const generationConfigInitialColumnGap = generationConfigInitialColumnGapElement;
const generationConfigInitialRowGap = generationConfigInitialRowGapElement;
const generationConfigMarginGrowth = generationConfigMarginGrowthElement;
const generationConfigColumnGrowth = generationConfigColumnGrowthElement;
const generationConfigRowGrowth = generationConfigRowGrowthElement;
const generationConfigMaxTiers = generationConfigMaxTiersElement;
const generationConfigRoomAttempts = generationConfigRoomAttemptsElement;
const generationConfigMaxAttempts = generationConfigMaxAttemptsElement;
const generationConfigClearance = generationConfigClearanceElement;
const generationConfigWallThickness = generationConfigWallThicknessElement;
const generationConfigCorridorRealization = generationConfigCorridorRealizationElement;
const generationConfigCatalogAttempts = generationConfigCatalogAttemptsElement;
const generationConfigCatalogInitialSlack = generationConfigCatalogInitialSlackElement;
const generationConfigCatalogSlackGrowth = generationConfigCatalogSlackGrowthElement;
const generationConfigCatalogRoomCandidates = generationConfigCatalogRoomCandidatesElement;
const generationConfigCatalogRouteStates = generationConfigCatalogRouteStatesElement;
const generationConfigCatalogRouteMargin = generationConfigCatalogRouteMarginElement;
const generationConfigCatalogGuideWeight = generationConfigCatalogGuideWeightElement;
const generationConfigCatalogTurnPenalty = generationConfigCatalogTurnPenaltyElement;
const geometryPolicyPanel = geometryPolicyPanelElement;
const geometryPolicyForm = geometryPolicyFormElement;
const geometryPolicyInitialMargin = geometryPolicyInitialMarginElement;
const geometryPolicyInitialColumnGap = geometryPolicyInitialColumnGapElement;
const geometryPolicyInitialRowGap = geometryPolicyInitialRowGapElement;
const geometryPolicyMarginGrowth = geometryPolicyMarginGrowthElement;
const geometryPolicyColumnGrowth = geometryPolicyColumnGrowthElement;
const geometryPolicyRowGrowth = geometryPolicyRowGrowthElement;
const geometryPolicyMaxTiers = geometryPolicyMaxTiersElement;
const geometryPolicyRoomAttempts = geometryPolicyRoomAttemptsElement;
const geometryPolicyMaxAttempts = geometryPolicyMaxAttemptsElement;
const geometryPolicyApply = geometryPolicyApplyElement;
const geometryPolicyReset = geometryPolicyResetElement;
const geometryPolicyMode = geometryPolicyModeElement;
const geometryPolicyBudget = geometryPolicyBudgetElement;
const geometryPolicyValidation = geometryPolicyValidationElement;
const geometryPolicyImpact = geometryPolicyImpactElement;
const geometryPolicyStatus = geometryPolicyStatusElement;
const geometryPolicyPresets = geometryPolicyPresetsElements;
const corridorRealizationPanel = corridorRealizationPanelElement;
const corridorRealizationForm = corridorRealizationFormElement;
const corridorRealizationSelect = corridorRealizationSelectElement;
const corridorRealizationApply = corridorRealizationApplyElement;
const corridorRealizationReset = corridorRealizationResetElement;
const corridorRealizationMode = corridorRealizationModeElement;
const corridorRealizationImpact = corridorRealizationImpactElement;
const corridorRealizationStatus = corridorRealizationStatusElement;
const placementPolicyPanel = policyPanel;
const placementPolicyForm = policyForm;
const placementPolicyClearance = policyClearance;
const placementPolicyWallThickness = policyWallThickness;
const placementPolicyApply = policyApply;
const placementPolicyReset = policyReset;
const placementPolicyMode = policyMode;
const placementPolicyValidation = policyValidation;
const placementPolicyImpact = policyImpact;
const placementPolicyStatus = policyStatus;
const placementPolicyPresets = policyPresets;
const batch = await fetchBatch();
const voxelEvidence = await fetchVoxelEvidence();
let persistedGenerationConfig = await fetchGenerationConfig();
const viewerSearch = new URLSearchParams(location.search);
const requestedCandidate = viewerSearch.get('candidate');
const renderInspectionOnce = viewerSearch.get('inspection') === 'once';
const initialSelection = batch.accepted.find((entry) => entry.candidateId === requestedCandidate)
  ?? batch.accepted[0]
  ?? null;
let activeView: ViewMode = initialViewMode();
let currentLayout: LayoutArtifact | null = null;
let currentIntermediate: IntermediateContext = emptyIntermediateContext();
let currentGeometry: Geometry2dArtifact | null = null;
let committedGeometry: Geometry2dArtifact | null = null;
let currentCatalog: ShapeCatalog | null = null;
let currentCatalogRef: string | null = null;
let currentCatalogError: string | null = null;
let currentSelection: SelectionEntry | null = null;
let currentPlacement: PiecePlacement | null = null;
let currentPlacementValidation: ValidationReport | null = null;
let committedPlacement: PiecePlacement | null = null;
let committedPlacementValidation: ValidationReport | null = null;
let currentBuiltFlowValidation: BuiltFlowValidationReport | null = null;
let committedBuiltFlowValidation: BuiltFlowValidationReport | null = null;
let currentPolicyExperimentId: string | null = null;
let policyExperimentRevision = 0;
let policyExperimentBusy = false;
let currentGeometryExperimentId: string | null = null;
let geometryExperimentRevision = 0;
let geometryExperimentBusy = false;
let currentCorridorExperimentId: string | null = null;
let corridorExperimentRevision = 0;
let corridorExperimentBusy = false;
let generationConfigRevision = 0;
let generationConfigBusy = false;
let configuredBuildId: string | null = null;
let voxelInspectionSurface: AshaRendererInspectionSurface | null = null;
let voxelInspectionMount: Promise<AshaRendererInspectionSurface> | null = null;
let voxelInspectionRevision = 0;
let voxelInspectionReadoutFrame: number | null = null;

voxelInspectionCanvas.addEventListener('pointerdown', () => voxelInspectionCanvas.focus());
voxelDoorStateControl.addEventListener('change', () => {
  if (activeView === 'voxel3d') {
    void renderVoxelInspection();
  }
});
generationConfigForm.addEventListener('submit', (event) => {
  event.preventDefault();
  void applyGenerationConfig();
});
generationConfigReset.addEventListener('click', () => {
  populateGenerationConfigControls(generationConfigWithDefaults(persistedGenerationConfig));
  void applyGenerationConfig();
});
for (const input of generationConfigInputs()) {
  input.addEventListener('input', validateGenerationConfigControls);
  input.addEventListener('change', validateGenerationConfigControls);
}
placementPolicyForm.addEventListener('submit', (event) => {
  event.preventDefault();
  void applyPlacementPolicyExperiment();
});
geometryPolicyForm.addEventListener('submit', (event) => {
  event.preventDefault();
  void applyGeometryPolicyExperiment();
});
corridorRealizationForm.addEventListener('submit', (event) => {
  event.preventDefault();
  void applyCorridorRealizationExperiment();
});
corridorRealizationReset.addEventListener('click', resetCorridorRealizationExperiment);
geometryPolicyReset.addEventListener('click', resetGeometryPolicyExperiment);
for (const input of geometryPolicyInputs()) {
  input.addEventListener('input', validateGeometryPolicyControls);
}
for (const preset of geometryPolicyPresets) {
  preset.addEventListener('click', () => {
    applyGeometryPolicyPreset(preset.dataset.geometryPolicyPreset ?? '');
  });
}
placementPolicyReset.addEventListener('click', resetPlacementPolicyExperiment);
placementPolicyClearance.addEventListener('input', validatePlacementPolicyControls);
placementPolicyWallThickness.addEventListener('input', validatePlacementPolicyControls);
for (const preset of placementPolicyPresets) {
  preset.addEventListener('click', () => {
    placementPolicyClearance.value = preset.dataset.clearance ?? '';
    placementPolicyWallThickness.value = preset.dataset.wallThickness ?? '';
    validatePlacementPolicyControls();
  });
}
window.addEventListener('pagehide', () => {
  generationConfigRevision += 1;
  policyExperimentRevision += 1;
  geometryExperimentRevision += 1;
  corridorExperimentRevision += 1;
  voxelInspectionRevision += 1;
  stopVoxelInspectionReadoutSync();
  voxelInspectionSurface?.dispose();
  voxelInspectionSurface = null;
  voxelInspectionMount = null;
});

for (const tab of viewTabs) {
  tab.addEventListener('click', () => {
    const nextView = tab.dataset.view;
    if (nextView === 'layout' || nextView === 'intermediate' || nextView === 'build' || nextView === 'voxel' || nextView === 'voxel3d' || nextView === 'catalog') {
      activeView = nextView;
      history.replaceState(null, '', `#${activeView}`);
      renderActiveView();
    }
  });
}

if (initialSelection === null) {
  const artifact = await fetchArtifact('/api/artifacts/first-run');
  const validation = await fetchValidation(artifactUrl(artifact.validationRef));
  currentLayout = artifact.layout;
  currentIntermediate = emptyIntermediateContext();
  currentGeometry = null;
  committedGeometry = null;
  currentCatalog = await fetchDefaultCatalog();
  currentCatalogRef = currentCatalog === null ? null : 'fixtures/shape-catalogs/2d-basic.json';
  currentCatalogError = currentCatalog === null ? 'failed to load default fixture catalog' : null;
  currentSelection = null;
  currentPlacement = null;
  currentPlacementValidation = null;
  committedPlacement = null;
  committedPlacementValidation = null;
  currentBuiltFlowValidation = null;
  committedBuiltFlowValidation = null;
  currentPolicyExperimentId = null;
  currentGeometryExperimentId = null;
  currentCorridorExperimentId = null;
  configuredBuildId = null;
  populateGenerationConfigControls(persistedGenerationConfig);
  syncGenerationConfigControls();
  syncGeometryPolicyControls();
  syncCorridorRealizationControls();
  syncPlacementPolicyControls();
  syncVoxelDoorStateControls();
  renderBatchList(batchPanel, batch, null, selectEntry);
  renderSummary(summaryPanel, artifact, null, batch);
  renderContext(
    diagnosticsPanel,
    artifact,
    null,
    batch,
    validation,
    emptyIntermediateContext(),
    null,
  );
  renderActiveView();
} else {
  await selectEntry(initialSelection);
}

async function selectEntry(entry: SelectionEntry): Promise<void> {
  if (generationConfigBusy) {
    setGenerationConfigStatus(
      'loading',
      'Finish the current configured rebuild before switching candidates.',
    );
    return;
  }
  generationConfigRevision += 1;
  const selectionRevision = ++policyExperimentRevision;
  geometryExperimentRevision += 1;
  corridorExperimentRevision += 1;
  policyExperimentBusy = false;
  geometryExperimentBusy = false;
  corridorExperimentBusy = false;
  const artifact = await fetchArtifact(artifactUrl(entry.artifactRef));
  const validation = await fetchValidation(artifactUrl(entry.validationRef));
  const intermediate = await fetchIntermediateContext(entry);
  const [geometry, placement, placementValidation, builtFlowValidation, generationConfig] = await Promise.all([
    fetchOptionalArtifact<Geometry2dArtifact>(entry.geometryRef),
    fetchOptionalArtifact<PiecePlacement>(entry.piecePlacementRef),
    fetchOptionalArtifact<ValidationReport>(entry.piecePlacementValidationRef),
    fetchOptionalArtifact<BuiltFlowValidationReport>(entry.builtFlowValidationRef),
    fetchGenerationConfig(),
  ]);
  if (selectionRevision !== policyExperimentRevision) {
    return;
  }
  const catalogResult = await fetchCatalogForEntry(entry, placement);
  if (selectionRevision !== policyExperimentRevision) {
    return;
  }
  currentLayout = artifact.layout;
  currentIntermediate = intermediate;
  currentGeometry = geometry;
  committedGeometry = geometry;
  currentCatalog = catalogResult.catalog;
  currentCatalogRef = catalogResult.ref;
  currentCatalogError = catalogResult.error;
  currentSelection = entry;
  currentPlacement = placement;
  currentPlacementValidation = placementValidation;
  committedPlacement = placement;
  committedPlacementValidation = placementValidation;
  currentBuiltFlowValidation = builtFlowValidation;
  committedBuiltFlowValidation = builtFlowValidation;
  currentPolicyExperimentId = null;
  currentGeometryExperimentId = null;
  currentCorridorExperimentId = null;
  configuredBuildId = null;
  persistedGenerationConfig = generationConfig;
  populateGenerationConfigControls(persistedGenerationConfig);
  syncGenerationConfigControls();
  syncVoxelDoorStateControls();
  syncGeometryPolicyControls();
  syncCorridorRealizationControls();
  syncPlacementPolicyControls();
  renderBatchList(batchPanel, batch, entry.candidateId, selectEntry);
  renderSummary(summaryPanel, artifact, entry, batch);
  renderContext(diagnosticsPanel, artifact, entry, batch, validation, intermediate, placementValidation);
  renderActiveView();
}

function generationConfigInputs(): readonly (HTMLInputElement | HTMLSelectElement)[] {
  return [
    generationConfigInitialMargin,
    generationConfigInitialColumnGap,
    generationConfigInitialRowGap,
    generationConfigMarginGrowth,
    generationConfigColumnGrowth,
    generationConfigRowGrowth,
    generationConfigMaxTiers,
    generationConfigRoomAttempts,
    generationConfigMaxAttempts,
    generationConfigClearance,
    generationConfigWallThickness,
    generationConfigCatalogAttempts,
    generationConfigCatalogInitialSlack,
    generationConfigCatalogSlackGrowth,
    generationConfigCatalogRoomCandidates,
    generationConfigCatalogRouteStates,
    generationConfigCatalogRouteMargin,
    generationConfigCatalogGuideWeight,
    generationConfigCatalogTurnPenalty,
    generationConfigCorridorRealization,
  ];
}

function populateGenerationConfigControls(config: ViewerGenerationConfig): void {
  generationConfigInitialMargin.value = String(config.geometryLayoutPolicy.initialRoomMargin.value);
  generationConfigInitialColumnGap.value = String(config.geometryLayoutPolicy.initialColumnGap.value);
  generationConfigInitialRowGap.value = String(config.geometryLayoutPolicy.initialRowGap.value);
  generationConfigMarginGrowth.value = String(config.geometryLayoutPolicy.roomMarginGrowth.value);
  generationConfigColumnGrowth.value = String(config.geometryLayoutPolicy.columnGapGrowth.value);
  generationConfigRowGrowth.value = String(config.geometryLayoutPolicy.rowGapGrowth.value);
  generationConfigMaxTiers.value = String(config.geometryLayoutPolicy.maxSpacingTiers.value);
  generationConfigRoomAttempts.value = String(config.geometryLayoutPolicy.roomOrderAttemptsPerTier.value);
  generationConfigMaxAttempts.value = String(config.geometryLayoutPolicy.maxSearchAttempts.value);
  generationConfigClearance.value = String(config.placementPolicy.minimumClearanceCells.value);
  generationConfigWallThickness.value = String(config.placementPolicy.wallThicknessCells.value);
  generationConfigCatalogAttempts.value = String(
    config.catalogAwareGenerationPolicy.maxGenerationAttempts.value,
  );
  generationConfigCatalogInitialSlack.value = String(
    config.catalogAwareGenerationPolicy.initialRoomSlackCells.value,
  );
  generationConfigCatalogSlackGrowth.value = String(
    config.catalogAwareGenerationPolicy.roomSlackGrowthCells.value,
  );
  generationConfigCatalogRoomCandidates.value = String(
    config.catalogAwareGenerationPolicy.maxRoomCandidates.value,
  );
  generationConfigCatalogRouteStates.value = String(
    config.catalogAwareGenerationPolicy.maxRoutingStatesPerSection.value,
  );
  generationConfigCatalogRouteMargin.value = String(
    config.catalogAwareGenerationPolicy.routeMarginCells.value,
  );
  generationConfigCatalogGuideWeight.value = String(
    config.catalogAwareGenerationPolicy.guideDistanceWeight.value,
  );
  generationConfigCatalogTurnPenalty.value = String(
    config.catalogAwareGenerationPolicy.turnPenalty.value,
  );
  generationConfigCorridorRealization.value = config.corridorRealization.value;
  validateGenerationConfigControls();
}

function generationConfigWithDefaults(config: ViewerGenerationConfig): ViewerGenerationConfig {
  const reset = structuredClone(config);
  for (const setting of Object.values(reset.geometryLayoutPolicy)) {
    setting.value = setting.defaultValue;
  }
  for (const setting of Object.values(reset.placementPolicy)) {
    setting.value = setting.defaultValue;
  }
  for (const setting of Object.values(reset.catalogAwareGenerationPolicy)) {
    setting.value = setting.defaultValue;
  }
  reset.corridorRealization.value = reset.corridorRealization.defaultValue;
  return reset;
}

function generationConfigFromControls(): ViewerGenerationConfig | null {
  if (!validateGenerationConfigControls()) {
    return null;
  }
  const config = structuredClone(persistedGenerationConfig);
  config.geometryLayoutPolicy.initialRoomMargin.value = Number(generationConfigInitialMargin.value);
  config.geometryLayoutPolicy.initialColumnGap.value = Number(generationConfigInitialColumnGap.value);
  config.geometryLayoutPolicy.initialRowGap.value = Number(generationConfigInitialRowGap.value);
  config.geometryLayoutPolicy.roomMarginGrowth.value = Number(generationConfigMarginGrowth.value);
  config.geometryLayoutPolicy.columnGapGrowth.value = Number(generationConfigColumnGrowth.value);
  config.geometryLayoutPolicy.rowGapGrowth.value = Number(generationConfigRowGrowth.value);
  config.geometryLayoutPolicy.maxSpacingTiers.value = Number(generationConfigMaxTiers.value);
  config.geometryLayoutPolicy.roomOrderAttemptsPerTier.value = Number(generationConfigRoomAttempts.value);
  config.geometryLayoutPolicy.maxSearchAttempts.value = Number(generationConfigMaxAttempts.value);
  config.placementPolicy.minimumClearanceCells.value = Number(generationConfigClearance.value);
  config.placementPolicy.wallThicknessCells.value = Number(generationConfigWallThickness.value);
  config.catalogAwareGenerationPolicy.maxGenerationAttempts.value =
    Number(generationConfigCatalogAttempts.value);
  config.catalogAwareGenerationPolicy.initialRoomSlackCells.value =
    Number(generationConfigCatalogInitialSlack.value);
  config.catalogAwareGenerationPolicy.roomSlackGrowthCells.value =
    Number(generationConfigCatalogSlackGrowth.value);
  config.catalogAwareGenerationPolicy.maxRoomCandidates.value =
    Number(generationConfigCatalogRoomCandidates.value);
  config.catalogAwareGenerationPolicy.maxRoutingStatesPerSection.value =
    Number(generationConfigCatalogRouteStates.value);
  config.catalogAwareGenerationPolicy.routeMarginCells.value =
    Number(generationConfigCatalogRouteMargin.value);
  config.catalogAwareGenerationPolicy.guideDistanceWeight.value =
    Number(generationConfigCatalogGuideWeight.value);
  config.catalogAwareGenerationPolicy.turnPenalty.value =
    Number(generationConfigCatalogTurnPenalty.value);
  config.corridorRealization.value = generationConfigCorridorRealization.value as CorridorRealization;
  return config;
}

function validateGenerationConfigControls(): boolean {
  const gridValues = [
    [generationConfigInitialMargin, 32, 1_024, 'Initial outer margin'],
    [generationConfigInitialColumnGap, 32, 1_024, 'Initial column gap'],
    [generationConfigInitialRowGap, 32, 1_024, 'Initial row gap'],
    [generationConfigMarginGrowth, 0, 512, 'Margin growth'],
    [generationConfigColumnGrowth, 0, 512, 'Column growth'],
    [generationConfigRowGrowth, 0, 512, 'Row growth'],
  ] as const;
  let issue = '';
  for (const [input, minimum, maximum, label] of gridValues) {
    const value = Number(input.value);
    const inputIssue = !Number.isInteger(value) || value < minimum || value > maximum
      ? `${label} must be an integer from ${minimum} through ${maximum}.`
      : value % 8 !== 0
        ? `${label} must align to the 8-unit route grid.`
        : '';
    input.setCustomValidity(inputIssue);
    issue ||= inputIssue;
  }
  for (const [input, minimum, maximum, label] of [
    [generationConfigMaxTiers, 1, 8, 'Maximum tiers'],
    [generationConfigRoomAttempts, 1, 32, 'Room orders per tier'],
    [generationConfigClearance, 3, 64, 'Room clearance'],
    [generationConfigWallThickness, 1, 8, 'Route wall buffer'],
    [generationConfigCatalogAttempts, 1, 16, 'Catalog generation attempts'],
    [generationConfigCatalogInitialSlack, 0, 128, 'Catalog initial room slack'],
    [generationConfigCatalogSlackGrowth, 0, 128, 'Catalog room slack growth'],
    [generationConfigCatalogRoomCandidates, 1, 64, 'Catalog room candidates'],
    [generationConfigCatalogRouteStates, 100, 1_000_000, 'Catalog route state budget'],
    [generationConfigCatalogRouteMargin, 8, 256, 'Catalog route margin'],
    [generationConfigCatalogGuideWeight, 0, 1_000, 'Catalog guide weight'],
    [generationConfigCatalogTurnPenalty, 0, 1_000, 'Catalog turn penalty'],
  ] as const) {
    const value = Number(input.value);
    const inputIssue = !Number.isInteger(value) || value < minimum || value > maximum
      ? `${label} must be an integer from ${minimum} through ${maximum}.`
      : '';
    input.setCustomValidity(inputIssue);
    issue ||= inputIssue;
  }
  const maxAttempts = Number(generationConfigMaxAttempts.value);
  const availableAttempts = Number(generationConfigMaxTiers.value)
    * Number(generationConfigRoomAttempts.value) * 4;
  const attemptsIssue = !Number.isInteger(maxAttempts)
    || maxAttempts < 1
    || maxAttempts > availableAttempts
    ? `Maximum route attempts must be from 1 through ${availableAttempts}.`
    : '';
  generationConfigMaxAttempts.setCustomValidity(attemptsIssue);
  issue ||= attemptsIssue;
  const requiredClearance = Number(generationConfigWallThickness.value) * 2 + 1;
  const clearanceIssue = Number(generationConfigClearance.value) < requiredClearance
    ? `Room clearance must be at least ${requiredClearance} for this route wall buffer.`
    : '';
  generationConfigClearance.setCustomValidity(clearanceIssue);
  issue ||= clearanceIssue;
  const maximumCatalogSlack = Number(generationConfigCatalogInitialSlack.value)
    + Number(generationConfigCatalogSlackGrowth.value)
      * (Number(generationConfigCatalogAttempts.value) - 1);
  const catalogSlackIssue = maximumCatalogSlack > 128
    ? 'Catalog room slack must remain at most 128 cells across all attempts.'
    : '';
  generationConfigCatalogSlackGrowth.setCustomValidity(catalogSlackIssue);
  issue ||= catalogSlackIssue;
  const corridorIssue = !['catalog', 'hybrid', 'procedural']
    .includes(generationConfigCorridorRealization.value)
    ? 'Corridor realization must be catalog, hybrid, or procedural.'
    : '';
  generationConfigCorridorRealization.setCustomValidity(corridorIssue);
  issue ||= corridorIssue;

  const valid = issue.length === 0;
  generationConfigValidation.dataset.state = valid ? 'valid' : 'invalid';
  generationConfigValidation.textContent = valid
    ? 'Configuration values are valid; Apply rebuilds the complete pipeline before persisting.'
    : issue;
  const draft = valid ? generationConfigFromControlsWithoutValidation() : null;
  const dirty = draft !== null
    && generationConfigValueSignature(draft) !== generationConfigValueSignature(persistedGenerationConfig);
  generationConfigPanel.dataset.configState = dirty ? 'unsaved' : 'persisted';
  generationConfigMode.dataset.mode = dirty ? 'experiment' : 'persisted';
  generationConfigMode.textContent = dirty ? 'Unsaved changes' : 'Persisted configuration';
  const selectedCandidateNeedsBuild = currentSelection !== null && configuredBuildId === null;
  generationConfigApply.disabled = !valid
    || (!dirty && !selectedCandidateNeedsBuild)
    || currentSelection === null
    || generationConfigBusy;
  generationConfigReset.disabled = currentSelection === null || generationConfigBusy;
  return valid;
}

function generationConfigFromControlsWithoutValidation(): ViewerGenerationConfig | null {
  const numbers = generationConfigInputs()
    .filter((input): input is HTMLInputElement => input instanceof HTMLInputElement)
    .map((input) => Number(input.value));
  if (numbers.some((value) => !Number.isFinite(value))) {
    return null;
  }
  const config = structuredClone(persistedGenerationConfig);
  config.geometryLayoutPolicy.initialRoomMargin.value = numbers[0];
  config.geometryLayoutPolicy.initialColumnGap.value = numbers[1];
  config.geometryLayoutPolicy.initialRowGap.value = numbers[2];
  config.geometryLayoutPolicy.roomMarginGrowth.value = numbers[3];
  config.geometryLayoutPolicy.columnGapGrowth.value = numbers[4];
  config.geometryLayoutPolicy.rowGapGrowth.value = numbers[5];
  config.geometryLayoutPolicy.maxSpacingTiers.value = numbers[6];
  config.geometryLayoutPolicy.roomOrderAttemptsPerTier.value = numbers[7];
  config.geometryLayoutPolicy.maxSearchAttempts.value = numbers[8];
  config.placementPolicy.minimumClearanceCells.value = numbers[9];
  config.placementPolicy.wallThicknessCells.value = numbers[10];
  config.catalogAwareGenerationPolicy.maxGenerationAttempts.value = numbers[11];
  config.catalogAwareGenerationPolicy.initialRoomSlackCells.value = numbers[12];
  config.catalogAwareGenerationPolicy.roomSlackGrowthCells.value = numbers[13];
  config.catalogAwareGenerationPolicy.maxRoomCandidates.value = numbers[14];
  config.catalogAwareGenerationPolicy.maxRoutingStatesPerSection.value = numbers[15];
  config.catalogAwareGenerationPolicy.routeMarginCells.value = numbers[16];
  config.catalogAwareGenerationPolicy.guideDistanceWeight.value = numbers[17];
  config.catalogAwareGenerationPolicy.turnPenalty.value = numbers[18];
  config.corridorRealization.value = generationConfigCorridorRealization.value as CorridorRealization;
  return config;
}

function generationConfigValueSignature(config: ViewerGenerationConfig): string {
  return JSON.stringify({
    geometry: Object.fromEntries(
      Object.entries(config.geometryLayoutPolicy).map(([key, setting]) => [key, setting.value]),
    ),
    placement: Object.fromEntries(
      Object.entries(config.placementPolicy).map(([key, setting]) => [key, setting.value]),
    ),
    catalogAware: Object.fromEntries(
      Object.entries(config.catalogAwareGenerationPolicy)
        .map(([key, setting]) => [key, setting.value]),
    ),
    corridorRealization: config.corridorRealization.value,
  });
}

function syncGenerationConfigControls(preserveStatus = false): void {
  const enabled = currentSelection !== null && !generationConfigBusy;
  for (const input of generationConfigInputs()) {
    input.disabled = !enabled;
  }
  generationConfigPanel.dataset.buildId = configuredBuildId ?? '';
  if (!preserveStatus) {
    if (currentSelection === null) {
      setGenerationConfigStatus('idle', 'Select a generated candidate to rebuild.');
    } else if (configuredBuildId !== null) {
      setGenerationConfigStatus('ready', 'Persisted configuration build is active in Build, Voxel, and Voxel 3D.');
    } else {
      setGenerationConfigStatus(
        'idle',
        'Persisted configuration loaded; edit any settings or rebuild this candidate as-is.',
      );
    }
  }
  validateGenerationConfigControls();
}

async function applyGenerationConfig(): Promise<void> {
  const selection = currentSelection;
  const config = generationConfigFromControls();
  if (selection === null || config === null) {
    setGenerationConfigStatus('error', 'Select a candidate and correct the configuration values first.');
    return;
  }
  const revision = ++generationConfigRevision;
  generationConfigBusy = true;
  syncGenerationConfigControls();
  setGenerationConfigStatus(
    'loading',
    `Rebuilding ${selection.candidateId} with combined geometry, placement, and corridor settings…`,
  );
  try {
    const response = await fetch('/api/generation-config/rebuild', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ candidateId: selection.candidateId, config }),
    });
    const result = await readJsonResponse<
      GenerationConfigRebuildResponse | PlacementPolicyExperimentError
    >(response);
    if (revision !== generationConfigRevision) {
      return;
    }
    if (!response.ok || 'error' in result) {
      throw new Error(
        'evidence' in result && result.evidence !== undefined
          ? formatPureCatalogExhaustion(result.evidence)
          : 'detail' in result
            ? result.detail
            : `rebuild request failed with ${response.status}`,
      );
    }
    if (
      result.kind !== 'asha_procgen.viewer_generation_rebuild.v1'
      || result.candidateId !== selection.candidateId
      || result.persisted !== true
      || result.nativeAuthority !== false
      || result.geometryValidation.ok !== true
      || result.placementValidation.ok !== true
      || result.builtFlowValidation.ok !== true
    ) {
      throw new Error('generation config rebuild returned an invalid response envelope');
    }
    persistedGenerationConfig = result.config;
    configuredBuildId = result.buildId;
    currentGeometry = result.geometry;
    currentPlacement = result.placement;
    currentPlacementValidation = result.placementValidation;
    currentBuiltFlowValidation = result.builtFlowValidation;
    currentGeometryExperimentId = null;
    currentPolicyExperimentId = null;
    currentCorridorExperimentId = null;
    populateGenerationConfigControls(result.config);
    generationConfigImpact.textContent = `Configured build: ${result.metrics.footprintWidth} × ${result.metrics.footprintHeight} footprint; ${result.metrics.corridorPrefabInstances} corridor prefabs; ${result.metrics.routedCorridorCells.toLocaleString()} routed cells.`;
    syncVoxelDoorStateControls();
    syncGenerationConfigControls();
    setGenerationConfigStatus(
      'ready',
      `Configuration persisted and ${selection.candidateId} rebuilt with verified placement and built flow.`,
    );
    renderActiveView();
  } catch (error) {
    if (revision === generationConfigRevision) {
      setGenerationConfigStatus(
        'error',
        `Rebuild failed; persisted configuration and current result were unchanged: ${describeError(error)}`,
      );
    }
  } finally {
    if (revision === generationConfigRevision) {
      generationConfigBusy = false;
      syncGenerationConfigControls(generationConfigStatus.dataset.state === 'error');
    }
  }
}

function setGenerationConfigStatus(state: 'idle' | 'loading' | 'ready' | 'error', message: string): void {
  generationConfigStatus.dataset.state = state;
  generationConfigStatus.textContent = message;
}

function geometryPolicyInputs(): readonly HTMLInputElement[] {
  return [
    geometryPolicyInitialMargin,
    geometryPolicyInitialColumnGap,
    geometryPolicyInitialRowGap,
    geometryPolicyMarginGrowth,
    geometryPolicyColumnGrowth,
    geometryPolicyRowGrowth,
    geometryPolicyMaxTiers,
    geometryPolicyRoomAttempts,
    geometryPolicyMaxAttempts,
  ];
}

async function applyGeometryPolicyExperiment(): Promise<void> {
  const selection = currentSelection;
  if (currentCorridorExperimentId !== null) {
    setGeometryPolicyStatus('error', 'Reset the temporary corridor realization before changing geometry.');
    return;
  }
  if (selection === null || committedGeometry === null || committedPlacement === null) {
    setGeometryPolicyStatus('error', 'Select a generated candidate with committed geometry first.');
    return;
  }
  const policy = geometryPolicyFromControls();
  if (policy === null) {
    setGeometryPolicyStatus('error', 'Correct the geometry policy values before applying.');
    return;
  }
  const revision = ++geometryExperimentRevision;
  policyExperimentRevision += 1;
  corridorExperimentRevision += 1;
  setGeometryPolicyBusy(true);
  setGeometryPolicyStatus(
    'loading',
    `Regenerating ${selection.candidateId} through at most ${policy.maxSpacingTiers} spacing tiers and ${policy.maxSearchAttempts} route attempts…`,
  );
  try {
    const response = await fetch('/api/experiments/geometry-layout-policy', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        candidateId: selection.candidateId,
        geometryLayoutPolicy: policy,
      }),
    });
    const result = (await response.json()) as
      | GeometryLayoutPolicyExperimentResponse
      | PlacementPolicyExperimentError;
    if (revision !== geometryExperimentRevision) {
      return;
    }
    if (!response.ok || 'error' in result) {
      throw new Error(
        'evidence' in result && result.evidence !== undefined
          ? formatPureCatalogExhaustion(result.evidence)
          : 'detail' in result
            ? result.detail
            : `experiment request failed with ${response.status}`,
      );
    }
    if (
      result.kind !== 'asha_procgen.geometry_layout_policy_experiment.v1'
      || result.candidateId !== selection.candidateId
      || result.persisted !== false
      || result.nativeAuthority !== false
    ) {
      throw new Error('geometry-policy experiment returned an invalid response envelope');
    }
    currentGeometry = result.geometry;
    currentPlacement = result.placement;
    currentPlacementValidation = result.placementValidation;
    currentBuiltFlowValidation = result.builtFlowValidation;
    currentGeometryExperimentId = result.experimentId;
    currentPolicyExperimentId = null;
    currentCorridorExperimentId = null;
    syncVoxelDoorStateControls();
    syncGeometryPolicyControls();
    syncCorridorRealizationControls();
    syncPlacementPolicyControls();
    const search = result.geometry.layoutSearch;
    setGeometryPolicyStatus(
      'ready',
      `Temporary Rust geometry applied at tier ${search.spacingTier + 1}/${policy.maxSpacingTiers} after ${search.searchAttempts} route attempt(s). Not persisted; no native authority claim.`,
    );
    renderActiveView();
  } catch (error) {
    if (revision === geometryExperimentRevision) {
      setGeometryPolicyStatus('error', `Geometry experiment failed: ${describeError(error)}`);
    }
  } finally {
    if (revision === geometryExperimentRevision) {
      setGeometryPolicyBusy(false);
    }
  }
}

function formatPureCatalogExhaustion(evidence: PureCatalogExhaustionEvidence): string {
  const { failure, budgets } = evidence;
  const endpoints = failure.requiredEndpoints
    .map((endpoint) => `${endpoint.id}:${endpoint.direction}`)
    .join(', ');
  const fixedPort = failure.fixedPort == null
    ? 'no fixed neighbor port'
    : `fixed ${failure.fixedPort.neighborPieceId}.${failure.fixedPort.neighborExitId}`
      + ` @ ${failure.fixedPort.cell.x},${failure.fixedPort.cell.y}`
      + ` facing ${failure.fixedPort.direction} (needs ${failure.fixedPort.requiredOppositeDirection})`
      + (failure.fixedPort.offsetFromEnvelopeAnchor == null
        ? ''
        : `, lane offset ${failure.fixedPort.offsetFromEnvelopeAnchor.x},${failure.fixedPort.offsetFromEnvelopeAnchor.y}`);
  const bounds = failure.originBounds == null
    ? ''
    : ` Room origin bounds x=${failure.originBounds.minX}..${failure.originBounds.maxX},`
      + ` y=${failure.originBounds.minY}..${failure.originBounds.maxY}.`;
  const lane = failure.laneEnvelope == null
    ? ''
    : ` Lane ${failure.laneEnvelope.sourceHint} ±${failure.laneEnvelope.envelopeCells}`
      + ` cells (x=${failure.laneEnvelope.bounds.minX}..${failure.laneEnvelope.bounds.maxX},`
      + ` y=${failure.laneEnvelope.bounds.minY}..${failure.laneEnvelope.bounds.maxY}).`;
  return `${failure.pieceId} (${failure.requirementKind}) ${failure.reason}: ${failure.detail}`
    + ` Required endpoints: ${endpoints || 'none'}; ${fixedPort}.`
    + bounds
    + lane
    + ` Exhausted families: ${failure.exhaustedFamilies.join(', ') || 'none'}`
    + ` across ${failure.candidateCount} candidate(s).`
    + ` Budgets: decisions ${budgets.decisions}/${budgets.maxDecisions},`
    + ` backtracks ${budgets.backtracks}/${budgets.maxBacktracks},`
    + ` chain expansions ${budgets.chainExpansions}/${budgets.maxChainExpansionsPerSection}.`;
}

function resetGeometryPolicyExperiment(): void {
  geometryExperimentRevision += 1;
  policyExperimentRevision += 1;
  corridorExperimentRevision += 1;
  currentGeometry = committedGeometry;
  currentPlacement = committedPlacement;
  currentPlacementValidation = committedPlacementValidation;
  currentBuiltFlowValidation = committedBuiltFlowValidation;
  currentGeometryExperimentId = null;
  currentPolicyExperimentId = null;
  currentCorridorExperimentId = null;
  syncVoxelDoorStateControls();
  setGeometryPolicyBusy(false);
  syncGeometryPolicyControls();
  syncCorridorRealizationControls();
  syncPlacementPolicyControls();
  renderActiveView();
}

function syncGeometryPolicyControls(): void {
  const geometry = currentGeometry;
  const enabled = geometry !== null
    && currentSelection !== null
    && currentCorridorExperimentId === null;
  if (geometry !== null) {
    const policy = geometry.layoutPolicy;
    geometryPolicyInitialMargin.value = String(policy.initialRoomMargin);
    geometryPolicyInitialColumnGap.value = String(policy.initialColumnGap);
    geometryPolicyInitialRowGap.value = String(policy.initialRowGap);
    geometryPolicyMarginGrowth.value = String(policy.roomMarginGrowth);
    geometryPolicyColumnGrowth.value = String(policy.columnGapGrowth);
    geometryPolicyRowGrowth.value = String(policy.rowGapGrowth);
    geometryPolicyMaxTiers.value = String(policy.maxSpacingTiers);
    geometryPolicyRoomAttempts.value = String(policy.roomOrderAttemptsPerTier);
    geometryPolicyMaxAttempts.value = String(policy.maxSearchAttempts);
    geometryPolicyPanel.dataset.spacingTier = String(geometry.layoutSearch.spacingTier);
    geometryPolicyPanel.dataset.searchAttempts = String(geometry.layoutSearch.searchAttempts);
  } else {
    for (const input of geometryPolicyInputs()) {
      input.value = '';
    }
    delete geometryPolicyPanel.dataset.spacingTier;
    delete geometryPolicyPanel.dataset.searchAttempts;
  }
  const experimentActive = currentGeometryExperimentId !== null;
  geometryPolicyPanel.dataset.mode = experimentActive ? 'experiment' : 'committed';
  geometryPolicyPanel.dataset.experimentId = currentGeometryExperimentId ?? '';
  geometryPolicyMode.dataset.mode = experimentActive ? 'experiment' : 'committed';
  geometryPolicyMode.textContent = experimentActive ? 'Temporary experiment' : 'Committed policy';
  for (const input of geometryPolicyInputs()) {
    input.disabled = !enabled || geometryExperimentBusy;
  }
  geometryPolicyReset.disabled = !experimentActive || geometryExperimentBusy;
  for (const preset of geometryPolicyPresets) {
    preset.disabled = !enabled || geometryExperimentBusy;
  }
  if (currentCorridorExperimentId !== null) {
    setGeometryPolicyStatus('idle', 'Reset the temporary corridor realization before changing geometry.');
  } else if (!enabled) {
    setGeometryPolicyStatus('idle', 'Select a generated candidate to experiment.');
  } else if (experimentActive) {
    setGeometryPolicyStatus('ready', 'Temporary Rust geometry active. Not persisted; no native authority claim.');
  } else {
    setGeometryPolicyStatus('idle', 'Committed geometry active. Change values and regenerate.');
  }
  validateGeometryPolicyControls();
  updateGeometryPolicyImpact();
}

function geometryPolicyFromControls(): GeometryLayoutPolicy | null {
  if (!validateGeometryPolicyControls()) {
    return null;
  }
  const maxSpacingTiers = Number(geometryPolicyMaxTiers.value);
  const roomOrderAttemptsPerTier = Number(geometryPolicyRoomAttempts.value);
  return {
    kind: 'asha_procgen.geometry_layout_policy.v1',
    schemaVersion: 1,
    initialRoomMargin: Number(geometryPolicyInitialMargin.value),
    initialColumnGap: Number(geometryPolicyInitialColumnGap.value),
    initialRowGap: Number(geometryPolicyInitialRowGap.value),
    roomMarginGrowth: Number(geometryPolicyMarginGrowth.value),
    columnGapGrowth: Number(geometryPolicyColumnGrowth.value),
    rowGapGrowth: Number(geometryPolicyRowGrowth.value),
    maxSpacingTiers,
    roomOrderAttemptsPerTier,
    maxSearchAttempts: Number(geometryPolicyMaxAttempts.value),
  };
}

function validateGeometryPolicyControls(): boolean {
  const values = [
    [geometryPolicyInitialMargin, 32, 1_024, 'Initial outer margin'],
    [geometryPolicyInitialColumnGap, 32, 1_024, 'Initial column gap'],
    [geometryPolicyInitialRowGap, 32, 1_024, 'Initial row gap'],
    [geometryPolicyMarginGrowth, 0, 512, 'Margin growth'],
    [geometryPolicyColumnGrowth, 0, 512, 'Column growth'],
    [geometryPolicyRowGrowth, 0, 512, 'Row growth'],
  ] as const;
  let issue = '';
  for (const [input, minimum, maximum, label] of values) {
    const value = Number(input.value);
    let inputIssue = '';
    if (!Number.isInteger(value) || value < minimum || value > maximum) {
      inputIssue = `${label} must be an integer from ${minimum} through ${maximum}.`;
    } else if (value % 8 !== 0) {
      inputIssue = `${label} must align to the 8-unit route grid.`;
    }
    input.setCustomValidity(inputIssue);
    issue ||= inputIssue;
  }
  const tiers = Number(geometryPolicyMaxTiers.value);
  const roomAttempts = Number(geometryPolicyRoomAttempts.value);
  const maxAttempts = Number(geometryPolicyMaxAttempts.value);
  const tiersIssue = Number.isInteger(tiers) && tiers >= 1 && tiers <= 8
    ? ''
    : 'Maximum tiers must be an integer from 1 through 8.';
  const roomAttemptsIssue = Number.isInteger(roomAttempts) && roomAttempts >= 1 && roomAttempts <= 32
    ? ''
    : 'Room orders per tier must be an integer from 1 through 32.';
  geometryPolicyMaxTiers.setCustomValidity(tiersIssue);
  geometryPolicyRoomAttempts.setCustomValidity(roomAttemptsIssue);
  issue ||= tiersIssue || roomAttemptsIssue;
  const capacity = Number.isInteger(tiers) && Number.isInteger(roomAttempts)
    ? tiers * roomAttempts * 4
    : 0;
  const maxAttemptsIssue = Number.isInteger(maxAttempts)
    && maxAttempts >= 1
    && maxAttempts <= capacity
    ? ''
    : `Maximum route attempts must be an integer from 1 through ${capacity}.`;
  geometryPolicyMaxAttempts.max = String(Math.max(1, capacity));
  geometryPolicyMaxAttempts.setCustomValidity(maxAttemptsIssue);
  issue ||= maxAttemptsIssue;
  if (issue === '' && tiers > 0) {
    const finalTier = tiers - 1;
    for (const [initial, growth, label] of [
      [Number(geometryPolicyInitialMargin.value), Number(geometryPolicyMarginGrowth.value), 'Outer margin'],
      [Number(geometryPolicyInitialColumnGap.value), Number(geometryPolicyColumnGrowth.value), 'Column gap'],
      [Number(geometryPolicyInitialRowGap.value), Number(geometryPolicyRowGrowth.value), 'Row gap'],
    ] as const) {
      if (initial + growth * finalTier > 2_048) {
        issue = `${label} exceeds 2048 units at the final tier.`;
        break;
      }
    }
  }
  geometryPolicyBudget.textContent = `capacity ${capacity} route attempts`;
  const valid = issue === '';
  geometryPolicyValidation.dataset.state = valid ? 'valid' : 'invalid';
  geometryPolicyValidation.textContent = valid
    ? `Valid compact-first search: ${tiers} tier(s), ${roomAttempts} room order(s) per tier, capped at ${maxAttempts}/${capacity} route attempts.`
    : issue;
  geometryPolicyApply.disabled = currentSelection === null
    || committedGeometry === null
    || geometryExperimentBusy
    || currentCorridorExperimentId !== null
    || !valid;
  return valid;
}

function setGeometryPolicyBusy(busy: boolean): void {
  geometryExperimentBusy = busy;
  for (const input of geometryPolicyInputs()) {
    input.disabled = busy || currentSelection === null;
  }
  geometryPolicyReset.disabled = busy || currentGeometryExperimentId === null;
  for (const preset of geometryPolicyPresets) {
    preset.disabled = busy || currentSelection === null;
  }
  validateGeometryPolicyControls();
}

function applyGeometryPolicyPreset(name: string): void {
  const preset = name === 'compact'
    ? [64, 96, 48, 32, 48, 24, 4, 3, 48]
    : name === 'roomy'
      ? [160, 240, 96, 64, 96, 48, 4, 4, 64]
      : [96, 144, 64, 48, 72, 40, 5, 4, 80];
  [
    geometryPolicyInitialMargin,
    geometryPolicyInitialColumnGap,
    geometryPolicyInitialRowGap,
    geometryPolicyMarginGrowth,
    geometryPolicyColumnGrowth,
    geometryPolicyRowGrowth,
    geometryPolicyMaxTiers,
    geometryPolicyRoomAttempts,
    geometryPolicyMaxAttempts,
  ].forEach((input, index) => {
    input.value = String(preset[index]);
  });
  validateGeometryPolicyControls();
}

function updateGeometryPolicyImpact(): void {
  if (committedGeometry === null) {
    geometryPolicyImpact.textContent = 'Waiting for committed geometry.';
    return;
  }
  if (currentGeometryExperimentId === null || currentGeometry === null) {
    const search = committedGeometry.layoutSearch;
    geometryPolicyImpact.textContent = `Committed frame: ${committedGeometry.bounds.width} × ${committedGeometry.bounds.height}; successful tier ${search.spacingTier + 1}; ${search.searchAttempts} route attempt(s).`;
    return;
  }
  geometryPolicyImpact.textContent = `Geometry impact: frame ${committedGeometry.bounds.width} × ${committedGeometry.bounds.height} → ${currentGeometry.bounds.width} × ${currentGeometry.bounds.height}; successful tier ${currentGeometry.layoutSearch.spacingTier + 1}; ${currentGeometry.layoutSearch.searchAttempts} route attempt(s).`;
}

function setGeometryPolicyStatus(state: 'idle' | 'loading' | 'ready' | 'error', message: string): void {
  geometryPolicyStatus.dataset.state = state;
  geometryPolicyStatus.textContent = message;
}

async function applyCorridorRealizationExperiment(): Promise<void> {
  const selection = currentSelection;
  if (currentGeometryExperimentId !== null || currentPolicyExperimentId !== null) {
    setCorridorRealizationStatus(
      'error',
      'Reset the temporary geometry or placement policy before changing corridor realization.',
    );
    return;
  }
  if (selection === null || committedPlacement === null) {
    setCorridorRealizationStatus('error', 'Select a generated candidate with a committed placement first.');
    return;
  }
  const corridorRealization = corridorRealizationSelect.value;
  if (!['catalog', 'hybrid', 'procedural'].includes(corridorRealization)) {
    setCorridorRealizationStatus('error', 'Choose pure catalog, hybrid, or procedural corridors.');
    return;
  }
  const revision = ++corridorExperimentRevision;
  setCorridorRealizationBusy(true);
  setCorridorRealizationStatus(
    'loading',
    `Rebuilding ${selection.candidateId} with ${corridorRealization} corridors…`,
  );
  try {
    const response = await fetch('/api/experiments/corridor-realization', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        candidateId: selection.candidateId,
        corridorRealization,
      }),
    });
    const result = (await response.json()) as
      | CorridorRealizationExperimentResponse
      | PlacementPolicyExperimentError;
    if (revision !== corridorExperimentRevision) {
      return;
    }
    if (!response.ok || 'error' in result) {
      throw new Error(
        'evidence' in result && result.evidence !== undefined
          ? formatPureCatalogExhaustion(result.evidence)
          : 'detail' in result
            ? result.detail
            : `experiment request failed with ${response.status}`,
      );
    }
    if (
      result.kind !== 'asha_procgen.corridor_realization_experiment.v1'
      || result.candidateId !== selection.candidateId
      || result.corridorRealization !== corridorRealization
      || result.persisted !== false
      || result.nativeAuthority !== false
      || result.placementValidation.ok !== true
      || result.builtFlowValidation.ok !== true
    ) {
      throw new Error('corridor-realization experiment returned an invalid response envelope');
    }
    currentPlacement = result.placement;
    currentPlacementValidation = result.placementValidation;
    currentBuiltFlowValidation = result.builtFlowValidation;
    currentCorridorExperimentId = result.experimentId;
    syncVoxelDoorStateControls();
    syncCorridorRealizationControls();
    syncGeometryPolicyControls();
    syncPlacementPolicyControls();
    setCorridorRealizationStatus(
      'ready',
      `Temporary ${corridorRealization} realization applied: ${result.metrics.corridorPrefabInstances} corridor prefabs and ${result.metrics.routedCorridorCells} routed cells. Placement and built flow verified; not persisted.`,
    );
    renderActiveView();
  } catch (error) {
    if (revision === corridorExperimentRevision) {
      setCorridorRealizationStatus('error', `Corridor experiment failed: ${describeError(error)}`);
    }
  } finally {
    if (revision === corridorExperimentRevision) {
      setCorridorRealizationBusy(false);
    }
  }
}

function resetCorridorRealizationExperiment(): void {
  corridorExperimentRevision += 1;
  currentPlacement = committedPlacement;
  currentPlacementValidation = committedPlacementValidation;
  currentBuiltFlowValidation = committedBuiltFlowValidation;
  currentCorridorExperimentId = null;
  syncVoxelDoorStateControls();
  setCorridorRealizationBusy(false);
  syncCorridorRealizationControls();
  syncGeometryPolicyControls();
  syncPlacementPolicyControls();
  renderActiveView();
}

function corridorRealizationFor(placement: PiecePlacement): CorridorRealization {
  return placement.corridorRealization ?? 'hybrid';
}

function corridorPrefabCount(placement: PiecePlacement): number {
  return placement.instances.filter((instance) =>
    instance.requirementKind === 'connector'
      || instance.requirementKind === 'corridor'
      || instance.requirementKind === 'bend'
      || instance.requirementKind === 'junction').length;
}

function syncCorridorRealizationControls(): void {
  const enabled = currentSelection !== null
    && committedPlacement !== null
    && currentGeometryExperimentId === null
    && currentPolicyExperimentId === null;
  const active = currentCorridorExperimentId !== null;
  const placement = currentPlacement ?? committedPlacement;
  if (placement !== null) {
    corridorRealizationSelect.value = corridorRealizationFor(placement);
    corridorRealizationPanel.dataset.corridorRealization = corridorRealizationFor(placement);
  }
  corridorRealizationPanel.dataset.mode = active ? 'experiment' : 'committed';
  corridorRealizationPanel.dataset.experimentId = currentCorridorExperimentId ?? '';
  corridorRealizationMode.dataset.mode = active ? 'experiment' : 'committed';
  corridorRealizationMode.textContent = active ? 'Temporary experiment' : 'Committed mode';
  corridorRealizationSelect.disabled = !enabled || corridorExperimentBusy;
  corridorRealizationApply.disabled = !enabled || corridorExperimentBusy;
  corridorRealizationReset.disabled = !active || corridorExperimentBusy;
  if (currentGeometryExperimentId !== null || currentPolicyExperimentId !== null) {
    setCorridorRealizationStatus('idle', 'Reset the other temporary experiment before changing corridor realization.');
  } else if (!enabled) {
    setCorridorRealizationStatus('idle', 'Select a generated candidate to experiment.');
  } else if (active) {
    setCorridorRealizationStatus('ready', 'Temporary corridor realization active. Not persisted.');
  } else {
    setCorridorRealizationStatus('idle', 'Committed corridor realization active. Choose a mode and rebuild.');
  }
  updateCorridorRealizationImpact();
}

function setCorridorRealizationBusy(busy: boolean): void {
  corridorExperimentBusy = busy;
  corridorRealizationSelect.disabled = busy || currentSelection === null;
  corridorRealizationApply.disabled = busy || currentSelection === null;
  corridorRealizationReset.disabled = busy || currentCorridorExperimentId === null;
}

function updateCorridorRealizationImpact(): void {
  if (committedPlacement === null) {
    corridorRealizationImpact.textContent = 'Waiting for a committed placement.';
    return;
  }
  const committedMode = corridorRealizationFor(committedPlacement);
  const committedPrefabs = corridorPrefabCount(committedPlacement);
  const committedCells = committedPlacement.connectionCells.length;
  const committedFootprint = placementPolicyMetrics(committedPlacement);
  if (currentCorridorExperimentId === null || currentPlacement === null) {
    corridorRealizationImpact.textContent = `Committed ${committedMode}: ${committedPrefabs} corridor prefabs; ${committedCells.toLocaleString()} routed corridor cells; ${committedFootprint.width} × ${committedFootprint.height} footprint.`;
    return;
  }
  const experimentFootprint = placementPolicyMetrics(currentPlacement);
  corridorRealizationImpact.textContent = `Comparison: ${committedMode} ${committedPrefabs} prefabs / ${committedCells.toLocaleString()} routed cells / ${committedFootprint.width} × ${committedFootprint.height} → ${corridorRealizationFor(currentPlacement)} ${corridorPrefabCount(currentPlacement)} prefabs / ${currentPlacement.connectionCells.length.toLocaleString()} routed cells / ${experimentFootprint.width} × ${experimentFootprint.height}.`;
}

function setCorridorRealizationStatus(
  state: 'idle' | 'loading' | 'ready' | 'error',
  message: string,
): void {
  corridorRealizationStatus.dataset.state = state;
  corridorRealizationStatus.textContent = message;
}

async function applyPlacementPolicyExperiment(): Promise<void> {
  const selection = currentSelection;
  if (currentCorridorExperimentId !== null) {
    setPlacementPolicyStatus('error', 'Reset the temporary corridor realization before changing placement policy.');
    return;
  }
  if (currentGeometryExperimentId !== null) {
    setPlacementPolicyStatus('error', 'Reset the temporary geometry before changing downstream piece placement.');
    return;
  }
  if (selection === null || committedPlacement === null) {
    setPlacementPolicyStatus('error', 'Select a generated candidate with a piece placement first.');
    return;
  }
  const policy = policyFromControls();
  if (policy === null) {
    setPlacementPolicyStatus('error', placementPolicyClearance.validationMessage || placementPolicyWallThickness.validationMessage);
    return;
  }

  const revision = ++policyExperimentRevision;
  setPlacementPolicyBusy(true);
  setPlacementPolicyStatus(
    'loading',
    `Reassembling ${selection.candidateId} in Rust with clearance ${policy.minimumClearanceCells} and wall thickness ${policy.wallThicknessCells}…`,
  );
  try {
    const response = await fetch('/api/experiments/placement-policy', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ candidateId: selection.candidateId, placementPolicy: policy }),
    });
    const result = (await response.json()) as PlacementPolicyExperimentResponse | PlacementPolicyExperimentError;
    if (revision !== policyExperimentRevision) {
      return;
    }
    if (!response.ok || 'error' in result) {
      throw new Error('detail' in result ? result.detail : `experiment request failed with ${response.status}`);
    }
    if (
      result.kind !== 'asha_procgen.placement_policy_experiment.v1'
      || result.candidateId !== selection.candidateId
      || result.persisted !== false
      || result.nativeAuthority !== false
    ) {
      throw new Error('placement-policy experiment returned an invalid response envelope');
    }
    currentPlacement = result.placement;
    currentPlacementValidation = result.validation;
    currentBuiltFlowValidation = null;
    currentPolicyExperimentId = result.experimentId;
    syncVoxelDoorStateControls();
    syncCorridorRealizationControls();
    syncPlacementPolicyControls();
    setPlacementPolicyStatus(
      'ready',
      `Temporary Rust placement applied: ${result.placement.instances.length} pieces, ${result.placement.occupiedCells.length} occupied cells. Not persisted; no native authority claim.`,
    );
    renderActiveView();
  } catch (error) {
    if (revision === policyExperimentRevision) {
      setPlacementPolicyStatus('error', `Placement experiment failed: ${describeError(error)}`);
    }
  } finally {
    if (revision === policyExperimentRevision) {
      setPlacementPolicyBusy(false);
    }
  }
}

function resetPlacementPolicyExperiment(): void {
  policyExperimentRevision += 1;
  currentPlacement = committedPlacement;
  currentPlacementValidation = committedPlacementValidation;
  currentBuiltFlowValidation = committedBuiltFlowValidation;
  currentPolicyExperimentId = null;
  syncVoxelDoorStateControls();
  setPlacementPolicyBusy(false);
  syncCorridorRealizationControls();
  syncPlacementPolicyControls();
  renderActiveView();
}

function syncVoxelDoorStateControls(): void {
  const previous = voxelDoorStateControl.value;
  const verified = currentPlacement !== null
    && currentBuiltFlowValidation?.ok === true
    && currentBuiltFlowValidation.placementId === currentPlacement.placementId
    && currentBuiltFlowValidation.portalCount === currentPlacement.gatePortals.length;
  const options: HTMLOptionElement[] = [];
  const initial = document.createElement('option');
  initial.value = 'initial';
  initial.textContent = 'Initial verified state';
  options.push(initial);
  if (verified && currentBuiltFlowValidation !== null) {
    for (const step of currentBuiltFlowValidation.progression.slice(1)) {
      const option = document.createElement('option');
      option.value = `step:${step.step}`;
      option.textContent = step.items.length === 0
        ? `Progression ${step.step}`
        : `After collecting ${step.items.join(', ')}`;
      options.push(option);
    }
  }
  const all = document.createElement('option');
  all.value = 'all';
  all.textContent = 'All unlocked';
  options.push(all);
  voxelDoorStateControl.replaceChildren(...options);
  voxelDoorStateControl.disabled = !verified;
  voxelDoorStateControl.value = options.some((option) => option.value === previous) ? previous : 'initial';
  voxelDoorLegend.textContent = verified
    ? 'Red locked · Blue unlocked · verified Procgen portals'
    : 'Doors hidden: this placement has no matching successful built-flow report.';
}

function voxelDoorProjectionState(placement: PiecePlacement): {
  readonly includedPortalIds: ReadonlySet<string>;
  readonly openPortalIds: ReadonlySet<string>;
  readonly label: string;
} {
  const validation = currentBuiltFlowValidation;
  const verified = validation?.ok === true
    && validation.placementId === placement.placementId
    && validation.portalCount === placement.gatePortals.length;
  if (!verified || validation === null) {
    return { includedPortalIds: new Set(), openPortalIds: new Set(), label: 'unverified doors hidden' };
  }
  const includedPortalIds = new Set(placement.gatePortals.map((portal) => portal.id));
  if (voxelDoorStateControl.value === 'all') {
    return { includedPortalIds, openPortalIds: includedPortalIds, label: 'all unlocked' };
  }
  const requestedStep = voxelDoorStateControl.value.startsWith('step:')
    ? Number.parseInt(voxelDoorStateControl.value.slice('step:'.length), 10)
    : 0;
  const step = validation.progression.find((candidate) => candidate.step === requestedStep)
    ?? validation.progression[0];
  if (step === undefined) {
    return { includedPortalIds, openPortalIds: new Set(), label: 'initial' };
  }
  return {
    includedPortalIds,
    openPortalIds: new Set(step.openPortals),
    label: step.items.length === 0 ? 'initial' : `items ${step.items.join(', ')}`,
  };
}

function syncPlacementPolicyControls(): void {
  const placement = currentPlacement;
  const enabled = placement !== null
    && currentSelection !== null
    && currentGeometryExperimentId === null
    && currentCorridorExperimentId === null;
  if (placement !== null) {
    placementPolicyClearance.value = String(placement.placementPolicy.minimumClearanceCells);
    placementPolicyWallThickness.value = String(placement.placementPolicy.wallThicknessCells);
    placementPolicyPanel.dataset.minimumClearanceCells = String(placement.placementPolicy.minimumClearanceCells);
    placementPolicyPanel.dataset.wallThicknessCells = String(placement.placementPolicy.wallThicknessCells);
  } else {
    placementPolicyClearance.value = '';
    placementPolicyWallThickness.value = '';
    delete placementPolicyPanel.dataset.minimumClearanceCells;
    delete placementPolicyPanel.dataset.wallThicknessCells;
  }
  const experimentActive = currentPolicyExperimentId !== null;
  placementPolicyPanel.dataset.mode = experimentActive ? 'experiment' : 'committed';
  placementPolicyPanel.dataset.experimentId = currentPolicyExperimentId ?? '';
  placementPolicyMode.dataset.mode = experimentActive ? 'experiment' : 'committed';
  placementPolicyMode.textContent = experimentActive ? 'Temporary experiment' : 'Committed policy';
  placementPolicyClearance.disabled = !enabled || policyExperimentBusy;
  placementPolicyWallThickness.disabled = !enabled || policyExperimentBusy;
  placementPolicyReset.disabled = !experimentActive || policyExperimentBusy;
  for (const preset of placementPolicyPresets) {
    preset.disabled = !enabled || policyExperimentBusy;
  }
  if (currentGeometryExperimentId !== null || currentCorridorExperimentId !== null) {
    setPlacementPolicyStatus('idle', 'Reset the other temporary experiment before changing downstream placement policy.');
  } else if (!enabled) {
    setPlacementPolicyStatus('idle', 'Select a generated candidate to experiment.');
  } else if (experimentActive) {
    setPlacementPolicyStatus('ready', 'Temporary Rust placement active. Not persisted; no native authority claim.');
  } else {
    setPlacementPolicyStatus('idle', 'Committed placement active. Change a value and apply to rerun Rust assembly.');
  }
  validatePlacementPolicyControls();
  updatePlacementPolicyImpact();
}

function policyFromControls(): PiecePlacementPolicy | null {
  if (!validatePlacementPolicyControls()) {
    return null;
  }
  return {
    schemaVersion: 1,
    minimumClearanceCells: Number(placementPolicyClearance.value),
    contactPolicy: 'glued_exits_only',
    wallThicknessCells: Number(placementPolicyWallThickness.value),
    doorwayWidthCells: 1,
    preservePieceBoundaries: true,
  };
}

function validatePlacementPolicyControls(): boolean {
  const clearance = Number(placementPolicyClearance.value);
  const wallThickness = Number(placementPolicyWallThickness.value);
  let clearanceIssue = '';
  let wallIssue = '';
  if (!Number.isInteger(wallThickness) || wallThickness < 1 || wallThickness > 8) {
    wallIssue = 'Wall thickness must be an integer from 1 through 8.';
  }
  const requiredClearance = wallIssue === '' ? Math.max(3, wallThickness * 2 + 1) : 3;
  placementPolicyClearance.min = String(requiredClearance);
  if (!Number.isInteger(clearance)) {
    clearanceIssue = 'Room clearance must be a whole number.';
  } else if (clearance < requiredClearance) {
    clearanceIssue = `Room clearance ${clearance} is invalid: route wall buffer ${wallThickness} requires at least ${requiredClearance} (2 × ${wallThickness} + 1).`;
  } else if (clearance > 64) {
    clearanceIssue = 'Room clearance must be 64 or less.';
  }
  placementPolicyClearance.setCustomValidity(clearanceIssue);
  placementPolicyWallThickness.setCustomValidity(wallIssue);
  const valid = clearanceIssue === '' && wallIssue === '';
  placementPolicyValidation.dataset.state = valid ? 'valid' : 'invalid';
  placementPolicyValidation.textContent = valid
    ? `Valid policy. Target room-origin scale: ${clearance + wallThickness} cells. The view will auto-fit; compare the footprint below after applying.`
    : clearanceIssue || wallIssue;
  placementPolicyApply.disabled = currentSelection === null || committedPlacement === null || policyExperimentBusy;
  if (currentGeometryExperimentId !== null) {
    placementPolicyApply.disabled = true;
  }
  if (currentCorridorExperimentId !== null) {
    placementPolicyApply.disabled = true;
  }
  return valid;
}

function setPlacementPolicyBusy(busy: boolean): void {
  policyExperimentBusy = busy;
  placementPolicyClearance.disabled = busy || currentSelection === null;
  placementPolicyWallThickness.disabled = busy || currentSelection === null;
  placementPolicyReset.disabled = busy || currentPolicyExperimentId === null;
  for (const preset of placementPolicyPresets) {
    preset.disabled = busy || currentSelection === null;
  }
  validatePlacementPolicyControls();
}

interface PlacementPolicyMetrics {
  readonly width: number;
  readonly height: number;
  readonly routedCells: number;
}

function placementPolicyMetrics(placement: PiecePlacement): PlacementPolicyMetrics {
  const cells = [...placement.occupiedCells, ...placement.connectionCells];
  if (cells.length === 0) {
    return { width: 0, height: 0, routedCells: placement.connectionCells.length };
  }
  const xs = cells.map((cell) => cell.x);
  const ys = cells.map((cell) => cell.y);
  return {
    width: Math.max(...xs) - Math.min(...xs) + 1,
    height: Math.max(...ys) - Math.min(...ys) + 1,
    routedCells: placement.connectionCells.length,
  };
}

function updatePlacementPolicyImpact(): void {
  if (committedPlacement === null) {
    placementPolicyImpact.textContent = 'Waiting for a committed placement.';
    return;
  }
  const committed = placementPolicyMetrics(committedPlacement);
  if (currentPolicyExperimentId === null || currentPlacement === null) {
    placementPolicyImpact.textContent = `Committed footprint: ${committed.width} × ${committed.height} cells; ${committed.routedCells.toLocaleString()} routed corridor cells.`;
    return;
  }
  const experiment = placementPolicyMetrics(currentPlacement);
  const routedDelta = experiment.routedCells - committed.routedCells;
  const routedDeltaLabel = `${routedDelta >= 0 ? '+' : ''}${routedDelta.toLocaleString()}`;
  placementPolicyImpact.textContent = `Generation impact: footprint ${committed.width} × ${committed.height} → ${experiment.width} × ${experiment.height}; routed corridor cells ${committed.routedCells.toLocaleString()} → ${experiment.routedCells.toLocaleString()} (${routedDeltaLabel}). Camera auto-fit can make the visual scale look similar.`;
}

function setPlacementPolicyStatus(state: 'idle' | 'loading' | 'ready' | 'error', message: string): void {
  placementPolicyStatus.dataset.state = state;
  placementPolicyStatus.textContent = message;
}

async function fetchBatch(): Promise<SelectionReport> {
  const response = await fetch('/api/batches/v2');
  if (!response.ok) {
    return {
      batchId: 'first-run-fallback',
      requestedCount: 1,
      generatedCount: 1,
      accepted: [],
      rejected: [],
    };
  }
  return (await response.json()) as SelectionReport;
}

async function fetchGenerationConfig(): Promise<ViewerGenerationConfig> {
  const response = await fetch('/api/generation-config');
  const result = await readJsonResponse<
    ViewerGenerationConfig | PlacementPolicyExperimentError
  >(response);
  if (!response.ok || 'error' in result) {
    throw new Error(
      'detail' in result
        ? result.detail
        : `failed to load generation configuration: ${response.status}`,
    );
  }
  if (
    result.kind !== 'asha_procgen.viewer_generation_config.v1'
    || result.schemaVersion !== 1
  ) {
    throw new Error('generation configuration returned an invalid response envelope');
  }
  return result;
}

async function readJsonResponse<T>(response: Response): Promise<T> {
  const body = await response.text();
  try {
    return JSON.parse(body) as T;
  } catch {
    const summary = body.trim().slice(0, 160) || '(empty response)';
    throw new Error(`request returned ${response.status} with non-JSON content: ${summary}`);
  }
}

async function fetchVoxelEvidence(): Promise<NativeVoxelEvidence | null> {
  const response = await fetch('/api/evidence/native-voxel-extrusion');
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as NativeVoxelEvidence;
}

async function fetchArtifact(url: string): Promise<AcceptedArtifact> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`failed to load artifact: ${response.status}`);
  }
  return (await response.json()) as AcceptedArtifact;
}

async function fetchValidation(url: string): Promise<ValidationReport> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`failed to load validation: ${response.status}`);
  }
  return (await response.json()) as ValidationReport;
}

async function fetchIntermediateContext(entry: SelectionEntry): Promise<IntermediateContext> {
  const [spatialIntent, breakdown, validation] = await Promise.all([
    fetchOptionalArtifact<SpatialIntentReport>(entry.spatialIntentRef),
    fetchOptionalArtifact<IntermediateBreakdown>(entry.intermediateBreakdownRef),
    fetchOptionalArtifact<ValidationReport>(entry.intermediateValidationRef),
  ]);
  return { spatialIntent, breakdown, validation };
}

async function fetchOptionalArtifact<T>(path: string | undefined): Promise<T | null> {
  if (path === undefined) {
    return null;
  }
  const response = await fetch(artifactUrl(path));
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as T;
}

async function fetchCatalogForEntry(
  entry: SelectionEntry,
  placement: PiecePlacement | null,
): Promise<{
  readonly catalog: ShapeCatalog | null;
  readonly ref: string | null;
  readonly error: string | null;
}> {
  const refs = [
    entry.shapeCatalogRef,
    placement?.sourceCatalogRef,
    'fixtures/shape-catalogs/2d-basic.json',
  ].filter((value, index, values): value is string => {
    return value !== undefined && values.indexOf(value) === index;
  });
  for (const ref of refs) {
    for (const url of catalogUrls(ref)) {
      try {
        const response = await fetch(url);
        if (!response.ok) {
          continue;
        }
        return {
          catalog: (await response.json()) as ShapeCatalog,
          ref,
          error: null,
        };
      } catch {
        // Try the next URL/ref. The visible tab reports the final failure below.
      }
    }
  }
  return {
    catalog: null,
    ref: refs[0] ?? null,
    error: refs.length === 0
      ? 'no catalog ref was available'
      : `failed to load ${refs.join(', ')}`,
  };
}

function catalogUrls(ref: string): readonly string[] {
  const urls = [artifactUrl(ref)];
  if (ref.startsWith('fixtures/')) {
    urls.push(`/${ref}`);
  }
  return urls;
}

async function fetchDefaultCatalog(): Promise<ShapeCatalog | null> {
  for (const url of catalogUrls('fixtures/shape-catalogs/2d-basic.json')) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return (await response.json()) as ShapeCatalog;
      }
    } catch {
      // Best-effort fallback for first-run/no-batch mode.
    }
  }
  return null;
}

function emptyIntermediateContext(): IntermediateContext {
  return {
    spatialIntent: null,
    breakdown: null,
    validation: null,
  };
}

function initialViewMode(): ViewMode {
  if (location.hash === '#catalog') {
    return 'catalog';
  }
  if (location.hash === '#intermediate') {
    return 'intermediate';
  }
  if (location.hash === '#build') {
    return 'build';
  }
  if (location.hash === '#voxel') {
    return 'voxel';
  }
  if (location.hash === '#voxel3d') {
    return 'voxel3d';
  }
  return 'layout';
}

function artifactUrl(path: string): string {
  return `/api/artifacts/by-path?path=${encodeURIComponent(path)}`;
}

function renderBatchList(
  target: HTMLElement,
  report: SelectionReport,
  selectedCandidateId: string | null,
  onSelect: (entry: SelectionEntry) => void,
): void {
  const header = document.createElement('div');
  header.className = 'batch-header';
  header.append(
    metric('Batch', report.batchId),
    metric('Accepted', `${report.accepted.length}/${report.generatedCount}`),
  );

  const buttons = report.accepted.map((entry, index) => {
    const button = document.createElement('button');
    button.className = 'candidate-button';
    button.type = 'button';
    button.dataset.candidateId = entry.candidateId;
    button.dataset.selected = entry.candidateId === selectedCandidateId ? 'true' : 'false';
    button.addEventListener('click', () => onSelect(entry));

    const rank = document.createElement('span');
    rank.className = 'candidate-rank';
    rank.textContent = String(index + 1).padStart(2, '0');
    const name = document.createElement('span');
    name.className = 'candidate-name';
    name.textContent = shortCandidate(entry.candidateId);
    const score = document.createElement('span');
    score.className = 'candidate-score';
    score.textContent = entry.overall.toFixed(2);
    const tags = document.createElement('span');
    tags.className = 'candidate-tags';
    tags.textContent = entry.tags.slice(0, 4).join(' / ');

    button.append(rank, name, score, tags);
    return button;
  });

  target.replaceChildren(header, ...buttons);
}

function renderSummary(
  target: HTMLElement,
  artifact: AcceptedArtifact,
  selection: SelectionEntry | null,
  report: SelectionReport,
): void {
  const metrics = artifact.scoreSummary.metrics;
  const topTags = selection?.tags.slice(0, 8).join(', ') ?? 'first-run';
  target.replaceChildren(
    metric('Artifact', artifact.artifactId),
    metric('Candidate', artifact.layout.candidateId),
    metric('Overall', artifact.scoreSummary.overall.toFixed(2)),
    metric('Nodes', String(metrics.nodeCount ?? artifact.layout.rooms.length)),
    metric('Edges', String(metrics.edgeCount ?? artifact.layout.links.length)),
    metric('Loops', String(metrics.loopCount ?? 0)),
    metric('Hubs', String(metrics.hubCount ?? 0)),
    metric('Pressure', String(metrics.pressureEdgeCount ?? 0)),
    metric('Profile', selection?.profileSequence ?? 'first-run'),
    metric('Rejected', String(report.rejected.length)),
    metric('Tags', topTags),
  );
}

function renderContext(
  target: HTMLElement,
  artifact: AcceptedArtifact,
  selection: SelectionEntry | null,
  report: SelectionReport,
  validation: ValidationReport,
  intermediate: IntermediateContext,
  placementValidation: ValidationReport | null,
): void {
  target.replaceChildren(
    contextSection('Artifact Refs', [
      refLine('artifact', selection?.artifactRef ?? '/api/artifacts/first-run'),
      refLine('validation', artifact.validationRef),
      refLine('score', artifact.scoreRef),
      refLine('layout', selection?.layoutRef ?? artifact.layout.layoutId),
      refLine('profile', report.profileRef ?? 'first-run'),
    ]),
    contextSection('Intermediate Refs', intermediateRefLines(selection)),
    contextSection('Build Refs', buildRefLines(selection)),
    contextSection('Piece Placement', piecePlacementLines(selection, placementValidation)),
    contextSection('Intermediate', intermediateLines(intermediate)),
    contextSection('Validation', validationLines(validation)),
    contextSection('Provenance', provenanceLines(artifact.candidate.provenance)),
    contextSection('Batch Rejections', rejectionLines(report)),
  );
}

function buildRefLines(selection: SelectionEntry | null): readonly HTMLElement[] {
  if (selection === null || selection.geometryRef === undefined) {
    const empty = document.createElement('p');
    empty.className = 'diagnostic-empty';
    empty.textContent = 'No geometry/build artifact refs are available for this selection.';
    return [empty];
  }
  return [
    refLine('connections', selection.physicalConnectionPlanRef ?? 'missing'),
    refLine('geometry', selection.geometryRef),
    refLine('gvalid', selection.geometryValidationRef ?? 'missing'),
    refLine('preview', selection.htmlPreviewRef ?? 'missing'),
    refLine('html', selection.htmlRef ?? 'missing'),
    refLine('catalog', selection.shapeCatalogRef ?? 'missing'),
    refLine('creport', selection.catalogInspectionRef ?? 'missing'),
    refLine('plan', selection.piecePlanRef ?? 'missing'),
    refLine('match', selection.shapeMatchRef ?? 'missing'),
    refLine('place', selection.piecePlacementRef ?? 'missing'),
    refLine('pvalid', selection.piecePlacementValidationRef ?? 'missing'),
  ];
}

function piecePlacementLines(
  selection: SelectionEntry | null,
  validation: ValidationReport | null,
): readonly HTMLElement[] {
  if (selection === null || selection.piecePlacementRef === undefined) {
    const empty = document.createElement('p');
    empty.className = 'diagnostic-empty';
    empty.textContent = 'No catalog piece placement loaded; Build tab will use geometry fallback.';
    return [empty];
  }
  const lines = [
    contextLine('placement', selection.piecePlacementRef),
    contextLine('shape match', selection.shapeMatchRef ?? 'missing'),
  ];
  if (validation !== null) {
    lines.push(...validationLines(validation));
  }
  return lines;
}

function intermediateRefLines(selection: SelectionEntry | null): readonly HTMLElement[] {
  if (selection === null || selection.intermediateBreakdownRef === undefined) {
    const empty = document.createElement('p');
    empty.className = 'diagnostic-empty';
    empty.textContent = 'No intermediate artifact refs are available for this selection.';
    return [empty];
  }
  return [
    refLine('analysis', selection.analysisRef ?? 'missing'),
    refLine('rules', selection.compatibleRulesRef ?? 'missing'),
    refLine('intent', selection.spatialIntentRef ?? 'missing'),
    refLine('breakdown', selection.intermediateBreakdownRef),
    refLine('ivalid', selection.intermediateValidationRef ?? 'missing'),
  ];
}

function intermediateLines(intermediate: IntermediateContext): readonly HTMLElement[] {
  const lines: HTMLElement[] = [];
  if (intermediate.breakdown === null) {
    const empty = document.createElement('p');
    empty.className = 'diagnostic-empty';
    empty.textContent = 'No intermediate breakdown loaded.';
    lines.push(empty);
  } else {
    lines.push(
      contextLine(
        `schema ${intermediate.breakdown.schemaVersion}`,
        `${intermediate.breakdown.regions.length} regions / ${intermediate.breakdown.connectors.length} connectors / ${intermediate.breakdown.constraints.length} constraints`,
      ),
    );
    const roles = tally(intermediate.breakdown.regions.map((region) => region.role));
    lines.push(contextLine('region roles', roles.join(', ')));
    const affordances = tally(
      intermediate.breakdown.connectors.flatMap((connector) => connector.affordances ?? []),
    );
    lines.push(contextLine('affordances', affordances.join(', ') || 'none'));
  }
  if (intermediate.spatialIntent !== null) {
    lines.push(
      contextLine('spatial intent', `${intermediate.spatialIntent.annotations.length} annotations`),
    );
  }
  if (intermediate.validation !== null) {
    lines.push(...validationLines(intermediate.validation));
  }
  return lines;
}

function validationLines(validation: ValidationReport): readonly HTMLElement[] {
  const status = document.createElement('p');
  status.className = validation.ok ? 'status-ok' : 'status-fail';
  status.textContent = validation.ok
    ? 'ok'
    : `${validation.fatalCount} fatal diagnostic(s)`;
  const diagnostics = validation.diagnostics.map(diagnosticLine);
  if (diagnostics.length === 0) {
    const empty = document.createElement('p');
    empty.className = 'diagnostic-empty';
    empty.textContent = 'No validation diagnostics for the selected candidate.';
    return [status, empty];
  }
  return [status, ...diagnostics];
}

function provenanceLines(provenance: readonly ProvenanceStep[]): readonly HTMLElement[] {
  return provenance.slice(-8).map((step) => {
    const item = document.createElement('p');
    item.className = 'context-line';
    const seedText = step.seed === null ? '' : ` seed ${step.seed}`;
    item.textContent = `${step.step}. ${step.command}${seedText}`;
    const detail = document.createElement('small');
    detail.textContent = step.summary;
    item.append(detail);
    return item;
  });
}

function contextLine(label: string, detailText: string): HTMLElement {
  const item = document.createElement('p');
  item.className = 'context-line';
  item.textContent = label;
  const detail = document.createElement('small');
  detail.textContent = detailText;
  item.append(detail);
  return item;
}

function tally(values: readonly string[]): readonly string[] {
  const counts = new Map<string, number>();
  for (const value of values) {
    counts.set(value, (counts.get(value) ?? 0) + 1);
  }
  return [...counts.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([value, count]) => `${value}:${count}`);
}

function rejectionLines(report: SelectionReport): readonly HTMLElement[] {
  if (report.rejected.length === 0) {
    const empty = document.createElement('p');
    empty.className = 'diagnostic-empty';
    empty.textContent = 'No rejected candidates in the current sample batch.';
    return [empty];
  }
  return report.rejected.flatMap((rejection) =>
    rejection.diagnostics.map((diagnostic) => {
      const line = diagnosticLine(diagnostic);
      line.prepend(`${shortCandidate(rejection.candidateId)} `);
      return line;
    }),
  );
}

function diagnosticLine(diagnostic: Diagnostic): HTMLElement {
  const item = document.createElement('p');
  item.className = `diagnostic-line ${diagnostic.severity}`;
  const code = document.createElement('strong');
  code.textContent = diagnostic.code;
  const detail = document.createElement('span');
  detail.textContent = ` ${diagnostic.detail}`;
  item.append(code, detail);
  if (diagnostic.repairHint !== undefined) {
    const repair = document.createElement('small');
    repair.textContent = diagnostic.repairHint;
    item.append(repair);
  }
  return item;
}

function contextSection(title: string, children: readonly HTMLElement[]): HTMLElement {
  const section = document.createElement('section');
  section.className = 'context-section';
  const heading = document.createElement('h2');
  heading.textContent = title;
  section.append(heading, ...children);
  return section;
}

function refLine(label: string, value: string): HTMLElement {
  const line = document.createElement('p');
  line.className = 'ref-line';
  const labelElement = document.createElement('strong');
  labelElement.textContent = label;
  const valueElement = document.createElement('span');
  valueElement.textContent = value;
  line.append(labelElement, valueElement);
  return line;
}

function metric(label: string, value: string): HTMLElement {
  const wrapper = document.createElement('div');
  wrapper.className = 'metric';
  const labelElement = document.createElement('span');
  labelElement.className = 'metric-label';
  labelElement.textContent = label;
  const valueElement = document.createElement('span');
  valueElement.className = 'metric-value';
  valueElement.textContent = value;
  wrapper.append(labelElement, valueElement);
  return wrapper;
}

function renderActiveView(): void {
  for (const tab of viewTabs) {
    tab.dataset.selected = tab.dataset.view === activeView ? 'true' : 'false';
  }
  layoutSvg.style.height = '';
  layoutSvg.style.minWidth = '';
  const generationConfigVisible = activeView === 'build' || activeView === 'voxel' || activeView === 'voxel3d';
  generationConfigPanel.hidden = !generationConfigVisible;
  geometryPolicyPanel.hidden = true;
  corridorRealizationPanel.hidden = true;
  placementPolicyPanel.hidden = true;
  const inspectionActive = activeView === 'voxel3d';
  layoutSvg.style.display = inspectionActive ? 'none' : '';
  voxelInspectionPanel.hidden = !inspectionActive;
  if (!inspectionActive) {
    voxelInspectionRevision += 1;
    voxelInspectionSurface?.stop();
    stopVoxelInspectionReadoutSync();
  }
  if (inspectionActive) {
    void renderVoxelInspection();
    return;
  }
  if (activeView === 'build') {
    renderBuildGrid(layoutSvg, currentGeometry, currentPlacement, currentPlacementValidation);
    return;
  }
  if (activeView === 'voxel') {
    const projectionMode = configuredBuildId !== null
      ? 'configured'
      : currentPolicyExperimentId !== null
          || currentGeometryExperimentId !== null
          || currentCorridorExperimentId !== null
        ? 'temporary'
        : 'committed';
    renderVoxelBuild(
      layoutSvg,
      currentPlacement,
      voxelEvidence,
      projectionMode,
    );
    return;
  }
  if (activeView === 'catalog') {
    renderShapeCatalog(layoutSvg, currentCatalog, currentCatalogRef, currentCatalogError);
    return;
  }
  if (activeView === 'intermediate') {
    renderIntermediate(layoutSvg, currentIntermediate.breakdown);
    return;
  }
  if (currentLayout === null) {
    renderEmptySvg(layoutSvg, 'No layout loaded.');
    return;
  }
  renderLayout(layoutSvg, currentLayout);
}

async function renderVoxelInspection(): Promise<void> {
  const revision = ++voxelInspectionRevision;
  let projection: VoxelInspectionProjection;
  let doorPreviewLabel = 'unverified doors hidden';
  try {
    if (currentPlacement === null) {
      throw new Error('no piece placement is available for voxel extrusion');
    }
    const doorState = voxelDoorProjectionState(currentPlacement);
    doorPreviewLabel = doorState.label;
    projection = buildVoxelInspectionProjection(
      compilePlacementExtrusion(currentPlacement),
      doorState,
    );
    if (projection.frame.ops.length > ASHA_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS) {
      throw new Error(
        `projection has ${projection.frame.ops.length} ops; engine host limit is ${ASHA_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS}`,
      );
    }
  } catch (error) {
    setVoxelInspectionDiagnostic('error', `Voxel 3D unavailable: ${describeError(error)}`);
    voxelInspectionSurface?.stop();
    return;
  }

  voxelInspectionPanel.dataset.placementId = projection.placementId;
  voxelInspectionPanel.dataset.projectedVoxelCount = String(projection.projectedVoxelCount);
  voxelInspectionPanel.dataset.projectedNodeCount = String(projection.projectedNodeCount);
  voxelInspectionPanel.dataset.omittedCeilingVoxelCount = String(projection.omittedCeilingVoxelCount);
  voxelInspectionPanel.dataset.doorNodeCount = String(projection.doorNodeCount);
  voxelInspectionPanel.dataset.lockedDoorCount = String(projection.lockedDoorCount);
  voxelInspectionPanel.dataset.unlockedDoorCount = String(projection.unlockedDoorCount);
  voxelInspectionPanel.dataset.doorPreviewState = doorPreviewLabel;
  voxelInspectionPanel.dataset.ceilingY = String(projection.ceilingY);
  const activeExperimentId = currentGeometryExperimentId
    ?? currentPolicyExperimentId
    ?? currentCorridorExperimentId;
  voxelInspectionPanel.dataset.policyMode = configuredBuildId !== null
    ? 'configured'
    : activeExperimentId === null
      ? 'committed'
      : 'experiment';
  voxelInspectionPanel.dataset.policyExperimentId = activeExperimentId ?? '';
  setVoxelInspectionDiagnostic('loading', `Mounting engine projection for ${projection.placementId}…`);

  try {
    const surface = await ensureVoxelInspectionSurface(projection);
    if (revision !== voxelInspectionRevision || activeView !== 'voxel3d') {
      surface.stop();
      return;
    }
    const receipt = surface.replaceFrame(projection.frame);
    if (!receipt.applied) {
      const detail = receipt.diagnostics.map((diagnostic) => diagnostic.message).join('; ');
      throw new Error(detail || 'engine renderer host rejected the projection frame');
    }
    const gridReceipt = surface.setGrid(projection.grid);
    if (!gridReceipt.applied || gridReceipt.grid === null) {
      const detail = gridReceipt.diagnostics.map((diagnostic) => diagnostic.message).join('; ');
      throw new Error(detail || 'engine renderer host rejected the inspection grid');
    }
    surface.resizeToCanvas();
    surface.renderOnce();
    if (!renderInspectionOnce) {
      surface.start();
    }
    const readout = syncVoxelInspectionReadout(surface);
    startVoxelInspectionReadoutSync(surface);
    voxelInspectionPanel.dataset.rendererHost = surface.kind;
    voxelInspectionPanel.dataset.rendererAuthority = surface.authority;
    voxelInspectionPanel.dataset.rendererStatus = readout.status;
    voxelInspectionPanel.dataset.frameHash = readout.retainedFrameHash;
    const pickPoints = Array.from({ length: 9 }, (_, row) =>
      Array.from({ length: 9 }, (_, column) => [
        voxelInspectionCanvas.clientWidth * (column + 1) / 10,
        voxelInspectionCanvas.clientHeight * (row + 1) / 10,
      ] as const),
    ).flat();
    const pickHits = pickPoints
      .map((point) => surface.pick({ point }).hint)
      .filter((hint) => hint !== null);
    const focusedDoor = pickHits
      .map((hint) => String(hint))
      .find((hint) => hint.includes('procgen-door:')) ?? '';
    voxelInspectionPanel.dataset.pickHitCount = String(pickHits.length);
    voxelInspectionPanel.dataset.focusedDoor = focusedDoor;
    voxelDoorLegend.textContent = focusedDoor.length > 0
      ? `Red locked · Blue unlocked · ${doorPreviewLabel} · ${focusedDoor}`
      : `Red locked · Blue unlocked · ${doorPreviewLabel} · pick a door for source edge/item details`;
    setVoxelInspectionDiagnostic(
      'ready',
      `${projection.projectedVoxelCount} floor/wall voxels in ${projection.projectedNodeCount} compacted nodes · ${projection.lockedDoorCount} locked red / ${projection.unlockedDoorCount} unlocked blue doors · ${projection.omittedCeilingVoxelCount} ceiling voxels omitted · engine frame ${readout.retainedFrameHash}`,
    );
  } catch (error) {
    if (revision === voxelInspectionRevision) {
      setVoxelInspectionDiagnostic('error', `Engine renderer unavailable: ${describeError(error)}`);
    }
  }
}

async function ensureVoxelInspectionSurface(
  projection: VoxelInspectionProjection,
): Promise<AshaRendererInspectionSurface> {
  if (voxelInspectionSurface !== null) {
    return voxelInspectionSurface;
  }
  voxelInspectionMount ??= mountAshaRendererInspectionSurface(voxelInspectionCanvas, {
    autoStart: false,
    clearColor: 0x10151c,
    frame: projection.frame,
    initialGrid: projection.grid,
    controls: {
      initialPosition: projection.camera.position,
      initialTarget: projection.camera.target,
      moveSpeed: projection.camera.moveSpeed,
      orbitDegreesPerPixel: 0.22,
    },
  });
  try {
    voxelInspectionSurface = await voxelInspectionMount;
    return voxelInspectionSurface;
  } catch (error) {
    voxelInspectionMount = null;
    throw error;
  }
}

function syncVoxelInspectionReadout(
  surface: AshaRendererInspectionSurface,
): ReturnType<AshaRendererInspectionSurface['readout']> {
  const readout = surface.readout();
  voxelInspectionPanel.dataset.cameraRevision = String(readout.cameraRevision);
  voxelInspectionPanel.dataset.cameraDistance = readout.cameraDistance.toFixed(3);
  voxelInspectionPanel.dataset.lastCameraChange = readout.lastCameraChange;
  voxelInspectionPanel.dataset.dragging = String(readout.dragging);
  voxelInspectionPanel.dataset.pressedMovementKeys = readout.pressedMovementKeys.join(',');
  voxelInspectionPanel.dataset.pressedOrbitKeys = readout.pressedOrbitKeys.join(',');
  voxelInspectionPanel.dataset.gridRevision = String(readout.gridRevision);
  voxelInspectionPanel.dataset.gridLineCount = String(readout.grid?.renderedLineCount ?? 0);
  return readout;
}

function startVoxelInspectionReadoutSync(surface: AshaRendererInspectionSurface): void {
  stopVoxelInspectionReadoutSync();
  const sync = (): void => {
    if (surface !== voxelInspectionSurface || activeView !== 'voxel3d') {
      voxelInspectionReadoutFrame = null;
      return;
    }
    syncVoxelInspectionReadout(surface);
    voxelInspectionReadoutFrame = requestAnimationFrame(sync);
  };
  voxelInspectionReadoutFrame = requestAnimationFrame(sync);
}

function stopVoxelInspectionReadoutSync(): void {
  if (voxelInspectionReadoutFrame !== null) {
    cancelAnimationFrame(voxelInspectionReadoutFrame);
    voxelInspectionReadoutFrame = null;
  }
}

function setVoxelInspectionDiagnostic(state: 'loading' | 'ready' | 'error', message: string): void {
  voxelInspectionDiagnostic.dataset.state = state;
  voxelInspectionDiagnostic.textContent = message;
}

interface VoxelPoint {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

interface ProjectedPoint {
  readonly x: number;
  readonly y: number;
}

function renderVoxelBuild(
  target: SVGSVGElement,
  placement: PiecePlacement | null,
  evidence: NativeVoxelEvidence | null,
  mode: 'committed' | 'temporary' | 'configured',
): void {
  target.replaceChildren();
  if (placement === null) {
    renderEmptySvg(target, 'No piece placement is available for voxel extrusion.');
    return;
  }

  let plan: VoxelExtrusionPlan;
  try {
    plan = compilePlacementExtrusion(placement);
  } catch (error) {
    renderEmptySvg(target, `Voxel extrusion unavailable: ${describeError(error)}`);
    return;
  }

  const margin = 36;
  const headerHeight = 112;
  const tileWidth = 15;
  const tileHeight = 8;
  const voxelHeight = 13;
  const bounds = projectedVoxelBounds(plan, tileWidth, tileHeight, voxelHeight);
  const width = Math.max(900, Math.ceil(bounds.maxX - bounds.minX + margin * 2));
  const height = Math.max(620, Math.ceil(bounds.maxY - bounds.minY + margin * 2 + headerHeight));
  const offsetX = margin - bounds.minX;
  const offsetY = margin + headerHeight - bounds.minY;
  target.setAttribute('viewBox', `0 0 ${width} ${height}`);
  target.style.height = `${height}px`;
  target.style.minWidth = `${width}px`;

  const title = createSvg('text');
  title.setAttribute('class', 'voxel-title');
  title.setAttribute('x', String(margin));
  title.setAttribute('y', '30');
  title.textContent = 'Native Voxel Extrusion Cutaway';
  target.append(title);

  const verified = mode === 'committed' && evidence?.placementId === placement.placementId;
  const detail = createSvg('text');
  detail.setAttribute('class', `voxel-detail ${verified ? 'verified' : 'unverified'}`);
  detail.setAttribute('x', String(margin));
  detail.setAttribute('y', '53');
  detail.textContent = verified && evidence !== null
    ? `${plan.solidVoxelCount} voxels / ${evidence.authority.acceptedCommands} native commands / ${evidence.authority.voxelStateHash}`
    : mode === 'temporary'
      ? `${plan.solidVoxelCount} voxel experiment / temporary Rust placement / no native authority receipt`
      : mode === 'configured'
        ? `${plan.solidVoxelCount} voxel configured build / persisted Rust placement / no native authority receipt`
      : `${plan.solidVoxelCount} voxel proposal / selected placement has no matching native authority receipt`;
  target.append(detail);

  const source = createSvg('text');
  source.setAttribute('class', 'voxel-source');
  source.setAttribute('x', String(margin));
  source.setAttribute('y', '75');
  source.textContent = verified && evidence !== null
    ? `ASHA ${evidence.ashaEngineCommit.slice(0, 12)} / deterministic ${evidence.authority.deterministic ? 'yes' : 'no'} / XZ floor plan with ghosted ceiling`
    : `${mode === 'temporary' ? 'temporary policy experiment / ' : mode === 'configured' ? 'persisted generation config / ' : ''}${placement.placementId} / XZ floor plan with ghosted ceiling`;
  target.append(source);

  appendVoxelLegend(target, margin, 91);

  const solidKeys = new Set(plan.solidVoxels.map((voxel) => voxelKey3(voxel.coord)));
  const voxels = [...plan.solidVoxels].sort((left, right) => {
    const leftDepth = left.coord.x + left.coord.z;
    const rightDepth = right.coord.x + right.coord.z;
    return leftDepth - rightDepth || left.coord.y - right.coord.y || left.coord.x - right.coord.x;
  });
  for (const voxel of voxels) {
    const materialClass = voxelMaterialClass(voxel.material);
    const coord = voxel.coord;
    if (!solidKeys.has(voxelKey3({ x: coord.x, y: coord.y + 1, z: coord.z }))) {
      appendVoxelFace(target, voxelFacePoints(coord, 'top'), materialClass, 'top', offsetX, offsetY, tileWidth, tileHeight, voxelHeight);
    }
    if (!solidKeys.has(voxelKey3({ x: coord.x + 1, y: coord.y, z: coord.z }))) {
      appendVoxelFace(target, voxelFacePoints(coord, 'east'), materialClass, 'east', offsetX, offsetY, tileWidth, tileHeight, voxelHeight);
    }
    if (!solidKeys.has(voxelKey3({ x: coord.x, y: coord.y, z: coord.z + 1 }))) {
      appendVoxelFace(target, voxelFacePoints(coord, 'south'), materialClass, 'south', offsetX, offsetY, tileWidth, tileHeight, voxelHeight);
    }
  }
}

function projectedVoxelBounds(
  plan: VoxelExtrusionPlan,
  tileWidth: number,
  tileHeight: number,
  voxelHeight: number,
): { readonly minX: number; readonly minY: number; readonly maxX: number; readonly maxY: number } {
  const min = plan.buildBounds.min;
  const max = plan.buildBounds.maxExclusive;
  const corners: VoxelPoint[] = [];
  for (const x of [min.x, max.x]) {
    for (const y of [min.y, max.y]) {
      for (const z of [min.z, max.z]) {
        corners.push({ x, y, z });
      }
    }
  }
  const projected = corners.map((point) => projectVoxel(point, tileWidth, tileHeight, voxelHeight));
  return {
    minX: Math.min(...projected.map((point) => point.x)),
    minY: Math.min(...projected.map((point) => point.y)),
    maxX: Math.max(...projected.map((point) => point.x)),
    maxY: Math.max(...projected.map((point) => point.y)),
  };
}

function voxelFacePoints(coord: VoxelPoint, face: 'top' | 'east' | 'south'): readonly VoxelPoint[] {
  const { x, y, z } = coord;
  if (face === 'top') {
    return [
      { x, y: y + 1, z },
      { x: x + 1, y: y + 1, z },
      { x: x + 1, y: y + 1, z: z + 1 },
      { x, y: y + 1, z: z + 1 },
    ];
  }
  if (face === 'east') {
    return [
      { x: x + 1, y, z },
      { x: x + 1, y: y + 1, z },
      { x: x + 1, y: y + 1, z: z + 1 },
      { x: x + 1, y, z: z + 1 },
    ];
  }
  return [
    { x, y, z: z + 1 },
    { x, y: y + 1, z: z + 1 },
    { x: x + 1, y: y + 1, z: z + 1 },
    { x: x + 1, y, z: z + 1 },
  ];
}

function appendVoxelFace(
  target: SVGSVGElement,
  points: readonly VoxelPoint[],
  materialClass: string,
  face: string,
  offsetX: number,
  offsetY: number,
  tileWidth: number,
  tileHeight: number,
  voxelHeight: number,
): void {
  const polygon = createSvg('polygon');
  polygon.setAttribute('class', `voxel-face ${materialClass} ${face}`);
  polygon.setAttribute('points', points.map((point) => {
    const projected = projectVoxel(point, tileWidth, tileHeight, voxelHeight);
    return `${projected.x + offsetX},${projected.y + offsetY}`;
  }).join(' '));
  target.append(polygon);
}

function projectVoxel(
  point: VoxelPoint,
  tileWidth: number,
  tileHeight: number,
  voxelHeight: number,
): ProjectedPoint {
  return {
    x: (point.x - point.z) * tileWidth / 2,
    y: (point.x + point.z) * tileHeight / 2 - point.y * voxelHeight,
  };
}

function appendVoxelLegend(target: SVGSVGElement, x: number, y: number): void {
  const entries = [
    ['wall', 'Wall'],
    ['floor', 'Floor'],
    ['ceiling', 'Ceiling (ghosted)'],
  ] as const;
  entries.forEach(([className, label], index) => {
    const swatch = createSvg('rect');
    swatch.setAttribute('class', `voxel-legend-swatch ${className}`);
    swatch.setAttribute('x', String(x + index * 104));
    swatch.setAttribute('y', String(y));
    swatch.setAttribute('width', '12');
    swatch.setAttribute('height', '12');
    target.append(swatch);
    const text = createSvg('text');
    text.setAttribute('class', 'voxel-legend-label');
    text.setAttribute('x', String(x + 17 + index * 104));
    text.setAttribute('y', String(y + 11));
    text.textContent = label;
    target.append(text);
  });
}

function voxelMaterialClass(material: number): string {
  if (material === 1) {
    return 'wall';
  }
  if (material === 2) {
    return 'floor';
  }
  if (material === 3) {
    return 'ceiling';
  }
  return 'unknown';
}

function voxelKey3(point: VoxelPoint): string {
  return `${point.x},${point.y},${point.z}`;
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function renderShapeCatalog(
  target: SVGSVGElement,
  catalog: ShapeCatalog | null,
  catalogRef: string | null,
  catalogError: string | null,
): void {
  target.replaceChildren();
  if (catalog === null) {
    const detail = catalogError ?? (catalogRef === null ? 'no catalog ref was available' : `could not load ${catalogRef}`);
    renderEmptySvg(target, `No build piece catalog loaded: ${detail}`);
    return;
  }

  const margin = 24;
  const headerHeight = 68;
  const columns = 2;
  const cardWidth = 306;
  const cardHeight = 214;
  const gap = 12;
  const rows = Math.ceil(catalog.shapes.length / columns);
  const width = margin * 2 + columns * cardWidth + (columns - 1) * gap;
  const height = margin * 2 + headerHeight + rows * cardHeight + Math.max(0, rows - 1) * gap;
  target.setAttribute('viewBox', `0 0 ${width} ${height}`);
  target.style.height = `${height}px`;
  target.style.minWidth = `${width}px`;

  const title = createSvg('text');
  title.setAttribute('class', 'intermediate-role-label');
  title.setAttribute('x', String(margin));
  title.setAttribute('y', '28');
  title.textContent = 'Build Piece Catalog';
  target.append(title);

  const stats = createSvg('text');
  stats.setAttribute('class', 'intermediate-region-detail');
  stats.setAttribute('x', String(margin));
  stats.setAttribute('y', '50');
  stats.textContent = `${catalog.catalogId} / ${catalog.shapes.length} shapes / clearance ${catalog.placementPolicy.minimumClearanceCells} / wall ${catalog.placementPolicy.wallThicknessCells} / door ${catalog.placementPolicy.doorwayWidthCells} / ${catalogRef ?? 'catalog ref unknown'}`;
  target.append(stats);

  for (const [index, shape] of catalog.shapes.entries()) {
    const column = index % columns;
    const row = Math.floor(index / columns);
    const x = margin + column * (cardWidth + gap);
    const y = margin + headerHeight + row * (cardHeight + gap);
    renderCatalogShapeCard(target, shape, x, y, cardWidth, cardHeight);
  }
}

function renderCatalogShapeCard(
  target: SVGSVGElement,
  shape: CatalogShape,
  x: number,
  y: number,
  width: number,
  height: number,
): void {
  const group = createSvg('g');
  group.setAttribute('class', `catalog-shape-card ${slugClass(shape.pieceKinds[0] ?? 'piece')}`);
  group.setAttribute('transform', `translate(${x} ${y})`);

  const frame = createSvg('rect');
  frame.setAttribute('class', 'catalog-card-frame');
  frame.setAttribute('width', String(width));
  frame.setAttribute('height', String(height));
  frame.setAttribute('rx', '6');
  group.append(frame);

  const title = createSvg('text');
  title.setAttribute('class', 'catalog-card-title');
  title.setAttribute('x', '12');
  title.setAttribute('y', '22');
  title.textContent = truncateText(shape.label, 30);
  group.append(title);

  const subtitle = createSvg('text');
  subtitle.setAttribute('class', 'catalog-card-detail');
  subtitle.setAttribute('x', '12');
  subtitle.setAttribute('y', '42');
  subtitle.textContent = shape.shapeId.replace('shape.', '');
  group.append(subtitle);

  renderCatalogMiniShape(group, shape, 14, 60);

  const metadataX = 132;
  const lines = [
    `kinds: ${shape.pieceKinds.join(', ')}`,
    `exits: ${shape.exits.map((exit) => exit.direction[0]?.toUpperCase() ?? '?').join(' ') || 'none'}`,
    `sockets: ${shape.featureSockets.map((socket) => socket.kind).join(', ') || 'none'}`,
    `xforms: ${shape.allowedTransforms.map(shortTransform).join(' ')}`,
    `tags: ${shape.tags.slice(0, 4).join(', ')}`,
  ];
  lines.forEach((line, index) => {
    const text = createSvg('text');
    text.setAttribute('class', 'catalog-card-detail');
    text.setAttribute('x', String(metadataX));
    text.setAttribute('y', String(68 + index * 23));
    text.textContent = truncateText(line, 30);
    group.append(text);
  });

  target.append(group);
}

function renderCatalogMiniShape(
  group: SVGElement,
  shape: CatalogShape,
  x: number,
  y: number,
): void {
  const cellPixels = 16;
  const allCells = [
    ...shape.footprint,
    ...shape.reservedCells,
    ...shape.exits,
    ...shape.featureSockets,
  ];
  const minX = Math.min(...allCells.map((cell) => cell.x), 0);
  const minY = Math.min(...allCells.map((cell) => cell.y), 0);
  const maxX = Math.max(...allCells.map((cell) => cell.x), 1);
  const maxY = Math.max(...allCells.map((cell) => cell.y), 1);
  const columns = maxX - minX + 1;
  const rows = maxY - minY + 1;

  const background = createSvg('rect');
  background.setAttribute('class', 'catalog-mini-bg');
  background.setAttribute('x', String(x));
  background.setAttribute('y', String(y));
  background.setAttribute('width', String(columns * cellPixels));
  background.setAttribute('height', String(rows * cellPixels));
  group.append(background);

  for (const cell of shape.reservedCells) {
    appendCatalogCell(group, cell, minX, minY, x, y, cellPixels, 'reserved');
  }
  for (const cell of shape.footprint) {
    appendCatalogCell(group, cell, minX, minY, x, y, cellPixels, `footprint ${slugClass(shape.pieceKinds[0] ?? 'piece')}`);
  }
  for (const exit of shape.exits) {
    appendCatalogCell(group, exit, minX, minY, x, y, cellPixels, `exit ${slugClass(exit.direction)}`);
  }
  for (const socket of shape.featureSockets) {
    const center = normalizeCatalogPoint(socket, minX, minY, x, y, cellPixels);
    const marker = createSvg('circle');
    marker.setAttribute('class', `catalog-socket ${slugClass(socket.kind)}`);
    marker.setAttribute('cx', String(center.x));
    marker.setAttribute('cy', String(center.y));
    marker.setAttribute('r', '4');
    group.append(marker);

    const label = createSvg('text');
    label.setAttribute('class', 'catalog-socket-label');
    label.setAttribute('x', String(center.x));
    label.setAttribute('y', String(center.y + 3));
    label.textContent = contentSymbol(socket.kind);
    group.append(label);
  }
}

function appendCatalogCell(
  group: SVGElement,
  cell: GridCell,
  minX: number,
  minY: number,
  originX: number,
  originY: number,
  cellPixels: number,
  className: string,
): void {
  const normalized = normalizeCatalogPoint(cell, minX, minY, originX, originY, cellPixels);
  const rect = createSvg('rect');
  rect.setAttribute('class', `catalog-cell ${className}`);
  rect.setAttribute('x', String(normalized.x - cellPixels / 2));
  rect.setAttribute('y', String(normalized.y - cellPixels / 2));
  rect.setAttribute('width', String(cellPixels));
  rect.setAttribute('height', String(cellPixels));
  group.append(rect);
}

function normalizeCatalogPoint(
  cell: GridCell,
  minX: number,
  minY: number,
  originX: number,
  originY: number,
  cellPixels: number,
): { readonly x: number; readonly y: number } {
  return {
    x: originX + (cell.x - minX) * cellPixels + cellPixels / 2,
    y: originY + (cell.y - minY) * cellPixels + cellPixels / 2,
  };
}

function shortTransform(transform: string): string {
  switch (transform) {
    case 'identity':
      return 'I';
    case 'rotate90':
      return 'R90';
    case 'rotate180':
      return 'R180';
    case 'rotate270':
      return 'R270';
    case 'mirrorX':
      return 'MX';
    case 'mirrorY':
      return 'MY';
    default:
      return transform;
  }
}

function renderLayout(target: SVGSVGElement, layout: LayoutArtifact): void {
  target.replaceChildren();
  const roomById = new Map(layout.rooms.map((room) => [room.nodeId, room]));
  const maxX = Math.max(...layout.rooms.map((room) => room.x + room.width), 900);
  const maxY = Math.max(...layout.rooms.map((room) => room.y + room.height), 620);
  target.setAttribute('viewBox', `0 0 ${maxX + 120} ${maxY + 120}`);

  for (const link of layout.links) {
    const from = roomById.get(link.fromNode);
    const to = roomById.get(link.toNode);
    if (from === undefined || to === undefined) {
      continue;
    }
    const fromPoint = center(from);
    const toPoint = center(to);
    const path = createSvg('path');
    const controlX = (fromPoint.x + toPoint.x) / 2;
    path.setAttribute('class', `link ${link.traversal}`);
    path.setAttribute(
      'd',
      `M ${fromPoint.x} ${fromPoint.y} C ${controlX} ${fromPoint.y}, ${controlX} ${toPoint.y}, ${toPoint.x} ${toPoint.y}`,
    );
    target.append(path);

    const labelText = describeLink(link);
    if (labelText !== null) {
      const label = createSvg('text');
      label.setAttribute('class', 'edge-label');
      label.setAttribute('x', String((fromPoint.x + toPoint.x) / 2));
      label.setAttribute('y', String((fromPoint.y + toPoint.y) / 2 - 8));
      label.textContent = labelText;
      target.append(label);
    }
  }

  for (const room of layout.rooms) {
    const rect = createSvg('rect');
    rect.setAttribute('class', `room ${room.kind}`);
    rect.setAttribute('x', String(room.x));
    rect.setAttribute('y', String(room.y));
    rect.setAttribute('width', String(room.width));
    rect.setAttribute('height', String(room.height));
    rect.setAttribute('rx', '6');
    target.append(rect);

    const label = createSvg('text');
    label.setAttribute('class', 'room-label');
    label.setAttribute('x', String(room.x + room.width / 2));
    label.setAttribute('y', String(room.y + room.height / 2 + 4));
    label.textContent = room.label;
    target.append(label);
  }
}

function renderIntermediate(
  target: SVGSVGElement,
  breakdown: IntermediateBreakdown | null,
): void {
  target.replaceChildren();
  if (breakdown === null) {
    renderEmptySvg(target, 'No intermediate breakdown loaded.');
    return;
  }
  const regionsByRole = new Map<string, IntermediateRegion[]>();
  for (const region of breakdown.regions) {
    const regions = regionsByRole.get(region.role) ?? [];
    regions.push(region);
    regionsByRole.set(region.role, regions);
  }
  const roles = [...regionsByRole.keys()].sort();
  const columnWidth = 210;
  const rowHeight = 126;
  const cardWidth = 168;
  const cardHeight = 76;
  const positions = new Map<string, { readonly x: number; readonly y: number }>();
  roles.forEach((role, columnIndex) => {
    const regions = regionsByRole.get(role) ?? [];
    regions
      .slice()
      .sort((left, right) => left.id.localeCompare(right.id))
      .forEach((region, rowIndex) => {
        positions.set(region.id, {
          x: 70 + columnIndex * columnWidth,
          y: 96 + rowIndex * rowHeight,
        });
      });
  });
  const maxRows = Math.max(...[...regionsByRole.values()].map((regions) => regions.length), 1);
  const width = Math.max(900, 140 + roles.length * columnWidth);
  const height = Math.max(620, 160 + maxRows * rowHeight);
  target.setAttribute('viewBox', `0 0 ${width} ${height}`);

  roles.forEach((role, index) => {
    const heading = createSvg('text');
    heading.setAttribute('class', 'intermediate-role-label');
    heading.setAttribute('x', String(70 + index * columnWidth));
    heading.setAttribute('y', '48');
    heading.textContent = role.replaceAll('_', ' ');
    target.append(heading);
  });

  for (const connector of breakdown.connectors) {
    const from = positions.get(connector.fromRegion);
    const to = positions.get(connector.toRegion);
    if (from === undefined || to === undefined) {
      continue;
    }
    const fromPoint = {
      x: from.x + cardWidth,
      y: from.y + cardHeight / 2,
    };
    const toPoint = {
      x: to.x,
      y: to.y + cardHeight / 2,
    };
    const path = createSvg('path');
    const controlX = (fromPoint.x + toPoint.x) / 2;
    path.setAttribute('class', `intermediate-link ${connectorClass(connector)}`);
    path.setAttribute(
      'd',
      `M ${fromPoint.x} ${fromPoint.y} C ${controlX} ${fromPoint.y}, ${controlX} ${toPoint.y}, ${toPoint.x} ${toPoint.y}`,
    );
    target.append(path);

    const badge = createSvg('text');
    badge.setAttribute('class', 'intermediate-edge-label');
    badge.setAttribute('x', String((fromPoint.x + toPoint.x) / 2));
    badge.setAttribute('y', String((fromPoint.y + toPoint.y) / 2 - 8));
    badge.textContent = connectorBadge(connector);
    target.append(badge);
  }

  for (const [regionId, position] of positions) {
    const region = breakdown.regions.find((candidate) => candidate.id === regionId);
    if (region === undefined) {
      continue;
    }
    const group = createSvg('g');
    group.setAttribute('class', `intermediate-region ${slugClass(region.role)}`);
    const rect = createSvg('rect');
    rect.setAttribute('x', String(position.x));
    rect.setAttribute('y', String(position.y));
    rect.setAttribute('width', String(cardWidth));
    rect.setAttribute('height', String(cardHeight));
    rect.setAttribute('rx', '6');
    group.append(rect);

    const title = createSvg('text');
    title.setAttribute('class', 'intermediate-region-title');
    title.setAttribute('x', String(position.x + 12));
    title.setAttribute('y', String(position.y + 22));
    title.textContent = regionLabel(region);
    group.append(title);

    const detail = createSvg('text');
    detail.setAttribute('class', 'intermediate-region-detail');
    detail.setAttribute('x', String(position.x + 12));
    detail.setAttribute('y', String(position.y + 43));
    detail.textContent = `${region.geometryRole ?? 'role?'} / ${region.scaleBand ?? 'scale?'}`;
    group.append(detail);

    const anchor = createSvg('text');
    anchor.setAttribute('class', 'intermediate-region-detail');
    anchor.setAttribute('x', String(position.x + 12));
    anchor.setAttribute('y', String(position.y + 62));
    anchor.textContent = region.anchorNode ?? region.anchorQuality ?? 'derived';
    group.append(anchor);
    target.append(group);
  }
}

interface BuildCell {
  readonly kind: 'room' | 'corridor';
  readonly role: string;
}

interface BuildPlan {
  readonly cellSize: number;
  readonly cellPixels: number;
  readonly columns: number;
  readonly rows: number;
  readonly cells: Map<string, BuildCell>;
}

function renderBuildGrid(
  target: SVGSVGElement,
  geometry: Geometry2dArtifact | null,
  placement: PiecePlacement | null,
  placementValidation: ValidationReport | null,
): void {
  target.replaceChildren();
  if (placement !== null) {
    renderPiecePlacementGrid(target, placement, placementValidation);
    return;
  }
  if (geometry === null) {
    renderEmptySvg(target, 'No geometry or piece placement build artifact loaded.');
    return;
  }

  const plan = buildGridPlan(geometry);
  const margin = 24;
  const headerHeight = 54;
  const width = margin * 2 + plan.columns * plan.cellPixels;
  const height = margin * 2 + headerHeight + plan.rows * plan.cellPixels;
  target.setAttribute('viewBox', `0 0 ${width} ${height}`);

  const title = createSvg('text');
  title.setAttribute('class', 'intermediate-role-label');
  title.setAttribute('x', String(margin));
  title.setAttribute('y', '28');
  title.textContent = 'Geometry Build Grid';
  target.append(title);

  const stats = createSvg('text');
  stats.setAttribute('class', 'intermediate-region-detail');
  stats.setAttribute('x', String(margin));
  stats.setAttribute('y', '48');
  stats.textContent = `${plan.columns} x ${plan.rows} cells / ${geometry.rooms.length} rooms / ${geometry.corridors.length} corridors / ${geometry.contents.length} markers`;
  target.append(stats);

  const grid = createSvg('g');
  grid.setAttribute('transform', `translate(${margin} ${margin + headerHeight})`);
  target.append(grid);

  const background = createSvg('rect');
  background.setAttribute('x', '0');
  background.setAttribute('y', '0');
  background.setAttribute('width', String(plan.columns * plan.cellPixels));
  background.setAttribute('height', String(plan.rows * plan.cellPixels));
  background.setAttribute('fill', '#111820');
  grid.append(background);

  for (const [key, cell] of plan.cells) {
    const [column, row] = key.split(',').map(Number);
    const rect = createSvg('rect');
    rect.setAttribute('class', `build-cell ${cell.kind} ${slugClass(cell.role)}`);
    rect.setAttribute('x', String(column * plan.cellPixels));
    rect.setAttribute('y', String(row * plan.cellPixels));
    rect.setAttribute('width', String(plan.cellPixels));
    rect.setAttribute('height', String(plan.cellPixels));
    grid.append(rect);
  }

  for (let column = 0; column <= plan.columns; column += 1) {
    const line = createSvg('line');
    line.setAttribute('class', 'build-grid-line');
    line.setAttribute('x1', String(column * plan.cellPixels));
    line.setAttribute('y1', '0');
    line.setAttribute('x2', String(column * plan.cellPixels));
    line.setAttribute('y2', String(plan.rows * plan.cellPixels));
    grid.append(line);
  }
  for (let row = 0; row <= plan.rows; row += 1) {
    const line = createSvg('line');
    line.setAttribute('class', 'build-grid-line');
    line.setAttribute('x1', '0');
    line.setAttribute('y1', String(row * plan.cellPixels));
    line.setAttribute('x2', String(plan.columns * plan.cellPixels));
    line.setAttribute('y2', String(row * plan.cellPixels));
    grid.append(line);
  }

  for (const room of geometry.rooms) {
    const centerPoint = rectCenter(room.rect);
    const centerCell = pointToCell(centerPoint, plan.cellSize);
    const label = createSvg('text');
    label.setAttribute('class', 'build-label');
    label.setAttribute('x', String(centerCell.column * plan.cellPixels + 3));
    label.setAttribute('y', String(centerCell.row * plan.cellPixels + 12));
    label.textContent = buildRoomLabel(room);
    grid.append(label);
  }

  for (const [index, content] of geometry.contents.entries()) {
    const room = geometry.rooms.find((candidate) => candidate.id === content.roomId);
    if (room === undefined) {
      continue;
    }
    const centerPoint = rectCenter(room.rect);
    const centerCell = pointToCell(centerPoint, plan.cellSize);
    const markerX = centerCell.column * plan.cellPixels + 8 + (index % 3) * 12;
    const markerY = centerCell.row * plan.cellPixels + 25 + (index % 2) * 12;
    const marker = createSvg('circle');
    marker.setAttribute('class', `build-marker ${slugClass(content.kind)}`);
    marker.setAttribute('cx', String(markerX));
    marker.setAttribute('cy', String(markerY));
    marker.setAttribute('r', '7');
    grid.append(marker);

    const label = createSvg('text');
    label.setAttribute('class', 'build-marker-label');
    label.setAttribute('x', String(markerX));
    label.setAttribute('y', String(markerY + 3));
    label.textContent = contentSymbol(content.kind);
    grid.append(label);
  }
}

function renderPiecePlacementGrid(
  target: SVGSVGElement,
  placement: PiecePlacement,
  validation: ValidationReport | null,
): void {
  const plan = piecePlacementGridPlan(placement);
  const margin = 24;
  const headerHeight = 64;
  const width = margin * 2 + plan.columns * plan.cellPixels;
  const height = margin * 2 + headerHeight + plan.rows * plan.cellPixels;
  target.setAttribute('viewBox', `0 0 ${width} ${height}`);

  const title = createSvg('text');
  title.setAttribute('class', 'intermediate-role-label');
  title.setAttribute('x', String(margin));
  title.setAttribute('y', '28');
  title.textContent = 'Piece Placement Grid';
  target.append(title);

  const stats = createSvg('text');
  stats.setAttribute('class', 'intermediate-region-detail');
  stats.setAttribute('x', String(margin));
  stats.setAttribute('y', '50');
  const connectivity = placement.gridConnectivity.replace('_', '-');
  const catalogSearch = placement.catalogSearch === undefined
    ? ''
    : ` / search ${placement.catalogSearch.decisions} decisions, ${placement.catalogSearch.backtracks} backtracks`;
  stats.textContent = `${corridorRealizationFor(placement)} / ${placement.instances.length} pieces / ${placement.occupiedCells.length} occupied / ${placement.connectionCells.length} connection${catalogSearch} / clearance ${placement.placementPolicy.minimumClearanceCells} / wall ${placement.placementPolicy.wallThicknessCells} / ${connectivity} / ${validation?.ok === false ? `${validation.fatalCount} fatal` : 'valid'}`;
  target.append(stats);

  const grid = createSvg('g');
  grid.setAttribute('transform', `translate(${margin} ${margin + headerHeight})`);
  target.append(grid);

  const background = createSvg('rect');
  background.setAttribute('x', '0');
  background.setAttribute('y', '0');
  background.setAttribute('width', String(plan.columns * plan.cellPixels));
  background.setAttribute('height', String(plan.rows * plan.cellPixels));
  background.setAttribute('fill', '#111820');
  grid.append(background);

  const centers = new Map<string, { readonly x: number; readonly y: number }>();
  for (const instance of placement.instances) {
    const cells = instance.occupiedCells.map((cell) => normalizeCell(cell, plan));
    if (cells.length === 0) {
      continue;
    }
    const minColumn = Math.min(...cells.map((cell) => cell.column));
    const maxColumn = Math.max(...cells.map((cell) => cell.column));
    const minRow = Math.min(...cells.map((cell) => cell.row));
    const maxRow = Math.max(...cells.map((cell) => cell.row));
    centers.set(instance.instanceId, {
      x: ((minColumn + maxColumn + 1) / 2) * plan.cellPixels,
      y: ((minRow + maxRow + 1) / 2) * plan.cellPixels,
    });
  }

  for (const glued of placement.gluedExits) {
    const from = centers.get(glued.fromInstance);
    const to = centers.get(glued.toInstance);
    if (from === undefined || to === undefined) {
      continue;
    }
    const line = createSvg('line');
    line.setAttribute('class', `build-glue-link ${glueClass(glued)}`);
    line.setAttribute('x1', String(from.x));
    line.setAttribute('y1', String(from.y));
    line.setAttribute('x2', String(to.x));
    line.setAttribute('y2', String(to.y));
    grid.append(line);
  }

  for (const cell of placement.reservedCells) {
    const normalized = normalizeCell(cell, plan);
    const rect = createSvg('rect');
    rect.setAttribute('class', 'build-cell reserved');
    rect.setAttribute('x', String(normalized.column * plan.cellPixels));
    rect.setAttribute('y', String(normalized.row * plan.cellPixels));
    rect.setAttribute('width', String(plan.cellPixels));
    rect.setAttribute('height', String(plan.cellPixels));
    grid.append(rect);
  }

  for (const cell of placement.connectionCells) {
    const normalized = normalizeCell(cell, plan);
    const rect = createSvg('rect');
    rect.setAttribute('class', 'build-cell connection');
    rect.setAttribute('x', String(normalized.column * plan.cellPixels));
    rect.setAttribute('y', String(normalized.row * plan.cellPixels));
    rect.setAttribute('width', String(plan.cellPixels));
    rect.setAttribute('height', String(plan.cellPixels));
    const titleElement = createSvg('title');
    titleElement.textContent = cell.instanceId;
    rect.append(titleElement);
    grid.append(rect);
  }

  const instancesById = new Map(placement.instances.map((instance) => [instance.instanceId, instance]));
  for (const cell of placement.occupiedCells) {
    const normalized = normalizeCell(cell, plan);
    const instance = instancesById.get(cell.instanceId);
    const rect = createSvg('rect');
    rect.setAttribute(
      'class',
      `build-cell piece ${slugClass(instance?.requirementKind ?? 'piece')} ${slugClass(instance?.role ?? 'piece')}`,
    );
    rect.setAttribute('x', String(normalized.column * plan.cellPixels));
    rect.setAttribute('y', String(normalized.row * plan.cellPixels));
    rect.setAttribute('width', String(plan.cellPixels));
    rect.setAttribute('height', String(plan.cellPixels));
    const titleElement = createSvg('title');
    titleElement.textContent = instance === undefined
      ? cell.instanceId
      : `${instance.pieceId} / ${instance.shapeId} / ${instance.transform}`;
    rect.append(titleElement);
    grid.append(rect);
  }

  for (let column = 0; column <= plan.columns; column += 1) {
    const line = createSvg('line');
    line.setAttribute('class', 'build-grid-line');
    line.setAttribute('x1', String(column * plan.cellPixels));
    line.setAttribute('y1', '0');
    line.setAttribute('x2', String(column * plan.cellPixels));
    line.setAttribute('y2', String(plan.rows * plan.cellPixels));
    grid.append(line);
  }
  for (let row = 0; row <= plan.rows; row += 1) {
    const line = createSvg('line');
    line.setAttribute('class', 'build-grid-line');
    line.setAttribute('x1', '0');
    line.setAttribute('y1', String(row * plan.cellPixels));
    line.setAttribute('x2', String(plan.columns * plan.cellPixels));
    line.setAttribute('y2', String(row * plan.cellPixels));
    grid.append(line);
  }

  for (const instance of placement.instances) {
    const center = centers.get(instance.instanceId);
    if (center === undefined) {
      continue;
    }
    const label = createSvg('text');
    label.setAttribute('class', 'build-label piece-label');
    label.setAttribute('x', String(center.x - 8));
    label.setAttribute('y', String(center.y + 4));
    label.textContent = pieceLabel(instance);
    grid.append(label);

    instance.featurePlacements.forEach((feature, index) => {
      const marker = createSvg('circle');
      marker.setAttribute('class', `build-marker ${slugClass(feature.kind)}`);
      marker.setAttribute('cx', String(center.x + 10 + (index % 2) * 10));
      marker.setAttribute('cy', String(center.y - 10 + Math.floor(index / 2) * 10));
      marker.setAttribute('r', '6');
      grid.append(marker);

      const markerLabel = createSvg('text');
      markerLabel.setAttribute('class', 'build-marker-label');
      markerLabel.setAttribute('x', marker.getAttribute('cx') ?? String(center.x));
      markerLabel.setAttribute('y', String(Number(marker.getAttribute('cy') ?? center.y) + 3));
      markerLabel.textContent = contentSymbol(feature.kind);
      grid.append(markerLabel);
    });
  }

  for (const dangling of placement.danglingExits) {
    const center = centers.get(dangling.instanceId);
    if (center === undefined) {
      continue;
    }
    const marker = createSvg('rect');
    marker.setAttribute('class', 'build-dangling');
    marker.setAttribute('x', String(center.x - 6));
    marker.setAttribute('y', String(center.y - 6));
    marker.setAttribute('width', '12');
    marker.setAttribute('height', '12');
    marker.setAttribute('transform', `rotate(45 ${center.x} ${center.y})`);
    grid.append(marker);
  }
}

interface PiecePlacementGridPlan {
  readonly cellPixels: number;
  readonly minX: number;
  readonly minY: number;
  readonly columns: number;
  readonly rows: number;
}

function piecePlacementGridPlan(placement: PiecePlacement): PiecePlacementGridPlan {
  const allCells = [
    ...placement.occupiedCells,
    ...placement.connectionCells,
    ...placement.reservedCells,
  ];
  const minX = Math.min(...allCells.map((cell) => cell.x), 0);
  const minY = Math.min(...allCells.map((cell) => cell.y), 0);
  const maxX = Math.max(...allCells.map((cell) => cell.x), 1);
  const maxY = Math.max(...allCells.map((cell) => cell.y), 1);
  return {
    cellPixels: 14,
    minX,
    minY,
    columns: maxX - minX + 3,
    rows: maxY - minY + 3,
  };
}

function normalizeCell(
  cell: GridCell | PlacementCellRef,
  plan: PiecePlacementGridPlan,
): { readonly column: number; readonly row: number } {
  return {
    column: cell.x - plan.minX + 1,
    row: cell.y - plan.minY + 1,
  };
}

function pieceLabel(instance: PieceInstance): string {
  switch (instance.requirementKind) {
    case 'connector':
      return 'CON';
    case 'corridor':
      return 'COR';
    case 'threshold':
      return 'GATE';
    case 'hazard':
      return 'HAZ';
    case 'reward':
      return 'REW';
    case 'resource':
      return 'RES';
    case 'secret':
      return 'SEC';
    case 'shortcut':
      return 'SCT';
    default:
      return instance.requirementKind.slice(0, 4).toUpperCase();
  }
}

function glueClass(glued: GluedExit): string {
  if (glued.tags.some((tag) => tag.includes('hidden'))) {
    return 'hidden';
  }
  if (glued.tags.some((tag) => tag.includes('locked'))) {
    return 'locked';
  }
  if (glued.tags.some((tag) => tag.includes('shortcut'))) {
    return 'shortcut';
  }
  if (glued.tags.some((tag) => tag.includes('pressure'))) {
    return 'pressure';
  }
  return 'standard';
}

function buildGridPlan(geometry: Geometry2dArtifact): BuildPlan {
  const cellSize = 24;
  const cellPixels = 16;
  const columns = Math.ceil(geometry.bounds.width / cellSize) + 1;
  const rows = Math.ceil(geometry.bounds.height / cellSize) + 1;
  const cells = new Map<string, BuildCell>();

  for (const room of geometry.rooms) {
    const startColumn = Math.floor(room.rect.x / cellSize);
    const endColumn = Math.ceil((room.rect.x + room.rect.width) / cellSize);
    const startRow = Math.floor(room.rect.y / cellSize);
    const endRow = Math.ceil((room.rect.y + room.rect.height) / cellSize);
    for (let row = startRow; row < endRow; row += 1) {
      for (let column = startColumn; column < endColumn; column += 1) {
        setBuildCell(cells, column, row, { kind: 'room', role: room.role });
      }
    }
  }

  for (const corridor of geometry.corridors) {
    for (let index = 0; index < corridor.points.length - 1; index += 1) {
      digCorridorSegment(cells, corridor.points[index], corridor.points[index + 1], cellSize);
    }
  }

  return { cellSize, cellPixels, columns, rows, cells };
}

function digCorridorSegment(
  cells: Map<string, BuildCell>,
  start: GeometryPoint,
  end: GeometryPoint,
  cellSize: number,
): void {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const steps = Math.max(1, Math.ceil(Math.max(Math.abs(dx), Math.abs(dy)) / cellSize));
  for (let step = 0; step <= steps; step += 1) {
    const ratio = step / steps;
    const point = {
      x: start.x + dx * ratio,
      y: start.y + dy * ratio,
    };
    const cell = pointToCell(point, cellSize);
    const key = cellKey(cell.column, cell.row);
    if (!cells.has(key)) {
      cells.set(key, { kind: 'corridor', role: 'corridor' });
    }
  }
}

function setBuildCell(
  cells: Map<string, BuildCell>,
  column: number,
  row: number,
  cell: BuildCell,
): void {
  cells.set(cellKey(column, row), cell);
}

function pointToCell(
  point: GeometryPoint,
  cellSize: number,
): { readonly column: number; readonly row: number } {
  return {
    column: Math.floor(point.x / cellSize),
    row: Math.floor(point.y / cellSize),
  };
}

function cellKey(column: number, row: number): string {
  return `${column},${row}`;
}

function rectCenter(rect: GeometryRect): GeometryPoint {
  return {
    x: rect.x + rect.width / 2,
    y: rect.y + rect.height / 2,
  };
}

function buildRoomLabel(room: GeometryRoom): string {
  const source = room.sourceNodes[0] ?? room.id;
  if (room.role === 'start' || room.role === 'goal') {
    return room.role.toUpperCase();
  }
  return source.replace('gate.', 'G.').replace('hazard.', 'H.').replace('treasure.', 'T.');
}

function contentSymbol(kind: string): string {
  switch (kind) {
    case 'key_pickup':
      return 'K';
    case 'locked_gate':
    case 'gate_line':
      return 'L';
    case 'boss_threshold':
    case 'boss_space':
      return 'B';
    case 'hazard':
    case 'hazard_zone':
      return '!';
    case 'reward_cache':
      return '$';
    case 'secret_route_marker':
    case 'secret_marker':
      return '?';
    case 'shortcut_marker':
      return 'S';
    case 'resource_clue':
      return 'R';
    case 'start_marker':
      return 'A';
    case 'goal_marker':
      return 'Z';
    default:
      return '*';
  }
}

function renderEmptySvg(target: SVGSVGElement, message: string): void {
  target.replaceChildren();
  target.setAttribute('viewBox', '0 0 900 620');
  const text = createSvg('text');
  text.setAttribute('class', 'empty-svg-label');
  text.setAttribute('x', '450');
  text.setAttribute('y', '310');
  text.textContent = message;
  target.append(text);
}

function center(room: LayoutRoom): { readonly x: number; readonly y: number } {
  return {
    x: room.x + room.width / 2,
    y: room.y + room.height / 2,
  };
}

function describeLink(link: LayoutLink): string | null {
  if (link.requiredItem !== null) {
    return `requires ${link.requiredItem.replace('item.', '')}`;
  }
  if (link.traversal === 'hidden') {
    return 'hidden';
  }
  if (link.traversal === 'one_way_return') {
    return 'one-way';
  }
  return null;
}

function connectorClass(connector: IntermediateConnector): string {
  const values = [...connector.intents, ...(connector.affordances ?? [])];
  if (values.some((value) => value.includes('hidden'))) {
    return 'hidden';
  }
  if (values.some((value) => value.includes('locked') || value.includes('gated'))) {
    return 'locked';
  }
  if (values.some((value) => value.includes('shortcut'))) {
    return 'shortcut';
  }
  if (values.some((value) => value.includes('pressure'))) {
    return 'pressure';
  }
  if (values.some((value) => value.includes('rejoin') || value.includes('return'))) {
    return 'rejoin';
  }
  return 'standard';
}

function connectorBadge(connector: IntermediateConnector): string {
  const affordances = connector.affordances ?? [];
  const labels = affordances.length > 0 ? affordances : connector.intents;
  const base = labels.slice(0, 2).map((value) => value.replaceAll('_', ' ')).join(' / ');
  const constraintCount = connector.constraintRefs?.length ?? 0;
  if (constraintCount > 0) {
    return `${base} (${constraintCount})`;
  }
  return base || connector.edgeId;
}

function regionLabel(region: IntermediateRegion): string {
  const node = region.nodeIds?.[0] ?? region.id.replace('region.', '');
  return node.replaceAll('_', '.');
}

function shortCandidate(candidateId: string): string {
  return candidateId.replace('candidate.first_slice.', '').replace('candidate.first-slice.', '');
}

function truncateText(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }
  return `${value.slice(0, Math.max(0, maxLength - 3))}...`;
}

function slugClass(value: string): string {
  return value.replaceAll('_', '-').replaceAll('.', '-');
}

function createSvg(name: string): SVGElement {
  return document.createElementNS('http://www.w3.org/2000/svg', name);
}
