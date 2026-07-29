use std::collections::{BTreeMap, BTreeSet, VecDeque};

use engine_spatial::{validate_voxel_address, validate_voxel_material_slot, MAX_SOLID_VOXELS};
use rusty_procgen_preflight::core::ProcgenCore;
use rusty_procgen_preflight::{
    CorridorRealization, GluedExit, GridConnectivity, PieceContactPolicy, PiecePlacement,
    PiecePlacementPolicy, PlacementCellRef, Severity,
};

use crate::{
    DoorPortalExtrusion, ExtrusionBounds, ExtrusionOptions, PlanVoxel, SpatialExtrusionError,
    VoxelCoordinate, VoxelExtrusionPlan,
};

type Cell = (i32, i32);

pub fn compile_placement_extrusion(
    placement: &PiecePlacement,
    options: ExtrusionOptions,
) -> Result<VoxelExtrusionPlan, SpatialExtrusionError> {
    validate_placement(placement)?;
    validate_options(options)?;

    let occupied = owned_cells_by_position(&placement.occupied_cells, "occupied")?;
    let reserved = owned_cells_by_position(&placement.reserved_cells, "reserved")?;
    let contact_sections = catalog_contact_sections(placement);
    validate_owned_clearance(
        placement,
        &occupied,
        &placement.placement_policy,
        &contact_sections,
    )?;
    let openings = declared_opening_cells(placement, &occupied, &reserved, &contact_sections)?;

    let mut walkable = BTreeSet::new();
    walkable.extend(placement.occupied_cells.iter().map(|cell| (cell.x, cell.y)));
    walkable.extend(openings.iter().copied());
    if walkable.is_empty() {
        return Err(SpatialExtrusionError::malformed(
            "empty_placement",
            "piece placement has no occupied or connection cells to extrude",
        ));
    }

    let boundary = build_wall_shell(&walkable, placement.placement_policy.wall_thickness_cells)?;
    let mut solids = BTreeMap::new();
    for &(x, z) in &walkable {
        set_solid(
            &mut solids,
            [i64::from(x), options.floor_y, i64::from(z)],
            options.floor_material,
        )?;
        set_solid(
            &mut solids,
            [i64::from(x), options.ceiling_y, i64::from(z)],
            options.ceiling_material,
        )?;
        ensure_voxel_quota(solids.len())?;
    }
    for &(x, z) in &boundary {
        for y in options.wall_min_y..=options.wall_max_y {
            set_solid(
                &mut solids,
                [i64::from(x), y, i64::from(z)],
                options.wall_material,
            )?;
            ensure_voxel_quota(solids.len())?;
        }
    }
    ensure_voxel_quota(solids.len())?;

    let solid_voxels = solids
        .into_iter()
        .map(|(address, material)| PlanVoxel {
            coord: address.into(),
            material,
        })
        .collect::<Vec<_>>();
    let build_bounds = bounds_for(&solid_voxels)?;
    let resident_chunk_count = required_chunk_count(&solid_voxels, options.chunk_size);
    let door_portals = placement
        .gate_portals
        .iter()
        .map(|portal| DoorPortalExtrusion {
            id: portal.id.clone(),
            source_edge: portal.source_edge.clone(),
            required_item: portal.required_item.clone(),
            traversal: portal.traversal.clone(),
            orientation: portal.orientation.clone(),
            cells: portal.cells.clone(),
            min_y: options.wall_min_y,
            max_exclusive_y: options.wall_max_y + 1,
        })
        .collect::<Vec<_>>();

    Ok(VoxelExtrusionPlan {
        schema_version: 1,
        placement_id: placement.placement_id.clone(),
        coordinate_mapping: "placement_x_y_to_voxel_x_z".to_owned(),
        walkable_cell_count: walkable.len(),
        opening_cell_count: openings.len(),
        boundary_cell_count: boundary.len(),
        solid_voxel_count: solid_voxels.len(),
        resident_chunk_count,
        solid_voxels,
        door_portals,
        build_bounds,
    })
}

