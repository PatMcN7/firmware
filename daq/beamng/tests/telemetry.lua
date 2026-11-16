-- Run from repository root: luajit daq/beamng/tests/telemetry.lua
-- Prints one real mod-produced JSON packet on stdout for Rust decoder smoke tests.
local mod = 'daq/beamng/mod/'
local sent = {}
local closed = 0
local lastPacket
local function encode(value)
  if type(value) == 'number' then return tostring(value) end
  if type(value) == 'string' then return string.format('%q', value) end
  local parts = {}
  for key, item in pairs(value) do
    parts[#parts + 1] = encode(key) .. ':' .. encode(item)
  end
  table.sort(parts)
  return '{' .. table.concat(parts, ',') .. '}'
end
jsonEncode = function(packet) lastPacket = packet; return encode(packet) end
log = function() end
socket = {udp = function()
  return {
    settimeout = function(_, timeout) assert(timeout == 0) end,
    sendto = function(_, bytes, ip, port)
      assert(ip == '127.0.0.1' and port == 4445)
      sent[#sent + 1] = bytes
      return #bytes
    end,
    close = function() closed = closed + 1 end
  }
end}
electrics = {values = {rpm = 4500, wheelspeed = 20, throttle = 0.5,
  gearIndex = -1, steering_input = -0.25, brake = 0, oiltemp = 0/0}}
sensors = {gx = 1.2, gy = -2.4, gz = 9.81}
wheels = {wheels = {
  [0] = {name = 'FL', wheelID = 0, hasTire = true, wheelSpeed = 21, angularVelocity = 60, downForce = 3200,
    lastSlip = 0.5, lastSideSlip = 0.2, propulsionTorque = 150,
    brakeSurfaceTemperature = 140, brakeCoreTemperature = 100},
  [1] = {name = 'custom_axle', downForce = 9000}
}}
playerInfo = {firstPlayerSeated = true}
obj = {
  getVelocity = function() return {x = 20, y = 0, z = 0, length = function() return 20 end} end,
  getPositionXYZ = function() return 100, 200, 3 end,
  getRollPitchYaw = function() return 0.1, 0.2, 0.3 end,
  getID = function() return 42 end,
  getWheelAvgTemperature = function(_, id) assert(id == 0); return 353.15 end,
  getWheelCoreTemperature = function(_, id) assert(id == 0); return 343.15 end
}
local telemetry = dofile(mod .. 'lua/vehicle/extensions/daqAppTelemetry.lua')
assert(not telemetry.start(0, 30))
assert(not telemetry.start(4445, 999))
assert(telemetry.start(4445, 30))
telemetry.updateGFX(1/60)
assert(#sent == 0)
telemetry.updateGFX(1/60)
assert(#sent == 1 and lastPacket.version == 1)
assert(lastPacket.signals.rpm == 4500 and lastPacket.signals.gear == -1)
assert(lastPacket.signals.wheel_FL_load_n == 3200)
assert(math.abs(lastPacket.signals.wheel_FL_tire_avg_temp_c - 80) < 1e-6)
assert(math.abs(lastPacket.signals.wheel_FL_tire_core_temp_c - 70) < 1e-6)
assert(lastPacket.signals.wheel_FR_load_n == nil)
assert(lastPacket.signals.oil_temp_c == nil) -- NaN omitted
assert(lastPacket.signals.clutch == nil) -- unavailable, not a made-up zero
local fixture = sent[1]
playerInfo.firstPlayerSeated = false
telemetry.updateGFX(1)
assert(#sent == 1)
playerInfo.firstPlayerSeated = true
telemetry.updateGFX(0)
assert(#sent == 1)
telemetry.updateGFX(1) -- slow frame sends only one packet, never catch-up bursts
assert(#sent == 2)
-- Unavailable APIs, invalid values, and tireless wheels never become fake temperatures.
obj.getWheelAvgTemperature = nil
obj.getWheelCoreTemperature = function() return 0 end
telemetry.onReset()
telemetry.updateGFX(0.04)
assert(lastPacket.signals.wheel_FL_tire_avg_temp_c == nil)
assert(lastPacket.signals.wheel_FL_tire_core_temp_c == nil)
assert(lastPacket.sequence == 1 and lastPacket.sim_time == 0.04)
obj.getWheelAvgTemperature = function() error('unavailable on this vehicle') end
obj.getWheelCoreTemperature = function() return 0/0 end
telemetry.updateGFX(0.04)
assert(lastPacket.signals.wheel_FL_tire_avg_temp_c == nil)
assert(lastPacket.signals.wheel_FL_tire_core_temp_c == nil)
wheels.wheels[0].hasTire = false
obj.getWheelAvgTemperature = function() error('must not query a tireless wheel') end
obj.getWheelCoreTemperature = obj.getWheelAvgTemperature
telemetry.updateGFX(0.04)
assert(lastPacket.signals.wheel_FL_tire_avg_temp_c == nil)
assert(lastPacket.signals.wheel_FL_tire_core_temp_c == nil)
telemetry.onExtensionUnloaded()
telemetry.updateGFX(1)
assert(#sent == 5 and closed == 1)

-- GE loader follows vehicle switches/respawns and tears down the old sender.
local queue = {}
local currentId = 42
local vehicles = {}
for _, id in ipairs({42, 43}) do
  vehicles[id] = {
    getID = function() return id end,
    queueLuaCommand = function(_, command) queue[#queue + 1] = {id, command} end
  }
end
be = {
  getPlayerVehicle = function() return vehicles[currentId] end,
  getObjectByID = function(_, id) return vehicles[id] end
}
setExtensionUnloadMode = function(_, mode) assert(mode == 'manual') end
local ge = dofile(mod .. 'lua/ge/extensions/daqApp.lua')
ge.onExtensionLoaded()
assert(#queue == 1 and queue[1][2]:find('start%(4445, 30%)'))
ge.onUpdate(1)
assert(#queue == 1) -- no repeated extension reloads
currentId = 43
ge.onVehicleSwitched()
assert(#queue == 3 and queue[2][1] == 42 and queue[2][2]:find('stop'))
assert(queue[3][1] == 43)
ge.onVehicleSpawned(43)
ge.onUpdate(1)
assert(#queue == 4)
assert(not ge.configure(0, 30))
assert(ge.configure(4446, 60))
assert(queue[#queue][2]:find('start%(4446, 60%)'))
ge.onExtensionUnloaded()
assert(queue[#queue][2]:find('stop'))
io.write(fixture .. '\n')
