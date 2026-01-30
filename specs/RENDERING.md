# Rendering Specification

## Scene Contents

### Ground Plane
- Flat quad at y=0
- Size: 50×50 units centered at origin
- Color: Dark gray (#404040)
- Optional: grid lines or checkered pattern for spatial reference

### Player Character (Humanoid)
Simple low-poly humanoid made of basic shapes:
```
      [Head]        - Cube 0.4×0.4×0.4 at y=1.6
        |
     [Torso]        - Cube 0.6×0.8×0.3 at y=1.0
      /   \
   [Arms]           - Cubes 0.2×0.6×0.2 at ±0.4 from center
      |
    [Legs]          - Cubes 0.2×0.8×0.2 at ±0.15 from center
```
Total height: ~2.0 units

- Local player color: Blue (#4444FF)
- Other players color: Red (#FF4444)
- Forward direction: -Z in local space (character "looks" toward -Z)

### Static Environment Objects
Scattered around the world for visual reference:
- 4-6 cubes of varying sizes (1-3 units)
- 2-3 wall segments (tall thin boxes)
- Colors: Various grays, browns, greens
- Positions: Spread across the 50×50 area, avoiding center spawn

## Camera

### Third-Person Setup
- Attached to local player
- Distance: 5 units behind player
- Height: 2 units above player pivot
- Look target: Player position + (0, 1, 0)
- Follows player rotation (yaw only)

### Controls
- Mouse X movement → Player yaw rotation
- Mouse captured (hidden, relative mode)
- ESC to release mouse / quit

## Coordinate System
- Right-handed coordinate system
- Y-up
- Player spawns at origin (0, 0, 0)
- Forward is -Z direction

## Render Pipeline
1. Clear to sky blue (#87CEEB)
2. Render ground plane
3. Render static environment objects
4. Render all player characters
5. No shadows, no lighting (flat shaded / solid colors)

## Shader Requirements
Minimal vertex + fragment shader:
- Vertex: MVP transform only
- Fragment: Output solid color (passed as uniform or vertex attribute)
