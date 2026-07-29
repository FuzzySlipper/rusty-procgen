//! Explicit downstream mapping from Procgen artifacts into Rusty Engine owners.
//!
//! Dungeon selection and provenance remain local. Asset, prefab, scene, entity,
//! and atomic content validation are delegated to the pinned public Engine
//! crates instead of being copied into this adapter.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use asset_catalog::{
    decode_catalog, decode_lock, encode_catalog, encode_lock, generate_lock, validate_catalog,
    validate_lock, AssetCatalog, AssetLock, CatalogEntry,
};
use authored_scene::{
    decode_scene, encode_scene, FlatSceneDocument, NodeMetadata, SceneAdmissionPlan,
    SceneEntityInstance, SceneEntityReference, SceneMetadata, SceneNodeKind, SceneNodeRecord,
    SceneResolutionContext, SceneTransform, CURRENT_SCENE_SCHEMA_VERSION,
};
use content_store::{
    admit_source_batch, decode_prefab_registry, encode_manifest, encode_prefab_registry,
    validate_prefab_registry, ArtifactRole, ContentArtifact, ContentBody, ContentManifest,
    ContentSourceBatch, ContentStoreIdentity, ContentWrite, ContentWriteCandidate,
    ContentWriteSetDraft, PrefabDefinition, PrefabPart, PrefabPartRoleBinding, PrefabPartSource,
    PrefabRegistry, PrefabRegistryValidationContext, PrefabTransform, ValidatedPrefabRegistry,
    PREFAB_DEFINITION_SCHEMA_VERSION, PREFAB_REGISTRY_SCHEMA_VERSION,
};
use core_assets::{AssetHash, AssetId};
use core_ids::{PrefabId, PrefabPartId, SceneId, SceneNodeId};
use core_math::Vec3;
use entity_state::{EntityState, Quat};
use rusty_procgen_preflight::{PiecePlacement, PieceShapeMatchReport, ShapeCatalog};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_SELECTED_INSTANCES: usize = 4_096;
pub const ENGINE_PUBLIC_REPOSITORY: &str = "https://github.com/FuzzySlipper/rusty-engine";

const CATALOG_KIND: &str = "rusty_procgen.shape_catalog.v1";
const MATCH_KIND: &str = "rusty_procgen.piece_shape_match.v1";
const PLACEMENT_KIND: &str = "rusty_procgen.piece_placement.v1";
const MAPPING_KIND: &str = "rusty_procgen.engine_publish_mapping.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PublishDiagnosticCode {
    InvalidProvenance,
    MissingShape,
    MissingPrefabMapping,
    MissingStableRole,
    IncompatibleSourceAsset,
    DuplicateInstanceIdentity,
    InvalidTransform,
    StalePin,
    QuotaExceeded,
    LateValidation,
    Io,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishError {
    pub code: PublishDiagnosticCode,
    pub message: String,
}

