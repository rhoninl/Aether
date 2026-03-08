-- hello_world.lua
-- The simplest Aether Lua script: spawns an entity and logs a greeting.

function on_init()
    print("[hello_world] script loaded!")

    local npc = aether.entity.spawn("friendly_npc")
    aether.entity.set_position(npc, 5.0, 0.0, 3.0)

    print("[hello_world] spawned NPC at (5, 0, 3), entity id = " .. npc)
end

function on_tick()
    -- Read current frame timing
    local dt = aether.time.dt
    local tick = aether.time.tick

    -- Only print every 60 ticks (~1 second at 60Hz)
    if tick % 60 == 0 then
        print("[hello_world] tick " .. tick .. ", dt = " .. string.format("%.4f", dt) .. "s")
    end
end
