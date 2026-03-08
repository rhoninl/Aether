-- @stage: PrePhysics
-- @reads: Transform
-- @writes: Velocity

-- npc_patrol.lua
-- Demonstrates a simple NPC patrol pattern using the Aether Lua API.
-- The NPC walks between waypoints, pausing briefly at each one.

local npc_id = nil
local waypoints = {
    { x = 5.0, y = 0.0, z = 5.0 },
    { x = 15.0, y = 0.0, z = 5.0 },
    { x = 15.0, y = 0.0, z = 15.0 },
    { x = 5.0, y = 0.0, z = 15.0 },
}
local current_waypoint = 1
local patrol_speed = 3.0
local arrive_threshold = 0.5
local pause_timer = 0.0
local pause_duration = 1.0

function on_init()
    npc_id = aether.entity.spawn("patrol_guard")
    local wp = waypoints[1]
    aether.entity.set_position(npc_id, wp.x, wp.y, wp.z)
    print("[npc_patrol] guard spawned at waypoint 1")
end

function on_tick()
    if npc_id == nil then return end

    local dt = aether.time.dt

    -- If paused at a waypoint, count down
    if pause_timer > 0 then
        pause_timer = pause_timer - dt
        return
    end

    -- Get current position and target waypoint
    local pos = aether.entity.position(npc_id)
    local target = waypoints[current_waypoint]

    -- Calculate distance to target
    local dx = target.x - pos.x
    local dz = target.z - pos.z
    local dist = math.sqrt(dx * dx + dz * dz)

    if dist < arrive_threshold then
        -- Arrived at waypoint — pause and move to next
        pause_timer = pause_duration
        current_waypoint = current_waypoint % #waypoints + 1
        local next_wp = waypoints[current_waypoint]
        print("[npc_patrol] arrived! next waypoint -> ("
            .. next_wp.x .. ", " .. next_wp.z .. ")")
    else
        -- Move toward waypoint
        local nx = dx / dist
        local nz = dz / dist
        local new_x = pos.x + nx * patrol_speed * dt
        local new_z = pos.z + nz * patrol_speed * dt
        aether.entity.set_position(npc_id, new_x, pos.y, new_z)
    end
end

function on_save()
    return {
        current_waypoint = current_waypoint,
        pause_timer = pause_timer,
        npc_id = npc_id,
    }
end

function on_reload(state)
    current_waypoint = state.current_waypoint or 1
    pause_timer = state.pause_timer or 0
    npc_id = state.npc_id
    print("[npc_patrol] reloaded! resuming at waypoint " .. current_waypoint)
end