impl PublishError {
    fn new(code: PublishDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for PublishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for PublishError {}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PublishConfiguration {
    pub kind: String,
    pub candidate_ref: String,
    pub selected_instance_ids: Vec<String>,
    pub mappings: Vec<PrefabMapping>,
    pub instance_identities: Vec<InstanceIdentity>,
    pub source_assets: Vec<SourceAsset>,
    pub project: ProjectConfiguration,
    pub scene: SceneConfiguration,
    pub prefab_registry_artifact: String,
    pub asset_catalog_artifact: String,
    pub asset_lock_artifact: String,
    pub provenance_artifact: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PrefabMapping {
    pub shape_id: String,
    pub prefab_id: u64,
    pub part_id: u64,
    pub part_namespace: String,
    pub stable_role: String,
    pub source: MappedPartSource,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum MappedPartSource {
    Scene { asset: String },
    EntityDefinition { stable_id: String },
    VoxelObject { asset: String },
}

impl MappedPartSource {
    fn asset(&self) -> Option<&str> {
        match self {
            Self::Scene { asset } | Self::VoxelObject { asset } => Some(asset),
            Self::EntityDefinition { .. } => None,
        }
    }

    fn to_engine(&self) -> PrefabPartSource {
        match self {
            Self::Scene { asset } => PrefabPartSource::Scene {
                asset: asset.clone(),
            },
            Self::EntityDefinition { stable_id } => PrefabPartSource::EntityDefinition {
                stable_id: stable_id.clone(),
            },
            Self::VoxelObject { asset } => PrefabPartSource::VoxelObject {
                asset: asset.clone(),
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InstanceIdentity {
    pub procgen_instance_id: String,
    pub prefab_instance_id: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourceAsset {
    pub asset_id: String,
    pub artifact: String,
    pub source: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectConfiguration {
    pub id: u64,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SceneConfiguration {
    pub id: u64,
    pub artifact: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PublicationProvenance {
    pub candidate_ref: String,
    pub catalog_id: String,
    pub catalog_ref: String,
    pub plan_id: String,
    pub plan_ref: String,
    pub match_id: String,
    pub match_ref: String,
    pub placement_id: String,
    pub instances: Vec<PublishedInstanceProvenance>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PublishedInstanceProvenance {
    pub procgen_instance_id: String,
    pub prefab_instance_id: u64,
    pub piece_id: String,
    pub shape_id: String,
    pub prefab_id: u64,
    pub match_score: i32,
    pub source_requirement_ref: String,
}

#[derive(Debug)]
pub struct CompiledPublication {
    pub asset_catalog: AssetCatalog,
    pub asset_lock: AssetLock,
    pub prefab_registry: PrefabRegistry,
    pub scene: FlatSceneDocument,
    pub manifest: ContentManifest,
    pub candidate: ContentWriteCandidate,
    pub provenance: PublicationProvenance,
    pub admitted_state: EntityState,
    bodies: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PublicationEvidence {
    pub kind: &'static str,
    pub schema_version: u32,
    pub engine_source: EngineSourceEvidence,
    pub input: InputEvidence,
    pub output: OutputEvidence,
    pub readback: ReadbackEvidence,
    pub fail_closed_cases: Vec<PublishDiagnosticCode>,
    pub non_claims: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EngineSourceEvidence {
    pub public_repository: String,
    pub commit: String,
    pub crates: Vec<&'static str>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EngineSourceManifest {
    schema_version: u32,
    public_repository: String,
    commit: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputEvidence {
    pub candidate_ref: String,
    pub catalog_id: String,
    pub match_id: String,
    pub placement_id: String,
    pub selected_instances: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OutputEvidence {
    pub prefab_definitions: usize,
    pub scene_nodes: usize,
    pub asset_entries: usize,
    pub manifest_artifacts: usize,
    pub content_set_hash: String,
    pub candidate_hash: String,
    pub provenance: PublicationProvenance,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReadbackEvidence {
    pub canonical_catalog: bool,
    pub canonical_lock: bool,
    pub canonical_prefabs: bool,
    pub canonical_scene: bool,
    pub admitted_entities: usize,
    pub resolved_prefab_instances: usize,
    pub content_load_order_verified: bool,
}

pub struct CompileInput<'a> {
    pub catalog: &'a ShapeCatalog,
    pub shape_match: &'a PieceShapeMatchReport,
    pub placement: &'a PiecePlacement,
    pub configuration: &'a PublishConfiguration,
    pub source_bodies: &'a BTreeMap<String, Vec<u8>>,
}

pub struct CheckedInputs {
    pub catalog: ShapeCatalog,
    pub shape_match: PieceShapeMatchReport,
    pub placement: PiecePlacement,
    pub configuration: PublishConfiguration,
    pub source_bodies: BTreeMap<String, Vec<u8>>,
}

struct ValidatedSource {
    id: AssetId,
    hash: AssetHash,
    bytes: Vec<u8>,
}

type SourceInventory = BTreeMap<String, ValidatedSource>;

pub fn compile_publication(input: CompileInput<'_>) -> Result<CompiledPublication, PublishError> {
    let CompileInput {
        catalog,
        shape_match,
        placement,
        configuration,
        source_bodies,
    } = input;
    validate_provenance(catalog, shape_match, placement, configuration)?;

    if configuration.selected_instance_ids.len() > MAX_SELECTED_INSTANCES {
        return Err(PublishError::new(
            PublishDiagnosticCode::QuotaExceeded,
            format!(
                "selected {} instances; maximum is {MAX_SELECTED_INSTANCES}",
                configuration.selected_instance_ids.len()
            ),
        ));
    }

    let shapes = unique_by(
        &catalog.shapes,
        |shape| shape.shape_id.as_str(),
        PublishDiagnosticCode::MissingShape,
        "shape catalog",
    )?;
    let matches = unique_by(
        &shape_match.matches,
        |shape_match| shape_match.piece_id.as_str(),
        PublishDiagnosticCode::InvalidProvenance,
        "shape match",
    )?;
    let placements = unique_by(
        &placement.instances,
        |instance| instance.instance_id.as_str(),
        PublishDiagnosticCode::DuplicateInstanceIdentity,
        "piece placement",
    )?;
    let mappings = unique_by(
        &configuration.mappings,
        |mapping| mapping.shape_id.as_str(),
        PublishDiagnosticCode::MissingPrefabMapping,
        "prefab mapping",
    )?;
    let identities = unique_by(
        &configuration.instance_identities,
        |identity| identity.procgen_instance_id.as_str(),
        PublishDiagnosticCode::DuplicateInstanceIdentity,
        "instance identity mapping",
    )?;
    unique_positive_ids(
        configuration
            .instance_identities
            .iter()
            .map(|identity| identity.prefab_instance_id),
        "prefab instance identity",
    )?;
    unique_positive_ids(
        configuration
            .mappings
            .iter()
            .map(|mapping| mapping.prefab_id),
        "prefab identity",
    )?;

    let mut seen_selected = BTreeSet::new();
    let mut selected = Vec::with_capacity(configuration.selected_instance_ids.len());
    for selected_id in &configuration.selected_instance_ids {
        if !seen_selected.insert(selected_id.as_str()) {
            return Err(PublishError::new(
                PublishDiagnosticCode::DuplicateInstanceIdentity,
                format!("selected instance identity {selected_id} is duplicated"),
            ));
        }
        let instance = placements.get(selected_id.as_str()).ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::DuplicateInstanceIdentity,
                format!("selected instance {selected_id} is missing from the placement"),
            )
        })?;
        let shape = shapes.get(instance.shape_id.as_str()).ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::MissingShape,
                format!("placement references missing shape {}", instance.shape_id),
            )
        })?;
        let matched = matches.get(instance.piece_id.as_str()).ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::InvalidProvenance,
                format!(
                    "placement instance {} has no shape match",
                    instance.instance_id
                ),
            )
        })?;
        if matched.shape_id != instance.shape_id
            || matched.transform != instance.transform
            || matched.source_requirement_ref != instance.source_requirement_ref
        {
            return Err(PublishError::new(
                PublishDiagnosticCode::InvalidProvenance,
                format!(
                    "placement instance {} diverges from its shape-match provenance",
                    instance.instance_id
                ),
            ));
        }
        let mapping = mappings.get(instance.shape_id.as_str()).ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::MissingPrefabMapping,
                format!("shape {} has no stable prefab mapping", instance.shape_id),
            )
        })?;
        validate_stable_role(&mapping.stable_role, &instance.shape_id)?;
        let identity = identities
            .get(instance.instance_id.as_str())
            .ok_or_else(|| {
                PublishError::new(
                    PublishDiagnosticCode::DuplicateInstanceIdentity,
                    format!(
                        "instance {} has no stable authored identity",
                        instance.instance_id
                    ),
                )
            })?;
        selected.push((*instance, *shape, *matched, *mapping, *identity));
    }

    let source_inventory = validate_source_inventory(configuration, source_bodies, &selected)?;
    let asset_catalog = build_asset_catalog(configuration, &source_inventory)?;
    let catalog_report = validate_catalog(&asset_catalog);
    if !catalog_report.is_ok() {
        return Err(PublishError::new(
            PublishDiagnosticCode::LateValidation,
            diagnostics(catalog_report.diagnostics().into_iter().map(|diagnostic| {
                format!(
                    "{}@{}: {}",
                    diagnostic.code, diagnostic.path, diagnostic.message
                )
            })),
        ));
    }
    let asset_lock = generate_lock(&asset_catalog);
    let lock_report = validate_lock(&asset_lock, &asset_catalog);
    if !lock_report.is_clean() {
        return Err(PublishError::new(
            PublishDiagnosticCode::LateValidation,
            "Rusty Engine rejected the generated asset lock",
        ));
    }

    let prefab_registry = build_prefab_registry(&selected);
    let prefab_context = PrefabRegistryValidationContext::from_asset_ids(
        asset_catalog.iter().map(|entry| entry.id.clone()),
        std::iter::empty(),
    );
    let prefab_report = validate_prefab_registry(&prefab_registry, &prefab_context);
    if !prefab_report.is_valid() {
        return Err(PublishError::new(
            PublishDiagnosticCode::LateValidation,
            diagnostics(prefab_report.diagnostics.iter().map(|diagnostic| {
                format!(
                    "{}@{}: {}",
                    diagnostic.code.as_str(),
                    diagnostic.path,
                    diagnostic.message
                )
            })),
        ));
    }

    let (scene, provenance) =
        build_scene_and_provenance(catalog, shape_match, placement, configuration, &selected)?;
    let resolution = SceneResolutionContext {
        prefab_ids: prefab_registry
            .definitions
            .iter()
            .map(|definition| definition.id)
            .collect(),
        ..SceneResolutionContext::default()
    };
    let admission = SceneAdmissionPlan::prepare(&scene, &resolution).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("Rusty Engine scene admission rejected the publication: {error}"),
        )
    })?;
    let mut admitted_state = EntityState::default();
    admission.apply(&mut admitted_state, 0).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("Rusty Engine entity admission rejected the publication: {error}"),
        )
    })?;

    let (manifest, bodies) = encode_owned_bodies(
        configuration,
        &asset_catalog,
        &asset_lock,
        &prefab_registry,
        &scene,
        &provenance,
        &source_inventory,
    )?;
    let prior_manifest = ContentManifest::new(Vec::new());
    let writes = bodies
        .iter()
        .map(|(path, bytes)| ContentWrite::new(path, bytes.clone()))
        .collect();
    let candidate = ContentWriteCandidate::build(
        0,
        &prior_manifest,
        ContentWriteSetDraft {
            next_manifest: manifest.clone(),
            writes,
            moves: Vec::new(),
            deletes: Vec::new(),
        },
    )
    .map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("Rusty Engine content publication rejected the write set: {error}"),
        )
    })?;
    let prior_identity =
        ContentStoreIdentity::from_manifest(0, &prior_manifest).map_err(|error| {
            PublishError::new(
                PublishDiagnosticCode::LateValidation,
                format!("could not identify empty content store: {error}"),
            )
        })?;
    let authorized = candidate
        .clone()
        .authorize(&prior_identity)
        .map_err(|error| {
            PublishError::new(
                PublishDiagnosticCode::LateValidation,
                format!("content publication authorization failed: {error}"),
            )
        })?;
    let next_identity = ContentStoreIdentity::from_manifest(1, &manifest).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("could not identify publication content: {error}"),
        )
    })?;
    authorized.confirm(&next_identity).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("content publication confirmation failed: {error}"),
        )
    })?;

    strict_readback(&manifest, &bodies, &prefab_context, &resolution)?;

    Ok(CompiledPublication {
        asset_catalog,
        asset_lock,
        prefab_registry,
        scene,
        manifest,
        candidate,
        provenance,
        admitted_state,
        bodies,
    })
}

