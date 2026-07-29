fn build_catalog_inspect_command(args: BuildCatalogInspectArgs) -> Result<(), String> {
    let catalog: ShapeCatalog = read_json(&args.catalog)?;
    let report = inspect_shape_catalog(&catalog, &args.catalog);
    write_json(&args.out, &report)
}

fn inspect_shape_catalog(catalog: &ShapeCatalog, catalog_path: &Path) -> CatalogInspectionReport {
    let mut piece_kinds = BTreeSet::new();
    let mut feature_sockets = BTreeSet::new();
    let mut exit_directions = BTreeSet::new();
    let mut transforms = BTreeSet::new();
    let mut diagnostics = Vec::new();
    let mut seen_shapes = BTreeSet::new();
    let mut shapes = Vec::new();

    if catalog.kind != "rusty_procgen.shape_catalog.v1" {
        diagnostics.push(fatal(
            "catalog_kind_invalid",
            None,
            None,
            format!("Expected rusty_procgen.shape_catalog.v1, got {}.", catalog.kind),
        ));
    }
    if catalog.cell_size <= 0 {
        diagnostics.push(fatal(
            "catalog_cell_size_invalid",
            None,
            None,
            "Catalog cellSize must be positive.",
        ));
    }
    validate_piece_placement_policy(&catalog.placement_policy, &mut diagnostics);
    validate_catalog_search_policy(&catalog.catalog_search_policy, &mut diagnostics);

    for shape in &catalog.shapes {
        if !seen_shapes.insert(shape.shape_id.as_str()) {
            diagnostics.push(fatal(
                "catalog_shape_duplicate",
                None,
                None,
                format!("Duplicate shape id {}.", shape.shape_id),
            ));
        }
        if shape.piece_kinds.is_empty() {
            diagnostics.push(fatal(
                "catalog_shape_piece_kind_missing",
                None,
                None,
                format!("Shape {} has no piece kinds.", shape.shape_id),
            ));
        }
        if shape.footprint.is_empty() {
            diagnostics.push(fatal(
                "catalog_shape_footprint_missing",
                None,
                None,
                format!("Shape {} has no footprint cells.", shape.shape_id),
            ));
        }
        if shape.exits.is_empty() {
            diagnostics.push(fatal(
                "catalog_shape_exit_missing",
                None,
                None,
                format!("Shape {} has no exits.", shape.shape_id),
            ));
        }
        if shape.allowed_transforms.is_empty() {
            diagnostics.push(fatal(
                "catalog_shape_transform_missing",
                None,
                None,
                format!("Shape {} has no allowed transforms.", shape.shape_id),
            ));
        }
        if shape.piece_kinds.iter().any(|kind| kind == "junction")
            && !shape.tags.iter().any(|tag| tag == "planned_junction")
        {
            diagnostics.push(fatal(
                "catalog_junction_ownership_tag_missing",
                None,
                None,
                format!(
                    "Junction shape {} must be explicitly tagged planned_junction.",
                    shape.shape_id
                ),
            ));
        }

        piece_kinds.extend(shape.piece_kinds.iter().cloned());
        transforms.extend(shape.allowed_transforms.iter().cloned());
        exit_directions.extend(shape.exits.iter().map(|exit| exit.direction.clone()));
        feature_sockets.extend(
            shape
                .feature_sockets
                .iter()
                .map(|socket| socket.kind.clone()),
        );
        shapes.push(CatalogShapeSummary {
            shape_id: shape.shape_id.clone(),
            piece_kinds: shape.piece_kinds.clone(),
            footprint_cells: shape.footprint.len(),
            reserved_cells: shape.reserved_cells.len(),
            exit_count: shape.exits.len(),
            feature_socket_kinds: dedupe_strings(
                shape
                    .feature_sockets
                    .iter()
                    .map(|socket| socket.kind.clone())
                    .collect(),
            ),
            allowed_transforms: shape.allowed_transforms.clone(),
            tags: shape.tags.clone(),
        });
    }

    CatalogInspectionReport {
        kind: "rusty_procgen.catalog_inspection.v1".to_owned(),
        schema_version: 1,
        catalog_id: catalog.catalog_id.clone(),
        catalog_ref: display_path(catalog_path),
        shape_count: catalog.shapes.len(),
        placement_policy: catalog.placement_policy.clone(),
        catalog_search_policy: catalog.catalog_search_policy.clone(),
        piece_kinds: piece_kinds.into_iter().collect(),
        feature_sockets: feature_sockets.into_iter().collect(),
        exit_directions: exit_directions.into_iter().collect(),
        transforms: transforms.into_iter().collect(),
        shapes,
        diagnostics,
    }
}

fn validate_catalog_search_policy(
    policy: &CatalogSearchPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if policy.schema_version != 1
        || policy.max_decisions == 0
        || policy.max_backtracks == 0
        || policy.max_chain_expansions_per_section == 0
        || policy.max_room_origin_alternatives == 0
        || policy.max_room_rotation_alternatives == 0
        || policy.max_room_rotation_alternatives > 4
    {
        diagnostics.push(fatal(
            "catalog_search_policy_invalid",
            None,
            None,
            "Catalog search policy must use schemaVersion 1 with positive bounded budgets and at most four 90-degree room rotations.",
        ));
    }
}

fn validate_piece_placement_policy(
    policy: &PiecePlacementPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if policy.schema_version != 1 {
        diagnostics.push(fatal(
            "catalog_placement_policy_schema_invalid",
            None,
            None,
            format!(
                "Placement policy schemaVersion must be 1, got {}.",
                policy.schema_version
            ),
        ));
    }
    if policy.minimum_clearance_cells < 0 {
        diagnostics.push(fatal(
            "catalog_minimum_clearance_invalid",
            None,
            None,
            "Placement policy minimumClearanceCells must be non-negative.",
        ));
    }
    if policy.wall_thickness_cells <= 0 {
        diagnostics.push(fatal(
            "catalog_wall_thickness_invalid",
            None,
            None,
            "Placement policy wallThicknessCells must be positive.",
        ));
    }
    if policy.minimum_clearance_cells < policy.wall_thickness_cells * 2 + 1 {
        diagnostics.push(fatal(
            "catalog_minimum_clearance_too_small_for_walls",
            None,
            None,
            format!(
                "Placement policy minimumClearanceCells must be at least twice wallThicknessCells plus one (minimum {} for wall thickness {}).",
                policy.wall_thickness_cells * 2 + 1,
                policy.wall_thickness_cells
            ),
        ));
    }
    if policy.doorway_width_cells <= 0 || policy.doorway_width_cells % 2 == 0 {
        diagnostics.push(fatal(
            "catalog_doorway_width_invalid",
            None,
            None,
            "Placement policy doorwayWidthCells must be a positive odd number.",
        ));
    }
    if policy.doorway_width_cells != 1 {
        diagnostics.push(fatal(
            "catalog_doorway_width_unsupported",
            None,
            None,
            "Placement policy schemaVersion 1 supports doorwayWidthCells=1 only; wider openings require authoritative oriented-footprint routing.",
        ));
    }
    if !policy.preserve_piece_boundaries {
        diagnostics.push(fatal(
            "catalog_piece_boundary_preservation_required",
            None,
            None,
            "Placement policy schemaVersion 1 requires preservePieceBoundaries=true.",
        ));
    }
}
