export interface VoxelCoord {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface PlacementCell {
  readonly x: number;
  readonly y: number;
}

export interface PlacementOwnedCell extends PlacementCell {
  readonly instanceId: string;
}

export interface PiecePlacementPolicy {
  readonly schemaVersion: 1;
  readonly minimumClearanceCells: number;
  readonly contactPolicy: 'glued_exits_only';
  readonly wallThicknessCells: number;
  readonly doorwayWidthCells: number;
  readonly preservePieceBoundaries: boolean;
}

export interface PlacementGluedExit {
  readonly id: string;
  readonly sourceSection?: string;
  readonly fromInstance: string;
  readonly fromCell: PlacementCell;
  readonly fromDirection: 'north' | 'east' | 'south' | 'west';
  readonly fromWidth: number;
  readonly toInstance: string;
  readonly toCell: PlacementCell;
  readonly toDirection: 'north' | 'east' | 'south' | 'west';
  readonly toWidth: number;
  readonly routePoints?: readonly PlacementCell[];
}

export interface PlacementInstance {
  readonly instanceId: string;
  readonly sourceRefs: readonly string[];
}

export interface PlacementGatePortal {
  readonly id: string;
  readonly sourceEdge: string;
  readonly sourceCorridor: string;
  readonly linkId: string;
  readonly fromPiece: string;
  readonly fromInstance: string;
  readonly toPiece: string;
  readonly toInstance: string;
  readonly cells: readonly PlacementCell[];
  readonly orientation: 'north' | 'east' | 'south' | 'west';
  readonly width: number;
  readonly traversal: string;
  readonly requiredItem: string | null;
  readonly provenance: readonly string[];
}

export interface PiecePlacementForExtrusion {
  readonly kind: string;
  readonly placementId: string;
  readonly corridorRealization?: 'catalog' | 'hybrid' | 'procedural';
  readonly gridConnectivity: 'four_way' | 'eight_way';
  readonly placementPolicy: PiecePlacementPolicy;
  readonly instances?: readonly PlacementInstance[];
  readonly occupiedCells: readonly PlacementOwnedCell[];
  readonly connectionCells: readonly PlacementOwnedCell[];
  readonly reservedCells: readonly PlacementOwnedCell[];
  readonly gluedExits: readonly PlacementGluedExit[];
  readonly gatePortals: readonly PlacementGatePortal[];
}

export interface VoxelExtrusionOptions {
  readonly chunkSize: number;
  readonly floorY: number;
  readonly wallMinY: number;
  readonly wallMaxY: number;
  readonly ceilingY: number;
  readonly floorMaterial: number;
  readonly wallMaterial: number;
  readonly ceilingMaterial: number;
}

export interface VoxelExtrusionPlan {
  readonly placementId: string;
  readonly coordinateMapping: 'placement_x_y_to_voxel_x_z';
  readonly solidVoxels: readonly {
    readonly coord: VoxelCoord;
    readonly material: number;
  }[];
  readonly walkableCellCount: number;
  readonly openingCellCount: number;
  readonly boundaryCellCount: number;
  readonly solidVoxelCount: number;
  readonly residentChunkCount: number;
  readonly doorPortals: readonly {
    readonly id: string;
    readonly sourceEdge: string;
    readonly requiredItem: string | null;
    readonly traversal: string;
    readonly orientation: 'north' | 'east' | 'south' | 'west';
    readonly cells: readonly PlacementCell[];
    readonly minY: number;
    readonly maxExclusiveY: number;
  }[];
  readonly buildBounds: {
    readonly min: VoxelCoord;
    readonly maxExclusive: VoxelCoord;
  };
}

const DEFAULT_OPTIONS: VoxelExtrusionOptions = {
  chunkSize: 2,
  floorY: 0,
  wallMinY: 1,
  wallMaxY: 3,
  ceilingY: 4,
  floorMaterial: 2,
  wallMaterial: 1,
  ceilingMaterial: 3,
};

interface MutableVoxel {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly material: number;
}

export function compilePlacementExtrusion(
  placement: PiecePlacementForExtrusion,
  overrides: Partial<VoxelExtrusionOptions> = {},
): VoxelExtrusionPlan {
  validatePlacement(placement);
  const options = { ...DEFAULT_OPTIONS, ...overrides };
  validateOptions(options);

  const occupiedByCell = ownedCellsByPosition(placement.occupiedCells, 'occupied');
  const reservedByCell = ownedCellsByPosition(placement.reservedCells, 'reserved');
  const contactSections = catalogContactSections(placement);
  validateOwnedClearance(
    placement,
    occupiedByCell,
    placement.placementPolicy,
    contactSections,
  );
  const opening = declaredOpeningCells(
    placement,
    occupiedByCell,
    reservedByCell,
    contactSections,
  );
  const walkable = new Map<string, PlacementCell>();
  for (const cell of placement.occupiedCells) {
    walkable.set(cellKey(cell.x, cell.y), cell);
  }
  for (const cell of opening.values()) {
    walkable.set(cellKey(cell.x, cell.y), cell);
  }
  if (walkable.size === 0) {
    throw new Error('piece placement has no occupied or connection cells to extrude');
  }

  const boundary = buildWallShell(
    walkable,
    placement.placementPolicy.wallThicknessCells,
  );

  const solids = new Map<string, MutableVoxel>();
  for (const cell of walkable.values()) {
    setSolid(solids, cell.x, options.floorY, cell.y, options.floorMaterial);
    setSolid(solids, cell.x, options.ceilingY, cell.y, options.ceilingMaterial);
  }
  for (const cell of boundary.values()) {
    for (let y = options.wallMinY; y <= options.wallMaxY; y += 1) {
      setSolid(solids, cell.x, y, cell.y, options.wallMaterial);
    }
  }

  const sortedSolids = [...solids.values()].sort(compareVoxel);
  const chunks = requiredChunks(sortedSolids, options.chunkSize);

  return {
    placementId: placement.placementId,
    coordinateMapping: 'placement_x_y_to_voxel_x_z',
    solidVoxels: sortedSolids.map((voxel) => ({
      coord: { x: voxel.x, y: voxel.y, z: voxel.z },
      material: voxel.material,
    })),
    walkableCellCount: walkable.size,
    openingCellCount: opening.size,
    boundaryCellCount: boundary.size,
    solidVoxelCount: sortedSolids.length,
    residentChunkCount: chunks.length,
    doorPortals: placement.gatePortals.map((portal) => ({
      id: portal.id,
      sourceEdge: portal.sourceEdge,
      requiredItem: portal.requiredItem,
      traversal: portal.traversal,
      orientation: portal.orientation,
      cells: portal.cells,
      minY: options.wallMinY,
      maxExclusiveY: options.wallMaxY + 1,
    })),
    buildBounds: boundsFor(sortedSolids),
  };
}

function validatePlacement(placement: PiecePlacementForExtrusion): void {
  if (placement.kind !== 'rusty_procgen.piece_placement.v1') {
    throw new Error(`unsupported placement kind: ${placement.kind}`);
  }
  if (
    placement.corridorRealization !== undefined
    && placement.corridorRealization !== 'catalog'
    && placement.corridorRealization !== 'hybrid'
    && placement.corridorRealization !== 'procedural'
  ) {
    throw new Error(`unsupported corridor realization: ${String(placement.corridorRealization)}`);
  }
  if (placement.gridConnectivity !== 'four_way') {
    throw new Error(`first voxel extrusion proof requires four_way connectivity, got ${placement.gridConnectivity}`);
  }
  validatePlacementPolicy(placement.placementPolicy);
  if (!Array.isArray(placement.gluedExits)) {
    throw new Error('piece placement gluedExits must be an array');
  }
  if (!Array.isArray(placement.gatePortals)) {
    throw new Error('piece placement gatePortals must be an array');
  }
  const portalIds = new Set<string>();
  const sourceEdges = new Set<string>();
  for (const portal of placement.gatePortals) {
    if (portalIds.has(portal.id) || sourceEdges.has(portal.sourceEdge)) {
      throw new Error(`duplicate verified gate portal ${portal.id} or source edge ${portal.sourceEdge}`);
    }
    portalIds.add(portal.id);
    sourceEdges.add(portal.sourceEdge);
    if (!Number.isInteger(portal.width) || portal.width <= 0 || portal.cells.length !== portal.width) {
      throw new Error(`gate portal ${portal.id} must provide exactly one cell per width unit`);
    }
    if (!portal.cells.every((cell: PlacementCell) => Number.isInteger(cell.x) && Number.isInteger(cell.y))) {
      throw new Error(`gate portal ${portal.id} contains a non-integral placement cell`);
    }
  }
}

function validatePlacementPolicy(policy: PiecePlacementPolicy): void {
  if (policy?.schemaVersion !== 1) {
    throw new Error(`unsupported placement policy schema: ${String(policy?.schemaVersion)}`);
  }
  if (!Number.isInteger(policy.minimumClearanceCells) || policy.minimumClearanceCells < 0) {
    throw new Error('placement policy minimumClearanceCells must be a non-negative integer');
  }
  if (policy.contactPolicy !== 'glued_exits_only') {
    throw new Error(`unsupported placement contact policy: ${String(policy.contactPolicy)}`);
  }
  if (!Number.isInteger(policy.wallThicknessCells) || policy.wallThicknessCells <= 0) {
    throw new Error('placement policy wallThicknessCells must be a positive integer');
  }
  if (policy.minimumClearanceCells < policy.wallThicknessCells * 2 + 1) {
    throw new Error(
      'placement policy minimumClearanceCells must be at least twice wallThicknessCells plus one',
    );
  }
  if (
    !Number.isInteger(policy.doorwayWidthCells)
    || policy.doorwayWidthCells <= 0
    || policy.doorwayWidthCells % 2 === 0
  ) {
    throw new Error('placement policy doorwayWidthCells must be a positive odd integer');
  }
  if (policy.doorwayWidthCells !== 1) {
    throw new Error(
      'placement policy schema 1 supports doorwayWidthCells=1 only; wider openings require oriented placement routing',
    );
  }
  if (policy.preservePieceBoundaries !== true) {
    throw new Error('placement policy schema 1 requires preservePieceBoundaries=true');
  }
}

function validateOptions(options: VoxelExtrusionOptions): void {
  if (!Number.isInteger(options.chunkSize) || options.chunkSize <= 0) {
    throw new Error('chunkSize must be a positive integer');
  }
  if (options.wallMinY > options.wallMaxY) {
    throw new Error('wallMinY must be less than or equal to wallMaxY');
  }
  if (options.floorY >= options.wallMinY || options.ceilingY <= options.wallMaxY) {
    throw new Error('floor, wall, and ceiling heights must form a non-overlapping enclosure');
  }
}

function ownedCellsByPosition(
  cells: readonly PlacementOwnedCell[],
  kind: 'occupied' | 'reserved',
): ReadonlyMap<string, PlacementOwnedCell> {
  const byCell = new Map<string, PlacementOwnedCell>();
  for (const cell of cells) {
    if (!Number.isInteger(cell.x) || !Number.isInteger(cell.y)) {
      throw new Error(`piece placement ${kind} coordinates must be integers`);
    }
    if (typeof cell.instanceId !== 'string' || cell.instanceId.length === 0) {
      throw new Error(`piece placement ${kind} cell ${cell.x},${cell.y} has no instance owner`);
    }
    const key = cellKey(cell.x, cell.y);
    const existing = byCell.get(key);
    if (existing !== undefined) {
      throw new Error(
        `piece placement ${kind} cell ${key} is shared by ${existing.instanceId} and ${cell.instanceId}`,
      );
    }
    byCell.set(key, cell);
  }
  return byCell;
}

function validateOwnedClearance(
  placement: PiecePlacementForExtrusion,
  occupiedByCell: ReadonlyMap<string, PlacementOwnedCell>,
  policy: PiecePlacementPolicy,
  contactSections: ReadonlyMap<string, ReadonlySet<string>>,
): void {
  const clearance = policy.minimumClearanceCells;
  for (const cell of occupiedByCell.values()) {
    for (let dy = -clearance; dy <= clearance; dy += 1) {
      for (let dx = -clearance; dx <= clearance; dx += 1) {
        const distance = Math.abs(dx) + Math.abs(dy);
        if (distance === 0 || distance > clearance) {
          continue;
        }
        const other = occupiedByCell.get(cellKey(cell.x + dx, cell.y + dy));
        if (other !== undefined && other.instanceId !== cell.instanceId) {
          const sameOwnedSection = placement.corridorRealization !== 'procedural'
            && [...contactSections.values()].some((instances) =>
              instances.has(cell.instanceId) && instances.has(other.instanceId));
          if (sameOwnedSection) {
            continue;
          }
          throw new Error(
            `piece boundary clearance ${clearance} violated by ${cell.instanceId} and ${other.instanceId}`,
          );
        }
      }
    }
  }
}

function catalogContactSections(
  placement: PiecePlacementForExtrusion,
): ReadonlyMap<string, ReadonlySet<string>> {
  const sections = new Map<string, Set<string>>();
  for (const instance of placement.instances ?? []) {
    for (const sourceRef of instance.sourceRefs) {
      if (!sourceRef.startsWith('physicalSection:')) {
        continue;
      }
      const section = sourceRef.slice('physicalSection:'.length);
      const instances = sections.get(section) ?? new Set<string>();
      instances.add(instance.instanceId);
      sections.set(section, instances);
    }
  }
  for (const glued of placement.gluedExits) {
    if (typeof glued.sourceSection !== 'string' || glued.sourceSection.length === 0) {
      continue;
    }
    const instances = sections.get(glued.sourceSection) ?? new Set<string>();
    instances.add(glued.fromInstance);
    instances.add(glued.toInstance);
    sections.set(glued.sourceSection, instances);
  }
  return sections;
}

function declaredOpeningCells(
  placement: PiecePlacementForExtrusion,
  occupiedByCell: ReadonlyMap<string, PlacementOwnedCell>,
  reservedByCell: ReadonlyMap<string, PlacementOwnedCell>,
  contactSections: ReadonlyMap<string, ReadonlySet<string>>,
): ReadonlyMap<string, PlacementCell> {
  const openingsByOwner = new Map<string, PlacementGluedExit>();
  for (const glued of placement.gluedExits) {
    if (
      typeof glued.id !== 'string'
      || glued.id.length === 0
      || typeof glued.fromInstance !== 'string'
      || glued.fromInstance.length === 0
      || typeof glued.toInstance !== 'string'
      || glued.toInstance.length === 0
      || !validCell(glued.fromCell)
      || !validCell(glued.toCell)
      || !validDirection(glued.fromDirection)
      || !validDirection(glued.toDirection)
      || glued.fromWidth !== 1
      || glued.toWidth !== 1
    ) {
      throw new Error('piece placement glued exits require valid width-1 transformed endpoints');
    }
    if (
      (placement.corridorRealization ?? 'hybrid') !== 'procedural'
      && (glued.routePoints?.length ?? 0) < 2
      && oppositeDirection(glued.fromDirection) !== glued.toDirection
    ) {
      throw new Error(`piece placement glued exit ${glued.id} has incompatible directions`);
    }
    validateEndpointGeometry(glued.id, glued.fromInstance, glued.fromCell, glued.fromDirection, occupiedByCell);
    validateEndpointGeometry(glued.id, glued.toInstance, glued.toCell, glued.toDirection, occupiedByCell);
    const owner = `connection.${slugifyLabel(glued.id)}`;
    if (openingsByOwner.has(owner)) {
      throw new Error(`piece placement has duplicate routed opening owner ${owner}`);
    }
    openingsByOwner.set(owner, glued);
  }

  const openings = new Map<string, PlacementCell>();
  if (placement.corridorRealization === 'catalog') {
    if (placement.connectionCells.length !== 0) {
      throw new Error('pure catalog placement must not contain generated connection cells');
    }
    for (const glued of placement.gluedExits) {
      const distance = Math.abs(glued.fromCell.x - glued.toCell.x)
        + Math.abs(glued.fromCell.y - glued.toCell.y);
      const fromOwner = occupiedByCell.get(cellKey(glued.fromCell.x, glued.fromCell.y));
      const toOwner = occupiedByCell.get(cellKey(glued.toCell.x, glued.toCell.y));
      if (
        distance !== 1
        || oppositeDirection(glued.fromDirection) !== glued.toDirection
        || fromOwner?.instanceId !== glued.toInstance
        || toOwner?.instanceId !== glued.fromInstance
      ) {
        throw new Error(`pure catalog glue ${glued.id} is not an exact adjacent prefab-port join`);
      }
    }
    return openings;
  }
  const routeCellsByOwner = new Map<string, Map<string, PlacementCell>>();
  for (const cell of placement.connectionCells) {
    if (!Number.isInteger(cell.x) || !Number.isInteger(cell.y)) {
      throw new Error('piece placement connection coordinates must be integers');
    }
    const glued = openingsByOwner.get(cell.instanceId);
    if (glued === undefined) {
      throw new Error(`connection cell ${cell.x},${cell.y} is not owned by a declared glued exit`);
    }
    const key = cellKey(cell.x, cell.y);
    const route = routeCellsByOwner.get(cell.instanceId) ?? new Map<string, PlacementCell>();
    if (route.has(key)) {
      throw new Error(`connection ${cell.instanceId} repeats route cell ${key}`);
    }
    route.set(key, cell);
    routeCellsByOwner.set(cell.instanceId, route);
    if (occupiedByCell.has(key)) {
      throw new Error(`doorway ${glued.id} crosses occupied cell ${key}`);
    }
    const reserved = reservedByCell.get(key);
    const declaredEndpointReservation = reserved !== undefined && (
      (sameCell(cell, glued.fromCell) && reserved.instanceId === glued.fromInstance)
      || (sameCell(cell, glued.toCell) && reserved.instanceId === glued.toInstance)
    );
    if (reserved !== undefined && !declaredEndpointReservation) {
      throw new Error(`doorway ${glued.id} crosses reservation ${reserved.instanceId} at ${key}`);
    }
    validateOpeningWallClearance(
      cell,
      glued,
      occupiedByCell,
      placement.placementPolicy.wallThicknessCells,
      placement.corridorRealization === 'hybrid'
        ? contactSections.get(glued.sourceSection ?? '')
        : undefined,
    );
    openings.set(key, cell);
  }
  for (const [owner, glued] of openingsByOwner) {
    const route = routeCellsByOwner.get(owner);
    if (route === undefined) {
      throw new Error(`declared glued exit ${owner} has no routed connection cells`);
    }
    const fromKey = cellKey(glued.fromCell.x, glued.fromCell.y);
    const toKey = cellKey(glued.toCell.x, glued.toCell.y);
    if (!route.has(fromKey) || !route.has(toKey)) {
      throw new Error(`declared glued exit ${owner} route does not include both transformed exit cells`);
    }
    const sameSectionInstances = placement.corridorRealization === 'hybrid'
      ? contactSections.get(glued.sourceSection ?? '')
      : undefined;
    const traversable = new Map(route);
    if (sameSectionInstances !== undefined) {
      for (const occupied of occupiedByCell.values()) {
        if (sameSectionInstances.has(occupied.instanceId)) {
          traversable.set(cellKey(occupied.x, occupied.y), occupied);
        }
      }
    }
    const reachable = new Set<string>([fromKey]);
    const queue = [glued.fromCell];
    while (queue.length > 0) {
      const position = queue.shift();
      if (position === undefined) break;
      for (const neighbor of cardinalNeighbors(position)) {
        const neighborKey = cellKey(neighbor.x, neighbor.y);
        if (traversable.has(neighborKey) && !reachable.has(neighborKey)) {
          reachable.add(neighborKey);
          queue.push(neighbor);
        }
      }
    }
    if (!reachable.has(toKey) || [...route.keys()].some((key) => !reachable.has(key))) {
      throw new Error(`declared glued exit ${owner} route is disconnected`);
    }
  }
  return openings;
}

function validateOpeningWallClearance(
  opened: PlacementCell,
  glued: PlacementGluedExit,
  occupiedByCell: ReadonlyMap<string, PlacementOwnedCell>,
  wallThickness: number,
  sameSectionInstances: ReadonlySet<string> | undefined,
): void {
  for (let dy = -wallThickness; dy <= wallThickness; dy += 1) {
    for (let dx = -wallThickness; dx <= wallThickness; dx += 1) {
      if (Math.abs(dx) + Math.abs(dy) > wallThickness) {
        continue;
      }
      const occupied = occupiedByCell.get(cellKey(opened.x + dx, opened.y + dy));
      if (occupied === undefined) continue;
      const allowed = occupied.instanceId === glued.fromInstance
        ? endpointTunnelContains(opened, glued.fromCell, glued.fromDirection, wallThickness)
        : occupied.instanceId === glued.toInstance
          ? endpointTunnelContains(opened, glued.toCell, glued.toDirection, wallThickness)
          : sameSectionInstances?.has(occupied.instanceId) === true;
      if (!allowed) {
        throw new Error(
          `doorway ${glued.id} enters non-exit wall clearance of piece ${occupied.instanceId}`,
        );
      }
    }
  }
}

function validateEndpointGeometry(
  gluedId: string,
  instanceId: string,
  exit: PlacementCell,
  direction: PlacementGluedExit['fromDirection'],
  occupiedByCell: ReadonlyMap<string, PlacementOwnedCell>,
): void {
  const vector = directionVector(direction);
  const inside = occupiedByCell.get(cellKey(exit.x - vector.x, exit.y - vector.y));
  if (inside?.instanceId !== instanceId) {
    throw new Error(
      `glued exit ${gluedId} endpoint ${exit.x},${exit.y} is not on the declared ${direction} boundary of ${instanceId}`,
    );
  }
}

function endpointTunnelContains(
  position: PlacementCell,
  exit: PlacementCell,
  direction: PlacementGluedExit['fromDirection'],
  wallThickness: number,
): boolean {
  const vector = directionVector(direction);
  for (let step = 0; step < wallThickness; step += 1) {
    if (position.x === exit.x + vector.x * step && position.y === exit.y + vector.y * step) {
      return true;
    }
  }
  return false;
}

function directionVector(direction: PlacementGluedExit['fromDirection']): PlacementCell {
  switch (direction) {
    case 'north': return { x: 0, y: -1 };
    case 'east': return { x: 1, y: 0 };
    case 'south': return { x: 0, y: 1 };
    case 'west': return { x: -1, y: 0 };
  }
}

function oppositeDirection(
  direction: PlacementGluedExit['fromDirection'],
): PlacementGluedExit['fromDirection'] {
  switch (direction) {
    case 'north': return 'south';
    case 'east': return 'west';
    case 'south': return 'north';
    case 'west': return 'east';
  }
}

function validCell(cell: PlacementCell | null | undefined): cell is PlacementCell {
  return cell !== undefined && cell !== null && Number.isInteger(cell.x) && Number.isInteger(cell.y);
}

function validDirection(value: string | undefined): value is PlacementGluedExit['fromDirection'] {
  return value === 'north' || value === 'east' || value === 'south' || value === 'west';
}

function sameCell(left: PlacementCell, right: PlacementCell): boolean {
  return left.x === right.x && left.y === right.y;
}

function buildWallShell(
  walkable: ReadonlyMap<string, PlacementCell>,
  thickness: number,
): ReadonlyMap<string, PlacementCell> {
  const boundary = new Map<string, PlacementCell>();
  let frontier = [...walkable.values()];
  for (let layer = 0; layer < thickness; layer += 1) {
    const next = new Map<string, PlacementCell>();
    for (const cell of frontier) {
      for (const neighbor of cardinalNeighbors(cell)) {
        const key = cellKey(neighbor.x, neighbor.y);
        if (!walkable.has(key) && !boundary.has(key)) {
          boundary.set(key, neighbor);
          next.set(key, neighbor);
        }
      }
    }
    frontier = [...next.values()];
  }
  return boundary;
}

function slugifyLabel(label: string): string {
  const slug = label
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
  return slug.length === 0 ? 'fork' : slug;
}

function cardinalNeighbors(cell: PlacementCell): readonly PlacementCell[] {
  return [
    { x: cell.x + 1, y: cell.y },
    { x: cell.x - 1, y: cell.y },
    { x: cell.x, y: cell.y + 1 },
    { x: cell.x, y: cell.y - 1 },
  ];
}

function setSolid(
  solids: Map<string, MutableVoxel>,
  x: number,
  y: number,
  z: number,
  material: number,
): void {
  solids.set(voxelKey(x, y, z), { x, y, z, material });
}

function requiredChunks(solids: readonly MutableVoxel[], chunkSize: number): VoxelCoord[] {
  const chunks = new Map<string, VoxelCoord>();
  for (const voxel of solids) {
    const chunk = {
      x: floorDiv(voxel.x, chunkSize),
      y: floorDiv(voxel.y, chunkSize),
      z: floorDiv(voxel.z, chunkSize),
    };
    chunks.set(voxelKey(chunk.x, chunk.y, chunk.z), chunk);
  }
  return [...chunks.values()].sort(compareVoxel);
}

function boundsFor(solids: readonly MutableVoxel[]): VoxelExtrusionPlan['buildBounds'] {
  const first = solids[0];
  if (first === undefined) {
    throw new Error('voxel extrusion requires at least one solid');
  }
  let minX = first.x;
  let minY = first.y;
  let minZ = first.z;
  let maxX = first.x;
  let maxY = first.y;
  let maxZ = first.z;
  for (let index = 1; index < solids.length; index += 1) {
    const voxel = solids[index]!;
    minX = Math.min(minX, voxel.x);
    minY = Math.min(minY, voxel.y);
    minZ = Math.min(minZ, voxel.z);
    maxX = Math.max(maxX, voxel.x);
    maxY = Math.max(maxY, voxel.y);
    maxZ = Math.max(maxZ, voxel.z);
  }
  return {
    min: {
      x: minX,
      y: minY,
      z: minZ,
    },
    maxExclusive: {
      x: maxX + 1,
      y: maxY + 1,
      z: maxZ + 1,
    },
  };
}

function floorDiv(value: number, divisor: number): number {
  return Math.floor(value / divisor);
}

function compareVoxel(left: VoxelCoord, right: VoxelCoord): number {
  return left.x - right.x || left.y - right.y || left.z - right.z;
}

function cellKey(x: number, y: number): string {
  return `${x},${y}`;
}

function voxelKey(x: number, y: number, z: number): string {
  return `${x},${y},${z}`;
}