impl CompiledPublication {
    pub fn evidence(&self) -> Result<PublicationEvidence, PublishError> {
        let engine_source = engine_source_manifest()?;
        let manifest_identity =
            ContentStoreIdentity::from_manifest(1, &self.manifest).map_err(|error| {
                PublishError::new(
                    PublishDiagnosticCode::LateValidation,
                    format!("could not calculate publication identity: {error}"),
                )
            })?;
        let load_plan = content_store::ContentLoadPlan::build(&self.manifest).map_err(|error| {
            PublishError::new(
                PublishDiagnosticCode::LateValidation,
                format!("could not build content load plan: {error}"),
            )
        })?;
        Ok(PublicationEvidence {
            kind: "rusty_procgen.evidence.engine_authored_publication.v1",
            schema_version: 1,
            engine_source: EngineSourceEvidence {
                public_repository: engine_source.public_repository,
                commit: engine_source.commit,
                crates: vec![
                    "asset-catalog",
                    "authored-scene",
                    "content-store",
                    "entity-state",
                ],
            },
            input: InputEvidence {
                candidate_ref: self.provenance.candidate_ref.clone(),
                catalog_id: self.provenance.catalog_id.clone(),
                match_id: self.provenance.match_id.clone(),
                placement_id: self.provenance.placement_id.clone(),
                selected_instances: self.provenance.instances.len(),
            },
            output: OutputEvidence {
                prefab_definitions: self.prefab_registry.definitions.len(),
                scene_nodes: self.scene.nodes.len(),
                asset_entries: self.asset_catalog.entries.len(),
                manifest_artifacts: self.manifest.artifacts.len(),
                content_set_hash: manifest_identity.content_set_hash.to_string(),
                candidate_hash: self.candidate.candidate_hash().to_string(),
                provenance: self.provenance.clone(),
            },
            readback: ReadbackEvidence {
                canonical_catalog: true,
                canonical_lock: true,
                canonical_prefabs: true,
                canonical_scene: true,
                admitted_entities: self.admitted_state.snapshot().entities.len(),
                resolved_prefab_instances: self.scene.nodes.len(),
                content_load_order_verified: load_plan.verify_order(),
            },
            fail_closed_cases: vec![
                PublishDiagnosticCode::MissingPrefabMapping,
                PublishDiagnosticCode::MissingStableRole,
                PublishDiagnosticCode::IncompatibleSourceAsset,
                PublishDiagnosticCode::DuplicateInstanceIdentity,
                PublishDiagnosticCode::InvalidTransform,
                PublishDiagnosticCode::StalePin,
                PublishDiagnosticCode::QuotaExceeded,
                PublishDiagnosticCode::LateValidation,
            ],
            non_claims: vec![
                "not_voxel_realization_proof",
                "not_renderer_proof",
                "not_navigation_proof",
                "not_collision_proof",
            ],
        })
    }

