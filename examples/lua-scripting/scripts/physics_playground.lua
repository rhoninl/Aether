-- @stage: PostPhysics
-- @reads: Transform, RigidBody, ColliderComponent
-- @writes: Velocity

-- physics_playground.lua
-- Demonstrates physics API usage: spawns objects, applies forces,
-- performs raycasts, and plays sounds on impact.

local objects = {}
local spawn_timer = 0.0
local spawn_interval = 2.0
local max_objects = 10
local launch_force = 50.0

function on_init()
    print("[physics] playground initialized, spawning objects every "
        .. spawn_interval .. "s")

    -- Spawn initial platform
    local platform = aether.entity.spawn("static_platform")
    aether.entity.set_position(platform, 0.0, 0.0, 0.0)
    print("[physics] platform placed at origin")
end

function on_tick()
    local dt = aether.time.dt
    spawn_timer = spawn_timer + dt

    -- Periodically spawn a new physics object
    if spawn_timer >= spawn_interval and #objects < max_objects then
        spawn_timer = 0.0
        spawn_physics_object()
    end

    -- Check each object: if it fell below y=-20, respawn it
    for i, obj in ipairs(objects) do
        local ok, pos = pcall(aether.entity.position, obj.id)
        if ok and pos.y < -20 then
            aether.entity.set_position(obj.id, 0, 10, 0)
            print("[physics] object " .. i .. " respawned (fell off)")
        end
    end
end

function on_post_physics()
    -- After physics step: do a downward raycast from each object
    for i, obj in ipairs(objects) do
        local ok, pos = pcall(aether.entity.position, obj.id)
        if ok then
            local hit = aether.physics.raycast(
                pos.x, pos.y, pos.z,  -- origin
                0, -1, 0,              -- direction (down)
                0.5                    -- max distance
            )
            if hit and not obj.on_ground then
                obj.on_ground = true
                aether.audio.play("impact_thud", 0.6, pos.x, pos.y, pos.z)
            elseif not hit then
                obj.on_ground = false
            end
        end
    end
end

function spawn_physics_object()
    -- Alternate between spheres and cubes
    local template = #objects % 2 == 0 and "physics_sphere" or "physics_cube"
    local id = aether.entity.spawn(template)

    -- Random-ish position above the platform (using tick for variety)
    local tick = aether.time.tick
    local x = math.sin(tick * 0.7) * 5
    local z = math.cos(tick * 0.3) * 5
    aether.entity.set_position(id, x, 15.0, z)

    -- Apply a small sideways force for fun
    local fx = math.cos(tick * 0.5) * launch_force
    local fz = math.sin(tick * 0.9) * launch_force
    aether.physics.apply_force(id, fx, 0, fz)

    table.insert(objects, { id = id, on_ground = false })
    print("[physics] spawned " .. template .. " #" .. #objects
        .. " at (" .. string.format("%.1f", x) .. ", 15, "
        .. string.format("%.1f", z) .. ")")
end