fn validate_placement(placement: &PiecePlacement) -> Result<(), SpatialExtrusionError> {
    if placement.kind != "rusty_procgen.piece_placement.v1" || placement.schema_version != 1 {
        return Err(SpatialExtrusionError::malformed(
            "unsupported_schema",
            format!(
                "expected rusty_procgen.piece_placement.v1 schema 1, received {} schema {}",
                placement.kind, placement.schema_version
            ),
        ));
    }
    if placement.grid_connectivity != GridConnectivity::FourWay {
        return Err(SpatialExtrusionError::malformed(
            "unsupported_connectivity",
            "voxel extrusion requires four_way placement connectivity",
        ));
    }
    validate_policy(&placement.placement_policy)?;
    let report = ProcgenCore::validate_placement(placement);
    if !report.ok {
        let detail = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Fatal)
            .take(4)
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.detail))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(SpatialExtrusionError::malformed(
            "placement_validation_failed",
            detail,
        ));
    }

    let mut portal_ids = BTreeSet::new();
    let mut source_edges = BTreeSet::new();
    for portal in &placement.gate_portals {
        if portal.id.is_empty()
            || portal.source_edge.is_empty()
            || !portal_ids.insert(portal.id.as_str())
            || !source_edges.insert(portal.source_edge.as_str())
        {
            return Err(SpatialExtrusionError::malformed(
                "duplicate_portal_identity",
                format!(
                    "duplicate or empty gate portal {} / source edge {}",
                    portal.id, portal.source_edge
                ),
            ));
        }
        if portal.width <= 0 || portal.cells.len() != portal.width as usize {
            return Err(SpatialExtrusionError::malformed(
                "invalid_portal_width",
                format!(
                    "gate portal {} must provide exactly one cell per positive width unit",
                    portal.id
                ),
            ));
        }
        direction_vector(&portal.orientation).ok_or_else(|| {
            SpatialExtrusionError::malformed(
                "invalid_portal_orientation",
                format!(
                    "gate portal {} has unsupported orientation {}",
                    portal.id, portal.orientation
                ),
            )
        })?;
    }
    Ok(())
}

fn validate_policy(policy: &PiecePlacementPolicy) -> Result<(), SpatialExtrusionError> {
    if policy.schema_version != 1 {
        return Err(SpatialExtrusionError::malformed(
            "unsupported_policy_schema",
            format!(
                "expected placement policy schema 1, received {}",
                policy.schema_version
            ),
        ));
    }
    if policy.minimum_clearance_cells < 0 {
        return Err(SpatialExtrusionError::malformed(
            "invalid_clearance",
            "minimum clearance must be non-negative",
        ));
    }
    if policy.contact_policy != PieceContactPolicy::GluedExitsOnly {
        return Err(SpatialExtrusionError::malformed(
            "unsupported_contact_policy",
            "placement policy must use glued_exits_only",
        ));
    }
    if policy.wall_thickness_cells <= 0 {
        return Err(SpatialExtrusionError::malformed(
            "invalid_wall_thickness",
            "wall thickness must be positive",
        ));
    }
    let minimum = policy
        .wall_thickness_cells
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            SpatialExtrusionError::malformed(
                "policy_overflow",
                "wall thickness cannot be represented safely",
            )
        })?;
    if policy.minimum_clearance_cells < minimum {
        return Err(SpatialExtrusionError::malformed(
            "invalid_clearance",
            "minimum clearance must be at least twice wall thickness plus one",
        ));
    }
    if policy.doorway_width_cells != 1 {
        return Err(SpatialExtrusionError::malformed(
            "unsupported_doorway_width",
            "placement policy schema 1 supports doorway width 1 only",
        ));
    }
    if !policy.preserve_piece_boundaries {
        return Err(SpatialExtrusionError::malformed(
            "piece_boundaries_required",
            "placement policy schema 1 requires preserved piece boundaries",
        ));
    }
    Ok(())
}

