-- @stage: NetworkSync

-- multiplayer_sync.lua
-- Demonstrates network API usage: emitting events, sending RPCs,
-- and using persistent storage for world state.

local score = 0
local player_count = 0
local sync_interval = 1.0
local sync_timer = 0.0

function on_init()
    -- Load persisted score from world storage
    local saved = aether.storage.get("high_score")
    if saved then
        score = tonumber(saved) or 0
        print("[sync] loaded high score: " .. score)
    else
        print("[sync] no saved score, starting at 0")
    end

    -- Announce server start
    aether.network.emit("server_event", '{"type":"world_start"}')
    print("[sync] world started, broadcasting to clients")
end

function on_network_sync()
    local dt = aether.time.dt
    sync_timer = sync_timer + dt

    if sync_timer >= sync_interval then
        sync_timer = 0.0

        -- Increment score as a demo
        score = score + 1

        -- Persist to world storage
        aether.storage.set("high_score", tostring(score))

        -- Broadcast score update to all clients
        local payload = '{"score":' .. score .. ',"players":' .. player_count .. '}'
        aether.network.emit("score_update", payload)

        -- Send RPC to leaderboard service
        if score % 10 == 0 then
            aether.network.rpc(
                "leaderboard_service",
                "update_score",
                '{"world_id":"demo","score":' .. score .. '}'
            )
            print("[sync] leaderboard updated, score = " .. score)
        end
    end
end

function on_save()
    return { score = score, player_count = player_count }
end

function on_reload(state)
    score = state.score or 0
    player_count = state.player_count or 0
    print("[sync] reloaded, score = " .. score)
end
