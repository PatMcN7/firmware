-- Load with extensions.load('daqApp') in BeamNG's GE Lua console.
-- Follows the player vehicle, including switches, respawns, and level changes.
local M = {}
local port = 4445
local rate = 30
local attachedId
local poll = 0

local function detach()
  if attachedId then
    local vehicle = be:getObjectByID(attachedId)
    if vehicle then
      vehicle:queueLuaCommand('if extensions.daqAppTelemetry then extensions.daqAppTelemetry.stop() end')
    end
  end
  attachedId = nil
end

local function attach()
  local vehicle = be:getPlayerVehicle(0)
  if not vehicle then detach(); return end
  local id = vehicle:getID()
  if id == attachedId then return end
  detach()
  vehicle:queueLuaCommand(string.format(
    "extensions.load('daqAppTelemetry'); extensions.daqAppTelemetry.start(%d, %d)", port, rate))
  attachedId = id
end

local function configure(newPort, newRate)
  newPort = tonumber(newPort) or 4445
  newRate = tonumber(newRate) or 30
  if newPort ~= math.floor(newPort) or newPort < 1 or newPort > 65535 then return false end
  if newRate ~= math.floor(newRate) or newRate < 1 or newRate > 120 then return false end
  detach()
  port, rate = newPort, newRate
  attach()
  return true
end

local function onUpdate(dt)
  poll = poll + dt
  if poll < 0.5 then return end
  poll = 0
  attach()
end

local function onExtensionLoaded()
  setExtensionUnloadMode(M, 'manual')
  attach()
  log('I', 'daqApp', 'DAQApp telemetry enabled on localhost:' .. port)
end

local function onVehicleSpawned(id)
  if id == attachedId then attachedId = nil end
  -- The periodic poll attaches once this vehicle becomes the player vehicle.
end

M.configure = configure
M.onExtensionLoaded = onExtensionLoaded
M.onExtensionUnloaded = detach
M.onUpdate = onUpdate
M.onVehicleSwitched = attach
M.onVehicleSpawned = onVehicleSpawned
return M