    pub fn body(&self, path: &str) -> Option<&[u8]> {
        self.bodies.get(path).map(Vec::as_slice)
    }
}

pub fn write_evidence_atomic(
    path: &Path,
    evidence: &PublicationEvidence,
) -> Result<(), PublishError> {
    let parent = path.parent().ok_or_else(|| {
        PublishError::new(
            PublishDiagnosticCode::Io,
            format!("evidence path {} has no parent", path.display()),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::Io,
            format!("could not create {}: {error}", parent.display()),
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::Io,
                format!("evidence path {} has no UTF-8 file name", path.display()),
            )
        })?;
    let temporary = parent.join(format!(".{file_name}.next"));
    let bytes = serde_json::to_vec_pretty(evidence).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::Io,
            format!("could not encode publication evidence: {error}"),
        )
    })?;
    let mut terminated = bytes;
    terminated.push(b'\n');
    fs::write(&temporary, terminated).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::Io,
            format!("could not stage {}: {error}", temporary.display()),
        )
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        PublishError::new(
            PublishDiagnosticCode::Io,
            format!("could not publish {}: {error}", path.display()),
        )
    })
}

pub fn load_checked_inputs(repo_root: &Path) -> Result<CheckedInputs, PublishError> {
    let catalog = read_json(repo_root.join("fixtures/shape-catalogs/2d-basic.json"))?;
    let shape_match = read_json(
        repo_root.join("artifacts/samples/batch-v2/candidate-006/piece-shape-match.json"),
    )?;
    let placement =
        read_json(repo_root.join("artifacts/samples/batch-v2/candidate-006/piece-placement.json"))?;
    let configuration: PublishConfiguration =
        read_json(repo_root.join("fixtures/prefab-mappings/first-slice.json"))?;
    let mut source_bodies = BTreeMap::new();
    for source in &configuration.source_assets {
        let bytes = fs::read(repo_root.join(&source.source)).map_err(|error| {
            PublishError::new(
                PublishDiagnosticCode::Io,
                format!("could not read {}: {error}", source.source),
            )
        })?;
        source_bodies.insert(source.source.clone(), bytes);
    }
    Ok(CheckedInputs {
        catalog,
        shape_match,
        placement,
        configuration,
        source_bodies,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, PublishError> {
    let bytes = fs::read(&path).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::Io,
            format!("could not read {}: {error}", path.display()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::InvalidProvenance,
            format!("could not decode {}: {error}", path.display()),
        )
    })
}

fn validate_provenance(
    catalog: &ShapeCatalog,
    shape_match: &PieceShapeMatchReport,
    placement: &PiecePlacement,
    configuration: &PublishConfiguration,
) -> Result<(), PublishError> {
    let kinds_match = catalog.kind == CATALOG_KIND
        && shape_match.kind == MATCH_KIND
        && placement.kind == PLACEMENT_KIND
        && configuration.kind == MAPPING_KIND;
    let aligned = shape_match.ok
        && shape_match.unmatched_count == 0
        && catalog.catalog_id == shape_match.catalog_id
        && shape_match.catalog_id == placement.catalog_id
        && shape_match.match_id == placement.match_id
        && shape_match.plan_id == placement.plan_id
        && shape_match.source_plan_ref == placement.source_plan_ref
        && shape_match.source_catalog_ref == placement.source_catalog_ref
        && !configuration.candidate_ref.is_empty();
    if !kinds_match || !aligned {
        return Err(PublishError::new(
            PublishDiagnosticCode::InvalidProvenance,
            "catalog, match, placement, and candidate provenance are not aligned",
        ));
    }
    if placement.cell_size <= 0 {
        return Err(PublishError::new(
            PublishDiagnosticCode::InvalidTransform,
            "placement cell size must be positive",
        ));
    }
    Ok(())
}

type Selected<'a> = (
    &'a rusty_procgen_preflight::PieceInstance,
    &'a rusty_procgen_preflight::CatalogShape,
    &'a rusty_procgen_preflight::MatchedPiece,
    &'a PrefabMapping,
    &'a InstanceIdentity,
);