pub(crate) fn validate_options(options: ExtrusionOptions) -> Result<(), SpatialExtrusionError> {
    if options.chunk_size == 0 {
        return Err(SpatialExtrusionError::malformed(
            "invalid_chunk_size",
            "chunk size must be positive",
        ));
    }
    if options.wall_min_y > options.wall_max_y
        || options.floor_y >= options.wall_min_y
        || options.ceiling_y <= options.wall_max_y
    {
        return Err(SpatialExtrusionError::malformed(
            "invalid_enclosure",
            "floor, wall, and ceiling heights must form a non-overlapping enclosure",
        ));
    }
    let mut materials = BTreeSet::new();
    for material in options.allowed_materials() {
        validate_voxel_material_slot(material)
            .map_err(|_| SpatialExtrusionError::UnknownMaterial { material })?;
        if !materials.insert(material) {
            return Err(SpatialExtrusionError::malformed(
                "duplicate_enclosure_material",
                "floor, wall, and ceiling materials must be distinct",
            ));
        }
    }
    Ok(())
}

fn owned_cells_by_position<'a>(
    cells: &'a [PlacementCellRef],
    kind: &'static str,
) -> Result<BTreeMap<Cell, &'a PlacementCellRef>, SpatialExtrusionError> {
    let mut by_cell = BTreeMap::new();
    for cell in cells {
        if cell.instance_id.is_empty() {
            return Err(SpatialExtrusionError::malformed(
                "missing_cell_owner",
                format!("{kind} cell {},{} has no instance owner", cell.x, cell.y),
            ));
        }
        if let Some(existing) = by_cell.insert((cell.x, cell.y), cell) {
            return Err(SpatialExtrusionError::malformed(
                "duplicate_owned_cell",
                format!(
                    "{kind} cell {},{} is shared by {} and {}",
                    cell.x, cell.y, existing.instance_id, cell.instance_id
                ),
            ));
        }
    }
    Ok(by_cell)
}