fn validate_source_inventory(
    configuration: &PublishConfiguration,
    source_bodies: &BTreeMap<String, Vec<u8>>,
    selected: &[Selected<'_>],
) -> Result<SourceInventory, PublishError> {
    let source_assets = unique_by(
        &configuration.source_assets,
        |source| source.asset_id.as_str(),
        PublishDiagnosticCode::IncompatibleSourceAsset,
        "source asset inventory",
    )?;
    unique_by(
        &configuration.source_assets,
        |source| source.artifact.as_str(),
        PublishDiagnosticCode::IncompatibleSourceAsset,
        "source artifact inventory",
    )?;
    unique_by(
        &configuration.source_assets,
        |source| source.source.as_str(),
        PublishDiagnosticCode::IncompatibleSourceAsset,
        "source body inventory",
    )?;

    let mut result = BTreeMap::new();
    for source in &configuration.source_assets {
        let id = AssetId::parse(&source.asset_id).map_err(|error| {
            PublishError::new(
                PublishDiagnosticCode::IncompatibleSourceAsset,
                format!("invalid asset id {}: {error}", source.asset_id),
            )
        })?;
        let configured_hash = AssetHash::parse(&source.content_hash).map_err(|error| {
            PublishError::new(
                PublishDiagnosticCode::StalePin,
                format!("invalid pin for {}: {error}", source.asset_id),
            )
        })?;
        let body = source_bodies.get(&source.source).ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::StalePin,
                format!("source body {} is missing", source.source),
            )
        })?;
        let actual = sha256(body);
        if configured_hash.as_str() != actual {
            return Err(PublishError::new(
                PublishDiagnosticCode::StalePin,
                format!(
                    "source {} expected {}, observed {actual}",
                    source.asset_id, source.content_hash
                ),
            ));
        }
        result.insert(
            source.asset_id.clone(),
            ValidatedSource {
                id,
                hash: configured_hash,
                bytes: body.clone(),
            },
        );
    }

    for (_, _, _, mapping, _) in selected {
        if let Some(asset) = mapping.source.asset() {
            let source = source_assets.get(asset).ok_or_else(|| {
                PublishError::new(
                    PublishDiagnosticCode::IncompatibleSourceAsset,
                    format!("prefab source {asset} is absent from the source inventory"),
                )
            })?;
            let parsed = AssetId::parse(&source.asset_id).map_err(|error| {
                PublishError::new(
                    PublishDiagnosticCode::IncompatibleSourceAsset,
                    format!("invalid prefab source {}: {error}", source.asset_id),
                )
            })?;
            match &mapping.source {
                MappedPartSource::VoxelObject { .. }
                    if parsed.kind() != core_assets::AssetKind::VoxelObject =>
                {
                    return Err(PublishError::new(
                        PublishDiagnosticCode::IncompatibleSourceAsset,
                        format!("prefab source {asset} is not a voxel-object asset"),
                    ));
                }
                MappedPartSource::Scene { .. }
                    if parsed.kind() != core_assets::AssetKind::Scene =>
                {
                    return Err(PublishError::new(
                        PublishDiagnosticCode::IncompatibleSourceAsset,
                        format!("prefab source {asset} is not a scene asset"),
                    ));
                }
                _ => {}
            }
        }
    }
    Ok(result)
}

fn build_asset_catalog(
    configuration: &PublishConfiguration,
    sources: &SourceInventory,
) -> Result<AssetCatalog, PublishError> {
    let mut entries = Vec::with_capacity(configuration.source_assets.len());
    for source in &configuration.source_assets {
        let validated = sources.get(&source.asset_id).ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::IncompatibleSourceAsset,
                format!("validated source {} disappeared", source.asset_id),
            )
        })?;
        entries.push(
            CatalogEntry::new(validated.id.clone(), 1)
                .with_hash(validated.hash.clone())
                .with_source(source.artifact.clone())
                .with_label(source.asset_id.clone()),
        );
    }
    Ok(AssetCatalog::from_entries(entries).canonical())
}

fn build_prefab_registry(selected: &[Selected<'_>]) -> PrefabRegistry {
    let mut selected_mappings = BTreeMap::new();
    for (_, shape, _, mapping, _) in selected {
        selected_mappings.insert(mapping.shape_id.as_str(), (*shape, *mapping));
    }
    let definitions = selected_mappings
        .into_values()
        .map(|(shape, mapping)| PrefabDefinition {
            id: PrefabId::new(mapping.prefab_id),
            schema_version: PREFAB_DEFINITION_SCHEMA_VERSION,
            display_name: shape.label.clone(),
            parts: vec![PrefabPart {
                id: PrefabPartId::new(mapping.part_id),
                namespace: mapping.part_namespace.clone(),
                display_name: format!("{} source", shape.label),
                parent: None,
                transform: PrefabTransform::IDENTITY,
                source: mapping.source.to_engine(),
            }],
            part_roles: vec![PrefabPartRoleBinding {
                role: mapping.stable_role.clone(),
                part: PrefabPartId::new(mapping.part_id),
            }],
            variant: None,
        })
        .collect();
    PrefabRegistry {
        schema_version: PREFAB_REGISTRY_SCHEMA_VERSION,
        definitions,
    }
    .canonical()
}

fn build_scene_and_provenance(
    catalog: &ShapeCatalog,
    shape_match: &PieceShapeMatchReport,
    placement: &PiecePlacement,
    configuration: &PublishConfiguration,
    selected: &[Selected<'_>],
) -> Result<(FlatSceneDocument, PublicationProvenance), PublishError> {
    let mut nodes = Vec::with_capacity(selected.len());
    let mut provenance_instances = Vec::with_capacity(selected.len());
    for (order, (instance, shape, matched, mapping, identity)) in selected.iter().enumerate() {
        let transform =
            placement_transform(instance, &shape.allowed_transforms, placement.cell_size)?;
        nodes.push(SceneNodeRecord {
            id: SceneNodeId::new(identity.prefab_instance_id),
            parent: None,
            child_order: order as u32,
            transform,
            kind: SceneNodeKind::EntityInstance(SceneEntityInstance {
                instance_id: instance.instance_id.clone(),
                reference: SceneEntityReference::Prefab {
                    prefab_id: PrefabId::new(mapping.prefab_id),
                    variant_id: None,
                    instantiation_seed: shape_match.seed,
                },
                spawn_marker_id: None,
            }),
            metadata: NodeMetadata {
                label: Some(shape.label.clone()),
                tags: vec![
                    "rusty-procgen".to_owned(),
                    format!("prefab-{}", mapping.prefab_id),
                    format!("prefab-instance-{}", identity.prefab_instance_id),
                ],
            },
        });
        provenance_instances.push(PublishedInstanceProvenance {
            procgen_instance_id: instance.instance_id.clone(),
            prefab_instance_id: identity.prefab_instance_id,
            piece_id: instance.piece_id.clone(),
            shape_id: instance.shape_id.clone(),
            prefab_id: mapping.prefab_id,
            match_score: matched.score,
            source_requirement_ref: instance.source_requirement_ref.clone(),
        });
    }
    let mut scene = FlatSceneDocument {
        id: SceneId::new(configuration.scene.id),
        revision: 0,
        schema_version: CURRENT_SCENE_SCHEMA_VERSION,
        metadata: SceneMetadata {
            name: Some(format!("{} generated layout", configuration.project.name)),
            authoring_format_version: CURRENT_SCENE_SCHEMA_VERSION,
        },
        dependencies: Vec::new(),
        nodes,
    };
    scene.canonicalize();
    Ok((
        scene,
        PublicationProvenance {
            candidate_ref: configuration.candidate_ref.clone(),
            catalog_id: catalog.catalog_id.clone(),
            catalog_ref: placement.source_catalog_ref.clone(),
            plan_id: placement.plan_id.clone(),
            plan_ref: placement.source_plan_ref.clone(),
            match_id: placement.match_id.clone(),
            match_ref: placement.source_match_ref.clone(),
            placement_id: placement.placement_id.clone(),
            instances: provenance_instances,
        },
    ))
}

fn placement_transform(
    instance: &rusty_procgen_preflight::PieceInstance,
    allowed_transforms: &[String],
    cell_size: i32,
) -> Result<SceneTransform, PublishError> {
    if !allowed_transforms.contains(&instance.transform) {
        return Err(PublishError::new(
            PublishDiagnosticCode::InvalidTransform,
            format!(
                "shape {} does not allow transform {}",
                instance.shape_id, instance.transform
            ),
        ));
    }
    let half_sqrt = std::f32::consts::FRAC_1_SQRT_2;
    let rotation = match instance.transform.as_str() {
        "identity" => Quat::IDENTITY,
        "rotate90" => Quat::new(0.0, half_sqrt, 0.0, half_sqrt),
        "rotate180" => Quat::new(0.0, 1.0, 0.0, 0.0),
        "rotate270" => Quat::new(0.0, -half_sqrt, 0.0, half_sqrt),
        other => {
            return Err(PublishError::new(
                PublishDiagnosticCode::InvalidTransform,
                format!("publication adapter does not support transform {other}"),
            ));
        }
    };
    let x = instance.origin.x.checked_mul(cell_size).ok_or_else(|| {
        PublishError::new(
            PublishDiagnosticCode::InvalidTransform,
            format!("instance {} x translation overflowed", instance.instance_id),
        )
    })?;
    let z = instance.origin.y.checked_mul(cell_size).ok_or_else(|| {
        PublishError::new(
            PublishDiagnosticCode::InvalidTransform,
            format!("instance {} z translation overflowed", instance.instance_id),
        )
    })?;
    Ok(SceneTransform {
        translation: Vec3::new(x as f32, 0.0, z as f32),
        rotation,
        scale: Vec3::ONE,
    })
}

#[allow(clippy::too_many_arguments)]
fn encode_owned_bodies(
    configuration: &PublishConfiguration,
    catalog: &AssetCatalog,
    lock: &AssetLock,
    prefabs: &PrefabRegistry,
    scene: &FlatSceneDocument,
    provenance: &PublicationProvenance,
    sources: &SourceInventory,
) -> Result<(ContentManifest, BTreeMap<String, Vec<u8>>), PublishError> {
    let mut bodies = BTreeMap::new();
    insert_body(
        &mut bodies,
        &configuration.asset_catalog_artifact,
        encode_catalog(catalog)?.into_bytes(),
    )?;
    insert_body(
        &mut bodies,
        &configuration.asset_lock_artifact,
        encode_lock(lock)?.into_bytes(),
    )?;
    insert_body(
        &mut bodies,
        &configuration.prefab_registry_artifact,
        encode_prefab_registry(
            &ValidatedPrefabRegistry::new(
                prefabs.clone(),
                &PrefabRegistryValidationContext::from_asset_ids(
                    catalog.iter().map(|entry| entry.id.clone()),
                    std::iter::empty(),
                ),
            )
            .map_err(|report| {
                PublishError::new(
                    PublishDiagnosticCode::LateValidation,
                    diagnostics(report.diagnostics.iter().map(|diagnostic| {
                        format!(
                            "{}@{}: {}",
                            diagnostic.code.as_str(),
                            diagnostic.path,
                            diagnostic.message
                        )
                    })),
                )
            })?,
        )?
        .into_bytes(),
    )?;
    insert_body(
        &mut bodies,
        &configuration.scene.artifact,
        encode_scene(scene)?.into_bytes(),
    )?;
    let mut provenance_json = serde_json::to_vec_pretty(provenance).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("could not encode provenance: {error}"),
        )
    })?;
    provenance_json.push(b'\n');
    insert_body(
        &mut bodies,
        &configuration.provenance_artifact,
        provenance_json,
    )?;
    for source in &configuration.source_assets {
        let validated = sources.get(&source.asset_id).ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::IncompatibleSourceAsset,
                format!("validated source {} disappeared", source.asset_id),
            )
        })?;
        insert_body(&mut bodies, &source.artifact, validated.bytes.clone())?;
    }

    let mut artifacts = Vec::with_capacity(bodies.len());
    for (path, bytes) in &bodies {
        let role = if path == &configuration.asset_catalog_artifact {
            ArtifactRole::AssetCatalog
        } else if path == &configuration.asset_lock_artifact {
            ArtifactRole::AssetLock
        } else if path == &configuration.prefab_registry_artifact {
            ArtifactRole::PrefabRegistry
        } else if path == &configuration.scene.artifact {
            ArtifactRole::SceneDocument
        } else if path == &configuration.provenance_artifact {
            ArtifactRole::GeneratedMetadata
        } else {
            ArtifactRole::Resource("resource:procgen-prefab-source".to_owned())
        };
        artifacts.push(if path == &configuration.provenance_artifact {
            ContentArtifact::generated(path, role, bytes)
        } else {
            ContentArtifact::durable(path, role, bytes)
        });
    }
    let manifest = ContentManifest::new(artifacts).canonical();
    manifest.validate().map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("Rusty Engine rejected the content manifest: {error}"),
        )
    })?;
    Ok((manifest, bodies))
}