fn validate_owned_clearance(
    placement: &PiecePlacement,
    occupied: &BTreeMap<Cell, &PlacementCellRef>,
    policy: &PiecePlacementPolicy,
    contact_sections: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(), SpatialExtrusionError> {
    let clearance = policy.minimum_clearance_cells;
    for cell in occupied.values() {
        for dy in -clearance..=clearance {
            for dx in -clearance..=clearance {
                let distance = i64::from(dx).abs() + i64::from(dy).abs();
                if distance == 0 || distance > i64::from(clearance) {
                    continue;
                }
                let Some(x) = cell.x.checked_add(dx) else {
                    return Err(coordinate_overflow());
                };
                let Some(y) = cell.y.checked_add(dy) else {
                    return Err(coordinate_overflow());
                };
                let Some(other) = occupied.get(&(x, y)) else {
                    continue;
                };
                if other.instance_id == cell.instance_id {
                    continue;
                }
                let same_owned_section = placement.corridor_realization
                    != CorridorRealization::Procedural
                    && contact_sections.values().any(|instances| {
                        instances.contains(&cell.instance_id)
                            && instances.contains(&other.instance_id)
                    });
                if !same_owned_section {
                    return Err(SpatialExtrusionError::malformed(
                        "piece_clearance_violation",
                        format!(
                            "clearance {clearance} violated by {} and {}",
                            cell.instance_id, other.instance_id
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn catalog_contact_sections(placement: &PiecePlacement) -> BTreeMap<String, BTreeSet<String>> {
    let mut sections = BTreeMap::<String, BTreeSet<String>>::new();
    for instance in &placement.instances {
        for source_ref in &instance.source_refs {
            if let Some(section) = source_ref.strip_prefix("physicalSection:") {
                sections
                    .entry(section.to_owned())
                    .or_default()
                    .insert(instance.instance_id.clone());
            }
        }
    }
    for glued in &placement.glued_exits {
        if glued.source_section.is_empty() {
            continue;
        }
        let instances = sections.entry(glued.source_section.clone()).or_default();
        instances.insert(glued.from_instance.clone());
        instances.insert(glued.to_instance.clone());
    }
    sections
}

fn declared_opening_cells(
    placement: &PiecePlacement,
    occupied: &BTreeMap<Cell, &PlacementCellRef>,
    reserved: &BTreeMap<Cell, &PlacementCellRef>,
    contact_sections: &BTreeMap<String, BTreeSet<String>>,
) -> Result<BTreeSet<Cell>, SpatialExtrusionError> {
    let mut openings_by_owner = BTreeMap::<String, &GluedExit>::new();
    for glued in &placement.glued_exits {
        validate_glued_exit(placement, glued, occupied)?;
        let owner = format!("connection.{}", slugify_label(&glued.id));
        if openings_by_owner.insert(owner.clone(), glued).is_some() {
            return Err(SpatialExtrusionError::malformed(
                "duplicate_opening_owner",
                format!("piece placement repeats routed opening owner {owner}"),
            ));
        }
    }

    let mut openings = BTreeSet::new();
    if placement.corridor_realization == CorridorRealization::Catalog {
        if !placement.connection_cells.is_empty() {
            return Err(SpatialExtrusionError::malformed(
                "catalog_connection_cells",
                "pure catalog placement must not contain generated connection cells",
            ));
        }
        for glued in &placement.glued_exits {
            let from = (glued.from_cell.x, glued.from_cell.y);
            let to = (glued.to_cell.x, glued.to_cell.y);
            let distance = (i64::from(from.0) - i64::from(to.0)).abs()
                + (i64::from(from.1) - i64::from(to.1)).abs();
            let from_owner = occupied.get(&from).map(|cell| cell.instance_id.as_str());
            let to_owner = occupied.get(&to).map(|cell| cell.instance_id.as_str());
            if distance != 1
                || opposite_direction(&glued.from_direction) != Some(glued.to_direction.as_str())
                || from_owner != Some(glued.to_instance.as_str())
                || to_owner != Some(glued.from_instance.as_str())
            {
                return Err(SpatialExtrusionError::malformed(
                    "invalid_catalog_glue",
                    format!(
                        "pure catalog glue {} is not an exact adjacent prefab-port join",
                        glued.id
                    ),
                ));
            }
        }
        return Ok(openings);
    }

    let mut routes = BTreeMap::<String, BTreeSet<Cell>>::new();
    for cell in &placement.connection_cells {
        let Some(glued) = openings_by_owner.get(&cell.instance_id) else {
            return Err(SpatialExtrusionError::malformed(
                "unknown_connection_owner",
                format!(
                    "connection cell {},{} is not owned by a declared glued exit",
                    cell.x, cell.y
                ),
            ));
        };
        let key = (cell.x, cell.y);
        if !routes
            .entry(cell.instance_id.clone())
            .or_default()
            .insert(key)
        {
            return Err(SpatialExtrusionError::malformed(
                "duplicate_connection_cell",
                format!(
                    "connection {} repeats route cell {},{}",
                    cell.instance_id, cell.x, cell.y
                ),
            ));
        }
        if occupied.contains_key(&key) {
            return Err(SpatialExtrusionError::malformed(
                "opening_crosses_occupied",
                format!(
                    "doorway {} crosses occupied cell {},{}",
                    glued.id, cell.x, cell.y
                ),
            ));
        }
        let declared_endpoint_reservation = reserved.get(&key).is_some_and(|owner| {
            (key == (glued.from_cell.x, glued.from_cell.y)
                && owner.instance_id == glued.from_instance)
                || (key == (glued.to_cell.x, glued.to_cell.y)
                    && owner.instance_id == glued.to_instance)
        });
        if reserved.contains_key(&key) && !declared_endpoint_reservation {
            return Err(SpatialExtrusionError::malformed(
                "opening_crosses_reservation",
                format!("doorway {} crosses a foreign reservation", glued.id),
            ));
        }
        validate_opening_wall_clearance(
            key,
            glued,
            occupied,
            placement.placement_policy.wall_thickness_cells,
            if placement.corridor_realization == CorridorRealization::Hybrid {
                contact_sections.get(&glued.source_section)
            } else {
                None
            },
        )?;
        openings.insert(key);
    }

    for (owner, glued) in openings_by_owner {
        let Some(route) = routes.get(&owner) else {
            return Err(SpatialExtrusionError::malformed(
                "missing_connection_route",
                format!("declared glued exit {owner} has no routed connection cells"),
            ));
        };
        let from = (glued.from_cell.x, glued.from_cell.y);
        let to = (glued.to_cell.x, glued.to_cell.y);
        if !route.contains(&from) || !route.contains(&to) {
            return Err(SpatialExtrusionError::malformed(
                "incomplete_connection_route",
                format!("declared glued exit {owner} route omits an endpoint"),
            ));
        }
        let same_section_instances =
            if placement.corridor_realization == CorridorRealization::Hybrid {
                contact_sections.get(&glued.source_section)
            } else {
                None
            };
        let mut traversable = route.clone();
        if let Some(instances) = same_section_instances {
            traversable.extend(
                occupied
                    .iter()
                    .filter(|(_, cell)| instances.contains(&cell.instance_id))
                    .map(|(&position, _)| position),
            );
        }
        let reachable = reachable_cells(from, &traversable)?;
        if !reachable.contains(&to) || route.iter().any(|cell| !reachable.contains(cell)) {
            return Err(SpatialExtrusionError::malformed(
                "disconnected_connection_route",
                format!("declared glued exit {owner} route is disconnected"),
            ));
        }
    }
    Ok(openings)
}

fn validate_glued_exit(
    placement: &PiecePlacement,
    glued: &GluedExit,
    occupied: &BTreeMap<Cell, &PlacementCellRef>,
) -> Result<(), SpatialExtrusionError> {
    if glued.id.is_empty()
        || glued.from_instance.is_empty()
        || glued.to_instance.is_empty()
        || glued.from_width != 1
        || glued.to_width != 1
        || direction_vector(&glued.from_direction).is_none()
        || direction_vector(&glued.to_direction).is_none()
    {
        return Err(SpatialExtrusionError::malformed(
            "invalid_glued_exit",
            "piece placement glued exits require identified width-1 transformed endpoints",
        ));
    }
    if placement.corridor_realization != CorridorRealization::Procedural
        && glued.route_points.len() < 2
        && opposite_direction(&glued.from_direction) != Some(glued.to_direction.as_str())
    {
        return Err(SpatialExtrusionError::malformed(
            "incompatible_exit_directions",
            format!(
                "piece placement glued exit {} has incompatible directions",
                glued.id
            ),
        ));
    }
    validate_endpoint_geometry(
        glued,
        &glued.from_instance,
        (glued.from_cell.x, glued.from_cell.y),
        &glued.from_direction,
        occupied,
    )?;
    validate_endpoint_geometry(
        glued,
        &glued.to_instance,
        (glued.to_cell.x, glued.to_cell.y),
        &glued.to_direction,
        occupied,
    )
}

fn validate_endpoint_geometry(
    glued: &GluedExit,
    instance_id: &str,
    exit: Cell,
    direction: &str,
    occupied: &BTreeMap<Cell, &PlacementCellRef>,
) -> Result<(), SpatialExtrusionError> {
    let vector = direction_vector(direction).expect("validated direction");
    let inside = (
        exit.0
            .checked_sub(vector.0)
            .ok_or_else(coordinate_overflow)?,
        exit.1
            .checked_sub(vector.1)
            .ok_or_else(coordinate_overflow)?,
    );
    if occupied
        .get(&inside)
        .map(|owner| owner.instance_id.as_str())
        != Some(instance_id)
    {
        return Err(SpatialExtrusionError::malformed(
            "invalid_exit_boundary",
            format!(
                "glued exit {} endpoint {},{} is not on the {direction} boundary of {instance_id}",
                glued.id, exit.0, exit.1
            ),
        ));
    }
    Ok(())
}

fn validate_opening_wall_clearance(
    opened: Cell,
    glued: &GluedExit,
    occupied: &BTreeMap<Cell, &PlacementCellRef>,
    wall_thickness: i32,
    same_section_instances: Option<&BTreeSet<String>>,
) -> Result<(), SpatialExtrusionError> {
    for dy in -wall_thickness..=wall_thickness {
        for dx in -wall_thickness..=wall_thickness {
            if i64::from(dx).abs() + i64::from(dy).abs() > i64::from(wall_thickness) {
                continue;
            }
            let position = (
                opened.0.checked_add(dx).ok_or_else(coordinate_overflow)?,
                opened.1.checked_add(dy).ok_or_else(coordinate_overflow)?,
            );
            let Some(owner) = occupied.get(&position) else {
                continue;
            };
            let allowed = if owner.instance_id == glued.from_instance {
                endpoint_tunnel_contains(
                    opened,
                    (glued.from_cell.x, glued.from_cell.y),
                    &glued.from_direction,
                    wall_thickness,
                )?
            } else if owner.instance_id == glued.to_instance {
                endpoint_tunnel_contains(
                    opened,
                    (glued.to_cell.x, glued.to_cell.y),
                    &glued.to_direction,
                    wall_thickness,
                )?
            } else {
                same_section_instances
                    .is_some_and(|instances| instances.contains(&owner.instance_id))
            };
            if !allowed {
                return Err(SpatialExtrusionError::malformed(
                    "opening_wall_clearance",
                    format!(
                        "doorway {} enters non-exit wall clearance of {}",
                        glued.id, owner.instance_id
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn endpoint_tunnel_contains(
    position: Cell,
    exit: Cell,
    direction: &str,
    wall_thickness: i32,
) -> Result<bool, SpatialExtrusionError> {
    let vector = direction_vector(direction).ok_or_else(|| {
        SpatialExtrusionError::malformed(
            "invalid_direction",
            format!("unsupported direction {direction}"),
        )
    })?;
    for step in 0..wall_thickness {
        let x = exit
            .0
            .checked_add(vector.0.checked_mul(step).ok_or_else(coordinate_overflow)?)
            .ok_or_else(coordinate_overflow)?;
        let y = exit
            .1
            .checked_add(vector.1.checked_mul(step).ok_or_else(coordinate_overflow)?)
            .ok_or_else(coordinate_overflow)?;
        if position == (x, y) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn reachable_cells(
    start: Cell,
    traversable: &BTreeSet<Cell>,
) -> Result<BTreeSet<Cell>, SpatialExtrusionError> {
    let mut reachable = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(cell) = queue.pop_front() {
        for neighbor in cardinal_neighbors(cell)? {
            if traversable.contains(&neighbor) && reachable.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    Ok(reachable)
}

fn build_wall_shell(
    walkable: &BTreeSet<Cell>,
    thickness: i32,
) -> Result<BTreeSet<Cell>, SpatialExtrusionError> {
    let mut boundary = BTreeSet::new();
    let mut frontier = walkable.clone();
    for _ in 0..thickness {
        let mut next = BTreeSet::new();
        for &cell in &frontier {
            for neighbor in cardinal_neighbors(cell)? {
                if !walkable.contains(&neighbor) && boundary.insert(neighbor) {
                    next.insert(neighbor);
                }
            }
        }
        frontier = next;
    }
    Ok(boundary)
}

fn cardinal_neighbors(cell: Cell) -> Result<[Cell; 4], SpatialExtrusionError> {
    Ok([
        (
            cell.0.checked_add(1).ok_or_else(coordinate_overflow)?,
            cell.1,
        ),
        (
            cell.0.checked_sub(1).ok_or_else(coordinate_overflow)?,
            cell.1,
        ),
        (
            cell.0,
            cell.1.checked_add(1).ok_or_else(coordinate_overflow)?,
        ),
        (
            cell.0,
            cell.1.checked_sub(1).ok_or_else(coordinate_overflow)?,
        ),
    ])
}

fn direction_vector(direction: &str) -> Option<Cell> {
    match direction {
        "north" => Some((0, -1)),
        "east" => Some((1, 0)),
        "south" => Some((0, 1)),
        "west" => Some((-1, 0)),
        _ => None,
    }
}

fn opposite_direction(direction: &str) -> Option<&'static str> {
    match direction {
        "north" => Some("south"),
        "east" => Some("west"),
        "south" => Some("north"),
        "west" => Some("east"),
        _ => None,
    }
}

fn slugify_label(label: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for character in label.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            if separator && !output.is_empty() {
                output.push('_');
            }
            output.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    if output.is_empty() {
        "fork".to_owned()
    } else {
        output
    }
}

fn set_solid(
    solids: &mut BTreeMap<[i64; 3], u16>,
    address: [i64; 3],
    material: u16,
) -> Result<(), SpatialExtrusionError> {
    validate_voxel_address(address).map_err(|error| {
        SpatialExtrusionError::malformed("coordinate_out_of_bounds", error.to_string())
    })?;
    solids.insert(address, material);
    Ok(())
}

fn ensure_voxel_quota(actual: usize) -> Result<(), SpatialExtrusionError> {
    if actual > MAX_SOLID_VOXELS {
        return Err(SpatialExtrusionError::TooManySolidVoxels {
            limit: MAX_SOLID_VOXELS,
            actual,
        });
    }
    Ok(())
}

fn required_chunk_count(voxels: &[PlanVoxel], chunk_size: u32) -> usize {
    let chunk_size = i64::from(chunk_size);
    voxels
        .iter()
        .map(|voxel| {
            [
                voxel.coord.x.div_euclid(chunk_size),
                voxel.coord.y.div_euclid(chunk_size),
                voxel.coord.z.div_euclid(chunk_size),
            ]
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn bounds_for(voxels: &[PlanVoxel]) -> Result<ExtrusionBounds, SpatialExtrusionError> {
    let Some(first) = voxels.first() else {
        return Err(SpatialExtrusionError::malformed(
            "empty_extrusion",
            "voxel extrusion requires at least one solid",
        ));
    };
    let mut min = first.coord;
    let mut max = first.coord;
    for voxel in &voxels[1..] {
        min.x = min.x.min(voxel.coord.x);
        min.y = min.y.min(voxel.coord.y);
        min.z = min.z.min(voxel.coord.z);
        max.x = max.x.max(voxel.coord.x);
        max.y = max.y.max(voxel.coord.y);
        max.z = max.z.max(voxel.coord.z);
    }
    Ok(ExtrusionBounds {
        min,
        max_exclusive: VoxelCoordinate {
            x: max.x.checked_add(1).ok_or_else(coordinate_overflow)?,
            y: max.y.checked_add(1).ok_or_else(coordinate_overflow)?,
            z: max.z.checked_add(1).ok_or_else(coordinate_overflow)?,
        },
    })
}

fn coordinate_overflow() -> SpatialExtrusionError {
    SpatialExtrusionError::malformed(
        "coordinate_overflow",
        "placement coordinate arithmetic overflowed",
    )
}

#[cfg(test)]
mod tests {
    use super::slugify_label;

    #[test]
    fn opening_owner_slug_matches_the_established_projection() {
        assert_eq!(slugify_label("Edge / Alpha"), "edge_alpha");
        assert_eq!(slugify_label("***"), "fork");
    }
}