fn strict_readback(
    manifest: &ContentManifest,
    bodies: &BTreeMap<String, Vec<u8>>,
    prefab_context: &PrefabRegistryValidationContext,
    scene_resolution: &SceneResolutionContext,
) -> Result<(), PublishError> {
    let manifest_json = encode_manifest(manifest).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("could not encode content manifest: {error}"),
        )
    })?;
    let batch = admit_source_batch(ContentSourceBatch {
        manifest_json,
        bodies: bodies
            .iter()
            .map(|(path, bytes)| ContentBody::new(path, bytes.clone()))
            .collect(),
    })
    .map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!(
                "Rusty Engine rejected strict content readback at {:?}: {:?}",
                error.path, error.code
            ),
        )
    })?;
    let decoded_manifest = batch.manifest.clone();

    let role_body = |role: ArtifactRole| -> Result<&[u8], PublishError> {
        let artifact = decoded_manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.role == role)
            .ok_or_else(|| {
                PublishError::new(
                    PublishDiagnosticCode::LateValidation,
                    format!("reopened manifest has no {} artifact", role.tag()),
                )
            })?;
        batch.body(&artifact.path).ok_or_else(|| {
            PublishError::new(
                PublishDiagnosticCode::LateValidation,
                format!("reopened batch has no body for {}", artifact.path),
            )
        })
    };
    let catalog = decode_catalog(owner_text(role_body(ArtifactRole::AssetCatalog)?)?)?;
    let lock = decode_lock(owner_text(role_body(ArtifactRole::AssetLock)?)?)?;
    if !validate_catalog(&catalog).is_ok() || !validate_lock(&lock, &catalog).is_clean() {
        return Err(PublishError::new(
            PublishDiagnosticCode::LateValidation,
            "reopened asset authority did not validate",
        ));
    }
    let prefabs = decode_prefab_registry(
        owner_text(role_body(ArtifactRole::PrefabRegistry)?)?,
        prefab_context,
    )?;
    if !validate_prefab_registry(prefabs.as_registry(), prefab_context).is_valid() {
        return Err(PublishError::new(
            PublishDiagnosticCode::LateValidation,
            "reopened prefab registry did not validate",
        ));
    }
    let scene = decode_scene(owner_text(role_body(ArtifactRole::SceneDocument)?)?)?;
    let plan = SceneAdmissionPlan::prepare(&scene, scene_resolution).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("reopened scene did not admit: {error}"),
        )
    })?;
    let mut state = EntityState::default();
    plan.apply(&mut state, 0).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("reopened scene entity admission failed: {error}"),
        )
    })?;
    Ok(())
}

fn owner_text(bytes: &[u8]) -> Result<&str, PublishError> {
    std::str::from_utf8(bytes).map_err(|error| {
        PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("reopened owner body was not UTF-8: {error}"),
        )
    })
}

fn insert_body(
    bodies: &mut BTreeMap<String, Vec<u8>>,
    path: &str,
    bytes: Vec<u8>,
) -> Result<(), PublishError> {
    if bodies.insert(path.to_owned(), bytes).is_some() {
        return Err(PublishError::new(
            PublishDiagnosticCode::LateValidation,
            format!("publication path {path} is duplicated"),
        ));
    }
    Ok(())
}

fn unique_by<'a, T>(
    values: &'a [T],
    key: impl Fn(&T) -> &str,
    code: PublishDiagnosticCode,
    label: &str,
) -> Result<BTreeMap<&'a str, &'a T>, PublishError> {
    let mut result = BTreeMap::new();
    for value in values {
        let identity = key(value);
        if result.insert(identity, value).is_some() {
            return Err(PublishError::new(
                code,
                format!("{label} contains duplicate identity {identity}"),
            ));
        }
    }
    Ok(result)
}

fn unique_positive_ids(
    values: impl IntoIterator<Item = u64>,
    label: &str,
) -> Result<(), PublishError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value == 0 || !seen.insert(value) {
            return Err(PublishError::new(
                PublishDiagnosticCode::DuplicateInstanceIdentity,
                format!("{label} {value} must be positive and unique"),
            ));
        }
    }
    Ok(())
}

fn validate_stable_role(role: &str, shape_id: &str) -> Result<(), PublishError> {
    let valid = !role.is_empty()
        && role.split('/').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with('-')
                && !segment.ends_with('-')
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        });
    if !valid {
        return Err(PublishError::new(
            PublishDiagnosticCode::MissingStableRole,
            format!("shape {shape_id} requires a slash-scoped stable part role"),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn engine_source_manifest() -> Result<EngineSourceManifest, PublishError> {
    let source: EngineSourceManifest =
        serde_json::from_str(include_str!("../../../engine-source.json")).map_err(|error| {
            PublishError::new(
                PublishDiagnosticCode::StalePin,
                format!("engine-source.json is invalid: {error}"),
            )
        })?;
    if source.schema_version != 1
        || source.public_repository != ENGINE_PUBLIC_REPOSITORY
        || source.commit.len() != 40
        || !source
            .commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PublishError::new(
            PublishDiagnosticCode::StalePin,
            "engine-source.json does not name schema 1 and one exact public Engine commit",
        ));
    }
    Ok(source)
}

fn diagnostics(values: impl IntoIterator<Item = String>) -> String {
    values.into_iter().collect::<Vec<_>>().join(", ")
}

impl From<asset_catalog::AssetCatalogCodecError> for PublishError {
    fn from(error: asset_catalog::AssetCatalogCodecError) -> Self {
        Self::new(
            PublishDiagnosticCode::LateValidation,
            format!("Rusty Engine asset codec rejected publication: {error}"),
        )
    }
}

impl From<content_store::PrefabCodecError> for PublishError {
    fn from(error: content_store::PrefabCodecError) -> Self {
        Self::new(
            PublishDiagnosticCode::LateValidation,
            format!("Rusty Engine prefab codec rejected publication: {error}"),
        )
    }
}

impl From<authored_scene::SceneCodecError> for PublishError {
    fn from(error: authored_scene::SceneCodecError) -> Self {
        Self::new(
            PublishDiagnosticCode::LateValidation,
            format!("Rusty Engine scene codec rejected publication: {error}"),
        )
    }
}
